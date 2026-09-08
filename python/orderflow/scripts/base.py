"""A–G machines. Lifecycle inactive → watch → armed → … → cooldown."""

from __future__ import annotations

from typing import Any

from orderflow.scripts.eval import EVALUATORS, View
from orderflow.scripts.machine import ScriptMachine


class ScriptA(ScriptMachine):
    def __init__(self) -> None:
        super().__init__(name="A", is_entry=True, notes={"leave_bars_from_toml": True})


class ScriptB(ScriptMachine):
    def __init__(self) -> None:
        super().__init__(name="B", is_entry=True)


class ScriptC(ScriptMachine):
    def __init__(self) -> None:
        super().__init__(name="C", is_entry=True, notes={"trap_bars_from_toml": True})


class ScriptD(ScriptMachine):
    def __init__(self) -> None:
        super().__init__(name="D", is_entry=True)


class ScriptE(ScriptMachine):
    def __init__(self) -> None:
        super().__init__(
            name="E",
            is_entry=True,
            notes={"reverse_on_first_divergence": False},
        )


class ScriptF(ScriptMachine):
    def __init__(self) -> None:
        super().__init__(
            name="F",
            is_entry=True,
            evaluation="not_evaluated",
            notes={"reason": "no_l2"},
        )


class ScriptG(ScriptMachine):
    def __init__(self) -> None:
        super().__init__(name="G", is_entry=False)


class UnfinishedAuction:
    """Display-only. Not an entry."""

    is_entry = False
    wired = True

    def snapshot(self) -> dict[str, Any]:
        return {
            "kind": "unfinished_auction",
            "is_entry": False,
            "wired": True,
            "evaluation": "display",
        }


def preview(name: str, hist: list[Any], params: dict[str, Any]) -> View:
    return EVALUATORS[name](hist, params)
