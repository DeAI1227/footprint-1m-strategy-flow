"""OOS A–G screens on frozen bars. Do not pick 300 vs 400. Live still gated."""

from __future__ import annotations

import unittest

from orderflow.calibrate.screens import run_screens, screen_a, screen_f
from orderflow.decision.snapshot import FrozenBar


def _bar(
    i: int,
    *,
    high: float,
    low: float,
    close: float,
    session: str = "us",
    dale=None,
    valtos=None,
    finished_low: bool = False,
    unfinished_high: bool = False,
    poc: float | None = None,
    delta: float = 1.0,
    cvd: float = 1.0,
) -> FrozenBar:
    return FrozenBar(
        bar={"open_ms": i * 60_000, "state": "closed", "high": high, "low": low, "close": close},
        footprint={
            "open_ms": i * 60_000,
            "session": session,
            "high": high,
            "low": low,
            "close": close,
            "delta": delta,
            "cvd": cvd,
            "poc": poc if poc is not None else close,
            "finished_low": finished_low,
            "finished_high": False,
            "unfinished_high": unfinished_high,
            "unfinished_low": False,
            "unfinished_is_entry": False,
            "bid_vol": 2.0,
            "ask_vol": 8.0,
            "dale": dale
            or {
                "aligned": False,
                "stacked_buy": False,
                "stacked_sell": False,
                "buy_imb_prices": [],
                "sell_imb_prices": [],
            },
            "valtos": valtos
            or {
                "aligned": False,
                "stacked_buy": False,
                "stacked_sell": False,
                "buy_imb_prices": [],
                "sell_imb_prices": [],
            },
        },
    )


BUY = {
    "aligned": True,
    "stacked_buy": True,
    "stacked_sell": False,
    "buy_imb_prices": [100.00, 100.01, 100.02],
    "sell_imb_prices": [],
}


class TestOosScreens(unittest.TestCase):
    def test_a_punch_and_no_leave_do_not_choose_a_rate(self):
        bars = [
            _bar(0, high=100.02, low=100.00, close=100.02, dale=BUY),  # armed
            _bar(1, high=100.06, low=100.04, close=100.05),  # leave
            _bar(2, high=100.03, low=99.97, close=99.97),  # punch
        ]
        a = screen_a(bars, "dale", leave_bars=1)["all"]
        self.assertEqual(a["armed"], 1)
        self.assertEqual(a["left"], 1)
        self.assertEqual(a["punch"], 1)
        valtos = screen_a(bars, "valtos", leave_bars=1)["all"]
        self.assertEqual(valtos["armed"], 0)

        out = run_screens(bars)
        self.assertIsNone(out["chosen_armed_rate"])
        self.assertTrue(out["still_open"])
        self.assertFalse(out["out_of_sample_validated"])
        self.assertFalse(out["script_g_is_entry"])
        self.assertEqual(out["script_f"], "not_evaluated")
        self.assertEqual(out["F"]["all"]["not_evaluated"], 3)

    def test_no_leave_is_chase_not_a(self):
        bars = [
            _bar(0, high=100.02, low=100.00, close=100.02, dale=BUY),
            _bar(1, high=100.03, low=100.00, close=100.01),
        ]
        a = screen_a(bars, "dale")["all"]
        self.assertEqual(a["no_leave"], 1)
        self.assertEqual(a["left"], 0)

    def test_f_without_book_is_not_evaluated(self):
        bars = [_bar(0, high=1, low=0, close=1)]
        f = screen_f(bars)["all"]
        self.assertEqual(f["not_evaluated"], 1)
        self.assertEqual(f["reason"], "no_l2")

    def test_unfinished_entry_flag_is_rejected(self):
        bar = _bar(0, high=1, low=0, close=1)
        bar.footprint["unfinished_is_entry"] = True
        with self.assertRaises(ValueError):
            run_screens([bar])


if __name__ == "__main__":
    unittest.main()
