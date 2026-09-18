"""Runnable CAD/mesh checks. Run AFTER clawlight.py exports updated STLs."""
import itertools
import json
from math import radians, sqrt, tan
from pathlib import Path

import numpy as np
import trimesh
from build123d import Align, Axis, Cone, Cylinder, Face, GeomType, Wire, extrude
import clawlight as m
import hand_joint as j
import usb_coupon as u


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
                driver=m.cy(x,z,seated_rear,m.DEPTH+30,m.SCREW_DRIVER_DIAMETER)
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
        bearing=m.box(m.TOWER_X-d/2+.3,m.TOWER_X-d/2+.5,
                      m.RETAINER_Y,m.RETAINER_Y+.1,z-.2,z+.2)
        assert abs(overlap(bearing,retainer)-bearing.volume)<1e-5, 'Missing forward LED flange bearing pad'
    screw_count=len(m.COVER_SCREWS)+len(m.LED_SCREWS)
    report['checks'].append(f'DIN 7982 min/max heads seat clear; all {screw_count} pilots retain tip clearance and front skin; seating annuli and driver access clear')


def coupon_checks(coupons,report):
    for name,shape in coupons.items():
        assert shape.is_valid and len(shape.solids())==1 and shape.volume>0,name
    report['checks'].append('Both active USB coupon parts are valid single solids')
    usb_crops={}
    for source,name in (('04_tower_black','01_USB_base_coupon'),
                        ('05_rear_cover_black','02_USB_cover_coupon')):
        production=m.parts()[source]
        bounds=production.bounding_box()
        crop=production & m.box(bounds.min.X-1,bounds.max.X+1,
                                bounds.min.Y-1,bounds.max.Y+1,
                                bounds.min.Z-1,u.COUPON_HEIGHT)
        expected=m.print_pose(source,crop)
        common=overlap(expected,coupons[name])
        assert abs(common-expected.volume)<1e-5 and abs(common-coupons[name].volume)<1e-5, 'USB coupon differs from its actual production crop: '+name
        usb_crops[name]=crop
    clear(*usb_crops.values(),'USB coupon assembly alignment')
    for crop in usb_crops.values():
        clear(crop,m.parts()['07_PCB_tray_black'],'Production tray / USB fit fixture')
        for electronics in m.electronics().values():
            clear(crop,electronics,'Electronics / USB fit fixture')
    report['checks'].append('Both USB fixture pieces exactly match production crops; the unchanged production tray and electronics align without interference')


def shoulder_checks(shell,report):
    # Inspect a continuous 2 mm strip immediately inside each exterior shoulder
    # chamfer. The former cavity broke through this exact region.
    width=2.0
    inset=.15
    start=(m.UPRIGHT_LEFT-m.TOWER_CHAMFER,m.BASE_HEIGHT)
    end=(m.UPRIGHT_LEFT,m.BASE_HEIGHT+m.TOWER_CHAMFER)
    def inward(point,distance):
        return (point[0]+distance/sqrt(2),point[1]-distance/sqrt(2))
    points=[inward(start,inset),inward(end,inset),
            inward(end,inset+width),inward(start,inset+width)]
    volumes=[]
    for mirrored in (False,True):
        profile=[(2*m.TOWER_X-x,z) if mirrored else (x,z) for x,z in points]
        strip=m.profile(profile,m.WALL,m.REAR)
        assert abs(overlap(strip,shell)-strip.volume)<1e-5, 'Tower shoulder has a gap or loses its continuous 2 mm reinforcement'
        volumes.append(strip.volume)
    report['tower_shoulders']={'continuous_strip_width_mm':width,
                               'verified_depth_y_mm':[m.WALL,m.REAR],
                               'solid_probe_volumes_mm3':volumes}
    report['checks'].append('Both formerly open shoulders contain continuous 2 mm reinforced strips from the front wall to the rear seam')


