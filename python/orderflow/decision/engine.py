"""Decision engine: hard vetoes first, then A–G, then one mutex. Shadow only."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from orderflow.config import LIVE_MODES
from orderflow.decision.snapshot import FrozenBar
from orderflow.decision.vetoes import confirmations, hard_vetoes
from orderflow.scripts.eval import EVALUATORS
from orderflow.scripts.machine import HOLD, ScriptMachine
from orderflow.scripts.unfinished import UnfinishedAuction

SCRIPTS = ("A", "B", "C", "D", "E", "F", "G")


@dataclass
class DecisionEngine:
    params: dict[str, Any]
    mode: str = "shadow"
    warmup_bars: int = 5
    cooldown_bars: int = 5
    history: list[FrozenBar] = field(default_factory=list)
    machines: dict[str, ScriptMachine] = field(default_factory=dict)
    clean_closed: int = 0

    def __post_init__(self) -> None:
        if "warmup_bars" in self.params:
            self.warmup_bars = int(self.params["warmup_bars"])
        if "cooldown_bars" in self.params:
            self.cooldown_bars = int(self.params["cooldown_bars"])
        notes = {
            "A": {"leave_bars_from_toml": True},
            "C": {"trap_bars_from_toml": True},
            "E": {"reverse_on_first_divergence": bool(self.params.get("script_e_reverse"))},
            "F": {"reason": "no_l2"},
            "G": {},
        }
        for name in SCRIPTS:
            self.machines[name] = ScriptMachine(
                name=name,
                is_entry=False if name == "G" else True,
                enabled=bool(self.params.get(f"script_{name.lower()}_enabled", True)),
                evaluation="not_evaluated" if name == "F" else "inactive",
                notes=notes.get(name, {}),
            )

    def warmup_ok(self) -> bool:
        return self.clean_closed >= self.warmup_bars

    def holder(self) -> str | None:
        for name, m in self.machines.items():
            if m.state in HOLD:
                return name
        return None

    def step(self, bar: FrozenBar) -> dict[str, Any]:
        self.history.append(bar)
        if len(self.history) > 1440:
            self.history = self.history[-1440:]
        if bar.closed and not bar.chaos and not bar.quality.get("okx_gap"):
            self.clean_closed += 1

        holder = self.holder()
        script_snaps: dict[str, dict[str, Any]] = {}
        armed: str | None = None

        for name in SCRIPTS:
            machine = self.machines[name]
            view = EVALUATORS[name](self.history, self.params)
            mutex = holder is not None and holder != name
            cooldown = machine.state == "cooldown" and machine.cooldown_left > 0
            vetoes = hard_vetoes(
                bar,
                params=self.params,
                mode=self.mode,
                warmup_ok=self.warmup_ok(),
                mutex_busy=mutex,
                cooldown=cooldown,
            )
            can_arm = (
                view.eligible
                and view.is_entry
                and machine.enabled
                and not vetoes
                and self.mode not in LIVE_MODES
            )
            snap = machine.apply(
                view,
                vetoes=vetoes,
                can_arm=can_arm,
                cooldown_bars=self.cooldown_bars,
            )
            snap["vetoes"] = vetoes
            snap["confirms"] = confirmations(bar, view.side)
            script_snaps[name] = snap
            if machine.state == "armed":
                armed = name
                holder = name

        unfinished = UnfinishedAuction().snapshot()
        unfinished["evaluation"] = script_snaps["G"]["evaluation"]

        can_open = (
            armed is not None
            and self.mode not in LIVE_MODES
            and not script_snaps[armed]["vetoes"]
        )
        intent: dict[str, Any] = {
            "kind": "none",
            "copied_price_onto_okx": False,
            "live": False,
        }
        if can_open:
            intent = {
                "kind": "shadow_signal",
                "script": armed,
                "side": script_snaps[armed].get("side"),
                "limit_from": "okx_structure",
                "copied_price_onto_okx": False,
                "live": False,
                "flatten_only": armed == "E"
                and not bool(self.params.get("script_e_reverse")),
            }

        return {
            "event": "decision",
            "mode": self.mode,
            "live_gate": "closed",
            "calibration_complete": False,
            "warmup_ok": self.warmup_ok(),
            "warmup_bars": self.clean_closed,
            "main": armed,
            "can_open": can_open,
            "intent": intent,
            "scripts": script_snaps,
            "unfinished": unfinished,
            "open_ms": bar.open_ms,
            "copied_price_onto_okx": False,
            "resonance_mode": (bar.resonance or {}).get("mode")
            or self.params.get("resonance")
            or "off",
        }


def all_fresh(params: dict[str, Any] | None = None) -> dict[str, dict]:
    eng = DecisionEngine(params=params or {}, mode="shadow")
    return {name: m.snapshot() for name, m in eng.machines.items()}
