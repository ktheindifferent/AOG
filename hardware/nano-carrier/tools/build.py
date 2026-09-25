#!/usr/bin/python3
"""Generate the routed KiCad 6 carrier, manufacturing outputs and preview.

Requires Debian packages kicad, kicad-footprints, kicad-symbols,
python3-numpy, python3-cairosvg, xvfb. Run through xvfb-run if headless.
The router is deliberately conservative; KiCad DRC is the release gate.
"""
import argparse
import csv
import heapq
import html
import json
import math
import re
import uuid
from pathlib import Path
import numpy as np
import pcbnew as k
from design import PARTS, TESTPOINTS, HOLES, WIDTH, HEIGHT, NAME, LEFT, RIGHT

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'rev-a'
LIB=Path('/usr/share/kicad/footprints')
SCALE=0.25
NX=NY=321
def pt(x,y): return k.wxPoint(k.FromMM(float(x)),k.FromMM(float(y)))
def size(x,y): return k.wxSize(k.FromMM(float(x)),k.FromMM(float(y)))
def mm(v): return k.ToMM(v)
def pos(p): return (mm(p.x),mm(p.y))
def uid(s):return str(uuid.uuid5(uuid.NAMESPACE_URL,'https://github.com/PixelCoda/AOG/hardware/'+s))
def q(s):return json.dumps(str(s),ensure_ascii=False)

def footprint_name(part):
    if part.kind=='socket15':return 'Connector_PinSocket_2.54mm:PinSocket_1x15_P2.54mm_Vertical'
    if part.kind.startswith('terminal'):
        n=int(part.kind[-1])
        return f'AOG:TerminalBlock_METZ_RT0340{n}HBLC_P3.81_Drill1.1'
    return {'R0805':'Resistor_SMD:R_0805_2012Metric',
            'C0805':'Capacitor_SMD:C_0805_2012Metric',
            'SOT23':'Package_TO_SOT_SMD:SOT-23',
            'LED0805':'LED_SMD:LED_0805_2012Metric'}[part.kind]

def text(board,content,x,y,size=1,angle=0,layer=k.F_SilkS):
    t=k.PCB_TEXT(board);t.SetText(content);t.SetPosition(pt(x,y))
    t.SetTextSize(k.wxSize(k.FromMM(size),k.FromMM(size)));t.SetTextThickness(k.FromMM(.15));t.SetTextAngle(angle*10)
    t.SetLayer(layer);board.Add(t);return t
def line(board,a,b,layer=k.F_SilkS,width=.15):
    d=k.PCB_SHAPE();d.SetShape(k.SHAPE_T_SEGMENT);d.SetStart(pt(*a));d.SetEnd(pt(*b))
    d.SetLayer(layer);d.SetWidth(k.FromMM(width));board.Add(d)

