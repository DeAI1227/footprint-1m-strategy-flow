#!/usr/bin/env python3
"""Rebuild closed 1m bid×ask footprints from OKX public trades and print calibration tables.

Clock: exchange event time, bar [t, t+60_000). Last incomplete minute dropped.
Taker: side=buy → ask (taker buy); side=sell → bid (taker sell).
Diagonal imbalance: ask[p] vs bid[p-bucket], ignore zeros, optional two-sided min volume.
"""
from __future__ import annotations

import argparse
import json
import math
from collections import defaultdict
from datetime import datetime, timezone
from typing import Iterable


def iso(ts_ms: int) -> str:
    return datetime.fromtimestamp(ts_ms / 1000, timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")


def session_of(ts_ms: int) -> str:
    h = datetime.fromtimestamp(ts_ms / 1000, timezone.utc).hour
    if 0 <= h < 8:
        return "asia"
    if 13 <= h < 21:
        return "us"
    if 21 <= h:
        return "thin"
    return "eu"


def load_trades(paths) -> list[tuple[int, float, float, str]]:
    if isinstance(paths, str):
        paths = [paths]
    seen: set[str] = set()
    rows = []
    for path in paths:
        with open(path) as f:
            for line in f:
                r = json.loads(line)
                tid = str(r["tradeId"])
                if tid in seen:
                    continue
                seen.add(tid)
                rows.append((int(r["ts"]), float(r["px"]), float(r["sz"]), r["side"]))
    rows.sort(key=lambda x: x[0])
    return rows


def build_bars(trades, bucket: float):
    bars = {}
    for ts, px, sz, side in trades:
        t0 = ts - (ts % 60_000)
        b = bars.get(t0)
        if b is None:
            b = {
                "bid": defaultdict(float),
                "ask": defaultdict(float),
                "n": 0,
                "open": px,
                "close": px,
            }
            bars[t0] = b
        lvl = round(math.floor(px / bucket + 1e-12) * bucket, 10)
        if side == "buy":
            b["ask"][lvl] += sz
        else:
            b["bid"][lvl] += sz
        b["close"] = px
        b["n"] += 1
    if not bars:
        return []
    last = max(bars)
    # drop forming last minute
    keys = sorted(k for k in bars if k < last)
    out = []
    for t0 in keys:
        b = bars[t0]
        prices = sorted(set(b["bid"]) | set(b["ask"]))
        if not prices:
            continue
        bid_vol = sum(b["bid"].values())
        ask_vol = sum(b["ask"].values())
        out.append(
            {
                "t0": t0,
                "bid": dict(b["bid"]),
                "ask": dict(b["ask"]),
                "prices": prices,
                "high": max(prices),
                "low": min(prices),
                "bid_vol": bid_vol,
                "ask_vol": ask_vol,
                "delta": ask_vol - bid_vol,
                "open": b["open"],
                "close": b["close"],
                "n": b["n"],
                "session": session_of(t0),
            }
        )
    return out


def nonempty_side_volumes(bars) -> list[float]:
    xs = []
    for b in bars:
        for d in (b["bid"], b["ask"]):
            for v in d.values():
                if v > 0:
                    xs.append(v)
    return xs


def percentile(xs: list[float], p: float) -> float:
    if not xs:
        return 0.0
    ys = sorted(xs)
    if p <= 0:
        return ys[0]
    if p >= 100:
        return ys[-1]
    k = (len(ys) - 1) * (p / 100.0)
    f = math.floor(k)
    c = math.ceil(k)
    if f == c:
        return ys[int(k)]
    return ys[f] * (c - k) + ys[c] * (k - f)


def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float]:
    if n <= 0:
        return (0.0, 0.0)
    p = k / n
    z2 = z * z
    den = 1 + z2 / n
    centre = (p + z2 / (2 * n)) / den
    half = z * math.sqrt((p * (1 - p) + z2 / (4 * n)) / n) / den
    return (max(0.0, centre - half), min(1.0, centre + half))


def tick_prices(bar, bucket: float) -> list[float]:
    lo, hi = bar["low"], bar["high"]
    n = int(round((hi - lo) / bucket)) + 1
    return [round(lo + i * bucket, 10) for i in range(n)]


