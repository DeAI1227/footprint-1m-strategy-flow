#!/usr/bin/env python3
"""One-shot Days 2–7 tables from OKX public trades. Prints markdown-ready blocks."""
from __future__ import annotations

import os
import sys
from collections import defaultdict
from statistics import median

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from rebuild_sol_footprint_stats import (
    bar_metrics,
    build_bars,
    fmt_ci,
    iso,
    load_trades,
    nonempty_side_volumes,
    percentile,
    stacked_runs,
    tick_prices,
)

PATHS = [
    "/tmp/sol_okx_asia_d1.jsonl",
    "/tmp/sol_okx_eu.jsonl",
    "/tmp/sol_okx_us.jsonl",
    "/tmp/sol_okx_trades.jsonl",
]


def pct(xs, p):
    return percentile(xs, p)


def med(xs):
    return median(xs) if xs else 0.0


def agg(bars, bucket, ratio, min_vol):
    ms = [bar_metrics(b, bucket, ratio, min_vol, 3) for b in bars]
    n = len(ms)
    if n == 0:
        return None
    fracs = [m["imb_frac"] for m in ms]
    k3 = sum(m["has3"] for m in ms)
    k4 = sum(m["has4"] for m in ms)
    kc = sum(m["chaos"] for m in ms)
    ka = sum(m["aligned3"] for m in ms)
    kx = sum(m["contra3"] for m in ms)
    # 4-stack direction using same OHLC rule, recomputed from has4 + aligned/contra overlap isn't in metrics.
    # Approximate: count has4 among aligned3/contra3 is wrong. Recompute from stacks in a second pass below.
    ku = sum(m["unf"] for m in ms)
    kuh = sum(m["unf_hi"] for m in ms)
    kul = sum(m["unf_lo"] for m in ms)
    both = sum(m["unf_hi"] and m["unf_lo"] for m in ms)
    va_ok = sum(m["va_ok"] for m in ms)
    zeros = sum(m["zero_pairs"] for m in ms)
    va_w = [m["va_width"] for m in ms if m["va_ok"]]
    pocs = [m["poc"] for m in ms]
    aligned_poc = sum(1 for i in range(1, len(pocs)) if pocs[i] is not None and pocs[i] == pocs[i - 1])
    unique_poc = len({p for p in pocs if p is not None})
    up = sum(m["up"] for m in ms)
    down = sum(m["down"] for m in ms)
    cols = [m["n_cols"] for m in ms]
    return {
        "n": n,
        "med_imb": 100 * sorted(fracs)[n // 2],
        "k3": k3,
        "k4": k4,
        "kc": kc,
        "ka": ka,
        "kx": kx,
        "ku": ku,
        "kuh": kuh,
        "kul": kul,
        "both": both,
        "va_ok": va_ok,
        "zeros": zeros,
        "va_w_med": med(va_w),
        "aligned_poc": aligned_poc,
        "unique_poc": unique_poc,
        "up": up,
        "down": down,
        "cols_med": med(cols),
        "ms": ms,
        "bars": bars,
    }


def ci(k, n):
    return fmt_ci(k, n)


def extra_poc_va(bars, bucket):
    """POC location vs bar mid; range; finished extremes."""
    lo_n = mid_n = hi_n = 0
    ranges = []
    fin_hi = fin_lo = 0
    unf_hi = unf_lo = 0
    for b in bars:
        rng = (b["high"] - b["low"]) / bucket
        ranges.append(rng)
        mid = (b["high"] + b["low"]) / 2
        vol_at = defaultdict(float)
        for p, v in b["bid"].items():
            vol_at[p] += v
        for p, v in b["ask"].items():
            vol_at[p] += v
        if not vol_at:
            continue
        poc = max(vol_at, key=vol_at.get)
        if poc < mid - bucket * 0.25:
            lo_n += 1
        elif poc > mid + bucket * 0.25:
            hi_n += 1
        else:
            mid_n += 1
        bh = b["bid"].get(b["high"], 0.0)
        ah = b["ask"].get(b["high"], 0.0)
        bl = b["bid"].get(b["low"], 0.0)
        al = b["ask"].get(b["low"], 0.0)
        if ah > 0 and bh == 0:
            fin_hi += 1
        if bh > 0 and ah > 0:
            unf_hi += 1
        if bl > 0 and al == 0:
            fin_lo += 1
        if bl > 0 and al > 0:
            unf_lo += 1
    return {
        "poc_lo": lo_n,
        "poc_mid": mid_n,
        "poc_hi": hi_n,
        "range_med": med(ranges),
        "fin_hi": fin_hi,
        "fin_lo": fin_lo,
        "unf_hi": unf_hi,
        "unf_lo": unf_lo,
    }


def stack4_dir(bars, bucket, ratio, min_vol):

    a4 = c4 = k4 = 0
    for bar in bars:
        m = bar_metrics(bar, bucket, ratio, min_vol, 3)
        prices = tick_prices(bar, bucket)
        bid, ask = bar["bid"], bar["ask"]
        buy, sell = [], []
        for p in prices:
            a = ask.get(p, 0.0)
            b_below = bid.get(round(p - bucket, 10), 0.0)
            b_here = bid.get(p, 0.0)
            a_above = ask.get(round(p + bucket, 10), 0.0)
            if a > 0 and b_below > 0 and a >= min_vol and b_below >= min_vol and a >= ratio * b_below:
                buy.append(p)
            if b_here > 0 and a_above > 0 and b_here >= min_vol and a_above >= min_vol and b_here >= ratio * a_above:
                sell.append(p)
        bs = stacked_runs([p in set(buy) for p in prices])
        ss = stacked_runs([p in set(sell) for p in prices])
        has4 = bs >= 4 or ss >= 4
        if not has4:
            continue
        k4 += 1
        up = bar["close"] > bar["open"]
        down = bar["close"] < bar["open"]
        if (bs >= 4 and up) or (ss >= 4 and down):
            a4 += 1
        if (bs >= 4 and down) or (ss >= 4 and up):
            c4 += 1
    return k4, a4, c4


def span3(bars, bucket):
    n = len(bars)
    k = sum(1 for b in bars if (b["high"] - b["low"]) / bucket + 1 >= 3 - 1e-9)
    return k, n


def main():
    paths = [p for p in PATHS if os.path.exists(p)]
    print("files", paths)
    trades = load_trades(paths)
    print(f"trades={len(trades)} {iso(trades[0][0])} → {iso(trades[-1][0])}")

    bars01 = build_bars(trades, 0.01)
    print(f"closed 0.01 bars={len(bars01)} {iso(bars01[0]['t0'])} → {iso(bars01[-1]['t0'])}")

    by = defaultdict(list)
    for b in bars01:
        by[b["session"]].append(b)
        by["all"].append(b)
    print("session counts", {k: len(v) for k, v in by.items() if k != "all"}, "all", len(by["all"]))

    print("\n===== DAY 2 buckets @ 200% no minvol =====")
    for buck in (0.01, 0.02, 0.04):
        bars = build_bars(trades, buck) if buck != 0.01 else bars01
        bb = defaultdict(list)
        for b in bars:
            bb[b["session"]].append(b)
            bb["all"].append(b)
        for sess in ("all", "asia", "eu", "us", "thin"):
            subset = bb.get(sess) or []
            if not subset:
                continue
            a = agg(subset, buck, 2.0, 0.0)
            kspan, nspan = span3(subset, buck)
            print(
                f"b={buck} {sess}: n={a['n']} cols_med={a['cols_med']:.1f} "
                f"imb_med={a['med_imb']:.1f}% 3s={ci(a['k3'], a['n'])} 4s={a['k4']}/{a['n']} "
                f"chaos={a['kc']} span3={kspan}/{nspan} ({100*kspan/nspan:.0f}%)"
            )

    print("\n===== DAY 3 ignore zeros + minvol scan @ 0.01 200% =====")
    for sess in ("all", "asia", "eu", "us", "thin"):
        subset = by.get(sess) or []
        if not subset:
            continue
        vols = nonempty_side_volumes(subset)
        zeros = agg(subset, 0.01, 2.0, 0.0)["zeros"]
        print(
            f"{sess}: cells={len(vols)} p10={pct(vols,10):.2f} p20={pct(vols,20):.2f} "
            f"p25={pct(vols,25):.2f} p30={pct(vols,30):.2f} p40={pct(vols,40):.2f} p50={pct(vols,50):.2f} "
            f"zero_pairs={zeros}"
        )
        p25 = pct(vols, 25)
        for label, mv in [
            ("none", 0.0),
            ("p10", pct(vols, 10)),
            ("p20", pct(vols, 20)),
            ("p25", p25),
            ("p30", pct(vols, 30)),
            ("p40", pct(vols, 40)),
            ("p50", pct(vols, 50)),
        ]:
            a = agg(subset, 0.01, 2.0, mv)
            print(
                f"  {sess} {label} min={mv:.2f}: imb_med={a['med_imb']:.1f}% "
                f"3s={ci(a['k3'], a['n'])} 4s={a['k4']}/{a['n']}"
            )

    print("\n===== DAY 4/5 rates @ session p25 =====")
    for sess in ("all", "asia", "eu", "us", "thin"):
        subset = by.get(sess) or []
        if not subset:
            continue
        p25 = pct(nonempty_side_volumes(subset), 25)
        print(f"\n# {sess} n={len(subset)} p25={p25:.2f}")
        for ratio, name in ((2.0, "200"), (3.0, "300"), (4.0, "400")):
            a = agg(subset, 0.01, ratio, p25)
            k4, a4, c4 = stack4_dir(subset, 0.01, ratio, p25)
            print(
                f"{name}: imb_med={a['med_imb']:.1f}% 3s={ci(a['k3'], a['n'])} "
                f"aligned3={a['ka']}/{a['n']} contra3={a['kx']} "
                f"4s={a['k4']}/{a['n']} aligned4={a4} contra4={c4} chaos={a['kc']} "
                f"up={a['up']} down={a['down']}"
            )

    print("\n===== DAY 6 POC/VA (no imbalance) =====")
    for sess in ("all", "asia", "eu", "us", "thin"):
        subset = by.get(sess) or []
        if not subset:
            continue
        a = agg(subset, 0.01, 2.0, 0.0)
        x = extra_poc_va(subset, 0.01)
        print(
            f"{sess}: n={a['n']} va={a['va_ok']}/{a['n']} aligned_poc={a['aligned_poc']}/{max(a['n']-1,1)} "
            f"unique_poc={a['unique_poc']} poc_lo/mid/hi={x['poc_lo']}/{x['poc_mid']}/{x['poc_hi']} "
            f"range_med={x['range_med']:.1f}tick va_w_med={a['va_w_med']:.1f}"
        )

    print("\n===== DAY 7 unfinished =====")
    for sess in ("all", "asia", "eu", "us", "thin"):
        subset = by.get(sess) or []
        if not subset:
            continue
        a = agg(subset, 0.01, 2.0, 0.0)
        x = extra_poc_va(subset, 0.01)
        print(
            f"{sess}: unf_any={ci(a['ku'], a['n'])} hi={a['kuh']} lo={a['kul']} both={a['both']} "
            f"finished_hi={x['fin_hi']} finished_lo={x['fin_lo']}"
        )


if __name__ == "__main__":
    sys.path.insert(0, "/agent/scripts")
    main()
