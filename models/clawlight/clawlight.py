"""Claw Light: millimetres, X right, Y rear, Z up; desk Z=0.

Run with build123d 0.11.1: python clawlight.py
Screw pilots and LED bores use the user's coupon results. PCB outline and
LED body/flanges and USB shell are measured; component envelopes
remain provisional. Measurements and coupon tests: 2026-09-17.
Print exports are individually oriented and translated onto Z=0.
"""
from pathlib import Path
from functools import lru_cache
from math import radians, tan
import json

from build123d import (
    Align, Axis, Box, Color, Compound, Cone, Cylinder, Face,
    Plane, Sphere, Wire, chamfer, export_step, export_stl, extrude,
)

import hand_joint as joint

OUT = Path(__file__).resolve().parent
WALL = 2.4
BASE_FLOOR_THICKNESS = 2.0
DEPTH = 27.4
MASCOT_DEPTH = 20.5  # Independent of electronics depth; leave room for both hand pins.
MASCOT_SCALE = 0.65
TOWER_X = 33.0
BASE_WIDTH = 24.0
BASE_HEIGHT = 15.2
UPRIGHT_WIDTH = 14.0
TOWER_HEIGHT = 45.5
UPRIGHT_WALL = 1.8
BASE_SIDE_WALL = 2.0
BASE_LEFT, BASE_RIGHT = TOWER_X-BASE_WIDTH/2, TOWER_X+BASE_WIDTH/2
UPRIGHT_LEFT, UPRIGHT_RIGHT = TOWER_X-UPRIGHT_WIDTH/2, TOWER_X+UPRIGHT_WIDTH/2
INNER_LEFT, INNER_RIGHT = BASE_LEFT+BASE_SIDE_WALL, BASE_RIGHT-BASE_SIDE_WALL
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
COVER_INNER_CHAMFER = 0.2
EYE_CORNER_CHAMFER = 0.35
EYE_CHAMFER = 0.25
EYE_TONGUE_CORNER_CHAMFER = 0.50  # Scales with the printed eye, unlike hardware bores.
EYE_TONGUE_ENTRY_CHAMFER = 0.25
USB_CORNER_CHAMFER = 0.15  # Keep the narrower throat clear of the USB shell corners.
SMALL_CHAMFER = 0.3
LED_DIAMETERS = (4.75, 4.75, 4.75)  # measured; green, yellow, red
LED_FLANGES = (5.4, 5.4, 5.4)
LED_FLANGE_THICKNESS = 1.2
LED_TOTAL_LENGTH = 8.5  # User caliper measurement: tip to bottom of the plastic, flange included.
LED_BARREL_LENGTH = LED_TOTAL_LENGTH-LED_FLANGE_THICKNESS
LED_PROTRUSION = 3.5
LED_SEAT_Y = LED_BARREL_LENGTH-LED_PROTRUSION
LED_COLLAR_DIAMETER = 6.3
LED_PITCH = 6.5
LED_Z = tuple(27.0 + i * LED_PITCH for i in range(3))
LED_BORE_DIAMETERS = (5.1, 5.1, 5.1)  # coupon fit, independently of measured LED body
LED_BORE_LEAD_IN = 0.4  # 45-degree entry, leaving 0.6 mm between adjacent mouths.
LED_AXIAL_CLEARANCE = 0.25
LED_SUPPORT_Y = LED_SEAT_Y+LED_FLANGE_THICKNESS+LED_AXIAL_CLEARANCE
RETAINER_LEFT, RETAINER_RIGHT = TOWER_X-3.9, TOWER_X-.7
RETAINER_FOOT_RIGHT = TOWER_X+3.2
RETAINER_BOTTOM = 17.5
RETAINER_MOUNT_Y, RETAINER_BACK_Y = 6.0, 9.0
RETAINER_THICKNESS = RETAINER_BACK_Y-RETAINER_MOUNT_Y
RETAINER_KEY_ENGAGEMENT = 1.0
RETAINER_TOP = TOWER_HEIGHT-WALL+RETAINER_KEY_ENGAGEMENT
RETAINER_KEY_CLEARANCE = 0.25
RETAINER_RELEASE_DROP = 1.3
RETAINER_PAD_HALF_HEIGHT = 1.5
RETAINER_CHAMFER = 0.15
PCB_WIDTH = 18.0
PCB_LENGTH = 22.4
PCB_THICKNESS = 0.7
PCB_LENGTH_WITH_USB = 23.9  # measured overall length: USB projects 1.5 mm
PCB_UNDERSIDE = 0.6  # Provisional component/solder envelope below the board.
PCB_TOP_HEIGHT = 4.0
PCB_FRONT = 3.75
PCB_REAR = PCB_FRONT+PCB_LENGTH
# Integral guides lower the board 1.8 mm; retain 0.2 mm below underside components.
PCB_BOTTOM = 2.8
PCB_TOP = PCB_BOTTOM+PCB_THICKNESS
PCB_FLOOR_GAP = PCB_BOTTOM-BASE_FLOOR_THICKNESS
PCB_SIDE_CLEARANCE = 0.10
PCB_END_CLEARANCE = 0.25
PCB_TOP_CLEARANCE = 0.10
PCB_RAIL_LOWER_OVERLAP = 0.60  # Narrow bearing strip leaves GPIO solder pads accessible.
PCB_RAIL_UPPER_OVERLAP = 0.30
PCB_RAIL_ROOF_THICKNESS = 1.0
PCB_RAIL_TOP = PCB_TOP+PCB_TOP_CLEARANCE+PCB_RAIL_ROOF_THICKNESS
PCB_RAIL_ENTRY_CHAMFER = 0.25
PCB_RAIL_EDGE_CHAMFER = 0.10
PCB_FRONT_STOP_Y = PCB_FRONT-PCB_END_CLEARANCE
PCB_CORNER_LENGTH = 2.5  # Across each non-USB end corner; the middle 13 mm stays open.
PCB_CORNER_OVERLAP = 0.60
PCB_CORNER_ENTRY_CHAMFER = 0.15
# User-requested corner holders assume these small patches are free of components.
PCB_CORNER_COMPONENT_CLEARANCE = 0.20
PCB_CORNER_CLEAR_DEPTH = 1.0
USB_WIDTH, USB_HEIGHT = 8.9, 3.1
USB_SEATING_Z_OFFSET = 0.2  # Latest print: lower the previous +1.2 mm correction by 1 mm.
USB_CENTER_ABOVE_PCB = USB_HEIGHT/2 + USB_SEATING_Z_OFFSET
USB_CENTER_Z = PCB_BOTTOM + PCB_THICKNESS + USB_CENTER_ABOVE_PCB
USB_REAR = PCB_REAR + PCB_LENGTH_WITH_USB - PCB_LENGTH
USB_CLEARANCE_X, USB_CLEARANCE_Z = 0.125, 0.05  # Per side; opening reduced 0.15 x 0.30 mm.
USB_OPENING_WIDTH = USB_WIDTH+2*USB_CLEARANCE_X
USB_OPENING_HEIGHT = USB_HEIGHT+2*USB_CLEARANCE_Z
USB_ENTRY_CHAMFER = 0.2
USB_CABLE_BODY_START = USB_REAR+0.1
COVER_INNER_SKIN = 1.0
COVER_POCKET_BACK = DEPTH-COVER_INNER_SKIN
COVER_POCKET_CLEARANCE = 0.25
COVER_POCKET_LEFT = INNER_LEFT-COVER_POCKET_CLEARANCE
COVER_POCKET_RIGHT = INNER_RIGHT+COVER_POCKET_CLEARANCE
COVER_POCKET_BOTTOM = BASE_FLOOR_THICKNESS-COVER_POCKET_CLEARANCE
COVER_POCKET_TOP = max(PCB_TOP+PCB_TOP_HEIGHT,PCB_RAIL_TOP,
                       USB_CENTER_Z+USB_OPENING_HEIGHT/2+USB_ENTRY_CHAMFER)+COVER_POCKET_CLEARANCE
