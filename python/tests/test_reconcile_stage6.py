"""Stage 6: fixture reconcile. No API keys. Live stays gated."""

from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path

from orderflow.boot import boot_once, live_allowed
from orderflow.reconcile import WIRED, reconcile

ROOT = Path(__file__).resolve().parents[2]
PARAMS = ROOT / "params"


class TestReconcile(unittest.TestCase):
    def test_wired_and_match(self):
        self.assertTrue(WIRED)
        local = {
            "working": [{"client_id": "of-1", "symbol": "SOL"}],
            "positions": [{"symbol": "SOL", "qty": 1, "avg_px": 100, "mark": 100}],
        }
        out = reconcile(local, local)
        self.assertTrue(out["ok"])
        self.assertTrue(out["exchange_is_truth"])
        self.assertFalse(out["block_new_entries"])
        self.assertFalse(out["live"])
        self.assertFalse(out["copied_price_onto_okx"])

    def test_ghost_order_blocks_and_repairs_to_exchange(self):
        local = {"working": [{"client_id": "ghost"}], "positions": []}
        exchange = {"working": [], "positions": []}
        out = reconcile(local, exchange)
        self.assertFalse(out["ok"])
        self.assertEqual(out["ghosts"], ["ghost"])
        self.assertTrue(out["block_new_entries"])
        self.assertTrue(out["reduce_only"])
        self.assertEqual(out["repaired"], exchange)

    def test_missing_and_duplicate(self):
        local = {
            "working": [
                {"client_id": "a"},
                {"client_id": "a"},
            ]
        }
        exchange = {"working": [{"client_id": "a"}, {"client_id": "b"}]}
        out = reconcile(local, exchange)
        self.assertIn("b", out["missing"])
        self.assertIn("a", out["duplicates"])

    def test_over_limit_is_reduce_only(self):
        exchange = {
            "working": [],
            "positions": [{"symbol": "SOL", "qty": 50, "avg_px": 100, "mark": 100}],
        }
        out = reconcile({"working": [], "positions": []}, exchange, symbol_cap_notional=1000)
        self.assertTrue(out["over_limit"])
        self.assertTrue(out["reduce_only"])
        self.assertTrue(out["block_new_entries"])

    def test_cli_reconcile_no_secrets(self):
        local = ROOT / "python/tests/fixtures/local_ledger.json"
        exchange = ROOT / "python/tests/fixtures/exchange_ledger.json"
        proc = subprocess.run(
            [
                sys.executable,
                "-m",
                "orderflow",
                "--mode",
                "sim",
                "--once",
                "--config-dir",
                str(PARAMS),
                "--reconcile-local",
                str(local),
                "--reconcile-exchange",
                str(exchange),
            ],
            check=False,
            capture_output=True,
            text=True,
            cwd=str(ROOT),
            env={**dict(**{k: v for k, v in __import__("os").environ.items()}), "PYTHONPATH": str(ROOT / "python")},
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)
        lines = [json.loads(x) for x in proc.stdout.splitlines() if x.strip()]
        rec = next(x for x in lines if x.get("event") == "reconcile")
        self.assertTrue(rec["block_new_entries"])
        blob = proc.stdout.lower()
        self.assertNotIn("secret", blob)
        self.assertNotIn("apikey", blob)
        self.assertNotIn("passphrase", blob)

    def test_live_still_denied(self):
        self.assertFalse(live_allowed())
        d = boot_once("live", PARAMS)
        self.assertEqual(d["reason"], "params_not_calibrated")


if __name__ == "__main__":
    unittest.main()
