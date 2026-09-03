"""Load params/*.toml. Numbers come from toml only. No secrets."""

from __future__ import annotations

import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHOOL = "footprint"
FORBIDDEN_SCHOOLS = ("market_profile", "tpo", "vwap", "avwap", "naked_poc", "smc", "ict")
LIVE_MODES = frozenset({"live", "live_small"})
MODES = ("shadow", "sim", "live_small", "live")


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def default_config_dir() -> Path:
    return repo_root() / "params"


def _load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as f:
        return tomllib.load(f)


@dataclass(frozen=True)
class AppConfig:
    runtime: dict[str, Any]
    sol: dict[str, Any]
    sui: dict[str, Any]
    config_dir: Path

    def require_footprint_school(self) -> None:
        school = self.runtime.get("school")
        if school != SCHOOL:
            raise ValueError(f"runtime.school must be {SCHOOL!r}, got {school!r}")
        if self.runtime.get("execution_venue") != "okx":
            raise ValueError("execution_venue must be okx")
        if self.sol["bucket"] == self.sui["bucket"]:
            raise ValueError("SUI bucket must not copy SOL bucket")
        if self.sol["imbalance_rate_dale"] == self.sol["imbalance_rate_valtos"]:
            raise ValueError("do not collapse Dale 300% and Valtos 400% into one number")


def load_config(config_dir: Path | None = None) -> AppConfig:
    config_dir = Path(config_dir) if config_dir is not None else default_config_dir()
    cfg = AppConfig(
        runtime=_load_toml(config_dir / "runtime.toml"),
        sol=_load_toml(config_dir / "sol.toml"),
        sui=_load_toml(config_dir / "sui.toml"),
        config_dir=config_dir,
    )
    cfg.require_footprint_school()
    return cfg
