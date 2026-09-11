"""A–G failure screens on frozen Rust bars. Never rebuild a matrix. Never pick 300 vs 400."""

from __future__ import annotations

from collections import defaultdict
from typing import Any

from ..decision.snapshot import FrozenBar

LOOK_A = 20
LOOK_B = 8
LOOK_G = 12
LEAVE_WINDOW = 12
D_RETURN = 30
SWING_N = 5


def _fully_outside(bar: FrozenBar, zone: dict[str, Any]) -> bool:
    if zone["side"] == "buy":
        return bar.low > zone["hi"]
    return bar.high < zone["lo"]


def _touches(bar: FrozenBar, zone: dict[str, Any]) -> bool:
    return bar.low <= zone["hi"] and bar.high >= zone["lo"]


def _excess_against(bar: FrozenBar, zone: dict[str, Any]) -> bool:
    if zone["side"] == "buy":
        return bool(bar.footprint.get("finished_low"))
    return bool(bar.footprint.get("finished_high"))


def _session(bar: FrozenBar) -> str:
    return str(bar.footprint.get("session") or "unknown")


def _vol(bar: FrozenBar) -> float:
    return float(bar.footprint.get("bid_vol") or 0) + float(bar.footprint.get("ask_vol") or 0)


def _empty_a() -> dict[str, int]:
    return {
        "armed": 0,
        "no_leave": 0,
        "left": 0,
        "reject": 0,
        "punch": 0,
        "inside": 0,
        "no_return": 0,
    }


def screen_a(bars: list[FrozenBar], rate: str, leave_bars: int = 1) -> dict[str, Any]:
    overall = _empty_a()
    by: dict[str, dict[str, int]] = defaultdict(_empty_a)
    n = len(bars)
    for i, bar in enumerate(bars):
        z = bar.zone_from_rate(rate)
        if not z:
            continue
        sess = _session(bar)
        for bucket in (overall, by[sess]):
            bucket["armed"] += 1
        leave_i = None
        run = 0
        for j in range(i + 1, min(i + 1 + LEAVE_WINDOW, n)):
            if _fully_outside(bars[j], z):
                run += 1
                if run >= leave_bars:
                    leave_i = j
                    break
            else:
                run = 0
        if leave_i is None:
            for bucket in (overall, by[sess]):
                bucket["no_leave"] += 1
            continue
        for bucket in (overall, by[sess]):
            bucket["left"] += 1
        found = False
        for k in range(leave_i + 1, min(leave_i + 1 + LOOK_A, n)):
            b = bars[k]
            if not _touches(b, z):
                continue
            found = True
            if z["side"] == "buy":
                punched = b.close < z["lo"]
                rejected = _excess_against(b, z) or b.close > z["hi"]
            else:
                punched = b.close > z["hi"]
                rejected = _excess_against(b, z) or b.close < z["lo"]
            for bucket in (overall, by[sess]):
                if punched:
                    bucket["punch"] += 1
                elif rejected:
                    bucket["reject"] += 1
                else:
                    bucket["inside"] += 1
            break
        if not found:
            for bucket in (overall, by[sess]):
                bucket["no_return"] += 1
    return {"all": overall, "sessions": dict(by)}


