#!/usr/bin/env python3
"""Session-split shadow tables from a Rust footprint_closed journal.

Reads frozen snapshots only. Does not rebuild a bid×ask matrix from trades.
Does not select 300 vs 400. Does not authorize live.

Aligned flags already include the engine's rolling session p25 + stack 3 + bar
direction. nonempty_side_p25 here is a batch snapshot of this journal's frozen
cells (same rule as observation notes; not a frozen SOL lot).
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))

from orderflow.calibrate import journal_stats  # noqa: E402
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


def pct_ci(k: int, n: int) -> str:
    if n <= 0:
        return "—"
    lo, hi = wilson(k, n)
    return f"{100.0 * k / n:.1f}% ({100.0 * lo:.1f}–{100.0 * hi:.1f})"


def print_row(name: str, row: dict) -> None:
    n = int(row["bars"])
    p25 = row.get("nonempty_side_p25")
    p25s = "n/a" if p25 is None else f"{p25:.2f}"
    print(
        f"{name:12} n={n:5d} p25={p25s:>8}  "
        f"200% 3s={pct_ci(int(row['record_stack3']), n):>22}  "
        f"300% 3s={pct_ci(int(row['dale_stack3']), n):>22}  "
        f"400% 3s={pct_ci(int(row['valtos_stack3']), n):>22}"
    )
    print(
        f"{'':12}         aligned  "
        f"200%={pct_ci(int(row['record_aligned']), n):>22}  "
        f"300%={pct_ci(int(row['dale_aligned']), n):>22}  "
        f"400%={pct_ci(int(row['valtos_aligned']), n):>22}  "
        f"unfin={pct_ci(int(row['unfinished']), n)} chaos={int(row['chaos'])}"
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("journal", help="Rust JSONL with footprint_closed events")
    args = ap.parse_args()
    bars = load_journal(args.journal)
    stats = journal_stats(bars)
    print("# OOS / shadow from frozen journal (not a second matrix)")
    print(f"# journal={args.journal}")
    print("# chosen_armed_rate=None still_open=true out_of_sample_validated=false live=false")
    print()
    overall = {
        "bars": stats["bars"],
        "record_stack3": stats["record_stack3"],
        "dale_stack3": stats["dale_stack3"],
        "valtos_stack3": stats["valtos_stack3"],
        "record_aligned": stats["record_aligned"],
        "dale_aligned": stats["dale_aligned"],
        "valtos_aligned": stats["valtos_aligned"],
        "unfinished": stats["unfinished"],
        "chaos": stats["chaos"],
        "nonempty_side_p25": stats["nonempty_side_p25"],
    }
    print_row("all", overall)
    print()
    for row in stats["sessions"]:
        print_row(str(row["session"]), row)
    print()
    print(json.dumps({k: v for k, v in stats.items() if k != "note"}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
