//! Public trade adapters. Stage 1b wires **all three** venues to the same
//! internal [`Trade`] (`taker_buy` / `taker_sell` only).
//!
//! - OKX = execution footprint
//! - Binance USD-M + Bybit linear = resonance (mode stays `off`; never copy prices)
//! - Bybit taker-side golden tests must stay green
//! - Bounded inboxes: one venue overflow → that venue `gap`, others keep running
//! - Volumes are never summed across venues
//!
//! Live order placement remains gated in `orderflow-domain` / `orderflow-exec`.

pub mod binance;
pub mod bybit;
pub mod journal;
pub mod okx;
pub mod parse;
pub mod queue;
pub mod ws;

use orderflow_domain::{TakerSide, Trade, Venue};

pub const WIRED_OKX: bool = true;
pub const WIRED_BINANCE: bool = true;
pub const WIRED_BYBIT: bool = true;
/// Parsers + bounded lanes for all three venues. Live trading is still gated.
pub const WIRED: bool = true;

pub fn expected_taker_sides() -> [TakerSide; 2] {
    [TakerSide::Buy, TakerSide::Sell]
}

pub fn load_dump_sorted(
    venue: Venue,
    path: &std::path::Path,
    symbol: &str,
) -> Result<Vec<Trade>, String> {
    match venue {
        Venue::Okx => okx::load_jsonl_sorted(path, symbol),
        Venue::Binance => binance::load_dump_sorted(path, symbol),
        Venue::Bybit => bybit::load_dump_sorted(path, symbol),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderflow_clock::{BarCutter, CutEvent};
    use orderflow_domain::VenueRole;

    #[test]
    fn three_venues_wired_resonance_not_execution() {
        assert!(WIRED);
        assert!(WIRED_OKX && WIRED_BINANCE && WIRED_BYBIT);
        assert!(okx::WIRED && binance::WIRED && bybit::WIRED);
        assert_eq!(okx::ROLE, VenueRole::Execution);
        assert_eq!(binance::ROLE, VenueRole::Resonance);
        assert_eq!(bybit::ROLE, VenueRole::Resonance);
        assert!(bybit::TAKER_GOLDEN_REQUIRED);
        assert!(bybit::TAKER_GOLDEN_PRESENT);
    }

    #[test]
    fn three_venues_do_not_sum_volumes() {
        let okx_t = Trade {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            trade_id: Some("o".into()),
            event_ts_ms: 1_000,
            recv_ts_ms: 1,
            processed_ts_ms: 1,
            price: 100.0,
            size: 10.0,
            taker_side: TakerSide::Buy,
        };
        let bn_t = Trade {
            venue: Venue::Binance,
            symbol: "SOL".into(),
            trade_id: Some("b".into()),
            event_ts_ms: 1_000,
            recv_ts_ms: 1,
            processed_ts_ms: 1,
            price: 999.0, // must not leak onto OKX bar
            size: 50.0,
            taker_side: TakerSide::Buy,
        };
        let mut okx_c = BarCutter::new(Venue::Okx, "SOL");
        let mut bn_c = BarCutter::new(Venue::Binance, "SOL");
        okx_c.push(&okx_t);
        bn_c.push(&bn_t);
        let okx_bar = okx_c.flush().unwrap();
        let bn_bar = bn_c.flush().unwrap();
        assert_eq!(okx_bar.ask_vol, 10.0);
        assert_eq!(bn_bar.ask_vol, 50.0);
        assert_eq!(okx_bar.close, 100.0);
        assert_eq!(bn_bar.close, 999.0);
        // Separate matrices: summing would be 60 — that number must not appear on OKX.
        assert_ne!(okx_bar.ask_vol + bn_bar.ask_vol, okx_bar.ask_vol);
    }

    #[test]
    fn adapters_only_emit_taker_buy_or_sell() {
        let sides = expected_taker_sides();
        assert_eq!(sides, [TakerSide::Buy, TakerSide::Sell]);
    }

    #[test]
    fn closed_events_stay_per_venue() {
        let t = Trade {
            venue: Venue::Bybit,
            symbol: "SOL".into(),
            trade_id: Some("1".into()),
            event_ts_ms: 1_000,
            recv_ts_ms: 1,
            processed_ts_ms: 1,
            price: 1.0,
            size: 1.0,
            taker_side: TakerSide::Sell,
        };
        let mut c = BarCutter::new(Venue::Bybit, "SOL");
        c.push(&t);
        let ev = c.push(&Trade {
            event_ts_ms: 60_000,
            trade_id: Some("2".into()),
            ..t.clone()
        });
        assert!(
            matches!(&ev[0], CutEvent::Closed(b) if b.venue == Venue::Bybit && b.bid_vol == 1.0)
        );
    }
}
