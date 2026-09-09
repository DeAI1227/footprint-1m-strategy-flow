"""CLI: python -m orderflow --mode shadow --once

Reads params/*.toml, writes one JSON log line, no secrets.
Live / live_small exit 2 with reason=params_not_calibrated.
Optional --journal PATH steps the Stage 5 decision engine on Rust snapshots.
Optional --reconcile-local / --reconcile-exchange compare fixtures (no API keys).
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .boot import boot_line
from .config import MODES, default_config_dir, load_config
from .decision.engine import DecisionEngine
from .decision.snapshot import load_journal
from .logfmt import json_log
from .reconcile import reconcile


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="orderflow")
    p.add_argument("--mode", default="shadow", choices=list(MODES))
    p.add_argument("--config-dir", default=str(default_config_dir()))
    p.add_argument("--once", action="store_true")
    p.add_argument("--journal", help="Rust closed-1m JSONL (footprint/context/book/resonance)")
    p.add_argument("--symbol", default="SOL", choices=("SOL", "SUI"))
    p.add_argument("--journal-sui", help="Second Rust journal: SUI shadow in parallel with --journal")
    p.add_argument("--reconcile-local", help="Local ledger JSON (Rust snapshot)")
    p.add_argument("--reconcile-exchange", help="Exchange ledger JSON fixture (no API keys)")
    args = p.parse_args(argv)

    line, code = boot_line(args.mode, args.config_dir)
    print(line)
    if code != 0:
        return code

    if args.reconcile_local or args.reconcile_exchange:
        if not (args.reconcile_local and args.reconcile_exchange):
            print('{"level":"error","event":"reconcile_error","error":"need both --reconcile-local and --reconcile-exchange"}')
            return 2
        cfg = load_config(args.config_dir)
        risk = cfg.runtime.get("risk") or {}
        local = json.loads(Path(args.reconcile_local).read_text())
        exchange = json.loads(Path(args.reconcile_exchange).read_text())
        rec = reconcile(
            local,
            exchange,
            symbol_cap_notional=float(risk.get("symbol_cap_notional") or 1000),
            account_cap_notional=float(risk.get("account_cap_notional") or 2000),
        )
        print(json_log("info" if rec["ok"] else "error", rec))
        return 0

    if args.journal and args.journal_sui:
        cfg = load_config(args.config_dir)
        sol_eng = DecisionEngine(params=cfg.sol, mode=args.mode)
        sui_eng = DecisionEngine(params=cfg.sui, mode=args.mode)
        sol_n = sui_n = sol_armed = sui_armed = 0
        for bar in load_journal(args.journal):
            dec = sol_eng.step(bar)
            sol_n += 1
            if dec["main"]:
                sol_armed += 1
        for bar in load_journal(args.journal_sui):
            dec = sui_eng.step(bar)
            sui_n += 1
            if dec["main"]:
                sui_armed += 1
        print(
            json.dumps(
                {
                    "level": "info",
                    "event": "shadow_parallel_done",
                    "sol_bars": sol_n,
                    "sui_bars": sui_n,
                    "sol_armed_bars": sol_armed,
                    "sui_armed_bars": sui_armed,
                    "sol_bucket": cfg.sol["bucket"],
                    "sui_bucket": cfg.sui["bucket"],
                    "copied_price_onto_okx": False,
                    "live": False,
                    "note": "stage 7: SOL and SUI shadow in parallel; separate tables; live still gated",
                },
                ensure_ascii=False,
            )
        )
        return 0

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
            '{"level":"info","event":"idle","note":"stage 7: OKX private decode, SOL+SUI shadow parallel. Resonance off. Live still gated."}'
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
