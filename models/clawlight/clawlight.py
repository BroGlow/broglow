"""Claw Light: millimetres, X right, Y rear, Z up; desk Z=0.

Run with build123d 0.11.1: python clawlight.py
The default electronics are EXPLICIT PROVISIONAL ENVELOPES, not measured parts.
Print exports are individually oriented and translated onto Z=0.
"""
from pathlib import Path
from functools import lru_cache
from math import ceil, radians, tan
import json

from build123d import (
    Align, Axis, Box, Color, Compound, Cone, Cylinder, Face,
    Sphere, Text, Wire, chamfer, export_step, export_stl, extrude,
)

OUT = Path(__file__).resolve().parent
WALL = 2.4
DEPTH = 40.0
COVER = 2.4
REAR = DEPTH - COVER
FIT = 0.25  # clearance PER SIDE, not diametral
EYE_CLEARANCE = 0.30
BODY_CHAMFER = 0.8
FACE_CHAMFER = 0.6
APPENDAGE_CHAMFER = 0.4
HAND_RECESS_END_CHAMFER = 2.0  # full-depth 45° transitions at both recess ends
TOWER_CHAMFER = 0.6
BED_CHAMFER = 0.4
EYE_CORNER_CHAMFER = 0.35
EYE_CHAMFER = 0.25
USB_CORNER_CHAMFER = 0.8
SMALL_CHAMFER = 0.3
LED_DIAMETERS = (5.0, 5.0, 5.0)  # green, yellow, red; measure each
LED_FLANGES = (6.0, 6.0, 6.0)
LED_FLANGE_THICKNESS = 1.0
LED_BARREL_LENGTH = 8.5  # flange front to dome tip
LED_PITCH = 12.0
LED_Z = tuple(44.0 + i * LED_PITCH for i in range(3))
LED_BORE_CLEARANCE = 0.30  # diametral
LED_BORE_LEAD_IN = 0.3
LED_RETAINER_GAP = 0.25  # optional thin soft shim removes axial play
LEAD_SLOT = 3.6  # pair of 2.54 mm spaced leads, oriented vertically
PCB_WIDTH = 18.0
PCB_LENGTH = 22.5
PCB_THICKNESS = 1.6
PCB_UNDERSIDE = 2.0
PCB_TOP_HEIGHT = 4.0
PCB_REAR = 35.2
PCB_BOTTOM = 9.0  # 0.4 mm spare below the provisional 2 mm underside envelope
USB_WIDTH, USB_HEIGHT = 9.0, 3.5
USB_CENTER_Z = PCB_BOTTOM + PCB_THICKNESS + 1.5
USB_REAR = 37.6
USB_OPENING_WIDTH, USB_OPENING_HEIGHT = 14.0, 10.0
# User hardware: DIN 7982 ST2.2 x 6.5, A2/304, countersunk Phillips.
# DIN 7982 is 80 degrees; ISO 7050 is 90. Length includes the head.
# Angle reference: REYHER Technical Information 2016.11 EN, p.11 / Table 10.
# Dimensions/tolerances: https://eshop.boellhoff.de/out/media/pdf/DIN_7982_Edelstahl_A2___en.pdf
SCREW_DIAMETER = 2.2
SCREW_LENGTH = 6.5
SCREW_LENGTH_TOLERANCE = 0.8
SCREW_HEAD_DIAMETER_MIN = 4.0
SCREW_HEAD_DIAMETER_MAX = 4.3
SCREW_HEAD_ANGLE_DEG = 80.0
SCREW_CLEARANCE = SCREW_DIAMETER + 0.3
SCREW_HEAD_CLEARANCE = 0.3
SCREW_PILOT_DIAMETER = 1.8  # tune from coupon, not a metal tapping-drill table
SCREW_TIP_CLEARANCE = 0.3
SCREW_TIP_WALL = 0.8
SCREW_POINT_LENGTH_MAX = 2.0
SCREW_DRIVER_DIAMETER = 4.0
COUNTERSINK_DIAMETER = SCREW_HEAD_DIAMETER_MAX + SCREW_HEAD_CLEARANCE
COUNTERSINK_DEPTH = (COUNTERSINK_DIAMETER-SCREW_CLEARANCE)/2/tan(radians(SCREW_HEAD_ANGLE_DEG/2))
MAX_HEAD_SEATING_DEPTH = (COUNTERSINK_DIAMETER-SCREW_HEAD_DIAMETER_MIN)/2/tan(radians(SCREW_HEAD_ANGLE_DEG/2))
COVER_BOSS_DIAMETER = 5.6
LED_BOSS_DIAMETER = 5.6
COVER_SCREWS = ((38.6, 20.0), (59.4, 20.0), (55.2, 72.7))
LED_SCREWS = ((45.8, 38.0), (45.8, 62.0))
RETAINER_Y = WALL + LED_FLANGE_THICKNESS + LED_RETAINER_GAP  # flange pad face
RETAINER_THICKNESS = 3.0  # structural plate; forward flange pads are additional
COVER_PILOT_DEPTH = SCREW_LENGTH+SCREW_LENGTH_TOLERANCE+MAX_HEAD_SEATING_DEPTH-COVER+SCREW_TIP_CLEARANCE
RETAINER_PILOT_DEPTH = SCREW_LENGTH+SCREW_LENGTH_TOLERANCE+MAX_HEAD_SEATING_DEPTH-RETAINER_THICKNESS+SCREW_TIP_CLEARANCE
RETAINER_MOUNT_Y = max(RETAINER_Y,ceil((RETAINER_PILOT_DEPTH+SCREW_TIP_WALL)*10)/10)
RETAINER_BACK_Y = RETAINER_MOUNT_Y + RETAINER_THICKNESS
RETAINER_LEFT, RETAINER_RIGHT = 42.65, 52.2
RETAINER_BOTTOM, RETAINER_TOP = 34.0, 73.35
LEAD_SLOT_X = 47.0