PCB_REAR_STOP_INSET = 1.5
PCB_REAR_STOP_FRONT = PCB_REAR+PCB_END_CLEARANCE
PCB_REAR_STOP_Z_CLEARANCE = 0.2
WIRE_CORRIDOR_DEPTH = 6.0
WIRE_REAR_CLEARANCE = 0.5
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
SCREW_PILOT_DIAMETER = 1.9  # user coupon choice for both C/R, same print setup as LED bores
SCREW_TIP_CLEARANCE = 0.3
SCREW_TIP_WALL = 0.8
SCREW_POINT_LENGTH_MAX = 2.0
SCREW_DRIVER_DIAMETER = 4.0
COUNTERSINK_DIAMETER = SCREW_HEAD_DIAMETER_MAX + SCREW_HEAD_CLEARANCE
COUNTERSINK_DEPTH = (COUNTERSINK_DIAMETER-SCREW_CLEARANCE)/2/tan(radians(SCREW_HEAD_ANGLE_DEG/2))
MAX_HEAD_SEATING_DEPTH = (COUNTERSINK_DIAMETER-SCREW_HEAD_DIAMETER_MIN)/2/tan(radians(SCREW_HEAD_ANGLE_DEG/2))
COVER_BOSS_DIAMETER = 5.6
COVER_SCREWS = ((TOWER_X-8.5,10.5),(TOWER_X+8.5,10.5),(TOWER_X+4.0,41.2))
COVER_PILOT_DEPTH = SCREW_LENGTH+SCREW_LENGTH_TOLERANCE+MAX_HEAD_SEATING_DEPTH-COVER+SCREW_TIP_CLEARANCE
COVER_BOSS_FRONT = REAR-COVER_PILOT_DEPTH-SCREW_TIP_WALL
LED_SCREWS = ((TOWER_X-.5,20.5),)
LED_BOSS_DIAMETER = 5.6
RETAINER_PILOT_DEPTH = SCREW_LENGTH+SCREW_LENGTH_TOLERANCE+MAX_HEAD_SEATING_DEPTH-RETAINER_THICKNESS+SCREW_TIP_CLEARANCE

