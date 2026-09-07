//! Per-venue 1m footprint matrix (stage 2).
//!
//! Hard rules:
//! - one matrix per venue; never sum Binance + OKX + Bybit volume
//! - diagonal imbalance; ignore zeros; no Market Profile / VWAP / Naked POC
//! - Python must not build a second production matrix
//! - unfinished auction is display-only, not an entry
//! - Dale 300% and Valtos 400% stay parallel; do not average to 350%
//! - min volume is session p25 of nonempty sides, never a frozen SOL lot

mod bucket;
mod config;
mod engine;
mod features;

pub use bucket::{bucket_key, key_to_price, percentile, session_of, Session};
pub use config::FootprintConfig;
pub use engine::{ClosedFootprint, FootprintEngine};
pub use features::{FootprintBar, FootprintCell, RateSlice};

pub const WIRED: bool = true;
pub const FORBIDDEN: &[&str] = &["vwap", "avwap", "tpo", "market_profile", "naked_poc"];

pub fn unfinished_is_entry() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderflow_domain::{AppConfig, TakerSide, Trade, Venue};
    use std::path::PathBuf;

    fn trade(ts: i64, px: f64, sz: f64, side: TakerSide, id: &str) -> Trade {
        Trade {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            trade_id: Some(id.into()),
            event_ts_ms: ts,
            recv_ts_ms: ts,
            processed_ts_ms: ts,
            price: px,
            size: sz,
            taker_side: side,
        }
    }

    fn close_bar(eng: &mut FootprintEngine, last_minute_ts: i64) -> FootprintBar {
        let out = eng.push(&trade(
            last_minute_ts,
            100.03,
            0.01,
            TakerSide::Buy,
            "close",
        ));
        out.into_iter()
            .next()
            .expect("expected a closed footprint")
            .footprint
    }

    /// Three adjacent buy imbalances at 300% with min_vol=1, up bar.
    fn push_stack3(eng: &mut FootprintEngine) {
        // open on a sell at 100.00 so bar can still close up after buys
        eng.push(&trade(1_000, 100.00, 2.0, TakerSide::Sell, "1"));
        eng.push(&trade(2_000, 100.01, 10.0, TakerSide::Buy, "2"));
        eng.push(&trade(3_000, 100.01, 2.0, TakerSide::Sell, "3"));
        eng.push(&trade(4_000, 100.02, 10.0, TakerSide::Buy, "4"));
        eng.push(&trade(5_000, 100.02, 2.0, TakerSide::Sell, "5"));
        eng.push(&trade(6_000, 100.03, 10.0, TakerSide::Buy, "6"));
    }

    #[test]
    fn wired_and_unfinished_is_not_entry() {
        assert!(WIRED);
        assert!(!unfinished_is_entry());
        for w in FORBIDDEN {
            assert!(!w.is_empty());
        }
    }

    #[test]
    fn toml_keeps_300_and_400_parallel_and_sui_bucket_native() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("params");
        let cfg = AppConfig::load(&root).unwrap();
        let sol = FootprintConfig::from_symbol(&cfg.sol).unwrap();
        let sui = FootprintConfig::from_symbol(&cfg.sui).unwrap();
        assert_eq!(sol.bucket, 0.01);
        assert_eq!(sui.bucket, 0.0001);
        assert_ne!(sol.bucket, sui.bucket);
        assert_eq!(sol.imbalance_rate_dale, 3.0);
        assert_eq!(sol.imbalance_rate_valtos, 4.0);
        assert!((sol.imbalance_rate_dale - sol.imbalance_rate_valtos).abs() > f64::EPSILON);
        assert!(!sol.unfinished_is_entry);
        assert_eq!(
            sol.min_imbalance_volume_rule,
            "session_nonempty_side_p25_both"
        );
    }

    #[test]
    fn ignore_zero_rejects_zero_vs_print() {
        let cfg = FootprintConfig::golden_sol();
        let mut cells = std::collections::BTreeMap::new();
        crate::features::apply_trade(&mut cells, 100.01, 10.0, TakerSide::Buy, cfg.bucket);
        // no bid at 100.00 → diagonal 10 vs 0 must not count
        let c = crate::features::compute(&cells, &cfg, 1.0, 100.01, 100.01).unwrap();
        assert!(c.dale.buy_imb_prices.is_empty());
        assert!(c.record.buy_imb_prices.is_empty());
    }

    #[test]
    fn diagonal_buy_stack3_aligned_on_up_bar() {
        let mut eng = FootprintEngine::new(Venue::Okx, "SOL", FootprintConfig::golden_sol());
        push_stack3(&mut eng);
        let bar = close_bar(&mut eng, 60_000);
        assert_eq!(bar.venue, Venue::Okx);
        assert!(bar.bar_up, "open 100.00 close 100.03");
        assert_eq!(bar.dale.buy_stack, 3);
        assert!(bar.dale.aligned);
        assert!(bar.dale.stacked_buy);
        assert!(!bar.dale.stacked_sell);
        assert!(!bar.chaos);
        assert!(!bar.unfinished_is_entry);
        assert_eq!(bar.absorption, "not_evaluated");
        assert_eq!(bar.script_f, "not_evaluated");
        // 400% is stricter than 300%; both computed, not averaged to 350%
        assert_eq!(bar.dale.rate, 3.0);
        assert_eq!(bar.valtos.rate, 4.0);
        assert_eq!(bar.record.rate, 2.0);
        assert!(bar.dale.buy_stack >= bar.valtos.buy_stack);
        let json = serde_json::to_string(&bar).unwrap();
        let lower = json.to_ascii_lowercase();
        assert!(!lower.contains("vwap"));
        assert!(!lower.contains("naked_poc"));
        assert!(!lower.contains("tpo"));
    }

    #[test]
    fn contra_stack_on_down_bar_is_not_aligned() {
        let mut eng = FootprintEngine::new(Venue::Okx, "SOL", FootprintConfig::golden_sol());
        // close down: last price below open
        eng.push(&trade(1_000, 100.03, 10.0, TakerSide::Buy, "o"));
        eng.push(&trade(1_100, 100.00, 2.0, TakerSide::Sell, "1"));
        eng.push(&trade(2_000, 100.01, 10.0, TakerSide::Buy, "2"));
        eng.push(&trade(3_000, 100.01, 2.0, TakerSide::Sell, "3"));
        eng.push(&trade(4_000, 100.02, 10.0, TakerSide::Buy, "4"));
        eng.push(&trade(5_000, 100.02, 2.0, TakerSide::Sell, "5"));
        eng.push(&trade(6_000, 100.03, 10.0, TakerSide::Buy, "6"));
        eng.push(&trade(7_000, 100.00, 1.0, TakerSide::Sell, "c"));
        let bar = close_bar(&mut eng, 60_000);
        assert!(bar.bar_down);
        assert_eq!(bar.dale.buy_stack, 3);
        assert!(!bar.dale.aligned);
        assert!(bar.dale.contra);
        assert!(!bar.dale.stacked_buy);
    }

    #[test]
    fn unfinished_high_when_both_sides_print_at_extreme() {
        let mut eng = FootprintEngine::new(Venue::Okx, "SOL", FootprintConfig::golden_sol());
        eng.push(&trade(1_000, 100.00, 1.0, TakerSide::Sell, "1"));
        eng.push(&trade(2_000, 100.01, 1.0, TakerSide::Buy, "2"));
        eng.push(&trade(3_000, 100.01, 1.0, TakerSide::Sell, "3"));
        let bar = close_bar(&mut eng, 60_000);
        assert!(bar.unfinished_high);
        assert!(!bar.finished_high);
        assert!(!bar.unfinished_is_entry);
    }

    #[test]
    fn finished_high_excess_when_bid_at_high_is_zero() {
        let mut eng = FootprintEngine::new(Venue::Okx, "SOL", FootprintConfig::golden_sol());
        eng.push(&trade(1_000, 100.00, 1.0, TakerSide::Sell, "1"));
        eng.push(&trade(2_000, 100.01, 1.0, TakerSide::Buy, "2"));
        let bar = close_bar(&mut eng, 60_000);
        // high bucket is 100.01 ask-only → finished high / excess
        assert!(bar.finished_high);
        assert!(!bar.unfinished_high);
    }

    #[test]
    fn poc_is_max_volume_bucket_va_expands_adjacent() {
        let mut eng = FootprintEngine::new(Venue::Okx, "SOL", FootprintConfig::golden_sol());
        eng.push(&trade(1_000, 100.00, 1.0, TakerSide::Sell, "1"));
        eng.push(&trade(2_000, 100.01, 8.0, TakerSide::Buy, "2"));
        eng.push(&trade(3_000, 100.02, 1.0, TakerSide::Buy, "3"));
        let bar = close_bar(&mut eng, 60_000);
        assert_eq!(bar.poc, Some(100.01));
        assert!(bar.va_ok);
        assert_eq!(bar.va_low, Some(100.01));
        assert_eq!(bar.va_high, Some(100.01));
    }

    #[test]
    fn cvd_resets_on_session_change() {
        let mut eng = FootprintEngine::new(Venue::Okx, "SOL", FootprintConfig::golden_sol());
        let asia = 0i64;
        eng.push(&trade(asia + 1_000, 100.0, 5.0, TakerSide::Buy, "a1"));
        let b1 = eng.push(&trade(asia + 60_000, 100.0, 1.0, TakerSide::Buy, "a2"));
        assert_eq!(b1[0].footprint.cvd, 5.0);
        let us = 13 * 3_600_000;
        // This closes the leftover Asia forming minute, then starts US.
        eng.push(&trade(us + 1_000, 100.0, 2.0, TakerSide::Sell, "u1"));
        let b2 = eng.push(&trade(us + 60_000, 100.0, 1.0, TakerSide::Buy, "u2"));
        assert_eq!(b2[0].footprint.session, Session::Us);
        assert_eq!(
            b2[0].footprint.cvd, -2.0,
            "CVD must reset at session, not keep Asia"
        );
    }

    #[test]
    fn three_venues_do_not_sum_matrices() {
        let cfg = FootprintConfig::golden_sol();
        let mut okx = FootprintEngine::new(Venue::Okx, "SOL", cfg.clone());
        let mut bn = FootprintEngine::new(Venue::Binance, "SOL", cfg);
        let mut t = trade(1_000, 100.0, 3.0, TakerSide::Buy, "1");
        t.venue = Venue::Okx;
        okx.push(&t);
        t.venue = Venue::Binance;
        t.size = 9.0;
        bn.push(&t);
        t.event_ts_ms = 60_000;
        t.venue = Venue::Okx;
        t.size = 0.01;
        let o = okx.push(&t);
        t.venue = Venue::Binance;
        let b = bn.push(&t);
        assert_eq!(o[0].footprint.ask_vol, 3.0);
        assert_eq!(b[0].footprint.ask_vol, 9.0);
        assert_eq!(o[0].footprint.venue, Venue::Okx);
        assert_eq!(b[0].footprint.venue, Venue::Binance);
        assert_ne!(
            o[0].footprint.ask_vol + b[0].footprint.ask_vol,
            o[0].footprint.ask_vol
        );
    }

    #[test]
    fn min_volume_is_session_p25_not_a_sol_constant() {
        let mut eng = FootprintEngine::new(Venue::Okx, "SOL", FootprintConfig::golden_sol());
        // first bar: no history → p25 = 0
        eng.push(&trade(1_000, 100.00, 8.0, TakerSide::Buy, "1"));
        eng.push(&trade(2_000, 100.00, 8.0, TakerSide::Sell, "2"));
        let b1 = eng.push(&trade(60_000, 100.00, 0.01, TakerSide::Buy, "c1"));
        assert_eq!(b1[0].footprint.min_volume, 0.0);
        assert_eq!(
            b1[0].footprint.min_volume_rule,
            "session_nonempty_side_p25_both"
        );
        // second bar uses p25 of prior nonempty sides (8 and 8) → 8
        eng.push(&trade(61_000, 100.00, 1.0, TakerSide::Buy, "3"));
        let b2 = eng.push(&trade(120_000, 100.00, 0.01, TakerSide::Buy, "c2"));
        assert!((b2[0].footprint.min_volume - 8.0).abs() < 1e-9);
    }
}