# Coupon samples share the production bores, countersinks and post diameters.
COUPON_PILOT_DIAMETERS = (1.7, 1.8, 1.9, 2.0)
COUPON_LED_DIAMETERS = (5.1, 5.3, 5.5)
COUPON_COLUMNS = (7.0, 20.0, 33.0, 46.0)
COUPON_LED_Y, COUPON_COVER_Y, COUPON_RETAINER_Y = 8.0, 25.0, 40.0
COUPON_WIDTH, COUPON_LENGTH = 68.0, 50.0
COUPON_BASE_THICKNESS = WALL
COUPON_POST_HEIGHT = ceil((max(COVER_PILOT_DEPTH,RETAINER_PILOT_DEPTH)+SCREW_TIP_WALL)*10)/10
COUPON_POST_TOP = COUPON_BASE_THICKNESS + COUPON_POST_HEIGHT
COUPON_CAP_WIDTH, COUPON_CAP_LENGTH = 54.0, 8.0
COUPON_LABEL_SIZE, COUPON_LABEL_DEPTH = 2.4, 0.4
COUPON_CORNER_CHAMFER, COUPON_BED_CHAMFER = 1.0, 0.25
EYE_TONGUE_WIDTH, EYE_TONGUE_HEIGHT, EYE_TONGUE_DEPTH = 4.9, 9.9, 1.15
EYE_POCKET_DEPTH = 1.4
COUPON_EYE_X, COUPON_EYE_Y = 57.0, 8.0
BOOLEAN_OVERLAP = 0.1
ORANGE, BLACK = (1.0, 0.53, 0.20), (0.42, 0.44, 0.46)


def box(x0, x1, y0, y1, z0, z1):
    return Box(x1-x0, y1-y0, z1-z0, align=(Align.MIN,)*3).translate((x0,y0,z0))


def cy(x, z, y0, y1, diameter):
    """Cylinder axis +Y; screw, LED and insertion datums share this axis."""
    return Cylinder(diameter/2, y1-y0, align=(Align.CENTER, Align.CENTER, Align.MIN)).rotate(Axis.X, -90).translate((x,y0,z))


def countersunk_hole_z(x,y,z0,z1):
    """Shared vertical clearance + 80-degree seat; large mouth at z1."""
    s = Cylinder(SCREW_CLEARANCE/2,z1-z0+2*BOOLEAN_OVERLAP,
                 align=(Align.CENTER,Align.CENTER,Align.MIN)).translate((x,y,z0-BOOLEAN_OVERLAP))
    s += Cone(SCREW_CLEARANCE/2,COUNTERSINK_DIAMETER/2,COUNTERSINK_DEPTH,
              align=(Align.CENTER,Align.CENTER,Align.MIN)).translate((x,y,z1-COUNTERSINK_DEPTH))
    return s


