//! Bid/ask ladders keyed by integer ticks. Size 0 deletes the level.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Clone, Default)]
pub struct Ladder {
    /// Tick key → size. Bids best = max key; asks best = min key.
    levels: BTreeMap<i64, f64>,
}

impl Ladder {
    pub fn apply(&mut self, tick: i64, size: f64) {
        if size <= 0.0 {
            self.levels.remove(&tick);
        } else {
            self.levels.insert(tick, size);
        }
    }

    pub fn clear(&mut self) {
        self.levels.clear();
    }

    pub fn len(&self) -> usize {
        self.levels.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    pub fn size_at(&self, tick: i64) -> f64 {
        self.levels.get(&tick).copied().unwrap_or(0.0)
    }

    pub fn best_tick(&self, side: Side) -> Option<i64> {
        match side {
            Side::Bid => self.levels.keys().next_back().copied(),
            Side::Ask => self.levels.keys().next().copied(),
        }
    }

    /// Top `n` levels from the touch, best first.
    pub fn top(&self, side: Side, n: usize) -> Vec<(i64, f64)> {
        match side {
            Side::Bid => self
                .levels
                .iter()
                .rev()
                .take(n)
                .map(|(k, v)| (*k, *v))
                .collect(),
            Side::Ask => self.levels.iter().take(n).map(|(k, v)| (*k, *v)).collect(),
        }
    }

    pub fn median_size(&self, side: Side, n: usize) -> f64 {
        let mut xs: Vec<f64> = self.top(side, n).into_iter().map(|(_, s)| s).collect();
        if xs.is_empty() {
            return 0.0;
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        xs[xs.len() / 2]
    }
}

pub fn tick_of(px: f64, tick_sz: f64) -> i64 {
    (px / tick_sz + 1e-12).round() as i64
}

pub fn px_of(tick: i64, tick_sz: f64) -> f64 {
    tick as f64 * tick_sz
}

pub fn tick_decimals(tick_sz: f64) -> usize {
    if tick_sz >= 1.0 {
        return 0;
    }
    let mut d = 0usize;
    let mut x = tick_sz;
    while x < 0.5 && d < 12 {
        x *= 10.0;
        d += 1;
    }
    d
}

pub fn format_px(tick: i64, tick_sz: f64) -> String {
    let p = px_of(tick, tick_sz);
    let d = tick_decimals(tick_sz);
    format!("{p:.d$}")
}

pub fn format_sz(sz: f64) -> String {
    if (sz - sz.round()).abs() < 1e-9 {
        format!("{}", sz.round() as i64)
    } else {
        let mut s = format!("{sz:.8}");
        while s.contains('.') && s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
        s
    }
}

/// OKX historical CRC32 (signed). After 2026-06-23 production checksum is always 0
/// and must be ignored; seqId/prevSeqId is the integrity path.
pub fn okx_checksum(bids: &[(String, String)], asks: &[(String, String)]) -> i32 {
    let mut parts = Vec::new();
    for i in 0..25 {
        if let Some((p, s)) = bids.get(i) {
            parts.push(format!("{p}:{s}"));
        }
        if let Some((p, s)) = asks.get(i) {
            parts.push(format!("{p}:{s}"));
        }
    }
    let s = parts.join(":");
    crc32fast::hash(s.as_bytes()) as i32
}
