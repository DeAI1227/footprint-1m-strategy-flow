//! OKX public trade adapter.
//!
//! `side=buy` → taker buy (hits ask). `side=sell` → taker sell (hits bid).
//! Role is execution. Other venues must not copy these prices onto OKX orders
//! and this adapter must not copy Binance/Bybit prices either.

use std::path::Path;

use orderflow_domain::{TakerSide, Trade, Venue, VenueRole};
use serde::Deserialize;
use serde_json::Value;

use crate::parse::{load_jsonl_sorted as load_jsonl_lines, ParseError};

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

/// Map OKX `side` to internal taker side. Wrong mapping reverses the whole school.
pub fn parse_side(side: &str) -> Result<TakerSide, ParseError> {
    match side.trim().to_ascii_lowercase().as_str() {
        "buy" => Ok(TakerSide::Buy),
        "sell" => Ok(TakerSide::Sell),
        other => Err(ParseError::BadSide(other.into())),
    }
}

pub fn row_to_trade(row: &OkxTradeRow, recv_ts_ms: i64, symbol: &str) -> Result<Trade, ParseError> {
    let price: f64 = row.px.parse().map_err(|_| ParseError::BadNumber("px"))?;
    let size: f64 = row.sz.parse().map_err(|_| ParseError::BadNumber("sz"))?;
    let event_ts_ms: i64 = row.ts.parse().map_err(|_| ParseError::BadNumber("ts"))?;
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

pub fn parse_trade_json_line(
    line: &str,
    recv_ts_ms: i64,
    symbol: &str,
) -> Result<Trade, ParseError> {
    let row: OkxTradeRow = serde_json::from_str(line).map_err(ParseError::Json)?;
    row_to_trade(&row, recv_ts_ms, symbol)
}

/// Accept either a bare trade object or a WS envelope `{ "data": [ trade, ... ] }`.
pub fn parse_ws_or_row(
    value: &Value,
    recv_ts_ms: i64,
    symbol: &str,
) -> Result<Vec<Trade>, ParseError> {
    if let Some(arr) = value.get("data").and_then(|d| d.as_array()) {
        let mut out = Vec::with_capacity(arr.len());
        for item in arr {
            let row: OkxTradeRow =
                serde_json::from_value(item.clone()).map_err(ParseError::Json)?;
            out.push(row_to_trade(&row, recv_ts_ms, symbol)?);
        }
        return Ok(out);
    }
    let row: OkxTradeRow = serde_json::from_value(value.clone()).map_err(ParseError::Json)?;
    Ok(vec![row_to_trade(&row, recv_ts_ms, symbol)?])
}

pub fn parse_frame(text: &str, recv_ts_ms: i64, symbol: &str) -> Result<Vec<Trade>, ParseError> {
    let v: Value = serde_json::from_str(text).map_err(ParseError::Json)?;
    if crate::ws::is_control_frame(Venue::Okx, &v) {
        return Ok(Vec::new());
    }
    parse_ws_or_row(&v, recv_ts_ms, symbol)
}

/// Read newest-first or any-order OKX history-trades JSONL; yields trades sorted by event time.
pub fn load_jsonl_sorted(path: &Path, symbol: &str) -> Result<Vec<Trade>, String> {
    load_jsonl_lines(path, |line, recv| {
        Ok(vec![
            parse_trade_json_line(line, recv, symbol).map_err(|e| e.to_string())?
        ])
    })
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
        assert_eq!(parse_side("buy").unwrap(), TakerSide::Buy);
        assert_eq!(parse_side("sell").unwrap(), TakerSide::Sell);
        assert!(parse_side("xyz").is_err());
    }

    #[test]
    fn okx_json_line_parses() {
        let line = r#"{"instId":"SOL-USDT-SWAP","tradeId":"1","px":"100.33","sz":"35","side":"buy","ts":"1788328169431","source":"0"}"#;
        let t = parse_trade_json_line(line, 1, "SOL").unwrap();
        assert_eq!(t.venue, Venue::Okx);
        assert_eq!(t.price, 100.33);
        assert_eq!(t.size, 35.0);
        assert_eq!(t.taker_side, TakerSide::Buy);
        assert_eq!(t.event_ts_ms, 1_788_328_169_431);
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
        let trades = parse_ws_or_row(&v, 9, "SOL").unwrap();
        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].taker_side, TakerSide::Sell);
        assert_eq!(trades[1].taker_side, TakerSide::Buy);
    }

    #[test]
    fn replay_jsonl_builds_immutable_closed_bars() {
        let mut tmp = NamedTempFile::new().unwrap();
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
        let trades = load_jsonl_sorted(tmp.path(), "SOL").unwrap();
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
}
