//! Missed closed 1m: stop opens, never stop flatten/risk.

use orderflow_domain::Venue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissedBarEvent {
    pub venue: Venue,
    pub symbol: String,
    pub expected_open_ms: i64,
    pub observed_open_ms: i64,
}

pub struct MissedBarDetector {
    last_closed_open_ms: std::collections::HashMap<(Venue, String), i64>,
}

impl Default for MissedBarDetector {
    fn default() -> Self {
        Self {
            last_closed_open_ms: std::collections::HashMap::new(),
        }
    }
}

impl MissedBarDetector {
    /// Call on each closed 1m. If the previous closed bar is not exactly 60s earlier
    /// (after the first observation), this is a miss.
    pub fn on_closed(
        &mut self,
        venue: Venue,
        symbol: &str,
        open_ms: i64,
    ) -> Option<MissedBarEvent> {
        let key = (venue, symbol.to_ascii_uppercase());
        let miss = if let Some(prev) = self.last_closed_open_ms.get(&key) {
            if open_ms - *prev != 60_000 {
                Some(MissedBarEvent {
                    venue,
                    symbol: key.1.clone(),
                    expected_open_ms: *prev + 60_000,
                    observed_open_ms: open_ms,
                })
            } else {
                None
            }
        } else {
            None
        };
        self.last_closed_open_ms.insert(key, open_ms);
        miss
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_is_miss() {
        let mut d = MissedBarDetector::default();
        assert!(d.on_closed(Venue::Okx, "SOL", 1_000_000).is_none());
        let miss = d.on_closed(Venue::Okx, "SOL", 1_000_000 + 120_000).unwrap();
        assert_eq!(miss.expected_open_ms, 1_000_000 + 60_000);
        assert_eq!(miss.symbol, "SOL");
    }
}