def prepare_library():
    dest=OUT/'AOG.pretty';dest.mkdir(exist_ok=True)
    for n in [2,3,4]:
        name=f'TerminalBlock_METZ_RT0340{n}HBLC_P3.81_Drill1.1'
        # Manufacturer drawing 2025-08-21 pp4/5: body n*pitch-0.11 by
        # 7.3 mm; 0.9x0.5 mm solder tails need a 1.1 mm finished hole.
        # KiCad6's stock footprint uses an undersized 0.7 mm hole.
        items=[f'(footprint "{name}" (version 20211014) (generator pcbnew) (layer "F.Cu")',
               '(descr "METZ Type086; checked against manufacturer 2025-08-21 drawing; 1.1mm finished drills") (attr through_hole)',
               f'(fp_text reference "REF**" (at {(n-1)*3.81/2} -5.2) (layer "F.SilkS") (effects (font (size 1 1) (thickness .15))))',
               f'(fp_text value "{name}" (at 0 5.2) (layer "F.Fab") (effects (font (size 1 1) (thickness .15))))']
        for layer,left,right,top,bottom,width in [('F.SilkS',-1.95,(n-1)*3.81+1.95,-3.8,3.7,.12),
                ('F.Fab',-1.85,(n-1)*3.81+1.85,-3.7,3.6,.1),('F.CrtYd',-2.35,(n-1)*3.81+2.35,-4.2,4.1,.05)]:
            for a,z in [((left,top),(right,top)),((right,top),(right,bottom)),((right,bottom),(left,bottom)),((left,bottom),(left,top))]:
                items.append(f'(fp_line (start {a[0]} {a[1]}) (end {z[0]} {z[1]}) (layer "{layer}") (width {width}))')
        items.append(f'(fp_line (start -1.85 2.7) (end {(n-1)*3.81+1.85} 2.7) (layer "F.Fab") (width .1))')
        for i in range(n):
            items.append(f'(pad "{i+1}" thru_hole {"rect" if i==0 else "circle"} (at {i*3.81} 0) (size 2.2 2.2) (drill 1.1) (layers *.Cu *.Mask))')
        items.append(')');(dest/(name+'.kicad_mod')).write_text('\n'.join(items)+'\n')
    (OUT/'fp-lib-table').write_text('(fp_lib_table\n (lib (name "AOG")(type "KiCad")(uri "${KIPRJMOD}/AOG.pretty")(options "")(descr "AOG manufacturer-checked footprints"))\n)\n')

