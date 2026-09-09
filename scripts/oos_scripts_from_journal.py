#!/usr/bin/env python3
"""OOS A–G failure screens from a Rust footprint_closed journal.

Reads frozen snapshots only. 300% (dale) and 400% (valtos) in parallel.
Does not select a rate. Does not authorize live. F stays not_evaluated without L2.
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))

from orderflow.calibrate.screens import run_screens  # noqa: E402
from orderflow.decision.snapshot import load_journal  # noqa: E402


def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float]:
    if n <= 0:
        return (0.0, 0.0)
    p = k / n
    z2 = z * z
    den = 1 + z2 / n
    centre = (p + z2 / (2 * n)) / den
    half = z * math.sqrt((p * (1 - p) + z2 / (4 * n)) / n) / den
    return (max(0.0, centre - half), min(1.0, centre + half))


def pct(k: int, n: int) -> str:
    if n <= 0:
        return "—"
    lo, hi = wilson(k, n)
    return f"{100.0 * k / n:.0f}% ({100.0 * lo:.0f}–{100.0 * hi:.0f})"


def print_a(label: str, row: dict) -> None:
    left = int(row["left"])
    print(f"## {label}")
    print(f"- armed aligned 3-stack: {row['armed']}")
    print(f"- never left in 12m: {row['no_leave']} (chase, not A)")
    print(f"- left then eligible A: {left}")
    if left:
        print(f"- pullback reject/excess: {row['reject']} ({pct(row['reject'], left)} of left)")
        print(f"- pullback punched: {row['punch']} ({pct(row['punch'], left)})")
        print(f"- touched, no clear reject: {row['inside']}")
        print(f"- no return in 20m: {row['no_return']}")
    print()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("journal")
    ap.add_argument("--tick", type=float, default=0.01)
    args = ap.parse_args()
    bars = load_journal(args.journal)
    out = run_screens(bars, tick=args.tick)
    print("# OOS A–G from frozen journal (not a second matrix)")
    print(f"# journal={args.journal} bars={out['bars']}")
    print("# chosen_armed_rate=None still_open=true out_of_sample_validated=false live=false")
    print("# F=not_evaluated G_is_entry=false E_reverse=false")
    print()
    for rate, title in (("dale", "300% Dale"), ("valtos", "400% Valtos")):
        print(f"===== {title} =====")
        block = out[rate]
        print_a(f"all A leave=1", block["A"]["all"])
        for sess in ("asia", "eu", "us", "thin"):
            row = block["A"]["sessions"].get(sess)
            if row and row["armed"]:
                print_a(f"{sess} A leave=1", row)
        b = block["B"]["all"]
        atk = b["attacks"]
        print(
            f"B all: attacks={atk} held={b['held']} second_punch={b['second_punch']} "
            f"({pct(b['second_punch'], atk)}) vacuum={b['vacuum']}"
        )
        d = block["D"]["all"]
        acc = d["accepted"]
        print(
            f"D all accept=3: accepted={acc} reject={d['reject']} punch={d['punch']} "
            f"({pct(d['punch'], acc)}) no_return={d['no_return']}"
        )
        g = block["G"]["all"]
        key = g["key_unf"]
        print(
            f"G all: key_unf={key} fill={g['fill']} ({pct(g['fill'], key)}) "
            f"extend={g['extend']} ({pct(g['extend'], key)}) neither={g['neither']} cheap={g['cheap']}"
        )
        print()
    c = out["C"]["all"]
    br = c["breaks"]
    print("===== C TRAP_BARS=3 (no imbalance ratio) =====")
    print(f"breaks={br} reclaim={c['reclaim']} ({pct(c['reclaim'], br)}) accepted_outside={c['accepted']}")
    e = out["E"]["all"]
    print("===== E CVD session-anchored =====")
    print(
        f"segments={e['segments']} first_up={e['first_up']} later_up={e['later_up']} "
        f"first_dn={e['first_dn']} later_dn={e['later_dn']}"
    )
    print("===== F =====")
    print(f"not_evaluated={out['F']['all']['not_evaluated']} reason=no_l2")
    print()
    print(json.dumps(out, ensure_ascii=False, indent=2, default=str))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
