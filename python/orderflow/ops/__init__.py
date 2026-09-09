"""Tokyo ops: health, crash fuse inspect, funding black window as clock.

No secrets. No HTTP. Live stays gated by params.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from ..config import AppConfig, load_config
from ..logfmt import json_log

WIRED = True
REGION = "ap-northeast-1"


def funding_black_window(open_ms: int, hours: list[int], black_min: int) -> bool:
    """UTC 00/08/16 ± N minutes. Clock, not a kill-zone. Same math as Rust."""
    minute = (open_ms // 60_000) % (24 * 60)
    black = int(black_min)
    for h in hours:
        center = int(h) * 60
        d = abs(minute - center)
        if d > 12 * 60:
            d = 24 * 60 - d
        if d <= black:
            return True
    return False


def crash_tripped(path: Path | None) -> bool:
    if path is None or not path.exists():
        return False
    data = json.loads(path.read_text())
    return bool(data.get("tripped"))


def disk_status(free_bytes: int | None, min_free: int) -> str:
    if free_bytes is None:
        return "unknown"
    if free_bytes < min_free:
        return "below"
    return "ok"


def ops_report(
    cfg: AppConfig,
    mode: str = "shadow",
    *,
    fuse_path: Path | None = None,
    free_bytes: int | None = None,
    now_ms: int = 0,
) -> dict[str, Any]:
    ops = cfg.runtime.get("ops") or {}
    exec_cfg = cfg.runtime.get("exec") or {}
    cal = cfg.runtime.get("calibration") or {}
    hours = list(cfg.sol.get("funding_hours_utc") or [0, 8, 16])
    black_min = int(cfg.sol.get("funding_black_window_minutes") or 15)
    min_free = int(ops.get("disk_min_free_bytes") or 1_073_741_824)
    return {
        "event": "ops_check",
        "ops_wired": True,
        "live_wired": False,
        "copied_price_onto_okx": False,
        "region": REGION,
        "mode": mode,
        "calibration_complete": bool(cal.get("calibration_complete")),
        "live_send": bool(exec_cfg.get("live_send")),
        "crash_tripped": crash_tripped(fuse_path),
        "disk": disk_status(free_bytes, min_free),
        "funding_black_now": funding_black_window(now_ms, hours, black_min),
        "live_allowed": False,
        "sol_tick": cfg.sol["tick_sz"],
        "sui_tick": cfg.sui["tick_sz"],
        "note": "stage 8: Tokyo ops health; funding window is clock; live still gated",
    }


def ops_line(
    mode: str = "shadow",
    config_dir: Path | None = None,
    fuse_path: Path | None = None,
    free_bytes: int | None = None,
    now_ms: int = 0,
) -> str:
    cfg = load_config(config_dir)
    return json_log("info", ops_report(cfg, mode, fuse_path=fuse_path, free_bytes=free_bytes, now_ms=now_ms))