def new_board():
    b=k.BOARD();b.SetFileName(str(OUT/(NAME+'.kicad_pcb')))
    b.GetDesignSettings().SetCopperLayerCount(2)
    b.GetDesignSettings().m_CopperEdgeClearance=k.FromMM(.5)
    b.GetDesignSettings().m_MinClearance=k.FromMM(.25)
    b.GetDesignSettings().m_TrackMinWidth=k.FromMM(.25)
    b.GetDesignSettings().m_MinThroughDrill=k.FromMM(.3)
    b.GetDesignSettings().m_HoleToHoleMin=k.FromMM(.25)
    b.GetDesignSettings().m_ViasMinSize=k.FromMM(.7)
    default=b.GetDesignSettings().GetNetClasses().GetDefault()
    default.SetClearance(k.FromMM(.25));default.SetTrackWidth(k.FromMM(.3))
    default.SetViaDiameter(k.FromMM(.8));default.SetViaDrill(k.FromMM(.4))
    allnets=sorted({n for p in PARTS for n in p.nets}|{n for _,_,_,n in TESTPOINTS})
    nets={}
    for i,n in enumerate(allnets,1):
        ni=k.NETINFO_ITEM(b,n,i);b.Add(ni);nets[n]=ni
    for p in PARTS:
        lib,fn=footprint_name(p).split(':')
        fp=k.FootprintLoad(str(OUT/'AOG.pretty') if lib=='AOG' else str(LIB/(lib+'.pretty')),fn)
        if not fp:raise RuntimeError('Missing footprint '+fn)
        fp.SetFPID(k.LIB_ID(lib,fn))
        fp.SetReference(p.ref);fp.SetValue(p.value)
        fp.SetOrientationDegrees(p.angle);fp.SetPosition(pt(p.x,p.y))
        fp.SetPath(k.KIID_PATH('/'+uid(p.ref)))
        fp.Value().SetVisible(False)
        fp.Reference().SetTextSize(size(.8,.8));fp.Reference().SetTextThickness(k.FromMM(.13))
        for pad in fp.Pads():
            pad.SetNet(nets[p.nets[int(pad.GetNumber())-1]])
        # Library socket courtyard includes Harwin's 38.5 x 2.5 mm body.
        if p.ref=='C9':fp.Reference().SetPosition(pt(65,53))
        if p.ref=='J10':fp.Reference().SetVisible(False)
        b.Add(fp)
    for i,(x,y) in enumerate(HOLES,1):
        fp=k.FootprintLoad(str(LIB/'MountingHole.pretty'),'MountingHole_3.2mm_M3')
        fp.SetReference('H'+str(i));fp.SetValue('M3 NPTH');fp.SetPosition(pt(x,y))
        fp.Reference().SetVisible(False);fp.Value().SetVisible(False);b.Add(fp)
    for ref,x,y,net in TESTPOINTS:
        fp=k.FootprintLoad(str(LIB/'TestPoint.pretty'),'TestPoint_Pad_D1.5mm')
        fp.SetReference(ref);fp.SetValue(net);fp.SetPosition(pt(x,y));fp.Value().SetVisible(False)
        fp.Reference().SetTextSize(size(.75,.75));fp.Reference().SetTextThickness(k.FromMM(.13))
        if ref=='TP1':fp.Reference().SetPosition(pt(x+3,y))
        for p in fp.Pads():p.SetNet(nets[net])
        b.Add(fp)
    for a,z in [((0,0),(80,0)),((80,0),(80,80)),((80,80),(0,80)),((0,80),(0,0))]:line(b,a,z,k.Edge_Cuts,.05)
    # Outline of the removable module. No populated components under the Nano.
    for a,z in [((30.8,.6),(30.8,45.5)),((30.8,45.5),(49.2,45.5)),((49.2,45.5),(49.2,.6))]:line(b,a,z)
    text(b,'USB',40,2,1.1)
    text(b,'NANO',40,19,1.7);text(b,'5V / ATmega328P',40,22, .8)
    text(b,'AOG',40,29,2.7);text(b,'CARRIER  REV A',40,32, .9)
    text(b,'USB POWER ONLY',40,39, .8)
    text(b,'WATER D6',18,8,1.05);text(b,'WATER D8',18,28,1.05)
    text(b,'OVF 1 / D2',19,49,.95);text(b,'OVF 2 / D4',19,63,.95)
    text(b,'ANALOG A0',62,8,1.05);text(b,'pH OR CO2',62,10,.85)
    text(b,'DHT11 / D7',62,28,.9)
    text(b,'CCS811',64,44,1);text(b,'3.3V ONLY',64,46,.85)
    # Pin labels appear beside the actual connector contacts, not cable colors.
    labels={'J3':['5V','D6','GND'],'J4':['5V','D8','GND'],'J5':['5V','A','GND'],
            'J6':['5V','D7','GND'],'J7':['D2','GND'],'J8':['D4','GND'],
            'J9':['3V3','GND','SDA','SCL']}
    for fp in b.GetFootprints():
        if fp.GetReference() in labels:
            lab=labels[fp.GetReference()]
            for pad in fp.Pads():
                x,y=pos(pad.GetPosition());dx=6.2 if x<40 else -6.2
                text(b,lab[int(pad.GetNumber())-1],x+dx,y,.8)
            fp.Reference().SetVisible(False)
    text(b,'WAKE  ADDR',58.9,67,.8);text(b,'GND   GND',58.9,78,.8)
    text(b,'PWR',44,76,.8)
    # Useful permanent setup information remains readable on the back.
    text(b,'AOG NANO CARRIER / REV A',40,46,1.2,layer=k.B_SilkS)
    text(b,'ROLE: SENSOR KIT  /  DUAL OVF',40,50,1,layer=k.B_SilkS)
    text(b,'D6 + D8: powered DS18B20, 4.7k pull-ups fitted',40,54, .85,layer=k.B_SilkS)
    text(b,'A0: one analog module only. Never a bare pH probe.',40,57,.85,layer=k.B_SilkS)
    text(b,'CCS811: 3V3 breakout, address 0x5A, WAKE low',40,60,.85,layer=k.B_SilkS)
    text(b,'OVF: dry contact closed to GND = safe',40,63,.85,layer=k.B_SilkS)
    text(b,'USB power only. No VIN/external power input.',40,66,.85,layer=k.B_SilkS)
    text(b,'Prototype: verify sensors and supply before use.',40,69,.85,layer=k.B_SilkS)
    text(b,'github.com/PixelCoda/AOG',34,77,1,layer=k.B_SilkS)
    for d in b.GetDrawings():
        if isinstance(d,k.PCB_TEXT) and d.GetLayer()==k.B_SilkS:d.SetMirrored(True)
    return b,nets

