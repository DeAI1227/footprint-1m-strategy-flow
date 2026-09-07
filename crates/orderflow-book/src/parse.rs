//! Venue-native L2 frames → [`BookDelta`]. Integrity fields stay venue-specific
//! until the engine applies them.

use std::fs;
use std::path::Path;

use orderflow_domain::Venue;
use serde_json::Value;

use crate::ladder::Side;

#[derive(Debug, Clone)]
pub struct LevelUpdate {
    pub side: Side,
    pub price: f64,
    pub size: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookMsgKind {
    Snapshot,
    Delta,
}

#[derive(Debug, Clone)]
pub struct BookDelta {
    pub venue: Venue,
    pub kind: BookMsgKind,
    pub event_ts_ms: i64,
    pub updates: Vec<LevelUpdate>,
    /// OKX `seqId`; Binance final `u`; Bybit `u`.
    pub seq: Option<i64>,
    /// OKX `prevSeqId`; Binance `pu`; Bybit previous `u` is implied.
    pub prev_seq: Option<i64>,
    /// Binance first update id `U` (range start).
    pub seq_from: Option<i64>,
    /// OKX checksum. 0 means deprecated / ignore.
    pub checksum: Option<i32>,
    pub bids_raw: Vec<(String, String)>,
    pub asks_raw: Vec<(String, String)>,
}

#[derive(Debug)]
pub enum BookParseError {
    Json(serde_json::Error),
    Missing(&'static str),
    BadNumber(&'static str),
    Control,
}

impl std::fmt::Display for BookParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "json: {e}"),
            Self::Missing(s) => write!(f, "missing {s}"),
            Self::BadNumber(s) => write!(f, "bad number {s}"),
            Self::Control => write!(f, "control frame"),
        }
    }
}

impl std::error::Error for BookParseError {}

fn i64_of(v: &Value, field: &'static str) -> Result<i64, BookParseError> {
    if let Some(n) = v.as_i64() {
        return Ok(n);
    }
    if let Some(n) = v.as_u64() {
        return Ok(n as i64);
    }
    if let Some(s) = v.as_str() {
        return s.parse().map_err(|_| BookParseError::BadNumber(field));
    }
    Err(BookParseError::BadNumber(field))
}

fn ts_ms(v: &Value) -> Result<i64, BookParseError> {
    i64_of(v, "ts")
}

fn levels_okx(
    arr: &Value,
    side: Side,
) -> Result<(Vec<LevelUpdate>, Vec<(String, String)>), BookParseError> {
    let mut updates = Vec::new();
    let mut raw = Vec::new();
    let Some(items) = arr.as_array() else {
        return Err(BookParseError::Missing("levels"));
    };
    for row in items {
        let cols = row.as_array().ok_or(BookParseError::Missing("level"))?;
        if cols.len() < 2 {
            return Err(BookParseError::Missing("px/sz"));
        }
        let px_s = cols[0].as_str().unwrap_or("").to_string();
        let sz_s = cols[1].as_str().unwrap_or("").to_string();
        raw.push((px_s.clone(), sz_s.clone()));
        updates.push(LevelUpdate {
            side,
            price: px_s.parse().map_err(|_| BookParseError::BadNumber("px"))?,
            size: sz_s.parse().map_err(|_| BookParseError::BadNumber("sz"))?,
        });
    }
    Ok((updates, raw))
}

fn levels_ba(
    arr: &Value,
    side: Side,
) -> Result<(Vec<LevelUpdate>, Vec<(String, String)>), BookParseError> {
    levels_okx(arr, side)
}

pub fn parse_frame(venue: Venue, text: &str) -> Result<BookDelta, BookParseError> {
    let v: Value = serde_json::from_str(text).map_err(BookParseError::Json)?;
    parse_value(venue, &v)
}

pub fn parse_value(venue: Venue, v: &Value) -> Result<BookDelta, BookParseError> {
    match venue {
        Venue::Okx => parse_okx(v),
        Venue::Binance => parse_binance(v),
        Venue::Bybit => parse_bybit(v),
    }
}

fn parse_okx(v: &Value) -> Result<BookDelta, BookParseError> {
    if v.get("event").is_some() && v.get("data").is_none() {
        return Err(BookParseError::Control);
    }
    let action = v
        .get("action")
        .and_then(|x| x.as_str())
        .unwrap_or("snapshot");
    let kind = if action == "update" {
        BookMsgKind::Delta
    } else {
        BookMsgKind::Snapshot
    };
    let row = v
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .ok_or(BookParseError::Missing("data"))?;
    let (mut bids, bids_raw) = levels_okx(
        row.get("bids").ok_or(BookParseError::Missing("bids"))?,
        Side::Bid,
    )?;
    let (asks, asks_raw) = levels_okx(
        row.get("asks").ok_or(BookParseError::Missing("asks"))?,
        Side::Ask,
    )?;
    bids.extend(asks);
    let checksum = row.get("checksum").and_then(|c| {
        c.as_i64()
            .or_else(|| c.as_u64().map(|n| n as i64))
            .map(|n| n as i32)
    });
    Ok(BookDelta {
        venue: Venue::Okx,
        kind,
        event_ts_ms: ts_ms(row.get("ts").ok_or(BookParseError::Missing("ts"))?)?,
        updates: bids,
        seq: row.get("seqId").map(|x| i64_of(x, "seqId")).transpose()?,
        prev_seq: row
            .get("prevSeqId")
            .map(|x| i64_of(x, "prevSeqId"))
            .transpose()?,
        seq_from: None,
        checksum,
        bids_raw,
        asks_raw,
    })
}

