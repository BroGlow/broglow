"""Runnable CAD/mesh checks. Run AFTER clawlight.py exports updated STLs."""
import itertools
import json
from math import radians, tan
from pathlib import Path

import numpy as np
import trimesh
from build123d import Align, Axis, Cone, Cylinder, GeomType
import clawlight as m


def overlap(a,b):
    c = a.intersect(b)
    if c is None:
        return 0.0
    return c.volume if hasattr(c,'volume') else sum(s.volume for s in c)


def clear(a,b,message):
    v=overlap(a,b)
    assert v<1e-5, f'{message}: {v:.6f} mm3 collision'


def cylinder_z(x,y,z0,z1,diameter):
    return Cylinder(diameter/2,z1-z0,align=(Align.CENTER,Align.CENTER,Align.MIN)).translate((x,y,z0))


def screw_head_z(x,y,rear,diameter):
    """Physical 80-degree head, including the recess caused by seat clearance."""
    slope=tan(radians(m.SCREW_HEAD_ANGLE_DEG/2))
    seating=(m.COUNTERSINK_DIAMETER-diameter)/2/slope
    height=(diameter-m.SCREW_DIAMETER)/2/slope
    return Cone(m.SCREW_DIAMETER/2,diameter/2,height,
                align=(Align.CENTER,Align.CENTER,Align.MIN)).translate((x,y,rear-seating-height))


def screw_checks(parts,report):
    shell=parts['04_tower_black']
    longest=m.SCREW_LENGTH+m.SCREW_LENGTH_TOLERANCE
    shortest=m.SCREW_LENGTH-m.SCREW_LENGTH_TOLERANCE
    slope=tan(radians(m.SCREW_HEAD_ANGLE_DEG/2))
    assert 0<m.SCREW_PILOT_DIAMETER<m.SCREW_DIAMETER<m.SCREW_CLEARANCE
    assert m.COUNTERSINK_DIAMETER>m.SCREW_HEAD_DIAMETER_MAX>=m.SCREW_HEAD_DIAMETER_MIN
    assert abs(m.COUNTERSINK_DEPTH-(m.COUNTERSINK_DIAMETER-m.SCREW_CLEARANCE)/2/slope)<1e-8
    assert abs(m.MAX_HEAD_SEATING_DEPTH-(m.COUNTERSINK_DIAMETER-m.SCREW_HEAD_DIAMETER_MIN)/2/slope)<1e-8
    report['screws']={'standard':'DIN 7982 ST2.2 x 6.5, countersunk Phillips',
                      'head_angle_degrees':m.SCREW_HEAD_ANGLE_DEG,'closures':{}}
    for name,locations,mount,rear,depth in (
        ('05_rear_cover_black',m.COVER_SCREWS,m.REAR,m.DEPTH,m.COVER_PILOT_DEPTH),
        ('06_LED_retainer_black',m.LED_SCREWS,m.RETAINER_MOUNT_Y,m.RETAINER_BACK_Y,m.RETAINER_PILOT_DEPTH),
    ):
        plate=parts[name]
        floor=mount-depth
        worst_tip=rear-longest-m.MAX_HEAD_SEATING_DEPTH
        assert floor>=m.SCREW_TIP_WALL-1e-8, name+' pilot pierces front skin'
        assert worst_tip-floor>=m.SCREW_TIP_CLEARANCE-1e-8, name+' long screw bottoms out'
        assert rear-mount>m.COUNTERSINK_DEPTH, name+' countersink consumes plate thickness'
        least_seating=(m.COUNTERSINK_DIAMETER-m.SCREW_HEAD_DIAMETER_MAX)/2/slope
        min_thread=shortest+least_seating-(rear-mount)-m.SCREW_POINT_LENGTH_MAX
        assert min_thread>0, name+' shortest screw cannot reach with full-diameter thread'
        for x,z in locations:
            pilot=m.cy(x,z,floor,mount,m.SCREW_PILOT_DIAMETER)
            clear(pilot,shell,name+' blind pilot depth')
            skin=m.cy(x,z,0,m.SCREW_TIP_WALL,m.SCREW_PILOT_DIAMETER)
            assert abs(overlap(skin,shell)-skin.volume)<1e-5, name+' missing tip-protection skin'
            # A continuous seating annulus below cosmetic rear edge bevels.
            ring=m.cy(x,z,rear-.7,rear-.6,m.COUNTERSINK_DIAMETER+.8)
            ring-=m.cy(x,z,rear-.71,rear-.59,m.COUNTERSINK_DIAMETER)
            assert abs(overlap(ring,plate)-ring.volume)<1e-5, name+' head seat breaks into an edge or lead slot'
            for head_d in (m.SCREW_HEAD_DIAMETER_MIN,m.SCREW_HEAD_DIAMETER_MAX):
                head=screw_head_z(x,-z,rear,head_d).rotate(Axis.X,-90)
                seated_rear=rear-(m.COUNTERSINK_DIAMETER-head_d)/2/slope
                driver=m.cy(x,z,seated_rear,65,m.SCREW_DRIVER_DIAMETER)
                for other_name,other in parts.items():
                    clear(head,other,name+' seated head / '+other_name)
                    if name=='06_LED_retainer_black' and other_name=='05_rear_cover_black':
                        continue  # Internal fasteners are serviced with the cover removed.
                    clear(driver,other,name+' recessed-head driver / '+other_name)
        report['screws']['closures'][name]={
            'pilot_floor_y_mm':floor,'worst_long_screw_tip_y_mm':worst_tip,
            'tip_to_pilot_floor_mm':worst_tip-floor,
            'minimum_full_diameter_thread_mm':min_thread,
        }
    retainer=parts['06_LED_retainer_black']
    assert abs(retainer.bounding_box().min.Y-m.RETAINER_Y)<1e-6
    assert abs(retainer.bounding_box().max.Y-m.RETAINER_BACK_Y)<1e-6
    for z,d in zip(m.LED_Z,m.LED_FLANGES):
        bearing=m.box(49-d/2+.3,49-d/2+.5,m.RETAINER_Y,m.RETAINER_Y+.1,z-.2,z+.2)
        assert abs(overlap(bearing,retainer)-bearing.volume)<1e-5, 'Missing forward LED flange bearing pad'
    report['checks'].append('DIN 7982 min/max heads seat clear; all five pilots retain tip clearance and front skin; seating annuli and driver access clear')


