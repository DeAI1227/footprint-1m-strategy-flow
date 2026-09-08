//! Crypto regime vetoes. Never an entry reason. No liquidation-map hunting.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::config::ContextConfig;

#[derive(Debug, Clone, Default)]
pub struct RegimeInputs {
    /// Hour-open ms → OI.
    pub oi_1h: BTreeMap<i64, f64>,
    /// Bar-open ms → liquidation notional.
    pub liq_1m: BTreeMap<i64, f64>,
    /// Event-time ms → funding rate.
    pub funding_rate: BTreeMap<i64, f64>,
    pub stream_present: bool,
}

impl RegimeInputs {
    pub fn load_jsonl(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut out = Self {
            stream_present: true,
            ..Self::default()
        };
        for (i, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(line)
                .map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
            let ev = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
            match ev {
                "oi_1h" => {
                    let ts = i64_field(&v, "ts_ms")?;
                    let oi = f64_field(&v, "oi")?;
                    out.oi_1h.insert(hour_open_ms(ts), oi);
                }
                "liq_1m" => {
                    let ts = v
                        .get("open_ms")
                        .or_else(|| v.get("ts_ms"))
                        .and_then(json_i64)
                        .ok_or_else(|| format!("{}:{}: missing open_ms", path.display(), i + 1))?;
                    let n = f64_field(&v, "notional")?;
                    out.liq_1m.insert(bar_open_ms_local(ts), n);
                }
                "funding" => {
                    let ts = i64_field(&v, "ts_ms")?;
                    let rate = f64_field(&v, "rate")?;
                    out.funding_rate.insert(ts, rate);
                }
                _ => {}
            }
        }
        Ok(out)
    }

    pub fn is_empty(&self) -> bool {
        !self.stream_present && self.oi_1h.is_empty() && self.liq_1m.is_empty()
    }
}

fn json_i64(v: &serde_json::Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_u64().map(|n| n as i64))
        .or_else(|| v.as_str()?.parse().ok())
}

fn i64_field(v: &serde_json::Value, k: &str) -> Result<i64, String> {
    v.get(k)
        .and_then(json_i64)
        .ok_or_else(|| format!("missing {k}"))
}

fn f64_field(v: &serde_json::Value, k: &str) -> Result<f64, String> {
    let x = v.get(k).ok_or_else(|| format!("missing {k}"))?;
    if let Some(n) = x.as_f64() {
        return Ok(n);
    }
    if let Some(n) = x.as_i64() {
        return Ok(n as f64);
    }
    if let Some(s) = x.as_str() {
        return s.parse().map_err(|_| format!("bad {k}"));
    }
    Err(format!("bad {k}"))
}

pub fn hour_open_ms(ts_ms: i64) -> i64 {
    ts_ms - ts_ms.rem_euclid(3_600_000)
}

fn bar_open_ms_local(ts_ms: i64) -> i64 {
    ts_ms - ts_ms.rem_euclid(60_000)
}

/// UTC 00/08/16 ± N minutes. Clock, not a kill-zone.
pub fn funding_black_window(open_ms: i64, hours: &[u32], black_min: u32) -> bool {
    let minute = ((open_ms.div_euclid(60_000)).rem_euclid(24 * 60)) as i64;
    let black = black_min as i64;
    hours.iter().any(|&h| {
        let center = (h as i64) * 60;
        let mut d = (minute - center).abs();
        if d > 12 * 60 {
            d = 24 * 60 - d;
        }
        d <= black
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct RegimeSnap {
    pub liquidation_regime: &'static str,
    pub funding_crowding: &'static str,
    pub funding_black_window: bool,
    pub funding_rate: Option<f64>,
    pub basis_stress: &'static str,
    pub oi_divergence: &'static str,
    pub fee_regime: &'static str,
    pub latency_regime: &'static str,
    pub news_or_halt_regime: &'static str,
    pub liq_stream_missing: bool,
    pub oi_1h_chg: Option<f64>,
    pub liq_1m_notional: Option<f64>,
    /// Veto only. Never an entry.
    pub new_entries_blocked: bool,
}

pub fn evaluate_regime(
    cfg: &ContextConfig,
    inputs: &RegimeInputs,
    open_ms: i64,
    liq_seen: &[f64],
) -> RegimeSnap {
    let black = funding_black_window(
        open_ms,
        &cfg.funding_hours_utc,
        cfg.funding_black_window_minutes,
    );
    let funding_rate = nearest_funding(&inputs.funding_rate, open_ms);
    let hour = hour_open_ms(open_ms);
    let oi_now = inputs.oi_1h.get(&hour).copied();
    let oi_prev = inputs.oi_1h.range(..hour).next_back().map(|(_, v)| *v);
    let oi_1h_chg = match (oi_now, oi_prev) {
        (Some(now), Some(prev)) if prev != 0.0 => Some((now - prev) / prev),
        _ => None,
    };
    let liq_1m_notional = inputs.liq_1m.get(&open_ms).copied();
    let liq_stream_missing = !inputs.stream_present;
    let p95 = percentile_95(liq_seen);
    let liq_hit = match (liq_1m_notional, p95) {
        (Some(n), Some(th)) => n >= th && n > 0.0,
        _ => false,
    };
    let oi_hit = oi_1h_chg
        .map(|c| c <= cfg.liq_oi_1h_veto_pct)
        .unwrap_or(false);

    let liquidation_regime = if liq_stream_missing && oi_1h_chg.is_none() {
        "not_evaluated"
    } else if oi_hit || liq_hit {
        "true"
    } else if !liq_stream_missing || oi_1h_chg.is_some() {
        "false"
    } else {
        "not_evaluated"
    };

    let blocked = liquidation_regime == "true" || black;
    RegimeSnap {
        liquidation_regime,
        funding_crowding: "not_evaluated",
        funding_black_window: black,
        funding_rate,
        basis_stress: "not_evaluated",
        oi_divergence: "not_evaluated",
        fee_regime: "not_evaluated",
        latency_regime: "not_evaluated",
        news_or_halt_regime: "not_evaluated",
        liq_stream_missing,
        oi_1h_chg,
        liq_1m_notional,
        new_entries_blocked: blocked,
    }
}

fn nearest_funding(map: &BTreeMap<i64, f64>, open_ms: i64) -> Option<f64> {
    map.range(..=open_ms).next_back().map(|(_, v)| *v)
}

fn percentile_95(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut ys = xs.to_vec();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let k = ((ys.len() - 1) as f64 * 0.95).round() as usize;
    Some(ys[k.min(ys.len() - 1)])
}
