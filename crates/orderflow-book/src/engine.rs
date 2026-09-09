//! Per-venue L2 engine.
//!
//! Integrity (do not mix venue rules):
//! - OKX: `seqId`/`prevSeqId`. `checksum==0` is ignored (deprecated 2026-06-23).
//! - Binance USD-M: `pu` must equal previous `u`. REST snapshot seeds `lastUpdateId`.
//! - Bybit: `type=snapshot` resets; `type=delta` applies; `u==1` overwrites; size 0 deletes.
//!
//! A toxic book on one venue never copies prices onto OKX and never marks the other
//! two venues' books bad.

use orderflow_domain::{QualityVector, Trade, Venue, VenueRole};
use serde::Serialize;

use crate::ladder::{format_px, format_sz, okx_checksum, px_of, tick_of, Ladder, Side};
use crate::parse::{BookDelta, BookMsgKind, BookParseError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BookHealth {
    /// No snapshot yet, or currently resyncing.
    Rebuilding,
    Ok,
    Degraded,
    Bad,
}

impl BookHealth {
    pub fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rebuilding => "book_rebuilding",
            Self::Ok => "book_ok",
            Self::Degraded => "book_degraded",
            Self::Bad => "book_bad",
        }
    }
}

/// Four footprint↔DOM reads. `not_evaluated` when the book is toxic or there is no wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WallRead {
    NotEvaluated,
    EatThrough,
    Yield,
    Absorb,
    FakeWall,
}

#[derive(Debug, Clone)]
pub struct BookConfig {
    pub tick_sz: f64,
    pub min_levels: usize,
    pub wall_mult_of_median: f64,
    pub top_n: usize,
}

impl BookConfig {
    pub fn sol() -> Self {
        Self {
            tick_sz: 0.01,
            min_levels: 5,
            wall_mult_of_median: 3.0,
            top_n: 25,
        }
    }

