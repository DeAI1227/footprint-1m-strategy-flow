//! Numbers come from `params/*.toml`. Do not invent a VWAP length or a daily POC.

use orderflow_domain::{ResonanceMode, SymbolParams};

pub const STACK_WINDOWS: [u32; 5] = [5, 15, 60, 240, 1440];
pub const FORBIDDEN: &[&str] = &["vwap", "avwap", "tpo", "market_profile", "naked_poc"];

#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub swing_n: u32,
    pub leave_bars: u32,
    pub accept_bars: u32,
    pub trap_bars: u32,
    pub liq_oi_1h_veto_pct: f64,
    pub liq_1m_notional_rule: String,
    pub funding_hours_utc: Vec<u32>,
    pub funding_black_window_minutes: u32,
    pub resonance: ResonanceMode,
    pub resonance_k: u32,
}

impl ContextConfig {
    pub fn from_symbol(p: &SymbolParams, resonance_k: u32) -> Result<Self, String> {
        if p.swing_n == 0 {
            return Err("swing_n must be > 0".into());
        }
        if p.accept_bars == 0 {
            return Err("accept_bars must be > 0".into());
        }
        if p.liq_1m_notional_rule != "sample_p95" {
            return Err(
                "liq_1m_notional_rule must be sample_p95 (never a frozen USDT constant)".into(),
            );
        }
        Ok(Self {
            swing_n: p.swing_n,
            leave_bars: p.leave_bars,
            accept_bars: p.accept_bars,
            trap_bars: p.trap_bars,
            liq_oi_1h_veto_pct: p.liq_oi_1h_veto_pct,
            liq_1m_notional_rule: p.liq_1m_notional_rule.clone(),
            funding_hours_utc: p.funding_hours_utc.clone(),
            funding_black_window_minutes: p.funding_black_window_minutes,
            resonance: p.resonance,
            resonance_k,
        })
    }

    pub fn golden_sol() -> Self {
        Self {
            swing_n: 5,
            leave_bars: 1,
            accept_bars: 3,
            trap_bars: 3,
            liq_oi_1h_veto_pct: -0.02,
            liq_1m_notional_rule: "sample_p95".into(),
            funding_hours_utc: vec![0, 8, 16],
            funding_black_window_minutes: 15,
            resonance: ResonanceMode::Off,
            resonance_k: 1,
        }
    }
}
