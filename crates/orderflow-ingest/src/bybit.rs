//! Bybit linear publicTrade adapter (resonance venue, **not** a backup).
//!
//! Official v5 WS `publicTrade.{symbol}` field table
//! (<https://bybit-exchange.github.io/docs/v5/websocket/public/trade>):
//! - `S` — **Side of taker.** `Buy`, `Sell`
//! - `T` — timestamp (**milliseconds**) the order is filled
//! - `v` size, `p` price, `i` trade id
//!
//! `S` is **not** maker side. Guessing this wrong reverses the whole school.
//! Historical `public.bybit.com` CSV `side` uses the same taker meaning
//! (week-3 research script: buy → ask / taker buy).
//!
//! Timestamp unit is **not** inherited from OKX or Binance; normalize by magnitude.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use orderflow_domain::{TakerSide, Trade, Venue, VenueRole};
use serde_json::Value;

use crate::parse::{
    json_f64, json_ms_field, json_string, load_jsonl_sorted as load_jsonl_lines, looks_like_json,
    parse_ts_str, peek_first_nonempty_line, sort_dedup_trades, ParseError,
};

pub const VENUE: &str = "bybit";
pub const WIRED: bool = true;
pub const ROLE: VenueRole = VenueRole::Resonance;
/// Golden tests in this module must stay green before any live parse path is used.
pub const TAKER_GOLDEN_REQUIRED: bool = true;
pub const TAKER_GOLDEN_PRESENT: bool = true;

/// Official: `S` is side of **taker**. `Buy` → hits ask; `Sell` → hits bid.
pub fn parse_taker_side(side: &str) -> Result<TakerSide, ParseError> {
    match side.trim().to_ascii_lowercase().as_str() {
        "buy" => Ok(TakerSide::Buy),
        "sell" => Ok(TakerSide::Sell),
        other => Err(ParseError::BadSide(other.into())),
    }
}

fn trade_from_row(obj: &Value, recv_ts_ms: i64, symbol: &str) -> Result<Trade, ParseError> {
    let side = obj
        .get("S")
        .or_else(|| obj.get("side"))
        .and_then(|v| v.as_str())
        .ok_or(ParseError::MissingField("S"))?;
    let price = json_f64(
        obj.get("p")
            .or_else(|| obj.get("price"))
            .ok_or(ParseError::MissingField("p"))?,
        "p",
    )?;
    let size = json_f64(
        obj.get("v")
            .or_else(|| obj.get("size"))
            .ok_or(ParseError::MissingField("v"))?,
        "v",
    )?;
    let ts_v = obj
        .get("T")
        .or_else(|| obj.get("ts"))
        .or_else(|| obj.get("timestamp"))
        .ok_or(ParseError::MissingField("T"))?;
    let event_ts_ms = json_ms_field(ts_v)?;
    let trade_id = obj
        .get("i")
        .or_else(|| obj.get("execId"))
        .and_then(json_string);
    Ok(Trade {
        venue: Venue::Bybit,
        symbol: symbol.to_string(),
        trade_id,
        event_ts_ms,
        recv_ts_ms,
        processed_ts_ms: recv_ts_ms,
        price,
        size,
        taker_side: parse_taker_side(side)?,
    })
}

/// Bare trade object or WS envelope `{ "topic": "publicTrade.SOLUSDT", "data": [ ... ] }`.
pub fn parse_ws_or_row(
    value: &Value,
    recv_ts_ms: i64,
    symbol: &str,
) -> Result<Vec<Trade>, ParseError> {
    if crate::ws::is_control_frame(Venue::Bybit, value) {
        return Ok(Vec::new());
    }
    if let Some(arr) = value.get("data").and_then(|d| d.as_array()) {
        if arr.is_empty() {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity(arr.len());
        for item in arr {
            out.push(trade_from_row(item, recv_ts_ms, symbol)?);
        }
        return Ok(out);
    }
    Ok(vec![trade_from_row(value, recv_ts_ms, symbol)?])
}

pub fn parse_frame(text: &str, recv_ts_ms: i64, symbol: &str) -> Result<Vec<Trade>, ParseError> {
    let v: Value = serde_json::from_str(text).map_err(ParseError::Json)?;
    parse_ws_or_row(&v, recv_ts_ms, symbol)
}

pub fn load_jsonl_sorted(path: &Path, symbol: &str) -> Result<Vec<Trade>, String> {
    load_jsonl_lines(path, |line, recv| {
        let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        parse_ws_or_row(&v, recv, symbol).map_err(|e| e.to_string())
    })
}

/// `public.bybit.com/trading/{symbol}/{symbol}{day}.csv.gz` columns:
/// `timestamp,symbol,side,size,price,tickDirection,trdMatchID,...`
/// `timestamp` is often seconds (float); `side` is taker side.
pub fn load_public_csv_sorted(path: &Path, symbol: &str) -> Result<Vec<Trade>, String> {
    let f = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut trades = Vec::new();
    let mut recv = 0_i64;
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| format!("line {i}: {e}"))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.to_ascii_lowercase().starts_with("timestamp") {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 5 {
            return Err(format!(
                "line {i}: expected ≥5 CSV columns, got {}",
                parts.len()
            ));
        }
        recv += 1;
        let event_ts_ms = parse_ts_str(parts[0]).map_err(|e| format!("line {i}: {e}"))?;
        let side = parse_taker_side(parts[2]).map_err(|e| format!("line {i}: {e}"))?;
        trades.push(Trade {
            venue: Venue::Bybit,
            symbol: symbol.to_string(),
            trade_id: parts
                .get(6)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            event_ts_ms,
            recv_ts_ms: recv,
            processed_ts_ms: recv,
            price: parts[4]
                .parse()
                .map_err(|_| format!("line {i}: bad price"))?,
            size: parts[3]
                .parse()
                .map_err(|_| format!("line {i}: bad size"))?,
            taker_side: side,
        });
    }
    sort_dedup_trades(&mut trades);
    Ok(trades)
}