def coupon_checks(coupons,report):
    base=coupons['00_fit_coupon']
    for name,shape in coupons.items():
        assert shape.is_valid and len(shape.solids())==1 and shape.volume>0,name
    for row,thickness,depth,name in (
        (m.COUPON_COVER_Y,m.COVER,m.COVER_PILOT_DEPTH,'C'),
        (m.COUPON_RETAINER_Y,m.RETAINER_THICKNESS,m.RETAINER_PILOT_DEPTH,'R'),
    ):
        cap=m.coupon_cap(thickness,name).translate((0,row-m.COUPON_CAP_LENGTH/2,m.COUPON_POST_TOP))
        clear(cap,base,'Coupon '+name+' cap seated on pilot posts')
        head_face=m.COUPON_POST_TOP+thickness
        pilot_floor=m.COUPON_POST_TOP-depth
        tip=head_face-m.SCREW_LENGTH-m.SCREW_LENGTH_TOLERANCE-m.MAX_HEAD_SEATING_DEPTH
        assert tip-pilot_floor>=m.SCREW_TIP_CLEARANCE-1e-8
        assert pilot_floor>=m.SCREW_TIP_WALL
        for x,d in zip(m.COUPON_COLUMNS,m.COUPON_PILOT_DIAMETERS):
            clear(cylinder_z(x,row,pilot_floor,m.COUPON_POST_TOP,d),base,'Coupon pilot depth')
            floor=cylinder_z(x,row,pilot_floor-m.SCREW_TIP_WALL,pilot_floor,d)
            assert abs(overlap(floor,base)-floor.volume)<1e-5, 'Coupon pilot needs a solid floor'
            for head_d in (m.SCREW_HEAD_DIAMETER_MIN,m.SCREW_HEAD_DIAMETER_MAX):
                head=screw_head_z(x,row,head_face,head_d)
                clear(head,cap,'Coupon '+name+' seated screw head')
                clear(head,base,'Coupon '+name+' head / post')
    for x,d in zip(m.COUPON_COLUMNS,m.COUPON_LED_DIAMETERS):
        clear(cylinder_z(x,m.COUPON_LED_Y,0,m.COUPON_BASE_THICKNESS,d),base,'Coupon LED bore')
    report['checks'].append('Three valid coupon solids; C/R caps seat on correct-depth pilot rows, with both screw head limits and long-screw tip clearance checked')


