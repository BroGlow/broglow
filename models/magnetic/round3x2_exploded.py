"""Detached Ø3×2 N52 assembly; silver parts are purchased magnets."""

from refined import assembly


def gen_step():
    return assembly("round2", explode=10)