EYE_TONGUE_WIDTH, EYE_TONGUE_HEIGHT, EYE_TONGUE_DEPTH = 4.9, 9.9, 1.15
EYE_POCKET_DEPTH = 1.4
HAND_SOCKET_WALL = 0.8
HAND_SOCKET_GUSSET_OVERLAP = 0.1
HAND_SOCKET_MOUTH_CHAMFER = 0.15
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
    s = box(BASE_LEFT,BASE_RIGHT,y0,y1,0,BASE_HEIGHT) + box(UPRIGHT_LEFT,UPRIGHT_RIGHT,y0,y1,BASE_HEIGHT-1,TOWER_HEIGHT)
    # Same profile for shell and cover keeps the beveled seam aligned.
    return chamfer(s.edges().filter_by(Axis.Y),length=TOWER_CHAMFER)


def mascot():
    def bevel(piece,front,amount):
        piece=chamfer(piece.edges().filter_by(Axis.Y),length=BODY_CHAMFER)
        return chamfer(y_face_edges(piece,front),length=amount)

    rear = MASCOT_DEPTH/MASCOT_SCALE
    s = bevel(box(-25,25,0,rear,15,55),0,FACE_CHAMFER)
    # Change the rear datum before uniform scaling to preserve 45-degree ramps.
    for x in (-25,-15,10,20):
        s += bevel(box(x,x+5,5,rear,0,16),5,APPENDAGE_CHAMFER)
    s += bevel(box(-30,-24,5,rear,25,40),5,APPENDAGE_CHAMFER)
    s += bevel(profile([(24,25),(30,25),(30,29),(35,29),(40,34),
                       (40,41),(35,36),(30,36),(30,40),(24,40)],5,rear),5,APPENDAGE_CHAMFER)
    # 45-degree bed chamfers preserve support-free printing and reduce first-layer flare.
    s = chamfer(y_face_edges(s,rear),length=BED_CHAMFER)
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
    s = s.scale(MASCOT_SCALE)
    for pin in joint.hand_pins().solids():
        s += pin
    return label(s, 'orange_mascot_four_feet_straight_pins_with_45_degree_haunches', ORANGE)


def eye(x):
    s = box(x-2.5,x+2.5,-3,0,35,45)
    s = chamfer(s.edges().filter_by(Axis.Y),length=EYE_CORNER_CHAMFER)
    s = chamfer(y_face_edges(s,-3),length=EYE_CHAMFER)
    tongue = box(x-EYE_TONGUE_WIDTH/2,x+EYE_TONGUE_WIDTH/2,0,EYE_TONGUE_DEPTH,
                 40-EYE_TONGUE_HEIGHT/2,40+EYE_TONGUE_HEIGHT/2)
    tongue = chamfer(tongue.edges().filter_by(Axis.Y),length=EYE_TONGUE_CORNER_CHAMFER)
    tongue = chamfer(y_face_edges(tongue,EYE_TONGUE_DEPTH),length=EYE_TONGUE_ENTRY_CHAMFER)
    s += tongue
    return label(s.scale(MASCOT_SCALE), 'black_eye_left' if x<0 else 'black_eye_right')


