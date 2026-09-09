"""Stage 9: fill-number interfaces. Do not complete calibration. Live still gated."""

from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path

from orderflow.boot import boot_once, live_allowed
from orderflow.calibrate import (
    COMPLETE,
    FILL_ORDER,
    WIRED,
    fill_report,
    journal_stats,
    promote_fill,
)
from orderflow.config import load_config
from orderflow.decision.snapshot import FrozenBar

ROOT = Path(__file__).resolve().parents[2]
PARAMS = ROOT / "params"
BIN = ROOT / "target" / "debug" / "orderflowd"
FIXTURE = ROOT / "crates" / "orderflow-calibrate" / "tests" / "fixtures" / "shadow_stats.jsonl"


def _bar(open_ms: int, dale: bool, valtos: bool, unfinished: bool = False) -> FrozenBar:
    return FrozenBar(
        bar={"open_ms": open_ms, "state": "closed"},
        footprint={
            "open_ms": open_ms,
            "dale": {"aligned": dale, "rate": 3.0},
            "valtos": {"aligned": valtos, "rate": 4.0},
            "record": {"aligned": True, "rate": 2.0},
            "unfinished_high": unfinished,
            "unfinished_low": False,
            "unfinished_is_entry": False,
            "chaos": False,
        },
    )


class TestCalibrateStage9(unittest.TestCase):
    def test_wired_but_not_complete(self):
        self.assertTrue(WIRED)
        self.assertFalse(COMPLETE)
        self.assertEqual(FILL_ORDER[0], "sol_bucket")
        self.assertEqual(FILL_ORDER[-1], "out_of_sample")

    def test_report_keeps_300_and_400_open(self):
        cfg = load_config(PARAMS)
        r = fill_report(cfg)
        self.assertTrue(r["observation_frozen"])
        self.assertFalse(r["calibration_complete"])
        self.assertFalse(r["out_of_sample_validated"])
        self.assertFalse(r["live_allowed"])
        self.assertFalse(r["promote_allowed"])
        self.assertIsNone(r["chosen_armed_rate"])
        self.assertEqual(r["next_step"], "record_vs_armed")
        self.assertEqual(r["sol_bucket"], 0.01)
        self.assertEqual(r["sui_bucket"], 0.0001)
        self.assertEqual(r["imbalance_rate_dale"], 3.0)
        self.assertEqual(r["imbalance_rate_valtos"], 4.0)
        self.assertEqual(r["errors"], [])
        self.assertFalse(live_allowed())

    def test_promote_refuses_observation_freeze(self):
        cfg = load_config(PARAMS)
        reason, message = promote_fill(cfg)
        self.assertEqual(reason, "params_not_calibrated")
        self.assertIn("參數未校準", message)
        d = boot_once("live", PARAMS)
        self.assertEqual(d["reason"], "params_not_calibrated")

    def test_stats_do_not_choose_a_rate(self):
        stats = journal_stats([_bar(0, True, False), _bar(60_000, False, True, unfinished=True)])
        self.assertEqual(stats["bars"], 2)
        self.assertEqual(stats["dale_aligned"], 1)
        self.assertEqual(stats["valtos_aligned"], 1)
        self.assertEqual(stats["unfinished"], 1)
        self.assertIsNone(stats["chosen_armed_rate"])
        self.assertTrue(stats["still_open"])

    def test_cli_calibrate_check(self):
        proc = subprocess.run(
            [sys.executable, "-m", "orderflow", "--calibrate-check", "--config-dir", str(PARAMS)],
            check=False,
            capture_output=True,
            text=True,
            cwd=str(ROOT),
            env={
                **{k: v for k, v in __import__("os").environ.items()},
                "PYTHONPATH": str(ROOT / "python"),
            },
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        line = json.loads(proc.stdout.splitlines()[0])
        self.assertEqual(line["event"], "calibrate_check")
        self.assertFalse(line["complete"])
        self.assertFalse(line["promote_allowed"])
        lower = proc.stdout.lower()
        self.assertNotIn("apikey", lower)
        self.assertNotIn("passphrase", lower)

    def test_cli_promote_live_exits_2(self):
        proc = subprocess.run(
            [sys.executable, "-m", "orderflow", "--promote-live", "--config-dir", str(PARAMS)],
            check=False,
            capture_output=True,
            text=True,
            cwd=str(ROOT),
            env={
                **{k: v for k, v in __import__("os").environ.items()},
                "PYTHONPATH": str(ROOT / "python"),
            },
        )
        self.assertEqual(proc.returncode, 2)
        line = json.loads(proc.stdout.splitlines()[0])
        self.assertEqual(line["reason"], "params_not_calibrated")


@unittest.skipUnless(BIN.is_file(), "orderflowd not built")
class TestOrderflowdCalibrate(unittest.TestCase):
    def test_check_journal_and_promote(self):
        check = subprocess.run(
            [str(BIN), "--calibrate-check", "--config-dir", str(PARAMS)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(check.returncode, 0, check.stderr)
        line = json.loads(check.stdout.splitlines()[0])
        self.assertEqual(line["fill"]["event"], "calibrate_check")
        self.assertFalse(line["fill"]["complete"])
        self.assertIsNone(line["fill"]["chosen_armed_rate"])

        stats = subprocess.run(
            [
                str(BIN),
                "--calibrate-journal",
                str(FIXTURE),
                "--config-dir",
                str(PARAMS),
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(stats.returncode, 0, stats.stderr)
        sline = json.loads(stats.stdout.splitlines()[0])
        self.assertEqual(sline["stats"]["bars"], 2)
        self.assertIsNone(sline["stats"]["chosen_armed_rate"])

        promo = subprocess.run(
            [str(BIN), "--promote-live", "--config-dir", str(PARAMS)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(promo.returncode, 2)
        denied = json.loads(promo.stderr.splitlines()[0])
        self.assertEqual(denied["reason"], "params_not_calibrated")


if __name__ == "__main__":
    unittest.main()
