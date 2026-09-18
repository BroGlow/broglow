"""Production-derived USB fit fixture; millimetres, assembly Z is vertical.

Print both crops; the base has integral PCB guides.
Fit the actual PCB and close with two production DIN 7982 screws. The open
top provides access without changing the USB interface or fixed PCB guides.
"""
from build123d import Compound, Solid
from OCP.BRepBuilderAPI import BRepBuilderAPI_Transform
from OCP.TopLoc import TopLoc_Location
import clawlight as m

COUPON_HEIGHT = 14.0
CROP_MARGIN = 1.0
LAYOUT_GAP = 6.0


def coupon_parts():
    lower_screws = [z for _, z in m.COVER_SCREWS if z < COUPON_HEIGHT]
    assert len(lower_screws) == 2
    assert max(lower_screws) + m.COVER_BOSS_DIAMETER / 2 < COUPON_HEIGHT
    assert m.PCB_RAIL_TOP < COUPON_HEIGHT
    production = m.parts()
    result = {}
    for source, name in (('04_tower_black', '01_USB_base_coupon'),
                         ('05_rear_cover_black', '02_USB_cover_coupon')):
        shape = production[source]
        bounds = shape.bounding_box()
        crop = m.box(bounds.min.X-CROP_MARGIN, bounds.max.X+CROP_MARGIN,
                     bounds.min.Y-CROP_MARGIN, bounds.max.Y+CROP_MARGIN,
                     bounds.min.Z-CROP_MARGIN, COUPON_HEIGHT)
        # Keep the cut rim square: every surviving production surface stays intact.
        result[name] = m.label(m.print_pose(source, shape & crop), name)
    return result


def gen_step():
    base, cover = coupon_parts().values()
    cover = cover.translate((base.bounding_box().max.X+LAYOUT_GAP, 0, 0))
    # Bake each print transform into a copy of its geometry. This keeps nested
    # Boolean placements out of the STEP fixture without changing print parts.
    leaves = []
    for shape in (base, cover):
        solid = shape.solids()[0]
        transform = solid.wrapped.Location().Transformation()
        unlocated = solid.wrapped.Located(TopLoc_Location())
        baked = Solid(BRepBuilderAPI_Transform(unlocated, transform, True).Shape())
        leaves.append(m.label(baked, shape.label))
    return Compound(children=leaves,
                    label='USB_fit_fixture_production_base_and_cover_print_layout')


if __name__ == '__main__':
    for name, shape in coupon_parts().items():
        assert shape.is_valid and len(shape.solids()) == 1 and shape.volume > 0, name
        bounds = shape.bounding_box()
        assert abs(bounds.min.Z) < 1e-6, name
        assert abs(bounds.size.X-m.BASE_WIDTH) < 1e-6, name
        assert abs(bounds.size.Y-COUPON_HEIGHT) < 1e-6, name
        print(f'{name}: valid single solid, bed Z=0, size={tuple(bounds.size)} mm')