def cover_screw_boss(x,z):
    """Short full-diameter screw land with a 45-degree lead-in from its side wall."""
    if z<BASE_HEIGHT:
        wall_x=INNER_LEFT if x<TOWER_X else INNER_RIGHT
    else:
        wall_x=UPRIGHT_LEFT+UPRIGHT_WALL if x<TOWER_X else UPRIGHT_RIGHT-UPRIGHT_WALL
    inward=1 if x>wall_x else -1
    root=wall_x-inward*BOOLEAN_OVERLAP
    tip=x+inward*COVER_BOSS_DIAMETER/2
    start=COVER_BOSS_FRONT-abs(tip-root)
    face=Face(Wire.make_polygon([(root,start,z-COVER_BOSS_DIAMETER/2),
                                 (tip,COVER_BOSS_FRONT,z-COVER_BOSS_DIAMETER/2),
                                 (tip,REAR,z-COVER_BOSS_DIAMETER/2),
                                 (root,REAR,z-COVER_BOSS_DIAMETER/2)],close=True))
    ramp=extrude(face,amount=COVER_BOSS_DIAMETER,dir=(0,0,1))
    return cy(x,z,start,REAR,COVER_BOSS_DIAMETER) & ramp


def tower():
    outer = tower_blank(0,REAR)
    outer = chamfer(y_face_edges(outer,0),length=BED_CHAMFER)
    s = outer
    # Continuous open rear cavity; roof shoulders taper internally at 45 degrees.
    neck_left, neck_right = UPRIGHT_LEFT+UPRIGHT_WALL, UPRIGHT_RIGHT-UPRIGHT_WALL
    shoulder_top = BASE_HEIGHT-WALL
    shoulder_bottom = shoulder_top-(neck_left-INNER_LEFT)
    inner = [(INNER_LEFT,BASE_FLOOR_THICKNESS),(INNER_RIGHT,BASE_FLOOR_THICKNESS),(INNER_RIGHT,shoulder_bottom),
             (neck_right,shoulder_top),(neck_right,TOWER_HEIGHT-WALL),
             (neck_left,TOWER_HEIGHT-WALL),(neck_left,shoulder_top),(INNER_LEFT,shoulder_bottom)]
    s -= profile(inner,WALL,REAR+1)
    for x,z in COVER_SCREWS:
        s += cover_screw_boss(x,z)
        s -= cy(x,z,REAR-COVER_PILOT_DEPTH,REAR+BOOLEAN_OVERLAP,SCREW_PILOT_DIAMETER)
    for x,z in LED_SCREWS:
        s += cy(x,z,0,RETAINER_MOUNT_Y,LED_BOSS_DIAMETER)
        s -= cy(x,z,RETAINER_MOUNT_Y-RETAINER_PILOT_DEPTH,
                RETAINER_MOUNT_Y+BOOLEAN_OVERLAP,SCREW_PILOT_DIAMETER)
    s -= retainer_key_pocket()
    for z,d in zip(LED_Z,LED_BORE_DIAMETERS):
        # A front-rooted collar sets LED projection using its flange shoulder.
        s += cy(TOWER_X,z,WALL-BOOLEAN_OVERLAP,LED_SEAT_Y,LED_COLLAR_DIAMETER)
        s -= cy(TOWER_X,z,-BOOLEAN_OVERLAP,LED_SEAT_Y+BOOLEAN_OVERLAP,d)
        # Small front bezel; the rear bore and flange-bearing plane are untouched.
        r=d/2
        s -= Cone(r+LED_BORE_LEAD_IN,r,LED_BORE_LEAD_IN,align=(Align.CENTER,Align.CENTER,Align.MIN)).rotate(Axis.X,-90).translate((TOWER_X,0,z))
    s = s & outer
    # Fixed C-guides capture the PCB edges directly; they grow from the front
    # wall in the +Y print direction and stop at the cover seam.
    s += pcb_guides()
    for y,z in joint.JOINT_CENTERS_YZ:
        # A blind reinforced socket leaves the inside cavity closed. Shared
        # nested sections allow straight insertion; the rear haunch prints at 45 degrees.
        s += joint.diamond_socket(joint.JOINT_X,joint.JOINT_X+joint.SOCKET_DEPTH+HAND_SOCKET_WALL,
                                  y,z,joint.SOCKET_DIAGONAL+2*HAND_SOCKET_WALL)
        # In front-down printing the rear socket boss starts inside the hollow
        # cavity. A 45-degree leading gusset grows it from the existing side wall.
        wall_x=UPRIGHT_LEFT+UPRIGHT_WALL-HAND_SOCKET_GUSSET_OVERLAP
        tip_x=joint.JOINT_X+joint.SOCKET_DEPTH+HAND_SOCKET_WALL
        radius=(joint.SOCKET_DIAGONAL+2*HAND_SOCKET_WALL)/2
        leading_y=y-radius
        bottom=z-radius
        face=Face(Wire.make_polygon([(wall_x,leading_y-(tip_x-wall_x),bottom),
                                     (tip_x,leading_y,bottom),(tip_x,y+radius,bottom),
                                     (wall_x,y+radius,bottom)],close=True))
        s += extrude(face,amount=2*radius,dir=(0,0,1))
        hole = joint.diamond_socket(joint.JOINT_X-BOOLEAN_OVERLAP,
                                    joint.JOINT_X+joint.SOCKET_DEPTH,
                                    y,z)
        s -= hole
        mouth=[e for e in s.edges() if abs(e.bounding_box().min.X-joint.JOINT_X)<1e-6
               and abs(e.bounding_box().max.X-joint.JOINT_X)<1e-6
               and e.bounding_box().min.Y>y-joint.SOCKET_DIAGONAL
               and e.bounding_box().max.Y<y+joint.SOCKET_DIAGONAL
               and e.bounding_box().min.Z>z-joint.SOCKET_DIAGONAL
               and e.bounding_box().max.Z<z+joint.SOCKET_DIAGONAL]
        s = chamfer(mouth,length=HAND_SOCKET_MOUTH_CHAMFER)
    # Trim the socket reinforcement as well as the rails to the mating plane.
    s &= box(BASE_LEFT,BASE_RIGHT,0,REAR,0,TOWER_HEIGHT)
    return label(s,'black_tower_open_rear_straight_hand_sockets')


