"""Per-script lifecycle. One machine cannot arm while another holds the mutex."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from orderflow.scripts.eval import View

LIFECYCLE = (
    "inactive",
    "watch",
    "armed",
    "executed",
    "manage",
    "exit",
    "cooldown",
)

HOLD = frozenset({"armed", "executed", "manage"})


@dataclass
class ScriptMachine:
    name: str
    is_entry: bool = True
    wired: bool = True
    enabled: bool = True
    state: str = "inactive"
    evaluation: str = "inactive"
    reason: str = ""
    sentence: str = ""
    side: str | None = None
    cooldown_left: int = 0
    notes: dict[str, Any] = field(default_factory=dict)

    def snapshot(self) -> dict[str, Any]:
        return {
            "script": self.name,
            "wired": self.wired,
            "enabled": self.enabled,
            "state": self.state,
            "is_entry": self.is_entry,
            "evaluation": self.evaluation,
            "reason": self.reason,
            "sentence": self.sentence,
            "side": self.side,
            "cooldown_left": self.cooldown_left,
            **self.notes,
        }

    def tick_cooldown(self) -> None:
        if self.state == "cooldown" and self.cooldown_left > 0:
            self.cooldown_left -= 1
            if self.cooldown_left <= 0:
                self.state = "inactive"
                self.evaluation = "inactive"

    def apply(
        self,
        view: View,
        *,
        vetoes: list[str],
        can_arm: bool,
        cooldown_bars: int,
    ) -> dict[str, Any]:
        self.tick_cooldown()
        self.evaluation = view.evaluation
        self.reason = view.reason
        self.sentence = view.sentence
        self.side = view.side
        self.is_entry = view.is_entry

        if not self.enabled:
            self.state = "inactive"
            self.evaluation = "disabled"
            return self.snapshot()

        if self.state == "cooldown":
            return self.snapshot()

        if view.evaluation in {"not_evaluated", "disabled"}:
            self.state = "inactive"
            return self.snapshot()

        if view.evaluation == "invalid" and self.state in HOLD:
            self._to_cooldown(cooldown_bars)
            return self.snapshot()

        if view.eligible and view.is_entry and can_arm and not vetoes:
            self.state = "armed"
            self.evaluation = "ok"
            return self.snapshot()

        if view.eligible and not view.is_entry:
            self.state = "watch"
            return self.snapshot()

        if view.eligible and vetoes:
            self.state = "watch"
            self.evaluation = "vetoed"
            self.reason = ",".join(vetoes)
            return self.snapshot()

        if view.evaluation in {"watch", "record", "display"}:
            if self.state in HOLD:
                self._to_cooldown(cooldown_bars)
            else:
                self.state = "watch" if view.evaluation != "record" else "inactive"
            return self.snapshot()

        if self.state in HOLD and not view.eligible:
            self._to_cooldown(cooldown_bars)
            return self.snapshot()

        if self.state not in HOLD:
            self.state = "inactive" if view.evaluation == "inactive" else "watch"
        return self.snapshot()

    def _to_cooldown(self, bars: int) -> None:
        self.state = "cooldown"
        self.cooldown_left = max(1, bars)
        self.evaluation = "cooldown"