def stacked_runs(flags: Iterable[bool]) -> int:
    best = cur = 0
    for f in flags:
        if f:
            cur += 1
            best = max(best, cur)
        else:
            cur = 0
    return best


def bar_metrics(bar, bucket: float, ratio: float, min_vol: float, stack_n: int):
    prices = tick_prices(bar, bucket)
    bid, ask = bar["bid"], bar["ask"]
    buy_imb = []
    sell_imb = []
    nonempty_cols = 0
    imb_cols = 0
    zero_pairs = 0
    for p in prices:
        a = ask.get(p, 0.0)
        b_below = bid.get(round(p - bucket, 10), 0.0)
        b_here = bid.get(p, 0.0)
        a_above = ask.get(round(p + bucket, 10), 0.0)
        if a > 0 or b_here > 0:
            nonempty_cols += 1
        # buy imbalance: ask[p] vs bid[p-bucket]
        if a == 0 or b_below == 0:
            if (a > 0) != (b_below > 0):
                zero_pairs += 1
        else:
            if a >= min_vol and b_below >= min_vol and a >= ratio * b_below:
                buy_imb.append(p)
        # sell imbalance: bid[p] vs ask[p+bucket]
        if b_here == 0 or a_above == 0:
            if (b_here > 0) != (a_above > 0):
                zero_pairs += 1
        else:
            if b_here >= min_vol and a_above >= min_vol and b_here >= ratio * a_above:
                sell_imb.append(p)
        if (a >= min_vol and b_below >= min_vol and a >= ratio * b_below) or (
            b_here >= min_vol and a_above >= min_vol and b_here >= ratio * a_above
        ):
            imb_cols += 1

    buy_set = set(buy_imb)
    sell_set = set(sell_imb)
    buy_flags = [p in buy_set for p in prices]
    sell_flags = [p in sell_set for p in prices]
    buy_stack = stacked_runs(buy_flags)
    sell_stack = stacked_runs(sell_flags)
    has3 = buy_stack >= 3 or sell_stack >= 3
    has4 = buy_stack >= 4 or sell_stack >= 4
    chaos = buy_stack >= 3 and sell_stack >= 3
    bar_up = bar["close"] > bar["open"]
    bar_down = bar["close"] < bar["open"]
    aligned3 = False
    contra3 = False
    if has3:
        if (buy_stack >= 3 and bar_up) or (sell_stack >= 3 and bar_down):
            aligned3 = True
        if (buy_stack >= 3 and bar_down) or (sell_stack >= 3 and bar_up):
            contra3 = True
        if chaos:
            aligned3 = False
    # ATAS / Dale / Orderflows: unfinished = both sides printed at the extreme.
    # Finished high = bid[high]==0 (excess); finished low = ask[low]==0.
    unfinished_high = bid.get(bar["high"], 0.0) > 0 and ask.get(bar["high"], 0.0) > 0
    unfinished_low = bid.get(bar["low"], 0.0) > 0 and ask.get(bar["low"], 0.0) > 0
    imb_frac = (imb_cols / nonempty_cols) if nonempty_cols else 0.0
    # POC / VA
    vol_at = defaultdict(float)
    for p, v in bid.items():
        vol_at[p] += v
    for p, v in ask.items():
        vol_at[p] += v
    poc = max(vol_at, key=vol_at.get) if vol_at else None
    total = sum(vol_at.values())
    va_ok = False
    va_width = 0
    if poc is not None and total > 0:
        included = {poc}
        acc = vol_at[poc]
        lo = hi = poc
        while acc < 0.7 * total:
            left = round(lo - bucket, 10)
            right = round(hi + bucket, 10)
            lv = vol_at.get(left, 0.0) if left not in included else 0.0
            rv = vol_at.get(right, 0.0) if right not in included else 0.0
            if lv == 0 and rv == 0:
                break
            if lv >= rv:
                included.add(left)
                acc += lv
                lo = left
            else:
                included.add(right)
                acc += rv
                hi = right
        va_ok = acc >= 0.7 * total
        va_width = int(round((hi - lo) / bucket)) + 1 if va_ok else 0
    return {
        "imb_frac": imb_frac,
        "has3": has3,
        "has4": has4,
        "chaos": chaos,
        "aligned3": aligned3,
        "contra3": contra3,
        "zero_pairs": zero_pairs,
        "unf_hi": unfinished_high,
        "unf_lo": unfinished_low,
        "unf": unfinished_high or unfinished_low,
        "poc": poc,
        "va_ok": va_ok,
        "va_width": va_width,
        "n_cols": nonempty_cols,
        "up": bar_up,
        "down": bar_down,
    }


