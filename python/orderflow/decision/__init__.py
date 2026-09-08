# Read frozen 1m snapshots from Rust. Write a sentence or do not arm.
# Hard vetoes run before any script can arm. Resonance off never copies prices.

from .snapshot import FrozenBar, load_journal
from .vetoes import hard_vetoes

WIRED = True
FORBIDDEN = ("vwap", "avwap", "tpo", "market_profile", "naked_poc")

__all__ = [
    "WIRED",
    "FORBIDDEN",
    "DecisionEngine",
    "FrozenBar",
    "load_journal",
    "hard_vetoes",
    "all_fresh",
]


def __getattr__(name: str):
    if name in {"DecisionEngine", "all_fresh"}:
        from .engine import DecisionEngine, all_fresh

        return {"DecisionEngine": DecisionEngine, "all_fresh": all_fresh}[name]
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