def rear_cover():
    s = tower_blank(REAR,DEPTH)
    s = chamfer(y_face_edges(s,DEPTH),length=BED_CHAMFER)
    # Flat exterior prints directly on the bed. All relief opens toward the
    # interior, leaving a 1 mm skin and full 2.4 mm material at the screws.
    s -= box(COVER_POCKET_LEFT,COVER_POCKET_RIGHT,REAR-BOOLEAN_OVERLAP,
             COVER_POCKET_BACK,COVER_POCKET_BOTTOM,COVER_POCKET_TOP)
    # The cover stops PCB withdrawal from the integral guides when unplugging USB.
    left, right = TOWER_X-PCB_WIDTH/2, TOWER_X+PCB_WIDTH/2
    for x0,x1 in ((left,left+PCB_REAR_STOP_INSET),(right-PCB_REAR_STOP_INSET,right)):
        s += box(x0,x1,PCB_REAR_STOP_FRONT,COVER_POCKET_BACK+BOOLEAN_OVERLAP,
                 PCB_BOTTOM-PCB_REAR_STOP_Z_CLEARANCE,PCB_TOP+PCB_REAR_STOP_Z_CLEARANCE)
    aperture = box(TOWER_X-USB_OPENING_WIDTH/2,TOWER_X+USB_OPENING_WIDTH/2,
                   REAR-BOOLEAN_OVERLAP,DEPTH+BOOLEAN_OVERLAP,
                   USB_CENTER_Z-USB_OPENING_HEIGHT/2,USB_CENTER_Z+USB_OPENING_HEIGHT/2)
    s -= chamfer(aperture.edges().filter_by(Axis.Y),length=USB_CORNER_CHAMFER)
    # Lead-ins on both sides retain the same tight USB throat between them.
    for y in (DEPTH,COVER_POCKET_BACK):
        mouth=[e for e in y_face_edges(s,y)
               if e.bounding_box().min.X>=TOWER_X-USB_OPENING_WIDTH/2-1e-6
               and e.bounding_box().max.X<=TOWER_X+USB_OPENING_WIDTH/2+1e-6
               and e.bounding_box().min.Z>=USB_CENTER_Z-USB_OPENING_HEIGHT/2-1e-6
               and e.bounding_box().max.Z<=USB_CENTER_Z+USB_OPENING_HEIGHT/2+1e-6]
        s = chamfer(mouth,length=USB_ENTRY_CHAMFER)
    for x,z in COVER_SCREWS:
        s -= countersunk_hole_y(x,z,REAR,DEPTH)
    # Inner perimeter, pocket mouth and screw-hole entrances share this plane.
    s = chamfer(y_face_edges(s,REAR),length=COVER_INNER_CHAMFER)
    return label(s,'black_flat_rear_cover_USB_and_PCB_end_stops')


