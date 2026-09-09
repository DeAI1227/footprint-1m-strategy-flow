//! Funding black window is clock, not a kill-zone. Reuses Stage 4 helper.

use orderflow_context::funding_black_window;
use orderflow_domain::SymbolParams;

pub fn funding_black_at(open_ms: i64, hours: &[u32], black_min: u32) -> bool {
    funding_black_window(open_ms, hours, black_min)
}

pub fn funding_black_for_symbol(open_ms: i64, p: &SymbolParams) -> bool {
    funding_black_at(
        open_ms,
        &p.funding_hours_utc,
        p.funding_black_window_minutes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_zero_is_black() {
        let hours = [0u32, 8, 16];
        assert!(funding_black_at(0, &hours, 15));
        assert!(!funding_black_at(20 * 60 * 1000, &hours, 15));
    }
}