def countersunk_hole_y(x,z,y0,y1):
    return countersunk_hole_z(x,-z,y0,y1).rotate(Axis.X,-90)


def profile(points, y0, y1):
    face = Face(Wire.make_polygon([(x,y0,z) for x,z in points], close=True))
    return extrude(face, amount=y1-y0, dir=(0,1,0))


def label(shape, name, color=BLACK):
    shape.label = name
    shape.color = Color(*color)
    return shape


def y_face_edges(shape, y, outer_only=False):
    """Edges on a named front/rear datum, independent of topology numbering."""
    faces = [f for f in shape.faces() if abs(f.bounding_box().min.Y-y)<1e-6
             and abs(f.bounding_box().max.Y-y)<1e-6]
    return [e for f in faces for e in (f.outer_wire().edges() if outer_only else f.edges())]


def tower_blank(y0,y1):
    s = box(35,63,y0,y1,0,28) + box(40,58,y0,y1,27,76)
    # Same profile for shell and cover keeps the beveled seam aligned.
    return chamfer(s.edges().filter_by(Axis.Y),length=TOWER_CHAMFER)


def mascot():
    def bevel(piece,front,amount):
        piece=chamfer(piece.edges().filter_by(Axis.Y),length=BODY_CHAMFER)
        return chamfer(y_face_edges(piece,front),length=amount)

    s = bevel(box(-25,25,0,40,15,55),0,FACE_CHAMFER)
    # Rear faces flush to Y=40: every foot and appendage starts ON the bed.
    for x in (-25,-15,10,20):
        s += bevel(box(x,x+5,5,40,0,16),5,APPENDAGE_CHAMFER)
    s += bevel(box(-30,-24,5,40,25,40),5,APPENDAGE_CHAMFER)
    s += bevel(profile([(24,25),(30,25),(30,29),(35,29),(40,34),
                       (40,41),(35,36),(30,36),(30,40),(24,40)],5,40),5,APPENDAGE_CHAMFER)
    # 45-degree bed chamfers preserve support-free printing and reduce first-layer flare.
    s = chamfer(y_face_edges(s,40),length=BED_CHAMFER)
    # Shallow open arm recess, retaining at least 5.8 mm material locally.
    recess = profile([(30,34.8),(35,34.8),(40,39.8),(40,44),(30,44)],12,28)
    # In rear-down print pose, Y-end caps would be horizontal shelves.
    # Taper both ends until their remaining caps are entirely outside the arm.
    recess = chamfer(y_face_edges(recess,12)+y_face_edges(recess,28),
                     length=HAND_RECESS_END_CHAMFER)
    s -= chamfer(recess.edges().filter_by(Axis.Y),length=SMALL_CHAMFER)
    for x in (-12.5,12.5):
        half_w, half_h = EYE_TONGUE_WIDTH/2+EYE_CLEARANCE, EYE_TONGUE_HEIGHT/2+EYE_CLEARANCE
        s -= box(x-half_w,x+half_w,-BOOLEAN_OVERLAP,EYE_POCKET_DEPTH,40-half_h,40+half_h)
    return label(s, 'orange_mascot_four_feet_bent_arm', ORANGE)


def eye(x):
    s = box(x-2.5,x+2.5,-3,0,35,45)
    s = chamfer(s.edges().filter_by(Axis.Y),length=EYE_CORNER_CHAMFER)
    s = chamfer(y_face_edges(s,-3),length=EYE_CHAMFER)
    s += box(x-EYE_TONGUE_WIDTH/2,x+EYE_TONGUE_WIDTH/2,0,EYE_TONGUE_DEPTH,
             40-EYE_TONGUE_HEIGHT/2,40+EYE_TONGUE_HEIGHT/2)
    return label(s, 'black_eye_left' if x<0 else 'black_eye_right')


