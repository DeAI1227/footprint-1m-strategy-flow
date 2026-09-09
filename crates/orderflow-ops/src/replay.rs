//! Spec / missed-bar JSONL replay. Cancels that symbol only. No HTTP.

use std::fs;
use std::path::Path;

use orderflow_domain::{AppConfig, Mode, Venue};
use orderflow_exec::{ExecGateway, OrderIntent};

use crate::missed::MissedBarDetector;
use crate::spec::{SpecSnapshot, SpecWatch};

pub fn run_spec_replay(path: &Path, cfg: &AppConfig) -> Result<serde_json::Value, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut watch = SpecWatch::default();
    let mut missed = MissedBarDetector::default();
    let mut gw = ExecGateway::new(Mode::Sim, cfg);
    let mut spec_changes = 0u32;
    let mut missed_n = 0u32;
    let mut rebuilds: Vec<String> = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
        match v.get("event").and_then(|x| x.as_str()).unwrap_or("") {
            "spec" => {
                let snap = SpecSnapshot {
                    venue: Venue::parse(v.get("venue").and_then(|x| x.as_str()).unwrap_or("okx"))?,
                    symbol: v
                        .get("symbol")
                        .and_then(|x| x.as_str())
                        .unwrap_or("SOL")
                        .to_string(),
                    tick_size: v.get("tick_size").and_then(|x| x.as_f64()).unwrap_or(0.0),
                    lot_size: v.get("lot_size").and_then(|x| x.as_f64()).unwrap_or(1.0),
                    contract_mult: v
                        .get("contract_mult")
                        .and_then(|x| x.as_f64())
                        .unwrap_or(1.0),
                };
                if let Some(ch) = watch.observe(snap) {
                    spec_changes += 1;
                    let sym = ch.current.symbol.to_ascii_uppercase();
                    gw.apply_tick_change(&sym);
                    rebuilds.push(sym);
                }
            }
            "closed" => {
                let symbol = v.get("symbol").and_then(|x| x.as_str()).unwrap_or("SOL");
                let venue = Venue::parse(v.get("venue").and_then(|x| x.as_str()).unwrap_or("okx"))?;
                let open_ms = v.get("open_ms").and_then(|x| x.as_i64()).unwrap_or(0);
                if missed.on_closed(venue, symbol, open_ms).is_some() {
                    missed_n += 1;
                    gw.on_missed_bar();
                } else {
                    gw.on_clean_closed(symbol);
                }
            }
            "intent" => {
                let intent: OrderIntent = serde_json::from_value(v)
                    .map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
                let _ = gw.submit(intent, cfg);
            }
            "book" => {
                let book: orderflow_exec::SimBook = serde_json::from_value(v)
                    .map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
                gw.set_book(book);
            }
            _ => {}
        }
    }
    let snap = gw.ledger.snapshot();
    Ok(serde_json::json!({
        "event": "spec_replay_done",
        "ops_wired": true,
        "live_wired": false,
        "copied_price_onto_okx": false,
        "spec_changes": spec_changes,
        "rebuilds": rebuilds,
        "missed_bars": missed_n,
        "spec_pause": gw.spec_paused_symbols(),
        "degrade": gw.risk.degrade,
        "working": snap.working.len(),
        "sol_paused": gw.spec_paused("SOL"),
        "sui_paused": gw.spec_paused("SUI"),
        "note": "stage 8: tick change rebuilds that symbol only; missed-bar stops opens not flatten; no API keys",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg() -> AppConfig {
        AppConfig::load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("params"),
        )
        .unwrap()
    }

    #[test]
    fn fixture_rebuilds_sol_not_sui() {
        let cfg = cfg();
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/spec_replay.jsonl");
        let out = run_spec_replay(&path, &cfg).unwrap();
        assert_eq!(out["ops_wired"], true);
        assert_eq!(out["live_wired"], false);
        assert_eq!(out["copied_price_onto_okx"], false);
        assert_eq!(out["sol_paused"], true);
        assert_eq!(out["sui_paused"], false);
        assert!(out["missed_bars"].as_u64().unwrap() >= 1);
        let line = out.to_string().to_ascii_lowercase();
        assert!(!line.contains("secret"));
        assert!(!line.contains("apikey"));
    }
}
