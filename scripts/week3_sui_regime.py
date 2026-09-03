#!/usr/bin/env python3
"""Week 3: SUI separate table + SOL regime veto + three-venue direction.

Does not reopen week-1 eyes or week-2 sentence knobs.
Does not sum venue volumes. Does not copy SOL bucket/p25 onto SUI.
"""
from __future__ import annotations

import csv
import json
import os
import sys
from collections import defaultdict
from datetime import datetime, timezone
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
    session_of,
    stacked_runs,
    tick_prices,
    wilson,
)
from week2_scripts_a_g import script_a

SOL_PATHS = [
    "/tmp/sol_okx_asia_d1.jsonl",
    "/tmp/sol_okx_eu.jsonl",
    "/tmp/sol_okx_us.jsonl",
    "/tmp/sol_okx_trades.jsonl",
    "/tmp/sol_okx_eu_d2.jsonl",
    "/tmp/sol_okx_us_d2.jsonl",
    "/tmp/sol_okx_asia_d3.jsonl",
]
SUI_PATHS = [
    "/tmp/sui_okx_trades.jsonl",
    "/tmp/sui_okx_us.jsonl",
    "/tmp/sui_okx_eu.jsonl",
    "/tmp/sui_okx_eu2.jsonl",
    "/tmp/sui_okx_asia.jsonl",
    "/tmp/sui_okx_us_d1.jsonl",
]
SUI_CANDLES = "/tmp/sui_okx_candles_1m.jsonl"
SUI_TICK = 0.0001
SUI_BUCKETS = (0.0001, 0.0002, 0.0005, 0.001, 0.002, 0.005, 0.01)
SOL_BUCKET = 0.01
LEAVE_BARS = 1
FUNDING_HOURS = (0, 8, 16)
BLACK_MIN = 15


def by_session(bars):
    out = defaultdict(list)
    for b in bars:
        out[b["session"]].append(b)
        out["all"].append(b)
    return out


def load_candles(path):
    rows = []
    with open(path) as f:
        for line in f:
            r = json.loads(line)
            ts = int(r[0])
            o, h, l, c = map(float, r[1:5])
            vol = float(r[5])
            confirm = str(r[-1]) if len(r) > 8 else "1"
            if confirm == "0":
                continue
            rows.append({"t0": ts, "open": o, "high": h, "low": l, "close": c, "vol": vol, "session": session_of(ts)})
    rows.sort(key=lambda x: x["t0"])
    return rows


def range_stats(candles, bucket, tick=SUI_TICK):
    spans = []
    cross3 = 0
    for c in candles:
        ticks = (c["high"] - c["low"]) / tick
        cols = (c["high"] - c["low"]) / bucket + 1e-12
        n_cols = int(cols) + (1 if cols > int(cols) + 1e-9 else 1)
        # number of price rows in [low, high] at this bucket
        n_rows = int(round((c["high"] - c["low"]) / bucket)) + 1
        spans.append((ticks, n_rows))
        if n_rows >= 3:
            cross3 += 1
    n = len(spans)
    ticks = [s[0] for s in spans]
    rows = [s[1] for s in spans]
    return {
        "n": n,
        "tick_p25": percentile(ticks, 25),
        "tick_p50": percentile(ticks, 50),
        "tick_p75": percentile(ticks, 75),
        "tick_p90": percentile(ticks, 90),
        "row_p50": percentile(rows, 50),
        "cross3": cross3,
        "cross3_pct": 100 * cross3 / n if n else 0,
    }


