//! Intents from the Python sentence layer. Never carry a peer-venue price.

use orderflow_domain::Venue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }

    pub fn sign(self) -> f64 {
        match self {
            Self::Buy => 1.0,
            Self::Sell => -1.0,
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentKind {
    ShadowSignal,
    SimOpen,
    Flatten,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Universe {
    Core,
    Research,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderIntent {
    pub kind: IntentKind,
    pub symbol: String,
    pub side: Side,
    /// Limit must come from OKX structure. Peer prices are forbidden.
    pub limit_px: Option<f64>,
    pub invalidation_px: Option<f64>,
    pub qty: Option<f64>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub copied_price_onto_okx: bool,
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub flatten_only: bool,
    #[serde(default)]
    pub low_weight: bool,
    #[serde(default)]
    pub universe: Universe,
    /// Must stay OKX. Binance/Bybit prices never land here.
    #[serde(default = "execution_venue")]
    pub venue: Venue,
}

fn execution_venue() -> Venue {
    Venue::Okx
}

impl Default for Universe {
    fn default() -> Self {
        Self::Core
    }
}

impl OrderIntent {
    pub fn sim_open(symbol: &str, side: Side, limit_px: f64, qty: f64) -> Self {
        Self {
            kind: IntentKind::SimOpen,
            symbol: symbol.to_string(),
            side,
            limit_px: Some(limit_px),
            invalidation_px: Some(if side == Side::Buy {
                limit_px - 1.0
            } else {
                limit_px + 1.0
            }),
            qty: Some(qty),
            script: Some("A".into()),
            client_id: None,
            copied_price_onto_okx: false,
            live: false,
            flatten_only: false,
            low_weight: false,
            universe: Universe::Core,
            venue: Venue::Okx,
        }
    }
}
