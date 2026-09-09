"""Fill-number interfaces. Observation freeze is not calibration complete.

Reads params and Rust frozen journals. Never rebuilds a footprint matrix.
Never selects 300 vs 400. Never authorizes live.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

from ..config import AppConfig, load_config
from ..decision.snapshot import FrozenBar, load_journal
from ..logfmt import json_log

WIRED = True
COMPLETE = False
FILL_ORDER = (
    "sol_bucket",
    "record_vs_armed",
    "liquidity_session",
    "sui_table",
    "out_of_sample",
)


def fill_report(cfg: AppConfig) -> dict[str, Any]:
    cal = cfg.runtime.get("calibration") or {}
    errors: list[str] = []
    if cfg.sol["bucket"] == cfg.sui["bucket"]:
        errors.append("sui_copied_sol_bucket")
    if cfg.sol["imbalance_rate_dale"] == cfg.sol["imbalance_rate_valtos"]:
        errors.append("averaged_armed_rate")
    if cfg.sui["imbalance_rate_dale"] == cfg.sui["imbalance_rate_valtos"]:
        errors.append("averaged_armed_rate")
    if cal.get("calibration_complete") and cal.get("status") == "observation_draft":
        errors.append("complete_flag_on_observation_draft")

    parallel = (
        cfg.sol.get("armed_rate_policy") == "parallel"
        and cfg.sui.get("armed_rate_policy") == "parallel"
    )
    return {
        "event": "calibrate_check",
        "wired": WIRED,
        "complete": COMPLETE,
        "observation_frozen": bool(cal.get("observation_frozen")),
        "calibration_complete": bool(cal.get("calibration_complete")),
        "out_of_sample_validated": bool(cal.get("out_of_sample_validated")),
        "live_allowed": False,
        "chosen_armed_rate": None if parallel else cfg.sol.get("armed_rate_policy"),
        "next_step": "record_vs_armed" if parallel else "out_of_sample",
        "fill_order": list(FILL_ORDER),
        "sol_bucket": cfg.sol["bucket"],
        "sui_bucket": cfg.sui["bucket"],
        "imbalance_rate_dale": cfg.sol["imbalance_rate_dale"],
        "imbalance_rate_valtos": cfg.sol["imbalance_rate_valtos"],
        "min_imbalance_volume_rule": cfg.sol["min_imbalance_volume_rule"],
        "errors": errors,
        "promote_allowed": False,
        "copied_price_onto_okx": False,
        "note": "stage 9: fill interfaces only; observation freeze is not live",
    }


def journal_stats(bars: list[FrozenBar]) -> dict[str, Any]:
    dale = valtos = record = unfinished = chaos = 0
    for bar in bars:
        fp = bar.footprint
        if fp.get("unfinished_is_entry"):
            raise ValueError("unfinished_is_entry must stay false")
        if (fp.get("dale") or {}).get("aligned"):
            dale += 1
        if (fp.get("valtos") or {}).get("aligned"):
            valtos += 1
        if (fp.get("record") or {}).get("aligned"):
            record += 1
        if fp.get("unfinished_high") or fp.get("unfinished_low"):
            unfinished += 1
        if fp.get("chaos"):
            chaos += 1
    return {
        "event": "calibrate_stats",
        "bars": len(bars),
        "dale_aligned": dale,
        "valtos_aligned": valtos,
        "record_aligned": record,
        "unfinished": unfinished,
        "chaos": chaos,
        "chosen_armed_rate": None,
        "still_open": True,
        "copied_price_onto_okx": False,
        "note": "stage 9: shadow stats only; do not select 300 vs 400; live still gated",
    }


def promote_fill(cfg: AppConfig) -> tuple[str, str]:
    cal = cfg.runtime.get("calibration") or {}
    if cal.get("observation_frozen") and not cal.get("out_of_sample_validated"):
        return (
            "params_not_calibrated",
            "參數未校準：21 日觀察稿不是樣本外驗證，禁止 live",
        )
    if cfg.sol.get("armed_rate_policy") == "parallel" or cfg.sui.get("armed_rate_policy") == "parallel":
        return (
            "armed_rate_still_parallel",
            "武裝比率仍是 300∥400 並列，尚未選定，禁止 live",
        )
    return ("exec_not_wired", "執行路徑未接線，即使旗標翻開也下不了單")


def check_line(config_dir: Path | None = None) -> str:
    cfg = load_config(config_dir)
    return json_log("info", fill_report(cfg))


def stats_line(path: Path) -> str:
    bars = load_journal(str(path))
    return json_log("info", journal_stats(bars))
