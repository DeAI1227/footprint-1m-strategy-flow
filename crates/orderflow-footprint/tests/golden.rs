//! Golden replay: a recorded SOL clip must reproduce the matrix, not PnL.
//! Three venues replay separately and must not share a summed book.

use orderflow_domain::Venue;
use orderflow_footprint::{FootprintConfig, FootprintEngine};
use orderflow_ingest::okx;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sol_okx_stack3.jsonl")
}

#[test]
fn golden_okx_sol_stack3_matches_matrix() {
    let trades = okx::load_jsonl_sorted(&fixture(), "SOL").unwrap();
    assert_eq!(trades.len(), 7);
    let mut eng = FootprintEngine::new(Venue::Okx, "SOL", FootprintConfig::golden_sol());
    let mut closed = Vec::new();
    for t in &trades {
        closed.extend(eng.push(t));
    }
    assert_eq!(closed.len(), 1, "last minute stays forming");
    let bar = &closed[0].footprint;
    assert_eq!(bar.venue, Venue::Okx);
    assert_eq!(bar.bucket, 0.01);
    assert!(bar.bar_up);
    assert_eq!(bar.dale.buy_stack, 3);
    assert!(bar.dale.aligned);
    assert!(bar.dale.stacked_buy);
    assert_eq!(bar.ask_vol, 30.0);
    assert_eq!(bar.bid_vol, 6.0);
    assert_eq!(bar.delta, 24.0);
    assert!(!bar.unfinished_is_entry);
    assert_eq!(bar.dale.rate, 3.0);
    assert_eq!(bar.valtos.rate, 4.0);
}

#[test]
fn golden_binance_replay_is_a_separate_matrix() {
    // Same clip, different venue tag: matrix is per-venue, never mixed into OKX.
    let mut trades = okx::load_jsonl_sorted(&fixture(), "SOL").unwrap();
    for t in &mut trades {
        t.venue = Venue::Binance;
    }
    let mut eng = FootprintEngine::new(Venue::Binance, "SOL", FootprintConfig::golden_sol());
    let mut closed = Vec::new();
    for t in &trades {
        closed.extend(eng.push(t));
    }
    assert_eq!(closed[0].footprint.venue, Venue::Binance);
    assert_eq!(closed[0].footprint.ask_vol, 30.0);
}
