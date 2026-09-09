"""Stage 7: SOL and SUI shadow in parallel. Independent param tables. No live."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from orderflow.boot import boot_once, live_allowed
from orderflow.config import load_config
from orderflow.decision.engine import DecisionEngine
from orderflow.decision.snapshot import FrozenBar

ROOT = Path(__file__).resolve().parents[2]
PARAMS = ROOT / "params"


def _bar(open_ms: int, **fp) -> FrozenBar:
    footprint = {
        "open_ms": open_ms,
        "high": 100.02,
        "low": 100.00,
        "close": 100.01,
        "delta": 1.0,
        "cvd": 1.0,
        "poc": 100.01,
        "chaos": False,
        "bid_vol": 2.0,
        "ask_vol": 3.0,
        "session": "us",
        "dale": {
            "aligned": False,
            "stacked_buy": False,
            "stacked_sell": False,
            "buy_imb_prices": [],
            "sell_imb_prices": [],
        },
        **fp,
    }
    return FrozenBar(
        bar={"open_ms": open_ms, "state": "closed"},
        footprint=footprint,
        context={},
        quality={},
    )


class TestShadowParallel(unittest.TestCase):
    def test_sol_and_sui_tables_stay_apart(self):
        cfg = load_config(PARAMS)
        self.assertEqual(cfg.sol["bucket"], 0.01)
        self.assertEqual(cfg.sui["bucket"], 0.0001)
        self.assertNotEqual(cfg.sol["okx_inst_id"], cfg.sui["okx_inst_id"])
        sol = DecisionEngine(params=dict(cfg.sol), mode="shadow")
        sui = DecisionEngine(params=dict(cfg.sui), mode="shadow")
        d_sol = sol.step(_bar(0, high=100.02, low=100.00))
        d_sui = sui.step(
            _bar(
                0,
                high=1.2346,
                low=1.2344,
                close=1.2345,
                poc=1.2345,
            )
        )
        self.assertIsNone(d_sol["main"])
        self.assertIsNone(d_sui["main"])
        self.assertFalse(d_sol["can_open"])
        self.assertFalse(d_sui["can_open"])
        self.assertNotEqual(sol.params["bucket"], sui.params["bucket"])
        # Mutex is per-symbol: SOL watch does not occupy SUI.
        self.assertIsNone(sol.holder())
        self.assertIsNone(sui.holder())

    def test_live_still_double_locked(self):
        self.assertFalse(live_allowed())
        d = boot_once("live", PARAMS)
        self.assertEqual(d["reason"], "params_not_calibrated")
        cfg = load_config(PARAMS)
        self.assertFalse(cfg.runtime.get("exec", {}).get("live_send", True))
        self.assertEqual(cfg.runtime.get("mode_default"), "shadow")

    def test_parallel_json_has_no_secrets(self):
        cfg = load_config(PARAMS)
        blob = json.dumps({"sol": cfg.sol["okx_inst_id"], "sui": cfg.sui["okx_inst_id"]})
        self.assertNotIn("secret", blob.lower())
        self.assertNotIn("apiKey", blob)
        self.assertNotIn("passphrase", blob)


if __name__ == "__main__":
    unittest.main()