pub fn load_dump_sorted(path: &Path, symbol: &str) -> Result<Vec<Trade>, String> {
    let first = peek_first_nonempty_line(path)?;
    if looks_like_json(&first) {
        load_jsonl_sorted(path, symbol)
    } else {
        load_public_csv_sorted(path, symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderflow_clock::{BarCutter, CutEvent};

    /// Official example payload from Bybit v5 publicTrade docs.
    const OFFICIAL_WS: &str = r#"{
        "topic": "publicTrade.BTCUSDT",
        "type": "snapshot",
        "ts": 1672304486868,
        "data": [
            {
                "T": 1672304486865,
                "s": "BTCUSDT",
                "S": "Buy",
                "v": "0.001",
                "p": "16578.50",
                "L": "PlusTick",
                "i": "20f43950-d8dd-5b31-9112-a178eb6023af",
                "BT": false,
                "seq": 1783284617
            }
        ]
    }"#;

    #[test]
    fn golden_required_flag_stays_on() {
        assert!(TAKER_GOLDEN_REQUIRED);
        assert!(TAKER_GOLDEN_PRESENT);
        assert!(WIRED);
    }

    #[test]
    fn golden_s_is_taker_side_not_maker() {
        // Official comment: "Side of taker. Buy, Sell"
        assert_eq!(parse_taker_side("Buy").unwrap(), TakerSide::Buy);
        assert_eq!(parse_taker_side("Sell").unwrap(), TakerSide::Sell);
        assert_eq!(parse_taker_side("buy").unwrap(), TakerSide::Buy);
        assert!(parse_taker_side("maker").is_err());
        let t = parse_frame(OFFICIAL_WS, 42, "SOL").unwrap().pop().unwrap();
        assert_eq!(t.venue, Venue::Bybit);
        assert_eq!(
            t.taker_side,
            TakerSide::Buy,
            "S=Buy is taker buy, not maker"
        );
        assert_eq!(t.event_ts_ms, 1_672_304_486_865, "T is milliseconds");
        assert_eq!(t.price, 16578.50);
        assert_eq!(t.size, 0.001);
        assert_eq!(
            t.trade_id.as_deref(),
            Some("20f43950-d8dd-5b31-9112-a178eb6023af")
        );
        // If someone treated S as maker side, Buy would be inverted to taker sell.
        assert_ne!(t.taker_side, TakerSide::Sell);
    }

    #[test]
    fn golden_s_sell_is_taker_sell() {
        let line =
            r#"{"T":1672304486865,"s":"SOLUSDT","S":"Sell","v":"2.5","p":"19.3928","i":"x"}"#;
        let t = parse_frame(line, 1, "SOL").unwrap().pop().unwrap();
        assert_eq!(t.taker_side, TakerSide::Sell);
        assert_eq!(t.event_ts_ms, 1_672_304_486_865);
    }

    #[test]
    fn golden_timestamp_seconds_csv_not_mixed_with_okx_ms() {
        // public.bybit.com uses seconds; OKX history uses ms strings. Magnitude decides.
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmp,
            "timestamp,symbol,side,size,price,tickDirection,trdMatchID"
        )
        .unwrap();
        writeln!(tmp, "1672304486.865,SOLUSDT,Buy,1.5,100.0,PlusTick,abc").unwrap();
        writeln!(tmp, "1672304546,SOLUSDT,Sell,2.0,101.0,MinusTick,def").unwrap();
        let trades = load_dump_sorted(tmp.path(), "SOL").unwrap();
        assert_eq!(trades[0].event_ts_ms, 1_672_304_486_865);
        assert_eq!(trades[0].taker_side, TakerSide::Buy);
        assert_eq!(trades[1].event_ts_ms, 1_672_304_546_000);
        assert_eq!(trades[1].taker_side, TakerSide::Sell);
    }

    #[test]
    fn subscribe_ack_is_empty() {
        let n = parse_frame(
            r#"{"success":true,"ret_msg":"","op":"subscribe","conn_id":"x"}"#,
            1,
            "SOL",
        )
        .unwrap();
        assert!(n.is_empty());
    }

    #[test]
    fn replay_csv_feeds_cutter_without_summing_into_okx() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmp,
            "timestamp,symbol,side,size,price,tickDirection,trdMatchID"
        )
        .unwrap();
        writeln!(tmp, "1000,SOLUSDT,Buy,1,100,PlusTick,1").unwrap();
        writeln!(tmp, "60000,SOLUSDT,Sell,2,101,MinusTick,2").unwrap();
        let trades = load_dump_sorted(tmp.path(), "SOL").unwrap();
        let mut cutter = BarCutter::new(Venue::Bybit, "SOL");
        let mut closed = Vec::new();
        for t in &trades {
            assert_eq!(t.venue, Venue::Bybit);
            for ev in cutter.push(t) {
                if let CutEvent::Closed(b) = ev {
                    assert_eq!(b.venue, Venue::Bybit);
                    closed.push(b);
                }
            }
        }
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].ask_vol, 1.0);
        assert_eq!(closed[0].bid_vol, 0.0);
    }
}
