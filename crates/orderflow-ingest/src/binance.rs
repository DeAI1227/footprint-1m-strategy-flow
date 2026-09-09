//! Binance USD-M public aggTrade adapter (resonance venue).
//!
//! Official aggTrade field `m` / REST `isBuyerMaker`:
//! "Is the buyer the market maker?"
//! - `true`  → buyer is maker → **taker sold** (hits bid)
//! - `false` → buyer is taker → **taker bought** (hits ask)
//!
//! Event time is `T` (trade time, milliseconds). Do not treat Binance prices as
//! OKX limit prices. Do not sum volume with other venues.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use orderflow_domain::{TakerSide, Trade, Venue, VenueRole};
use serde_json::Value;

use crate::parse::{
    json_f64, json_ms_field, json_string, load_jsonl_sorted as load_jsonl_lines, looks_like_json,
    peek_first_nonempty_line, sort_dedup_trades, ParseError,
};

pub const VENUE: &str = "binance";
pub const WIRED: bool = true;
pub const ROLE: VenueRole = VenueRole::Resonance;

/// Official mapping. Inverting this reverses Binance footprint vs OKX.
pub fn taker_from_is_buyer_maker(is_buyer_maker: bool) -> TakerSide {
    if is_buyer_maker {
        TakerSide::Sell
    } else {
        TakerSide::Buy
    }
}

pub fn parse_is_buyer_maker(v: &Value) -> Result<bool, ParseError> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::Number(n) => Ok(n.as_i64().unwrap_or(0) != 0),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" => Ok(false),
            other => Err(ParseError::BadSide(format!("isBuyerMaker={other}"))),
        },
        _ => Err(ParseError::MissingField("m")),
    }
}

fn trade_from_agg(obj: &Value, recv_ts_ms: i64, symbol: &str) -> Result<Trade, ParseError> {
    let price = json_f64(obj.get("p").ok_or(ParseError::MissingField("p"))?, "p")?;
    let size = json_f64(obj.get("q").ok_or(ParseError::MissingField("q"))?, "q")?;
    let ts_v = obj
        .get("T")
        .or_else(|| obj.get("E"))
        .ok_or(ParseError::MissingField("T"))?;
    let event_ts_ms = json_ms_field(ts_v)?;
    let m = obj
        .get("m")
        .or_else(|| obj.get("isBuyerMaker"))
        .ok_or(ParseError::MissingField("m"))?;
    let trade_id = obj.get("a").or_else(|| obj.get("id")).and_then(json_string);
    Ok(Trade {
        venue: Venue::Binance,
        symbol: symbol.to_string(),
        trade_id,
        event_ts_ms,
        recv_ts_ms,
        processed_ts_ms: recv_ts_ms,
        price,
        size,
        taker_side: taker_from_is_buyer_maker(parse_is_buyer_maker(m)?),
    })
}

/// Bare aggTrade object, combined-stream `{stream,data}`, or an array of trades.
pub fn parse_ws_or_row(
    value: &Value,
    recv_ts_ms: i64,
    symbol: &str,
) -> Result<Vec<Trade>, ParseError> {
    if crate::ws::is_control_frame(Venue::Binance, value) {
        return Ok(Vec::new());
    }
    if let Some(data) = value.get("data") {
        if let Some(arr) = data.as_array() {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(trade_from_agg(item, recv_ts_ms, symbol)?);
            }
            return Ok(out);
        }
        if data.is_object() {
            return Ok(vec![trade_from_agg(data, recv_ts_ms, symbol)?]);
        }
    }
    Ok(vec![trade_from_agg(value, recv_ts_ms, symbol)?])
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

