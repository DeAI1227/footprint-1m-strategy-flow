"""CLI: python -m orderflow --mode shadow --once

Reads params/*.toml, writes one JSON log line, no secrets.
Live / live_small exit 2 with reason=params_not_calibrated.
Optional --journal PATH steps the Stage 5 decision engine on Rust snapshots.
"""

from __future__ import annotations

import argparse
import json
import sys

from .boot import boot_line
from .config import MODES, default_config_dir, load_config
from .decision.engine import DecisionEngine
from .decision.snapshot import load_journal
from .logfmt import json_log


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="orderflow")
    p.add_argument("--mode", default="shadow", choices=list(MODES))
    p.add_argument("--config-dir", default=str(default_config_dir()))
    p.add_argument("--once", action="store_true")
    p.add_argument("--journal", help="Rust closed-1m JSONL (footprint/context/book/resonance)")
    p.add_argument("--symbol", default="SOL", choices=("SOL", "SUI"))
    args = p.parse_args(argv)

    line, code = boot_line(args.mode, args.config_dir)
    print(line)
    if code != 0:
        return code

    if args.journal:
        cfg = load_config(args.config_dir)
        params = cfg.sol if args.symbol == "SOL" else cfg.sui
        eng = DecisionEngine(params=params, mode=args.mode)
        n = 0
        armed = 0
        for bar in load_journal(args.journal):
            dec = eng.step(bar)
            n += 1
            if dec["main"]:
                armed += 1
            print(json_log("info", dec))
        print(
            json.dumps(
                {
                    "level": "info",
                    "event": "decision_done",
                    "bars": n,
                    "armed_bars": armed,
                    "copied_price_onto_okx": False,
                    "live": False,
                    "note": "stage 5: A–G on Rust snapshots; shadow only; live still gated",
                },
                ensure_ascii=False,
            )
        )
        return 0

    if not args.once:
        print(
            '{"level":"info","event":"idle","note":"stage 5: A–G decision on --journal Rust snapshots. Resonance off. Live still gated."}'
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