def cover_checks(cover,report):
    # Check the seated PCB and both end-stop extremes, not only nominal flush.
    projection=m.USB_REAR-m.DEPTH
    minimum=projection-m.TRAY_FRONT_CLEARANCE-m.PCB_END_CLEARANCE
    maximum=projection+m.PCB_END_CLEARANCE
    assert minimum>=-1e-8 and maximum<=.5+1e-8, 'USB must stay flush to 0.5 mm proud throughout endplay'
    assert abs(cover.bounding_box().max.Y-m.DEPTH)<1e-8
    assert abs(m.DEPTH-m.COVER_POCKET_BACK-m.COVER_INNER_SKIN)<1e-8
    assert m.COVER_INNER_SKIN>=1-1e-8, 'Flat cover skin is thinner than 1 mm'
    # Inspect the whole lower skin except the intentional aperture/bevel band.
    skin=m.box(m.COVER_POCKET_LEFT+.5,m.COVER_POCKET_RIGHT-.5,
               m.COVER_POCKET_BACK,m.DEPTH,
               m.COVER_POCKET_BOTTOM+.5,m.COVER_POCKET_TOP-.5)
    skin-=m.box(m.TOWER_X-m.USB_OPENING_WIDTH/2-m.USB_ENTRY_CHAMFER,
                m.TOWER_X+m.USB_OPENING_WIDTH/2+m.USB_ENTRY_CHAMFER,
                m.COVER_POCKET_BACK-.1,m.DEPTH+.1,
                m.USB_CENTER_Z-m.USB_OPENING_HEIGHT/2-m.USB_ENTRY_CHAMFER,
                m.USB_CENTER_Z+m.USB_OPENING_HEIGHT/2+m.USB_ENTRY_CHAMFER)
    assert abs(overlap(skin,cover)-skin.volume)<1e-5, 'Flat external USB cover skin contains an unintended pocket or break'
    # Probe removed edge material and the preserved USB throat behind its bevel.
    usb_top=m.USB_CENTER_Z+m.USB_OPENING_HEIGHT/2
    clear(m.box(m.TOWER_X-.5,m.TOWER_X+.5,m.COVER_POCKET_BACK+.025,
                m.COVER_POCKET_BACK+.05,usb_top+.025,usb_top+.05),cover,'Inner USB chamfer missing')
    throat=m.box(m.TOWER_X-.5,m.TOWER_X+.5,m.COVER_POCKET_BACK+.25,
                 m.COVER_POCKET_BACK+.30,usb_top+.025,usb_top+.05)
    assert abs(overlap(throat,cover)-throat.volume)<1e-6, 'Chamfer enlarges the USB throat'
    for x0,x1 in ((m.BASE_LEFT+.025,m.BASE_LEFT+.05),
                  (m.COVER_POCKET_LEFT-.05,m.COVER_POCKET_LEFT-.025)):
        clear(m.box(x0,x1,m.REAR+.025,m.REAR+.05,3,3.1),cover,'Inner cover edge chamfer missing')
    for side in (-1,1):
        edge=m.TOWER_X+side*m.PCB_WIDTH/2
        x0,x1=sorted((edge-side*.1,edge-side*(m.PCB_REAR_STOP_INSET-.1)))
        stop=m.box(x0,x1,m.PCB_REAR_STOP_FRONT,m.DEPTH,
                   m.PCB_BOTTOM+.05,m.PCB_TOP-.05)
        assert abs(overlap(stop,cover)-stop.volume)<1e-5, 'Cover-mounted PCB stop has no solid backing to the exterior skin'
    posed=m.print_pose('05_rear_cover_black',cover)
    for face in posed.faces():
        if face.bounding_box().max.Z>1e-6:
            assert face.normal_at().Z>=-sqrt(.5)-1e-6, 'Flat cover has a downward face requiring support above its print bed'
    report['flat_cover']={'skin_mm':m.COVER_INNER_SKIN,
                         'inner_edge_chamfer_mm':m.COVER_INNER_CHAMFER,
                         'usb_entry_chamfer_both_sides_mm':m.USB_ENTRY_CHAMFER,
                         'skin_probe_volume_mm3':skin.volume,
                         'pcb_stop_backing_thickness_mm':m.DEPTH-m.PCB_REAR_STOP_FRONT,
                         'usb_face_offset_mm':m.DEPTH-m.USB_REAR,
                         'usb_projection_range_mm':[minimum,maximum]}
    report['checks'].append('Cover has a continuous 1 mm skin and solid PCB stops; USB stays flush to 0.5 mm proud across endplay; above-bed downward faces meet the 45-degree limit')