class Router:
    """Two-layer grid routing with geometric keep-outs and no hidden jumpers."""
    def __init__(self,b):
        self.b=b; self.geometry=[];self.paths=[];self.drills=[]
        self.pads={}
        for fp in b.GetFootprints():
            for p in fp.Pads():
                x,y=pos(p.GetPosition());sx,sy=pos(p.GetSize())
                if int(round(p.GetOrientationDegrees()))%180:sx,sy=sy,sx
                layers=[0,1] if p.GetAttribute()==k.PAD_ATTRIB_PTH else [0]
                if p.GetAttribute()==k.PAD_ATTRIB_NPTH:layers=[0,1]
                code=p.GetNetCode() if p.GetNetCode() else -1
                if p.GetAttribute() in [k.PAD_ATTRIB_PTH,k.PAD_ATTRIB_NPTH]:
                    self.drills.append((x,y,mm(p.GetDrillSize().x)/2,code))
                self.geometry.append(('rect',layers,code,x,y,sx/2,sy/2))
                if code>0:self.pads.setdefault(code,[]).append((x,y,layers,fp.GetReference(),p.GetNumber()))
        self.codes={n.GetNetname():n.GetNetCode() for n in b.GetNetsByName().values()}

    def masks(self,net,width):
        # Board edge and NPTH mounting-hole copper keep-outs are explicit.
        a=np.zeros((2,NX,NY),dtype=np.bool_);v=a.copy()
        for target,radius in [(a,width/2),(v,.4)]:
            margin=.25+radius+.10
            target[:,:4,:]=True;target[:,-4:,:]=True;target[:,:,:4]=True;target[:,:,-4:]=True
            for kind,layers,code,x,y,rx,ry in self.geometry:
                if code==net:continue
                loX=max(0,math.floor((x-rx-margin)/SCALE));hiX=min(NX,math.ceil((x+rx+margin)/SCALE)+1)
                loY=max(0,math.floor((y-ry-margin)/SCALE));hiY=min(NY,math.ceil((y+ry+margin)/SCALE)+1)
                xx=np.arange(loX,hiX)[:,None]*SCALE;yy=np.arange(loY,hiY)[None,:]*SCALE
                dx=np.maximum(np.abs(xx-x)-rx,0);dy=np.maximum(np.abs(yy-y)-ry,0)
                inside=dx*dx+dy*dy<=margin*margin
                for l in layers:target[l,loX:hiX,loY:hiY]|=inside
            for x,y in HOLES:
                rr=3.2+radius
                xx=np.arange(NX)[:,None]*SCALE;yy=np.arange(NY)[None,:]*SCALE
                target |= ((xx-x)**2+(yy-y)**2<rr**2)[None,:,:]
        v=v[0]|v[1]
        xx=np.arange(NX)[:,None]*SCALE;yy=np.arange(NY)[None,:]*SCALE
        for x,y,radius,code in self.drills:
            v |= ((xx-x)**2+(yy-y)**2 < (radius+.2+.25+.10)**2)
        # A same-net existing plated hole can carry the layer change itself.
        for x,y,radius,code in self.drills:
            if code==net and abs(x/SCALE-round(x/SCALE))<1e-5 and abs(y/SCALE-round(y/SCALE))<1e-5:
                v[int(round(x/SCALE)),int(round(y/SCALE))]=False
        return a, v

    def astar(self,start,end,blocked,via_blocked):
        sx,sy,sl=start;ex,ey,el=end
        sx=int(round(sx/SCALE));sy=int(round(sy/SCALE));ex=int(round(ex/SCALE));ey=int(round(ey/SCALE))
        goals={(l,ex,ey) for l in el}
        costs={};parent={};heap=[]
        for l in sl:
            node=(l,sx,sy);costs[node]=0;heapq.heappush(heap,(abs(ex-sx)+abs(ey-sy),0,node));blocked[l,sx,sy]=False
        for l in el:blocked[l,ex,ey]=False
        while heap:
            _,cost,node=heapq.heappop(heap)
            if cost!=costs[node]:continue
            if node in goals:
                path=[node]
                while node in parent:node=parent[node];path.append(node)
                return list(reversed(path))
            l,x,y=node
            neighbors=[(l,x+1,y,1),(l,x-1,y,1),(l,x,y+1,1),(l,x,y-1,1)]
            if not via_blocked[x,y]:neighbors.append((1-l,x,y,18))
            for nl,xx,yy,step in neighbors:
                if xx<0 or yy<0 or xx>=NX or yy>=NY or blocked[nl,xx,yy]:continue
                new=(nl,xx,yy)
                # Prefer front horizontal and back vertical paths.
                extra=.06 if nl==l and ((l==0 and xx==x) or (l==1 and yy==y)) else 0
                nc=cost+step+extra
                if nc<costs.get(new,1e20):
                    costs[new]=nc;parent[new]=node
                    h=abs(ex-xx)+abs(ey-yy)
                    heapq.heappush(heap,(nc+h,nc,new))
        raise RuntimeError(f'No route: {start} -> {end}')

    def segment(self,a,z,layer,code,width):
        if math.dist(a,z)<.001:return
        tr=k.PCB_TRACK(self.b);tr.SetStart(pt(*a));tr.SetEnd(pt(*z));tr.SetWidth(k.FromMM(width))
        tr.SetLayer(k.F_Cu if layer==0 else k.B_Cu);tr.SetNetCode(code);self.b.Add(tr)
        # Axis-aligned routes; endpoint rounding stubs get conservative boxes.
        x=(a[0]+z[0])/2;y=(a[1]+z[1])/2
        self.geometry.append(('rect',[layer],code,x,y,abs(a[0]-z[0])/2+width/2,abs(a[1]-z[1])/2+width/2))

    def route(self):
        items=[]
        for name,code in self.codes.items():
            pads=self.pads.get(code,[])
            if len(pads)>1:items.append((name,code,pads))
        # Route short/local signals before widely shared rails. Explicit ground
        # tracks ensure continuity even without relying on a copper-zone fill.
        items.sort(key=lambda it:(it[0] in ['GND','+5V','+3V3'],len(it[2]),it[0]))
        for name,code,pads in items:
            width=.5 if name in ['GND','+5V','+3V3'] else .3
            visited=[pads[0]];todo=pads[1:]
            while todo:
                _,i,a,z=min((math.dist(a[:2],z[:2]),i,a,z) for a in visited for i,z in enumerate(todo))
                blocked,vias=self.masks(code,width)
                path=self.astar(a[:3],z[:3],blocked,vias)
                self.segment(a[:2],(path[0][1]*SCALE,path[0][2]*SCALE),path[0][0],code,width)
                start=path[0];prev=path[0];direction=None
                for n in path[1:]:
                    if n[0]!=prev[0]:
                        self.segment((start[1]*SCALE,start[2]*SCALE),(prev[1]*SCALE,prev[2]*SCALE),prev[0],code,width)
                        vx,vy=n[1]*SCALE,n[2]*SCALE
                        existing=any(math.hypot(vx-x,vy-y)<.001 and nc==code for x,y,r,nc in self.drills)
                        if not existing:
                            via=k.PCB_VIA(self.b);via.SetPosition(pt(vx,vy));via.SetWidth(k.FromMM(.8));via.SetDrill(k.FromMM(.4))
                            via.SetViaType(k.VIATYPE_THROUGH);via.SetLayerPair(k.F_Cu,k.B_Cu);via.SetNetCode(code);self.b.Add(via)
                            self.geometry.append(('rect',[0,1],code,vx,vy,.4,.4))
                            self.drills.append((vx,vy,.2,code))
                        start=n;direction=None
                    else:
                        nd=(n[1]-prev[1],n[2]-prev[2])
                        if direction is not None and nd!=direction:
                            self.segment((start[1]*SCALE,start[2]*SCALE),(prev[1]*SCALE,prev[2]*SCALE),prev[0],code,width);start=prev
                        direction=nd
                    prev=n
                self.segment((start[1]*SCALE,start[2]*SCALE),(prev[1]*SCALE,prev[2]*SCALE),prev[0],code,width)
                self.segment((prev[1]*SCALE,prev[2]*SCALE),z[:2],prev[0],code,width)
                self.paths.append({'net':name,'from':a[3:],'to':z[3:],'width_mm':width})
                visited.append(todo.pop(i))
            print('routed',name,len(pads),'pads',flush=True)