fn parse_binance(v: &Value) -> Result<BookDelta, BookParseError> {
    let data = v.get("data").unwrap_or(v);
    if data.get("lastUpdateId").is_some() && data.get("e").is_none() {
        // REST snapshot
        let (mut bids, bids_raw) = levels_ba(
            data.get("bids").ok_or(BookParseError::Missing("bids"))?,
            Side::Bid,
        )?;
        let (asks, asks_raw) = levels_ba(
            data.get("asks").ok_or(BookParseError::Missing("asks"))?,
            Side::Ask,
        )?;
        bids.extend(asks);
        let last = i64_of(
            data.get("lastUpdateId")
                .ok_or(BookParseError::Missing("lastUpdateId"))?,
            "lastUpdateId",
        )?;
        return Ok(BookDelta {
            venue: Venue::Binance,
            kind: BookMsgKind::Snapshot,
            event_ts_ms: 0,
            updates: bids,
            seq: Some(last),
            prev_seq: None,
            seq_from: None,
            checksum: None,
            bids_raw,
            asks_raw,
        });
    }
    if data.get("result").is_some() && data.get("id").is_some() {
        return Err(BookParseError::Control);
    }
    let (mut bids, bids_raw) = levels_ba(
        data.get("b").ok_or(BookParseError::Missing("b"))?,
        Side::Bid,
    )?;
    let (asks, asks_raw) = levels_ba(
        data.get("a").ok_or(BookParseError::Missing("a"))?,
        Side::Ask,
    )?;
    bids.extend(asks);
    Ok(BookDelta {
        venue: Venue::Binance,
        kind: BookMsgKind::Delta,
        event_ts_ms: data
            .get("T")
            .or_else(|| data.get("E"))
            .map(|x| i64_of(x, "T"))
            .transpose()?
            .unwrap_or(0),
        updates: bids,
        seq: Some(i64_of(
            data.get("u").ok_or(BookParseError::Missing("u"))?,
            "u",
        )?),
        prev_seq: data.get("pu").map(|x| i64_of(x, "pu")).transpose()?,
        seq_from: data.get("U").map(|x| i64_of(x, "U")).transpose()?,
        checksum: None,
        bids_raw,
        asks_raw,
    })
}

fn parse_bybit(v: &Value) -> Result<BookDelta, BookParseError> {
    let op = v.get("op").and_then(|x| x.as_str()).unwrap_or("");
    if matches!(op, "subscribe" | "ping" | "pong") {
        return Err(BookParseError::Control);
    }
    let typ = v.get("type").and_then(|x| x.as_str()).unwrap_or("snapshot");
    let kind = if typ == "delta" {
        BookMsgKind::Delta
    } else {
        BookMsgKind::Snapshot
    };
    let data = v.get("data").ok_or(BookParseError::Missing("data"))?;
    let (mut bids, bids_raw) = levels_ba(
        data.get("b").ok_or(BookParseError::Missing("b"))?,
        Side::Bid,
    )?;
    let (asks, asks_raw) = levels_ba(
        data.get("a").ok_or(BookParseError::Missing("a"))?,
        Side::Ask,
    )?;
    bids.extend(asks);
    let u = data.get("u").map(|x| i64_of(x, "u")).transpose()?;
    Ok(BookDelta {
        venue: Venue::Bybit,
        kind,
        event_ts_ms: v
            .get("cts")
            .or_else(|| v.get("ts"))
            .map(|x| ts_ms(x))
            .transpose()?
            .unwrap_or(0),
        updates: bids,
        seq: u,
        prev_seq: None,
        seq_from: data.get("seq").map(|x| i64_of(x, "seq")).transpose()?,
        checksum: None,
        bids_raw,
        asks_raw,
    })
}

pub fn subscribe_text(venue: Venue, market_symbol: &str) -> String {
    match venue {
        Venue::Okx => serde_json::json!({
            "op": "subscribe",
            "args": [{"channel": "books", "instId": market_symbol}]
        })
        .to_string(),
        Venue::Binance => serde_json::json!({
            "method": "SUBSCRIBE",
            "params": [format!("{}@depth@100ms", market_symbol.to_ascii_lowercase())],
            "id": 2
        })
        .to_string(),
        Venue::Bybit => serde_json::json!({
            "op": "subscribe",
            "args": [format!("orderbook.50.{market_symbol}")]
        })
        .to_string(),
    }
}

pub fn load_jsonl(venue: Venue, path: &Path) -> Result<Vec<(i64, String)>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match parse_frame(venue, line) {
            Ok(d) => out.push((d.event_ts_ms, line.to_string())),
            Err(BookParseError::Control) => continue,
            Err(e) => return Err(format!("{}:{}: {e}", path.display(), i + 1)),
        }
    }
    Ok(out)
}
