//! Three-venue direction only. Never copy a peer price onto OKX.

use std::collections::HashMap;

use orderflow_domain::{ResonanceMode, Venue};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct VenueDir {
    pub venue: Venue,
    pub open_ms: i64,
    pub delta_sign: i8,
    pub stack_sign: i8,
    pub healthy: bool,
}

impl VenueDir {
    pub fn from_delta(
        venue: Venue,
        open_ms: i64,
        delta: f64,
        stack_sign: i8,
        healthy: bool,
    ) -> Self {
        Self {
            venue,
            open_ms,
            delta_sign: sign(delta),
            stack_sign,
            healthy,
        }
    }
}

fn sign(x: f64) -> i8 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ResonanceSnap {
    pub open_ms: i64,
    pub mode: ResonanceMode,
    pub k: u32,
    pub okx_delta_sign: Option<i8>,
    pub binance_delta_sign: Option<i8>,
    pub bybit_delta_sign: Option<i8>,
    pub binance: &'static str,
    pub bybit: &'static str,
    pub peers_same_as_okx: u32,
    pub would_confirm_k: bool,
    pub would_confirm_all: bool,
    /// False while mode is `off`. Resonance never writes an OKX limit.
    pub used_for_entry: bool,
    pub copied_price_onto_okx: bool,
}

#[derive(Debug, Default)]
pub struct ResonanceBook {
    by_key: HashMap<(Venue, i64), VenueDir>,
}

impl ResonanceBook {
    pub fn insert(&mut self, d: VenueDir) {
        self.by_key.insert((d.venue, d.open_ms), d);
    }

    pub fn get(&self, venue: Venue, open_ms: i64) -> Option<&VenueDir> {
        self.by_key.get(&(venue, open_ms))
    }

    pub fn snapshot(&self, open_ms: i64, mode: ResonanceMode, k: u32) -> ResonanceSnap {
        join(
            open_ms,
            mode,
            k,
            self.get(Venue::Okx, open_ms).copied(),
            self.get(Venue::Binance, open_ms).copied(),
            self.get(Venue::Bybit, open_ms).copied(),
        )
    }

    pub fn okx_minutes(&self) -> Vec<i64> {
        let mut xs: Vec<i64> = self
            .by_key
            .keys()
            .filter(|(v, _)| *v == Venue::Okx)
            .map(|(_, t)| *t)
            .collect();
        xs.sort_unstable();
        xs
    }
}

pub fn join(
    open_ms: i64,
    mode: ResonanceMode,
    k: u32,
    okx: Option<VenueDir>,
    binance: Option<VenueDir>,
    bybit: Option<VenueDir>,
) -> ResonanceSnap {
    let peer = |d: Option<VenueDir>| -> (&'static str, Option<i8>) {
        match d {
            Some(x) if x.healthy && x.open_ms == open_ms => ("evaluated", Some(x.delta_sign)),
            _ => ("not_evaluated", None),
        }
    };
    let (binance_st, binance_sign) = peer(binance);
    let (bybit_st, bybit_sign) = peer(bybit);
    let okx_sign = okx
        .filter(|x| x.healthy && x.open_ms == open_ms)
        .map(|x| x.delta_sign);
    let mut same = 0u32;
    if let Some(s) = okx_sign {
        if s != 0 {
            if binance_sign == Some(s) {
                same += 1;
            }
            if bybit_sign == Some(s) {
                same += 1;
            }
        }
    }
    let would_k = okx_sign.unwrap_or(0) != 0 && same >= k;
    let would_all = okx_sign.unwrap_or(0) != 0
        && binance_st == "evaluated"
        && bybit_st == "evaluated"
        && binance_sign == okx_sign
        && bybit_sign == okx_sign;
    let used = match mode {
        ResonanceMode::Off => false,
        ResonanceMode::KOfN => would_k,
        ResonanceMode::All => would_all,
    };
    ResonanceSnap {
        open_ms,
        mode,
        k,
        okx_delta_sign: okx_sign,
        binance_delta_sign: binance_sign,
        bybit_delta_sign: bybit_sign,
        binance: binance_st,
        bybit: bybit_st,
        peers_same_as_okx: same,
        would_confirm_k: would_k,
        would_confirm_all: would_all,
        used_for_entry: used,
        copied_price_onto_okx: false,
    }
}