def project_file():
    project={'meta':{'filename':NAME+'.kicad_pro','version':1},'board':{'design_settings':{
        'rules':{'min_clearance':.25,'min_track_width':.25,'min_via_diameter':.7,'min_through_hole_diameter':.3,
                 'min_hole_to_hole':.25,'min_copper_edge_clearance':.5,'min_silk_clearance':.1},
        'rule_severities':{'courtyards_overlap':'error','missing_courtyard':'warning'}}},
        'net_settings':{'classes':[{'name':'Default','clearance':.25,'track_width':.3,'via_diameter':.8,'via_drill':.4,'nets':[]}],'version':2}}
    (OUT/(NAME+'.kicad_pro')).write_text(json.dumps(project,indent=2)+'\n')

def export_manufacturing(b):
    dest=OUT/'manufacturing';dest.mkdir(exist_ok=True)
    plot=k.PLOT_CONTROLLER(b);opts=plot.GetPlotOptions();opts.SetOutputDirectory(str(dest))
    opts.SetPlotFrameRef(False);opts.SetUseGerberProtelExtensions(True);opts.SetUseGerberAttributes(True)
    opts.SetUseGerberX2format(True);opts.SetSubtractMaskFromSilk(True)
    # Gerbers, drills and centroid all use the BOTTOM LEFT board corner.
    b.GetDesignSettings().SetAuxOrigin(pt(0,HEIGHT))
    opts.SetUseAuxOrigin(True);opts.SetAutoScale(False);opts.SetScale(1)
    opts.SetPlotReference(True);opts.SetPlotValue(False);opts.SetExcludeEdgeLayer(True)
    for layer,suffix in [(k.F_Cu,'F_Cu'),(k.B_Cu,'B_Cu'),(k.F_Mask,'F_Mask'),(k.B_Mask,'B_Mask'),
                         (k.F_SilkS,'F_Silkscreen'),(k.B_SilkS,'B_Silkscreen'),(k.F_Paste,'F_Paste'),(k.Edge_Cuts,'Edge_Cuts')]:
        plot.SetLayer(layer);plot.OpenPlotfile(suffix,k.PLOT_FORMAT_GERBER,suffix);plot.PlotLayer();plot.ClosePlot()
    drill=k.EXCELLON_WRITER(b);drill.SetFormat(True);drill.SetOptions(False,False,pt(0,HEIGHT),False)
    drill.CreateDrillandMapFilesSet(str(dest),True,False)

