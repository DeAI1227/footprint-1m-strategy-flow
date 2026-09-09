"""Read a frozen closed-1m bundle from Rust journal events. Never rebuild a matrix."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


def _d(x: Any) -> dict[str, Any]:
    return x if isinstance(x, dict) else {}


@dataclass
class FrozenBar:
    """One closed minute: footprint + context + optional book/resonance."""

    bar: dict[str, Any] = field(default_factory=dict)
    footprint: dict[str, Any] = field(default_factory=dict)
    context: dict[str, Any] = field(default_factory=dict)
    book: dict[str, Any] | None = None
    resonance: dict[str, Any] | None = None
    quality: dict[str, Any] = field(default_factory=dict)

    @property
    def open_ms(self) -> int:
        if self.bar.get("open_ms") is not None:
            return int(self.bar["open_ms"])
        if self.footprint.get("open_ms") is not None:
            return int(self.footprint["open_ms"])
        if self.context.get("open_ms") is not None:
            return int(self.context["open_ms"])
        return 0

    @property
    def closed(self) -> bool:
        return self.bar.get("state", "closed") == "closed"

    @property
    def high(self) -> float:
        return float(self.footprint.get("high") or self.bar.get("high") or 0.0)

    @property
    def low(self) -> float:
        return float(self.footprint.get("low") or self.bar.get("low") or 0.0)

    @property
    def close(self) -> float:
        return float(self.footprint.get("close") or self.bar.get("close") or 0.0)

    @property
    def delta(self) -> float:
        return float(self.footprint.get("delta") or 0.0)

    @property
    def cvd(self) -> float:
        return float(self.footprint.get("cvd") or 0.0)

    @property
    def poc(self) -> float | None:
        p = self.footprint.get("poc")
        return None if p is None else float(p)

    @property
    def chaos(self) -> bool:
        return bool(self.footprint.get("chaos"))

    def dale(self) -> dict[str, Any]:
        return _d(self.footprint.get("dale"))

    def regime(self) -> dict[str, Any]:
        return _d(self.context.get("regime"))

    def zone(self) -> dict[str, Any] | None:
        dale = self.dale()
        if dale.get("aligned") and dale.get("stacked_buy") and dale.get("buy_imb_prices"):
            xs = [float(x) for x in dale["buy_imb_prices"]]
            return {"side": "buy", "lo": min(xs), "hi": max(xs)}
        if dale.get("aligned") and dale.get("stacked_sell") and dale.get("sell_imb_prices"):
            xs = [float(x) for x in dale["sell_imb_prices"]]
            return {"side": "sell", "lo": min(xs), "hi": max(xs)}
        lo = self.context.get("old_edge_lo")
        hi = self.context.get("old_edge_hi")
        if lo is not None and hi is not None:
            side = "buy" if self.delta >= 0 else "sell"
            return {"side": side, "lo": float(lo), "hi": float(hi)}
        return None


def load_journal(path: str) -> list[FrozenBar]:
    import json
    from pathlib import Path

    by_open: dict[int, FrozenBar] = {}
    order: list[int] = []
    last_open: int | None = None

    def slot(open_ms: int) -> FrozenBar:
        nonlocal last_open
        if open_ms not in by_open:
            by_open[open_ms] = FrozenBar()
            order.append(open_ms)
        last_open = open_ms
        return by_open[open_ms]

    for line in Path(path).read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        rec = json.loads(line)
        ev = rec.get("event")
        if ev == "bar_closed":
            bar = _d(rec.get("bar"))
            row = slot(int(bar.get("open_ms") or 0))
            row.bar = bar
            row.quality = _d(rec.get("quality_snapshot") or rec.get("quality"))
        elif ev == "footprint_closed":
            fp = _d(rec.get("footprint"))
            row = slot(int(fp.get("open_ms") or last_open or 0))
            row.footprint = fp
        elif ev == "context_closed":
            ctx = _d(rec.get("context"))
            row = slot(int(ctx.get("open_ms") or last_open or 0))
            row.context = ctx
        elif ev == "book_closed":
            if last_open is None:
                continue
            by_open[last_open].book = _d(rec.get("book"))
        elif ev == "resonance_closed":
            res = _d(rec.get("resonance"))
            row = slot(int(res.get("open_ms") or last_open or 0))
            row.resonance = res
    return [by_open[k] for k in order if by_open[k].footprint or by_open[k].bar]
