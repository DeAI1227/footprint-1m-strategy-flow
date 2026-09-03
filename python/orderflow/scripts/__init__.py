# Stage 0: lifecycle stubs only. State machines land in stage 5.
# inactive → watch → armed → executed → manage → exit → cooldown
# One main script in_position per symbol. E does not reverse on first divergence.
# G is not an entry. F stays not_evaluated without L2.
# Unfinished auction is display-only.

from .a import ScriptA
from .b import ScriptB
from .c import ScriptC
from .d import ScriptD
from .e import ScriptE
from .f import ScriptF
from .g import ScriptG
from .unfinished import UnfinishedAuction

__all__ = ["SCRIPTS", "LIFECYCLE", "STUBS", "UnfinishedAuction", "all_disabled"]

SCRIPTS = ("A", "B", "C", "D", "E", "F", "G")
LIFECYCLE = (
    "inactive",
    "watch",
    "armed",
    "executed",
    "manage",
    "exit",
    "cooldown",
)

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
    return {name: cls().snapshot() for name, cls in STUBS.items()}
