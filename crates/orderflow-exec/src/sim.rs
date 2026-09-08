//! Local matching on the OKX book. Partial fills + queue-ahead. No peer prices.

use orderflow_domain::{TakerSide, Trade, Venue};
use serde::{Deserialize, Serialize};

use crate::intent::Side;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookLevel {
    pub price: f64,
    pub size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimBook {
    pub venue: Venue,
    pub tick_sz: f64,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

impl SimBook {
    pub fn okx(tick_sz: f64, bids: &[(f64, f64)], asks: &[(f64, f64)]) -> Self {
        Self {
            venue: Venue::Okx,
            tick_sz,
            bids: bids
                .iter()
                .map(|(p, s)| BookLevel {
                    price: *p,
                    size: *s,
                })
                .collect(),
            asks: asks
                .iter()
                .map(|(p, s)| BookLevel {
                    price: *p,
                    size: *s,
                })
                .collect(),
        }
    }

    pub fn size_at(&self, side: Side, price: f64) -> f64 {
        let levels = match side {
            Side::Buy => &self.bids,
            Side::Sell => &self.asks,
        };
        levels
            .iter()
            .find(|l| (l.price - price).abs() <= self.tick_sz * 0.5 + 1e-12)
            .map(|l| l.size)
            .unwrap_or(0.0)
    }

    pub fn best_bid(&self) -> Option<f64> {
        self.bids
            .iter()
            .max_by(|a, b| a.price.total_cmp(&b.price))
            .map(|l| l.price)
    }

    pub fn best_ask(&self) -> Option<f64> {
        self.asks
            .iter()
            .min_by(|a, b| a.price.total_cmp(&b.price))
            .map(|l| l.price)
    }

    pub fn opposite_best(&self, side: Side) -> Option<f64> {
        match side {
            Side::Buy => self.best_ask(),
            Side::Sell => self.best_bid(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingOrder {
    pub client_id: String,
    pub symbol: String,
    pub side: Side,
    pub limit_px: f64,
    pub qty: f64,
    pub filled: f64,
    /// Size already resting at our price when we joined. Trades eat this first.
    pub queue_ahead: f64,
    pub script: Option<String>,
}

impl WorkingOrder {
    pub fn remaining(&self) -> f64 {
        (self.qty - self.filled).max(0.0)
    }

    pub fn done(&self) -> bool {
        self.remaining() <= 1e-12
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub client_id: String,
    pub symbol: String,
    pub side: Side,
    pub price: f64,
    pub qty: f64,
    pub maker: bool,
    pub venue: Venue,
}

/// Apply one OKX public trade to a resting limit. Other venues never fill us.
pub fn match_trade(order: &mut WorkingOrder, trade: &Trade) -> Option<Fill> {
    if trade.venue != Venue::Okx {
        return None;
    }
    let crosses = match order.side {
        Side::Buy => trade.price <= order.limit_px + 1e-12,
        Side::Sell => trade.price >= order.limit_px - 1e-12,
    };
    if !crosses {
        return None;
    }
    let hits = match order.side {
        Side::Buy => trade.taker_side == TakerSide::Sell,
        Side::Sell => trade.taker_side == TakerSide::Buy,
    };
    if !hits {
        return None;
    }
    let mut left = trade.size;
    if order.queue_ahead > 0.0 {
        let eat = order.queue_ahead.min(left);
        order.queue_ahead -= eat;
        left -= eat;
    }
    if left <= 1e-12 {
        return None;
    }
    let qty = left.min(order.remaining());
    if qty <= 1e-12 {
        return None;
    }
    order.filled += qty;
    Some(Fill {
        client_id: order.client_id.clone(),
        symbol: order.symbol.clone(),
        side: order.side,
        price: order.limit_px,
        qty,
        maker: true,
        venue: Venue::Okx,
    })
}

/// Kill-switch / risk flatten: take the OKX opposite best. Not a peer price.
pub fn taker_fill(order: &WorkingOrder, book: &SimBook) -> Option<Fill> {
    if book.venue != Venue::Okx {
        return None;
    }
    let px = book.opposite_best(order.side)?;
    let qty = order.remaining();
    if qty <= 1e-12 {
        return None;
    }
    Some(Fill {
        client_id: order.client_id.clone(),
        symbol: order.symbol.clone(),
        side: order.side,
        price: px,
        qty,
        maker: false,
        venue: Venue::Okx,
    })
}
