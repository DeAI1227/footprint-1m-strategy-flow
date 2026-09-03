#!/usr/bin/env python3
"""Week 2: count script A–G failure screens on closed 1m OKX footprints.

Frozen week-1 eyes: bucket 0.01, session p25, Ignore Zero, stack 3,
bar-direction on, 300% and 400% in parallel. No L2 → F is not_evaluated.
"""
from __future__ import annotations

import os
import sys
from collections import defaultdict
from datetime import datetime, timezone

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from rebuild_sol_footprint_stats import (
    bar_metrics,
    build_bars,
    fmt_ci,
    iso,
    load_trades,
    nonempty_side_volumes,
    percentile,
    session_of,
    stacked_runs,
    tick_prices,
)

PATHS = [
    "/tmp/sol_okx_asia_d1.jsonl",
    "/tmp/sol_okx_eu.jsonl",
    "/tmp/sol_okx_us.jsonl",
    "/tmp/sol_okx_trades.jsonl",
    "/tmp/sol_okx_eu_d2.jsonl",
    "/tmp/sol_okx_us_d2.jsonl",
    "/tmp/sol_okx_asia_d3.jsonl",
]
BUCKET = 0.01
LOOK_A = 20  # bars to wait for a pullback after leave
LOOK_B = 8
LOOK_G = 12


def stack_runs(flags, prices):
    runs = []
    i = 0
    n = len(flags)
    while i < n:
        if not flags[i]:
            i += 1
            continue
        j = i
        while j < n and flags[j]:
            j += 1
        if j - i >= 3:
            runs.append((prices[i], prices[j - 1], j - i))
        i = j
    return runs


def zones_for_bar(bar, ratio, min_vol):
    prices = tick_prices(bar, BUCKET)
    bid, ask = bar["bid"], bar["ask"]
    buy, sell = [], []
    for p in prices:
        a = ask.get(p, 0.0)
        b_below = bid.get(round(p - BUCKET, 10), 0.0)
        b_here = bid.get(p, 0.0)
        a_above = ask.get(round(p + BUCKET, 10), 0.0)
        if a > 0 and b_below > 0 and a >= min_vol and b_below >= min_vol and a >= ratio * b_below:
            buy.append(p)
        if b_here > 0 and a_above > 0 and b_here >= min_vol and a_above >= min_vol and b_here >= ratio * a_above:
            sell.append(p)
    buy_flags = [p in set(buy) for p in prices]
    sell_flags = [p in set(sell) for p in prices]
    up = bar["close"] > bar["open"]
    down = bar["close"] < bar["open"]
    chaos = stacked_runs(buy_flags) >= 3 and stacked_runs(sell_flags) >= 3
    out = []
    if not chaos:
        if up:
            for lo, hi, n in stack_runs(buy_flags, prices):
                out.append({"side": "buy", "lo": min(lo, hi), "hi": max(lo, hi), "n": n})
        if down:
            for lo, hi, n in stack_runs(sell_flags, prices):
                out.append({"side": "sell", "lo": min(lo, hi), "hi": max(lo, hi), "n": n})
    m = bar_metrics(bar, BUCKET, ratio, min_vol, 3)
    return out, m


def fully_above(bar, zhi):
    return bar["low"] > zhi


def fully_below(bar, zlo):
    return bar["high"] < zlo


def touches(bar, zlo, zhi):
    return bar["low"] <= zhi and bar["high"] >= zlo


def excess_low(bar):
    return bar["bid"].get(bar["low"], 0.0) > 0 and bar["ask"].get(bar["low"], 0.0) == 0


def excess_high(bar):
    return bar["ask"].get(bar["high"], 0.0) > 0 and bar["bid"].get(bar["high"], 0.0) == 0


def script_a(bars, ratio, min_vol, leave_bars):
    armed = no_leave = 0
    left = 0
    ret_reject = ret_punch = ret_inside = no_return = 0
    for i, bar in enumerate(bars):
        zs, m = zones_for_bar(bar, ratio, min_vol)
        if not zs:
            continue
        armed += 1
        # one zone per bar (first armed)
        z = zs[0]
        # look for leave
        leave_i = None
        run = 0
        for j in range(i + 1, min(i + 1 + 12, len(bars))):
            b = bars[j]
            ok = fully_above(b, z["hi"]) if z["side"] == "buy" else fully_below(b, z["lo"])
            if ok:
                run += 1
                if run >= leave_bars:
                    leave_i = j
                    break
            else:
                run = 0
        if leave_i is None:
            no_leave += 1
            continue
        left += 1
        found = False
        for k in range(leave_i + 1, min(leave_i + 1 + LOOK_A, len(bars))):
            b = bars[k]
            if not touches(b, z["lo"], z["hi"]):
                continue
            found = True
            if z["side"] == "buy":
                # pullback from above into buy zone as support
                if b["close"] < z["lo"]:
                    ret_punch += 1
                elif excess_low(b) or b["close"] > z["hi"]:
                    ret_reject += 1
                else:
                    ret_inside += 1
            else:
                if b["close"] > z["hi"]:
                    ret_punch += 1
                elif excess_high(b) or b["close"] < z["lo"]:
                    ret_reject += 1
                else:
                    ret_inside += 1
            break
        if not found:
            no_return += 1
    return {
        "armed": armed,
        "no_leave": no_leave,
        "left": left,
        "reject": ret_reject,
        "punch": ret_punch,
        "inside": ret_inside,
        "no_return": no_return,
    }