def screen_b(bars: list[FrozenBar], rate: str, tick: float) -> dict[str, Any]:
    def empty() -> dict[str, int]:
        return {"attacks": 0, "held": 0, "second_punch": 0, "vacuum": 0}

    overall = empty()
    by: dict[str, dict[str, int]] = defaultdict(empty)
    edges: list[tuple[int, float, float]] = []
    n = len(bars)
    for i, bar in enumerate(bars):
        z = bar.zone_from_rate(rate)
        if z:
            edges.append((i, z["lo"], z["hi"]))
        edges = [e for e in edges if i - e[0] <= 30]
        vols = [_vol(h) for h in bars[max(0, i - 29) : i + 1]]
        p75 = sorted(vols)[int(0.75 * (len(vols) - 1))] if vols else 0.0
        vol = _vol(bar)
        keys: list[float] = []
        if bar.poc is not None:
            keys.append(bar.poc)
        for _, lo, hi in edges:
            keys.extend([lo, hi])
        near = any(abs(bar.high - k) <= 2 * tick or abs(bar.low - k) <= 2 * tick for k in keys)
        high_vol = vol >= p75 and p75 > 0
        sess = _session(bar)
        if high_vol and not near:
            overall["vacuum"] += 1
            by[sess]["vacuum"] += 1
            continue
        if not (high_vol and near and keys):
            continue
        rng = bar.high - bar.low
        if rng <= 0:
            continue
        pos = (bar.close - bar.low) / rng
        down_atk = bar.delta < 0 and pos >= 0.6
        up_atk = bar.delta > 0 and pos <= 0.4
        if not (down_atk or up_atk):
            continue
        overall["attacks"] += 1
        by[sess]["attacks"] += 1
        level = bar.low if down_atk else bar.high
        thru = False
        for j in range(i + 1, min(i + 1 + LOOK_B, n)):
            if down_atk and bars[j].close < level - tick:
                thru = True
                break
            if up_atk and bars[j].close > level + tick:
                thru = True
                break
        if thru:
            overall["second_punch"] += 1
            by[sess]["second_punch"] += 1
        else:
            overall["held"] += 1
            by[sess]["held"] += 1
    return {"all": overall, "sessions": dict(by)}


def _swings(bars: list[FrozenBar], n: int = SWING_N) -> tuple[set[int], set[int]]:
    highs: set[int] = set()
    lows: set[int] = set()
    for i in range(n, len(bars) - n):
        h = bars[i].high
        l = bars[i].low
        win = bars[i - n : i + n + 1]
        if h == max(b.high for b in win) and sum(1 for b in win if b.high == h) == 1:
            highs.add(i)
        if l == min(b.low for b in win) and sum(1 for b in win if b.low == l) == 1:
            lows.add(i)
    return highs, lows


def screen_c(bars: list[FrozenBar], trap_bars: int = 3) -> dict[str, Any]:
    def empty() -> dict[str, int]:
        return {"breaks": 0, "reclaim": 0, "accepted": 0}

    overall = empty()
    by: dict[str, dict[str, int]] = defaultdict(empty)
    hs, ls = _swings(bars)
    hi_px = lo_px = None
    n = len(bars)
    for i, bar in enumerate(bars):
        if i in hs:
            hi_px = bar.high
        if i in ls:
            lo_px = bar.low
        if hi_px is None or lo_px is None or hi_px <= lo_px:
            continue
        if bar.close > hi_px:
            side = "up"
        elif bar.close < lo_px:
            side = "down"
        else:
            continue
        if i > 0:
            prev = bars[i - 1]
            if side == "up" and prev.close > hi_px:
                continue
            if side == "down" and prev.close < lo_px:
                continue
        sess = _session(bar)
        overall["breaks"] += 1
        by[sess]["breaks"] += 1
        rec = False
        out_poc = 0
        for j in range(i + 1, min(i + 1 + max(trap_bars, 5), n)):
            inside = lo_px <= bars[j].close <= hi_px
            if j - i <= trap_bars and inside:
                rec = True
                break
            poc = bars[j].poc
            if poc is None:
                continue
            if (side == "up" and poc > hi_px) or (side == "down" and poc < lo_px):
                out_poc += 1
        if rec:
            overall["reclaim"] += 1
            by[sess]["reclaim"] += 1
        elif out_poc >= 3:
            overall["accepted"] += 1
            by[sess]["accepted"] += 1
    return {"all": overall, "sessions": dict(by)}