def tower():
    outer = tower_blank(0,REAR)
    outer = chamfer(y_face_edges(outer,0),length=BED_CHAMFER)
    s = outer
    # Continuous open rear cavity; roof shoulders taper internally at 45 degrees.
    inner = [(37.4,2.4),(60.6,2.4),(60.6,23.2),(55.6,28.2),
             (55.6,73.6),(42.4,73.6),(42.4,28.2),(37.4,23.2)]
    s -= profile(inner,WALL,REAR+1)
    for x,z in COVER_SCREWS:
        s += cy(x,z,0,REAR,COVER_BOSS_DIAMETER)
        s -= cy(x,z,REAR-COVER_PILOT_DEPTH,REAR+BOOLEAN_OVERLAP,SCREW_PILOT_DIAMETER)
    for x,z in LED_SCREWS:
        s += cy(x,z,0,RETAINER_MOUNT_Y,LED_BOSS_DIAMETER)
        s -= cy(x,z,RETAINER_MOUNT_Y-RETAINER_PILOT_DEPTH,
                RETAINER_MOUNT_Y+BOOLEAN_OVERLAP,SCREW_PILOT_DIAMETER)
    for z,d in zip(LED_Z,LED_DIAMETERS):
        s -= cy(49,z,-0.1,WALL+0.1,d+LED_BORE_CLEARANCE)
        # Small front bezel; the rear bore and flange-bearing plane are untouched.
        r=(d+LED_BORE_CLEARANCE)/2
        s -= Cone(r+LED_BORE_LEAD_IN,r,LED_BORE_LEAD_IN,align=(Align.CENTER,Align.CENTER,Align.MIN)).rotate(Axis.X,-90).translate((49,0,z))
    # Tray shelves supported continuously from the front skin in print orientation.
    s += box(37.3,40.1,WALL-0.1,REAR,3.0,5.0)
    s += box(57.9,60.7,WALL-0.1,REAR,3.0,5.0)
    # 1.3 mm-deep tie tunnels through shelves; the tray still bears either side.
    # Their 1.3 mm return edges are the only short internal bridges here.
    for y in (PCB_REAR-PCB_LENGTH+5, PCB_REAR-5):
        for x0,x1 in ((37.3,40.2),(57.8,60.7)):
            s -= box(x0,x1,y-1.8,y+1.8,3.7,5.1)
    # Front stop carries USB insertion force; cover takes cable extraction force.
    s += box(37.4,60.6,WALL-0.1,PCB_REAR-PCB_LENGTH-2.3-FIT,5.0,7.0)
    return label(s & outer,'black_tower_open_rear')


def rear_cover():
    s = tower_blank(REAR,40)
    s = chamfer(y_face_edges(s,40),length=BED_CHAMFER)
    opening = box(49-USB_OPENING_WIDTH/2,49+USB_OPENING_WIDTH/2,REAR-0.1,40.1,
                  USB_CENTER_Z-USB_OPENING_HEIGHT/2,USB_CENTER_Z+USB_OPENING_HEIGHT/2)
    s -= chamfer(opening.edges().filter_by(Axis.Y),length=USB_CORNER_CHAMFER)
    # Only the USB mouth edges: preserve the screw-head seating annuli.
    mouth = [e for e in y_face_edges(s,40) if e.bounding_box().min.X>41
             and e.bounding_box().max.X<57 and e.bounding_box().max.Z<18]
    s = chamfer(mouth,length=0.3)
    for x,z in COVER_SCREWS:
        s -= countersunk_hole_y(x,z,REAR,DEPTH)
    return label(s,'black_rear_cover_three_screws')


