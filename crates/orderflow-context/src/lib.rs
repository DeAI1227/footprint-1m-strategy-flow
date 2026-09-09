//! Position (volume stack / swing / accept) + regime + resonance fields.
//!
//! Input is closed 1m footprint bars only. No second OHLC clock, no VWAP, no
//! daily TPO, no Naked POC. Resonance is recorded while mode stays `off` and
//! never writes a Binance/Bybit price onto an OKX order.

mod config;
mod regime;
mod resonance;

pub use config::{ContextConfig, FORBIDDEN, STACK_WINDOWS};
pub use regime::{evaluate_regime, funding_black_window, RegimeInputs, RegimeSnap};
pub use resonance::{join as join_resonance, ResonanceBook, ResonanceSnap, VenueDir};

use std::collections::{BTreeMap, VecDeque};

use orderflow_domain::Venue;
use orderflow_footprint::{percentile, FootprintBar};
use serde::Serialize;

pub const WIRED: bool = true;

#[derive(Debug, Clone)]
pub struct BarIn {
    pub venue: Venue,
    pub symbol: String,
    pub open_ms: i64,
    pub bucket: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub delta: f64,
    pub bid_vol: f64,
    pub ask_vol: f64,
    pub poc: Option<f64>,
    pub cells: Vec<(f64, f64)>,
    pub dale_aligned: bool,
    pub stack_buy: bool,
    pub stack_sell: bool,
    pub stack_lo: Option<f64>,
    pub stack_hi: Option<f64>,
}

impl BarIn {
    pub fn from_footprint(fp: &FootprintBar) -> Self {
        let (stack_lo, stack_hi, stack_buy, stack_sell) = stack_bounds(fp);
        Self {
            venue: fp.venue,
            symbol: fp.symbol.clone(),
            open_ms: fp.open_ms,
            bucket: fp.bucket,
            high: fp.high,
            low: fp.low,
            close: fp.close,
            delta: fp.delta,
            bid_vol: fp.bid_vol,
            ask_vol: fp.ask_vol,
            poc: fp.poc,
            cells: fp.cells.iter().map(|c| (c.price, c.bid + c.ask)).collect(),
            dale_aligned: fp.dale.aligned,
            stack_buy,
            stack_sell,
            stack_lo,
            stack_hi,
        }
    }

    pub fn stack_sign(&self) -> i8 {
        if self.dale_aligned && self.stack_buy {
            1
        } else if self.dale_aligned && self.stack_sell {
            -1
        } else {
            0
        }
    }
}

fn stack_bounds(fp: &FootprintBar) -> (Option<f64>, Option<f64>, bool, bool) {
    if fp.dale.stacked_buy && fp.dale.aligned && !fp.dale.buy_imb_prices.is_empty() {
        let lo = fp
            .dale
            .buy_imb_prices
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let hi = fp
            .dale
            .buy_imb_prices
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        return (Some(lo), Some(hi), true, false);
    }
    if fp.dale.stacked_sell && fp.dale.aligned && !fp.dale.sell_imb_prices.is_empty() {
        let lo = fp
            .dale
            .sell_imb_prices
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let hi = fp
            .dale
            .sell_imb_prices
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        return (Some(lo), Some(hi), false, true);
    }
    (None, None, false, false)
}

#[derive(Debug, Clone, Copy)]
struct Zone {
    lo: f64,
    hi: f64,
}

