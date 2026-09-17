"""Coupon print layout; all three pieces must be printed and assembled for screw tests."""
from build123d import Compound
from clawlight import coupon_parts, COUPON_LENGTH, COUPON_CAP_LENGTH


def gen_step():
    p=coupon_parts()
    return Compound(children=[p['00_fit_coupon'],
        p['00b_coupon_cover_cap'].translate((0,COUPON_LENGTH+6,0)),
        p['00c_coupon_retainer_cap'].translate((0,COUPON_LENGTH+COUPON_CAP_LENGTH+12,0))],
        label='DIN7982_2p2x6p5_and_LED_fit_coupon_print_layout')
