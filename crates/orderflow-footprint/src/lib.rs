//! Stage 0 stub. Per-venue 1m footprint matrix is stage 2.
//!
//! Hard rules when this crate is wired:
//! - one matrix per venue; never sum Binance + OKX + Bybit volume
//! - diagonal imbalance; ignore zeros; no Market Profile / VWAP / Naked POC
//! - Python must not build a second production matrix
//! - unfinished auction is display-only, not an entry

pub const WIRED: bool = false;

pub fn unfinished_is_entry() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn stub_is_not_wired() {
        assert!(!super::WIRED);
        assert!(!super::unfinished_is_entry());
    }
}
