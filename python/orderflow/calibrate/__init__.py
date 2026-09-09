"""Fill-number interfaces. Observation freeze is not calibration complete.

Reads params and Rust frozen journals. Never rebuilds a footprint matrix.
Never selects 300 vs 400. Never authorizes live.
"""

from __future__ import annotations

import math
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


def _stack3(slice: dict[str, Any] | None) -> bool:
    d = slice or {}
    return int(d.get("buy_stack") or 0) >= 3 or int(d.get("sell_stack") or 0) >= 3


def _percentile(xs: list[float], p: float) -> float | None:
    if not xs:
        return None
    ys = sorted(xs)
    if p <= 0:
        return ys[0]
    if p >= 100:
        return ys[-1]
    k = (len(ys) - 1) * (p / 100.0)
    f = math.floor(k)
    c = math.ceil(k)
    if f == c:
        return ys[int(k)]
    return ys[f] * (c - k) + ys[c] * (k - f)


def _side_vols(fp: dict[str, Any]) -> list[float]:
    out: list[float] = []
    for cell in fp.get("cells") or []:
        if not isinstance(cell, dict):
            continue
        bid = float(cell.get("bid") or 0)
        ask = float(cell.get("ask") or 0)
        if bid > 0:
            out.append(bid)
        if ask > 0:
            out.append(ask)
    return out


def _empty_session(name: str) -> dict[str, Any]:
    return {
        "session": name,
        "bars": 0,
        "dale_aligned": 0,
        "valtos_aligned": 0,
        "record_aligned": 0,
        "dale_stack3": 0,
        "valtos_stack3": 0,
        "record_stack3": 0,
        "unfinished": 0,
        "chaos": 0,
        "nonempty_side_p25": None,
        "nonempty_side_cells": 0,
        "_vols": [],
    }


def journal_stats(bars: list[FrozenBar]) -> dict[str, Any]:
    dale = valtos = record = unfinished = chaos = 0
    dale_s = valtos_s = record_s = 0
    sessions: dict[str, dict[str, Any]] = {}
    vols_all: list[float] = []
    for bar in bars:
        fp = bar.footprint
        if fp.get("unfinished_is_entry"):
            raise ValueError("unfinished_is_entry must stay false")
        sess = str(fp.get("session") or "unknown")
        row = sessions.setdefault(sess, _empty_session(sess))
        row["bars"] += 1
        if (fp.get("dale") or {}).get("aligned"):
            dale += 1
            row["dale_aligned"] += 1
        if (fp.get("valtos") or {}).get("aligned"):
            valtos += 1
            row["valtos_aligned"] += 1
        if (fp.get("record") or {}).get("aligned"):
            record += 1
            row["record_aligned"] += 1
        if _stack3(fp.get("dale")):
            dale_s += 1
            row["dale_stack3"] += 1
        if _stack3(fp.get("valtos")):
            valtos_s += 1
            row["valtos_stack3"] += 1
        if _stack3(fp.get("record")):
            record_s += 1
            row["record_stack3"] += 1
        if fp.get("unfinished_high") or fp.get("unfinished_low"):
            unfinished += 1
            row["unfinished"] += 1
        if fp.get("chaos"):
            chaos += 1
            row["chaos"] += 1
        vols = _side_vols(fp)
        vols_all.extend(vols)
        row["_vols"].extend(vols)
    sess_out = []
    for name in sorted(sessions):
        row = sessions[name]
        vols = row.pop("_vols")
        row["nonempty_side_cells"] = len(vols)
        row["nonempty_side_p25"] = _percentile(vols, 25.0)
        sess_out.append(row)
    return {
        "event": "calibrate_stats",
        "bars": len(bars),
        "dale_aligned": dale,
        "valtos_aligned": valtos,
        "record_aligned": record,
        "dale_stack3": dale_s,
        "valtos_stack3": valtos_s,
        "record_stack3": record_s,
        "unfinished": unfinished,
        "chaos": chaos,
        "sessions": sess_out,
        "nonempty_side_p25": _percentile(vols_all, 25.0),
        "chosen_armed_rate": None,
        "still_open": True,
        "out_of_sample_validated": False,
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