def empty_and_punch(bars, bucket):
    empty_fracs = []
    nonempty_med = []
    punch = 0  # bars whose high-low spans >=3 buckets but only 1 trade
    sparse = 0  # >=3 rows, trades <= 3
    for b in bars:
        prices = tick_prices(b, bucket)
        n_rows = len(prices)
        nonempty = sum(1 for p in prices if b["bid"].get(p, 0) > 0 or b["ask"].get(p, 0) > 0)
        if n_rows:
            empty_fracs.append(1 - nonempty / n_rows)
            nonempty_med.append(nonempty)
        if n_rows >= 3 and b["n"] == 1:
            punch += 1
        if n_rows >= 3 and b["n"] <= 3:
            sparse += 1
    n = len(bars)
    return {
        "empty_p50": 100 * percentile(empty_fracs, 50) if empty_fracs else 0,
        "empty_p75": 100 * percentile(empty_fracs, 75) if empty_fracs else 0,
        "nonempty_p50": percentile(nonempty_med, 50) if nonempty_med else 0,
        "punch1": punch,
        "sparse3": sparse,
        "n": n,
    }


def footprint_table(bars, bucket, ratio, min_vol):
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
    zeros = sum(m["zero_pairs"] for m in ms)
    return {
        "n": n,
        "med_imb": 100 * sorted(fracs)[n // 2],
        "k3": k3,
        "k4": k4,
        "kc": kc,
        "ka": ka,
        "kx": kx,
        "zeros": zeros,
        "zeros_per": zeros / n,
        "p3": 100 * k3 / n,
        "pa": 100 * ka / n,
    }


def load_oi(path):
    rows = []
    with open(path) as f:
        for line in f:
            r = json.loads(line)
            rows.append({"t": int(r[0]), "oi": float(r[1]), "oi_usd": float(r[3])})
    rows.sort(key=lambda x: x["t"])
    # unique t
    by = {}
    for r in rows:
        by[r["t"]] = r
    return [by[k] for k in sorted(by)]


def oi_drops(rows, pct_cut):
    out = []
    for i in range(1, len(rows)):
        prev, cur = rows[i - 1], rows[i]
        if prev["oi"] <= 0:
            continue
        chg = (cur["oi"] - prev["oi"]) / prev["oi"]
        if chg <= -pct_cut:
            out.append({"t": cur["t"], "chg": chg, "oi": cur["oi"], "prev": prev["oi"]})
    return out


def load_liq(path):
    rows = []
    with open(path) as f:
        for line in f:
            r = json.loads(line)
            ts = int(r["ts"])
            px = float(r.get("bkPx") or 0)
            sz = float(r.get("sz") or 0)
            rows.append(
                {
                    "ts": ts,
                    "t0": ts - ts % 60_000,
                    "notional": px * sz,
                    "sz": sz,
                    "side": r.get("side"),
                    "posSide": r.get("posSide"),
                }
            )
    return rows


def load_funding(path):
    rows = []
    with open(path) as f:
        for line in f:
            r = json.loads(line)
            rows.append(
                {
                    "t": int(r["fundingTime"]),
                    "rate": float(r.get("realizedRate") or r.get("fundingRate") or 0),
                }
            )
    rows.sort(key=lambda x: x["t"])
    return rows


def in_black(ts_ms, minutes=BLACK_MIN):
    dt = datetime.fromtimestamp(ts_ms / 1000, timezone.utc)
    minute = dt.hour * 60 + dt.minute
    for h in FUNDING_HOURS:
        target = h * 60
        # circular distance in minutes within a day
        d = min((minute - target) % (24 * 60), (target - minute) % (24 * 60))
        if d <= minutes:
            return True
    return False


def load_binance_klines(path):
    bars = {}
    with open(path) as f:
        for line in f:
            parts = line.strip().split(",")
            if len(parts) < 11:
                continue
            t0 = int(float(parts[0]))
            vol = float(parts[5])
            taker_buy = float(parts[9])
            if vol <= 0:
                continue
            delta = taker_buy - (vol - taker_buy)
            o, c = float(parts[1]), float(parts[4])
            bars[t0] = {"delta": delta, "vol": vol, "bar_dir": 1 if c > o else (-1 if c < o else 0)}
    return bars


def load_bybit_minute_delta(path):
    bars = {}
    with open(path) as f:
        reader = csv.reader(f)
        for row in reader:
            if not row or row[0].lower().startswith("timestamp"):
                continue
            ts = float(row[0])
            ts_ms = int(ts * 1000) if ts < 1e12 else int(ts)
            t0 = ts_ms - ts_ms % 60_000
            side = row[2].lower()
            sz = float(row[3])
            b = bars.get(t0)
            if b is None:
                b = {"bid": 0.0, "ask": 0.0, "n": 0}
                bars[t0] = b
            if side == "buy":
                b["ask"] += sz
            else:
                b["bid"] += sz
            b["n"] += 1
    out = {}
    for t0, b in bars.items():
        if b["n"] <= 0:
            continue
        out[t0] = {"delta": b["ask"] - b["bid"], "vol": b["ask"] + b["bid"], "n": b["n"]}
    return out


def sign(x, eps=0.0):
    if x > eps:
        return 1
    if x < -eps:
        return -1
    return 0


def count_pair(a_map, b_map, label):
    both = agree = disagree = zero = 0
    for t0, da in a_map.items():
        if t0 not in b_map:
            continue
        sa, sb = sign(da), sign(b_map[t0])
        both += 1
        if sa == 0 or sb == 0:
            zero += 1
        elif sa == sb:
            agree += 1
        else:
            disagree += 1
    usable = agree + disagree
    if usable:
        print(
            f"{label}: overlap={both} zero_delta={zero} agree={agree} disagree={disagree} "
            f"agree_of_nonzero={100*agree/usable:.1f}%"
        )
    else:
        print(f"{label}: overlap={both} no nonzero")
    return both, agree, disagree, zero


def print_a(label, a):
    left = a["left"]
    print(f"## {label}")
    print(f"- armed aligned 3-stack: {a['armed']}")
    print(f"- never left (chase): {a['no_leave']}")
    print(f"- left / eligible A: {left}")
    if left:
        print(f"- reject: {a['reject']} ({100*a['reject']/left:.0f}%)")
        print(f"- punch: {a['punch']} ({100*a['punch']/left:.0f}%)")
        print(f"- no return 20m: {a['no_return']}")
    print()


def main():
    print("===== SUI instrument =====")
    print("OKX SUI-USDT-SWAP tickSz=0.0001 ctVal=1 (sz = SUI). Not SOL 0.01.")

    candles = load_candles(SUI_CANDLES)
    print(f"SUI candles closed={len(candles)} {iso(candles[0]['t0'])} → {iso(candles[-1]['t0'])}")
    print("\n===== Day 15 candle geometry (OHLC, not footprint) =====")
    byc = by_session(candles)
    for sess in ("all", "asia", "eu", "us", "thin"):
        subset = byc.get(sess) or []
        if len(subset) < 30:
            continue
        st = range_stats(subset, SUI_TICK)
        print(
            f"{sess} n={st['n']} tick_p50={st['tick_p50']:.1f} p25={st['tick_p25']:.1f} "
            f"p75={st['tick_p75']:.1f} p90={st['tick_p90']:.1f}"
        )
    print("\n# rows at candidate buckets (all candles)")
    for bkt in SUI_BUCKETS:
        st = range_stats(candles, bkt)
        print(
            f"bucket={bkt:g} median_rows={st['row_p50']:.1f} cross3={st['cross3']}/{st['n']} ({st['cross3_pct']:.1f}%)"
        )

    trades = load_trades([p for p in SUI_PATHS if os.path.exists(p)])
    print(f"\nSUI trades={len(trades)} {iso(trades[0][0])} → {iso(trades[-1][0])}")
    # single-print punches vs previous trade
    jump3 = jump10 = 0
    prev_px = None
    for ts, px, sz, side in trades:
        if prev_px is not None:
            ticks = abs(px - prev_px) / SUI_TICK
            if ticks >= 3:
                jump3 += 1
            if ticks >= 10:
                jump10 += 1
        prev_px = px
    print(f"trade-to-trade jumps >=3 tick: {jump3}  >=10 tick: {jump10}  of {max(len(trades)-1,1)}")

    print("\n===== Day 15/16 footprint at candidate buckets (200%, no min vol) =====")
    built = {bkt: build_bars(trades, bkt) for bkt in (0.0001, 0.0002, 0.0005, 0.001, 0.002)}
    bars001 = built[0.0001]
    print(f"closed 1m={len(bars001)} {iso(bars001[0]['t0'])} → {iso(bars001[-1]['t0'])}")
    by = by_session(bars001)
    print("sessions", {k: len(v) for k, v in by.items() if k != "all"})

    for bkt, bars in built.items():
        ep = empty_and_punch(bars, bkt)
        tab = footprint_table(bars, bkt, 2.0, 0.0)
        print(
            f"bkt={bkt:g} empty_p50={ep['empty_p50']:.1f}% empty_p75={ep['empty_p75']:.1f}% "
            f"nonempty_p50={ep['nonempty_p50']:.1f} punch1={ep['punch1']} sparse<=3trades={ep['sparse3']} "
            f"imb_med={tab['med_imb']:.0f}% 3stack={tab['k3']}/{tab['n']} ({tab['p3']:.1f}%)"
        )

    # pick observation bucket later in docs; compute p25 tables for 0.0001 and 0.0005
    print("\n===== Day 16 min-volume / 200/300/400 =====")
    for bkt in (0.0001, 0.0005, 0.001):
        bars = built[bkt]
        byb = by_session(bars)
        print(f"\n# bucket {bkt:g}")
        for sess in ("all", "asia", "eu", "us", "thin"):
            subset = byb.get(sess) or []
            if len(subset) < 40:
                continue
            vols = nonempty_side_volumes(subset)
            p25 = percentile(vols, 25)
            print(f"{sess} n={len(subset)} p25={p25:.3f} SUI cells={len(vols)}")
            for ratio, name in ((2.0, "200"), (3.0, "300"), (4.0, "400")):
                tab0 = footprint_table(subset, bkt, ratio, 0.0)
                tab = footprint_table(subset, bkt, ratio, p25)
                print(
                    f"  {name}% noMin 3s={tab0['p3']:.1f}% imb={tab0['med_imb']:.0f}% | "
                    f"p25 3s={tab['p3']:.1f}% aligned={tab['pa']:.1f}% imb={tab['med_imb']:.0f}% "
                    f"chaos={tab['kc']} zeros/bar={tab['zeros_per']:.1f}"
                )

    print("\n===== Day 17 SUI script A (LEAVE_BARS=1, session p25, bar-dir On) =====")
    # monkeypatch week2 bucket
    import week2_scripts_a_g as w2

    for bkt in (0.0001, 0.0005, 0.001):
        w2.BUCKET = bkt
        bars = built[bkt]
        byb = by_session(bars)
        print(f"\n# A bucket {bkt:g}")
        for ratio, name in ((3.0, "300%"), (4.0, "400%")):
            for sess in ("all", "asia", "eu", "us"):
                subset = byb.get(sess) or []
                if len(subset) < 80:
                    continue
                p25 = percentile(nonempty_side_volumes(subset), 25)
                a = script_a(subset, ratio, p25, LEAVE_BARS)
                print_a(f"{sess} {name} bkt={bkt:g} p25={p25:.2f}", a)

    print("\n===== SOL bars for days 18–20 (week-2 sample, eyes frozen) =====")
    sol_trades = load_trades([p for p in SOL_PATHS if os.path.exists(p)])
    sol_bars = build_bars(sol_trades, SOL_BUCKET)
    print(f"SOL closed 1m={len(sol_bars)} {iso(sol_bars[0]['t0'])} → {iso(sol_bars[-1]['t0'])}")
    sol_by = by_session(sol_bars)

    print("\n===== Day 18 OI / liquidation veto =====")
    sol_oi_5m = load_oi("/tmp/sol_okx_oi_5m.jsonl")
    sol_oi_1h = load_oi("/tmp/sol_okx_oi_1h.jsonl")
    sui_oi_1h = load_oi("/tmp/sui_okx_oi_1h.jsonl")
    for name, rows in (("SOL 5m", sol_oi_5m), ("SOL 1H", sol_oi_1h), ("SUI 1H", sui_oi_1h)):
        chgs = []
        for i in range(1, len(rows)):
            if rows[i - 1]["oi"] > 0:
                chgs.append((rows[i]["oi"] - rows[i - 1]["oi"]) / rows[i - 1]["oi"])
        neg = [c for c in chgs if c < 0]
        print(
            f"{name} n={len(rows)} {iso(rows[0]['t'])} → {iso(rows[-1]['t'])} "
            f"chg_p50={100*percentile(chgs,50):.2f}% neg_p90={100*percentile(neg,90) if neg else 0:.2f}% "
            f"neg_p95={100*percentile(neg,95) if neg else 0:.2f}% min={100*min(chgs):.2f}%"
        )
        for cut in (0.01, 0.02, 0.03, 0.05):
            d = oi_drops(rows, cut)
            print(f"  drops <= -{100*cut:.0f}% : {len(d)}")
            if d:
                worst = min(d, key=lambda x: x["chg"])
                print(f"    worst {iso(worst['t'])} {100*worst['chg']:.2f}% oi {worst['prev']:.0f}→{worst['oi']:.0f}")

    sol_liq = load_liq("/tmp/sol_okx_liq.jsonl")
    sui_liq = load_liq("/tmp/sui_okx_liq.jsonl")
    for name, liq in (("SOL", sol_liq), ("SUI", sui_liq)):
        if not liq:
            print(f"{name} liq empty")
            continue
        by_min = defaultdict(float)
        for r in liq:
            by_min[r["t0"]] += r["notional"]
        vals = list(by_min.values())
        print(
            f"{name} liq events={len(liq)} minutes={len(by_min)} "
            f"{iso(min(r['ts'] for r in liq))} → {iso(max(r['ts'] for r in liq))} "
            f"notional/min p50={percentile(vals,50):.0f} p90={percentile(vals,90):.0f} "
            f"p95={percentile(vals,95):.0f} p99={percentile(vals,99):.0f} max={max(vals):.0f}"
        )

    # overlay SOL 300% aligned stacks vs liq p95 minutes and 1H OI -2%
    p25_all = {}
    for sess, subset in sol_by.items():
        p25_all[sess] = percentile(nonempty_side_volumes(subset), 25)
    liq_by_min = defaultdict(float)
    for r in sol_liq:
        liq_by_min[r["t0"]] += r["notional"]
    liq_p95 = percentile(list(liq_by_min.values()), 95) if liq_by_min else 0
    liq_hot = {t for t, v in liq_by_min.items() if v >= liq_p95 and liq_p95 > 0}
    oi_hot_hours = {d["t"] for d in oi_drops(sol_oi_1h, 0.02)}

    def hour_floor(ts):
        return ts - (ts % 3_600_000)

    armed = liq_hit = oi_hit = both = 0
    for bar in sol_bars:
        m = bar_metrics(bar, SOL_BUCKET, 3.0, p25_all[bar["session"]], 3)
        if not m["aligned3"]:
            continue
        armed += 1
        lh = bar["t0"] in liq_hot
        oh = hour_floor(bar["t0"]) in oi_hot_hours or any(
            abs(bar["t0"] - t) < 3_600_000 for t in oi_hot_hours
        )
        if lh:
            liq_hit += 1
        if oh:
            oi_hit += 1
        if lh or oh:
            both += 1
    print(
        f"SOL 300% aligned 3-stack bars={armed} in liq p95 minutes={liq_hit} "
        f"in/near 1H OI -2% hour={oi_hit} either veto={both}"
    )
    print("F: not_evaluated still (no L2). Do not hunt squeeze. Veto new entries in those windows.")

    print("\n===== Day 19 session density + funding black window (SOL, 300% p25) =====")
    for sess in ("all", "asia", "eu", "us", "thin"):
        subset = sol_by.get(sess) or []
        if len(subset) < 40:
            continue
        p25 = p25_all[sess]
        tab = footprint_table(subset, SOL_BUCKET, 3.0, p25)
        print(
            f"{sess} n={tab['n']} p25={p25:.1f} 3stack={tab['k3']} ({tab['p3']:.1f}%) "
            f"aligned={tab['ka']} ({tab['pa']:.1f}%) ~every {tab['n']/tab['k3']:.1f}m" if tab["k3"] else f"{sess} 3stack=0"
        )

    inside = [b for b in sol_bars if in_black(b["t0"])]
    outside = [b for b in sol_bars if not in_black(b["t0"])]
    for label, subset in (("black ±15m of 00/08/16 UTC", inside), ("outside black", outside)):
        if len(subset) < 20:
            print(label, "too few", len(subset))
            continue
        # use each bar's own session p25
        k3 = ka = 0
        for b in subset:
            m = bar_metrics(b, SOL_BUCKET, 3.0, p25_all[b["session"]], 3)
            k3 += int(m["has3"])
            ka += int(m["aligned3"])
        n = len(subset)
        print(
            f"{label}: n={n} 3stack={k3} ({100*k3/n:.1f}%) aligned={ka} ({100*ka/n:.1f}%) "
            f"~every {n/k3:.1f}m" if k3 else f"{label}: n={n} 3stack=0"
        )
    funds = load_funding("/tmp/sol_okx_funding.jsonl")
    print("recent SOL funding:")
    for r in funds[-8:]:
        print(f"  {iso(r['t'])} rate={r['rate']:+.6f}")

    print("\n===== Day 20 three-venue direction (SOL 1m, do not sum volume) =====")
    bn = load_binance_klines("/tmp/sol_binance_um_1m.csv") if os.path.exists("/tmp/sol_binance_um_1m.csv") else {}
    byb = load_bybit_minute_delta("/tmp/sol_bybit_trades.csv") if os.path.exists("/tmp/sol_bybit_trades.csv") else {}
    print(f"OKX bars={len(sol_bars)} Binance klines={len(bn)} Bybit minutes={len(byb)}")

    okx_delta = {b["t0"]: b["delta"] for b in sol_bars if (b["bid_vol"] + b["ask_vol"]) > 0}
    bn_delta = {t: v["delta"] for t, v in bn.items()}
    by_delta = {t: v["delta"] for t, v in byb.items()}
    count_pair(okx_delta, bn_delta, "OKX vs Binance UM (delta sign)")
    count_pair(okx_delta, by_delta, "OKX vs Bybit (delta sign)")
    count_pair(bn_delta, by_delta, "Binance vs Bybit (delta sign)")

    n3 = agree3 = d3 = missing_bn = missing_by = 0
    for b in sol_bars:
        t0 = b["t0"]
        s_okx = sign(b["delta"])
        if t0 not in bn:
            missing_bn += 1
            continue
        if t0 not in byb:
            missing_by += 1
            continue
        s_bn = sign(bn[t0]["delta"])
        s_by = sign(byb[t0]["delta"])
        if s_okx == 0 or s_bn == 0 or s_by == 0:
            continue
        n3 += 1
        if s_okx == s_bn == s_by:
            agree3 += 1
        else:
            d3 += 1
    print(
        f"3-venue nonzero overlap={n3} all-agree={agree3} ({100*agree3/n3:.1f}% of 3-way) "
        f"not_all={d3} OKX bars missing Binance={missing_bn} missing Bybit-after-BN-filter counted separately"
    )
    print("Resonance mode stays conceptually off. Do not copy Binance/Bybit price onto OKX.")
    print("\nF still not_evaluated. Live still forbidden.")


if __name__ == "__main__":
    main()
