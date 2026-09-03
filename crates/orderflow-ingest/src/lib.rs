//! Public trade adapters. Stage 1 wires **OKX** normalize + JSONL replay.
//! Binance / Bybit stay stubs until stage 1b (Bybit taker golden required).
//!
//! Rules:
//! - adapters emit `taker_buy` / `taker_sell` only
//! - do not mix venue prices onto OKX orders
//! - one venue stall must not block the other two (queues land in later stages)

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use orderflow_domain::{TakerSide, Trade, Venue};
use serde::Deserialize;
use serde_json::Value;

pub const WIRED_OKX: bool = true;
pub const WIRED: bool = false; // all three venues; stage 1b flips when BN/BYBIT land

pub fn expected_taker_sides() -> [TakerSide; 2] {
    [TakerSide::Buy, TakerSide::Sell]
}

pub mod binance {
    pub const VENUE: &str = "binance";
    pub const WIRED: bool = false;
}

pub mod bybit {
    pub const VENUE: &str = "bybit";
    pub const WIRED: bool = false;
    /// Bybit taker-side golden tests are required before this module may parse live trades.
    pub const TAKER_GOLDEN_REQUIRED: bool = true;
}

pub mod okx {
    use super::*;
    use orderflow_domain::VenueRole;

    pub const VENUE: &str = "okx";
    pub const WIRED: bool = true;
    pub const ROLE: VenueRole = VenueRole::Execution;

    /// OKX public history-trades / WS trade row (one object in `data[]`).
    #[derive(Debug, Deserialize)]
    pub struct OkxTradeRow {
        #[serde(rename = "instId")]
        pub inst_id: String,
        #[serde(rename = "tradeId")]
        pub trade_id: String,
        pub px: String,
        pub sz: String,
        /// `buy` = taker buy (hits ask); `sell` = taker sell (hits bid).
        pub side: String,
        pub ts: String,
    }

    #[derive(Debug)]
    pub enum OkxParseError {
        Json(serde_json::Error),
        BadSide(String),
        BadNumber(&'static str),
    }

    impl std::fmt::Display for OkxParseError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Json(e) => write!(f, "json: {e}"),
                Self::BadSide(s) => write!(f, "bad side {s:?}"),
                Self::BadNumber(f0) => write!(f, "bad number field {f0}"),
            }
        }
    }

    impl std::error::Error for OkxParseError {}

    /// Map OKX `side` to internal taker side. Wrong mapping reverses the whole school.
    pub fn parse_side(side: &str) -> Result<TakerSide, OkxParseError> {
        match side.trim().to_ascii_lowercase().as_str() {
            "buy" => Ok(TakerSide::Buy),
            "sell" => Ok(TakerSide::Sell),
            other => Err(OkxParseError::BadSide(other.into())),
        }
    }

    pub fn row_to_trade(row: &OkxTradeRow, recv_ts_ms: i64, symbol: &str) -> Result<Trade, OkxParseError> {
        let price: f64 = row
            .px
            .parse()
            .map_err(|_| OkxParseError::BadNumber("px"))?;
        let size: f64 = row
            .sz
            .parse()
            .map_err(|_| OkxParseError::BadNumber("sz"))?;
        let event_ts_ms: i64 = row
            .ts
            .parse()
            .map_err(|_| OkxParseError::BadNumber("ts"))?;
        Ok(Trade {
            venue: Venue::Okx,
            symbol: symbol.to_string(),
            trade_id: Some(row.trade_id.clone()),
            event_ts_ms,
            recv_ts_ms,
            processed_ts_ms: recv_ts_ms,
            price,
            size,
            taker_side: parse_side(&row.side)?,
        })
    }

    pub fn parse_trade_json_line(line: &str, recv_ts_ms: i64, symbol: &str) -> Result<Trade, OkxParseError> {
        let row: OkxTradeRow = serde_json::from_str(line).map_err(OkxParseError::Json)?;
        row_to_trade(&row, recv_ts_ms, symbol)
    }

    /// Accept either a bare trade object or a WS envelope `{ "data": [ trade, ... ] }`.
    pub fn parse_ws_or_row(value: &Value, recv_ts_ms: i64, symbol: &str) -> Result<Vec<Trade>, OkxParseError> {
        if let Some(arr) = value.get("data").and_then(|d| d.as_array()) {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                let row: OkxTradeRow =
                    serde_json::from_value(item.clone()).map_err(OkxParseError::Json)?;
                out.push(row_to_trade(&row, recv_ts_ms, symbol)?);
            }
            return Ok(out);
        }
        let row: OkxTradeRow = serde_json::from_value(value.clone()).map_err(OkxParseError::Json)?;
        Ok(vec![row_to_trade(&row, recv_ts_ms, symbol)?])
    }

    /// Read newest-first or any-order OKX history-trades JSONL; yields trades sorted by event time.
    pub fn load_jsonl_sorted(path: &Path, symbol: &str) -> Result<Vec<Trade>, String> {
        let f = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
        let reader = BufReader::new(f);
        let mut trades = Vec::new();
        let mut recv = 0_i64;
        for (i, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| format!("line {i}: {e}"))?;
            if line.trim().is_empty() {
                continue;
            }
            recv += 1;
            let t = parse_trade_json_line(&line, recv, symbol).map_err(|e| format!("line {i}: {e}"))?;
            trades.push(t);
        }
        trades.sort_by_key(|t| (t.event_ts_ms, t.trade_id.clone()));
        // Dedup by trade_id if present.
        let mut seen = std::collections::HashSet::new();
        trades.retain(|t| match &t.trade_id {
            Some(id) => seen.insert(id.clone()),
            None => true,
        });
        Ok(trades)
    }
}