def retainer_key_pocket():
    """Roof pocket traps the rigid tab; its rear closes with a printable 45-degree ramp."""
    x0,x1=RETAINER_LEFT-RETAINER_KEY_CLEARANCE,RETAINER_RIGHT+RETAINER_KEY_CLEARANCE
    front=RETAINER_MOUNT_Y-RETAINER_KEY_CLEARANCE
    rear=RETAINER_BACK_Y+RETAINER_KEY_CLEARANCE
    top=RETAINER_TOP+RETAINER_KEY_CLEARANCE
    bottom=TOWER_HEIGHT-WALL-BOOLEAN_OVERLAP
    face=Face(Wire.make_polygon([(x0,front,bottom),(x0,front,top),
                                 (x0,rear,top),(x0,rear+top-bottom,bottom)],close=True))
    return extrude(face,amount=x1-x0,dir=(1,0,0))


def led_retainer():
    """Flat spine with three short flange pads, a rigid roof tab and one lower screw."""
    s=profile([(RETAINER_LEFT,RETAINER_BOTTOM),(RETAINER_FOOT_RIGHT,RETAINER_BOTTOM),
               (RETAINER_FOOT_RIGHT,23.5),(RETAINER_RIGHT,25.0),
               (RETAINER_RIGHT,RETAINER_TOP),(RETAINER_LEFT,RETAINER_TOP)],
              RETAINER_MOUNT_Y,RETAINER_BACK_Y)
    s=chamfer(s.edges().filter_by(Axis.Y),length=RETAINER_CHAMFER)
    for y in (RETAINER_MOUNT_Y,RETAINER_BACK_Y):
        s=chamfer(y_face_edges(s,y),length=RETAINER_CHAMFER)
    for z in LED_Z:
        pad=box(RETAINER_LEFT,RETAINER_RIGHT,LED_SUPPORT_Y,
                RETAINER_MOUNT_Y+RETAINER_CHAMFER+BOOLEAN_OVERLAP,
                z-RETAINER_PAD_HALF_HEIGHT,z+RETAINER_PAD_HALF_HEIGHT)
        pad=chamfer(pad.edges().filter_by(Axis.Y),length=RETAINER_CHAMFER)
        pad=chamfer(y_face_edges(pad,LED_SUPPORT_Y),length=RETAINER_CHAMFER)
        s+=pad
    for x,z in LED_SCREWS:
        s-=countersunk_hole_y(x,z,RETAINER_MOUNT_Y,RETAINER_BACK_Y)
    return label(s,'black_LED_retainer_rigid_roof_tab_one_lower_screw')


