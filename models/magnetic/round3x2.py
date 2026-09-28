"""Ø3×2 N52 joint, fit revision 5: Ø3.05 × 2.30 mm, flush touching faces."""

from refined import assembly, export_all, old

OUT = old.OUT / "round3x2-fit5"


def gen_step():
    return assembly("round2")


if __name__ == "__main__":
    import argparse
    from pathlib import Path
    from build123d import export_step

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=OUT)
    args = parser.parse_args()
    export_all(args.output_dir, "round2")
    export_step(gen_step(), args.output_dir / "round3x2.step")
    export_step(
        assembly("round2", explode=10), args.output_dir / "round3x2_exploded.step"
    )
