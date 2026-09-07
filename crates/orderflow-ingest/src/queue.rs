//! Bounded per-venue inboxes. Overflow marks **that** venue `gap` and drops
//! the trade. The push path never waits, so a stalled Binance/Bybit consumer
//! cannot block OKX.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use orderflow_domain::{QualityVector, Trade, Venue};

/// Default live-WS capacity. Replay uses a larger cap so dumps are not truncated.
pub const DEFAULT_CAP: usize = 8192;

pub struct BoundedInbox {
    venue: Venue,
    cap: usize,
    q: Mutex<VecDeque<Trade>>,
    dropped: AtomicU64,
    gap: AtomicBool,
}

impl BoundedInbox {
    pub fn new(venue: Venue, cap: usize) -> Self {
        assert!(cap > 0, "inbox cap must be > 0");
        Self {
            venue,
            cap,
            q: Mutex::new(VecDeque::with_capacity(cap.min(1024))),
            dropped: AtomicU64::new(0),
            gap: AtomicBool::new(false),
        }
    }

    pub fn venue(&self) -> Venue {
        self.venue
    }

    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Never blocks. Returns false on overflow (this venue only).
    pub fn try_push(&self, trade: Trade) -> bool {
        assert_eq!(trade.venue, self.venue, "inbox is per-venue; do not mix");
        let mut q = self.q.lock().expect("inbox mutex");
        if q.len() >= self.cap {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            self.gap.store(true, Ordering::Relaxed);
            return false;
        }
        q.push_back(trade);
        true
    }

    pub fn pop(&self) -> Option<Trade> {
        self.q.lock().expect("inbox mutex").pop_front()
    }

    pub fn drain(&self, max: usize) -> Vec<Trade> {
        let mut out = Vec::with_capacity(max.min(64));
        let mut q = self.q.lock().expect("inbox mutex");
        while out.len() < max {
            match q.pop_front() {
                Some(t) => out.push(t),
                None => break,
            }
        }
        out
    }

    pub fn len(&self) -> usize {
        self.q.lock().expect("inbox mutex").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn is_gap(&self) -> bool {
        self.gap.load(Ordering::Relaxed)
    }

    pub fn apply_gap(&self, quality: &mut QualityVector) {
        if self.is_gap() {
            quality.mark_gap(self.venue);
        }
    }
}

/// Three independent lanes. A full Binance lane does not stop OKX `try_push`.
pub struct ThreeLanes {
    pub okx: BoundedInbox,
    pub binance: BoundedInbox,
    pub bybit: BoundedInbox,
}

impl ThreeLanes {
    pub fn with_cap(cap: usize) -> Self {
        Self {
            okx: BoundedInbox::new(Venue::Okx, cap),
            binance: BoundedInbox::new(Venue::Binance, cap),
            bybit: BoundedInbox::new(Venue::Bybit, cap),
        }
    }

    pub fn inbox(&self, venue: Venue) -> &BoundedInbox {
        match venue {
            Venue::Okx => &self.okx,
            Venue::Binance => &self.binance,
            Venue::Bybit => &self.bybit,
        }
    }

    pub fn try_push(&self, trade: Trade) -> bool {
        self.inbox(trade.venue).try_push(trade)
    }

    pub fn apply_gaps(&self, quality: &mut QualityVector) {
        self.okx.apply_gap(quality);
        self.binance.apply_gap(quality);
        self.bybit.apply_gap(quality);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_trade(venue: Venue, i: i64) -> Trade {
        Trade {
            venue,
            symbol: "SOL".into(),
            trade_id: Some(format!("{venue:?}-{i}")),
            event_ts_ms: i,
            recv_ts_ms: i,
            processed_ts_ms: i,
            price: 100.0,
            size: 1.0,
            taker_side: orderflow_domain::TakerSide::Buy,
        }
    }

    #[test]
    fn full_binance_queue_does_not_block_okx() {
        let lanes = ThreeLanes {
            okx: BoundedInbox::new(Venue::Okx, 8),
            binance: BoundedInbox::new(Venue::Binance, 2),
            bybit: BoundedInbox::new(Venue::Bybit, 2),
        };
        for i in 0..10 {
            let _ = lanes.try_push(dummy_trade(Venue::Binance, i));
        }
        assert!(lanes.binance.is_gap());
        assert_eq!(lanes.binance.dropped(), 8);
        assert_eq!(lanes.binance.len(), 2);

        for i in 0..5 {
            assert!(
                lanes.try_push(dummy_trade(Venue::Okx, i)),
                "OKX must still accept while Binance is overflowing"
            );
        }
        assert!(!lanes.okx.is_gap());
        assert_eq!(lanes.okx.dropped(), 0);
        assert_eq!(lanes.okx.len(), 5);

        let mut q = QualityVector::default();
        lanes.apply_gaps(&mut q);
        assert!(q.binance_gap);
        assert!(!q.okx_gap);
        assert!(!q.bybit_gap);
    }

    #[test]
    fn bybit_overflow_does_not_mark_okx_gap() {
        let lanes = ThreeLanes::with_cap(1);
        assert!(lanes.try_push(dummy_trade(Venue::Bybit, 1)));
        assert!(!lanes.try_push(dummy_trade(Venue::Bybit, 2)));
        assert!(lanes.try_push(dummy_trade(Venue::Okx, 1)));
        let mut q = QualityVector::default();
        lanes.apply_gaps(&mut q);
        assert!(q.bybit_gap);
        assert!(!q.okx_gap);
        assert!(!q.binance_gap);
    }
}