def bom(b):
    with (OUT/'bom.csv').open('w',newline='') as f:
        w=csv.writer(f);w.writerow(['Designator','Quantity','Value','Footprint','Manufacturer','Manufacturer Part Number','Assembly','Notes'])
        for p in PARTS:w.writerow([p.ref,1,p.value,footprint_name(p),p.maker,p.mpn,'FIT',p.purpose])
    with (OUT/'positions.csv').open('w',newline='') as f:
        w=csv.writer(f);w.writerow(['Designator','Mid X (mm)','Mid Y (mm)','Layer','Rotation (deg)','Mount','Value'])
        for p in PARTS:
            fp=next(f for f in b.GetFootprints() if f.GetReference()==p.ref)
            # KiCad SMD origins are placement centers. THT socket/terminal
            # center is the midpoint of its pad row; confirm body orientation
            # against the assembly drawing, not an assumed vendor rotation.
            pads=list(fp.Pads());xs=[pos(z.GetPosition())[0] for z in pads];ys=[pos(z.GetPosition())[1] for z in pads]
            if p.kind.startswith('terminal') or p.kind=='socket15':x,y=(min(xs)+max(xs))/2,(min(ys)+max(ys))/2
            else:x,y=p.x,p.y
            w.writerow([p.ref,f'{x:.4f}',f'{HEIGHT-y:.4f}','Top',f'{p.angle:.1f}', 'THT' if p.kind.startswith(('terminal','socket')) else 'SMD',p.value])
    with (OUT/'connectivity.csv').open('w',newline='') as f:
        w=csv.writer(f);w.writerow(['Reference','Pad','Net'])
        for fp in sorted(b.GetFootprints(),key=lambda f:f.GetReference()):
            for pad in sorted(fp.Pads(),key=lambda z:z.GetNumber()):
                if pad.GetNumber():w.writerow([fp.GetReference(),pad.GetNumber(),pad.GetNetname()])