def led_retainer():
    s = box(RETAINER_LEFT,RETAINER_RIGHT,RETAINER_MOUNT_Y,RETAINER_BACK_Y,
            RETAINER_BOTTOM,RETAINER_TOP)
    s = chamfer(s.edges().filter_by(Axis.Y),length=SMALL_CHAMFER)
    s = chamfer(y_face_edges(s,RETAINER_BACK_Y,outer_only=True),length=0.15)
    # Raised flange pads preserve the original 0.25 mm LED play while longer
    # screw posts move the screw tips away from the front skin. Rear face prints down.
    for z,d in zip(LED_Z,LED_FLANGES):
        s += box(RETAINER_LEFT,RETAINER_RIGHT,RETAINER_Y,RETAINER_MOUNT_Y+BOOLEAN_OVERLAP,
                 z-d/2,z+d/2)
    for z in LED_Z:
        slot = box(LEAD_SLOT_X,RETAINER_RIGHT+1,RETAINER_Y-BOOLEAN_OVERLAP,
                   RETAINER_BACK_Y+BOOLEAN_OVERLAP,z-LEAD_SLOT/2,z+LEAD_SLOT/2)
        s -= chamfer(slot.edges().filter_by(Axis.Y),length=SMALL_CHAMFER)
    for x,z in LED_SCREWS:
        s -= countersunk_hole_y(x,z,RETAINER_MOUNT_Y,RETAINER_BACK_Y)
    return label(s,'black_LED_retainer_countersunk_screws_forward_flange_pads')


def pcb_tray():
    left, right = 49-PCB_WIDTH/2, 49+PCB_WIDTH/2
    front = PCB_REAR-PCB_LENGTH
    tray_left, tray_right = left-1.6, right+1.6
    s = box(tray_left,tray_right,front-2.3,REAR-FIT,5,6.6)
    s = chamfer(s.faces().sort_by(Axis.Z)[0].edges(),length=0.25)
    # Edge guides avoid clamping undocumented top components.
    for x0,x1 in ((tray_left,left-FIT),(right+FIT,tray_right)):
        s += box(x0,x1,front-2.3,REAR-FIT,6.6,PCB_BOTTOM+PCB_THICKNESS+0.2)
    # Four small bare-edge bearing pads; underside keepout remains open.
    for x0,x1 in ((left-FIT,left+1.5),(right-1.5,right+FIT)):
        for y0,y1 in ((front-FIT,front+1.5),(PCB_REAR-1.5,PCB_REAR+FIT)):
            s += box(x0,x1,y0,y1,6.5,PCB_BOTTOM)
    # Front/end stops contact PCB substrate, never USB shell or solder joints.
    s += box(left-FIT,right+FIT,front-1.5,front-FIT,6.5,PCB_BOTTOM+PCB_THICKNESS)
    for x0,x1 in ((left-FIT,left+3),(right-3,right+FIT)):
        s += box(x0,x1,PCB_REAR+FIT,PCB_REAR+1.5,6.5,PCB_BOTTOM+PCB_THICKNESS)
    # Two 2.5 mm ties; fit on bare PCB areas, adjust Y positions if required.
    for y in (front+5,PCB_REAR-5):
        for x in (left-0.8,right+0.8):
            s -= box(x-0.55,x+0.55,y-1.6,y+1.6,4.9,11.0)
    outline=box(tray_left,tray_right,front-2.3,REAR-FIT,5,PCB_BOTTOM+PCB_THICKNESS+0.2)
    s &= chamfer(outline.edges().filter_by(Axis.Z),length=SMALL_CHAMFER)
    # Soften guide ends, away from the thin tie-slot ligaments and PCB supports.
    ends=[e for e in s.edges() if abs(e.bounding_box().min.Z-(PCB_BOTTOM+PCB_THICKNESS+0.2))<1e-6
          and abs(e.bounding_box().max.Z-(PCB_BOTTOM+PCB_THICKNESS+0.2))<1e-6
          and (abs(e.center().Y-(front-2.3))<1e-6 or abs(e.center().Y-(REAR-FIT))<1e-6)]
    s = chamfer(ends,length=0.2)
    return label(s,'black_removable_PCB_tray')