/// Append-only JSONL journal of **closed** 1m bars. Never rewrite past lines.
pub mod journal {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::path::Path;

    use orderflow_domain::{Bar1m, QualityVector};
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    pub struct ClosedBarRecord<'a> {
        pub event: &'static str,
        pub bar: &'a Bar1m,
        pub quality_snapshot: &'a QualityVector,
    }

    pub struct JsonlJournal {
        path: std::path::PathBuf,
    }

    impl JsonlJournal {
        pub fn create(path: impl AsRef<Path>) -> Result<Self, String> {
            let path = path.as_ref().to_path_buf();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
            }
            // Truncate on create for a fresh replay run.
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
                .map_err(|e| format!("create {}: {e}", path.display()))?;
            Ok(Self { path })
        }

        pub fn append_closed(&self, bar: &Bar1m, quality: &QualityVector) -> Result<(), String> {
            use orderflow_domain::BarState;
            if !matches!(bar.state, BarState::Closed) {
                return Err("journal only accepts Closed bars".into());
            }
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .map_err(|e| format!("open {}: {e}", self.path.display()))?;
            let rec = ClosedBarRecord {
                event: "bar_closed",
                bar,
                quality_snapshot: quality,
            };
            let line = serde_json::to_string(&rec).map_err(|e| e.to_string())?;
            writeln!(f, "{line}").map_err(|e| e.to_string())?;
            Ok(())
        }

        pub fn path(&self) -> &Path {
            &self.path
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderflow_clock::{BarCutter, CutEvent};
    use orderflow_domain::BarState;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn okx_side_buy_is_taker_buy() {
        assert_eq!(okx::parse_side("buy").unwrap(), TakerSide::Buy);
        assert_eq!(okx::parse_side("sell").unwrap(), TakerSide::Sell);
        assert!(okx::parse_side("xyz").is_err());
    }

    #[test]
    fn okx_json_line_parses() {
        let line = r#"{"instId":"SOL-USDT-SWAP","tradeId":"1","px":"100.33","sz":"35","side":"buy","ts":"1788328169431","source":"0"}"#;
        let t = okx::parse_trade_json_line(line, 1, "SOL").unwrap();
        assert_eq!(t.venue, Venue::Okx);
        assert_eq!(t.price, 100.33);
        assert_eq!(t.size, 35.0);
        assert_eq!(t.taker_side, TakerSide::Buy);
        assert_eq!(t.event_ts_ms, 1788328169431);
    }

    #[test]
    fn okx_ws_envelope_parses_many() {
        let v = serde_json::json!({
            "arg": {"channel":"trades","instId":"SOL-USDT-SWAP"},
            "data": [
                {"instId":"SOL-USDT-SWAP","tradeId":"1","px":"1","sz":"1","side":"sell","ts":"1000"},
                {"instId":"SOL-USDT-SWAP","tradeId":"2","px":"2","sz":"2","side":"buy","ts":"1001"}
            ]
        });
        let trades = okx::parse_ws_or_row(&v, 9, "SOL").unwrap();
        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].taker_side, TakerSide::Sell);
        assert_eq!(trades[1].taker_side, TakerSide::Buy);
    }

    #[test]
    fn replay_jsonl_builds_immutable_closed_bars() {
        let mut tmp = NamedTempFile::new().unwrap();
        // Two minutes, newest-first file order (like OKX history dump).
        writeln!(
            tmp,
            r#"{{"instId":"SOL-USDT-SWAP","tradeId":"2","px":"101","sz":"2","side":"sell","ts":"60000"}}"#
        )
        .unwrap();
        writeln!(
            tmp,
            r#"{{"instId":"SOL-USDT-SWAP","tradeId":"1","px":"100","sz":"1","side":"buy","ts":"1000"}}"#
        )
        .unwrap();
        let trades = okx::load_jsonl_sorted(tmp.path(), "SOL").unwrap();
        assert_eq!(trades[0].trade_id.as_deref(), Some("1"));
        let mut cutter = BarCutter::new(Venue::Okx, "SOL");
        let mut closed = Vec::new();
        for t in &trades {
            for ev in cutter.push(t) {
                if let CutEvent::Closed(b) = ev {
                    closed.push(b);
                }
            }
        }
        if let Some(b) = cutter.flush() {
            closed.push(b);
        }
        assert_eq!(closed.len(), 2);
        assert!(matches!(closed[0].state, BarState::Closed));
        assert_eq!(closed[0].ask_vol, 1.0);
        assert_eq!(closed[1].bid_vol, 2.0);
    }

    #[test]
    fn journal_rejects_forming() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bars.jsonl");
        let j = journal::JsonlJournal::create(&path).unwrap();
        let forming = orderflow_domain::Bar1m {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            open_ms: 0,
            state: orderflow_domain::BarState::Forming,
            open: 1.0,
            high: 1.0,
            low: 1.0,
            close: 1.0,
            bid_vol: 0.0,
            ask_vol: 0.0,
            trade_count: 0,
            first_trade_ts_ms: 0,
            last_trade_ts_ms: 0,
        };
        assert!(j
            .append_closed(&forming, &orderflow_domain::QualityVector::default())
            .is_err());
    }

    #[test]
    fn stubs_mark_binance_bybit_unwired() {
        assert!(!super::WIRED);
        assert!(super::WIRED_OKX);
        assert!(!binance::WIRED);
        assert!(!bybit::WIRED);
        assert!(bybit::TAKER_GOLDEN_REQUIRED);
    }
}