def swings(bars, n=5):
    highs, lows = [], []
    for i in range(n, len(bars) - n):
        h = bars[i]["high"]
        l = bars[i]["low"]
        if h == max(b["high"] for b in bars[i - n : i + n + 1]) and sum(
            1 for b in bars[i - n : i + n + 1] if b["high"] == h
        ) == 1:
            highs.append(i)
        if l == min(b["low"] for b in bars[i - n : i + n + 1]) and sum(
            1 for b in bars[i - n : i + n + 1] if b["low"] == l
        ) == 1:
            lows.append(i)
    return highs, lows


def script_c(bars, trap_k):
    hs, ls = swings(bars, 5)
    last_h = last_l = None
    hi_px = lo_px = None
    breaks = 0
    reclaim = 0
    accepted = 0
    for i, bar in enumerate(bars):
        if i in hs:
            last_h = i
            hi_px = bar["high"]
        if i in ls:
            last_l = i
            lo_px = bar["low"]
        if hi_px is None or lo_px is None or hi_px <= lo_px:
            continue
        # break
        side = None
        if bar["close"] > hi_px:
            side = "up"
            edge = hi_px
        elif bar["close"] < lo_px:
            side = "down"
            edge = lo_px
        else:
            continue
        # skip if previous bar already outside (continuation)
        if i > 0:
            prev = bars[i - 1]
            if side == "up" and prev["close"] > hi_px:
                continue
            if side == "down" and prev["close"] < lo_px:
                continue
        breaks += 1
        rec = False
        out_poc = 0
        for j in range(i + 1, min(i + 1 + max(trap_k, 5), len(bars))):
            c = bars[j]["close"]
            inside = lo_px <= c <= hi_px
            if j - i <= trap_k and inside:
                rec = True
                break
            poc = None
            vol_at = defaultdict(float)
            for p, v in bars[j]["bid"].items():
                vol_at[p] += v
            for p, v in bars[j]["ask"].items():
                vol_at[p] += v
            if vol_at:
                poc = max(vol_at, key=vol_at.get)
                outside = poc > hi_px if side == "up" else poc < lo_px
                if outside:
                    out_poc += 1
        if rec:
            reclaim += 1
        elif out_poc >= 3:
            accepted += 1
    return {"breaks": breaks, "reclaim": reclaim, "accepted": accepted}


def script_b(bars, ratio, min_vol):
    """Absorption at prior stack edge / prior POC; count second drive-through."""
    vols = [b["bid_vol"] + b["ask_vol"] for b in bars]
    atk = []
    held = punch = vacuum = 0
    recent_edges = []
    for i, bar in enumerate(bars):
        zs, m = zones_for_bar(bar, ratio, min_vol)
        for z in zs:
            recent_edges.append((i, z["lo"], z["hi"], z["side"]))
        recent_edges = [e for e in recent_edges if i - e[0] <= 30]
        poc = m["poc"]
        # vacuum: high volume, no nearby key location
        win = vols[max(0, i - 29) : i + 1]
        p75 = percentile(win, 75) if win else 0
        vol = vols[i]
        keys = []
        if poc is not None and i > 0:
            keys.append(poc)
        for _, lo, hi, _ in recent_edges:
            keys.extend([lo, hi])
        near = any(abs(bar["high"] - k) <= 2 * BUCKET or abs(bar["low"] - k) <= 2 * BUCKET for k in keys)
        high_vol = vol >= p75 and p75 > 0
        if high_vol and not near:
            vacuum += 1
            continue
        if not (high_vol and near and keys):
            continue
        # failed progress: close in lower 40% of bar on down bar, or upper 40% on up bar
        rng = bar["high"] - bar["low"]
        if rng <= 0:
            continue
        pos = (bar["close"] - bar["low"]) / rng
        down_atk = bar["delta"] < 0 and pos >= 0.6
        up_atk = bar["delta"] > 0 and pos <= 0.4
        if not (down_atk or up_atk):
            continue
        atk.append(i)
        level = bar["low"] if down_atk else bar["high"]
        thru = False
        for j in range(i + 1, min(i + 1 + LOOK_B, len(bars))):
            if down_atk and bars[j]["close"] < level - BUCKET:
                thru = True
                break
            if up_atk and bars[j]["close"] > level + BUCKET:
                thru = True
                break
        if thru:
            punch += 1
        else:
            held += 1
    return {"attacks": len(atk), "held": held, "second_punch": punch, "vacuum": vacuum}