/// `data.binance.vision` USD-M aggTrades daily CSV:
/// `agg_trade_id,price,quantity,first_trade_id,last_trade_id,transact_time,is_buyer_maker`
pub fn load_aggtrades_csv_sorted(path: &Path, symbol: &str) -> Result<Vec<Trade>, String> {
    let f = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut trades = Vec::new();
    let mut recv = 0_i64;
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| format!("line {i}: {e}"))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.to_ascii_lowercase().starts_with("agg_trade_id")
            || line.to_ascii_lowercase().starts_with("id")
        {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 7 {
            return Err(format!(
                "line {i}: expected 7 CSV columns, got {}",
                parts.len()
            ));
        }
        recv += 1;
        let is_buyer_maker = match parts[6].trim().to_ascii_lowercase().as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            other => return Err(format!("line {i}: bad is_buyer_maker {other:?}")),
        };
        let event_ts_ms: i64 = parts[5]
            .trim()
            .parse()
            .map_err(|_| format!("line {i}: bad transact_time"))?;
        trades.push(Trade {
            venue: Venue::Binance,
            symbol: symbol.to_string(),
            trade_id: Some(parts[0].trim().to_string()),
            event_ts_ms,
            recv_ts_ms: recv,
            processed_ts_ms: recv,
            price: parts[1]
                .parse()
                .map_err(|_| format!("line {i}: bad price"))?,
            size: parts[2].parse().map_err(|_| format!("line {i}: bad qty"))?,
            taker_side: taker_from_is_buyer_maker(is_buyer_maker),
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
        load_aggtrades_csv_sorted(path, symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Official USD-M aggTrade: `m` true = buyer is maker = taker sell.
    #[test]
    fn golden_m_true_is_taker_sell() {
        assert_eq!(taker_from_is_buyer_maker(true), TakerSide::Sell);
        assert_eq!(taker_from_is_buyer_maker(false), TakerSide::Buy);
        let line = r#"{"e":"aggTrade","E":1672531200123,"s":"SOLUSDT","a":7,"p":"100.25","q":"3.5","f":1,"l":2,"T":1672531200100,"m":true}"#;
        let t = parse_frame(line, 9, "SOL").unwrap().pop().unwrap();
        assert_eq!(t.venue, Venue::Binance);
        assert_eq!(t.taker_side, TakerSide::Sell);
        assert_eq!(t.event_ts_ms, 1_672_531_200_100);
        assert_eq!(t.price, 100.25);
        assert_eq!(t.size, 3.5);
        assert_eq!(t.trade_id.as_deref(), Some("7"));
    }

    #[test]
    fn golden_m_false_is_taker_buy() {
        let line =
            r#"{"e":"aggTrade","s":"SOLUSDT","a":8,"p":"101","q":"1","T":1672531200200,"m":false}"#;
        let t = parse_frame(line, 1, "SOL").unwrap().pop().unwrap();
        assert_eq!(t.taker_side, TakerSide::Buy);
    }

    #[test]
    fn combined_stream_envelope() {
        let line = r#"{"stream":"solusdt@aggTrade","data":{"e":"aggTrade","s":"SOLUSDT","a":1,"p":"99","q":"2","T":1000,"m":false}}"#;
        let t = parse_frame(line, 1, "SOL").unwrap().pop().unwrap();
        assert_eq!(t.taker_side, TakerSide::Buy);
        assert_eq!(t.price, 99.0);
        assert_eq!(
            t.event_ts_ms, 1000,
            "Binance T is milliseconds, even when small"
        );
    }

    #[test]
    fn subscribe_ack_is_empty() {
        let n = parse_frame(r#"{"result":null,"id":1}"#, 1, "SOL").unwrap();
        assert!(n.is_empty());
    }

    #[test]
    fn csv_dump_maps_is_buyer_maker() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmp,
            "agg_trade_id,price,quantity,first_trade_id,last_trade_id,transact_time,is_buyer_maker"
        )
        .unwrap();
        writeln!(tmp, "10,100.0,1.0,1,1,1672531200000,true").unwrap();
        writeln!(tmp, "11,101.0,2.0,2,2,1672531200001,false").unwrap();
        let trades = load_dump_sorted(tmp.path(), "SOL").unwrap();
        assert_eq!(trades[0].taker_side, TakerSide::Sell);
        assert_eq!(trades[1].taker_side, TakerSide::Buy);
        assert_eq!(trades[0].event_ts_ms, 1_672_531_200_000);
    }
}
