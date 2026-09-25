#!/usr/bin/python3
"""Editable schematic, accurate placement preview, and assembly documentation."""
import copy
import csv
import html
import json
import math
import re
from pathlib import Path
import pcbnew as k
import cairosvg
from design import PARTS, HOLES, TESTPOINTS, NAME, WIDTH, HEIGHT, LEFT, RIGHT
from build import ROOT, OUT, uid, q, footprint_name, pos, mm

def parse(source):
    tokens=re.findall(r'"(?:\\.|[^"\\])*"|[()]|[^\s()]+',source)
    stack=[];root=None
    for t in tokens:
        if t=='(':
            n=[]
            if stack:stack[-1].append(n)
            else:root=n
            stack.append(n)
        elif t==')':stack.pop()
        else:stack[-1].append(t)
    assert not stack
    return root
def dump(x):return '('+' '.join(dump(y) if isinstance(y,list) else y for y in x)+')'
def children(x,key):return [y for y in x if isinstance(y,list) and y[0]==key]
def field(x,key):return next(y for y in x if isinstance(y,list) and y[0]==key)
def decode(s):return json.loads(s) if s.startswith('"') else s
def walk(x,key):
    for n in x:
        if isinstance(n,list):
            if n[0]==key:yield n
            yield from walk(n,key)