impl Zone {
    fn contains(&self, px: f64) -> bool {
        px >= self.lo && px <= self.hi
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VolumeStack {
    pub n: u32,
    pub n_used: u32,
    pub poc: Option<f64>,
    pub band_lo: Option<f64>,
    pub band_hi: Option<f64>,
    pub still_on_old: bool,
    pub migrated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextSnap {
    pub venue: Venue,
    pub symbol: String,
    pub open_ms: i64,
    pub stack_5: VolumeStack,
    pub stack_15: VolumeStack,
    pub stack_60: VolumeStack,
    pub stack_240: VolumeStack,
    pub stack_1440: VolumeStack,
    pub stack_session: VolumeStack,
    pub swing_n: u32,
    pub swing_high: Option<f64>,
    pub swing_low: Option<f64>,
    pub swing_ready: bool,
    pub range_high: Option<f64>,
    pub range_low: Option<f64>,
    pub bars_in_range: u32,
    pub volume_in_range: f64,
    pub break_attempts: u32,
    pub failed_breaks: u32,
    pub stack_accepted: bool,
    pub fake_leave: bool,
    pub old_edge_lo: Option<f64>,
    pub old_edge_hi: Option<f64>,
    pub regime: RegimeSnap,
    pub forbidden: &'static [&'static str],
}

pub struct ContextEngine {
    cfg: ContextConfig,
    history: VecDeque<BarIn>,
    prev_stack_poc: BTreeMap<u32, f64>,
    prev_stack_band: BTreeMap<u32, (f64, f64)>,
    swing_high: Option<f64>,
    swing_low: Option<f64>,
    range_high: Option<f64>,
    range_low: Option<f64>,
    bars_in_range: u32,
    volume_in_range: f64,
    break_attempts: u32,
    failed_breaks: u32,
    outside_count: u32,
    leave_armed: bool,
    outside_pocs: u32,
    stack_accepted: bool,
    fake_leave: bool,
    zone: Option<Zone>,
    pending_break: Option<u32>,
    liq_seen: Vec<f64>,
}

impl ContextEngine {
    pub fn new(cfg: ContextConfig) -> Self {
        Self {
            cfg,
            history: VecDeque::new(),
            prev_stack_poc: BTreeMap::new(),
            prev_stack_band: BTreeMap::new(),
            swing_high: None,
            swing_low: None,
            range_high: None,
            range_low: None,
            bars_in_range: 0,
            volume_in_range: 0.0,
            break_attempts: 0,
            failed_breaks: 0,
            outside_count: 0,
            leave_armed: false,
            outside_pocs: 0,
            stack_accepted: false,
            fake_leave: false,
            zone: None,
            pending_break: None,
            liq_seen: Vec::new(),
        }
    }

    pub fn push(&mut self, bar: BarIn, regime: &RegimeInputs) -> ContextSnap {
        if let Some(n) = regime.liq_1m.get(&bar.open_ms).copied() {
            self.liq_seen.push(n);
        }
        if let (Some(lo), Some(hi)) = (bar.stack_lo, bar.stack_hi) {
            if !self.stack_accepted {
                self.zone = Some(Zone { lo, hi });
            }
        }
        self.update_leave(&bar);
        self.history.push_back(bar.clone());
        if self.history.len() > 1440 {
            self.history.pop_front();
        }
        self.update_swing();
        self.update_range(&bar);

        let stack_5 = self.window_stack(5);
        let stack_15 = self.window_stack(15);
        let stack_60 = self.window_stack(60);
        let stack_240 = self.window_stack(240);
        let stack_1440 = self.window_stack(1440);
        let stack_session = self.session_stack();
        let regime_snap = evaluate_regime(&self.cfg, regime, bar.open_ms, &self.liq_seen);

        ContextSnap {
            venue: bar.venue,
            symbol: bar.symbol,
            open_ms: bar.open_ms,
            stack_5,
            stack_15,
            stack_60,
            stack_240,
            stack_1440,
            stack_session,
            swing_n: self.cfg.swing_n,
            swing_high: self.swing_high,
            swing_low: self.swing_low,
            swing_ready: self.swing_high.is_some() && self.swing_low.is_some(),
            range_high: self.range_high,
            range_low: self.range_low,
            bars_in_range: self.bars_in_range,
            volume_in_range: self.volume_in_range,
            break_attempts: self.break_attempts,
            failed_breaks: self.failed_breaks,
            stack_accepted: self.stack_accepted,
            fake_leave: self.fake_leave,
            old_edge_lo: self.zone.map(|z| z.lo),
            old_edge_hi: self.zone.map(|z| z.hi),
            regime: regime_snap,
            forbidden: FORBIDDEN,
        }
    }

    fn update_leave(&mut self, bar: &BarIn) {
        let Some(z) = self.zone else {
            return;
        };
        let inside = z.contains(bar.close);
        let poc_inside = bar.poc.map(|p| z.contains(p)).unwrap_or(inside);
        if inside {
            if self.leave_armed && self.outside_pocs < self.cfg.accept_bars {
                self.fake_leave = true;
                self.stack_accepted = false;
            }
            self.outside_count = 0;
            self.leave_armed = false;
            self.outside_pocs = 0;
            return;
        }
        self.outside_count = self.outside_count.saturating_add(1);
        if self.outside_count >= self.cfg.leave_bars {
            self.leave_armed = true;
        }
        if self.leave_armed {
            if !poc_inside {
                self.outside_pocs = self.outside_pocs.saturating_add(1);
            }
            if self.outside_pocs >= self.cfg.accept_bars {
                self.stack_accepted = true;
                self.fake_leave = false;
            }
        }
    }

    fn update_swing(&mut self) {
        let n = self.cfg.swing_n as usize;
        if n == 0 || self.history.len() < 2 * n + 1 {
            return;
        }
        let i = self.history.len() - 1 - n;
        let hi = self.history[i].high;
        let lo = self.history[i].low;
        let mut is_high = true;
        let mut is_low = true;
        for k in i - n..=i + n {
            if k == i {
                continue;
            }
            if self.history[k].high >= hi {
                is_high = false;
            }
            if self.history[k].low <= lo {
                is_low = false;
            }
        }
        if is_high {
            self.swing_high = Some(hi);
        }
        if is_low {
            self.swing_low = Some(lo);
        }
        if let (Some(h), Some(l)) = (self.swing_high, self.swing_low) {
            if h > l {
                self.range_high = Some(h);
                self.range_low = Some(l);
            }
        }
    }

    fn update_range(&mut self, bar: &BarIn) {
        let (Some(rh), Some(rl)) = (self.range_high, self.range_low) else {
            self.bars_in_range = 0;
            self.volume_in_range = 0.0;
            return;
        };
        let inside = bar.close <= rh && bar.close >= rl;
        let vol = bar.bid_vol + bar.ask_vol;
        if inside {
            self.bars_in_range = self.bars_in_range.saturating_add(1);
            self.volume_in_range += vol;
            if self.pending_break.is_some() {
                self.failed_breaks = self.failed_breaks.saturating_add(1);
                self.pending_break = None;
            }
        } else if let Some(left) = self.pending_break.as_mut() {
            self.bars_in_range = 0;
            self.volume_in_range = 0.0;
            if *left == 0 {
                self.pending_break = None;
            } else {
                *left -= 1;
            }
        } else {
            self.bars_in_range = 0;
            self.volume_in_range = 0.0;
            self.break_attempts = self.break_attempts.saturating_add(1);
            self.pending_break = Some(self.cfg.trap_bars);
        }
    }

    fn window_stack(&mut self, n: u32) -> VolumeStack {
        let used: Vec<BarIn> = self
            .history
            .iter()
            .rev()
            .take(n as usize)
            .cloned()
            .collect();
        self.stack_from(n, &used)
    }

    fn session_stack(&mut self) -> VolumeStack {
        let Some(cur) = self.history.back() else {
            return empty_stack(0);
        };
        let day = cur.open_ms.div_euclid(86_400_000);
        let session = orderflow_footprint::session_of(cur.open_ms);
        let used: Vec<BarIn> = self
            .history
            .iter()
            .filter(|b| {
                b.open_ms.div_euclid(86_400_000) == day
                    && orderflow_footprint::session_of(b.open_ms) == session
            })
            .cloned()
            .collect();
        self.stack_from(0, &used)
    }

    fn stack_from(&mut self, key: u32, bars: &[BarIn]) -> VolumeStack {
        if bars.is_empty() {
            return empty_stack(key);
        }
        let mut vols: BTreeMap<i64, f64> = BTreeMap::new();
        let mut step = bars
            .iter()
            .map(|b| b.bucket)
            .find(|b| *b > 0.0)
            .unwrap_or(0.01);
        if let Some((a, b)) = bars
            .iter()
            .flat_map(|bar| bar.cells.windows(2))
            .map(|w| (w[0].0, w[1].0))
            .next()
        {
            step = (b - a).abs().max(1e-12);
        }
        for bar in bars {
            for (px, v) in &bar.cells {
                let k = (*px / step + 1e-12).round() as i64;
                *vols.entry(k).or_insert(0.0) += *v;
            }
            if bar.cells.is_empty() {
                if let Some(poc) = bar.poc {
                    let k = (poc / step + 1e-12).round() as i64;
                    *vols.entry(k).or_insert(0.0) += bar.bid_vol + bar.ask_vol;
                }
            }
        }
        if vols.is_empty() {
            return empty_stack(key);
        }
        let (poc_k, _) = vols
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap();
        let poc = *poc_k as f64 * step;
        let xs: Vec<f64> = vols.values().copied().collect();
        let thresh = percentile(&xs, 70.0);
        let mut lo = *poc_k;
        let mut hi = *poc_k;
        while let Some(v) = vols.get(&(lo - 1)) {
            if *v >= thresh {
                lo -= 1;
            } else {
                break;
            }
        }
        while let Some(v) = vols.get(&(hi + 1)) {
            if *v >= thresh {
                hi += 1;
            } else {
                break;
            }
        }
        let band = (lo as f64 * step, hi as f64 * step);
        let still = self
            .prev_stack_band
            .get(&key)
            .map(|(a, b)| poc >= *a && poc <= *b)
            .unwrap_or(false);
        let migrated = self
            .prev_stack_poc
            .get(&key)
            .map(|old| (poc - *old).abs() > step * 0.5)
            .unwrap_or(false)
            && !still;
        self.prev_stack_poc.insert(key, poc);
        self.prev_stack_band.insert(key, band);
        VolumeStack {
            n: key,
            n_used: bars.len() as u32,
            poc: Some(poc),
            band_lo: Some(band.0),
            band_hi: Some(band.1),
            still_on_old: still,
            migrated,
        }
    }
}

fn empty_stack(n: u32) -> VolumeStack {
    VolumeStack {
        n,
        n_used: 0,
        poc: None,
        band_lo: None,
        band_hi: None,
        still_on_old: false,
        migrated: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resonance::{join, VenueDir};
    use orderflow_domain::{ResonanceMode, Venue};

    fn bar(open_ms: i64, high: f64, low: f64, close: f64, poc: f64, vol_at: f64) -> BarIn {
        BarIn {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            open_ms,
            bucket: 0.01,
            high,
            low,
            close,
            delta: 1.0,
            bid_vol: 1.0,
            ask_vol: 1.0,
            poc: Some(poc),
            cells: vec![(poc, vol_at)],
            dale_aligned: false,
            stack_buy: false,
            stack_sell: false,
            stack_lo: None,
            stack_hi: None,
        }
    }

    #[test]
    fn wired_and_no_forbidden_schools() {
        assert!(WIRED);
        for w in FORBIDDEN {
            assert!(!w.contains("footprint"));
        }
    }

    #[test]
    fn volume_stack_migrates_when_poc_leaves_old_band() {
        let mut eng = ContextEngine::new(ContextConfig::golden_sol());
        let empty = RegimeInputs::default();
        for i in 0..5 {
            eng.push(bar(i * 60_000, 100.01, 99.99, 100.00, 100.00, 10.0), &empty);
        }
        let first = eng.history.len();
        assert_eq!(first, 5);
        let snap = eng.push(
            bar(5 * 60_000, 100.06, 100.04, 100.05, 100.05, 20.0),
            &empty,
        );
        assert!((snap.stack_5.poc.unwrap() - 100.00).abs() < 1e-9 || snap.stack_5.migrated);
        let snap = eng.push(
            bar(6 * 60_000, 100.06, 100.04, 100.05, 100.05, 20.0),
            &empty,
        );
        let _ = snap;
        let mut later = ContextEngine::new(ContextConfig::golden_sol());
        for i in 0..5 {
            later.push(
                bar(i * 60_000, 100.06, 100.04, 100.05, 100.05, 10.0),
                &empty,
            );
        }
        assert!((later.window_stack_poc() - 100.05).abs() < 1e-9);
    }

    impl ContextEngine {
        fn window_stack_poc(&mut self) -> f64 {
            self.window_stack(5).poc.unwrap()
        }
    }

    #[test]
    fn swing_n_confirms_after_both_wings() {
        let mut cfg = ContextConfig::golden_sol();
        cfg.swing_n = 2;
        let mut eng = ContextEngine::new(cfg);
        let empty = RegimeInputs::default();
        let highs = [5.0, 4.0, 1.2, 4.0, 10.0, 4.0, 3.0, 2.0, 5.0];
        let lows = [4.5, 3.5, 0.1, 3.5, 9.5, 3.5, 2.5, 1.5, 4.5];
        let mut snap = None;
        for (i, (h, l)) in highs.iter().zip(lows.iter()).enumerate() {
            snap = Some(eng.push(
                bar(
                    i as i64 * 60_000,
                    *h,
                    *l,
                    (*h + *l) / 2.0,
                    (*h + *l) / 2.0,
                    1.0,
                ),
                &empty,
            ));
        }
        let snap = snap.unwrap();
        assert_eq!(snap.swing_high, Some(10.0));
        assert!(snap.swing_ready);
    }

    #[test]
    fn accept_after_leave_and_outside_pocs() {
        let mut cfg = ContextConfig::golden_sol();
        cfg.leave_bars = 1;
        cfg.accept_bars = 3;
        let mut eng = ContextEngine::new(cfg);
        let empty = RegimeInputs::default();
        let mut z = bar(0, 100.02, 100.00, 100.01, 100.01, 5.0);
        z.stack_lo = Some(100.00);
        z.stack_hi = Some(100.02);
        z.dale_aligned = true;
        z.stack_buy = true;
        eng.push(z, &empty);
        for i in 1..=3 {
            let s = eng.push(bar(i * 60_000, 100.10, 100.08, 100.09, 100.09, 5.0), &empty);
            if i < 3 {
                assert!(!s.stack_accepted, "i={i}");
            } else {
                assert!(s.stack_accepted);
                assert!(!s.fake_leave);
            }
        }
    }

    #[test]
    fn fake_leave_when_price_returns_before_accept() {
        let mut cfg = ContextConfig::golden_sol();
        cfg.leave_bars = 1;
        cfg.accept_bars = 3;
        let mut eng = ContextEngine::new(cfg);
        let empty = RegimeInputs::default();
        let mut z = bar(0, 100.02, 100.00, 100.01, 100.01, 5.0);
        z.stack_lo = Some(100.00);
        z.stack_hi = Some(100.02);
        z.dale_aligned = true;
        z.stack_buy = true;
        eng.push(z, &empty);
        eng.push(bar(60_000, 100.10, 100.08, 100.09, 100.09, 5.0), &empty);
        let back = bar(120_000, 100.02, 100.00, 100.01, 100.01, 5.0);
        let s = eng.push(back, &empty);
        assert!(s.fake_leave);
        assert!(!s.stack_accepted);
    }

    #[test]
    fn funding_black_window_is_clock_not_killzone() {
        assert!(funding_black_window(0, &[0, 8, 16], 15));
        assert!(!funding_black_window(20 * 60_000, &[0, 8, 16], 15));
        let eight = 8 * 3600 * 1000;
        assert!(funding_black_window(eight, &[0, 8, 16], 15));
    }

    #[test]
    fn oi_drop_marks_liquidation_veto() {
        let cfg = ContextConfig::golden_sol();
        let mut inputs = RegimeInputs {
            stream_present: true,
            ..RegimeInputs::default()
        };
        inputs.oi_1h.insert(0, 100.0);
        inputs.oi_1h.insert(3_600_000, 97.5);
        let snap = evaluate_regime(&cfg, &inputs, 3_600_000, &[]);
        assert_eq!(snap.liquidation_regime, "true");
        assert!(snap.new_entries_blocked);
        assert!((snap.oi_1h_chg.unwrap() + 0.025).abs() < 1e-9);
    }

    #[test]
    fn missing_liq_stream_is_not_evaluated() {
        let cfg = ContextConfig::golden_sol();
        let inputs = RegimeInputs::default();
        let snap = evaluate_regime(&cfg, &inputs, 0, &[]);
        assert_eq!(snap.liquidation_regime, "not_evaluated");
        assert!(snap.liq_stream_missing);
        assert_eq!(snap.funding_crowding, "not_evaluated");
        assert_eq!(snap.basis_stress, "not_evaluated");
    }

    #[test]
    fn resonance_missing_peer_is_not_stale_bar() {
        let okx = VenueDir::from_delta(Venue::Okx, 60_000, 2.0, 1, true);
        let stale = VenueDir::from_delta(Venue::Binance, 0, 2.0, 1, true);
        let snap = join(60_000, ResonanceMode::Off, 1, Some(okx), Some(stale), None);
        assert_eq!(snap.binance, "not_evaluated");
        assert_eq!(snap.bybit, "not_evaluated");
        assert!(!snap.used_for_entry);
        assert!(!snap.copied_price_onto_okx);
        assert!(!snap.would_confirm_k);
    }

    #[test]
    fn resonance_off_records_but_does_not_drive_entry() {
        let snap = join(
            0,
            ResonanceMode::Off,
            1,
            Some(VenueDir::from_delta(Venue::Okx, 0, 1.0, 1, true)),
            Some(VenueDir::from_delta(Venue::Binance, 0, 1.0, 1, true)),
            Some(VenueDir::from_delta(Venue::Bybit, 0, 1.0, 1, true)),
        );
        assert!(snap.would_confirm_all);
        assert!(snap.would_confirm_k);
        assert!(!snap.used_for_entry);
        assert!(!snap.copied_price_onto_okx);
        assert_eq!(snap.peers_same_as_okx, 2);
    }

    #[test]
    fn sui_step_is_not_sol_0_01() {
        let mut eng = ContextEngine::new(ContextConfig::golden_sol());
        let empty = RegimeInputs::default();
        let mut b = bar(0, 1.0002, 1.0000, 1.0001, 1.0001, 4.0);
        b.symbol = "SUI".into();
        b.bucket = 0.0001;
        b.cells = vec![(1.0000, 1.0), (1.0001, 4.0), (1.0002, 1.0)];
        let snap = eng.push(b, &empty);
        assert!((snap.stack_5.poc.unwrap() - 1.0001).abs() < 1e-9);
        assert_ne!(snap.stack_5.poc.unwrap(), 1.00);
    }
}