def script_d(bars, ratio, min_vol, accept_bars=3):
    """Leave a stack, POCs stay outside, then pullback to old edge."""
    events = reject = punch = none = 0
    for i, bar in enumerate(bars):
        zs, _ = zones_for_bar(bar, ratio, min_vol)
        if not zs:
            continue
        z = zs[0]
        # require accept_bars POCs outside
        if i + accept_bars >= len(bars):
            continue
        ok = True
        for j in range(i + 1, i + 1 + accept_bars):
            vol_at = defaultdict(float)
            for p, v in bars[j]["bid"].items():
                vol_at[p] += v
            for p, v in bars[j]["ask"].items():
                vol_at[p] += v
            if not vol_at:
                ok = False
                break
            poc = max(vol_at, key=vol_at.get)
            if z["side"] == "buy" and poc <= z["hi"]:
                ok = False
                break
            if z["side"] == "sell" and poc >= z["lo"]:
                ok = False
                break
        if not ok:
            continue
        events += 1
        found = False
        start = i + 1 + accept_bars
        for k in range(start, min(start + 30, len(bars))):
            b = bars[k]
            if not touches(b, z["lo"], z["hi"]):
                continue
            found = True
            if z["side"] == "buy":
                if b["close"] < z["lo"]:
                    punch += 1
                else:
                    reject += 1
            else:
                if b["close"] > z["hi"]:
                    punch += 1
                else:
                    reject += 1
            break
        if not found:
            none += 1
    return {"accepted": events, "reject": reject, "punch": punch, "no_return": none}


def script_e(bars):
    """Session-anchored CVD: price new extreme, CVD not. First vs later."""
    # reset CVD at session change
    cvd = 0.0
    prev_sess = None
    max_px = min_px = None
    max_cvd = min_cvd = None
    first_up = later_up = first_dn = later_dn = 0
    seen_up = seen_dn = False
    n_sess = 0
    for bar in bars:
        day = datetime.fromtimestamp(bar["t0"] / 1000, timezone.utc).date()
        sess = (day, bar["session"])
        if sess != prev_sess:
            n_sess += 1
            cvd = 0.0
            max_px = bar["high"]
            min_px = bar["low"]
            max_cvd = min_cvd = 0.0
            seen_up = seen_dn = False
            prev_sess = sess
        cvd += bar["delta"]
        px_new_h = bar["high"] > max_px
        px_new_l = bar["low"] < min_px
        if px_new_h and max_cvd is not None and cvd < max_cvd:
            if not seen_up:
                first_up += 1
                seen_up = True
            else:
                later_up += 1
        if px_new_l and min_cvd is not None and cvd > min_cvd:
            if not seen_dn:
                first_dn += 1
                seen_dn = True
            else:
                later_dn += 1
        max_px = max(max_px, bar["high"])
        min_px = min(min_px, bar["low"])
        max_cvd = max(max_cvd, cvd)
        min_cvd = min(min_cvd, cvd)
    return {
        "sessions": n_sess,
        "first_up": first_up,
        "later_up": later_up,
        "first_dn": first_dn,
        "later_dn": later_dn,
    }


def script_g(bars, ratio, min_vol):
    """Unfinished only at recent stack / POC / swing. Fill vs extend."""
    hs, ls = swings(bars, 5)
    swing_px = {}
    for i in hs:
        swing_px[i] = ("h", bars[i]["high"])
    for i in ls:
        swing_px[i] = ("l", bars[i]["low"])
    key_unf = fill = extend = neither = cheap = 0
    recent_z = []
    for i, bar in enumerate(bars):
        zs, m = zones_for_bar(bar, ratio, min_vol)
        for z in zs:
            recent_z.append((i, z["lo"], z["hi"]))
        recent_z = [e for e in recent_z if i - e[0] <= 20]
        unf_h = bar["bid"].get(bar["high"], 0) > 0 and bar["ask"].get(bar["high"], 0) > 0
        unf_l = bar["bid"].get(bar["low"], 0) > 0 and bar["ask"].get(bar["low"], 0) > 0
        if not (unf_h or unf_l):
            continue
        keys = [m["poc"]] if m["poc"] is not None else []
        for _, lo, hi in recent_z:
            keys.extend([lo, hi])
        for j, (kind, px) in swing_px.items():
            if 0 <= i - j <= 20:
                keys.append(px)
        loc_h = unf_h and any(abs(bar["high"] - k) <= 2 * BUCKET for k in keys if k is not None)
        loc_l = unf_l and any(abs(bar["low"] - k) <= 2 * BUCKET for k in keys if k is not None)
        if not (loc_h or loc_l):
            cheap += 1
            continue
        key_unf += 1
        target = bar["high"] if loc_h else bar["low"]
        side = "h" if loc_h else "l"
        got = False
        for j in range(i + 1, min(i + 1 + LOOK_G, len(bars))):
            b = bars[j]
            if side == "h":
                if b["high"] > target + BUCKET:
                    extend += 1
                    got = True
                    break
                if b["high"] >= target and b["close"] < target:
                    fill += 1
                    got = True
                    break
            else:
                if b["low"] < target - BUCKET:
                    extend += 1
                    got = True
                    break
                if b["low"] <= target and b["close"] > target:
                    fill += 1
                    got = True
                    break
        if not got:
            neither += 1
    return {
        "key_unf": key_unf,
        "fill": fill,
        "extend": extend,
        "neither": neither,
        "cheap": cheap,
    }


