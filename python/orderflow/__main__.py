"""CLI: python -m orderflow --mode shadow --once

Reads params/*.toml, writes one JSON log line, no secrets.
Live / live_small exit 2 with reason=params_not_calibrated.
"""

from __future__ import annotations

import argparse
import sys

from .boot import boot_line
from .config import MODES, default_config_dir


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="orderflow")
    p.add_argument("--mode", default="shadow", choices=list(MODES))
    p.add_argument("--config-dir", default=str(default_config_dir()))
    p.add_argument("--once", action="store_true")
    args = p.parse_args(argv)

    line, code = boot_line(args.mode, args.config_dir)
    print(line)
    if code != 0:
        return code
    if not args.once:
        print(
            '{"level":"info","event":"idle","note":"stage 0: no WS, no footprint, no orders"}'
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
