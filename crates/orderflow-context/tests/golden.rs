//! Golden: closed footprint bars feed context. No VWAP. Resonance does not copy prices.

use orderflow_context::{BarIn, ContextConfig, ContextEngine, RegimeInputs, FORBIDDEN};
use orderflow_domain::Venue;
use orderflow_footprint::{FootprintConfig, FootprintEngine};
use orderflow_ingest::okx;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../orderflow-footprint/tests/fixtures/sol_okx_stack3.jsonl")
}

#[test]
fn golden_okx_stack3_emits_context_without_vwap() {
    let trades = okx::load_jsonl_sorted(&fixture(), "SOL").unwrap();
    let mut fp = FootprintEngine::new(Venue::Okx, "SOL", FootprintConfig::golden_sol());
    let mut ctx = ContextEngine::new(ContextConfig::golden_sol());
    let empty = RegimeInputs::default();
    let mut snaps = Vec::new();
    for t in &trades {
        for closed in fp.push(t) {
            snaps.push(ctx.push(BarIn::from_footprint(&closed.footprint), &empty));
        }
    }
    assert_eq!(snaps.len(), 1);
    let s = &snaps[0];
    assert_eq!(s.venue, Venue::Okx);
    assert!(s.stack_5.poc.is_some());
    assert_eq!(s.regime.liquidation_regime, "not_evaluated");
    assert!(s.regime.liq_stream_missing);
    assert_eq!(s.regime.funding_crowding, "not_evaluated");
    assert!(!s.stack_accepted);
    let text = serde_json::to_string(s).unwrap();
    assert!(text.contains("forbidden"));
    assert!(!text.contains("vwap_value"));
    assert!(!text.contains("session_tpo"));
    assert!(!text.contains("naked_poc_list"));
    let _ = FORBIDDEN;
}

#[test]
fn golden_peer_price_is_never_an_okx_field() {
    let text = serde_json::to_string(&orderflow_context::join_resonance(
        0,
        orderflow_domain::ResonanceMode::Off,
        1,
        Some(orderflow_context::VenueDir::from_delta(
            Venue::Okx,
            0,
            1.0,
            1,
            true,
        )),
        Some(orderflow_context::VenueDir::from_delta(
            Venue::Binance,
            0,
            1.0,
            1,
            true,
        )),
        None,
    ))
    .unwrap();
    assert!(text.contains("copied_price_onto_okx"));
    assert!(text.contains("false"));
    assert!(!text.contains("binance_price"));
    assert!(!text.contains("bybit_price"));
}
