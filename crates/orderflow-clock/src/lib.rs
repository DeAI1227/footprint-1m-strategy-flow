//! Exchange-event 1m clock and bar cutter.
//!
//! Contract:
//! - bars are `[t, t+60_000)` on venue **event** time (not local sleep)
//! - only `Closed` bars for entries
//! - `Forming` = cancel / risk / warning
//! - **never rewrite a closed bar**; late trades only bump `QualityVector::late_trade`
//! - out-of-order trades inside a forming bar still apply by event time
//! - reconnect does not reopen closed bars

use orderflow_domain::{
    bar_open_ms, Bar1m, BarState, QualityVector, Trade, Venue, BAR_INTERVAL_MS,
};

pub const WIRED: bool = true;

/// One venue + symbol stream of 1m bars.
pub struct BarCutter {
    venue: Venue,
    symbol: String,
    forming: Option<Bar1m>,
    /// Highest closed bar open_ms. Late trades for any open_ms <= this are rejected.
    last_closed_open_ms: Option<i64>,
    quality: QualityVector,
    last_event_ts_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum CutEvent {
    /// Forming bar updated (not an entry signal).
    Forming(Bar1m),
    /// Immutable closed bar.
    Closed(Bar1m),
}

impl BarCutter {
    pub fn new(venue: Venue, symbol: impl Into<String>) -> Self {
        Self {
            venue,
            symbol: symbol.into(),
            forming: None,
            last_closed_open_ms: None,
            quality: QualityVector::default(),
            last_event_ts_ms: None,
        }
    }

    pub fn quality(&self) -> &QualityVector {
        &self.quality
    }

    pub fn forming(&self) -> Option<&Bar1m> {
        self.forming.as_ref()
    }

    pub fn mark_reconnect(&mut self) {
        self.quality.reconnect += 1;
        // Do not clear closed history. Do not reopen bars.
    }

    /// Push one normalized trade. Returns zero or more events (closes then forming).
    pub fn push(&mut self, trade: &Trade) -> Vec<CutEvent> {
        assert_eq!(trade.venue, self.venue);
        self.quality.trades_seen += 1;
        let mut out = Vec::new();
        let open = trade.bar_open_ms();

        if let Some(last) = self.last_event_ts_ms {
            if trade.event_ts_ms < last {
                self.quality.out_of_order += 1;
            }
        }
        self.last_event_ts_ms = Some(
            self.last_event_ts_ms
                .map(|t| t.max(trade.event_ts_ms))
                .unwrap_or(trade.event_ts_ms),
        );

        // Late: belongs to an already-closed minute.
        if let Some(last_closed) = self.last_closed_open_ms {
            if open <= last_closed {
                self.quality.late_trade += 1;
                return out;
            }
        }

        // Close forming if trade belongs to a later minute.
        if let Some(forming) = self.forming.take() {
            if open > forming.open_ms {
                let gap = ((open - forming.open_ms) / BAR_INTERVAL_MS - 1).max(0) as u32;
                self.quality.gap_minutes += gap;
                let closed = forming.into_closed();
                self.last_closed_open_ms = Some(closed.open_ms);
                self.quality.bars_closed += 1;
                out.push(CutEvent::Closed(closed));
            } else if open < forming.open_ms {
                // Trade older than current forming but newer than last_closed — still late
                // relative to stream progress (we already advanced the clock).
                self.forming = Some(forming);
                self.quality.late_trade += 1;
                return out;
            } else {
                // same minute
                self.forming = Some(forming);
            }
        }

        match self.forming.as_mut() {
            Some(bar) => {
                apply_trade(bar, trade);
                out.push(CutEvent::Forming(bar.clone()));
            }
            None => {
                let bar = new_forming(self.venue, &self.symbol, trade);
                out.push(CutEvent::Forming(bar.clone()));
                self.forming = Some(bar);
            }
        }
        out
    }

    /// Force-close the forming bar (e.g. end of replay). Does nothing if empty.
    pub fn flush(&mut self) -> Option<Bar1m> {
        let forming = self.forming.take()?;
        let closed = forming.into_closed();
        self.last_closed_open_ms = Some(closed.open_ms);
        self.quality.bars_closed += 1;
        Some(closed)
    }
}

fn new_forming(venue: Venue, symbol: &str, trade: &Trade) -> Bar1m {
    let mut bar = Bar1m {
        venue,
        symbol: symbol.to_string(),
        open_ms: trade.bar_open_ms(),
        state: BarState::Forming,
        open: trade.price,
        high: trade.price,
        low: trade.price,
        close: trade.price,
        bid_vol: 0.0,
        ask_vol: 0.0,
        trade_count: 0,
        first_trade_ts_ms: trade.event_ts_ms,
        last_trade_ts_ms: trade.event_ts_ms,
    };
    apply_trade(&mut bar, trade);
    bar
}

fn apply_trade(bar: &mut Bar1m, trade: &Trade) {
    debug_assert_eq!(bar.open_ms, trade.bar_open_ms());
    debug_assert!(matches!(bar.state, BarState::Forming));
    if trade.event_ts_ms < bar.first_trade_ts_ms {
        bar.first_trade_ts_ms = trade.event_ts_ms;
        bar.open = trade.price;
    }
    if trade.event_ts_ms >= bar.last_trade_ts_ms {
        bar.last_trade_ts_ms = trade.event_ts_ms;
        bar.close = trade.price;
    }
    bar.high = bar.high.max(trade.price);
    bar.low = bar.low.min(trade.price);
    match trade.taker_side {
        orderflow_domain::TakerSide::Buy => bar.ask_vol += trade.size,
        orderflow_domain::TakerSide::Sell => bar.bid_vol += trade.size,
    }
    bar.trade_count += 1;
}

pub fn bar_close_not_wired() -> &'static str {
    "orderflow-clock: stage 1 wired; use BarCutter"
}

