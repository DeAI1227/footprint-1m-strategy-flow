"""Compare Rust local ledger to an exchange snapshot. Exchange is truth.

No REST in stage 6 — the exchange side is a fixture so tests need no API keys.
"""

from __future__ import annotations

from collections import Counter
from typing import Any


def _orders(snap: dict[str, Any]) -> list[dict[str, Any]]:
    return list(snap.get("working") or snap.get("orders") or [])


def _positions(snap: dict[str, Any]) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for p in snap.get("positions") or []:
        out[str(p.get("symbol"))] = p
    return out


def _ids(orders: list[dict[str, Any]]) -> list[str]:
    return [str(o.get("client_id")) for o in orders if o.get("client_id")]


def reconcile(
    local: dict[str, Any],
    exchange: dict[str, Any],
    *,
    symbol_cap_notional: float = 1000.0,
    account_cap_notional: float = 2000.0,
) -> dict[str, Any]:
    loc_orders = _orders(local)
    ex_orders = _orders(exchange)
    loc_ids = _ids(loc_orders)
    ex_ids = set(_ids(ex_orders))
    loc_set = set(loc_ids)

    ghosts = sorted(loc_set - ex_ids)
    missing = sorted(ex_ids - loc_set)
    counts = Counter(loc_ids)
    duplicates = sorted([k for k, n in counts.items() if n > 1])

    mismatch = bool(ghosts or missing or duplicates)

    over_limit = False
    account_notional = 0.0
    for p in _positions(exchange).values():
        qty = abs(float(p.get("qty") or 0))
        mark = float(p.get("mark") or p.get("avg_px") or 0)
        notion = qty * mark
        account_notional += notion
        if notion > symbol_cap_notional:
            over_limit = True
    if account_notional > account_cap_notional:
        over_limit = True

    block_new = mismatch or over_limit
    reduce_only = over_limit or mismatch

    return {
        "event": "reconcile",
        "ok": not mismatch and not over_limit,
        "exchange_is_truth": True,
        "block_new_entries": block_new,
        "reduce_only": reduce_only,
        "ghosts": ghosts,
        "missing": missing,
        "duplicates": duplicates,
        "over_limit": over_limit,
        "repaired": exchange,
        "copied_price_onto_okx": False,
        "live": False,
        "note": "stage 6: fixture reconcile; no API keys; exchange is truth",
    }
