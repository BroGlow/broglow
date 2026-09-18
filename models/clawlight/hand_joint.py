"""Integral straight-entry hand joints, millimetres.

The ordinary diamond tip enters along +X. A rear haunch widens each section
monotonically toward the root, so the pin prints rear down and still slides
straight into its matching socket. All edge treatments are planar chamfers.
"""
from math import sqrt

from build123d import Compound, Face, GeomType, Wire, extrude

JOINT_X = 32.0
JOINT_CENTERS_YZ = ((7.0, 30.0), (20.0, 30.0))
PIN_DIAGONAL = 3.0
PIN_PROJECTION = 1.8
PIN_TIP_CHAMFER = 0.2
PIN_ROOT_OVERLAP = 0.02
SOCKET_DEPTH = 2.0  # Straight X depth, leaving 0.2 mm tip clearance.
SOCKET_DIAGONAL = 3.3  # User-tested smallest coupon: locating fit; glue for permanent retention.


def socket_cutter(x0, x1, y, z, diagonal=SOCKET_DIAGONAL):
    """Straight-entry haunched cutter; the final diamond is centred at y/z.

    At a section x, the diamond extends toward +Y by x1-x. The filled hull
    between those positions creates a 45-degree rear haunch and nested
    sections, allowing pure +X insertion without a diagonal assembly motion.
    """
    if x1 <= x0 or diagonal <= 0:
        raise ValueError('Socket X depth and diagonal must be positive')
    depth, r = x1-x0, diagonal/2
    face = Face(Wire.make_polygon([(x0,y-r,z), (x0,y,z-r),
                                   (x0,y+r,z), (x0,y,z+r)], close=True))
    straight = extrude(face, amount=depth, dir=(1,0,0))
    sloping = extrude(face.translate((0,depth,0)),
                      amount=depth*sqrt(2), dir=(1,-1,0))
    web = Face(Wire.make_polygon([(x0,y,z-r), (x0,y+depth,z-r),
                                  (x1,y,z-r)], close=True))
    return straight + sloping + extrude(web, amount=diagonal, dir=(0,0,1))


# Compatibility for the main generator: the cutter is haunched, not angled.
diamond_socket = socket_cutter


def _pin(x,y,z,diagonal=PIN_DIAGONAL,projection=PIN_PROJECTION,
         tip_chamfer=PIN_TIP_CHAMFER):
    if not 0 <= tip_chamfer < min(projection/2,diagonal/(2*sqrt(2))):
        raise ValueError('Tip chamfer must retain a positive end and straight pin')
    s = socket_cutter(x-PIN_ROOT_OVERLAP,x+projection,y,z,diagonal)
    if tip_chamfer:
        # Explicit planar cuts avoid OCC's curved corner patches at the two
        # diamond/haunch junctions. Each bevel occupies the last tip_chamfer
        # of X and shrinks the end diamond by that amount normal to its sides.
        end, r = x+projection, diagonal/2
        reach = diagonal+projection+2
        for sy in (-1,1):
            for sz in (-1,1):
                slope = sqrt(2)+(1 if sy > 0 else 0)
                q0 = r+(tip_chamfer if sy > 0 else 0)
                q1 = r-tip_chamfer*sqrt(2)-slope*0.01
                def point(xx,q):
                    return (xx,y+sy*q/2+sz*reach/sqrt(2),
                            z+sz*q/2-sy*reach/sqrt(2))
                face = Face(Wire.make_polygon([
                    point(end-tip_chamfer,q0), point(end+0.01,q1),
                    point(end+0.01,reach), point(end-tip_chamfer,reach)
                ],close=True))
                s -= extrude(face,amount=2*reach,dir=(0,-sz,sy))
    return s


def hand_pins(x=JOINT_X,centers=JOINT_CENTERS_YZ,diagonal=PIN_DIAGONAL,
              projection=PIN_PROJECTION,tip_chamfer=PIN_TIP_CHAMFER):
    """Two assembly-positioned pins to union into the mascot, not print alone."""
    return Compound(children=[_pin(x,y,z,diagonal,projection,tip_chamfer)
                              for y,z in centers],label='integral_straight_hand_pins')


if __name__ == '__main__':
    pins = hand_pins()
    assert pins.is_valid and len(pins.solids()) == 2
    for face in pins.faces():
        assert face.geom_type == GeomType.PLANE
        assert face.normal_at().Y <= 1/sqrt(2)+1e-6
    for y,z in JOINT_CENTERS_YZ:
        pin = _pin(JOINT_X,y,z)
        hole = socket_cutter(JOINT_X,JOINT_X+SOCKET_DEPTH,y,z)
        for t in (0,0.5,1,1.8,2):
            outside = pin.translate((-t,0,0))-hole
            assert outside.bounding_box().max.X <= JOINT_X+1e-6
        expected = SOCKET_DIAGONAL*SOCKET_DEPTH*(SOCKET_DIAGONAL+SOCKET_DEPTH)/2
        assert abs(hole.volume-expected)<1e-6
    print('PASS: straight-X insertion; supported planar pin faces; both 3.3 mm locating sockets clear')
