"""Shared disabled state-machine stub. Not wired until stage 5."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class ScriptStub:
    name: str
    is_entry: bool = True
    wired: bool = False
    enabled: bool = False
    state: str = "inactive"
    evaluation: str = "disabled"
    notes: dict[str, Any] = field(default_factory=dict)

    def step(self, _snapshot: Any = None) -> dict[str, Any]:
        return self.snapshot()

    def snapshot(self) -> dict[str, Any]:
        return {
            "script": self.name,
            "wired": self.wired,
            "enabled": self.enabled,
            "state": self.state,
            "is_entry": self.is_entry,
            "evaluation": self.evaluation,
            **self.notes,
        }


class ScriptA(ScriptStub):
    def __init__(self) -> None:
        super().__init__(name="A", is_entry=True, notes={"leave_bars_from_toml": True})


class ScriptB(ScriptStub):
    def __init__(self) -> None:
        super().__init__(name="B", is_entry=True)


class ScriptC(ScriptStub):
    def __init__(self) -> None:
        super().__init__(name="C", is_entry=True, notes={"trap_bars_from_toml": True})


class ScriptD(ScriptStub):
    def __init__(self) -> None:
        super().__init__(name="D", is_entry=True)


class ScriptE(ScriptStub):
    def __init__(self) -> None:
        super().__init__(
            name="E",
            is_entry=True,
            notes={"reverse_on_first_divergence": False},
        )


class ScriptF(ScriptStub):
    def __init__(self) -> None:
        super().__init__(
            name="F",
            is_entry=True,
            evaluation="not_evaluated",
            notes={"reason": "no_l2"},
        )


class ScriptG(ScriptStub):
    def __init__(self) -> None:
        super().__init__(name="G", is_entry=False)


class UnfinishedAuction:
    """Display-only. Not an entry."""

    is_entry = False
    wired = False

    def snapshot(self) -> dict[str, Any]:
        return {"kind": "unfinished_auction", "is_entry": False, "wired": False}
