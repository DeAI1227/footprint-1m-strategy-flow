//! Stage 0 stub. Three public WS adapters land in stage 1 / 1b.
//! Adapters must emit `taker_buy` / `taker_sell` only. Do not mix venue prices.
//! One venue stall must not block the other two.

use orderflow_domain::TakerSide;

pub const WIRED: bool = false;

pub fn expected_taker_sides() -> [TakerSide; 2] {
    [TakerSide::Buy, TakerSide::Sell]
}

pub mod binance {
    pub const VENUE: &str = "binance";
}

pub mod okx {
    pub const VENUE: &str = "okx";
}

pub mod bybit {
    pub const VENUE: &str = "bybit";
    /// Bybit taker-side golden tests are required before this module may parse live trades.
    pub const TAKER_GOLDEN_REQUIRED: bool = true;
}

#[cfg(test)]
mod tests {
    #[test]
    fn stub_is_not_wired() {
        assert!(!super::WIRED);
        assert!(super::bybit::TAKER_GOLDEN_REQUIRED);
    }
}
