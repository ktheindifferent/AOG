#!/usr/bin/python3
"""Validate the saved design and package a two-board factory assembly order."""
import csv
import hashlib
import html
import json
import math
import re
import zipfile
from datetime import datetime,timezone
from pathlib import Path
import pcbnew as k
from design import PARTS, NAME, TESTPOINTS, WIDTH, HEIGHT
from build import ROOT,OUT,footprint_name,check_pin_contract,pos,mm
from documents import parse,children,field,decode,walk

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()

def verify_schematic_connections(schematic):
    """Independently trace actual schematic wire/label coordinates to each pin."""
    parent={}
    def xy(a):return tuple(round(float(x),5) for x in a[1:3])
    def find(a):
        parent.setdefault(a,a)
        if parent[a]!=a:parent[a]=find(parent[a])
        return parent[a]
    def union(a,b):parent[find(a)]=find(b)
    for wire in children(schematic,'wire'):
        points=children(field(wire,'pts'),'xy')
        for a,b in zip(points,points[1:]):union(xy(a),xy(b))
    labels={}
    for label in children(schematic,'label'):
        name=decode(label[1]);point=xy(field(label,'at'))
        if name in labels:union(point,labels[name])
        else:labels[name]=point
    byroot={}
    for name,point in labels.items():
        root=find(point);assert root not in byroot,('schematic shorts named nets',name,byroot.get(root))
        byroot[root]=name
    libs={decode(x[1]):x for x in children(field(schematic,'lib_symbols'),'symbol')}
    expected={p.ref:p.nets for p in PARTS}|{ref:[net] for ref,x,y,net in TESTPOINTS}
    nc={xy(field(x,'at')) for x in children(schematic,'no_connect')}
    traced=[]
    for sym in children(schematic,'symbol'):
        ref=decode(next(p for p in children(sym,'property') if decode(p[1])=='Reference')[2])
        if ref not in expected:continue
        origin=field(sym,'at');x,y=map(float,origin[1:3]);assert float(origin[3])==0
        lib=libs[decode(field(sym,'lib_id')[1])]
        for pin in walk(lib,'pin'):
            n=int(decode(field(pin,'number')[1]));pinat=field(pin,'at')
            point=(round(x+float(pinat[1]),5),round(y-float(pinat[2]),5))
            want=expected[ref][n-1];got=byroot.get(find(point))
            if want.startswith('NC_'):assert point in nc and got is None,(ref,n,want,got)
            else:assert got==want,(ref,n,want,got)
            traced.append([ref,n,want])
    assert len(traced)==sum(len(x) for x in expected.values())
    with (OUT/'schematic-connectivity.csv').open('w',newline='') as f:
        w=csv.writer(f);w.writerow(['Reference','Pin','Verified net']);w.writerows(traced)

def vendor_files():
    with (OUT/'bom-jlc.csv').open('w',newline='') as f:
        w=csv.writer(f);w.writerow(['Comment','Designator','Footprint','LCSC Part #','Manufacturer','Manufacturer Part Number'])
        for p in PARTS:w.writerow([p.value,p.ref,footprint_name(p),'',p.maker,p.mpn])
    with (OUT/'positions.csv').open() as f:positions=list(csv.DictReader(f))
    with (OUT/'positions-jlc.csv').open('w',newline='') as f:
        w=csv.writer(f);w.writerow(['Designator','Mid X','Mid Y','Layer','Rotation'])
        for p in positions:w.writerow([p['Designator'],p['Mid X (mm)']+'mm',p['Mid Y (mm)']+'mm','top',p['Rotation (deg)']])

