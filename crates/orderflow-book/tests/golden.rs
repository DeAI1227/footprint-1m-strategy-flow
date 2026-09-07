//! Golden L2 replay: integrity and isolation, not PnL.

use orderflow_book::{load_jsonl, BookConfig, BookEngine, BookHealth};
use orderflow_domain::{TakerSide, Trade, Venue, VenueRole};
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sol_okx_books.jsonl")
}

fn trade(px: f64, sz: f64) -> Trade {
    Trade {
        venue: Venue::Okx,
        symbol: "SOL".into(),
        trade_id: Some("1".into()),
        event_ts_ms: 1_710_000_001_000,
        recv_ts_ms: 1,
        processed_ts_ms: 1,
        price: px,
        size: sz,
        taker_side: TakerSide::Sell,
    }
}

#[test]
fn golden_okx_book_snapshot_then_absorb() {
    let frames = load_jsonl(Venue::Okx, &fixture()).unwrap();
    assert_eq!(frames.len(), 3, "control subscribe line is skipped");
    let mut eng = BookEngine::new(Venue::Okx, VenueRole::Execution, BookConfig::sol());
    eng.apply_frame(&frames[0].1).unwrap();
    assert_eq!(eng.health(), BookHealth::Ok);
    eng.apply_trade(&trade(100.00, 3.0));
    eng.apply_frame(&frames[1].1).unwrap();
    eng.apply_frame(&frames[2].1).unwrap();
    let snap = eng.freeze_1m(Some(100.00), &[100.00]);
    assert_eq!(snap.venue, Venue::Okx);
    assert_eq!(snap.bid1, Some(100.00));
    assert_eq!(snap.read, orderflow_book::WallRead::Absorb);
    assert!(snap.dom_entries_allowed);
    assert_eq!(snap.script_f, "computed");
    assert!(snap.wall_on_poc);
}

#[test]
fn golden_does_not_copy_onto_binance_engine() {
    let frames = load_jsonl(Venue::Okx, &fixture()).unwrap();
    let mut okx = BookEngine::new(Venue::Okx, VenueRole::Execution, BookConfig::sol());
    let bn = BookEngine::new(Venue::Binance, VenueRole::Resonance, BookConfig::sol());
    okx.apply_frame(&frames[0].1).unwrap();
    assert_eq!(okx.health(), BookHealth::Ok);
    assert_eq!(bn.health(), BookHealth::Rebuilding);
    assert_ne!(okx.freeze_1m(None, &[]).bid1, bn.freeze_1m(None, &[]).bid1);
}