def fmt_ci(k, n) -> str:
    lo, hi = wilson(k, n)
    pct = 100 * k / n if n else 0
    return f"{k}/{n} ({pct:.1f}%, Wilson 95% {100*lo:.1f}–{100*hi:.1f}%)"


def summarize(label: str, bars, bucket, ratio, min_vol):
    ms = [bar_metrics(b, bucket, ratio, min_vol, 3) for b in bars]
    n = len(ms)
    if n == 0:
        print(f"## {label}: empty")
        return
    fracs = sorted(m["imb_frac"] for m in ms)
    med = fracs[n // 2]
    k3 = sum(m["has3"] for m in ms)
    k4 = sum(m["has4"] for m in ms)
    kc = sum(m["chaos"] for m in ms)
    ka = sum(m["aligned3"] for m in ms)
    kx = sum(m["contra3"] for m in ms)
    ku = sum(m["unf"] for m in ms)
    kuh = sum(m["unf_hi"] for m in ms)
    kul = sum(m["unf_lo"] for m in ms)
    pocs = [m["poc"] for m in ms if m["poc"] is not None]
    aligned_poc = 0
    for i in range(1, len(pocs)):
        if pocs[i] == pocs[i - 1]:
            aligned_poc += 1
    va_ok = sum(m["va_ok"] for m in ms)
    print(f"## {label}")
    print(f"- bars: {n}")
    print(f"- nonempty-col imb median: {100*med:.1f}%")
    print(f"- 3-stack: {fmt_ci(k3, n)}  ~every {n/k3:.1f}m" if k3 else f"- 3-stack: 0/{n}")
    print(f"- 4-stack: {fmt_ci(k4, n)}")
    print(f"- chaos 3+3: {kc}/{n}")
    print(f"- 3-stack aligned to bar OHLC: {fmt_ci(ka, n)}")
    print(f"- 3-stack contra to bar OHLC: {kx}/{n}")
    print(f"- unfinished high or low: {fmt_ci(ku, n)} (hi {kuh}, lo {kul})")
    print(f"- VA 70% reachable: {va_ok}/{n}")
    print(f"- aligned POC consecutive: {aligned_poc}/{max(n-1,1)}")
    print()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--path", nargs="+", default=["/tmp/sol_okx_trades.jsonl"])
    ap.add_argument("--bucket", type=float, default=0.01)
    args = ap.parse_args()
    trades = load_trades(args.path)
    if not trades:
        print("no trades")
        return
    print(f"trades={len(trades)} {iso(trades[0][0])} → {iso(trades[-1][0])}")
    bars = build_bars(trades, args.bucket)
    print(f"closed 1m bars={len(bars)} {iso(bars[0]['t0'])} → {iso(bars[-1]['t0'])}")
    vols = nonempty_side_volumes(bars)
    p25 = percentile(vols, 25)
    p50 = percentile(vols, 50)
    print(f"nonempty single-side cells={len(vols)} p25={p25:.3f} SOL p50={p50:.3f} SOL")
    print()
    by_sess = defaultdict(list)
    for b in bars:
        by_sess[b["session"]].append(b)
        by_sess["all"].append(b)

    for sess in ("all", "asia", "eu", "us", "thin"):
        if sess not in by_sess:
            continue
        subset = by_sess[sess]
        print(f"# session={sess} n={len(subset)}")
        vols_s = nonempty_side_volumes(subset)
        p25s = percentile(vols_s, 25)
        print(f"p25 nonempty single-side={p25s:.3f} SOL (n_cells={len(vols_s)})")
        for ratio, name in ((2.0, "200% record"), (3.0, "300% Dale"), (4.0, "400% Valtos")):
            summarize(f"{sess} {name} min={p25s:.3f}", subset, args.bucket, ratio, p25s)


if __name__ == "__main__":
    main()