def hand_joint_checks(parts,report):
    mascot=parts['01_mascot_orange']
    shell=parts['04_tower_black']
    assert abs(j.JOINT_X-m.UPRIGHT_LEFT)<1e-8, 'Hand joint datum misses tower mating face'
    assert j.SOCKET_DIAGONAL>j.PIN_DIAGONAL>0
    assert j.SOCKET_DEPTH>j.PIN_PROJECTION>0
    pins=j.hand_pins()
    assert len(pins.solids())==len(j.JOINT_CENTERS_YZ)
    for pin in pins.solids():
        assert abs(overlap(pin,mascot)-pin.volume)<1e-5, 'Integral hand pin is not fully joined to mascot'
        clear(pin,shell,'Hand pin / tower socket')
        for face in pin.rotate(Axis.X,-90).faces():
            assert abs(face.normal_at().Z)<=sqrt(.5)+1e-6, 'Hand pin or tip bevel requires supports'
    for y,z in j.JOINT_CENTERS_YZ:
        straight_core=Cylinder(j.PIN_DIAGONAL/6,j.PIN_PROJECTION,
                               align=(Align.CENTER,Align.CENTER,Align.MIN)).rotate(Axis.Y,90).translate((j.JOINT_X,y,z))
        assert abs(overlap(straight_core,mascot)-straight_core.volume)<1e-5, 'Hand pin does not retain a straight core normal to the hand face'
        hole=j.diamond_socket(j.JOINT_X,j.JOINT_X+j.SOCKET_DEPTH,y,z,j.SOCKET_DIAGONAL)
        clear(hole,shell,'Straight hand socket depth')
        # Probe real solid behind the blind end, along the same insertion axis.
        floor=j.diamond_socket(j.JOINT_X+j.SOCKET_DEPTH,
                               j.JOINT_X+j.SOCKET_DEPTH+.4,
                               y,z,j.PIN_DIAGONAL/2)
        assert abs(overlap(floor,shell)-floor.volume)<1e-5, 'Hand socket has no 0.4 mm solid blind floor'
    for distance in (0,.3,.6,1.2,j.PIN_PROJECTION,j.SOCKET_DEPTH+.2):
        clear(mascot.translate((-distance,0,0)),shell,'Straight hand-joint insertion path')
    # The rear socket starts inside the cavity when the tower prints front down.
    # Check a continuous solid strip from the wall to its leading edge, then
    # inspect the local exposed faces in the actual +Y printing direction.
    rear_y,rear_z=max(j.JOINT_CENTERS_YZ,key=lambda yz:yz[0])
    wall_x=m.UPRIGHT_LEFT+m.UPRIGHT_WALL
    tip_x=j.JOINT_X+j.SOCKET_DEPTH+m.HAND_SOCKET_WALL
    radius=(j.SOCKET_DIAGONAL+2*m.HAND_SOCKET_WALL)/2
    leading_y=rear_y-radius
    support_y=leading_y-(tip_x-wall_x)-m.HAND_SOCKET_GUSSET_OVERLAP
    assert m.HAND_SOCKET_GUSSET_OVERLAP>0 and tip_x>wall_x
    x0=wall_x-m.HAND_SOCKET_GUSSET_OVERLAP/2
    x1=tip_x-.05
    y0=leading_y-(tip_x-x0)+.15
    y1=leading_y-(tip_x-x1)+.15
    strip_face=Face(Wire.make_polygon([(x0,y0,rear_z-.1),(x1,y1,rear_z-.1),
                                       (x1,y1+.2,rear_z-.1),(x0,y0+.2,rear_z-.1)],close=True))
    strip=extrude(strip_face,amount=.2,dir=(0,0,1))
    assert abs(overlap(strip,shell)-strip.volume)<1e-5, 'Rear socket boss has no continuous leading support from the side wall'
    support_faces=[]
    for face in shell.faces():
        bounds=face.bounding_box()
        if (bounds.min.X>=wall_x-m.HAND_SOCKET_GUSSET_OVERLAP-1e-6
                and bounds.max.X<=tip_x+1e-6
                and bounds.min.Y>=support_y-1e-6
                and bounds.max.Y<=rear_y+radius+(tip_x-j.JOINT_X)+1e-6
                and bounds.min.Z>=rear_z-radius-1e-6
                and bounds.max.Z<=rear_z+radius+1e-6):
            support_faces.append(face)
    support_normals=[face.normal_at().Y for face in support_faces]
    assert support_normals and all(n>=-sqrt(.5)-1e-6 for n in support_normals), 'Rear socket boss contains a front-down overhang steeper than 45 degrees'
    assert any(abs(n+sqrt(.5))<1e-6 for n in support_normals), 'Rear socket leading 45-degree gusset face missing'
    report['hand_joint']={
        'integral_pin_count':len(j.JOINT_CENTERS_YZ),
        'pin_diagonal_mm':j.PIN_DIAGONAL,'socket_diagonal_mm':j.SOCKET_DIAGONAL,
        'front_guide_face_clearance_mm':(j.SOCKET_DIAGONAL-j.PIN_DIAGONAL)/(2*sqrt(2)),
        'tip_clearance_x_mm':j.SOCKET_DEPTH-j.PIN_PROJECTION,
        'insertion_direction':[1,0,0],
        'withdrawal_direction':[-1,0,0],
        'rear_socket_gusset':{
            'continuous_wall_to_boss_probe_volume_mm3':strip.volume,
            'local_faces_checked':len(support_faces),
            'minimum_print_direction_normal':min(support_normals),
        },
        'physical_fit':'User tested the 3.3 mm coupon: locating fit, not friction retention; optional glue for permanent attachment. Simultaneous two-pin assembly remains unverified.',
    }
    report['checks'].append('Integral straight hand pins clear both blind sockets and the straight-X insertion path; blind floors and support-free pin faces checked')
    report['checks'].append('Rear socket gusset has a continuous solid support strip from the wall; its local front-down faces have at most 45-degree overhangs')