def screen_d(bars: list[FrozenBar], rate: str, accept_bars: int = 3) -> dict[str, Any]:
    def empty() -> dict[str, int]:
        return {"accepted": 0, "reject": 0, "punch": 0, "no_return": 0}

    overall = empty()
    by: dict[str, dict[str, int]] = defaultdict(empty)
    n = len(bars)
    for i, bar in enumerate(bars):
        z = bar.zone_from_rate(rate)
        if not z or i + accept_bars >= n:
            continue
        ok = True
        for j in range(i + 1, i + 1 + accept_bars):
            poc = bars[j].poc
            if poc is None:
                ok = False
                break
            if z["side"] == "buy" and poc <= z["hi"]:
                ok = False
                break
            if z["side"] == "sell" and poc >= z["lo"]:
                ok = False
                break
        if not ok:
            continue
        sess = _session(bar)
        overall["accepted"] += 1
        by[sess]["accepted"] += 1
        found = False
        start = i + 1 + accept_bars
        for k in range(start, min(start + D_RETURN, n)):
            b = bars[k]
            if not _touches(b, z):
                continue
            found = True
            punched = b.close < z["lo"] if z["side"] == "buy" else b.close > z["hi"]
            for bucket in (overall, by[sess]):
                if punched:
                    bucket["punch"] += 1
                else:
                    bucket["reject"] += 1
            break
        if not found:
            overall["no_return"] += 1
            by[sess]["no_return"] += 1
    return {"all": overall, "sessions": dict(by)}


def screen_e(bars: list[FrozenBar]) -> dict[str, Any]:
    def empty() -> dict[str, int]:
        return {"segments": 0, "first_up": 0, "later_up": 0, "first_dn": 0, "later_dn": 0}

    overall = empty()
    by: dict[str, dict[str, int]] = defaultdict(empty)
    prev_key = None
    max_px = min_px = max_cvd = min_cvd = None
    seen_up = seen_dn = False
    for bar in bars:
        day = bar.open_ms // 86_400_000
        key = (day, _session(bar))
        sess = _session(bar)
        if key != prev_key:
            overall["segments"] += 1
            by[sess]["segments"] += 1
            max_px, min_px = bar.high, bar.low
            max_cvd = min_cvd = bar.cvd
            seen_up = seen_dn = False
            prev_key = key
            continue
        assert max_px is not None and min_px is not None
        assert max_cvd is not None and min_cvd is not None
        if bar.high > max_px and bar.cvd < max_cvd:
            if not seen_up:
                overall["first_up"] += 1
                by[sess]["first_up"] += 1
                seen_up = True
            else:
                overall["later_up"] += 1
                by[sess]["later_up"] += 1
        if bar.low < min_px and bar.cvd > min_cvd:
            if not seen_dn:
                overall["first_dn"] += 1
                by[sess]["first_dn"] += 1
                seen_dn = True
            else:
                overall["later_dn"] += 1
                by[sess]["later_dn"] += 1
        max_px = max(max_px, bar.high)
        min_px = min(min_px, bar.low)
        max_cvd = max(max_cvd, bar.cvd)
        min_cvd = min(min_cvd, bar.cvd)
    return {"all": overall, "sessions": dict(by)}


def screen_f(bars: list[FrozenBar]) -> dict[str, Any]:
    """Count frozen book reads. Never invent a wall from footprint volume."""
    n = len(bars)
    reads = {
        "eat_through": 0,
        "yield": 0,
        "absorb": 0,
        "fake_wall": 0,
        "no_wall": 0,
    }
    located = {
        "eat_through": 0,
        "yield": 0,
        "absorb": 0,
        "fake_wall": 0,
        "no_wall": 0,
    }
    evaluated = 0
    book_present = 0
    wall_on_poc = 0
    wall_on_stack = 0
    located_n = 0
    for b in bars:
        book = b.book
        if not book:
            continue
        book_present += 1
        if not book.get("book_ok"):
            continue
        evaluated += 1
        raw = str(book.get("read") or "not_evaluated")
        if raw == "yielding":
            raw = "yield"
        key = raw if raw in reads else "no_wall"
        reads[key] += 1
        on_poc = bool(book.get("wall_on_poc"))
        on_stack = bool(book.get("wall_on_stack"))
        if on_poc:
            wall_on_poc += 1
        if on_stack:
            wall_on_stack += 1
        if on_poc or on_stack:
            located_n += 1
            located[key] += 1
    return {
        "all": {
            "bars": n,
            "book_present": book_present,
            "evaluated": evaluated,
            "not_evaluated": n - evaluated,
            "reason": "ok" if evaluated else "no_l2",
            "wall_on_poc": wall_on_poc,
            "wall_on_stack": wall_on_stack,
            "located": located_n,
            **reads,
            "located_eat_through": located["eat_through"],
            "located_yield": located["yield"],
            "located_absorb": located["absorb"],
            "located_fake_wall": located["fake_wall"],
            "located_no_wall": located["no_wall"],
        }
    }