def print_a(label, a):
    left = a["left"]
    print(f"## {label}")
    print(f"- armed aligned 3-stack bars: {a['armed']}")
    print(f"- never left in 12m: {a['no_leave']} (chase, not A)")
    print(f"- left then eligible A: {left}")
    if left:
        print(f"- pullback reject/excess: {a['reject']} ({100*a['reject']/left:.0f}% of left)")
        print(f"- pullback punched through: {a['punch']} ({100*a['punch']/left:.0f}%)")
        print(f"- touched, no clear reject: {a['inside']}")
        print(f"- no return in {LOOK_A}m: {a['no_return']}")
    print()


def main():
    paths = [p for p in PATHS if os.path.exists(p)]
    trades = load_trades(paths)
    print(f"trades={len(trades)} {iso(trades[0][0])} → {iso(trades[-1][0])}")
    bars = build_bars(trades, BUCKET)
    print(f"closed 1m={len(bars)} {iso(bars[0]['t0'])} → {iso(bars[-1]['t0'])}")
    by = defaultdict(list)
    for b in bars:
        by[b["session"]].append(b)
        by["all"].append(b)
    print("sessions", {k: len(v) for k, v in by.items() if k != "all"})

    for ratio, name in ((3.0, "300%"), (4.0, "400%")):
        print(f"\n===== {name} =====")
        for sess in ("all", "asia", "eu", "us", "thin"):
            subset = by.get(sess) or []
            if len(subset) < 30:
                continue
            p25 = percentile(nonempty_side_volumes(subset), 25)
            print(f"\n# {sess} n={len(subset)} p25={p25:.1f}")
            for lb in (1, 2):
                a = script_a(subset, ratio, p25, lb)
                print_a(f"{sess} A leave={lb}", a)
            b = script_b(subset, ratio, p25)
            print(
                f"B {sess}: attacks_at_key={b['attacks']} held={b['held']} "
                f"second_punch={b['second_punch']} vacuum={b['vacuum']}"
            )
            d15 = script_d(subset, ratio, p25, 3)
            print(
                f"D {sess} accept=3: accepted={d15['accepted']} reject={d15['reject']} "
                f"punch={d15['punch']} no_return={d15['no_return']}"
            )
            g = script_g(subset, ratio, p25)
            print(
                f"G {sess}: key_unf={g['key_unf']} fill={g['fill']} extend={g['extend']} "
                f"neither={g['neither']} cheap_unf={g['cheap']}"
            )

    print("\n===== C trap bars (no imbalance ratio) =====")
    for sess in ("all", "asia", "eu", "us"):
        subset = by.get(sess) or []
        if len(subset) < 40:
            continue
        print(f"# {sess} n={len(subset)}")
        for k in (2, 3, 5):
            c = script_c(subset, k)
            rec = c["reclaim"]
            br = c["breaks"]
            print(
                f"TRAP_BARS={k}: breaks={br} reclaim={rec} "
                f"({100*rec/br:.0f}% of breaks)" if br else f"TRAP_BARS={k}: breaks=0",
                f"accepted_outside={c['accepted']}",
            )

    print("\n===== E CVD session-anchored =====")
    for sess in ("all", "asia", "eu", "us"):
        subset = by.get(sess) or []
        if len(subset) < 40:
            continue
        e = script_e(subset)
        print(
            f"{sess}: sessions={e['sessions']} first_up_div={e['first_up']} later_up={e['later_up']} "
            f"first_dn_div={e['first_dn']} later_dn={e['later_dn']}"
        )
    print("\nF: not_evaluated (no L2 book in public trades)")


if __name__ == "__main__":
    main()