def main(check_mesh=True):
    p=m.parts()
    printable=m.print_parts()
    coupons=m.coupon_parts()
    report={'status':'passed','hardware_fit':'User coupon-tested screw/LED bores and measured PCB, LED and USB-shell outlines; component allowances, cable plastic and assembled fit remain unverified',
            'mesh':{},'checks':[]}
    for n,s in p.items():
        assert s.is_valid and len(s.solids())==1 and s.volume>0,n
        # Rectangular style: no spherical/toroidal fillet surfaces in printed parts.
        assert all(f.geom_type in (GeomType.PLANE,GeomType.CYLINDER,GeomType.CONE) for f in s.faces()),n
        assert any(f.geom_type==GeomType.PLANE and sum(abs(v)>1e-3 for v in f.normal_at())>=2
                   for f in s.faces()),f'{n}: missing flat bevel faces'
    report['checks'].append('All printed parts have flat chamfers; no fillet surfaces')
    left_eye=m.print_pose('02_eye_left_black',p['02_eye_left_black'])
    right_eye=m.print_pose('03_eye_right_black',p['03_eye_right_black'])
    common=overlap(left_eye,right_eye)
    assert abs(common-left_eye.volume)<1e-6 and abs(common-right_eye.volume)<1e-6, 'Eyes do not share the same normalized print geometry'
    assert len(printable)==len(p)-1 and m.print_quantity('02_eye_black_x2')==2
    assert sum(m.print_quantity(name) for name in printable)==len(p)
    assert '02_eye_left_black' not in printable and '03_eye_right_black' not in printable
    report['print_inventory']={'assembly_occurrences':len(p),'unique_assembly_STLs':len(printable),
                               'coupon_STLs':len(coupons),'quantities':{name:m.print_quantity(name) for name in printable}}
    report['checks'].append('Both eyes have identical normalized solids; one eye STL with quantity two preserves all seven assembly occurrences')
    # The uniformly scaled mascot prints rear down. No recess face may form a shelf;
    # both ends must instead have 45° faces with opposite Y normal signs.
    recess_faces=[f for f in p['01_mascot_orange'].faces()
                  if f.bounding_box().min.X>=29.9*m.MASCOT_SCALE
                  and f.bounding_box().min.Y>10*m.MASCOT_SCALE
                  and f.bounding_box().max.Y<30*m.MASCOT_SCALE]
    normals=[f.normal_at().Y for f in recess_faces]
    limit=2**-0.5
    assert normals and all(abs(y)<=limit+1e-6 for y in normals), 'Hand recess has a shelf steeper than 45° overhang'
    assert any(abs(y-limit)<1e-6 for y in normals) and any(abs(y+limit)<1e-6 for y in normals), 'Both hand recess ends need 45° ramps'
    report['checks'].append('Both hand recess ends have full-depth 45-degree ramps; no horizontal recess ledges in rear-down print pose')
    for (na,a),(nb,b) in itertools.combinations(p.items(),2):
        clear(a,b,f'{na} / {nb}')
    report['checks'].append(f'{len(p)} valid single CAD solids; all {len(p)*(len(p)-1)//2} printed-part pairs have zero interfering volume')
    for en,e in m.electronics().items():
        for pn,s in p.items():
            clear(e,s,f'{en} / {pn}')
    report['checks'].append('Measured PCB, USB-shell and LED envelopes, plus provisional component allowances, clear every printed part')
    screw_checks(p,report)
    coupon_checks(coupons,report)
    hand_joint_checks(p,report)
    shoulder_checks(p['04_tower_black'],report)
    cover_checks(p['05_rear_cover_black'],report)
    assert abs(m.PCB_BOTTOM-m.TRAY_BASE_TOP-m.PCB_FLOOR_GAP)<1e-8
    assert m.PCB_BOTTOM-m.PCB_UNDERSIDE-m.TRAY_BASE_TOP>=0.4-1e-8
    report['checks'].append(f'PCB-to-floor gap is {m.PCB_FLOOR_GAP:g} mm; provisional underside component allowance retains at least 0.4 mm clearance')
    shell=p['04_tower_black']
    for delta in (0,1,5,10,20,40):
        clear(p['07_PCB_tray_black'].translate((0,delta,0)),shell,'Tray withdrawal')
        clear(p['06_LED_retainer_black'].translate((0,delta,0)),shell,'LED retainer withdrawal')
        for name,s in p.items():
            if name!='05_rear_cover_black':
                clear(p['05_rear_cover_black'].translate((0,delta,0)),s,'Rear cover withdrawal')
    report['checks'].append('Rear cover, PCB tray and LED strip withdrawal checked at 6 offsets')

    tray=p['07_PCB_tray_black']
    pcb=m.electronics()['pcb']
    # Cosmetic bevels must not thin the spring strips or groove capture lands.
    for mirrored in (False,True):
        x0,x1=m.TRAY_LEFT,m.CLIP_INNER_X
        if mirrored:
            x0,x1=2*m.TOWER_X-x1,2*m.TOWER_X-x0
        core=m.box(x0,x1,m.CLIP_BEAM_Y0+.5,m.CLIP_BEAM_Y1-.1,
                   m.TRAY_FLOOR_Z+.2,m.CLIP_TOP_Z)
        assert abs(overlap(core,tray)-core.volume)<1e-5, 'Chamfer weakens PCB spring strip'
        for y0,y1 in ((m.TRAY_FRONT,m.CLIP_BEAM_Y0-m.CLIP_ISOLATION_GAP),
                      (m.CLIP_BEAM_Y1+m.CLIP_ISOLATION_GAP,m.TRAY_BACK)):
            x0,x1=m.TRAY_LEFT-m.TRAY_KEY_EXTENSION+.25,m.TRAY_LEFT-.25
            if mirrored:
                x0,x1=2*m.TOWER_X-x1,2*m.TOWER_X-x0
            land=m.box(x0,x1,y0+.3,y1-.3,m.TRAY_BASE_TOP-.1,m.TRAY_BASE_TOP)
            assert abs(overlap(land,tray)-land.volume)<1e-5, 'Chamfer removes groove capture land'
    report['checks'].append('Chamfers preserve both spring-strip sections and all four groove capture lands')
    for face in tray.faces():
        if face.bounding_box().max.Z>m.TRAY_FLOOR_Z+1e-6:
            assert face.normal_at().Z>=-limit-1e-6, 'Tray has an underside steeper than 45 degrees'
    report['checks'].append('Tray clip/lip undersides have at most 45-degree overhangs above the print bed')
    # Front edge tucks under fixed lips before the rear snaps down.
    hinge=Axis((0,m.PCB_FRONT,m.PCB_BOTTOM),(1,0,0))
    clear(pcb.rotate(hinge,15),tray,'PCB 15-degree entry pose')
    # Finite motion past designed clearances must encounter physical stops.
    assert overlap(pcb.translate((0,-(m.PCB_END_CLEARANCE+.15),0)),tray)>1e-4, 'PCB front stop missing'
    assert overlap(pcb.translate((0,m.PCB_END_CLEARANCE+.15,0)),p['05_rear_cover_black'])>1e-4, 'Cover-mounted PCB rear stop missing'
    assert overlap(pcb.translate((0,0,.6)),tray)>1e-4, 'PCB retaining lips missing'
    assert overlap(tray.translate((0,-(m.TRAY_FRONT_CLEARANCE+.15),0)),shell)>1e-4, 'Tray front stop missing'
    rear_gap=m.COVER_POCKET_BACK-m.TRAY_BACK
    assert rear_gap>=m.FIT-1e-8, 'Tray has no running clearance to the inside cover stop'
    assert overlap(tray.translate((0,rear_gap+.15,0)),p['05_rear_cover_black'])>1e-4, 'Cover does not retain tray against USB extraction'
    key_capture=m.TRAY_KEY_EXTENSION-m.TRAY_RAIL_SIDE_CLEARANCE
    assert key_capture-m.CLIP_KEEPER_CLEARANCE>0, 'A groove key escapes its roof at permitted sideways tray travel'
    for dx in (-m.CLIP_KEEPER_CLEARANCE,0,m.CLIP_KEEPER_CLEARANCE):
        clear(tray.translate((dx,0,0)),shell,'Tray lateral running clearance')
        for dz in (-.4,m.TRAY_RAIL_VERTICAL_CLEARANCE+.15):
            assert overlap(tray.translate((dx,0,dz)),shell)>1e-4, 'Captured tray rail missing at lateral travel limit'
    clip_margin=m.CLIP_HOOK_OVERLAP-2*m.CLIP_KEEPER_CLEARANCE-m.PCB_SIDE_CLEARANCE
    assert clip_margin>=.15-1e-8, 'Keeper clearance permits PCB clips to release'
    for mirrored in (False,True):
        x0=m.TRAY_LEFT-m.CLIP_KEEPER_CLEARANCE-.1
        x1=m.TRAY_LEFT-m.CLIP_KEEPER_CLEARANCE
        if mirrored:
            x0,x1=2*m.TOWER_X-x1,2*m.TOWER_X-x0
        keeper=m.box(x0,x1,m.CLIP_HOOK_Y0+.1,m.CLIP_HOOK_Y1-.1,
                     m.CLIP_TIP_Z-.1,m.CLIP_TIP_Z+.1)
        assert abs(overlap(keeper,shell)-keeper.volume)<1e-5, 'Local clip keeper missing'
        # Rear keys engage the base before the seam; the cover has no ribs.
        x0=m.TRAY_LEFT-m.TRAY_KEY_EXTENSION+.25
        x1=m.TRAY_LEFT-m.TRAY_RAIL_SIDE_CLEARANCE-.05
        if mirrored:
            x0,x1=2*m.TOWER_X-x1,2*m.TOWER_X-x0
        y0=m.CLIP_BEAM_Y1+m.CLIP_ISOLATION_GAP+m.TRAY_KEY_CHAMFER
        y1=m.REAR-.05
        assert y1-y0>=.3-1e-8, 'Rear key has too little capture inside the base'
        roof=m.box(x0,x1,y0,y1,m.TRAY_BASE_TOP+m.TRAY_RAIL_VERTICAL_CLEARANCE+.02,
                   m.TRAY_BASE_TOP+m.TRAY_RAIL_VERTICAL_CLEARANCE+.12)
        assert abs(overlap(roof,shell)-roof.volume)<1e-5, 'Rear key has no guide roof inside the base'
        # Even at the rear stop the key must still catch the base when lifted.
        key=m.box(x0,x1,m.CLIP_BEAM_Y1+m.CLIP_ISOLATION_GAP,m.TRAY_BACK,
                  m.TRAY_FLOOR_Z,m.TRAY_BASE_TOP) & tray
        assert overlap(key.translate((0,rear_gap,m.TRAY_RAIL_VERTICAL_CLEARANCE+.1)),shell)>1e-5, 'Rear key escapes its base guide at the rear stop'
    floor=m.box(m.INNER_LEFT,m.INNER_RIGHT,m.TRAY_FRONT,m.REAR,
                0,m.BASE_FLOOR_THICKNESS)
    assert abs(overlap(floor,shell)-floor.volume)<1e-5, 'Base floor is not continuous and flat'
    above=m.box(m.INNER_LEFT,m.INNER_RIGHT,m.TRAY_FRONT+.01,m.REAR,
                m.BASE_FLOOR_THICKNESS,m.BASE_FLOOR_THICKNESS+.1)
    clear(above,shell,'Flat base floor has a raised shelf')
    pocket=m.box(m.COVER_POCKET_LEFT,m.COVER_POCKET_RIGHT,m.REAR,m.COVER_POCKET_BACK,
                 m.COVER_POCKET_BOTTOM,m.COVER_POCKET_TOP)
    clear(pocket,p['05_rear_cover_black'],'Cover pocket still has projecting tabs')
    report['flat_base_floor_mm']=m.BASE_FLOOR_THICKNESS
    assert abs(shell.bounding_box().max.Y-m.REAR)<1e-6, 'Base still has protrusions beyond the cover seam'
    # Fully seated board and cable-pull limit must clear the closed enclosure.
    for dy in (-m.PCB_END_CLEARANCE,m.PCB_END_CLEARANCE):
        for name in ('pcb','usb','top','underside'):
            moved=m.electronics()[name].translate((0,dy,0))
            for part in (shell,p['05_rear_cover_black'],tray):
                clear(moved,part,'Seated PCB endplay / enclosure')
    report['tray_clearances_mm']={
        'floor_gap':m.PCB_FLOOR_GAP,'rail_vertical':m.TRAY_RAIL_VERTICAL_CLEARANCE,
        'rail_side':m.TRAY_RAIL_SIDE_CLEARANCE,'local_clip_keeper':m.CLIP_KEEPER_CLEARANCE,
        'key_roof_overlap_nominal':key_capture,
        'key_roof_overlap_at_lateral_limit':key_capture-m.CLIP_KEEPER_CLEARANCE,
        'minimum_clip_overlap_after_tray_and_board_float':clip_margin,
        'rear_clearance_to_cover_stop':rear_gap,
        'rail_extension_beyond_shell_seam':m.TRAY_RAIL_BACK-m.REAR,
    }
    report['checks'].append('Tie-free PCB entry pose clears; board end/lift stops and tray front/rear/lift stops engage beyond design clearances')

    for z in m.LED_Z:
        for dz in (-1.27,1.27):
            lead=m.cy(m.TOWER_X,z+dz,m.WALL+m.LED_FLANGE_THICKNESS,18,0.7)
            for name,s in p.items():
                clear(lead,s,'Straight LED lead corridor '+name)
        resistor=m.cy(m.TOWER_X,z,15,22,2.5)
        for name,s in p.items():
            clear(resistor,s,'Resistor envelope '+name)
    route_y1=m.REAR-m.WIRE_REAR_CLEARANCE
    route_y0=route_y1-m.WIRE_CORRIDOR_DEPTH
    assert route_y1-route_y0>=6, 'No room for a 6 mm rear wire corridor'
    route=m.box(m.TOWER_X-3,m.TOWER_X+3,route_y0,route_y1,
                max(m.PCB_TOP+m.PCB_TOP_HEIGHT+1,
                    m.USB_CENTER_Z+m.USB_HEIGHT/2+.5),max(m.LED_Z)+2)
    for s in p.values():
        clear(route,s,'Continuous wire bundle corridor')
    metal_nose=m.box(m.TOWER_X-m.USB_WIDTH/2,m.TOWER_X+m.USB_WIDTH/2,
                     m.USB_REAR,m.USB_CABLE_BODY_START,
                     m.USB_CENTER_Z-m.USB_HEIGHT/2,m.USB_CENTER_Z+m.USB_HEIGHT/2)
    cable=m.box(m.TOWER_X-6,m.TOWER_X+6,m.USB_CABLE_BODY_START,m.DEPTH+20,
                m.USB_CENTER_Z-3.5,m.USB_CENTER_Z+3.5)
    for s in p.values():
        clear(metal_nose,s,'Metal USB plug-nose insertion envelope')
        clear(cable,s,'12 x 7 mm cable-overmould insertion envelope')
    report['usb_fit']={
        'measured_socket_width_height_mm':[m.USB_WIDTH,m.USB_HEIGHT],
        'aperture_width_height_mm':[m.USB_OPENING_WIDTH,m.USB_OPENING_HEIGHT],
        'socket_to_exterior_face_recess_mm':m.DEPTH-m.USB_REAR,
        'user_measured_vertical_correction_mm':m.USB_SEATING_Z_OFFSET,
        'aperture_center_z_mm':m.USB_CENTER_Z,
        'entry_chamfer_mm':m.USB_ENTRY_CHAMFER,
        'provisional_cable_body_width_height_mm':[12,7],
        'provisional_cable_body_start_y_mm':m.USB_CABLE_BODY_START,
        'assumed_exposed_metal_ahead_of_plastic_mm':m.USB_CABLE_BODY_START-m.USB_REAR,
        'flat_cover_skin_mm':m.COVER_INNER_SKIN,
        'cover_pcb_stop_backing_mm':m.DEPTH-m.PCB_REAR_STOP_FRONT,
        'physical_fit':'Cable plastic is unmeasured; requires at least the stated exposed-metal gap when fully mated',
    }
    report['checks'].append('Lead, 2.5 x 7 mm resistor, wire, USB cable and screwdriver access envelopes clear')
    for x in (-22.5,-12.5,12.5,22.5):
        foot=m.box((x-1)*m.MASCOT_SCALE,(x+1)*m.MASCOT_SCALE,
                   10*m.MASCOT_SCALE,30*m.MASCOT_SCALE,0,.1*m.MASCOT_SCALE)
        assert abs(overlap(foot,p['01_mascot_orange'])-foot.volume)<1e-5
    assert abs(shell.bounding_box().min.Z)<1e-6
    report['checks'].append('All four feet and tower pedestal contact Z=0')
    if not check_mesh:
        report['status']='cad-passed-stl-not-checked'
        print(json.dumps(report,indent=2))
        return report
    stl_files=sorted((m.OUT/'stl').rglob('*.stl'))
    assert {f.stem for f in stl_files}==set(printable)|set(coupons), 'STL directory has missing parts or stale exports'
    for f in stl_files:
        expected_parent=m.OUT/'stl'/('coupons' if f.stem in coupons else '')
        assert f.parent==expected_parent, 'STL is in the wrong folder: '+str(f)
        mesh=trimesh.load_mesh(f)
        assert mesh.is_watertight and mesh.is_winding_consistent and mesh.is_volume,f.name
        assert len(mesh.split())==1,f.name
        assert abs(mesh.bounds[0,2])<1e-5,f.name
        n=f.stem
        if n in printable:
            assert np.allclose(mesh.extents,list(m.print_pose(n,printable[n]).bounding_box().size),atol=.025)
            assert abs(mesh.volume-printable[n].volume)/printable[n].volume<.002
        elif n in coupons:
            assert np.allclose(mesh.extents,list(coupons[n].bounding_box().size),atol=.025)
            assert abs(mesh.volume-coupons[n].volume)/coupons[n].volume<.002
        else:
            raise AssertionError('Unexpected stale STL: '+f.name)
        report['mesh'][n]={'watertight':True,'components':1,'triangles':len(mesh.faces),
                           'dimensions_mm':mesh.extents.round(3).tolist(),'bed_z_mm':float(mesh.bounds[0,2]),
                           'quantity':m.print_quantity(n) if n in printable else 1}
    (m.OUT/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))


if __name__=='__main__':
    import argparse
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cad-only',action='store_true',help='Check current CAD without reading STLs or writing validation.json')
    main(check_mesh=not parser.parse_args().cad_only)
