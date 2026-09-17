"""Assembled view with illustrative, UNMEASURED electronics; not a print file."""
from build123d import Compound
from clawlight import parts, electronics


def gen_step():
    e=electronics()
    return Compound(children=list(parts().values())+[e[k] for k in ('pcb','usb','led_0','led_1','led_2')],
                    label='Appearance_with_PROVISIONAL_hardware_do_not_print')