    pub fn sui() -> Self {
        Self {
            tick_sz: 0.0001,
            min_levels: 5,
            wall_mult_of_median: 3.0,
            top_n: 25,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct WallTrack {
    side: Option<Side>,
    tick: i64,
    size: f64,
    eaten: f64,
    pulled: f64,
    replenished: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookRead {
    pub venue: Venue,
    pub role: VenueRole,
    pub health: BookHealth,
    pub book_ok: bool,
    /// Execution-venue DOM entries. Resonance book damage does not block OKX footprint.
    pub dom_entries_allowed: bool,
    pub resonance_read: &'static str,
    pub bid1: Option<f64>,
    pub ask1: Option<f64>,
    pub spread: Option<f64>,
    pub levels_bid: usize,
    pub levels_ask: usize,
    pub wall_side: Option<&'static str>,
    pub wall_price: Option<f64>,
    pub wall_size: Option<f64>,
    pub wall_on_poc: bool,
    pub wall_on_stack: bool,
    pub read: WallRead,
    pub checksum_fail: u32,
    pub seq_gap: u32,
    pub rebuilds: u32,
    pub script_f: &'static str,
}

pub struct BookEngine {
    venue: Venue,
    role: VenueRole,
    cfg: BookConfig,
    bids: Ladder,
    asks: Ladder,
    health: BookHealth,
    last_seq: Option<i64>,
    checksum_fail: u32,
    seq_gap: u32,
    rebuilds: u32,
    last_ts_ms: i64,
    trades_at: std::collections::HashMap<i64, f64>,
    last_trade_px: Option<f64>,
    wall: WallTrack,
}

impl BookEngine {
    pub fn new(venue: Venue, role: VenueRole, cfg: BookConfig) -> Self {
        Self {
            venue,
            role,
            cfg,
            bids: Ladder::default(),
            asks: Ladder::default(),
            health: BookHealth::Rebuilding,
            last_seq: None,
            checksum_fail: 0,
            seq_gap: 0,
            rebuilds: 0,
            last_ts_ms: 0,
            trades_at: std::collections::HashMap::new(),
            last_trade_px: None,
            wall: WallTrack::default(),
        }
    }

    pub fn venue(&self) -> Venue {
        self.venue
    }

    pub fn health(&self) -> BookHealth {
        self.health
    }

    pub fn apply_quality(&self, q: &mut QualityVector) {
        q.set_book_ok(self.venue, self.health.is_ok());
    }

    pub fn apply_trade(&mut self, trade: &Trade) {
        assert_eq!(trade.venue, self.venue);
        let t = tick_of(trade.price, self.cfg.tick_sz);
        *self.trades_at.entry(t).or_insert(0.0) += trade.size;
        self.last_trade_px = Some(trade.price);
    }

    pub fn apply_frame(&mut self, text: &str) -> Result<(), BookParseError> {
        let delta = match crate::parse::parse_frame(self.venue, text) {
            Err(BookParseError::Control) => return Ok(()),
            other => other?,
        };
        self.apply_delta(delta);
        Ok(())
    }

    pub fn apply_delta(&mut self, delta: BookDelta) {
        debug_assert_eq!(delta.venue, self.venue);
        if delta.event_ts_ms != 0 {
            self.last_ts_ms = delta.event_ts_ms;
        }
        if !self.accept_seq(&delta) {
            self.mark_bad_gap();
            return;
        }
        let force_snapshot = delta.kind == BookMsgKind::Snapshot
            || (self.venue == Venue::Bybit && delta.seq == Some(1));
        if force_snapshot {
            self.bids.clear();
            self.asks.clear();
            self.rebuilds += 1;
            self.wall = WallTrack::default();
        }
        for u in &delta.updates {
            let tick = tick_of(u.price, self.cfg.tick_sz);
            let prev = match u.side {
                Side::Bid => self.bids.size_at(tick),
                Side::Ask => self.asks.size_at(tick),
            };
            self.note_wall_size(u.side, tick, prev, u.size);
            match u.side {
                Side::Bid => self.bids.apply(tick, u.size),
                Side::Ask => self.asks.apply(tick, u.size),
            };
        }
        // Official OKX CRC is the book *after* the update. checksum==0 is deprecated (2026-06-23) and ignored.
        if let Some(cs) = delta.checksum {
            if cs != 0 && self.book_crc() != cs {
                self.checksum_fail += 1;
                self.mark_bad_gap();
                return;
            }
        }
        if let Some(s) = delta.seq {
            self.last_seq = Some(s);
        }
        self.recompute_health();
        self.refresh_wall_identity();
    }

    fn book_crc(&self) -> i32 {
        let bids: Vec<(String, String)> = self
            .bids
            .top(Side::Bid, 25)
            .into_iter()
            .map(|(t, s)| (format_px(t, self.cfg.tick_sz), format_sz(s)))
            .collect();
        let asks: Vec<(String, String)> = self
            .asks
            .top(Side::Ask, 25)
            .into_iter()
            .map(|(t, s)| (format_px(t, self.cfg.tick_sz), format_sz(s)))
            .collect();
        okx_checksum(&bids, &asks)
    }

    fn accept_seq(&mut self, delta: &BookDelta) -> bool {
        match self.venue {
            Venue::Okx => self.accept_okx(delta),
            Venue::Binance => self.accept_binance(delta),
            Venue::Bybit => self.accept_bybit(delta),
        }
    }

    fn accept_okx(&self, delta: &BookDelta) -> bool {
        match delta.kind {
            BookMsgKind::Snapshot => true,
            BookMsgKind::Delta => match (self.last_seq, delta.prev_seq) {
                (None, _) => false,
                (Some(local), Some(prev)) => prev == local,
                (Some(_), None) => false,
            },
        }
    }

    fn accept_binance(&self, delta: &BookDelta) -> bool {
        match delta.kind {
            BookMsgKind::Snapshot => true,
            BookMsgKind::Delta => match self.last_seq {
                None => false,
                Some(last) => {
                    if let Some(pu) = delta.prev_seq {
                        pu == last
                    } else if let (Some(from), Some(to)) = (delta.seq_from, delta.seq) {
                        from <= last + 1 && last + 1 <= to
                    } else {
                        false
                    }
                }
            },
        }
    }

    fn accept_bybit(&self, delta: &BookDelta) -> bool {
        match delta.kind {
            BookMsgKind::Snapshot => true,
            BookMsgKind::Delta => {
                if delta.seq == Some(1) {
                    return true;
                }
                match (self.last_seq, delta.seq) {
                    (None, _) => false,
                    (Some(_), None) => false,
                    (Some(last), Some(u)) => u == last || u == last + 1,
                }
            }
        }
    }

    fn mark_bad_gap(&mut self) {
        self.seq_gap += 1;
        self.health = BookHealth::Bad;
        self.last_seq = None;
        self.bids.clear();
        self.asks.clear();
    }

    fn recompute_health(&mut self) {
        let nb = self.bids.len();
        let na = self.asks.len();
        if nb == 0 || na == 0 {
            self.health = BookHealth::Rebuilding;
            return;
        }
        let bid1 = self.bids.best_tick(Side::Bid);
        let ask1 = self.asks.best_tick(Side::Ask);
        if let (Some(b), Some(a)) = (bid1, ask1) {
            if b >= a {
                self.health = BookHealth::Bad;
                return;
            }
        }
        if nb < self.cfg.min_levels || na < self.cfg.min_levels {
            self.health = BookHealth::Degraded;
            return;
        }
        self.health = BookHealth::Ok;
    }

    fn note_wall_size(&mut self, side: Side, tick: i64, prev: f64, new: f64) {
        if self.wall.side != Some(side) || self.wall.tick != tick {
            return;
        }
        if new > prev {
            self.wall.replenished += new - prev;
            self.wall.size = new;
            return;
        }
        let drop = prev - new;
        if drop <= 0.0 {
            self.wall.size = new;
            return;
        }
        let traded = self.trades_at.get(&tick).copied().unwrap_or(0.0);
        let eat = drop.min(traded);
        self.wall.eaten += eat;
        self.wall.pulled += drop - eat;
        self.wall.size = new;
        *self.trades_at.entry(tick).or_insert(0.0) = (traded - eat).max(0.0);
    }

    fn refresh_wall_identity(&mut self) {
        let cand = self.largest_wall();
        match cand {
            None => {
                // Keep the vanished wall so this 1m can still say yield / eat-through.
                if self.wall.side.is_some() {
                    self.wall.size = 0.0;
                }
            }
            Some((side, tick, size)) => {
                if self.wall.side != Some(side) || self.wall.tick != tick {
                    self.wall = WallTrack {
                        side: Some(side),
                        tick,
                        size,
                        ..WallTrack::default()
                    };
                } else {
                    self.wall.size = size;
                }
            }
        }
    }

    fn largest_wall(&self) -> Option<(Side, i64, f64)> {
        let bid_med = self.bids.median_size(Side::Bid, self.cfg.top_n);
        let ask_med = self.asks.median_size(Side::Ask, self.cfg.top_n);
        let bid_top = self.bids.top(Side::Bid, self.cfg.top_n);
        let ask_top = self.asks.top(Side::Ask, self.cfg.top_n);
        let bid_w = bid_top
            .into_iter()
            .filter(|(_, s)| *s >= bid_med * self.cfg.wall_mult_of_median && *s > 0.0)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let ask_w = ask_top
            .into_iter()
            .filter(|(_, s)| *s >= ask_med * self.cfg.wall_mult_of_median && *s > 0.0)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        match (bid_w, ask_w) {
            (Some((t, s)), None) => Some((Side::Bid, t, s)),
            (None, Some((t, s))) => Some((Side::Ask, t, s)),
            (Some((tb, sb)), Some((ta, sa))) => {
                if sb >= sa {
                    Some((Side::Bid, tb, sb))
                } else {
                    Some((Side::Ask, ta, sa))
                }
            }
            (None, None) => None,
        }
    }

    pub fn freeze_1m(&self, poc: Option<f64>, stack_prices: &[f64]) -> BookRead {
        let bid1 = self
            .bids
            .best_tick(Side::Bid)
            .map(|t| px_of(t, self.cfg.tick_sz));
        let ask1 = self
            .asks
            .best_tick(Side::Ask)
            .map(|t| px_of(t, self.cfg.tick_sz));
        let spread = match (bid1, ask1) {
            (Some(b), Some(a)) => Some(a - b),
            _ => None,
        };
        let wall_px = if self.wall.side.is_some() && self.wall.size > 0.0 {
            Some(px_of(self.wall.tick, self.cfg.tick_sz))
        } else {
            None
        };
        let wall_on_poc = match (wall_px, poc) {
            (Some(w), Some(p)) => (w - p).abs() < self.cfg.tick_sz * 0.5,
            _ => false,
        };
        let wall_on_stack = wall_px
            .map(|w| {
                stack_prices
                    .iter()
                    .any(|p| (*p - w).abs() < self.cfg.tick_sz * 0.5)
            })
            .unwrap_or(false);
        let book_ok = self.health.is_ok();
        let exec = self.role == VenueRole::Execution;
        // Seq-gap / no snapshot: do not invent a read. A wall that vanished this bar may
        // leave the book Degraded (fewer levels) and still must emit yield / eat-through.
        let read = if matches!(self.health, BookHealth::Bad | BookHealth::Rebuilding) {
            WallRead::NotEvaluated
        } else {
            classify_wall(&self.wall, self.last_trade_px, self.cfg.tick_sz)
        };
        let script_f = if book_ok { "computed" } else { "not_evaluated" };
        BookRead {
            venue: self.venue,
            role: self.role,
            health: self.health,
            book_ok,
            dom_entries_allowed: if exec { book_ok } else { true },
            resonance_read: if exec {
                "n/a"
            } else if book_ok {
                "evaluated"
            } else {
                "not_evaluated"
            },
            bid1,
            ask1,
            spread,
            levels_bid: self.bids.len(),
            levels_ask: self.asks.len(),
            wall_side: self.wall.side.map(|s| match s {
                Side::Bid => "bid",
                Side::Ask => "ask",
            }),
            wall_price: wall_px,
            wall_size: wall_px.map(|_| self.wall.size),
            wall_on_poc,
            wall_on_stack,
            read,
            checksum_fail: self.checksum_fail,
            seq_gap: self.seq_gap,
            rebuilds: self.rebuilds,
            script_f,
        }
    }

    /// Freeze this 1m then drop bar-local eat/pull counters. Wall identity stays.
    pub fn freeze_bar(&mut self, poc: Option<f64>, stack_prices: &[f64]) -> BookRead {
        let snap = self.freeze_1m(poc, stack_prices);
        self.trades_at.clear();
        self.wall.eaten = 0.0;
        self.wall.pulled = 0.0;
        self.wall.replenished = 0.0;
        snap
    }
}

fn classify_wall(wall: &WallTrack, last_px: Option<f64>, tick_sz: f64) -> WallRead {
    let Some(side) = wall.side else {
        return WallRead::NotEvaluated;
    };
    if wall.size == 0.0 && wall.eaten == 0.0 && wall.pulled == 0.0 && wall.replenished == 0.0 {
        return WallRead::NotEvaluated;
    }
    let wall_px = px_of(wall.tick, tick_sz);
    let crossed = match (side, last_px) {
        (Side::Ask, Some(px)) => px > wall_px + tick_sz * 0.1,
        (Side::Bid, Some(px)) => px < wall_px - tick_sz * 0.1,
        _ => false,
    };
    let mostly_eaten = wall.eaten > 0.0 && wall.eaten >= wall.pulled;
    let mostly_pulled = wall.pulled > 0.0 && wall.pulled > wall.eaten;
    let spoof_restore = wall.pulled > 0.0 && wall.replenished > 0.0 && wall.eaten < wall.pulled;
    let held = !crossed && (wall.replenished > 0.0 || (wall.eaten > 0.0 && wall.size > 0.0));
    if crossed && mostly_eaten {
        WallRead::EatThrough
    } else if crossed && mostly_pulled && wall.eaten > 0.0 {
        WallRead::FakeWall
    } else if spoof_restore {
        WallRead::FakeWall
    } else if (crossed || wall.size == 0.0) && mostly_pulled && wall.eaten == 0.0 {
        WallRead::Yield
    } else if held {
        WallRead::Absorb
    } else {
        WallRead::NotEvaluated
    }
}

/// Three independent books. Poisoning OKX does not mark Binance/Bybit.
pub struct ThreeBooks {
    pub okx: BookEngine,
    pub binance: BookEngine,
    pub bybit: BookEngine,
}

impl ThreeBooks {
    pub fn sol() -> Self {
        Self {
            okx: BookEngine::new(Venue::Okx, VenueRole::Execution, BookConfig::sol()),
            binance: BookEngine::new(Venue::Binance, VenueRole::Resonance, BookConfig::sol()),
            bybit: BookEngine::new(Venue::Bybit, VenueRole::Resonance, BookConfig::sol()),
        }
    }

    pub fn engine(&mut self, venue: Venue) -> &mut BookEngine {
        match venue {
            Venue::Okx => &mut self.okx,
            Venue::Binance => &mut self.binance,
            Venue::Bybit => &mut self.bybit,
        }
    }

    pub fn apply_quality(&self, q: &mut QualityVector) {
        self.okx.apply_quality(q);
        self.binance.apply_quality(q);
        self.bybit.apply_quality(q);
    }
}