pub fn entries_need_closed_bar(bar: &Bar1m) -> bool {
    matches!(bar.state, BarState::Closed)
}

/// Convenience: open minute for an event timestamp.
pub fn open_of(event_ts_ms: i64) -> i64 {
    bar_open_ms(event_ts_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderflow_domain::TakerSide;

    fn trade(ts: i64, px: f64, sz: f64, side: TakerSide) -> Trade {
        Trade {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            trade_id: Some(format!("t{ts}")),
            event_ts_ms: ts,
            recv_ts_ms: ts,
            processed_ts_ms: ts,
            price: px,
            size: sz,
            taker_side: side,
        }
    }

    #[test]
    fn wired_stage_1() {
        assert!(WIRED);
        assert!(bar_close_not_wired().contains("stage 1"));
    }

    #[test]
    fn closes_on_next_minute_and_never_rewrites() {
        let mut c = BarCutter::new(Venue::Okx, "SOL");
        let e1 = c.push(&trade(1_000, 100.0, 1.0, TakerSide::Buy));
        assert!(matches!(e1[0], CutEvent::Forming(_)));
        let e2 = c.push(&trade(60_000, 101.0, 2.0, TakerSide::Sell));
        assert!(matches!(e2[0], CutEvent::Closed(ref b) if b.open_ms == 0 && b.ask_vol == 1.0));
        assert!(matches!(e2[1], CutEvent::Forming(ref b) if b.open_ms == 60_000));
        let closed_open = match &e2[0] {
            CutEvent::Closed(b) => b.open,
            _ => panic!(),
        };
        // Late trade into minute 0 must not change the closed bar.
        let late = c.push(&trade(30_000, 999.0, 50.0, TakerSide::Buy));
        assert!(late.is_empty());
        assert_eq!(c.quality().late_trade, 1);
        // Closed snapshot we already emitted stays at open 100 — cutter does not hold it.
        assert_eq!(closed_open, 100.0);
    }

    #[test]
    fn out_of_order_inside_forming_updates_by_event_time() {
        let mut c = BarCutter::new(Venue::Okx, "SOL");
        c.push(&trade(10_000, 100.0, 1.0, TakerSide::Buy));
        // Earlier event arrives later (OOO).
        c.push(&trade(5_000, 99.0, 1.0, TakerSide::Sell));
        assert_eq!(c.quality().out_of_order, 1);
        let f = c.forming().unwrap();
        assert_eq!(f.open, 99.0);
        assert_eq!(f.close, 100.0);
        assert_eq!(f.low, 99.0);
        assert_eq!(f.high, 100.0);
        assert_eq!(f.bid_vol, 1.0);
        assert_eq!(f.ask_vol, 1.0);
        assert_eq!(f.trade_count, 2);
    }

    #[test]
    fn reconnect_does_not_reopen_closed() {
        let mut c = BarCutter::new(Venue::Okx, "SOL");
        c.push(&trade(1_000, 100.0, 1.0, TakerSide::Buy));
        c.push(&trade(60_000, 101.0, 1.0, TakerSide::Buy));
        assert_eq!(c.quality().bars_closed, 1);
        c.mark_reconnect();
        assert_eq!(c.quality().reconnect, 1);
        // Trade for the already-closed minute still late.
        assert!(c.push(&trade(2_000, 50.0, 9.0, TakerSide::Sell)).is_empty());
        assert_eq!(c.quality().late_trade, 1);
    }

    #[test]
    fn flush_closes_forming() {
        let mut c = BarCutter::new(Venue::Okx, "SOL");
        c.push(&trade(1_000, 100.0, 1.0, TakerSide::Buy));
        let b = c.flush().unwrap();
        assert!(matches!(b.state, BarState::Closed));
        assert!(b.entries_allowed());
        assert!(c.flush().is_none());
    }

    #[test]
    fn gap_minutes_counted_when_skipping() {
        let mut c = BarCutter::new(Venue::Okx, "SOL");
        c.push(&trade(1_000, 100.0, 1.0, TakerSide::Buy));
        // Jump three minutes ahead from open 0 → two missing middle minutes (60s, 120s).
        c.push(&trade(180_000, 102.0, 1.0, TakerSide::Buy));
        assert_eq!(c.quality().gap_minutes, 2);
        assert_eq!(c.quality().bars_closed, 1);
    }
}