def electronics():
    left, right = 49-PCB_WIDTH/2, 49+PCB_WIDTH/2
    front = PCB_REAR-PCB_LENGTH
    pcb = label(box(left,right,front,PCB_REAR,PCB_BOTTOM,PCB_BOTTOM+PCB_THICKNESS),
                'REFERENCE_ONLY_provisional_PCB',(0.05,0.35,0.22))
    # Central component envelopes exclude intentionally supported bare PCB edges.
    lower = box(left+1.6,right-1.6,front+1.6,PCB_REAR-1.6,PCB_BOTTOM-PCB_UNDERSIDE,PCB_BOTTOM)
    upper = box(left+0.5,right-0.5,front+0.5,PCB_REAR-3,PCB_BOTTOM+PCB_THICKNESS,
                PCB_BOTTOM+PCB_THICKNESS+PCB_TOP_HEIGHT)
    usb = label(box(49-USB_WIDTH/2,49+USB_WIDTH/2,PCB_REAR-3,USB_REAR,
                   USB_CENTER_Z-USB_HEIGHT/2,USB_CENTER_Z+USB_HEIGHT/2),
                'REFERENCE_ONLY_USB_shell',(0.6,0.63,0.65))
    parts = {'pcb':pcb,'underside':label(lower,'REFERENCE_ONLY_underside_keepout'),
             'top':label(upper,'REFERENCE_ONLY_top_keepout'),'usb':usb}
    for i,(z,d,f,c) in enumerate(zip(LED_Z,LED_DIAMETERS,LED_FLANGES,
                                    ((0.04,0.75,0.1),(1,0.7,0.01),(0.8,0.015,0.015)))):
        tip_center = WALL-LED_BARREL_LENGTH+d/2
        dome = Sphere(d/2).translate((49,tip_center,z))
        dome = dome & box(49-d,49+d,tip_center-d,tip_center,z-d,z+d)
        s = cy(49,z,tip_center,WALL,d) + dome
        s += cy(49,z,WALL,WALL+LED_FLANGE_THICKNESS,f)
        parts[f'led_{i}'] = label(s,f'REFERENCE_ONLY_LED_{i}',c)
    return parts


@lru_cache(maxsize=1)
def parts():
    assert PCB_WIDTH+3.2 < 23.2, 'PCB tray exceeds pedestal; enlarge pedestal first'
    assert PCB_REAR-PCB_LENGTH-2.3-FIT > WALL, 'PCB too long for pedestal'
    assert max(LED_FLANGES)<8.5, 'Widen/reposition retainer and screw bosses for larger LEDs'
    return {'01_mascot_orange':mascot(),'02_eye_left_black':eye(-12.5),
            '03_eye_right_black':eye(12.5),'04_tower_black':tower(),
            '05_rear_cover_black':rear_cover(),'06_LED_retainer_black':led_retainer(),
            '07_PCB_tray_black':pcb_tray()}


def gen_step():
    # Static assembly coordinates are the named desk/front/rear and screw datums above.
    return Compound(children=list(parts().values()),label='Claw_Light_printed_assembly_mm')


def print_pose(name, shape):
    if name.startswith('01'):
        s = shape.rotate(Axis.X,-90)  # z'=-Y: rear really is lowest
    elif name.startswith(('02','03','04')):
        s = shape.rotate(Axis.X,90)  # visible front DOWN; z'=Y
    elif name.startswith('06'):
        s = shape.rotate(Axis.X,-90)  # countersunk rear DOWN; flange pads build upward
    elif name.startswith('05'):
        s = shape.rotate(Axis.X,90)
    else:
        s = shape
    lo = s.bounding_box().min
    return s.translate((-lo.X,-lo.Y,-lo.Z))


def engraved_label(text,x,y,z,size=COUPON_LABEL_SIZE):
    letters = Text(text,font_size=size,font="Arial")
    return extrude(letters,amount=COUPON_LABEL_DEPTH+BOOLEAN_OVERLAP).translate((x,y,z-COUPON_LABEL_DEPTH))


