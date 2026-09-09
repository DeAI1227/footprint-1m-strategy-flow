//! Local orders, positions, fills. Exchange remains the truth on reconcile.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::intent::Side;
use crate::sim::Fill;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub qty: f64,
    pub avg_px: f64,
    pub realized_pnl: f64,
}

impl Position {
    pub fn notional(&self, mark: f64) -> f64 {
        self.qty.abs() * mark
    }

    pub fn unrealized(&self, mark: f64) -> f64 {
        if self.qty == 0.0 {
            return 0.0;
        }
        (mark - self.avg_px) * self.qty
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Ledger {
    pub working: BTreeMap<String, crate::sim::WorkingOrder>,
    pub positions: BTreeMap<String, Position>,
    pub fills: Vec<Fill>,
    pub seq: u64,
}

impl Ledger {
    pub fn next_client_id(&mut self, symbol: &str, script: &str, now_ms: i64) -> String {
        self.seq += 1;
        format!("of-{symbol}-{script}-{now_ms}-{}", self.seq)
    }

    pub fn position(&self, symbol: &str) -> f64 {
        self.positions.get(symbol).map(|p| p.qty).unwrap_or(0.0)
    }

    pub fn apply_fill(&mut self, fill: Fill) {
        let pos = self
            .positions
            .entry(fill.symbol.clone())
            .or_insert(Position {
                symbol: fill.symbol.clone(),
                qty: 0.0,
                avg_px: 0.0,
                realized_pnl: 0.0,
            });
        let signed = match fill.side {
            Side::Buy => fill.qty,
            Side::Sell => -fill.qty,
        };
        let new_qty = pos.qty + signed;
        if pos.qty == 0.0
            || (pos.qty > 0.0 && signed > 0.0)
            || (pos.qty < 0.0 && signed < 0.0)
        {
            let tot = pos.qty.abs() + fill.qty;
            pos.avg_px = if tot > 0.0 {
                (pos.avg_px * pos.qty.abs() + fill.price * fill.qty) / tot
            } else {
                fill.price
            };
        } else {
            let closed = fill.qty.min(pos.qty.abs());
            pos.realized_pnl += (fill.price - pos.avg_px) * closed * pos.qty.signum();
        }
        pos.qty = new_qty;
        if pos.qty.abs() < 1e-12 {
            pos.qty = 0.0;
            pos.avg_px = 0.0;
        }
        if let Some(w) = self.working.get_mut(&fill.client_id) {
            if w.done() {
                self.working.remove(&fill.client_id);
            }
        }
        self.fills.push(fill);
    }

    pub fn realized_pnl(&self) -> f64 {
        self.positions.values().map(|p| p.realized_pnl).sum()
    }

    pub fn unrealized(&self, marks: &BTreeMap<String, f64>) -> f64 {
        self.positions
            .values()
            .map(|p| p.unrealized(*marks.get(&p.symbol).unwrap_or(&p.avg_px)))
            .sum()
    }

    pub fn snapshot(&self) -> LedgerSnap {
        LedgerSnap {
            working: self.working.values().cloned().collect(),
            positions: self.positions.values().cloned().collect(),
            fill_n: self.fills.len(),
            realized_pnl: self.realized_pnl(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerSnap {
    pub working: Vec<crate::sim::WorkingOrder>,
    pub positions: Vec<Position>,
    pub fill_n: usize,
    pub realized_pnl: f64,
}
