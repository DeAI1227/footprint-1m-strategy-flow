"""Stage 5: A–G state machines, hard vetoes, mutex, cooldown. No live orders."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from orderflow.decision.engine import DecisionEngine
from orderflow.decision.snapshot import FrozenBar, load_journal
from orderflow.scripts import LIFECYCLE, all_disabled
from orderflow.scripts.f import ScriptF
from orderflow.scripts.g import ScriptG
from orderflow.scripts.unfinished import UnfinishedAuction


def _dale_buy(lo=100.00, hi=100.02):
    return {
        "aligned": True,
        "stacked_buy": True,
        "stacked_sell": False,
        "buy_imb_prices": [lo, 100.01, hi],
        "sell_imb_prices": [],
    }


def make(
    open_ms: int,
    *,
    high=100.02,
    low=100.00,
    close=100.01,
    delta=1.0,
    cvd=1.0,
    poc=100.01,
    chaos=False,
    bid_vol=2.0,
    ask_vol=8.0,
    dale=None,
    finished_low=False,
    finished_high=False,
    unfinished_low=False,
    unfinished_high=False,
    context=None,
    book=None,
    quality=None,
    resonance=None,
    session="us",
) -> FrozenBar:
    fp = {
        "open_ms": open_ms,
        "high": high,
        "low": low,
        "close": close,
        "delta": delta,
        "cvd": cvd,
        "poc": poc,
        "chaos": chaos,
        "bid_vol": bid_vol,
        "ask_vol": ask_vol,
        "session": session,
        "finished_low": finished_low,
        "finished_high": finished_high,
        "unfinished_low": unfinished_low,
        "unfinished_high": unfinished_high,
        "unfinished_is_entry": False,
        "va_low": 100.00,
        "va_high": 100.02,
        "dale": dale
        or {
            "aligned": False,
            "stacked_buy": False,
            "stacked_sell": False,
            "buy_imb_prices": [],
            "sell_imb_prices": [],
        },
    }
    return FrozenBar(
        bar={"open_ms": open_ms, "state": "closed", "high": high, "low": low, "close": close},
        footprint=fp,
        context=context or {},
        book=book,
        quality=quality or {},
        resonance=resonance,
    )


def engine(**extra) -> DecisionEngine:
    params = {
        "leave_bars": 1,
        "trap_bars": 3,
        "accept_bars": 3,
        "tick_sz": 0.01,
        "bucket": 0.01,
        "warmup_bars": 0,
        "cooldown_bars": 2,
        "script_e_reverse": False,
        "script_g_is_entry": False,
        "language_runnable": True,
        "armed_rate_policy": "parallel",
        "resonance": "off",
    }
    params.update(extra)
    return DecisionEngine(params=params, mode="shadow")


class TestLifecycleAndGates(unittest.TestCase):
    def test_fresh_machines_wired_inactive(self):
        snaps = all_disabled()
        self.assertEqual(set(snaps), set("ABCDEFG"))
        for name, snap in snaps.items():
            self.assertTrue(snap["wired"], name)
            self.assertEqual(snap["state"], "inactive")
        self.assertEqual(ScriptF().evaluation, "not_evaluated")
        self.assertFalse(ScriptG().is_entry)
        self.assertFalse(UnfinishedAuction.is_entry)
        self.assertTrue(UnfinishedAuction.wired)
        self.assertEqual(LIFECYCLE[0], "inactive")
        self.assertEqual(LIFECYCLE[-1], "cooldown")

    def test_a_chase_without_leave_does_not_arm(self):
        eng = engine()
        d = eng.step(make(0, dale=_dale_buy(), close=100.01, low=100.00, high=100.03))
        self.assertIsNone(d["main"])
        self.assertEqual(d["scripts"]["A"]["reason"], "no_leave")
        self.assertFalse(d["can_open"])

    def test_a_leave_retest_reject_arms_in_shadow(self):
        eng = engine()
        eng.step(make(0, dale=_dale_buy(), close=100.01, low=100.00, high=100.02))
        eng.step(make(60_000, high=100.10, low=100.04, close=100.08, dale=None))
        d = eng.step(
            make(
                120_000,
                high=100.03,
                low=100.00,
                close=100.02,
                delta=1.0,
                finished_low=True,
            )
        )
        self.assertEqual(d["main"], "A")
        self.assertEqual(d["scripts"]["A"]["state"], "armed")
        self.assertTrue(d["can_open"])
        self.assertEqual(d["intent"]["kind"], "shadow_signal")
        self.assertFalse(d["intent"]["copied_price_onto_okx"])
        self.assertFalse(d["intent"]["live"])

    def test_mutex_blocks_second_script(self):
        eng = engine()
        eng.machines["A"].state = "armed"
        d = eng.step(
            make(
                0,
                high=100.10,
                low=100.00,
                close=100.02,
                delta=-4,
                bid_vol=20,
                ask_vol=1,
                poc=100.00,
                finished_low=True,
                context={"swing_high": 100.10, "swing_low": 100.00},
            )
        )
        self.assertIn("other_script_in_position", d["scripts"]["B"]["vetoes"])
        self.assertNotEqual(d["scripts"]["B"]["state"], "armed")

    def test_cooldown_blocks_rearm(self):
        eng = engine()
        eng.step(make(0, dale=_dale_buy()))
        eng.step(make(60_000, high=100.10, low=100.04, close=100.08))
        d1 = eng.step(
            make(120_000, high=100.03, low=100.00, close=100.02, finished_low=True)
        )
        self.assertEqual(d1["main"], "A")
        d2 = eng.step(make(180_000, high=100.20, low=100.15, close=100.18))
        self.assertEqual(d2["scripts"]["A"]["state"], "cooldown")
        d3 = eng.step(
            make(240_000, high=100.03, low=100.00, close=100.02, finished_low=True)
        )
        self.assertEqual(d3["scripts"]["A"]["state"], "cooldown")
        self.assertNotEqual(d3["main"], "A")

    def test_warmup_blocks_arm(self):
        eng = engine(warmup_bars=5)
        eng.step(make(0, dale=_dale_buy()))
        eng.step(make(60_000, high=100.10, low=100.04, close=100.08))
        d = eng.step(
            make(120_000, high=100.03, low=100.00, close=100.02, finished_low=True)
        )
        self.assertIn("warmup", d["scripts"]["A"]["vetoes"])
        self.assertIsNone(d["main"])

    def test_liquidation_and_funding_veto(self):
        eng = engine()
        d = eng.step(
            make(
                0,
                dale=_dale_buy(),
                context={"regime": {"liquidation_regime": "true", "funding_black_window": False}},
            )
        )
        self.assertIn("liquidation_regime", d["scripts"]["A"]["vetoes"])
        d2 = eng.step(
            make(
                60_000,
                context={"regime": {"liquidation_regime": "false", "funding_black_window": True}},
            )
        )
        self.assertIn("funding_black_window", d2["scripts"]["A"]["vetoes"])

    def test_chaos_veto(self):
        eng = engine()
        d = eng.step(make(0, chaos=True, dale=_dale_buy()))
        self.assertIn("chaos_bar", d["scripts"]["A"]["vetoes"])

    def test_live_never_opens(self):
        eng = engine()
        eng.mode = "live"
        eng.step(make(0, dale=_dale_buy()))
        eng.step(make(60_000, high=100.10, low=100.04, close=100.08))
        d = eng.step(
            make(120_000, high=100.03, low=100.00, close=100.02, finished_low=True)
        )
        self.assertFalse(d["can_open"])
        self.assertEqual(d["intent"]["kind"], "none")
        self.assertIn("live_denied", d["scripts"]["A"]["vetoes"])


class TestScriptsBtoG(unittest.TestCase):
    def test_b_vacuum_is_record_only(self):
        eng = engine()
        quiet = dict(high=100.02, low=100.00, close=100.01, bid_vol=1.0, ask_vol=1.0, poc=100.01)
        eng.step(make(0, **quiet))
        eng.step(make(60_000, **quiet))
        eng.step(make(120_000, **quiet))
        d = eng.step(
            make(
                180_000,
                high=101.00,
                low=99.00,
                close=99.20,
                delta=-5,
                bid_vol=50,
                ask_vol=1,
                poc=99.50,
                context={},
            )
        )
        self.assertEqual(d["scripts"]["B"]["reason"], "vacuum")
        self.assertIsNone(d["main"])

    def test_c_failed_break_reclaim(self):
        eng = engine()
        ctx = {"range_high": 100.10, "range_low": 99.90, "failed_breaks": 1}
        d = eng.step(make(0, close=100.00, high=100.05, low=99.95, context=ctx, delta=-1))
        self.assertEqual(d["scripts"]["C"]["state"], "armed")
        self.assertEqual(d["main"], "C")

    def test_d_requires_accept_not_fake_leave(self):
        eng = engine()
        d = eng.step(make(0, context={"fake_leave": True, "stack_accepted": False}))
        self.assertEqual(d["scripts"]["D"]["reason"], "fake_leave")
        d2 = eng.step(
            make(
                60_000,
                high=100.03,
                low=100.00,
                close=100.02,
                finished_low=True,
                context={
                    "stack_accepted": True,
                    "old_edge_lo": 100.00,
                    "old_edge_hi": 100.02,
                    "fake_leave": False,
                },
            )
        )
        self.assertEqual(d2["scripts"]["D"]["state"], "armed")

    def test_e_first_divergence_flatten_only(self):
        eng = engine()
        eng.step(make(0, high=100.00, low=99.90, close=99.95, delta=1, cvd=3))
        eng.step(make(60_000, high=100.10, low=100.00, close=100.05, delta=1, cvd=5))
        d = eng.step(make(120_000, high=100.30, low=100.10, close=100.20, delta=1, cvd=4))
        self.assertEqual(d["scripts"]["E"]["reason"], "first_div_flatten")
        self.assertEqual(d["main"], "E")
        self.assertTrue(d["intent"]["flatten_only"])

    def test_f_without_book_is_not_evaluated(self):
        eng = engine()
        d = eng.step(make(0))
        self.assertEqual(d["scripts"]["F"]["evaluation"], "not_evaluated")
        self.assertEqual(d["scripts"]["F"]["reason"], "no_l2")
        self.assertIsNone(d["main"])

    def test_f_yield_is_veto_eat_through_can_arm(self):
        eng = engine()
        d = eng.step(
            make(
                0,
                delta=-2,
                book={"book_ok": True, "read": "yielding", "wall_side": "bid"},
            )
        )
        self.assertEqual(d["scripts"]["F"]["evaluation"], "veto")
        d2 = eng.step(
            make(
                60_000,
                delta=-2,
                book={"book_ok": True, "read": "eat_through", "wall_side": "bid"},
            )
        )
        self.assertEqual(d2["scripts"]["F"]["state"], "armed")
        self.assertEqual(d2["main"], "F")

    def test_g_never_arms_even_on_key_unfinished(self):
        eng = engine()
        d = eng.step(
            make(
                0,
                unfinished_high=True,
                high=100.02,
                poc=100.02,
                context={"swing_high": 100.02},
            )
        )
        self.assertEqual(d["scripts"]["G"]["evaluation"], "display")
        self.assertNotEqual(d["scripts"]["G"]["state"], "armed")
        self.assertFalse(d["scripts"]["G"]["is_entry"])
        self.assertIsNone(d["main"])

    def test_unfinished_flag_is_not_an_entry(self):
        self.assertFalse(UnfinishedAuction.is_entry)
        bar = make(0, unfinished_high=True, unfinished_low=True)
        self.assertFalse(bar.footprint["unfinished_is_entry"])

    def test_resonance_off_does_not_copy_price(self):
        eng = engine()
        d = eng.step(
            make(
                0,
                resonance={"mode": "off", "copied_price_onto_okx": False, "used_for_entry": False},
            )
        )
        self.assertFalse(d["copied_price_onto_okx"])
        self.assertEqual(d["intent"]["copied_price_onto_okx"], False)
        self.assertEqual(d["resonance_mode"], "off")

    def test_load_journal_merges_rust_events(self):
        lines = [
            {
                "event": "bar_closed",
                "bar": {"open_ms": 60_000, "state": "closed", "high": 100.02, "low": 100.00},
                "quality_snapshot": {"okx_gap": False},
            },
            {
                "event": "footprint_closed",
                "footprint": {
                    "open_ms": 60_000,
                    "high": 100.02,
                    "low": 100.00,
                    "close": 100.01,
                    "delta": 1.0,
                    "cvd": 1.0,
                    "poc": 100.01,
                    "chaos": False,
                    "bid_vol": 2.0,
                    "ask_vol": 3.0,
                },
            },
            {
                "event": "context_closed",
                "context": {
                    "open_ms": 60_000,
                    "stack_accepted": False,
                    "fake_leave": False,
                    "regime": {"liquidation_regime": "false", "funding_black_window": False},
                },
            },
            {"event": "book_closed", "book": {"book_ok": True, "read": "absorbing"}},
            {
                "event": "resonance_closed",
                "resonance": {
                    "open_ms": 60_000,
                    "mode": "off",
                    "copied_price_onto_okx": False,
                },
            },
        ]
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "j.jsonl"
            path.write_text("".join(json.dumps(x) + "\n" for x in lines))
            bars = load_journal(str(path))
        self.assertEqual(len(bars), 1)
        self.assertEqual(bars[0].open_ms, 60_000)
        self.assertTrue(bars[0].closed)
        self.assertEqual(bars[0].delta, 1.0)
        self.assertFalse(bars[0].context["stack_accepted"])
        self.assertEqual(bars[0].book["read"], "absorbing")
        self.assertEqual(bars[0].resonance["mode"], "off")
        self.assertFalse(bars[0].quality["okx_gap"])


if __name__ == "__main__":
    unittest.main()