def interactive_preview(b):
    ports=[]
    labels={'J3':['5V / probe VDD','D6 / probe DATA','GND'],
            'J4':['5V / probe VDD','D8 / probe DATA','GND'],
            'J5':['5V / module +','Analog OUT / Gravity A','GND / module −'],
            'J6':['5V','DHT11 DATA','GND'],'J7':['Overflow contact D2','GND'],
            'J8':['Overflow contact D4','GND'],'J9':['3.3V supply','GND','SDA','SCL'],
            'J10':['GND for WAKE','GND for ADDR']}
    for p in PARTS:
        if not p.kind.startswith('terminal'):continue
        fp=next(f for f in b.GetFootprints() if f.GetReference()==p.ref)
        ps=[pos(pad.GetPosition()) for pad in fp.Pads()];xs=[x for x,y in ps];ys=[y for x,y in ps]
        ports.append({'ref':p.ref,'name':p.value,'note':p.purpose,'pins':labels[p.ref],
                      'roles':['kit','ovf'] if p.ref in ['J3','J4','J5'] else ['ovf'] if p.ref in ['J7','J8'] else ['kit'],
                      'rect':[min(xs)-4.3,min(ys)-2.4,max(xs)-min(xs)+8.6,max(ys)-min(ys)+4.8] if p.angle else [min(xs)-2.4,min(ys)-4.3,max(xs)-min(xs)+4.8,8.6]})
    svg=(OUT/'board-top.svg').read_text().replace('width="80mm" height="80mm"','width="100%" height="100%"')
    overlays=''.join(f'<rect data-port="{p["ref"]}" x="{p["rect"][0]}" y="{p["rect"][1]}" width="{p["rect"][2]}" height="{p["rect"][3]}" rx="1"/>' for p in ports)
    svg=svg.replace('</svg>','<g class="port-outlines">'+overlays+'</g></svg>')
    template='''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>AOG Nano carrier · Rev A</title><style>
*{box-sizing:border-box}body{margin:0;background:#eef2ed;color:#203331;font:16px/1.5 system-ui,sans-serif}main{max-width:1120px;margin:auto;padding:32px 24px}header p{max-width:780px;color:#526662}h1{font-size:clamp(28px,5vw,42px);line-height:1.15;letter-spacing:-.035em;margin:6px 0 16px}h2{font-size:22px;margin:0 0 8px}.eyebrow{font-size:12px;letter-spacing:.18em;text-transform:uppercase}button{font:inherit;cursor:pointer;border:1px solid #b5c8be;border-radius:8px;background:white;color:#203331;padding:10px 16px}button:focus-visible,a:focus-visible{outline:3px solid #be6e1f;outline-offset:3px}button[aria-pressed=true]{background:#174b42;color:white;border-color:#174b42}.roles{display:flex;gap:8px;margin:22px 0}.layout{display:grid;grid-template-columns:minmax(0,1.45fr) minmax(280px,1fr);gap:24px;align-items:start}.board{background:#123e3d;border-radius:14px;overflow:hidden;box-shadow:0 14px 28px #123e3d20}.panel{background:#fff;border:1px solid #d3dfd7;border-radius:14px;padding:22px}.ports{display:flex;flex-wrap:wrap;gap:6px;margin:14px 0 24px}.ports button{font-size:13px;padding:7px 10px}.pins{padding:0;list-style:none}.pins li{padding:9px 0;border-bottom:1px solid #e3e9e4}.badge{font-size:12px;background:#ece5cf;border-radius:6px;padding:5px 9px;display:inline-block}.subtle{font-size:14px;color:#526662}a{color:#175d4b}footer{margin:26px 0;color:#526662;font-size:14px}.port-outlines rect{fill:transparent;stroke:transparent;stroke-width:.6;pointer-events:none}.port-outlines rect.active{stroke:#bad2c3}.port-outlines rect.selected{stroke:#ffca6d;fill:#ffca6d12}#role-note{min-height:44px}@media(max-width:760px){.layout{grid-template-columns:1fr}main{padding:24px 16px}}@media(prefers-reduced-motion:no-preference){.port-outlines rect{transition:stroke .15s}}
</style><main><header><span class="eyebrow">AOG hardware / Revision A</span><h1>One carrier. Two familiar roles.</h1><p>Order two identical assembled boards. Each takes one removable classic 5 V Nano. The connectors match your installed Sensor Kit and Dual OVF firmware.</p><span class="badge">First prototype · physical testing still required</span></header>
<div class="roles" aria-label="Firmware role"><button id="kit" aria-pressed="true">Sensor Kit</button><button id="ovf" aria-pressed="false">Dual OVF</button></div>
<div class="layout"><div class="board" aria-label="Actual top-side PCB layout">__SVG__</div><section class="panel"><h2 id="role-title">Sensor Kit</h2><p id="role-note" class="subtle"></p><div class="ports" aria-label="Select a connector"></div><h2 id="port-title"></h2><p id="port-note"></p><ol class="pins"></ol><p class="subtle">Follow the printed signal names. Pin order and cable colors differ between modules. Disconnect power before moving wires.</p><p><a href="assembly.pdf">Assembly drawing</a> · <a href="board-1-to-1.pdf">Print at actual size</a></p></section></div>
<footer>80 × 80 mm · 2 layers · socket rows 15.24 mm apart · 4.7 kΩ water pull-ups fitted.<br>USB powers each Nano and its carrier. Sensors, Nano modules and cable harnesses are external. This preview represents the PCB layout; the Nano is omitted to show the sockets.</footer>
</main><script>
const ports=__PORTS__;let role='kit',selected='J3';
function render(){for(const key of ['kit','ovf'])document.getElementById(key).setAttribute('aria-pressed',String(key===role));document.getElementById('role-title').textContent=role==='kit'?'Sensor Kit':'Dual OVF';document.getElementById('role-note').textContent=role==='kit'?'Air sensors and analog CO₂, with optional D6/D8 water probes.':'Overflow contacts and Atlas Gravity pH, with optional D6/D8 water probes.';const active=ports.filter(p=>p.roles.includes(role));if(!active.some(p=>p.ref===selected))selected=active[0].ref;const holder=document.querySelector('.ports');holder.replaceChildren();for(const p of active){const button=document.createElement('button');button.textContent=p.ref+' · '+p.name;button.setAttribute('aria-pressed',String(selected===p.ref));button.onclick=()=>{selected=p.ref;render()};holder.append(button)}const p=ports.find(p=>p.ref===selected);document.getElementById('port-title').textContent=p.ref+' · '+p.name;document.getElementById('port-note').textContent=p.ref==='J5'?(role==='kit'?'SEN0219 v1 analog output only. PWM v2 requires a different acquisition path.':'Connect the Atlas Gravity interface here; keep the electrode on the Gravity board.') :p.ref==='J9'?'3.3 V compatible CCS811 breakout, address 0x5A. Confirm its power rating and printed pin names before connecting.':p.note;document.querySelector('.pins').replaceChildren(...p.pins.map((name,i)=>{const li=document.createElement('li');li.textContent='Pin '+(i+1)+' · '+name;return li}));document.querySelectorAll('[data-port]').forEach(el=>{el.classList.toggle('active',active.some(p=>p.ref===el.dataset.port));el.classList.toggle('selected',selected===el.dataset.port)})}
for(const key of ['kit','ovf'])document.getElementById(key).onclick=()=>{role=key;render()};render();
</script></html>'''
    (OUT/'preview.html').write_text(template.replace('__SVG__',svg).replace('__PORTS__',json.dumps(ports)))

