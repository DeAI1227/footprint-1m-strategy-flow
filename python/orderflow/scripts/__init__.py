# inactive → watch → armed → executed → manage → exit → cooldown
# One main script armed per symbol. E does not reverse on first divergence.
# G is not an entry. F stays not_evaluated without a healthy L2.
# Unfinished auction is display-only.

from .a import ScriptA
from .b import ScriptB
from .c import ScriptC
from .d import ScriptD
from .e import ScriptE
from .f import ScriptF
from .g import ScriptG
from .machine import HOLD, LIFECYCLE, ScriptMachine
from .unfinished import UnfinishedAuction

__all__ = [
    "SCRIPTS",
    "LIFECYCLE",
    "HOLD",
    "STUBS",
    "UnfinishedAuction",
    "all_disabled",
    "ScriptMachine",
]

SCRIPTS = ("A", "B", "C", "D", "E", "F", "G")

STUBS = {
    "A": ScriptA,
    "B": ScriptB,
    "C": ScriptC,
    "D": ScriptD,
    "E": ScriptE,
    "F": ScriptF,
    "G": ScriptG,
}


def all_disabled() -> dict[str, dict]:
    """Fresh machines: wired, inactive. F stays not_evaluated until a book is present."""
    return {name: cls().snapshot() for name, cls in STUBS.items()}