def check_pin_contract(b):
    actual={(f.GetReference(),p.GetNumber()):p.GetNetname() for f in b.GetFootprints() for p in f.Pads()}
    for p in PARTS:
        for n,net in enumerate(p.nets,1):assert actual[p.ref,str(n)]==net,(p.ref,n,net)
    # Independent physical mapping of the existing firmware to USB-up Nano rows.
    for ref,pin,net in [('J2',7,'D6'),('J2',5,'D8'),('J2',6,'D7'),('J2',11,'D2'),('J2',9,'D4'),
                        ('J1',4,'A0'),('J1',8,'SDA_5V'),('J1',9,'SCL_5V'),('J1',12,'+5V'),('J1',14,'GND'),('J2',12,'GND')]:
        assert actual[ref,str(pin)]==net
    assert abs(PARTS[0].x+15.24-PARTS[1].x)<1e-6
    assert dict((p.ref,p.nets) for p in PARTS)['U1']==['GND','+3V3','+5V']
    assert len([p for p in PARTS if p.ref in ['R1','R2'] and p.value=='4.7k'])==2
    assert actual['J1','2']=='NC_3V3' and actual['J1','15']=='NC_VIN'
    print('PASS: pin contracts, pull-ups, socket spacing and regulator mapping',flush=True)

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--reuse',action='store_true');args=ap.parse_args()
    OUT.mkdir(exist_ok=True,parents=True);project_file();prepare_library()
    if args.reuse:b=k.LoadBoard(str(OUT/(NAME+'.kicad_pcb')))
    else:
        b,nets=new_board();router=Router(b);router.route()
        (OUT/'routing.json').write_text(json.dumps(router.paths,indent=2)+'\n')
        k.SaveBoard(str(OUT/(NAME+'.kicad_pcb')),b)
    # Re-load before release checks, so cached in-memory text geometry cannot
    # hide a silk collision that appears when the user opens the saved board.
    k.SaveBoard(str(OUT/(NAME+'.kicad_pcb')),b)
    b=k.LoadBoard(str(OUT/(NAME+'.kicad_pcb')))
    for p in PARTS:
        fp=next(f for f in b.GetFootprints() if f.GetReference()==p.ref)
        lib,name=footprint_name(p).split(':');fp.SetFPID(k.LIB_ID(lib,name))
    check_pin_contract(b);bom(b)
    b.BuildConnectivity()
    k.WriteDRCReport(b,str(OUT/'drc.txt'),k.EDA_UNITS_MILLIMETRES,True)
    print((OUT/'drc.txt').read_text()[-1000:],flush=True)
    report=(OUT/'drc.txt').read_text()
    assert '** Found 0 DRC violations **' in report, 'Do not release: DRC violations remain'
    assert '** Found 0 unconnected pads **' in report, 'Do not release: incomplete routing'
    export_manufacturing(b)
    for fp in b.GetFootprints():
        if fp.GetReference() in {p.ref for p in PARTS}|{t[0] for t in TESTPOINTS}:
            fp.SetPath(k.KIID_PATH('/'+uid(fp.GetReference())))
    k.SaveBoard(str(OUT/(NAME+'.kicad_pcb')),b)

if __name__=='__main__':main()