def screen_g(bars: list[FrozenBar], rate: str, tick: float) -> dict[str, Any]:
    def empty() -> dict[str, int]:
        return {"key_unf": 0, "fill": 0, "extend": 0, "neither": 0, "cheap": 0}

    overall = empty()
    by: dict[str, dict[str, int]] = defaultdict(empty)
    hs, ls = _swings(bars)
    swing_px: list[tuple[int, float]] = []
    for i in hs:
        swing_px.append((i, bars[i].high))
    for i in ls:
        swing_px.append((i, bars[i].low))
    recent_z: list[tuple[int, float, float]] = []
    n = len(bars)
    for i, bar in enumerate(bars):
        z = bar.zone_from_rate(rate)
        if z:
            recent_z.append((i, z["lo"], z["hi"]))
        recent_z = [e for e in recent_z if i - e[0] <= 20]
        unf_h = bool(bar.footprint.get("unfinished_high"))
        unf_l = bool(bar.footprint.get("unfinished_low"))
        if not (unf_h or unf_l):
            continue
        keys: list[float] = []
        if bar.poc is not None:
            keys.append(bar.poc)
        for _, lo, hi in recent_z:
            keys.extend([lo, hi])
        for j, px in swing_px:
            if 0 <= i - j <= 20:
                keys.append(px)
        loc_h = unf_h and any(abs(bar.high - k) <= 2 * tick for k in keys)
        loc_l = unf_l and any(abs(bar.low - k) <= 2 * tick for k in keys)
        sess = _session(bar)
        if not (loc_h or loc_l):
            overall["cheap"] += 1
            by[sess]["cheap"] += 1
            continue
        overall["key_unf"] += 1
        by[sess]["key_unf"] += 1
        target = bar.high if loc_h else bar.low
        side_h = loc_h
        got = False
        for j in range(i + 1, min(i + 1 + LOOK_G, n)):
            b = bars[j]
            if side_h:
                if b.high > target + tick:
                    overall["extend"] += 1
                    by[sess]["extend"] += 1
                    got = True
                    break
                if b.high >= target and b.close < target:
                    overall["fill"] += 1
                    by[sess]["fill"] += 1
                    got = True
                    break
            else:
                if b.low < target - tick:
                    overall["extend"] += 1
                    by[sess]["extend"] += 1
                    got = True
                    break
                if b.low <= target and b.close > target:
                    overall["fill"] += 1
                    by[sess]["fill"] += 1
                    got = True
                    break
        if not got:
            overall["neither"] += 1
            by[sess]["neither"] += 1
    return {"all": overall, "sessions": dict(by)}


def run_screens(bars: list[FrozenBar], *, tick: float = 0.01, leave_bars: int = 1, trap_bars: int = 3) -> dict[str, Any]:
    if any((b.footprint.get("unfinished_is_entry") for b in bars)):
        raise ValueError("unfinished_is_entry must stay false")
    f = screen_f(bars)
    script_f = "computed" if f["all"]["evaluated"] else "not_evaluated"
    return {
        "bars": len(bars),
        "chosen_armed_rate": None,
        "still_open": True,
        "out_of_sample_validated": False,
        "copied_price_onto_okx": False,
        "script_g_is_entry": False,
        "script_e_reverse": False,
        "script_f": script_f,
        "dale": {
            "A": screen_a(bars, "dale", leave_bars),
            "B": screen_b(bars, "dale", tick),
            "D": screen_d(bars, "dale"),
            "G": screen_g(bars, "dale", tick),
        },
        "valtos": {
            "A": screen_a(bars, "valtos", leave_bars),
            "B": screen_b(bars, "valtos", tick),
            "D": screen_d(bars, "valtos"),
            "G": screen_g(bars, "valtos", tick),
        },
        "C": screen_c(bars, trap_bars),
        "E": screen_e(bars),
        "F": f,
        "note": "oos A–G on frozen snapshots; do not select 300 vs 400; live still gated",
    }
