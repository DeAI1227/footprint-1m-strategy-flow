"""A–G predicates on frozen Rust bars. No second footprint matrix. No VWAP."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from orderflow.decision.snapshot import FrozenBar


@dataclass
class View:
    name: str
    eligible: bool
    side: str | None
    evaluation: str
    reason: str
    sentence: str
    is_entry: bool


def _fully_outside(bar: FrozenBar, zone: dict[str, Any]) -> bool:
    if zone["side"] == "buy":
        return bar.low > zone["hi"]
    return bar.high < zone["lo"]


def _touches(bar: FrozenBar, zone: dict[str, Any]) -> bool:
    return bar.low <= zone["hi"] and bar.high >= zone["lo"]


def _reject(bar: FrozenBar, zone: dict[str, Any]) -> bool:
    if zone["side"] == "buy":
        return bool(bar.footprint.get("finished_low") or bar.footprint.get("unfinished_low")) or (
            bar.close >= zone["lo"] and bar.delta >= 0
        )
    return bool(bar.footprint.get("finished_high") or bar.footprint.get("unfinished_high")) or (
        bar.close <= zone["hi"] and bar.delta <= 0
    )


def _punched(bar: FrozenBar, zone: dict[str, Any]) -> bool:
    if zone["side"] == "buy":
        return bar.close < zone["lo"]
    return bar.close > zone["hi"]


def _at_key(bar: FrozenBar, tick: float) -> bool:
    keys: list[float] = []
    if bar.poc is not None:
        keys.append(bar.poc)
    z = bar.zone()
    if z:
        keys.extend([z["lo"], z["hi"]])
    for k in ("old_edge_lo", "old_edge_hi", "swing_high", "swing_low", "range_high", "range_low"):
        v = bar.context.get(k)
        if v is not None:
            keys.append(float(v))
    stack = bar.context.get("stack_15") or {}
    for k in ("poc", "band_lo", "band_hi"):
        if stack.get(k) is not None:
            keys.append(float(stack[k]))
    return any(abs(bar.high - k) <= 2 * tick or abs(bar.low - k) <= 2 * tick for k in keys)


def eval_a(hist: list[FrozenBar], params: dict[str, Any]) -> View:
    leave_n = int(params.get("leave_bars") or 1)
    bar = hist[-1]
    # Need a prior stack, then leave, then this bar touches with reject.
    for i in range(len(hist) - 1):
        z = hist[i].zone()
        if not z:
            continue
        run = 0
        leave_j = None
        for j in range(i + 1, len(hist)):
            if _fully_outside(hist[j], z):
                run += 1
                if run >= leave_n:
                    leave_j = j
                    break
            else:
                run = 0
        if leave_j is None:
            continue
        if leave_j >= len(hist) - 1:
            return View("A", False, z["side"], "watch", "left_no_pullback", "已離開，尚未回踩", True)
        if not _touches(bar, z):
            continue
        if _punched(bar, z):
            return View("A", False, z["side"], "invalid", "punched", "回踩打穿，A 失效", True)
        if _reject(bar, z):
            return View("A", True, z["side"], "ok", "leave_retest_reject", "離開後回踩拒絕", True)
        return View("A", False, z["side"], "watch", "touch_no_reject", "回踩但無拒絕", True)
    z = bar.zone()
    if z:
        return View("A", False, z["side"], "watch", "no_leave", "當根堆疊尚未離開，不是回踩", True)
    return View("A", False, None, "inactive", "no_stack", "沒有同向堆疊區", True)


def eval_b(hist: list[FrozenBar], params: dict[str, Any]) -> View:
    tick = float(params.get("tick_sz") or params.get("bucket") or 0.01)
    bar = hist[-1]
    rng = bar.high - bar.low
    vols = [float(h.footprint.get("bid_vol") or 0) + float(h.footprint.get("ask_vol") or 0) for h in hist]
    vol = vols[-1]
    prior = vols[:-1]
    if len(prior) < 3:
        return View("B", False, None, "inactive", "short_hist", "量能視窗不足，不當攻擊", True)
    p75 = sorted(prior)[int(0.75 * (len(prior) - 1))]
    high_vol = vol >= p75 and p75 > 0
    near = _at_key(bar, tick)
    if high_vol and not near:
        return View("B", False, None, "record", "vacuum", "真空吸收只記錄，不開倉", True)
    if not (high_vol and near and rng > 0):
        return View("B", False, None, "inactive", "no_attack", "沒有關鍵位攻擊", True)
    pos = (bar.close - bar.low) / rng
    down_atk = bar.delta < 0 and pos >= 0.6
    up_atk = bar.delta > 0 and pos <= 0.4
    if not (down_atk or up_atk):
        return View("B", False, None, "watch", "no_fail", "攻擊尚未失敗", True)
    side = "buy" if down_atk else "sell"
    if bar.footprint.get("finished_low" if down_atk else "finished_high"):
        return View("B", True, side, "ok", "key_absorb", "關鍵位吸收反轉", True)
    return View("B", True, side, "ok", "failed_drive", "關鍵位推進失敗", True)


def eval_c(hist: list[FrozenBar], params: dict[str, Any]) -> View:
    bar = hist[-1]
    trap = int(params.get("trap_bars") or 3)
    rh = bar.context.get("range_high")
    rl = bar.context.get("range_low")
    if rh is None or rl is None:
        return View("C", False, None, "inactive", "no_range", "擺動區間未就緒", True)
    rh, rl = float(rh), float(rl)
    if rl <= 0 or rh <= rl:
        return View("C", False, None, "inactive", "no_range", "擺動區間未就緒", True)
    if int(bar.context.get("failed_breaks") or 0) > 0 and rl <= bar.close <= rh:
        side = "sell" if bar.delta > 0 else "buy"
        return View("C", True, side, "ok", "failed_break", "失敗突破收回", True)
    # History: a close outside then back inside within trap_bars.
    for i in range(max(0, len(hist) - trap - 1), len(hist) - 1):
        prev = hist[i]
        if prev.close > rh and bar.close <= rh:
            return View("C", True, "sell", "ok", "reclaim_high", "上破後收回", True)
        if prev.close < rl and bar.close >= rl:
            return View("C", True, "buy", "ok", "reclaim_low", "下破後收回", True)
    if bar.close > rh or bar.close < rl:
        return View("C", False, None, "watch", "outside", "收在區外，等收回", True)
    return View("C", False, None, "inactive", "inside", "仍在區間內", True)


def eval_d(hist: list[FrozenBar], _params: dict[str, Any]) -> View:
    bar = hist[-1]
    if not bar.context.get("stack_accepted"):
        if bar.context.get("fake_leave"):
            return View("D", False, None, "invalid", "fake_leave", "假離開，走 C 不走 D", True)
        return View("D", False, None, "inactive", "not_accepted", "量堆尚未被接受", True)
    lo = bar.context.get("old_edge_lo")
    hi = bar.context.get("old_edge_hi")
    if lo is None or hi is None:
        return View("D", False, None, "watch", "no_old_edge", "沒有舊沿", True)
    zone = {"side": "buy" if bar.delta >= 0 else "sell", "lo": float(lo), "hi": float(hi)}
    if not _touches(bar, zone):
        return View("D", False, zone["side"], "watch", "no_touch", "已接受，尚未回踩舊沿", True)
    if _punched(bar, zone):
        return View("D", False, zone["side"], "invalid", "punched", "回踩舊沿打穿", True)
    if _reject(bar, zone):
        return View("D", True, zone["side"], "ok", "accepted_retest", "量堆接受後回踩舊沿", True)
    return View("D", False, zone["side"], "watch", "touch_no_reject", "碰到舊沿但無拒絕", True)


def eval_e(hist: list[FrozenBar], params: dict[str, Any]) -> View:
    reverse = bool(params.get("script_e_reverse"))
    if len(hist) < 3:
        return View("E", False, None, "inactive", "short_hist", "CVD 視窗不足", True)
    bar = hist[-1]
    prev = hist[-2]
    same = (bar.delta > 0 and prev.delta > 0) or (bar.delta < 0 and prev.delta < 0)
    if not same:
        return View("E", False, None, "inactive", "no_run", "沒有同向主動連續", True)
    hh = bar.high > max(h.high for h in hist[:-1])
    ll = bar.low < min(h.low for h in hist[:-1])
    cvd_up = bar.cvd > prev.cvd
    cvd_dn = bar.cvd < prev.cvd
    div = (hh and not cvd_up) or (ll and not cvd_dn)
    if not div:
        return View("E", False, None, "watch", "no_div", "尚未 CVD 背離", True)
    side = "sell" if hh else "buy"
    if not reverse:
        return View(
            "E",
            True,
            side,
            "ok",
            "first_div_flatten",
            "第一次 CVD 背離只減倉不反手",
            True,
        )
    return View("E", True, side, "ok", "div_reverse", "背離且允許反手", True)


def eval_f(hist: list[FrozenBar], _params: dict[str, Any]) -> View:
    bar = hist[-1]
    book = bar.book
    if not book or not book.get("book_ok"):
        return View("F", False, None, "not_evaluated", "no_l2", "沒有健康 L2，腳本 F 不作評", True)
    read = book.get("read")
    if read == "yielding":
        return View("F", False, None, "veto", "yield", "讓路突破否決，不走 F", True)
    if read != "eat_through":
        return View("F", False, None, "watch", "no_eat", "不是真吃牆", True)
    wall_side = book.get("wall_side")
    # bid wall eaten by sells → breakdown (sell); ask wall eaten by buys → breakout (buy)
    if wall_side == "bid" and bar.delta < 0:
        return View("F", True, "sell", "ok", "eat_through", "真吃牆順突破", True)
    if wall_side == "ask" and bar.delta > 0:
        return View("F", True, "buy", "ok", "eat_through", "真吃牆順突破", True)
    return View("F", False, None, "watch", "delta_mismatch", "吃牆與當根 delta 不同向", True)


def eval_g(hist: list[FrozenBar], params: dict[str, Any]) -> View:
    bar = hist[-1]
    is_entry = bool(params.get("script_g_is_entry"))
    tick = float(params.get("tick_sz") or params.get("bucket") or 0.01)
    unfinished = bool(bar.footprint.get("unfinished_high") or bar.footprint.get("unfinished_low"))
    if not unfinished:
        return View("G", False, None, "inactive", "none", "沒有未完成拍賣", False)
    key = _at_key(bar, tick)
    if not key:
        return View("G", False, None, "record", "cheap", "非關鍵位未完成，只記錄", False)
    side = "sell" if bar.footprint.get("unfinished_high") else "buy"
    return View("G", True, side, "display", "key_unfinished", "關鍵位未完成，不當進場", is_entry)


EVALUATORS = {
    "A": eval_a,
    "B": eval_b,
    "C": eval_c,
    "D": eval_d,
    "E": eval_e,
    "F": eval_f,
    "G": eval_g,
}
