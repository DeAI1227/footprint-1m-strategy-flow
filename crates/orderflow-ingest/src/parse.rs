//! Shared parse helpers. Timestamp units are venue-specific at the wire;
//! adapters must normalize to **milliseconds** before constructing [`Trade`].

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use orderflow_domain::Trade;
use serde_json::Value;

#[derive(Debug)]
pub enum ParseError {
    Json(serde_json::Error),
    BadSide(String),
    BadNumber(&'static str),
    MissingField(&'static str),
    Empty,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "json: {e}"),
            Self::BadSide(s) => write!(f, "bad side {s:?}"),
            Self::BadNumber(field) => write!(f, "bad number field {field}"),
            Self::MissingField(field) => write!(f, "missing field {field}"),
            Self::Empty => write!(f, "empty payload"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Seconds vs milliseconds vs microseconds, decided by magnitude — never by venue name.
///
/// - `|t| < 1e11` → seconds (Bybit `public.bybit.com` CSV often uses this, sometimes fractional)
/// - `|t| < 1e14` → milliseconds (OKX `ts`, Binance `T`/`E`, Bybit v5 WS `T`)
/// - else → microseconds
pub fn event_ts_to_ms_f64(raw: f64) -> i64 {
    let abs = raw.abs();
    if abs < 1e11 {
        (raw * 1000.0).round() as i64
    } else if abs < 1e14 {
        raw.round() as i64
    } else {
        (raw / 1000.0).round() as i64
    }
}

pub fn event_ts_to_ms_i64(raw: i64) -> i64 {
    let abs = raw.abs();
    if abs < 100_000_000_000 {
        raw.saturating_mul(1000)
    } else if abs < 100_000_000_000_000 {
        raw
    } else {
        raw / 1000
    }
}

pub fn parse_ts_str(s: &str) -> Result<i64, ParseError> {
    let n: f64 = s.trim().parse().map_err(|_| ParseError::BadNumber("ts"))?;
    Ok(event_ts_to_ms_f64(n))
}

/// Explicit **milliseconds**. Use for OKX `ts`, Binance `T`/`E`, Bybit v5 WS `T`.
/// Do not apply the seconds heuristic here — that is dump-format specific.
pub fn json_ms_field(v: &Value) -> Result<i64, ParseError> {
    if let Some(n) = v.as_i64() {
        return Ok(n);
    }
    if let Some(n) = v.as_u64() {
        return Ok(n as i64);
    }
    if let Some(s) = v.as_str() {
        let s = s.trim();
        if s.contains('.') {
            let n: f64 = s.parse().map_err(|_| ParseError::BadNumber("ts"))?;
            return Ok(n.round() as i64);
        }
        return s.parse().map_err(|_| ParseError::BadNumber("ts"));
    }
    if let Some(n) = v.as_f64() {
        return Ok(n.round() as i64);
    }
    Err(ParseError::BadNumber("ts"))
}

pub fn json_f64(v: &Value, field: &'static str) -> Result<f64, ParseError> {
    if let Some(n) = v.as_f64() {
        return Ok(n);
    }
    if let Some(s) = v.as_str() {
        return s.parse().map_err(|_| ParseError::BadNumber(field));
    }
    Err(ParseError::BadNumber(field))
}

pub fn json_string(v: &Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        return Some(s.to_string());
    }
    if let Some(n) = v.as_i64() {
        return Some(n.to_string());
    }
    if let Some(n) = v.as_u64() {
        return Some(n.to_string());
    }
    None
}

/// Load JSONL; `parse_line` gets (line, synthetic recv_ts). Sorted by event time, deduped by trade_id.
pub fn load_jsonl_sorted<F>(path: &Path, mut parse_line: F) -> Result<Vec<Trade>, String>
where
    F: FnMut(&str, i64) -> Result<Vec<Trade>, String>,
{
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
        let mut parsed = parse_line(&line, recv).map_err(|e| format!("line {i}: {e}"))?;
        trades.append(&mut parsed);
    }
    sort_dedup_trades(&mut trades);
    Ok(trades)
}

pub fn sort_dedup_trades(trades: &mut Vec<Trade>) {
    trades.sort_by(|a, b| {
        (a.event_ts_ms, a.trade_id.clone()).cmp(&(b.event_ts_ms, b.trade_id.clone()))
    });
    let mut seen = std::collections::HashSet::new();
    trades.retain(|t| match &t.trade_id {
        Some(id) => seen.insert(id.clone()),
        None => true,
    });
}

pub fn peek_first_nonempty_line(path: &Path) -> Result<String, String> {
    let f = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| format!("line {i}: {e}"))?;
        if !line.trim().is_empty() {
            return Ok(line);
        }
    }
    Err(format!("{} is empty", path.display()))
}

pub fn looks_like_json(line: &str) -> bool {
    matches!(line.trim().as_bytes().first(), Some(b'{') | Some(b'['))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_units_by_magnitude_not_venue_name() {
        // Bybit v5 WS `T` is milliseconds.
        assert_eq!(event_ts_to_ms_i64(1_672_304_486_865), 1_672_304_486_865);
        // Bybit public CSV seconds (possibly fractional).
        assert_eq!(event_ts_to_ms_f64(1_672_304_486.865), 1_672_304_486_865);
        // Microseconds would be 16+ digits.
        assert_eq!(event_ts_to_ms_i64(1_672_304_486_865_000), 1_672_304_486_865);
        // Seconds as integer.
        assert_eq!(event_ts_to_ms_i64(1_672_304_486), 1_672_304_486_000);
    }

    #[test]
    fn json_ms_field_does_not_promote_small_ms_to_seconds() {
        let v = serde_json::json!(60000);
        assert_eq!(json_ms_field(&v).unwrap(), 60_000);
        let s = serde_json::json!("60000");
        assert_eq!(json_ms_field(&s).unwrap(), 60_000);
    }
}
