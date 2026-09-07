//! Per-venue 1m footprint engine. Compose with [`orderflow_clock::BarCutter`] so
//! closed bars stay immutable. One engine per venue; never sum volumes.

use std::collections::BTreeMap;

use orderflow_clock::{BarCutter, CutEvent};
use orderflow_domain::{bar_open_ms, Bar1m, Trade, Venue};

use crate::bucket::{percentile, session_key, Session};
use crate::config::FootprintConfig;
use crate::features::{apply_trade, compute, CellVol, FootprintBar};

pub struct ClosedFootprint {
    pub bar: Bar1m,
    pub footprint: FootprintBar,
}

struct FormingFp {
    open_ms: i64,
    cells: BTreeMap<i64, CellVol>,
    open: f64,
    close: f64,
    first_ts: i64,
    last_ts: i64,
    trade_count: u32,
}

pub struct FootprintEngine {
    venue: Venue,
    cutter: BarCutter,
    cfg: FootprintConfig,
    forming: Option<FormingFp>,
    cvd: f64,
    prev_poc: Option<f64>,
    session_key: Option<(i64, Session)>,
    session_side_vols: Vec<f64>,
}

impl FootprintEngine {
    pub fn new(venue: Venue, symbol: impl Into<String>, cfg: FootprintConfig) -> Self {
        let symbol = symbol.into();
        Self {
            venue,
            cutter: BarCutter::new(venue, symbol),
            cfg,
            forming: None,
            cvd: 0.0,
            prev_poc: None,
            session_key: None,
            session_side_vols: Vec::new(),
        }
    }

    pub fn venue(&self) -> Venue {
        self.venue
    }

    pub fn config(&self) -> &FootprintConfig {
        &self.cfg
    }

    pub fn cutter(&self) -> &BarCutter {
        &self.cutter
    }

    pub fn cvd(&self) -> f64 {
        self.cvd
    }

    pub fn mark_reconnect(&mut self) {
        self.cutter.mark_reconnect();
    }

    /// Push one normalized trade. Closed matrices are frozen; late trades do not rewrite them.
    pub fn push(&mut self, trade: &Trade) -> Vec<ClosedFootprint> {
        let late_before = self.cutter.quality().late_trade;
        let events = self.cutter.push(trade);
        let late_after = self.cutter.quality().late_trade;
        if late_after > late_before {
            return Vec::new();
        }
        let mut out = Vec::new();
        for ev in events {
            match ev {
                CutEvent::Closed(bar) => {
                    if let Some(footprint) = self.freeze_forming() {
                        out.push(ClosedFootprint { bar, footprint });
                    }
                }
                CutEvent::Forming(_) => {
                    self.apply_to_forming(trade);
                }
            }
        }
        out
    }

    fn apply_to_forming(&mut self, trade: &Trade) {
        let open_ms = bar_open_ms(trade.event_ts_ms);
        match self.forming.as_mut() {
            Some(f) if f.open_ms == open_ms => {
                if trade.event_ts_ms < f.first_ts {
                    f.first_ts = trade.event_ts_ms;
                    f.open = trade.price;
                }
                if trade.event_ts_ms >= f.last_ts {
                    f.last_ts = trade.event_ts_ms;
                    f.close = trade.price;
                }
                apply_trade(
                    &mut f.cells,
                    trade.price,
                    trade.size,
                    trade.taker_side,
                    self.cfg.bucket,
                );
                f.trade_count += 1;
            }
            _ => {
                let mut cells = BTreeMap::new();
                apply_trade(
                    &mut cells,
                    trade.price,
                    trade.size,
                    trade.taker_side,
                    self.cfg.bucket,
                );
                self.forming = Some(FormingFp {
                    open_ms,
                    cells,
                    open: trade.price,
                    close: trade.price,
                    first_ts: trade.event_ts_ms,
                    last_ts: trade.event_ts_ms,
                    trade_count: 1,
                });
            }
        }
    }

    fn freeze_forming(&mut self) -> Option<FootprintBar> {
        let f = self.forming.take()?;
        let sk = session_key(f.open_ms);
        if self.session_key != Some(sk) {
            self.cvd = 0.0;
            self.session_side_vols.clear();
            self.session_key = Some(sk);
        }
        let min_vol = percentile(&self.session_side_vols, 25.0);
        let computed = compute(&f.cells, &self.cfg, min_vol, f.open, f.close)?;
        let delta = computed.ask_vol - computed.bid_vol;
        self.cvd += delta;
        let aligned_poc = match (self.prev_poc, computed.poc) {
            (Some(a), Some(b)) => (a - b).abs() < self.cfg.bucket * 0.5,
            _ => false,
        };
        self.prev_poc = computed.poc;
        self.session_side_vols
            .extend_from_slice(&computed.nonempty_side_vols);

        let high = computed.cells.last().map(|c| c.price).unwrap_or(f.close);
        let low = computed.cells.first().map(|c| c.price).unwrap_or(f.open);

        Some(FootprintBar {
            venue: self.venue,
            symbol: self.cfg.symbol.clone(),
            open_ms: f.open_ms,
            bucket: self.cfg.bucket,
            session: sk.1,
            cells: computed.cells,
            open: f.open,
            high,
            low,
            close: f.close,
            bid_vol: computed.bid_vol,
            ask_vol: computed.ask_vol,
            delta,
            cvd: self.cvd,
            trade_count: f.trade_count,
            tape_speed: f.trade_count as f64,
            min_volume: min_vol,
            min_volume_rule: self.cfg.min_imbalance_volume_rule.clone(),
            bar_up: f.close > f.open,
            bar_down: f.close < f.open,
            poc: computed.poc,
            va_low: computed.va_low,
            va_high: computed.va_high,
            va_ok: computed.va_ok,
            va_width: computed.va_width,
            aligned_poc,
            unfinished_high: computed.unfinished_high,
            unfinished_low: computed.unfinished_low,
            finished_high: computed.finished_high,
            finished_low: computed.finished_low,
            unfinished_is_entry: false,
            chaos: computed.chaos,
            record: computed.record,
            dale: computed.dale,
            valtos: computed.valtos,
            absorption: "not_evaluated",
            script_f: "not_evaluated",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderflow_domain::TakerSide;

    fn t(venue: Venue, ts: i64, px: f64, sz: f64, side: TakerSide, id: &str) -> Trade {
        Trade {
            venue,
            symbol: "SOL".into(),
            trade_id: Some(id.into()),
            event_ts_ms: ts,
            recv_ts_ms: ts,
            processed_ts_ms: ts,
            price: px,
            size: sz,
            taker_side: side,
        }
    }

    #[test]
    fn late_trade_does_not_rewrite_closed_matrix() {
        let cfg = FootprintConfig::golden_sol();
        let mut eng = FootprintEngine::new(Venue::Okx, "SOL", cfg);
        eng.push(&t(Venue::Okx, 1_000, 100.00, 2.0, TakerSide::Sell, "1"));
        let closed = eng.push(&t(Venue::Okx, 60_000, 100.01, 1.0, TakerSide::Buy, "2"));
        assert_eq!(closed.len(), 1);
        let bid = closed[0].footprint.bid_vol;
        let ask = closed[0].footprint.ask_vol;
        let late = eng.push(&t(Venue::Okx, 2_000, 999.0, 50.0, TakerSide::Buy, "late"));
        assert!(late.is_empty());
        assert_eq!(eng.cutter().quality().late_trade, 1);
        assert_eq!(bid, 2.0);
        assert_eq!(ask, 0.0);
    }
}
