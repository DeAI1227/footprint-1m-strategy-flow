"""Stage 8: Tokyo ops. Crash fuse, tick rebuild, funding clock. Live still gated."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from orderflow.boot import boot_once, live_allowed
from orderflow.config import load_config
from orderflow.ops import (
    REGION,
    WIRED,
    crash_tripped,
    disk_status,
    funding_black_window,
    ops_report,
)

ROOT = Path(__file__).resolve().parents[2]
PARAMS = ROOT / "params"
UNIT = ROOT / "deploy" / "tokyo" / "orderflowd.service"
BIN = ROOT / "target" / "debug" / "orderflowd"


class TestOpsStage8(unittest.TestCase):
    def test_wired_tokyo_region(self):
        self.assertTrue(WIRED)
        self.assertEqual(REGION, "ap-northeast-1")

    def test_funding_black_is_clock_not_kill_zone(self):
        hours = [0, 8, 16]
        self.assertTrue(funding_black_window(0, hours, 15))
        self.assertFalse(funding_black_window(20 * 60 * 1000, hours, 15))
        cfg = load_config(PARAMS)
        self.assertEqual(cfg.sol["funding_hours_utc"], [0, 8, 16])
        self.assertEqual(cfg.sol["funding_black_window_minutes"], 15)

    def test_crash_fuse_does_not_auto_clear(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "crash.json"
            path.write_text(
                json.dumps({"timestamps_ms": [1, 2, 3, 4, 5], "tripped": True, "tripped_at_ms": 5})
            )
            self.assertTrue(crash_tripped(path))
            cfg = load_config(PARAMS)
            self.assertFalse(cfg.runtime["ops"]["clear_crash_on_start"])
            self.assertEqual(cfg.runtime["ops"]["crash_burst"], 5)

    def test_unknown_disk_does_not_trip(self):
        self.assertEqual(disk_status(None, 1_000), "unknown")
        self.assertEqual(disk_status(10, 1_000), "below")
        self.assertEqual(disk_status(2_000, 1_000), "ok")

    def test_sol_and_sui_ticks_stay_apart(self):
        cfg = load_config(PARAMS)
        self.assertEqual(cfg.sol["tick_sz"], 0.01)
        self.assertEqual(cfg.sui["tick_sz"], 0.0001)
        self.assertNotEqual(cfg.sol["bucket"], cfg.sui["bucket"])
        report = ops_report(cfg, "shadow")
        self.assertEqual(report["sol_tick"], 0.01)
        self.assertEqual(report["sui_tick"], 0.0001)
        self.assertFalse(report["live_allowed"])
        self.assertFalse(report["live_send"])
        self.assertFalse(report["copied_price_onto_okx"])

    def test_live_still_gated(self):
        self.assertFalse(live_allowed())
        d = boot_once("live", PARAMS)
        self.assertEqual(d["reason"], "params_not_calibrated")
        cfg = load_config(PARAMS)
        self.assertFalse(cfg.runtime["calibration"]["calibration_complete"])
        self.assertEqual(cfg.sol["imbalance_rate_dale"], 3.0)
        self.assertEqual(cfg.sol["imbalance_rate_valtos"], 4.0)

    def test_cli_ops_check(self):
        proc = subprocess.run(
            [
                sys.executable,
                "-m",
                "orderflow",
                "--ops-check",
                "--config-dir",
                str(PARAMS),
            ],
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
        self.assertEqual(line["event"], "ops_check")
        self.assertTrue(line["ops_wired"])
        self.assertFalse(line["live_wired"])
        self.assertFalse(line["live_allowed"])
        lower = proc.stdout.lower()
        self.assertNotIn("secret", lower)
        self.assertNotIn("apikey", lower)
        self.assertNotIn("passphrase", lower)

    def test_systemd_unit_has_no_secrets(self):
        text = UNIT.read_text()
        lower = text.lower()
        self.assertIn("StartLimitBurst=5", text)
        self.assertIn("StartLimitIntervalSec=120", text)
        self.assertIn("Restart=on-failure", text)
        self.assertNotIn("apikey", lower)
        self.assertNotIn("secret", lower)
        self.assertNotIn("passphrase", lower)
        self.assertNotIn("private_key", lower)


@unittest.skipUnless(BIN.is_file(), "orderflowd not built")
class TestOrderflowdOps(unittest.TestCase):
    def test_ops_check_and_live_still_denied(self):
        check = subprocess.run(
            [str(BIN), "--ops-check", "--config-dir", str(PARAMS)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(check.returncode, 0, check.stderr)
        line = json.loads(check.stdout.splitlines()[0])
        self.assertEqual(line["event"], "ops_check")
        self.assertFalse(line["health"]["live_allowed"])
        live = subprocess.run(
            [str(BIN), "--mode", "live", "--once", "--config-dir", str(PARAMS)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(live.returncode, 2)
        denied = json.loads(live.stdout.splitlines()[0])
        self.assertEqual(denied["reason"], "params_not_calibrated")

    def test_tripped_fuse_exits_2_without_ops_check(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "crash.json"
            path.write_text(
                json.dumps({"timestamps_ms": [1, 2, 3, 4, 5], "tripped": True, "tripped_at_ms": 5})
            )
            proc = subprocess.run(
                [
                    str(BIN),
                    "--mode",
                    "shadow",
                    "--once",
                    "--config-dir",
                    str(PARAMS),
                    "--crash-fuse",
                    str(path),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(proc.returncode, 2)
            self.assertIn("crash_fuse_tripped", proc.stderr)
            inspect = subprocess.run(
                [
                    str(BIN),
                    "--ops-check",
                    "--config-dir",
                    str(PARAMS),
                    "--crash-fuse",
                    str(path),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(inspect.returncode, 0, inspect.stderr)
            line = json.loads(inspect.stdout.splitlines()[0])
            self.assertTrue(line["health"]["crash_tripped"])


if __name__ == "__main__":
    unittest.main()
