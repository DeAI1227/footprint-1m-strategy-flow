"""Hard veto table. One hit blocks a new entry. Watch is still allowed."""

from __future__ import annotations

from typing import Any

from .snapshot import FrozenBar

LIVE_MODES = frozenset({"live", "live_small"})


def hard_vetoes(
    bar: FrozenBar,
    *,
    params: dict[str, Any],
    mode: str,
    warmup_ok: bool,
    mutex_busy: bool,
    cooldown: bool,
) -> list[str]:
    out: list[str] = []
    if not bar.closed:
        out.append("bar_not_closed")
    if bar.chaos:
        out.append("chaos_bar")
    q = bar.quality
    if q.get("okx_gap"):
        out.append("okx_gap")
    if q.get("okx_book_ok") is False and (bar.book or {}).get("dom_entries_allowed") is False:
        out.append("okx_book_bad")
    regime = bar.regime()
    if regime.get("liquidation_regime") == "true":
        out.append("liquidation_regime")
    if regime.get("funding_black_window"):
        out.append("funding_black_window")
    if regime.get("new_entries_blocked"):
        out.append("regime_blocks_entries")
    if not warmup_ok:
        out.append("warmup")
    if mutex_busy:
        out.append("other_script_in_position")
    if cooldown:
        out.append("cooldown")
    if mode in LIVE_MODES:
        out.append("live_denied")
    if params.get("armed_rate_policy") == "parallel" and mode in LIVE_MODES:
        out.append("armed_rate_still_parallel")
    if not params.get("language_runnable", True):
        out.append("language_not_runnable")
    res = bar.resonance or {}
    if res.get("copied_price_onto_okx"):
        out.append("copied_price_onto_okx")
    if not bar.context.get("swing_ready") and not bar.footprint:
        out.append("context_not_ready")
    return out


def confirmations(bar: FrozenBar, side: str | None) -> list[str]:
    """Bonus only. Cannot open by themselves."""
    hits: list[str] = []
    if side is None:
        return hits
    want = 1 if side == "buy" else -1
    if (1 if bar.delta > 0 else -1 if bar.delta < 0 else 0) == want:
        hits.append("okx_delta")
    res = bar.resonance or {}
    if res.get("mode") in ("k_of_n", "all") and res.get("used_for_entry"):
        hits.append("resonance")
    poc = bar.poc
    va_lo = bar.footprint.get("va_low")
    va_hi = bar.footprint.get("va_high")
    if poc is not None and va_lo is not None and va_hi is not None:
        if side == "buy" and bar.close >= float(va_lo):
            hits.append("va_side")
        if side == "sell" and bar.close <= float(va_hi):
            hits.append("va_side")
    sess = bar.footprint.get("session")
    if sess and sess != "thin":
        hits.append("not_thin")
    return hits