def pcb_guides():
    """Wall-braced side channels and two short far-end corner holders."""
    left = TOWER_X-PCB_WIDTH/2
    wall = INNER_LEFT-BOOLEAN_OVERLAP
    tip = left+PCB_RAIL_UPPER_OVERLAP
    rail = profile([(wall,BASE_FLOOR_THICKNESS-BOOLEAN_OVERLAP),
                    (left+PCB_RAIL_LOWER_OVERLAP,BASE_FLOOR_THICKNESS-BOOLEAN_OVERLAP),
                    (left+PCB_RAIL_LOWER_OVERLAP,PCB_BOTTOM),
                    (left-PCB_SIDE_CLEARANCE,PCB_BOTTOM),
                    (left-PCB_SIDE_CLEARANCE,PCB_TOP+PCB_TOP_CLEARANCE),
                    (tip,PCB_TOP+PCB_TOP_CLEARANCE),
                    (tip,PCB_RAIL_TOP),
                    (wall,PCB_RAIL_TOP+tip-wall)],WALL-BOOLEAN_OVERLAP,REAR)
    # Open only the C-slot mouth; keep the wall/floor roots solid to the seam.
    mouth=[e for e in y_face_edges(rail,REAR)
           if (abs(e.bounding_box().min.Z-PCB_BOTTOM)<1e-6
               and abs(e.bounding_box().max.Z-PCB_BOTTOM)<1e-6)
           or (abs(e.bounding_box().min.Z-(PCB_TOP+PCB_TOP_CLEARANCE))<1e-6
               and abs(e.bounding_box().max.Z-(PCB_TOP+PCB_TOP_CLEARANCE))<1e-6)
           or (abs(e.bounding_box().min.X-(left-PCB_SIDE_CLEARANCE))<1e-6
               and abs(e.bounding_box().max.X-(left-PCB_SIDE_CLEARANCE))<1e-6)]
    rail=chamfer(mouth,length=PCB_RAIL_ENTRY_CHAMFER)
    # Bevel the exposed upper edge without reducing the underside's capture land.
    top_tip=[e for e in rail.edges().filter_by(Axis.Y)
             if abs(e.center().X-(left+PCB_RAIL_UPPER_OVERLAP))<1e-6
             and abs(e.center().Z-PCB_RAIL_TOP)<1e-6]
    rail=chamfer(top_tip,length=PCB_RAIL_EDGE_CHAMFER)
    # A short C-section returns around each far corner. Its roof also grows
    # diagonally from the front wall, while the center stays open for components.
    root_y=WALL-BOOLEAN_OVERLAP
    end_y=PCB_FRONT+PCB_CORNER_OVERLAP
    corner_section=[(root_y,BASE_FLOOR_THICKNESS-BOOLEAN_OVERLAP),
                    (end_y,BASE_FLOOR_THICKNESS-BOOLEAN_OVERLAP),
                    (end_y,PCB_BOTTOM),(PCB_FRONT_STOP_Y,PCB_BOTTOM),
                    (PCB_FRONT_STOP_Y,PCB_TOP+PCB_TOP_CLEARANCE),
                    (end_y,PCB_TOP+PCB_TOP_CLEARANCE),(end_y,PCB_RAIL_TOP),
                    (root_y,PCB_RAIL_TOP+end_y-root_y)]
    face=Face(Wire.make_polygon([(wall,y,z) for y,z in corner_section],close=True))
    corner=extrude(face,amount=left+PCB_CORNER_LENGTH-wall,dir=(1,0,0))
    corner=chamfer(y_face_edges(corner,end_y),length=PCB_CORNER_ENTRY_CHAMFER)
    rail+=corner
    return rail+rail.mirror(Plane.YZ.offset(TOWER_X))


def electronics():
    left, right = TOWER_X-PCB_WIDTH/2, TOWER_X+PCB_WIDTH/2
    front = PCB_REAR-PCB_LENGTH
    pcb = label(box(left,right,front,PCB_REAR,PCB_BOTTOM,PCB_BOTTOM+PCB_THICKNESS),
                'REFERENCE_ONLY_provisional_PCB',(0.05,0.35,0.22))
    # Central component envelopes exclude intentionally supported bare PCB edges.
    lower = box(left+1.6,right-1.6,front+1.6,PCB_REAR-1.6,PCB_BOTTOM-PCB_UNDERSIDE,PCB_BOTTOM)
    upper = box(left+0.5,right-0.5,front+0.5,PCB_REAR-3,PCB_BOTTOM+PCB_THICKNESS,
                PCB_BOTTOM+PCB_THICKNESS+PCB_TOP_HEIGHT)
    # Preserve the central end-component envelope, relieving only the two
    # explicitly assumed bare corner patches for the new retaining lips.
    corner_width=PCB_CORNER_LENGTH+PCB_CORNER_COMPONENT_CLEARANCE
    for x0,x1 in ((left,left+corner_width),(right-corner_width,right)):
        upper-=box(x0,x1,front,front+PCB_CORNER_CLEAR_DEPTH,
                   PCB_TOP-BOOLEAN_OVERLAP,PCB_TOP+PCB_TOP_HEIGHT+BOOLEAN_OVERLAP)
    usb = label(box(TOWER_X-USB_WIDTH/2,TOWER_X+USB_WIDTH/2,PCB_REAR-3,USB_REAR,
                   USB_CENTER_Z-USB_HEIGHT/2,USB_CENTER_Z+USB_HEIGHT/2),
                'REFERENCE_ONLY_USB_shell',(0.6,0.63,0.65))
    parts = {'pcb':pcb,'underside':label(lower,'REFERENCE_ONLY_underside_keepout'),
             'top':label(upper,'REFERENCE_ONLY_top_keepout'),'usb':usb}
    for i,(z,d,f,c) in enumerate(zip(LED_Z,LED_DIAMETERS,LED_FLANGES,
                                    ((0.04,0.75,0.1),(1,0.7,0.01),(0.8,0.015,0.015)))):
        tip_center = LED_SEAT_Y-LED_BARREL_LENGTH+d/2
        dome = Sphere(d/2).translate((TOWER_X,tip_center,z))
        dome = dome & box(TOWER_X-d,TOWER_X+d,tip_center-d,tip_center,z-d,z+d)
        s = cy(TOWER_X,z,tip_center,LED_SEAT_Y,d) + dome
        s += cy(TOWER_X,z,LED_SEAT_Y,LED_SEAT_Y+LED_FLANGE_THICKNESS,f)
        parts[f'led_{i}'] = label(s,f'REFERENCE_ONLY_LED_{i}',c)
    return parts