def schematic():
    libs={}
    cache={}
    def symbol(lib,name):
        if lib=='AOG':
            old='MCP1700x-330xxTT';s=symbol('Regulator_Linear',old);s[1]=q(name)
            for sub in children(s,'symbol'):sub[1]=q(decode(sub[1]).replace(old,name))
            for prop in children(s,'property'):
                if decode(prop[1])=='Value':prop[2]=q('MCP1700T-3302E/TT')
            return s
        if lib not in libs:libs[lib]=parse(Path('/usr/share/kicad/symbols',lib+'.kicad_sym').read_text())
        s=copy.deepcopy(next(s for s in children(libs[lib],'symbol') if decode(s[1])==name))
        parent=children(s,'extends')
        if parent:
            original=decode(parent[0][1]);base=symbol(lib,original)
            # Use parent graphics and electrical pins, child's metadata.
            s=[x for x in s if x not in parent]
            s.extend(copy.deepcopy(x) for x in base if isinstance(x,list) and x[0]=='symbol')
            for sub in children(s,'symbol'):sub[1]=q(decode(sub[1]).replace(original,name))
        return s
    def lookup(p):
        if p.kind.startswith('terminal'):return 'Connector_Generic','Conn_01x0'+p.kind[-1]
        if p.kind=='socket15':return 'Connector_Generic','Conn_01x15'
        if p.kind=='R0805':return 'Device','R'
        if p.kind=='C0805':return 'Device','C'
        if p.kind=='LED0805':return 'Device','LED'
        if p.ref=='U1':return 'AOG','MCP1700T_3302E_TT'
        return 'Transistor_FET','BSS138'
    for p in PARTS:
        lib,name=lookup(p);lid=lib+':'+name
        if lid not in cache:
            s=symbol(lib,name);s[1]=q(lid);cache[lid]=s
    sh=['kicad_sch',['version','20211123'],['generator','eeschema'],['uuid',uid('sheet')],['paper',q('A2')],
        ['title_block',['title',q('AOG Nano Carrier — one PCB, two firmware roles')],['date',q('2026-09-25')],
         ['rev',q('A — prototype')],['company',q('AOG / PixelCoda')]],['lib_symbols',*cache.values()]]
    instances=[]
    for i,p in enumerate(PARTS):
        lib,name=lookup(p);lid=lib+':'+name;s=cache[lid]
        if i<2:x,y=60+i*115,65
        else:
            j=i-2;x=50+(j%7)*77;y=142+(j//7)*43
        if p.kind in ['R0805','C0805'] or p.ref in ['Q1','Q2']:
            # Keep both fields above the pin labels, including long I2C names.
            rx,ry,vx,vy=x+5,y-12,x+5,y-9
        else:rx,ry,vx,vy=x+5,y-8,x+5,y-5
        puid=uid(p.ref)
        inst=['symbol',['lib_id',q(lid)],['at',str(x),str(y),'0'],['unit','1'],['in_bom','yes'],['on_board','yes'],['uuid',puid],
              ['property',q('Reference'),q(p.ref),['id','0'],['at',str(rx),str(ry),'0'],['effects',['font',['size','1.27','1.27']],['justify','left']]],
              ['property',q('Value'),q(p.value),['id','1'],['at',str(vx),str(vy),'0'],['effects',['font',['size','1.27','1.27']],['justify','left']]],
              ['property',q('Footprint'),q(footprint_name(p)),['id','2'],['at',str(x),str(y),'0'],['effects',['font',['size','1.27','1.27']],'hide']],
              ['property',q('Datasheet'),q(''),['id','3'],['at',str(x),str(y),'0'],['effects',['font',['size','1.27','1.27']],'hide']]]
        pins=list(walk(s,'pin'))
        assert len(pins)==len(p.nets),(p.ref,len(pins),len(p.nets))
        for pin in pins:
            n=decode(field(pin,'number')[1]);net=p.nets[int(n)-1]
            a=field(pin,'at');px=x+float(a[1]);py=y-float(a[2]);angle=int(a[3])
            inst.append(['pin',q(n),['uuid',uid(p.ref+'/pin/'+n)]])
            if net.startswith('NC_'):
                sh.append(['no_connect',['at',str(px),str(py)],['uuid',uid(p.ref+'/NC/'+n)]])
                continue
            lx=px-5.08 if angle==0 else px+5.08
            sh.append(['wire',['pts',['xy',str(px),str(py)],['xy',str(lx),str(py)]],['stroke',['width','0'],['type','default'],['color','0','0','0','0']],['uuid',uid(p.ref+'/wire/'+n)]])
            sh.append(['label',q(net),['at',str(lx),str(py),'0'],['effects',['font',['size','1.27','1.27']],['justify','right' if angle==0 else 'left','bottom']],['uuid',uid(p.ref+'/net/'+n)]])
        sh.append(inst)
        instances.append(['path',q('/'+puid),['reference',q(p.ref)],['unit','1'],['value',q(p.value)],['footprint',q(footprint_name(p))]])
    notes=[(300,28,'USB-UP Nano sockets. J1 pin 1 = D13; J2 pin 1 = D12.'),
           (300,35,'15.24 mm between socket rows; 2.54 mm pitch; user supplies classic 5 V Nano.'),
           (300,42,'J3/J4: one externally powered DS18B20 per data pin. Pull-ups fitted.'),
           (300,49,'J5: pH Gravity board OR SEN0219 v1 analog CO2. Never both on one Nano.'),
           (300,56,'J9: 3.3 V breakout only. WAKE/ADDR can use J10 ground terminals.'),
           (300,63,'No Nano VIN, 3V3 or AREF connection. USB supplies the 5 V rail.'),
           (300,70,'R5/C5 filter A0; R6 pulls unplugged cable low. Electrode presence is not detected.'),
           (300,77,'NC marks on unused Nano pins are intentional. Equal net labels connect.'),
           (300,84,'R1/R2 4.7k water; R3/R4 overflow; R7 DHT; R8-R11 I2C pull-ups.'),
           (300,91,'TP1=5V; TP2=GND; TP3=3V3; TP4=A0. Test pads are copper only.'),
           (300,98,'Prototype; refer to validation and bring-up instructions before deployment.')]
    for j,(x,y,note) in enumerate(notes):
        sh.append(['text',q(note),['at',str(x),str(y),'0'],['effects',['font',['size','1.27','1.27']],['justify','left']],['uuid',uid('note'+str(j))]])
    # Copper-only test points are explicit electrical symbols, not assembly BOM parts.
    ts=symbol('Connector','TestPoint');ts[1]=q('Connector:TestPoint')
    next(x for x in sh if isinstance(x,list) and x[0]=='lib_symbols').append(ts)
    for i,(ref,bx,by,net) in enumerate(TESTPOINTS):
        x,y=65+i*60,115;puid=uid(ref)
        pin=next(walk(ts,'pin'));pa=field(pin,'at');px=x+float(pa[1]);py=y-float(pa[2])
        sh.append(['symbol',['lib_id',q('Connector:TestPoint')],['at',str(x),str(y),'0'],['unit','1'],['in_bom','no'],['on_board','yes'],['uuid',puid],
          ['property',q('Reference'),q(ref),['id','0'],['at',str(x+5),str(y-5),'0'],['effects',['font',['size','1.27','1.27']]]],
          ['property',q('Value'),q(net),['id','1'],['at',str(x+3),str(y-2),'0'],['effects',['font',['size','1.27','1.27']],'hide']],
          ['property',q('Footprint'),q('TestPoint:TestPoint_Pad_D1.5mm'),['id','2'],['at',str(x),str(y),'0'],['effects',['font',['size','1.27','1.27']],'hide']],
          ['pin',q('1'),['uuid',uid(ref+'/1')]]])
        sh.append(['label',q(net),['at',str(px),str(py),'0'],['effects',['font',['size','1.27','1.27']],['justify','left','bottom']],['uuid',uid(ref+'/label')]])
        instances.append(['path',q('/'+puid),['reference',q(ref)],['unit','1'],['value',q(net)],['footprint',q('TestPoint:TestPoint_Pad_D1.5mm')]])
    # The mating Nano supplies these nets through passive sockets. Explicit
    # power flags describe that external source to ERC; they are not parts.
    flag=symbol('power','PWR_FLAG');flag[1]=q('power:PWR_FLAG')
    next(x for x in sh if isinstance(x,list) and x[0]=='lib_symbols').append(flag)
    for i,net in enumerate(['+5V','GND'],1):
        x,y=350+i*35,115;ref='#FLG0'+str(i);puid=uid(ref)
        sh.append(['symbol',['lib_id',q('power:PWR_FLAG')],['at',str(x),str(y),'0'],['unit','1'],['in_bom','no'],['on_board','no'],['uuid',puid],
          ['property',q('Reference'),q(ref),['id','0'],['at',str(x),str(y),'0'],['effects',['font',['size','1.27','1.27']],'hide']],
          ['property',q('Value'),q('PWR_FLAG'),['id','1'],['at',str(x),str(y-5),'0'],['effects',['font',['size','1.27','1.27']]]],
          ['pin',q('1'),['uuid',uid(ref+'/1')]]])
        sh.append(['label',q(net),['at',str(x),str(y),'0'],['effects',['font',['size','1.27','1.27']],['justify','left','bottom']],['uuid',uid(ref+'/label')]])
        instances.append(['path',q('/'+puid),['reference',q(ref)],['unit','1'],['value',q('PWR_FLAG')],['footprint',q('')]])
    sh.append(['sheet_instances',['path',q('/'),['page',q('1')]]]);sh.append(['symbol_instances',*instances])
    output=dump(sh)
    (OUT/(NAME+'.kicad_sch')).write_text(output+'\n')
    local=symbol('AOG','MCP1700T_3302E_TT')
    (OUT/'AOG.kicad_sym').write_text(dump(['kicad_symbol_lib',['version','20211014'],['generator','kicad_symbol_editor'],local])+'\n')
    (OUT/'sym-lib-table').write_text('(sym_lib_table (lib (name "AOG")(type "KiCad")(uri "${KIPRJMOD}/AOG.kicad_sym")(options "")(descr "AOG flat regulator symbol")))\n')
    # Ensure generated schematic round-trips through its S-expression grammar.
    assert dump(parse(output))==output

def preview():
    b=k.LoadBoard(str(OUT/(NAME+'.kicad_pcb')))
    out=['<svg xmlns="http://www.w3.org/2000/svg" width="80mm" height="80mm" viewBox="0 0 80 80">',
         '<style>.pcb-label{font-family:DejaVu Sans,sans-serif;text-anchor:middle;dominant-baseline:central}</style>',
         '<rect width="80" height="80" rx=".2" fill="#123e3d"/>']
    for tr in b.GetTracks():
        if isinstance(tr,k.PCB_VIA):
            x,y=pos(tr.GetPosition());out.append(f'<circle cx="{x}" cy="{y}" r=".4" fill="#609986"/><circle cx="{x}" cy="{y}" r=".2" fill="#102626"/>')
        else:
            a=pos(tr.GetStart());z=pos(tr.GetEnd());color='#407b67' if tr.GetLayer()==k.F_Cu else '#225d67'
            out.append(f'<path d="M{a[0]},{a[1]}L{z[0]},{z[1]}" fill="none" stroke="{color}" stroke-width="{mm(tr.GetWidth())}" stroke-linecap="round"/>')
    # Draw component bodies from the physical footprint's fabrication outlines.
    for fp in b.GetFootprints():
        for g in fp.GraphicalItems():
            if isinstance(g,k.FP_SHAPE) and g.GetLayer() in [k.F_Fab,k.F_SilkS]:
                if g.GetShape()==k.SHAPE_T_SEGMENT:
                    a=pos(g.GetStart());z=pos(g.GetEnd())
                    out.append(f'<path d="M{a[0]},{a[1]}L{z[0]},{z[1]}" stroke="#90b5a6" stroke-width=".12"/>')
        for pad in fp.Pads():
            x,y=pos(pad.GetPosition());sx,sy=pos(pad.GetSize());ang=pad.GetOrientationDegrees()
            if pad.GetAttribute()==k.PAD_ATTRIB_NPTH:
                out.append(f'<circle cx="{x}" cy="{y}" r="{mm(pad.GetDrillSize().x)/2}" fill="#f3f1e8"/>');continue
            out.append(f'<rect x="{x-sx/2}" y="{y-sy/2}" width="{sx}" height="{sy}" rx=".13" fill="#c4a568" transform="rotate({-ang} {x} {y})"/>')
            if pad.GetAttribute()==k.PAD_ATTRIB_PTH:
                out.append(f'<circle cx="{x}" cy="{y}" r="{mm(pad.GetDrillSize().x)/2}" fill="#102626"/>')
        if fp.Reference().IsVisible():
            t=fp.Reference();x,y=pos(t.GetPosition());ang=t.GetTextAngle()/10
            out.append(f'<text class="pcb-label" x="{x}" y="{y}" font-size=".8" fill="#ebeadb" transform="rotate({-ang} {x} {y})">{html.escape(t.GetText())}</text>')
    for d in b.GetDrawings():
        if d.GetLayer()!=k.F_SilkS:continue
        if isinstance(d,k.PCB_TEXT):
            x,y=pos(d.GetPosition());out.append(f'<text class="pcb-label" x="{x}" y="{y}" font-size="{mm(d.GetTextSize().x)}" fill="#fffce8">{html.escape(d.GetText())}</text>')
        elif isinstance(d,k.PCB_SHAPE) and d.GetShape()==k.SHAPE_T_SEGMENT:
            a=pos(d.GetStart());z=pos(d.GetEnd());out.append(f'<path d="M{a[0]},{a[1]}L{z[0]},{z[1]}" stroke="#ebeadb" stroke-width=".15"/>')
    out.append('</svg>');svg='\n'.join(out)
    (OUT/'board-top.svg').write_text(svg)
    cairosvg.svg2png(bytestring=svg.encode(),write_to=str(OUT/'board-top.png'),output_width=1440,output_height=1440)
    cairosvg.svg2pdf(bytestring=svg.encode(),write_to=str(OUT/'board-1-to-1.pdf'))
    # Assembly sheet uses the actual PCB preview, and an explicit polarity table.
    sheet=f'''<svg xmlns="http://www.w3.org/2000/svg" width="297mm" height="210mm" viewBox="0 0 297 210">
    <rect width="297" height="210" fill="white"/>
    <g font-family="DejaVu Sans,sans-serif" fill="#172c30">
    <text x="12" y="16" font-size="7">AOG · Nano carrier / Rev A</text>
    <text x="12" y="24" font-size="3.6">Assembly drawing · top view · components on TOP · 80 × 80 mm</text>
    <svg x="12" y="32" width="150" height="150" viewBox="0 0 80 80">{svg[svg.index('>')+1:svg.rindex('</svg>')]}</svg>
    <text x="175" y="40" font-size="4.5">Populate all BOM parts</text>
    <text x="175" y="49" font-size="3.3">J1 + J2: FEMALE sockets, 15 pins each.</text>
    <text x="175" y="56" font-size="3.3">2.54 mm pitch; rows 15.24 mm apart.</text>
    <text x="175" y="63" font-size="3.3">Nano is user supplied; do not solder it.</text>
    <text x="175" y="77" font-size="4">Orientation checks</text>
    <text x="175" y="86" font-size="3.3">Nano USB points toward the top edge.</text>
    <text x="175" y="93" font-size="3.3">J3/4/7/8 wire entries face LEFT.</text>
    <text x="175" y="100" font-size="3.3">J5/6/9 wire entries face RIGHT.</text>
    <text x="175" y="107" font-size="3.3">J10 wire entries face BOTTOM.</text>
    <text x="175" y="121" font-size="4">SOT-23 / LED pin identities</text>
    <text x="175" y="130" font-size="3.3">U1: 1 GND, 2 OUT, 3 IN.</text>
    <text x="175" y="137" font-size="3.3">Q1/Q2: 1 gate, 2 source, 3 drain.</text>
    <text x="175" y="144" font-size="3.3">D1: pad 1 cathode; pad 2 anode.</text>
    <text x="175" y="158" font-size="3.3">TP1–TP4: copper pads, no fitted parts.</text>
    <text x="175" y="165" font-size="3.3">H1–H4: 3.2 mm non-plated holes.</text>
    <text x="12" y="194" font-size="3.5">Prototype design. Use the BOM, centroid file and Gerbers from the same release. See ASSEMBLY.md for testing.</text>
    </g></svg>'''
    (OUT/'assembly.svg').write_text(sheet)
    cairosvg.svg2pdf(bytestring=sheet.encode(),write_to=str(OUT/'assembly.pdf'))

def circuit_reference():
    groups=[('Nano sockets / USB power',['J1','J2']),
            ('Power / 3.3 V regulator',['C1','C2','R12','D1','U1','C7','C8']),
            ('Water probes: one DS18B20 on each bus',['J3','R1','C3','J4','R2','C4']),
            ('Analog input and overflow contacts',['J5','R5','R6','C5','J7','R3','C10','J8','R4','C11']),
            ('Climate / I2C level shifting',['J6','R7','C6','J9','J10','C9','Q1','R8','R9','Q2','R10','R11'])]
    allparts={p.ref:p for p in PARTS}
    pages=[]
    for title,refs in groups:
        s=['<svg xmlns="http://www.w3.org/2000/svg" width="297mm" height="210mm" viewBox="0 0 297 210"><rect width="297" height="210" fill="white"/><g font-family="DejaVu Sans,sans-serif" fill="#172c30">',
           f'<text x="12" y="16" font-size="6">{html.escape(title)}</text>',
           '<text x="12" y="24" font-size="3.3">AOG Nano carrier Rev A · Electrical connection reference · Same net names are electrically connected</text>']
        for i,ref in enumerate(refs):
            p=allparts[ref];x=12+(i%3)*94;y=36+(i//3)*30
            if ref in ['J1','J2']:x=12+i*140;y=36
            h=max(22,8+len(p.nets)*5)
            s.append(f'<rect x="{x}" y="{y}" width="86" height="{h}" rx="2" fill="#edf4f0" stroke="#bccfc8" stroke-width=".3"/>')
            s.append(f'<text x="{x+3}" y="{y+5}" font-size="3.5" font-weight="bold">{p.ref} · {html.escape(p.value)}</text>')
            for n,net in enumerate(p.nets,1):
                desc=net.replace('NC_','unconnected: ')
                s.append(f'<text x="{x+3}" y="{y+6+n*5}" font-size="3.1">Pad {n} → {html.escape(desc)}</text>')
        s.append('<text x="12" y="204" font-size="3">Pin-number reference only; editable KiCad schematic accompanies this drawing. Nano USB end is socket pin 1.</text></g></svg>')
        svg='\n'.join(s);idx=len(pages)+1
        fn=OUT/f'connections-{idx}.svg';fn.write_text(svg);cairosvg.svg2pdf(bytestring=svg.encode(),write_to=str(OUT/f'connections-{idx}.pdf'))
        pages.append(fn)

if __name__=='__main__':
    schematic();preview();circuit_reference()