def main():
    report=(OUT/'drc.txt').read_text()
    for s in ['Found 0 DRC violations','Found 0 unconnected pads','Found 0 Footprint errors']:assert s in report,s
    erc=(OUT/'erc.txt').read_text()
    assert 'ERC messages: 0  Errors 0  Warnings 0' in erc, 'Run and save KiCad schematic ERC before release'
    erc_proof=json.loads((OUT/'erc-verification.json').read_text())
    assert erc_proof['schematic_sha256']==sha(OUT/(NAME+'.kicad_sch')), 'Re-run ERC after changing the schematic'
    assert erc_proof['report_sha256']==sha(OUT/'erc.txt')
    b=k.LoadBoard(str(OUT/(NAME+'.kicad_pcb')));check_pin_contract(b)
    assert pos(b.GetDesignSettings().GetAuxOrigin())==(0.0,80.0)
    with (OUT/'bom.csv').open() as f:bom=list(csv.DictReader(f))
    with (OUT/'positions.csv').open() as f:positions=list(csv.DictReader(f))
    assert len(bom)==len(positions)==len(PARTS)==37
    assert len({p.mpn for p in PARTS})==14
    assert {p['Designator'] for p in bom}=={p.ref for p in PARTS}=={p['Designator'] for p in positions}
    footprints={f.GetReference():f for f in b.GetFootprints()}
    for p in PARTS:
        f=footprints[p.ref]
        assert str(f.GetFPID().GetLibNickname())+':'+str(f.GetFPID().GetLibItemName())==footprint_name(p)
        pads=list(f.Pads())
        if p.kind.startswith('terminal'):
            assert all(abs(mm(pad.GetDrillSize().x)-1.1)<1e-6 for pad in pads)
        row=next(r for r in positions if r['Designator']==p.ref)
        if row['Mount']=='THT':
            coords=[pos(pad.GetPosition()) for pad in pads]
            x=(min(t[0] for t in coords)+max(t[0] for t in coords))/2
            y=80-(min(t[1] for t in coords)+max(t[1] for t in coords))/2
        else:x,y=p.x,80-p.y
        assert abs(x-float(row['Mid X (mm)']))<.0001 and abs(y-float(row['Mid Y (mm)']))<.0001
    schematic=parse((OUT/(NAME+'.kicad_sch')).read_text())
    verify_schematic_connections(schematic)
    actual={decode(field(s,'property')[2]):s for s in children(schematic,'symbol')}
    for p in PARTS:
        sym=actual[p.ref];fp=next(x for x in children(sym,'property') if decode(x[1])=='Footprint')
        assert decode(fp[2])==footprint_name(p)
    mf=list((OUT/'manufacturing').iterdir())
    assert len(mf)==10
    for ext in ['.gtl','.gbl','.gts','.gbs','.gto','.gbo','.gtp','.gm1']:assert len([p for p in mf if p.suffix==ext])==1
    for p in [p for p in mf if p.suffix=='.drl']:
        coords=re.findall(r'X(-?\d+(?:\.\d+)?)Y(-?\d+(?:\.\d+)?)',p.read_text())
        assert coords and all(0<=float(x)<=80 and 0<=float(y)<=80 for x,y in coords)
    npth=(OUT/'manufacturing'/(NAME+'-NPTH.drl')).read_text()
    assert set(re.findall(r'X([\d.]+)Y([\d.]+)',npth))=={('3.5','3.5'),('3.5','76.5'),('76.5','3.5'),('76.5','76.5')}
    vendor_files();interactive_preview(b)
    validation={'checked_at_utc':datetime.now(timezone.utc).isoformat(),'kicad_version':k.GetBuildVersion(),
        'board_mm':[80,80],'copper_layers':2,'drc_violations':0,'unconnected_pads':0,
        'bom_placements':37,'smd_placements':27,'tht_placements':10,'unique_mpn':14,
        'terminal_finished_drill_mm':1.1,'socket_row_spacing_mm':15.24,
        'origins':'Gerber/drill/centroid: bottom-left, mm, X right, Y up',
        'pin_contract':'pass','schematic_wires_and_labels_to_pins':'pass','footprint_bom_schematic_match':'pass','archive_checks':'pass',
        'physical_hardware_tested':False,'sensor_models_confirmed':False,
        'erc_errors':0,'erc_warnings':0,'erc_verified_schematic_sha256':erc_proof['schematic_sha256'],
        'hashes':{p.name:sha(p) for p in [OUT/(NAME+'.kicad_pcb'),OUT/(NAME+'.kicad_sch'),OUT/'bom.csv',OUT/'positions.csv',OUT/'drc.txt']}}
    (OUT/'validation.json').write_text(json.dumps(validation,indent=2)+'\n')
    gerbers=OUT/'AOG-Nano-Carrier-Rev-A-Gerbers.zip'
    with zipfile.ZipFile(gerbers,'w',zipfile.ZIP_DEFLATED) as z:
        for p in sorted(mf):z.write(p,p.name)
    package=OUT/'AOG-Nano-Carrier-Rev-A-Assembly-Package.zip'
    with zipfile.ZipFile(package,'w',zipfile.ZIP_DEFLATED) as z:
        selected=[gerbers,*[p for p in OUT.iterdir() if p.suffix in ['.csv','.pdf','.svg','.png','.html','.kicad_sch','.kicad_pcb','.kicad_pro']]]
        selected += [OUT/'drc.txt',OUT/'validation.json',OUT/'fp-lib-table',OUT/'sym-lib-table',OUT/'AOG.kicad_sym',OUT/'erc-verification.json']
        selected += [p for p in [OUT/'erc.txt',OUT/'schematic.net'] if p.exists()]
        # Preserve the repository layout so the included README links work
        # after the customer extracts this standalone ordering package.
        for p in selected:z.write(p,'rev-a/'+p.name)
        for p in (OUT/'AOG.pretty').iterdir():z.write(p,'rev-a/AOG.pretty/'+p.name)
        for p in mf:z.write(p,'rev-a/manufacturing/'+p.name)
        for p in ROOT.glob('*.md'):z.write(p,p.name)
    for p in [gerbers,package]:
        with zipfile.ZipFile(p) as z:assert z.testzip() is None
    manifest={str(p.relative_to(OUT)):sha(p) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='SHA256SUMS' and not p.name.endswith(('.kicad_prl','.lck'))}
    (OUT/'SHA256SUMS').write_text(''.join(f'{h}  {name}\n' for name,h in manifest.items()))
    print(json.dumps(validation,indent=2));print('PACKAGED',package)

if __name__=='__main__':main()