@lru_cache(maxsize=1)
def parts():
    assert PCB_WIDTH+2*PCB_SIDE_CLEARANCE < INNER_RIGHT-INNER_LEFT, 'PCB guides exceed pedestal'
    assert PCB_FRONT_STOP_Y > WALL, 'PCB too long for pedestal'
    assert LED_SEAT_Y>=WALL, 'Requested LED projection exceeds the available barrel length'
    assert LED_COLLAR_DIAMETER>max(LED_FLANGES)
    return {'01_mascot_orange':mascot(),'02_eye_left_black':eye(-12.5),
            '03_eye_right_black':eye(12.5),'04_tower_black':tower(),
            '05_rear_cover_black':rear_cover(),'06_LED_retainer_black':led_retainer()}


def gen_step():
    # Static assembly coordinates are the named desk/front/rear and screw datums above.
    return Compound(children=list(parts().values()),label='Claw_Light_printed_assembly_mm')


def print_parts():
    """Unique, consecutively numbered print files; eye quantity is explicit."""
    p = parts()
    return {'01_mascot_orange':p['01_mascot_orange'],
            '02_eye_black_x2':p['02_eye_left_black'],
            '03_tower_black':p['04_tower_black'],
            '04_rear_cover_black':p['05_rear_cover_black'],
            '05_LED_retainer_black':p['06_LED_retainer_black']}


def print_quantity(name):
    return 2 if name=='02_eye_black_x2' else 1


def print_pose(name, shape):
    if 'mascot' in name:
        s = shape.rotate(Axis.X,-90)  # z'=-Y: rear really is lowest
    elif 'eye' in name or 'tower' in name:
        s = shape.rotate(Axis.X,90)  # visible front DOWN; z'=Y
    elif 'retainer' in name:
        s = shape.rotate(Axis.X,-90)  # flat screw-head face down; short flange pads grow upward
    elif 'rear_cover' in name:
        s = shape.rotate(Axis.X,-90)  # flat exterior down; interior relief and PCB stops build upward
    else:
        s = shape
    lo = s.bounding_box().min
    return s.translate((-lo.X,-lo.Y,-lo.Z))


def coupon_parts():
    """Only the outstanding USB fit test remains in the print set."""
    from usb_coupon import coupon_parts as usb_parts
    return usb_parts()


def export_all():
    (OUT/'stl'/'coupons').mkdir(parents=True,exist_ok=True)
    metadata = {}
    for old in ('02_eye_left_black.stl','03_eye_right_black.stl','06_PCB_tray_black.stl'):
        (OUT/'stl'/old).unlink(missing_ok=True)
    for name,s in print_parts().items():
        p = print_pose(name,s)
        export_stl(p,str(OUT/'stl'/f'{name}.stl'),tolerance=0.02,angular_tolerance=0.05)
        metadata[name] = {'file':f'stl/{name}.stl','quantity':print_quantity(name),'bounds_mm':list(p.bounding_box().size),
                          'solid_volume_mm3':s.volume,'assembly_label':s.label}
    for name,c in coupon_parts().items():
        export_stl(c,str(OUT/'stl'/'coupons'/f'{name}.stl'),tolerance=0.02,angular_tolerance=0.05)
        metadata[name] = {'file':f'stl/coupons/{name}.stl','quantity':1,'bounds_mm':list(c.bounding_box().size),
                          'solid_volume_mm3':c.volume,'assembly_label':c.label}
    export_step(gen_step(),str(OUT/'clawlight.step'))
    with open(OUT/'parts.json','w') as f:
        json.dump(metadata,f,indent=2)
    return metadata


if __name__=='__main__':
    print(json.dumps(export_all(),indent=2))
