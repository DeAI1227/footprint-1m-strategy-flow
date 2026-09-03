//! Stage 0 stub. Per-venue L2 is stage 3.
//! Unhealthy book → that venue's DOM scripts `not_evaluated`. Never fill OKX from another book.

use orderflow_domain::QualityVector;

pub const WIRED: bool = false;

pub fn unhealthy_book_invalidates_dom() -> QualityVector {
    QualityVector {
        okx_book_ok: false,
        binance_book_ok: false,
        bybit_book_ok: false,
        ..QualityVector::default()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stub_is_not_wired() {
        assert!(!super::WIRED);
    }
}
