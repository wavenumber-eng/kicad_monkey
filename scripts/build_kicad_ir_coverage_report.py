"""Build aggregate KiCad Monkey IR coverage reports."""

from __future__ import annotations

import sys
from pathlib import Path

from kicad_monkey.kicad_ir_coverage import main


if __name__ == "__main__":
    arguments = sys.argv[1:]
    if "--generated-root" not in arguments:
        repo_root = Path(__file__).resolve().parents[1]
        arguments.extend(
            [
                "--generated-root",
                str(repo_root / "tests" / "L3_rendering" / "output" / "corpus"),
            ]
        )
    raise SystemExit(main(arguments))
