//! Numbers come from `params/*.toml`. Do not hard-code 400% or a SOL lot size.

use orderflow_domain::{ArmedRatePolicy, ImbalanceStyle, SymbolParams, ValueAreaScope};

#[derive(Debug, Clone)]
pub struct FootprintConfig {
    pub symbol: String,
    pub bucket: f64,
    pub ignore_zero: bool,
    pub imbalance_style: ImbalanceStyle,
    pub imbalance_rate_record: f64,
    pub imbalance_rate_dale: f64,
    pub imbalance_rate_valtos: f64,
    pub armed_rate_policy: ArmedRatePolicy,
    pub stack_min_levels: u32,
    pub stack_require_bar_direction: bool,
    pub value_area_pct: f64,
    pub value_area_scope: ValueAreaScope,
    pub unfinished_is_entry: bool,
    pub min_imbalance_volume_rule: String,
}

impl FootprintConfig {
    pub fn from_symbol(p: &SymbolParams) -> Result<Self, String> {
        if (p.imbalance_rate_dale - p.imbalance_rate_valtos).abs() < f64::EPSILON {
            return Err("do not collapse Dale 300% and Valtos 400% into one number".into());
        }
        if p.imbalance_style != ImbalanceStyle::Diagonal {
            return Err("imbalance_style must be diagonal".into());
        }
        if p.value_area_scope != ValueAreaScope::Bar {
            return Err("value_area_scope must be bar (not a daily profile)".into());
        }
        if p.bucket <= 0.0 {
            return Err("bucket must be > 0".into());
        }
        if p.unfinished_is_entry {
            return Err("unfinished_is_entry must stay false".into());
        }
        Ok(Self {
            symbol: p.symbol.clone(),
            bucket: p.bucket,
            ignore_zero: p.ignore_zero,
            imbalance_style: p.imbalance_style,
            imbalance_rate_record: p.imbalance_rate_record,
            imbalance_rate_dale: p.imbalance_rate_dale,
            imbalance_rate_valtos: p.imbalance_rate_valtos,
            armed_rate_policy: p.armed_rate_policy,
            stack_min_levels: p.stack_min_levels,
            stack_require_bar_direction: p.stack_require_bar_direction,
            value_area_pct: p.value_area_pct,
            value_area_scope: p.value_area_scope,
            unfinished_is_entry: false,
            min_imbalance_volume_rule: p.min_imbalance_volume_rule.clone(),
        })
    }

    /// Tiny golden tests: SOL-like bucket, injected min volume, still 300∥400.
    pub fn golden_sol() -> Self {
        Self {
            symbol: "SOL".into(),
            bucket: 0.01,
            ignore_zero: true,
            imbalance_style: ImbalanceStyle::Diagonal,
            imbalance_rate_record: 2.0,
            imbalance_rate_dale: 3.0,
            imbalance_rate_valtos: 4.0,
            armed_rate_policy: ArmedRatePolicy::Parallel,
            stack_min_levels: 3,
            stack_require_bar_direction: true,
            value_area_pct: 0.70,
            value_area_scope: ValueAreaScope::Bar,
            unfinished_is_entry: false,
            min_imbalance_volume_rule: "session_nonempty_side_p25_both".into(),
        }
    }
}
