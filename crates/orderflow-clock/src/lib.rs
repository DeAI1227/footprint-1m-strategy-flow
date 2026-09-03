//! Stage 0 stub. Exchange-event 1m clock is stage 1.
//!
//! Contract (not implemented here):
//! - bars are `[t, t+60)` on venue event time
//! - only `Closed` bars for entries
//! - `Forming` = cancel / risk / warning
//! - never rewrite a closed bar

use orderflow_domain::{Bar1m, BarState};

pub const WIRED: bool = false;

pub fn bar_close_not_wired() -> &'static str {
    "orderflow-clock: stage 0 stub; 1m [t, t+60) lands in stage 1"
}

pub fn entries_need_closed_bar(bar: &Bar1m) -> bool {
    matches!(bar.state, BarState::Closed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderflow_domain::Venue;

    #[test]
    fn stub_is_not_wired() {
        assert!(!WIRED);
        assert!(bar_close_not_wired().contains("stage 0"));
        let forming = Bar1m {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            open_ms: 0,
            state: BarState::Forming,
        };
        assert!(!entries_need_closed_bar(&forming));
    }
}
