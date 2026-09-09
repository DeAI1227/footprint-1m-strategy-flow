//! Public-WS subscribe payloads and frame dispatch.
//!
//! TCP long-connect is optional (`--ws` on orderflowd). Parsers here must work
//! offline from recorded frames / dumps. One venue's parse error does not
//! touch another venue's inbox.

use orderflow_domain::{Trade, Venue};
use serde_json::Value;

use crate::parse::ParseError;

pub fn binance_agg_trade_stream(market_symbol: &str) -> String {
    format!("{}@aggTrade", market_symbol.to_ascii_lowercase())
}

pub fn bybit_public_trade_topic(market_symbol: &str) -> String {
    format!("publicTrade.{market_symbol}")
}

/// Venue-native subscribe JSON. `market_symbol` is the **venue** contract
/// (`SOLUSDT`, `SOL-USDT-SWAP`), not the internal `SOL` name.
pub fn subscribe_text(venue: Venue, market_symbol: &str) -> String {
    match venue {
        Venue::Binance => serde_json::json!({
            "method": "SUBSCRIBE",
            "params": [binance_agg_trade_stream(market_symbol)],
            "id": 1
        })
        .to_string(),
        Venue::Bybit => serde_json::json!({
            "op": "subscribe",
            "args": [bybit_public_trade_topic(market_symbol)]
        })
        .to_string(),
        Venue::Okx => serde_json::json!({
            "op": "subscribe",
            "args": [{"channel": "trades", "instId": market_symbol}]
        })
        .to_string(),
    }
}

pub fn is_control_frame(venue: Venue, v: &Value) -> bool {
    match venue {
        Venue::Binance => {
            v.get("result").is_some()
                && v.get("id").is_some()
                && v.get("data").is_none()
                && v.get("e").is_none()
        }
        Venue::Bybit => {
            let op = v.get("op").and_then(|x| x.as_str()).unwrap_or("");
            matches!(op, "subscribe" | "unsubscribe" | "ping" | "pong")
                || (v.get("success").is_some() && v.get("data").is_none())
        }
        Venue::Okx => {
            let event = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
            matches!(event, "subscribe" | "unsubscribe" | "error" | "login")
                || v.get("event").is_some() && v.get("data").is_none()
        }
    }
}

pub fn parse_frame(
    venue: Venue,
    text: &str,
    recv_ts_ms: i64,
    symbol: &str,
) -> Result<Vec<Trade>, ParseError> {
    match venue {
        Venue::Okx => crate::okx::parse_frame(text, recv_ts_ms, symbol),
        Venue::Binance => crate::binance::parse_frame(text, recv_ts_ms, symbol),
        Venue::Bybit => crate::bybit::parse_frame(text, recv_ts_ms, symbol),
    }
}

/// Push parsed trades into the matching lane. Overflow gaps **that** venue only.
pub fn ingest_frame(
    lanes: &crate::queue::ThreeLanes,
    venue: Venue,
    text: &str,
    recv_ts_ms: i64,
    symbol: &str,
) -> Result<usize, ParseError> {
    let trades = parse_frame(venue, text, recv_ts_ms, symbol)?;
    let mut n = 0usize;
    for t in trades {
        if lanes.try_push(t) {
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::ThreeLanes;
    use orderflow_domain::TakerSide;

    #[test]
    fn subscribe_payloads_are_venue_native() {
        let bn = subscribe_text(Venue::Binance, "SOLUSDT");
        assert!(bn.contains("solusdt@aggTrade"));
        let by = subscribe_text(Venue::Bybit, "SOLUSDT");
        assert!(by.contains("publicTrade.SOLUSDT"));
        let ok = subscribe_text(Venue::Okx, "SOL-USDT-SWAP");
        assert!(ok.contains("SOL-USDT-SWAP"));
        assert!(ok.contains("trades"));
    }

    #[test]
    fn ingest_binance_overflow_leaves_okx_lane_open() {
        let lanes = ThreeLanes::with_cap(1);
        let bn = r#"{"e":"aggTrade","s":"SOLUSDT","a":1,"p":"1","q":"1","T":1000,"m":false}"#;
        let bn2 = r#"{"e":"aggTrade","s":"SOLUSDT","a":2,"p":"1","q":"1","T":1001,"m":true}"#;
        assert_eq!(
            ingest_frame(&lanes, Venue::Binance, bn, 1, "SOL").unwrap(),
            1
        );
        assert_eq!(
            ingest_frame(&lanes, Venue::Binance, bn2, 2, "SOL").unwrap(),
            0
        );
        let ok = r#"{"instId":"SOL-USDT-SWAP","tradeId":"9","px":"100","sz":"1","side":"buy","ts":"1000"}"#;
        assert_eq!(ingest_frame(&lanes, Venue::Okx, ok, 3, "SOL").unwrap(), 1);
        assert!(lanes.binance.is_gap());
        assert!(!lanes.okx.is_gap());
        let t = lanes.okx.pop().unwrap();
        assert_eq!(t.taker_side, TakerSide::Buy);
        assert_eq!(t.venue, Venue::Okx);
    }
}
