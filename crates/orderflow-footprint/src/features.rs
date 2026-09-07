//! Closed-bar footprint features. Diagonal only. No VWAP / TPO / Naked POC.

use std::collections::BTreeMap;

use orderflow_domain::{ImbalanceStyle, TakerSide, Venue};
use serde::Serialize;

use crate::bucket::{key_to_price, Session};
use crate::config::FootprintConfig;

#[derive(Debug, Clone, Default)]
pub struct CellVol {
    pub bid: f64,
    pub ask: f64,
}

impl CellVol {
    pub fn total(&self) -> f64 {
        self.bid + self.ask
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FootprintCell {
    pub price: f64,
    pub bid: f64,
    pub ask: f64,
}

/// Imbalance / stack slice at one ratio (200% record, 300% Dale, 400% Valtos).
#[derive(Debug, Clone, Serialize)]
pub struct RateSlice {
    pub rate: f64,
    pub buy_imb_prices: Vec<f64>,
    pub sell_imb_prices: Vec<f64>,
    pub buy_stack: u32,
    pub sell_stack: u32,
    pub multiple_buy: bool,
    pub multiple_sell: bool,
    /// Stack ≥ min and bar-direction aligned (chaos excluded).
    pub stacked_buy: bool,
    pub stacked_sell: bool,
    pub aligned: bool,
    pub contra: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FootprintBar {
    pub venue: Venue,
    pub symbol: String,
    pub open_ms: i64,
    pub bucket: f64,
    pub session: Session,
    pub cells: Vec<FootprintCell>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub bid_vol: f64,
    pub ask_vol: f64,
    pub delta: f64,
    pub cvd: f64,
    pub trade_count: u32,
    pub tape_speed: f64,
    pub min_volume: f64,
    pub min_volume_rule: String,
    pub bar_up: bool,
    pub bar_down: bool,
    pub poc: Option<f64>,
    pub va_low: Option<f64>,
    pub va_high: Option<f64>,
    pub va_ok: bool,
    pub va_width: u32,
    pub aligned_poc: bool,
    pub unfinished_high: bool,
    pub unfinished_low: bool,
    pub finished_high: bool,
    pub finished_low: bool,
    pub unfinished_is_entry: bool,
    pub chaos: bool,
    pub record: RateSlice,
    pub dale: RateSlice,
    pub valtos: RateSlice,
    /// DOM absorption needs L2. Stage 2 leaves this explicit.
    pub absorption: &'static str,
    pub script_f: &'static str,
}

pub struct Computed {
    pub cells: Vec<FootprintCell>,
    pub bid_vol: f64,
    pub ask_vol: f64,
    pub poc: Option<f64>,
    pub va_low: Option<f64>,
    pub va_high: Option<f64>,
    pub va_ok: bool,
    pub va_width: u32,
    pub unfinished_high: bool,
    pub unfinished_low: bool,
    pub finished_high: bool,
    pub finished_low: bool,
    pub chaos: bool,
    pub record: RateSlice,
    pub dale: RateSlice,
    pub valtos: RateSlice,
    pub nonempty_side_vols: Vec<f64>,
}

pub fn nonempty_side_vols(cells: &BTreeMap<i64, CellVol>) -> Vec<f64> {
    let mut xs = Vec::new();
    for c in cells.values() {
        if c.bid > 0.0 {
            xs.push(c.bid);
        }
        if c.ask > 0.0 {
            xs.push(c.ask);
        }
    }
    xs
}

fn stacked_runs(flags: &[bool]) -> u32 {
    let mut best = 0u32;
    let mut cur = 0u32;
    for f in flags {
        if *f {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

fn rate_slice(
    cells: &BTreeMap<i64, CellVol>,
    min_key: i64,
    max_key: i64,
    bucket: f64,
    ratio: f64,
    min_vol: f64,
    ignore_zero: bool,
    stack_min: u32,
    bar_up: bool,
    bar_down: bool,
    require_dir: bool,
) -> RateSlice {
    let mut buy_imb = Vec::new();
    let mut sell_imb = Vec::new();
    let n = (max_key - min_key + 1) as usize;
    let mut buy_flags = vec![false; n];
    let mut sell_flags = vec![false; n];

    for k in min_key..=max_key {
        let i = (k - min_key) as usize;
        let ask = cells.get(&k).map(|c| c.ask).unwrap_or(0.0);
        let bid = cells.get(&k).map(|c| c.bid).unwrap_or(0.0);
        let bid_below = cells.get(&(k - 1)).map(|c| c.bid).unwrap_or(0.0);
        let ask_above = cells.get(&(k + 1)).map(|c| c.ask).unwrap_or(0.0);

        let buy_ok = if ignore_zero && (ask == 0.0 || bid_below == 0.0) {
            false
        } else {
            ask >= min_vol && bid_below >= min_vol && ask >= ratio * bid_below
        };
        let sell_ok = if ignore_zero && (bid == 0.0 || ask_above == 0.0) {
            false
        } else {
            bid >= min_vol && ask_above >= min_vol && bid >= ratio * ask_above
        };
        if buy_ok {
            buy_imb.push(key_to_price(k, bucket));
            buy_flags[i] = true;
        }
        if sell_ok {
            sell_imb.push(key_to_price(k, bucket));
            sell_flags[i] = true;
        }
    }

    let buy_stack = stacked_runs(&buy_flags);
    let sell_stack = stacked_runs(&sell_flags);
    let chaos = buy_stack >= stack_min && sell_stack >= stack_min;
    let buy_raw = buy_stack >= stack_min;
    let sell_raw = sell_stack >= stack_min;
    let stacked_buy = buy_raw && !chaos && (!require_dir || bar_up);
    let stacked_sell = sell_raw && !chaos && (!require_dir || bar_down);
    let mut aligned = (buy_raw && bar_up) || (sell_raw && bar_down);
    let contra = (buy_raw && bar_down) || (sell_raw && bar_up);
    if chaos {
        aligned = false;
    }
    RateSlice {
        rate: ratio,
        multiple_buy: buy_imb.len() as u32 >= stack_min && buy_stack < stack_min,
        multiple_sell: sell_imb.len() as u32 >= stack_min && sell_stack < stack_min,
        buy_imb_prices: buy_imb,
        sell_imb_prices: sell_imb,
        buy_stack,
        sell_stack,
        stacked_buy,
        stacked_sell,
        aligned,
        contra,
    }
}

pub fn compute(
    cells: &BTreeMap<i64, CellVol>,
    cfg: &FootprintConfig,
    min_vol: f64,
    open: f64,
    close: f64,
) -> Option<Computed> {
    if cells.is_empty() {
        return None;
    }
    debug_assert_eq!(cfg.imbalance_style, ImbalanceStyle::Diagonal);
    let min_key = *cells.keys().next()?;
    let max_key = *cells.keys().next_back()?;
    let bucket = cfg.bucket;
    let bar_up = close > open;
    let bar_down = close < open;

    let mut bid_vol = 0.0;
    let mut ask_vol = 0.0;
    let mut vol_at: BTreeMap<i64, f64> = BTreeMap::new();
    let mut out_cells = Vec::with_capacity(cells.len());
    for (k, c) in cells {
        bid_vol += c.bid;
        ask_vol += c.ask;
        vol_at.insert(*k, c.total());
        out_cells.push(FootprintCell {
            price: key_to_price(*k, bucket),
            bid: c.bid,
            ask: c.ask,
        });
    }

    let poc_key = vol_at
        .iter()
        .max_by(|a, b| {
            a.1.partial_cmp(b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.0.cmp(a.0)) // tie → lowest key
        })
        .map(|(k, _)| *k);
    let total: f64 = vol_at.values().sum();
    let mut va_low = poc_key;
    let mut va_high = poc_key;
    let mut va_ok = false;
    if let Some(poc) = poc_key {
        if total > 0.0 {
            let mut lo = poc;
            let mut hi = poc;
            let mut acc = vol_at.get(&poc).copied().unwrap_or(0.0);
            let target = cfg.value_area_pct * total;
            while acc < target {
                let lv = vol_at.get(&(lo - 1)).copied().unwrap_or(0.0);
                let rv = vol_at.get(&(hi + 1)).copied().unwrap_or(0.0);
                if lv == 0.0 && rv == 0.0 {
                    break;
                }
                if lv >= rv {
                    lo -= 1;
                    acc += lv;
                } else {
                    hi += 1;
                    acc += rv;
                }
            }
            va_ok = acc >= target;
            va_low = Some(lo);
            va_high = Some(hi);
        }
    }

    let high = max_key;
    let low = min_key;
    let bid_hi = cells.get(&high).map(|c| c.bid).unwrap_or(0.0);
    let ask_hi = cells.get(&high).map(|c| c.ask).unwrap_or(0.0);
    let bid_lo = cells.get(&low).map(|c| c.bid).unwrap_or(0.0);
    let ask_lo = cells.get(&low).map(|c| c.ask).unwrap_or(0.0);
    let unfinished_high = bid_hi > 0.0 && ask_hi > 0.0;
    let unfinished_low = bid_lo > 0.0 && ask_lo > 0.0;
    let finished_high = bid_hi == 0.0 && ask_hi > 0.0;
    let finished_low = ask_lo == 0.0 && bid_lo > 0.0;

    let record = rate_slice(
        cells,
        min_key,
        max_key,
        bucket,
        cfg.imbalance_rate_record,
        min_vol,
        cfg.ignore_zero,
        cfg.stack_min_levels,
        bar_up,
        bar_down,
        cfg.stack_require_bar_direction,
    );
    let dale = rate_slice(
        cells,
        min_key,
        max_key,
        bucket,
        cfg.imbalance_rate_dale,
        min_vol,
        cfg.ignore_zero,
        cfg.stack_min_levels,
        bar_up,
        bar_down,
        cfg.stack_require_bar_direction,
    );
    let valtos = rate_slice(
        cells,
        min_key,
        max_key,
        bucket,
        cfg.imbalance_rate_valtos,
        min_vol,
        cfg.ignore_zero,
        cfg.stack_min_levels,
        bar_up,
        bar_down,
        cfg.stack_require_bar_direction,
    );
    let chaos = dale.buy_stack >= cfg.stack_min_levels && dale.sell_stack >= cfg.stack_min_levels;

    Some(Computed {
        cells: out_cells,
        bid_vol,
        ask_vol,
        poc: poc_key.map(|k| key_to_price(k, bucket)),
        va_low: va_low.map(|k| key_to_price(k, bucket)),
        va_high: va_high.map(|k| key_to_price(k, bucket)),
        va_ok,
        va_width: match (va_ok, va_low, va_high) {
            (true, Some(lo), Some(hi)) => (hi - lo + 1) as u32,
            _ => 0,
        },
        unfinished_high,
        unfinished_low,
        finished_high,
        finished_low,
        chaos,
        record,
        dale,
        valtos,
        nonempty_side_vols: nonempty_side_vols(cells),
    })
}

/// Apply one trade into a forming cell map. Taker buy hits ask.
pub fn apply_trade(
    cells: &mut BTreeMap<i64, CellVol>,
    px: f64,
    size: f64,
    side: TakerSide,
    bucket: f64,
) {
    let k = crate::bucket::bucket_key(px, bucket);
    let c = cells.entry(k).or_default();
    match side {
        TakerSide::Buy => c.ask += size,
        TakerSide::Sell => c.bid += size,
    }
}
