"""Stage 0: shadow starts; live is refused. SUI bucket must not copy SOL."""

from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PARAMS = ROOT / "params"
BIN = ROOT / "target" / "debug" / "orderflowd"

sys.path.insert(0, str(ROOT / "python"))

from orderflow.boot import boot_once, live_allowed  # noqa: E402
from orderflow.config import load_config  # noqa: E402
from orderflow.scripts import all_disabled  # noqa: E402
from orderflow.scripts.f import ScriptF  # noqa: E402
from orderflow.scripts.g import ScriptG  # noqa: E402
from orderflow.scripts.unfinished import UnfinishedAuction  # noqa: E402


class TestPythonBoot(unittest.TestCase):
    def test_shadow_once_ok(self):
        d = boot_once("shadow", PARAMS)
        self.assertTrue(d["ok"])
        self.assertEqual(d["mode"], "shadow")
        self.assertEqual(d["live_gate"], "closed")
        self.assertEqual(d["event"], "boot")
        self.assertFalse(d["calibration_complete"])
        self.assertIn("live 閘門關閉", d["message"])
        line = json.dumps(d)
        self.assertNotIn("secret", line.lower())
        self.assertNotIn("apiKey", line)

    def test_live_refused_params_not_calibrated(self):
        d = boot_once("live", PARAMS)
        self.assertFalse(d["ok"])
        self.assertEqual(d["reason"], "params_not_calibrated")
        self.assertEqual(d["event"], "live_denied")
        self.assertIn("參數未校準", d["message"])

    def test_live_small_refused(self):
        d = boot_once("live_small", PARAMS)
        self.assertEqual(d["reason"], "params_not_calibrated")

    def test_live_allowed_false(self):
        self.assertFalse(live_allowed())

    def test_cli_live_exit_2(self):
        proc = subprocess.run(
            [
                sys.executable,
                "-m",
                "orderflow",
                "--mode",
                "live",
                "--once",
                "--config-dir",
                str(PARAMS),
            ],
            check=False,
            capture_output=True,
            text=True,
            cwd=str(ROOT),
            env={**dict(**{k: v for k, v in __import__("os").environ.items()}), "PYTHONPATH": str(ROOT / "python")},
        )
        self.assertEqual(proc.returncode, 2)
        line = json.loads(proc.stdout.splitlines()[0])
        self.assertEqual(line["reason"], "params_not_calibrated")
        self.assertIn("參數未校準", line["message"])

    def test_cli_shadow_ok(self):
        proc = subprocess.run(
            [
                sys.executable,
                "-m",
                "orderflow",
                "--mode",
                "shadow",
                "--once",
                "--config-dir",
                str(PARAMS),
            ],
            check=False,
            capture_output=True,
            text=True,
            cwd=str(ROOT),
            env={**dict(**{k: v for k, v in __import__("os").environ.items()}), "PYTHONPATH": str(ROOT / "python")},
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        line = json.loads(proc.stdout.splitlines()[0])
        self.assertTrue(line["ok"])


class TestParams(unittest.TestCase):
    def test_sui_bucket_is_native_tick_not_sol(self):
        cfg = load_config(PARAMS)
        self.assertEqual(cfg.sui["bucket"], 0.0001)
        self.assertEqual(cfg.sol["bucket"], 0.01)
        self.assertNotEqual(cfg.sol["bucket"], cfg.sui["bucket"])

    def test_resonance_default_off(self):
        cfg = load_config(PARAMS)
        self.assertEqual(cfg.runtime["resonance"], "off")
        self.assertEqual(cfg.sol["resonance"], "off")
        self.assertEqual(cfg.sui["resonance"], "off")

    def test_calibration_complete_false(self):
        cfg = load_config(PARAMS)
        self.assertFalse(cfg.runtime["calibration"]["calibration_complete"])
        self.assertFalse(cfg.runtime["calibration"]["out_of_sample_validated"])
        self.assertFalse(cfg.sol["live_enabled"])
        self.assertFalse(cfg.sui["live_enabled"])
        self.assertEqual(cfg.sol["armed_rate_policy"], "parallel")
        self.assertEqual(cfg.sol["imbalance_rate_dale"], 3.0)
        self.assertEqual(cfg.sol["imbalance_rate_valtos"], 4.0)
        self.assertEqual(cfg.sol["script_f"], "not_evaluated")
        self.assertFalse(cfg.sol["unfinished_is_entry"])
        self.assertFalse(cfg.sol["script_g_is_entry"])

    def test_sui_language_runnable_but_not_live(self):
        cfg = load_config(PARAMS)
        self.assertTrue(cfg.sui["language_runnable"])
        self.assertFalse(cfg.sui.get("shadow_only"))
        self.assertFalse(cfg.sui["live_enabled"])
        self.assertEqual(cfg.sui["okx_inst_id"], "SUI-USDT-SWAP")
        self.assertEqual(cfg.sol["okx_inst_id"], "SOL-USDT-SWAP")
        self.assertEqual(cfg.sol["ct_val"], 1.0)


class TestScriptStubs(unittest.TestCase):
    def test_all_disabled(self):
        snaps = all_disabled()
        self.assertEqual(set(snaps), set("ABCDEFG"))
        for snap in snaps.values():
            self.assertFalse(snap["wired"])
            self.assertEqual(snap["state"], "inactive")
        self.assertEqual(ScriptF().evaluation, "not_evaluated")
        self.assertFalse(ScriptG().is_entry)
        self.assertFalse(UnfinishedAuction.is_entry)


@unittest.skipUnless(BIN.is_file(), "orderflowd not built")
class TestOrderflowd(unittest.TestCase):
    def _run(self, mode: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(BIN), "--mode", mode, "--config-dir", str(PARAMS), "--once"],
            check=False,
            capture_output=True,
            text=True,
        )

    def test_shadow_once_ok(self):
        proc = self._run("shadow")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        line = json.loads(proc.stdout.splitlines()[0])
        self.assertTrue(line["ok"])
        self.assertEqual(line["mode"], "shadow")
        self.assertEqual(line["live_gate"], "closed")

    def test_live_refused(self):
        proc = self._run("live")
        self.assertEqual(proc.returncode, 2)
        line = json.loads(proc.stdout.splitlines()[0])
        self.assertEqual(line["reason"], "params_not_calibrated")
        self.assertIn("參數未校準", line["message"])


class TestNativeOptional(unittest.TestCase):
    def test_pyo3_if_present(self):
        try:
            import orderflow_native
        except ImportError:
            self.skipTest("orderflow_native not built")
        self.assertFalse(orderflow_native.live_allowed())
        snap = orderflow_native.FrozenBar1mSnapshot()
        self.assertFalse(snap.wired())


if __name__ == "__main__":
    unittest.main()