def main():
    p=m.parts()
    coupons=m.coupon_parts()
    report={'status':'passed','hardware_fit':'PROVISIONAL, physical parts unmeasured',
            'mesh':{},'checks':[]}
    for n,s in p.items():
        assert s.is_valid and len(s.solids())==1 and s.volume>0,n
        # Rectangular style: no spherical/toroidal fillet surfaces in printed parts.
        assert all(f.geom_type in (GeomType.PLANE,GeomType.CYLINDER,GeomType.CONE) for f in s.faces()),n
        assert any(f.geom_type==GeomType.PLANE and sum(abs(v)>1e-3 for v in f.normal_at())>=2
                   for f in s.faces()),f'{n}: missing flat bevel faces'
    report['checks'].append('All printed parts have flat chamfers; no fillet surfaces')
    # Rear-down print height is 40-Y. No hand-recess face may form a shelf;
    # both ends must instead have 45° faces with opposite Y normal signs.
    recess_faces=[f for f in p['01_mascot_orange'].faces()
                  if f.bounding_box().min.X>=29.9
                  and f.bounding_box().min.Y>10 and f.bounding_box().max.Y<30]
    normals=[f.normal_at().Y for f in recess_faces]
    limit=2**-0.5
    assert normals and all(abs(y)<=limit+1e-6 for y in normals), 'Hand recess has a shelf steeper than 45° overhang'
    assert any(abs(y-limit)<1e-6 for y in normals) and any(abs(y+limit)<1e-6 for y in normals), 'Both hand recess ends need 45° ramps'
    report['checks'].append('Both hand recess ends have full-depth 45-degree ramps; no horizontal recess ledges in rear-down print pose')
    for (na,a),(nb,b) in itertools.combinations(p.items(),2):
        clear(a,b,f'{na} / {nb}')
    report['checks'].append('7 valid single CAD solids; all 21 printed-part pairs have zero interfering volume')
    for en,e in m.electronics().items():
        for pn,s in p.items():
            clear(e,s,f'{en} / {pn}')
    report['checks'].append('Provisional PCB, component, USB and all three LED envelopes clear every printed part')
    screw_checks(p,report)
    coupon_checks(coupons,report)
    assert m.PCB_BOTTOM-m.PCB_UNDERSIDE-6.6>=0.39
    for y in (m.PCB_REAR-m.PCB_LENGTH+5,m.PCB_REAR-5):
        tie=m.box(38.4,59.6,y-1.25,y+1.25,4,5)
        tie+=m.box(38.7,39.7,y-1.25,y+1.25,4,10.4)
        tie+=m.box(58.3,59.3,y-1.25,y+1.25,4,10.4)
        for s in p.values():
            clear(tie,s,'2.5 x 1 mm cable tie under tray and through shelves')
    report['checks'].append('Tie underpasses clear shelves; underside electronics have 0.4 mm spare clearance')
    shell=p['04_tower_black']
    for delta in (0,1,5,10,20,40):
        clear(p['07_PCB_tray_black'].translate((0,delta,0)),shell,'Tray withdrawal')
        clear(p['06_LED_retainer_black'].translate((0,delta,0)),shell,'LED retainer withdrawal')
        for name,s in p.items():
            if name!='05_rear_cover_black':
                clear(p['05_rear_cover_black'].translate((0,delta,0)),s,'Rear cover withdrawal')
    report['checks'].append('Rear cover, PCB tray and LED strip withdrawal checked at 6 offsets')
    for z in m.LED_Z:
        for dz in (-1.27,1.27):
            lead=m.cy(49,z+dz,m.WALL+m.LED_FLANGE_THICKNESS,18,0.7)
            for name,s in p.items():
                clear(lead,s,'Straight LED lead corridor '+name)
        resistor=m.cy(49,z,15,22,2.5)
        for name,s in p.items():
            clear(resistor,s,'Resistor envelope '+name)
    route=m.box(46,52,23,30,16,70)
    for s in p.values():
        clear(route,s,'Continuous wire bundle corridor')
    cable=m.box(43,55,37.6,60,m.USB_CENTER_Z-3.5,m.USB_CENTER_Z+3.5)
    for s in p.values():
        clear(cable,s,'12 x 7 mm cable-overmould insertion envelope')
    report['checks'].append('Lead, 2.5 x 7 mm resistor, wire, USB cable and screwdriver access envelopes clear')
    for x in (-22.5,-12.5,12.5,22.5):
        foot=m.box(x-1,x+1,10,30,0,0.1)
        assert abs(overlap(foot,p['01_mascot_orange'])-foot.volume)<1e-5
    assert abs(shell.bounding_box().min.Z)<1e-6
    report['checks'].append('All four feet and tower pedestal contact Z=0')
    for f in sorted((m.OUT/'stl').glob('*.stl')):
        mesh=trimesh.load_mesh(f)
        assert mesh.is_watertight and mesh.is_winding_consistent and mesh.is_volume,f.name
        assert len(mesh.split())==1,f.name
        assert abs(mesh.bounds[0,2])<1e-5,f.name
        n=f.stem
        if n in p:
            assert np.allclose(mesh.extents,list(m.print_pose(n,p[n]).bounding_box().size),atol=.025)
            assert abs(mesh.volume-p[n].volume)/p[n].volume<.002
        elif n in coupons:
            assert np.allclose(mesh.extents,list(coupons[n].bounding_box().size),atol=.025)
            assert abs(mesh.volume-coupons[n].volume)/coupons[n].volume<.002
        else:
            raise AssertionError('Unexpected stale STL: '+f.name)
        report['mesh'][n]={'watertight':True,'components':1,'triangles':len(mesh.faces),
                           'dimensions_mm':mesh.extents.round(3).tolist(),'bed_z_mm':float(mesh.bounds[0,2])}
    (m.OUT/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))


if __name__=='__main__':
    main()
