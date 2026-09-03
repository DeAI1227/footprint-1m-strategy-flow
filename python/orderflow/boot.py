"""Stage 0 boot: shadow/sim start; live paths are hard-gated."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .config import LIVE_MODES, AppConfig, load_config
from .logfmt import json_log

SHADOW_OK_MESSAGE = "shadow/sim 已啟動；live 閘門關閉（觀察稿未樣本外驗證）"


@dataclass
class LiveDenied(Exception):
    reason: str
    message: str

    def __str__(self) -> str:
        return f"{self.reason}: {self.message}"


def live_allowed() -> bool:
    """Always false in stage 0. Observation freeze is not live authorization."""
    return False


def _calibration_complete(cfg: AppConfig) -> bool:
    cal = cfg.runtime.get("calibration") or {}
    return bool(
        cal.get("calibration_complete")
        and cal.get("out_of_sample_validated")
        and cal.get("status") == "out_of_sample_validated"
        and cfg.sol.get("calibration_complete")
        and cfg.sol.get("out_of_sample_validated")
    )


def live_open_allowed(mode: str, cfg: AppConfig) -> None:
    if mode not in LIVE_MODES:
        return
    if not _calibration_complete(cfg):
        raise LiveDenied(
            "params_not_calibrated",
            "參數未校準：21 日觀察稿不是樣本外驗證，禁止 live",
        )
    cal = cfg.runtime.get("calibration") or {}
    if not cal.get("live_authorized") or not cfg.sol.get("live_enabled"):
        raise LiveDenied("live_flag_off", "live 旗標關閉，禁止開倉")
    if cfg.sol.get("armed_rate_policy") == "parallel":
        raise LiveDenied(
            "armed_rate_still_parallel",
            "武裝比率仍是 300∥400 並列，尚未選定，禁止 live",
        )
    raise LiveDenied("exec_not_wired", "執行路徑未接線，即使旗標翻開也下不了單")


def boot_decision(mode: str, cfg: AppConfig) -> dict[str, Any]:
    cal = cfg.runtime.get("calibration") or {}
    base = {
        "ok": False,
        "mode": mode,
        "event": "live_denied",
        "live_gate": "closed",
        "school": "footprint",
        "execution_venue": cfg.runtime.get("execution_venue"),
        "resonance": cfg.runtime.get("resonance"),
        "calibration_status": cal.get("status"),
        "calibration_complete": bool(cal.get("calibration_complete")),
    }
    try:
        live_open_allowed(mode, cfg)
    except LiveDenied as err:
        base["reason"] = err.reason
        base["message"] = err.message
        return base
    if mode in LIVE_MODES:
        base["reason"] = "exec_not_wired"
        base["message"] = "執行路徑未接線，即使旗標翻開也下不了單"
        return base
    base.update(
        {
            "ok": True,
            "event": "boot",
            "message": SHADOW_OK_MESSAGE,
        }
    )
    return base


def boot_once(mode: str = "shadow", config_dir: Path | None = None) -> dict[str, Any]:
    cfg = load_config(config_dir)
    return boot_decision(mode, cfg)


def boot_line(mode: str = "shadow", config_dir: Path | None = None) -> tuple[str, int]:
    decision = boot_once(mode, config_dir)
    level = "info" if decision["ok"] else "error"
    return json_log(level, decision), (0 if decision["ok"] else 2)
