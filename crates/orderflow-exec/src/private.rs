//! OKX private WS decode. Fixture replay only — no TCP, no API keys.
//! Binance / Bybit private is not opened (they are not the execution venue).

use orderflow_domain::Venue;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::intent::Side;
use crate::sim::Fill;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderState {
    Live,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateOrder {
    pub cl_ord_id: String,
    pub ord_id: Option<String>,
    pub inst_id: String,
    pub symbol: String,
    pub side: Side,
    pub px: Option<f64>,
    pub sz: f64,
    pub acc_fill_sz: f64,
    pub state: OrderState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivatePosition {
    pub inst_id: String,
    pub symbol: String,
    pub qty: f64,
    pub avg_px: f64,
    pub mark_px: Option<f64>,
    pub liq_px: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSnap {
    pub pos_mode: String,
    pub td_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ack {
    pub op: String,
    pub ok: bool,
    pub cl_ord_id: Option<String>,
    pub ord_id: Option<String>,
    pub code: String,
    pub msg: String,
    pub class: RejectClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectClass {
    Ok,
    Precision,
    Spread,
    Position,
    Risk,
    RateLimit,
    Other,
}

#[derive(Debug, Clone)]
pub enum PrivateEvent {
    Control,
    Order(PrivateOrder),
    Fill(Fill),
    Position(PrivatePosition),
    Account(AccountSnap),
    Ack(Ack),
    Refused(&'static str),
}

pub fn private_subscribe_okx() -> String {
    serde_json::json!({
        "op": "subscribe",
        "args": [
            {"channel": "orders", "instType": "SWAP"},
            {"channel": "fills", "instType": "SWAP"},
            {"channel": "positions", "instType": "SWAP"},
            {"channel": "account"}
        ]
    })
    .to_string()
}

/// OKX clOrdId: 1–32 alphanumeric.
pub fn cl_ord_id(symbol: &str, script: &str, seq: u64) -> String {
    let tag = if symbol.eq_ignore_ascii_case("SUI") {
        "sui"
    } else {
        "sol"
    };
    let sc = script
        .chars()
        .next()
        .filter(|c| c.is_ascii_alphanumeric())
        .unwrap_or('X');
    let id = format!("of{tag}{sc}{seq:08}");
    debug_assert!(id.len() <= 32);
    debug_assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
    id
}

pub fn symbol_from_inst(inst_id: &str) -> String {
    if inst_id.starts_with("SUI") {
        "SUI".into()
    } else {
        "SOL".into()
    }
}

fn parse_f64(v: &Value, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(x) = v.get(*k) {
            if let Some(n) = x.as_f64() {
                return Some(n);
            }
            if let Some(s) = x.as_str() {
                if let Ok(n) = s.parse::<f64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn parse_side(v: &Value) -> Side {
    match v
        .get("side")
        .and_then(|x| x.as_str())
        .unwrap_or("buy")
        .to_ascii_lowercase()
        .as_str()
    {
        "sell" => Side::Sell,
        _ => Side::Buy,
    }
}

fn parse_state(s: &str) -> OrderState {
    match s {
        "live" => OrderState::Live,
        "partially_filled" => OrderState::PartiallyFilled,
        "filled" => OrderState::Filled,
        "canceled" | "cancelled" => OrderState::Canceled,
        _ => OrderState::Rejected,
    }
}

pub fn classify_reject(code: &str, msg: &str) -> RejectClass {
    if code == "0" || code.is_empty() {
        return RejectClass::Ok;
    }
    let blob = format!("{code} {msg}").to_ascii_lowercase();
    if blob.contains("precision") || blob.contains("tick") || code == "51000" {
        RejectClass::Precision
    } else if blob.contains("spread") || code == "51119" {
        RejectClass::Spread
    } else if blob.contains("position") || blob.contains("margin") {
        RejectClass::Position
    } else if blob.contains("rate") || blob.contains("limit") || code == "50011" {
        RejectClass::RateLimit
    } else if blob.contains("risk") {
        RejectClass::Risk
    } else {
        RejectClass::Other
    }
}

/// Decode one private JSON line. Never treats Binance/Bybit as execution.
pub fn parse_private_frame(text: &str) -> Result<Vec<PrivateEvent>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if v.get("venue")
        .and_then(|x| x.as_str())
        .map(|s| Venue::parse(s).ok())
        .flatten()
        .is_some_and(|ven| ven != Venue::Okx)
    {
        return Ok(vec![PrivateEvent::Refused("not_okx_private")]);
    }
    if let Some(stream) = v.get("stream").and_then(|x| x.as_str()) {
        if stream.contains("binance") || stream.contains("bybit") {
            return Ok(vec![PrivateEvent::Refused("not_okx_private")]);
        }
    }

    let event = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
    if matches!(event, "subscribe" | "unsubscribe" | "login" | "error") {
        return Ok(vec![PrivateEvent::Control]);
    }

    let op = v.get("op").and_then(|x| x.as_str()).unwrap_or("");
    if matches!(op, "order" | "cancel-order" | "batch-orders") {
        return Ok(vec![parse_ack(&v)]);
    }

    let channel = v
        .pointer("/arg/channel")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let data = v.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default();
    if data.is_empty() && matches!(op, "subscribe" | "login") {
        return Ok(vec![PrivateEvent::Control]);
    }

    let mut out = Vec::new();
    for row in data {
        match channel {
            "orders" => out.push(PrivateEvent::Order(parse_order(&row))),
            "fills" => {
                if let Some(f) = parse_fill(&row) {
                    out.push(PrivateEvent::Fill(f));
                }
            }
            "positions" => out.push(PrivateEvent::Position(parse_position(&row))),
            "account" | "balance_and_position" => {
                out.push(PrivateEvent::Account(parse_account(&row)))
            }
            _ => {
                if row.get("clOrdId").is_some() && row.get("fillSz").is_some() {
                    if let Some(f) = parse_fill(&row) {
                        out.push(PrivateEvent::Fill(f));
                    }
                }
            }
        }
    }
    if out.is_empty() {
        out.push(PrivateEvent::Control);
    }
    Ok(out)
}

fn parse_order(row: &Value) -> PrivateOrder {
    let inst = row
        .get("instId")
        .and_then(|x| x.as_str())
        .unwrap_or("SOL-USDT-SWAP");
    PrivateOrder {
        cl_ord_id: row
            .get("clOrdId")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        ord_id: row
            .get("ordId")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        inst_id: inst.to_string(),
        symbol: symbol_from_inst(inst),
        side: parse_side(row),
        px: parse_f64(row, &["px"]),
        sz: parse_f64(row, &["sz"]).unwrap_or(0.0),
        acc_fill_sz: parse_f64(row, &["accFillSz"]).unwrap_or(0.0),
        state: parse_state(row.get("state").and_then(|x| x.as_str()).unwrap_or("")),
    }
}

fn parse_fill(row: &Value) -> Option<Fill> {
    let inst = row.get("instId").and_then(|x| x.as_str()).unwrap_or("");
    let qty = parse_f64(row, &["fillSz"])?;
    if qty <= 0.0 {
        return None;
    }
    let exec = row
        .get("execType")
        .and_then(|x| x.as_str())
        .unwrap_or("M");
    Some(Fill {
        client_id: row
            .get("clOrdId")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        symbol: symbol_from_inst(inst),
        side: parse_side(row),
        price: parse_f64(row, &["fillPx"]).unwrap_or(0.0),
        qty,
        maker: exec != "T",
        venue: Venue::Okx,
    })
}

fn parse_position(row: &Value) -> PrivatePosition {
    let inst = row
        .get("instId")
        .and_then(|x| x.as_str())
        .unwrap_or("SOL-USDT-SWAP");
    let pos = parse_f64(row, &["pos"]).unwrap_or(0.0);
    PrivatePosition {
        inst_id: inst.to_string(),
        symbol: symbol_from_inst(inst),
        qty: pos,
        avg_px: parse_f64(row, &["avgPx"]).unwrap_or(0.0),
        mark_px: parse_f64(row, &["markPx"]),
        liq_px: parse_f64(row, &["liqPx"]),
    }
}

fn parse_account(row: &Value) -> AccountSnap {
    AccountSnap {
        pos_mode: row
            .get("posMode")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        td_mode: row
            .get("tdMode")
            .or_else(|| row.get("mgnMode"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
    }
}

fn parse_ack(v: &Value) -> PrivateEvent {
    let op = v.get("op").and_then(|x| x.as_str()).unwrap_or("order");
    let top = v.get("code").and_then(|x| x.as_str()).unwrap_or("0");
    let row = v
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or(Value::Null);
    let scode = row
        .get("sCode")
        .and_then(|x| x.as_str())
        .unwrap_or(top)
        .to_string();
    let msg = row
        .get("sMsg")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let ok = top == "0" && (scode == "0" || scode.is_empty());
    PrivateEvent::Ack(Ack {
        op: op.to_string(),
        ok,
        cl_ord_id: row
            .get("clOrdId")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        ord_id: row
            .get("ordId")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        code: scode.clone(),
        msg: msg.clone(),
        class: if ok {
            RejectClass::Ok
        } else {
            classify_reject(&scode, &msg)
        },
    })
}
