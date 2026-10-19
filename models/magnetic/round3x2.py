"""Ø3×2 N52 joint, fit revision 5: Ø3.05 × 2.30 mm, flush touching faces."""

from refined import export_all, old

OUT = old.OUT / "round3x2-fit5"


if __name__ == "__main__":
    import argparse
    from pathlib import Path

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=OUT)
    args = parser.parse_args()
    export_all(args.output_dir, "round2")
