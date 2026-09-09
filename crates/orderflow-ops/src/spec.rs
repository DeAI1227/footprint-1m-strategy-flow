//! Tick / lot / contract spec watch. Change rebuilds THAT symbol only.

use orderflow_domain::Venue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpecSnapshot {
    pub venue: Venue,
    pub symbol: String,
    pub tick_size: f64,
    pub lot_size: f64,
    pub contract_mult: f64,
}

impl SpecSnapshot {
    pub fn norm_symbol(&self) -> String {
        self.symbol.to_ascii_uppercase()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpecChange {
    pub previous: SpecSnapshot,
    pub current: SpecSnapshot,
}

impl SpecChange {
    pub fn tick_changed(&self) -> bool {
        self.previous.tick_size != self.current.tick_size
            || self.previous.lot_size != self.current.lot_size
            || self.previous.contract_mult != self.current.contract_mult
    }
}

/// Per-instrument watch. SOL tick change must not rebuild SUI.
pub struct SpecWatch {
    last: std::collections::HashMap<(Venue, String), SpecSnapshot>,
}

impl Default for SpecWatch {
    fn default() -> Self {
        Self {
            last: std::collections::HashMap::new(),
        }
    }
}

impl SpecWatch {
    pub fn observe(&mut self, snap: SpecSnapshot) -> Option<SpecChange> {
        let key = (snap.venue, snap.symbol.to_ascii_uppercase());
        if let Some(prev) = self.last.get(&key) {
            if (prev.tick_size - snap.tick_size).abs() > f64::EPSILON
                || (prev.lot_size - snap.lot_size).abs() > f64::EPSILON
                || (prev.contract_mult - snap.contract_mult).abs() > f64::EPSILON
            {
                let change = SpecChange {
                    previous: prev.clone(),
                    current: snap.clone(),
                };
                self.last.insert(key, snap);
                return Some(change);
            }
            return None;
        }
        self.last.insert(key, snap);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(symbol: &str, tick: f64) -> SpecSnapshot {
        SpecSnapshot {
            venue: Venue::Okx,
            symbol: symbol.into(),
            tick_size: tick,
            lot_size: 1.0,
            contract_mult: 1.0,
        }
    }

    #[test]
    fn sol_tick_does_not_touch_sui() {
        let mut w = SpecWatch::default();
        assert!(w.observe(snap("SOL", 0.01)).is_none());
        assert!(w.observe(snap("SUI", 0.0001)).is_none());
        let ch = w.observe(snap("SOL", 0.02)).unwrap();
        assert_eq!(ch.current.symbol, "SOL");
        assert!(ch.tick_changed());
        assert!(w.observe(snap("SUI", 0.0001)).is_none());
    }
}