def coupon():
    s = box(0,COUPON_WIDTH,0,COUPON_LENGTH,0,COUPON_BASE_THICKNESS)
    s = chamfer(s.edges().filter_by(Axis.Z),length=COUPON_CORNER_CHAMFER)
    s = chamfer(s.faces().sort_by(Axis.Z)[0].edges(),length=COUPON_BED_CHAMFER)
    for x,d in zip(COUPON_COLUMNS,COUPON_LED_DIAMETERS):
        s -= Cylinder(d/2,COUPON_BASE_THICKNESS+2*BOOLEAN_OVERLAP,
                      align=(Align.CENTER,Align.CENTER,Align.MIN)).translate((x,COUPON_LED_Y,-BOOLEAN_OVERLAP))
        s -= Cone(d/2+LED_BORE_LEAD_IN,d/2,LED_BORE_LEAD_IN,
                  align=(Align.CENTER,Align.CENTER,Align.MIN)).translate((x,COUPON_LED_Y,0))
        s -= engraved_label(f'{d:.1f}',x,COUPON_LED_Y+6,COUPON_BASE_THICKNESS)
    half_w=EYE_TONGUE_WIDTH/2+EYE_CLEARANCE
    half_h=EYE_TONGUE_HEIGHT/2+EYE_CLEARANCE
    s -= box(COUPON_EYE_X-half_w,COUPON_EYE_X+half_w,COUPON_EYE_Y-half_h,COUPON_EYE_Y+half_h,
             COUPON_BASE_THICKNESS-EYE_POCKET_DEPTH,COUPON_BASE_THICKNESS+BOOLEAN_OVERLAP)
    for row,depth,boss_d,name in ((COUPON_COVER_Y,COVER_PILOT_DEPTH,COVER_BOSS_DIAMETER,'C'),
                                 (COUPON_RETAINER_Y,RETAINER_PILOT_DEPTH,LED_BOSS_DIAMETER,'R')):
        for x,d in zip(COUPON_COLUMNS,COUPON_PILOT_DIAMETERS):
            s += Cylinder(boss_d/2,COUPON_POST_HEIGHT+BOOLEAN_OVERLAP,
                          align=(Align.CENTER,Align.CENTER,Align.MIN)).translate((x,row,COUPON_BASE_THICKNESS-BOOLEAN_OVERLAP))
            s -= Cylinder(d/2,depth+BOOLEAN_OVERLAP,align=(Align.CENTER,Align.CENTER,Align.MIN)).translate((x,row,COUPON_POST_TOP-depth))
            s -= engraved_label(f'{d:.1f}',x,row+6,COUPON_BASE_THICKNESS)
        s -= engraved_label(name,60,row,COUPON_BASE_THICKNESS,size=4)
    return label(s,'fit_coupon_LED_5p1_5p3_5p5_pilots_1p7_1p8_1p9_2p0')


def coupon_cap(thickness,name):
    s = box(0,COUPON_CAP_WIDTH,0,COUPON_CAP_LENGTH,0,thickness)
    s = chamfer(s.edges().filter_by(Axis.Z),length=SMALL_CHAMFER)
    s = chamfer(s.faces().sort_by(Axis.Z)[0].edges(),length=COUPON_BED_CHAMFER)
    for x in COUPON_COLUMNS:
        s -= countersunk_hole_z(x,COUPON_CAP_LENGTH/2,0,thickness)
    s -= engraved_label(name,COUPON_CAP_WIDTH-3,COUPON_CAP_LENGTH/2,thickness)
    return label(s,'coupon_'+name+'_80_degree_countersunk_cap')


def coupon_parts():
    retainer_cap=coupon_cap(RETAINER_THICKNESS,'R').rotate(Axis.X,180)
    lo=retainer_cap.bounding_box().min
    retainer_cap=retainer_cap.translate((-lo.X,-lo.Y,-lo.Z))
    return {'00_fit_coupon':coupon(),
            '00b_coupon_cover_cap':coupon_cap(COVER,'C'),
            '00c_coupon_retainer_cap':retainer_cap}


def export_all():
    (OUT/'stl').mkdir(exist_ok=True)
    metadata = {}
    for name,s in parts().items():
        p = print_pose(name,s)
        export_stl(p,str(OUT/'stl'/f'{name}.stl'),tolerance=0.02,angular_tolerance=0.05)
        metadata[name] = {'quantity':1,'bounds_mm':list(p.bounding_box().size),
                          'solid_volume_mm3':s.volume,'assembly_label':s.label}
    for name,c in coupon_parts().items():
        export_stl(c,str(OUT/'stl'/f'{name}.stl'),tolerance=0.02,angular_tolerance=0.05)
        metadata[name] = {'quantity':1,'bounds_mm':list(c.bounding_box().size),
                          'solid_volume_mm3':c.volume,'assembly_label':c.label}
    export_step(gen_step(),str(OUT/'clawlight.step'))
    with open(OUT/'parts.json','w') as f:
        json.dump(metadata,f,indent=2)
    return metadata


if __name__=='__main__':
    print(json.dumps(export_all(),indent=2))
