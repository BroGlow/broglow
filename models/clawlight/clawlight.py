"""Claw Light: millimetres, X right, Y rear, Z up; desk Z=0.

Run with build123d 0.11.1: python clawlight.py
Screw pilots and LED bores use the user's coupon results. PCB outline and
LED body/flanges and USB shell are measured; component envelopes
remain provisional. Measurements and coupon tests: 2026-09-17.
Print exports are individually oriented and translated onto Z=0.
"""
from pathlib import Path
from functools import lru_cache
from math import ceil, radians, tan
import json

from build123d import (
    Align, Axis, Box, Color, Compound, Cone, Cylinder, Face,
    Sphere, Wire, chamfer, export_step, export_stl, extrude,
)

import hand_joint as joint

OUT = Path(__file__).resolve().parent
WALL = 2.4
BASE_FLOOR_THICKNESS = 2.0
DEPTH = 27.4
MASCOT_SCALE = 0.8
TOWER_X = 41.0
BASE_WIDTH = 28.0
BASE_HEIGHT = 22.4
UPRIGHT_WIDTH = 18.0
TOWER_HEIGHT = 58.0
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
EYE_TONGUE_CORNER_CHAMFER = 0.50  # 0.40 mm after mascot scaling
EYE_TONGUE_ENTRY_CHAMFER = 0.25   # 0.20 mm after mascot scaling
USB_CORNER_CHAMFER = 0.3
SMALL_CHAMFER = 0.3
LED_DIAMETERS = (4.75, 4.75, 4.75)  # measured; green, yellow, red
LED_FLANGES = (5.4, 5.4, 5.4)
LED_FLANGE_THICKNESS = 1.2
LED_BARREL_LENGTH = 8.5  # flange front to dome tip
LED_PITCH = 9.0
LED_Z = tuple(32.0 + i * LED_PITCH for i in range(3))
LED_BORE_DIAMETERS = (5.1, 5.1, 5.1)  # coupon fit, independently of measured LED body
LED_BORE_LEAD_IN = 0.6  # 45-degree face relief: three 0.2 mm layers
LED_RETAINER_GAP = 0.25  # optional thin soft shim removes axial play
LEAD_SLOT = 3.6  # pair of 2.54 mm spaced leads, oriented vertically
PCB_WIDTH = 18.0
PCB_LENGTH = 22.4
PCB_THICKNESS = 0.7
PCB_LENGTH_WITH_USB = 23.9  # measured overall length: USB projects 1.5 mm
PCB_UNDERSIDE = 0.6  # revised allowance within requested 1 mm board-to-floor gap
PCB_TOP_HEIGHT = 4.0
PCB_FRONT = 3.75
PCB_REAR = PCB_FRONT+PCB_LENGTH
PCB_FLOOR_GAP = 1.0
TRAY_FLOOR_Z = BASE_FLOOR_THICKNESS
TRAY_BASE_THICKNESS = 1.6
TRAY_BASE_TOP = TRAY_FLOOR_Z+TRAY_BASE_THICKNESS
PCB_BOTTOM = TRAY_BASE_TOP + PCB_FLOOR_GAP
PCB_SIDE_CLEARANCE = 0.25
PCB_END_CLEARANCE = 0.25
PCB_TOP_CLEARANCE = 0.15
PCB_TOP = PCB_BOTTOM+PCB_THICKNESS
TRAY_SIDE_WIDTH = 1.6
TRAY_FRONT_EXTENSION = 1.05
PCB_FRONT_STOP_EXTENSION = 1.05
TRAY_LEFT, TRAY_RIGHT = TOWER_X-PCB_WIDTH/2-TRAY_SIDE_WIDTH, TOWER_X+PCB_WIDTH/2+TRAY_SIDE_WIDTH
TRAY_FRONT, TRAY_BACK = PCB_FRONT-TRAY_FRONT_EXTENSION, PCB_REAR
TRAY_RAIL_BACK = REAR  # No rail fingers project past the cover mating face.
TRAY_KEY_EXTENSION = 0.8
TRAY_RAIL_VERTICAL_CLEARANCE = 0.45
TRAY_RAIL_SIDE_CLEARANCE = 0.35
TRAY_FRONT_CLEARANCE = 0.0  # Assembly datum is the fully seated front stop.
CLIP_BEAM_THICKNESS = 0.8
CLIP_BEAM_LENGTH = 12.0
CLIP_HOOK_OVERLAP = 0.8
CLIP_HOOK_REAR_INSET = 2.2  # Rear tray keys engage the base before its rear seam.
CLIP_HOOK_LENGTH = 0.55  # Clear components through the full 0.25 mm PCB endplay.
CLIP_HOOK_Y1 = PCB_REAR-CLIP_HOOK_REAR_INSET
CLIP_HOOK_Y0 = CLIP_HOOK_Y1-CLIP_HOOK_LENGTH
CLIP_BEAM_Y0, CLIP_BEAM_Y1 = CLIP_HOOK_Y1-CLIP_BEAM_LENGTH, CLIP_HOOK_Y1
CLIP_ISOLATION_GAP = 0.6
CLIP_KEEPER_CLEARANCE = 0.2
CLIP_FREE_TRAVEL = 1.2  # free PETG clip travel target; confirm with the actual printed tray
CLIP_INNER_X = TRAY_LEFT+CLIP_BEAM_THICKNESS
CLIP_TIP_X = TOWER_X-PCB_WIDTH/2+CLIP_HOOK_OVERLAP
# Both sides of the triangular hook rise at 45 degrees. Its underside clears
# the actual PCB edge, not merely its innermost tip.
CLIP_UNDERSIDE_Z = PCB_TOP+PCB_TOP_CLEARANCE-(TOWER_X-PCB_WIDTH/2-CLIP_INNER_X)
CLIP_TIP_Z = PCB_TOP+PCB_TOP_CLEARANCE+CLIP_HOOK_OVERLAP
CLIP_TOP_Z = CLIP_TIP_Z+(CLIP_TIP_X-CLIP_INNER_X)
CLIP_KEEPER_TOP_Z = CLIP_TOP_Z+0.3
FRONT_LIP_OVERLAP_X = 0.4
FRONT_LIP_OVERLAP_Y = 0.3
FRONT_LIP_THICKNESS = 0.8
TRAY_BED_CHAMFER = 0.10  # Preserve the shortened rear-key corners.
TRAY_GUIDE_OUTER_CHAMFER = 0.30
TRAY_GUIDE_INNER_CHAMFER = 0.20
TRAY_GUIDE_CORNER_CHAMFER = 0.30
TRAY_FRONT_LIP_TOP_CHAMFER = 0.12
TRAY_STOP_OUTER_CHAMFER = 0.20
TRAY_FLOOR_RIM_CHAMFER = 0.15
TRAY_KEY_CHAMFER = 0.10  # Retain the short rear keys' bearing land inside the base.
TRAY_SPRING_ROOT_PROTECTION = 1.0
USB_WIDTH, USB_HEIGHT = 8.9, 3.1
USB_SEATING_Z_OFFSET = 1.2  # User-measured upward correction, 2026-09-18.
USB_CENTER_ABOVE_PCB = USB_HEIGHT/2 + USB_SEATING_Z_OFFSET
USB_CENTER_Z = PCB_BOTTOM + PCB_THICKNESS + USB_CENTER_ABOVE_PCB
USB_REAR = PCB_REAR + PCB_LENGTH_WITH_USB - PCB_LENGTH
USB_CLEARANCE = 0.2  # each side of the throat; entry chamfer adds lead-in
USB_OPENING_WIDTH = USB_WIDTH+2*USB_CLEARANCE
USB_OPENING_HEIGHT = USB_HEIGHT+2*USB_CLEARANCE
USB_ENTRY_CHAMFER = 0.2
USB_CABLE_BODY_START = USB_REAR+0.1
COVER_INNER_SKIN = 1.0
COVER_POCKET_BACK = DEPTH-COVER_INNER_SKIN
COVER_POCKET_CLEARANCE = 0.25
COVER_POCKET_LEFT = INNER_LEFT-COVER_POCKET_CLEARANCE
COVER_POCKET_RIGHT = INNER_RIGHT+COVER_POCKET_CLEARANCE
COVER_POCKET_BOTTOM = BASE_FLOOR_THICKNESS-COVER_POCKET_CLEARANCE
COVER_POCKET_TOP = max(PCB_TOP+PCB_TOP_HEIGHT,CLIP_TOP_Z,
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
LED_BOSS_DIAMETER = 4.8
COVER_SCREWS = ((TOWER_X-10.4,16.0),(TOWER_X+10.4,16.0),(TOWER_X+5.5,54.7))
LED_SCREWS = ((TOWER_X-3.2,26.0),(TOWER_X-3.2,45.5))
RETAINER_Y = WALL + LED_FLANGE_THICKNESS + LED_RETAINER_GAP  # flange pad face
RETAINER_THICKNESS = 3.0  # structural plate; forward flange pads are additional
COVER_PILOT_DEPTH = SCREW_LENGTH+SCREW_LENGTH_TOLERANCE+MAX_HEAD_SEATING_DEPTH-COVER+SCREW_TIP_CLEARANCE
RETAINER_PILOT_DEPTH = SCREW_LENGTH+SCREW_LENGTH_TOLERANCE+MAX_HEAD_SEATING_DEPTH-RETAINER_THICKNESS+SCREW_TIP_CLEARANCE
RETAINER_MOUNT_Y = max(RETAINER_Y,ceil((RETAINER_PILOT_DEPTH+SCREW_TIP_WALL)*10)/10)
RETAINER_BACK_Y = RETAINER_MOUNT_Y + RETAINER_THICKNESS
RETAINER_LEFT, RETAINER_RIGHT = TOWER_X-6.0, TOWER_X+3.2
RETAINER_BOTTOM, RETAINER_TOP = 22.8, 52.8
RETAINER_PROFILE_CHAMFER = 0.30
RETAINER_BED_CHAMFER = 0.15
RETAINER_EDGE_CHAMFER = 0.20
RETAINER_SLOT_CHAMFER = 0.15
RETAINER_RELIEF_CHAMFER = 0.15
LEAD_SLOT_X = TOWER_X-2.0

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

    rear = DEPTH/MASCOT_SCALE
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
        s += cy(x,z,0,REAR,COVER_BOSS_DIAMETER)
        s -= cy(x,z,REAR-COVER_PILOT_DEPTH,REAR+BOOLEAN_OVERLAP,SCREW_PILOT_DIAMETER)
    for x,z in LED_SCREWS:
        s += cy(x,z,0,RETAINER_MOUNT_Y,LED_BOSS_DIAMETER)
        s -= cy(x,z,RETAINER_MOUNT_Y-RETAINER_PILOT_DEPTH,
                RETAINER_MOUNT_Y+BOOLEAN_OVERLAP,SCREW_PILOT_DIAMETER)
    for z,d in zip(LED_Z,LED_BORE_DIAMETERS):
        s -= cy(TOWER_X,z,-0.1,WALL+0.1,d)
        # Small front bezel; the rear bore and flange-bearing plane are untouched.
        r=d/2
        s -= Cone(r+LED_BORE_LEAD_IN,r,LED_BORE_LEAD_IN,align=(Align.CENTER,Align.CENTER,Align.MIN)).rotate(Axis.X,-90).translate((TOWER_X,0,z))
    s = s & outer
    # Continuous front-supported rails capture the static tray keys. Above the
    # groove, their side faces also stop the seated PCB clips opening outward.
    # The tray rests directly on one flat floor; no raised side shelves.
    for x0,x1 in ((INNER_LEFT-BOOLEAN_OVERLAP,TRAY_LEFT-TRAY_RAIL_SIDE_CLEARANCE),
                  (TRAY_RIGHT+TRAY_RAIL_SIDE_CLEARANCE,INNER_RIGHT+BOOLEAN_OVERLAP)):
        s += box(x0,x1,WALL-BOOLEAN_OVERLAP,TRAY_RAIL_BACK,
                 TRAY_BASE_TOP+TRAY_RAIL_VERTICAL_CLEARANCE,CLIP_KEEPER_TOP_Z)
    for x0,x1 in ((INNER_LEFT-BOOLEAN_OVERLAP,TRAY_LEFT-CLIP_KEEPER_CLEARANCE),
                  (TRAY_RIGHT+CLIP_KEEPER_CLEARANCE,INNER_RIGHT+BOOLEAN_OVERLAP)):
        s += box(x0,x1,CLIP_HOOK_Y0,CLIP_HOOK_Y1,
                 TRAY_BASE_TOP+TRAY_RAIL_VERTICAL_CLEARANCE,CLIP_KEEPER_TOP_Z)
    # Front stop carries USB insertion force; cover takes cable extraction force.
    s += box(INNER_LEFT,INNER_RIGHT,WALL-BOOLEAN_OVERLAP,TRAY_FRONT-TRAY_FRONT_CLEARANCE,
             TRAY_FLOOR_Z,TRAY_BASE_TOP+0.4)
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
    # Cover-mounted PCB end stops allow a short tray and a flush USB port.
    # The surrounding inner skin also stops the tray at its own rear datum.
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
    return label(s,'black_flat_rear_cover_flush_USB_and_PCB_end_stops')


def led_retainer():
    s = box(RETAINER_LEFT,RETAINER_RIGHT,RETAINER_MOUNT_Y,RETAINER_BACK_Y,
            RETAINER_BOTTOM,RETAINER_TOP)
    s = chamfer(s.edges().filter_by(Axis.Y),length=RETAINER_PROFILE_CHAMFER)
    s = chamfer(y_face_edges(s,RETAINER_BACK_Y,outer_only=True),length=RETAINER_BED_CHAMFER)
    s = chamfer(y_face_edges(s,RETAINER_MOUNT_Y,outer_only=True),length=RETAINER_EDGE_CHAMFER)
    # Raised flange pads preserve the original 0.25 mm LED play while longer
    # screw posts move the screw tips away from the front skin. Rear face prints down.
    for z,d in zip(LED_Z,LED_FLANGES):
        pad = box(RETAINER_LEFT,RETAINER_RIGHT,RETAINER_Y,
                  RETAINER_MOUNT_Y+RETAINER_EDGE_CHAMFER+BOOLEAN_OVERLAP,z-d/2,z+d/2)
        pad = chamfer(pad.edges().filter_by(Axis.Y),length=RETAINER_EDGE_CHAMFER)
        pad = chamfer(y_face_edges(pad,RETAINER_Y),length=RETAINER_EDGE_CHAMFER)
        s += pad
    for z in LED_Z:
        slot = box(LEAD_SLOT_X,RETAINER_RIGHT+1,RETAINER_Y-BOOLEAN_OVERLAP,
                   RETAINER_BACK_Y+BOOLEAN_OVERLAP,z-LEAD_SLOT/2,z+LEAD_SLOT/2)
        s -= chamfer(slot.edges().filter_by(Axis.Y),length=RETAINER_PROFILE_CHAMFER)
    # Bevel the fresh slot rims after cutting them, retaining flat flange
    # bearing bands and the full 3.6 mm lead passage between the entrances.
    for y in (RETAINER_Y,RETAINER_BACK_Y):
        rims = [e for e in y_face_edges(s,y)
                if any(abs(e.center().Z-z)<=LEAD_SLOT/2+1e-6 for z in LED_Z)
                and e.center().X>=LEAD_SLOT_X-1e-6
                and e.bounding_box().max.X<RETAINER_RIGHT-1e-6]
        s = chamfer(rims,length=RETAINER_SLOT_CHAMFER)
    # Explicit side-mouth planes avoid tiny edge fragments at pad/plate joins.
    for z in LED_Z:
        for sign in (-1,1):
            edge_z = z+sign*LEAD_SLOT/2
            s -= profile([(RETAINER_RIGHT-RETAINER_SLOT_CHAMFER,edge_z),
                          (RETAINER_RIGHT+BOOLEAN_OVERLAP,edge_z),
                          (RETAINER_RIGHT+BOOLEAN_OVERLAP,
                           edge_z+sign*(RETAINER_SLOT_CHAMFER+BOOLEAN_OVERLAP))],
                         RETAINER_Y-BOOLEAN_OVERLAP,RETAINER_BACK_Y+BOOLEAN_OVERLAP)
    for x,z in LED_SCREWS:
        s -= cy(x,z,RETAINER_Y-BOOLEAN_OVERLAP,RETAINER_MOUNT_Y,
                LED_BOSS_DIAMETER+2*FIT)
        relief_radius = LED_BOSS_DIAMETER/2+FIT
        mouth = Cone(relief_radius+RETAINER_RELIEF_CHAMFER,relief_radius,
                     RETAINER_RELIEF_CHAMFER,align=(Align.CENTER,Align.CENTER,Align.MIN))
        s -= mouth.rotate(Axis.X,-90).translate((x,RETAINER_Y,z))
        s -= countersunk_hole_y(x,z,RETAINER_MOUNT_Y,RETAINER_BACK_Y)
    return label(s,'black_LED_retainer_countersunk_screws_forward_flange_pads')


def pcb_tray():
    left, right = TOWER_X-PCB_WIDTH/2, TOWER_X+PCB_WIDTH/2
    assert TRAY_LEFT-TRAY_KEY_EXTENSION>INNER_LEFT and TRAY_RIGHT+TRAY_KEY_EXTENSION<INNER_RIGHT
    assert CLIP_HOOK_OVERLAP>2*CLIP_KEEPER_CLEARANCE+PCB_SIDE_CLEARANCE
    assert CLIP_BEAM_Y0>PCB_FRONT+1.5 and CLIP_BEAM_Y1+CLIP_ISOLATION_GAP<TRAY_BACK
    assert FRONT_LIP_OVERLAP_X<0.5, 'Keep front lips outside the provisional top components'
    s = box(TRAY_LEFT,TRAY_RIGHT,TRAY_FRONT,TRAY_BACK,TRAY_FLOOR_Z,TRAY_BASE_TOP)
    # Static side/end guides take USB forces through the PCB substrate.
    for x0,x1,outer_x,inner_x in (
        (TRAY_LEFT,left-PCB_SIDE_CLEARANCE,TRAY_LEFT,left-PCB_SIDE_CLEARANCE),
        (right+PCB_SIDE_CLEARANCE,TRAY_RIGHT,TRAY_RIGHT,right+PCB_SIDE_CLEARANCE),
    ):
        guide=box(x0,x1,TRAY_FRONT,TRAY_BACK,TRAY_BASE_TOP,PCB_TOP+0.2)
        guide=chamfer(guide.edges().filter_by(Axis.Z),length=TRAY_GUIDE_CORNER_CHAMFER)
        for x,amount in ((outer_x,TRAY_GUIDE_OUTER_CHAMFER),(inner_x,TRAY_GUIDE_INNER_CHAMFER)):
            top_edges=[e for e in guide.edges()
                       if abs(e.bounding_box().min.Z-(PCB_TOP+0.2))<1e-6
                       and abs(e.bounding_box().max.Z-(PCB_TOP+0.2))<1e-6
                       and abs(e.bounding_box().min.X-x)<1e-6
                       and abs(e.bounding_box().max.X-x)<1e-6]
            guide=chamfer(top_edges,length=amount)
        # Restore the full section around the spring root and release slot.
        # Cosmetics must not thin the 0.8 mm bending strip or its anchor.
        guide+=box(x0,x1,CLIP_BEAM_Y0-TRAY_SPRING_ROOT_PROTECTION,
                   CLIP_BEAM_Y1+CLIP_ISOLATION_GAP,TRAY_BASE_TOP,PCB_TOP+0.2)
        s+=guide
    # Four small bare-edge bearing pads; no dead tail beyond the PCB rear.
    for x0,x1 in ((left-PCB_SIDE_CLEARANCE,left+1.5),(right-1.5,right+PCB_SIDE_CLEARANCE)):
        for y0,y1 in ((PCB_FRONT-PCB_END_CLEARANCE,PCB_FRONT+1.5),
                      (PCB_REAR-1.5,TRAY_BACK)):
            s += box(x0,x1,y0,y1,TRAY_BASE_TOP-BOOLEAN_OVERLAP,PCB_BOTTOM)
    front_stop=box(left-PCB_SIDE_CLEARANCE,right+PCB_SIDE_CLEARANCE,
                   PCB_FRONT-PCB_FRONT_STOP_EXTENSION,PCB_FRONT-PCB_END_CLEARANCE,TRAY_BASE_TOP-BOOLEAN_OVERLAP,PCB_TOP)
    away=[e for e in front_stop.edges() if abs(e.bounding_box().min.Z-PCB_TOP)<1e-6
          and abs(e.bounding_box().max.Z-PCB_TOP)<1e-6
          and abs(e.bounding_box().min.Y-(PCB_FRONT-PCB_FRONT_STOP_EXTENSION))<1e-6
          and abs(e.bounding_box().max.Y-(PCB_FRONT-PCB_FRONT_STOP_EXTENSION))<1e-6]
    s+=chamfer(away,length=TRAY_STOP_OUTER_CHAMFER)
    # The spring strips start on the bed. Full-depth slots free their complete
    # section, rather than leaving a rigid floor or an unsupported horizontal beam.
    for x0,x1 in ((TRAY_LEFT,CLIP_INNER_X),(2*TOWER_X-CLIP_INNER_X,TRAY_RIGHT)):
        s += box(x0,x1,CLIP_BEAM_Y0,CLIP_BEAM_Y1,TRAY_FLOOR_Z,CLIP_TOP_Z)
    for x0,x1 in ((CLIP_INNER_X,CLIP_INNER_X+CLIP_ISOLATION_GAP),
                  (2*TOWER_X-CLIP_INNER_X-CLIP_ISOLATION_GAP,2*TOWER_X-CLIP_INNER_X)):
        slot=box(x0,x1,CLIP_BEAM_Y0,CLIP_BEAM_Y1+CLIP_ISOLATION_GAP,
                 TRAY_FLOOR_Z-BOOLEAN_OVERLAP,CLIP_TOP_Z+BOOLEAN_OVERLAP)
        slot+=Cylinder(CLIP_ISOLATION_GAP/2,CLIP_TOP_Z-TRAY_FLOOR_Z+2*BOOLEAN_OVERLAP,
                       align=(Align.CENTER,Align.CENTER,Align.MIN)).translate(
                           ((x0+x1)/2,CLIP_BEAM_Y0,TRAY_FLOOR_Z-BOOLEAN_OVERLAP))
        s-=slot
    for x0,x1 in ((TRAY_LEFT-BOOLEAN_OVERLAP,CLIP_INNER_X+CLIP_ISOLATION_GAP),
                  (2*TOWER_X-CLIP_INNER_X-CLIP_ISOLATION_GAP,TRAY_RIGHT+BOOLEAN_OVERLAP)):
        s-=box(x0,x1,CLIP_BEAM_Y1,CLIP_BEAM_Y1+CLIP_ISOLATION_GAP,
               TRAY_FLOOR_Z-BOOLEAN_OVERLAP,CLIP_TOP_Z+BOOLEAN_OVERLAP)
    hook=[(CLIP_INNER_X,CLIP_UNDERSIDE_Z),(CLIP_TIP_X,CLIP_TIP_Z),(CLIP_INNER_X,CLIP_TOP_Z)]
    s+=profile(hook,CLIP_HOOK_Y0,CLIP_HOOK_Y1)
    s+=profile([(2*TOWER_X-x,z) for x,z in hook],CLIP_HOOK_Y0,CLIP_HOOK_Y1)
    # Short fixed front lips accept the board at an angle. Their 45-degree
    # undersides grow inward from the side guides, so USB forces reach the
    # front substrate stop rather than wedging the PCB on a front-facing ramp.
    lip_root=left-PCB_SIDE_CLEARANCE-BOOLEAN_OVERLAP
    lip_tip=left+FRONT_LIP_OVERLAP_X
    lip_bottom=PCB_TOP+PCB_TOP_CLEARANCE+FRONT_LIP_OVERLAP_X
    lip=[(lip_root,PCB_TOP+PCB_TOP_CLEARANCE+lip_root-left),
         (lip_tip,lip_bottom),(lip_tip,lip_bottom+FRONT_LIP_THICKNESS),
         (lip_root,lip_bottom+FRONT_LIP_THICKNESS)]
    for points in (lip,[(2*TOWER_X-x,z) for x,z in lip]):
        front_lip=profile(points,PCB_FRONT-PCB_END_CLEARANCE,PCB_FRONT+FRONT_LIP_OVERLAP_Y)
        s+=chamfer(front_lip.faces().sort_by(Axis.Z)[-1].edges(),length=TRAY_FRONT_LIP_TOP_CHAMFER)
    # Only the rigid ends carry groove keys; no key stiffens the spring span.
    for x0,x1 in ((TRAY_LEFT-TRAY_KEY_EXTENSION,TRAY_LEFT+BOOLEAN_OVERLAP),
                  (TRAY_RIGHT-BOOLEAN_OVERLAP,TRAY_RIGHT+TRAY_KEY_EXTENSION)):
        for y0,y1 in ((TRAY_FRONT,CLIP_BEAM_Y0-CLIP_ISOLATION_GAP),
                      (CLIP_BEAM_Y1+CLIP_ISOLATION_GAP,TRAY_BACK)):
            key=box(x0,x1,y0,y1,TRAY_FLOOR_Z,TRAY_BASE_TOP)
            s+=chamfer(key.edges().filter_by(Axis.Z),length=TRAY_KEY_CHAMFER)
    # Exposed floor rims only: preserve key tops that bear on the shell grooves.
    rims=[e for e in s.edges() if abs(e.bounding_box().min.Z-TRAY_BASE_TOP)<1e-6
          and abs(e.bounding_box().max.Z-TRAY_BASE_TOP)<1e-6
          and e.bounding_box().min.X>=CLIP_INNER_X-1e-6
          and e.bounding_box().max.X<=2*TOWER_X-CLIP_INNER_X+1e-6
          and any(abs(e.bounding_box().min.Y-y)<1e-6
                  and abs(e.bounding_box().max.Y-y)<1e-6 for y in (TRAY_FRONT,TRAY_BACK))]
    s=chamfer(rims,length=TRAY_FLOOR_RIM_CHAMFER)
    s=chamfer(s.faces().sort_by(Axis.Z)[0].edges(),length=TRAY_BED_CHAMFER)
    return label(s,'black_sliding_PCB_tray_PETG_snap_clips')


def electronics():
    left, right = TOWER_X-PCB_WIDTH/2, TOWER_X+PCB_WIDTH/2
    front = PCB_REAR-PCB_LENGTH
    pcb = label(box(left,right,front,PCB_REAR,PCB_BOTTOM,PCB_BOTTOM+PCB_THICKNESS),
                'REFERENCE_ONLY_provisional_PCB',(0.05,0.35,0.22))
    # Central component envelopes exclude intentionally supported bare PCB edges.
    lower = box(left+1.6,right-1.6,front+1.6,PCB_REAR-1.6,PCB_BOTTOM-PCB_UNDERSIDE,PCB_BOTTOM)
    upper = box(left+0.5,right-0.5,front+0.5,PCB_REAR-3,PCB_BOTTOM+PCB_THICKNESS,
                PCB_BOTTOM+PCB_THICKNESS+PCB_TOP_HEIGHT)
    usb = label(box(TOWER_X-USB_WIDTH/2,TOWER_X+USB_WIDTH/2,PCB_REAR-3,USB_REAR,
                   USB_CENTER_Z-USB_HEIGHT/2,USB_CENTER_Z+USB_HEIGHT/2),
                'REFERENCE_ONLY_USB_shell',(0.6,0.63,0.65))
    parts = {'pcb':pcb,'underside':label(lower,'REFERENCE_ONLY_underside_keepout'),
             'top':label(upper,'REFERENCE_ONLY_top_keepout'),'usb':usb}
    for i,(z,d,f,c) in enumerate(zip(LED_Z,LED_DIAMETERS,LED_FLANGES,
                                    ((0.04,0.75,0.1),(1,0.7,0.01),(0.8,0.015,0.015)))):
        tip_center = WALL-LED_BARREL_LENGTH+d/2
        dome = Sphere(d/2).translate((TOWER_X,tip_center,z))
        dome = dome & box(TOWER_X-d,TOWER_X+d,tip_center-d,tip_center,z-d,z+d)
        s = cy(TOWER_X,z,tip_center,WALL,d) + dome
        s += cy(TOWER_X,z,WALL,WALL+LED_FLANGE_THICKNESS,f)
        parts[f'led_{i}'] = label(s,f'REFERENCE_ONLY_LED_{i}',c)
    return parts


@lru_cache(maxsize=1)
def parts():
    assert PCB_WIDTH+2*TRAY_SIDE_WIDTH+2*TRAY_KEY_EXTENSION < INNER_RIGHT-INNER_LEFT, 'PCB tray exceeds pedestal; enlarge pedestal first'
    assert PCB_REAR-PCB_LENGTH-TRAY_FRONT_EXTENSION-TRAY_FRONT_CLEARANCE > WALL, 'PCB too long for pedestal'
    assert max(LED_FLANGES)<8.5, 'Widen/reposition retainer and screw bosses for larger LEDs'
    return {'01_mascot_orange':mascot(),'02_eye_left_black':eye(-12.5),
            '03_eye_right_black':eye(12.5),'04_tower_black':tower(),
            '05_rear_cover_black':rear_cover(),'06_LED_retainer_black':led_retainer(),
            '07_PCB_tray_black':pcb_tray()}


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
            '05_LED_retainer_black':p['06_LED_retainer_black'],
            '06_PCB_tray_black':p['07_PCB_tray_black']}


def print_quantity(name):
    return 2 if name=='02_eye_black_x2' else 1


def print_pose(name, shape):
    if 'mascot' in name:
        s = shape.rotate(Axis.X,-90)  # z'=-Y: rear really is lowest
    elif 'eye' in name or 'tower' in name:
        s = shape.rotate(Axis.X,90)  # visible front DOWN; z'=Y
    elif 'retainer' in name:
        s = shape.rotate(Axis.X,-90)  # countersunk rear DOWN; flange pads build upward
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
    for old in ('02_eye_left_black.stl','03_eye_right_black.stl'):
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
