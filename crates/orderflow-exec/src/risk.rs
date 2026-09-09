//! Account risk, kill switch, degrade. Independent of the sentence layer.

use std::fs;
use std::path::Path;

use orderflow_domain::RiskPlaceholder;
use serde::{Deserialize, Serialize};

use crate::intent::{OrderIntent, Side, Universe};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KillAction {
    Off,
    CancelAll,
    FlattenAll,
    ReduceOnly,
    Halt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Degrade {
    None = 0,
    /// Drop research universe first.
    StopResearch = 1,
    /// Then low-weight scripts.
    StopLowWeight = 2,
    /// Then new opens. Flatten and risk still run.
    StopOpens = 3,
    /// Last: flatten / risk only.
    RiskOnly = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillState {
    pub action: KillAction,
    pub reason: String,
    pub tripped_at_ms: i64,
}

impl Default for KillState {
    fn default() -> Self {
        Self {
            action: KillAction::Off,
            reason: String::new(),
            tripped_at_ms: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RiskEngine {
    pub limits: RiskPlaceholder,
    pub kill: KillState,
    pub degrade: Degrade,
    pub day_trades: u32,
    pub realized_pnl: f64,
    pub mark: f64,
    pub liq_px: Option<f64>,
    pub pos_qty: f64,
}

impl RiskEngine {
    pub fn new(limits: RiskPlaceholder) -> Self {
        Self {
            limits,
            kill: KillState::default(),
            degrade: Degrade::None,
            day_trades: 0,
            realized_pnl: 0.0,
            mark: 0.0,
            liq_px: None,
            pos_qty: 0.0,
        }
    }

    pub fn trip(&mut self, action: KillAction, reason: impl Into<String>, now_ms: i64) {
        self.kill = KillState {
            action,
            reason: reason.into(),
            tripped_at_ms: now_ms,
        };
    }

    pub fn persist(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let s = serde_json::to_string(&self.kill).map_err(|e| e.to_string())?;
        fs::write(path, s).map_err(|e| e.to_string())
    }

    pub fn load_persist(&mut self, path: &Path) -> Result<(), String> {
        if self.limits.kill_switch_clear_on_start {
            return Ok(());
        }
        if !path.exists() {
            return Ok(());
        }
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.kill = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn unrealized(&self) -> f64 {
        if self.mark <= 0.0 || self.pos_qty == 0.0 {
            return 0.0;
        }
        // Caller should keep avg elsewhere; here mark vs 0 is only used when
        // tests set realized directly. Unrealized is mark-to-liq helper.
        0.0
    }

    pub fn daily_pnl(&self, unrealized: f64) -> f64 {
        self.realized_pnl + unrealized
    }

    pub fn liq_buffer_breached(&self) -> bool {
        let Some(liq) = self.liq_px else {
            return false;
        };
        if self.mark <= 0.0 || self.pos_qty == 0.0 {
            return false;
        }
        let dist = if self.pos_qty > 0.0 {
            (self.mark - liq) / self.mark
        } else {
            (liq - self.mark) / self.mark
        };
        dist < self.limits.liq_buffer_pct
    }

    pub fn daily_loss_tripped(&self, unrealized: f64) -> bool {
        self.daily_pnl(unrealized) <= -self.limits.daily_loss_halt
    }

    pub fn allow_open(&self, intent: &OrderIntent, unrealized: f64) -> Result<(), &'static str> {
        match self.kill.action {
            KillAction::Halt => return Err("kill_halt"),
            KillAction::ReduceOnly | KillAction::FlattenAll | KillAction::CancelAll => {
                return Err("kill_reduce_only")
            }
            KillAction::Off => {}
        }
        if self.degrade >= Degrade::StopOpens {
            return Err("degrade_stop_opens");
        }
        if self.degrade >= Degrade::StopLowWeight && intent.low_weight {
            return Err("degrade_low_weight");
        }
        if self.degrade >= Degrade::StopResearch && intent.universe == Universe::Research {
            return Err("degrade_research");
        }
        if self.daily_loss_tripped(unrealized) {
            return Err("daily_loss");
        }
        if self.liq_buffer_breached() {
            return Err("liq_buffer");
        }
        if self.day_trades >= self.limits.max_day_trades {
            return Err("max_day_trades");
        }
        Ok(())
    }

    pub fn allow_flatten(&self) -> Result<(), &'static str> {
        match self.kill.action {
            KillAction::Halt => Err("kill_halt"),
            _ => Ok(()),
        }
    }

    /// Risk-first size. Tiny stops are clipped by caps, never explode.
    pub fn size_qty(
        &self,
        entry: f64,
        invalidation: f64,
        tick_sz: f64,
        ct_val: f64,
        lot: f64,
    ) -> f64 {
        let stop = (entry - invalidation).abs().max(tick_sz);
        let risk_capital = self.limits.equity * self.limits.risk_pct;
        let mut qty = risk_capital / stop;
        if ct_val > 0.0 {
            qty /= ct_val;
        }
        let mut notional = qty * entry * ct_val.max(1e-12);
        let cap = self
            .limits
            .symbol_cap_notional
            .min(self.limits.account_cap_notional);
        if notional > cap && entry > 0.0 {
            notional = cap;
            qty = notional / entry / ct_val.max(1e-12);
        }
        let lev_notional = self.limits.equity * self.limits.leverage_cap;
        if qty * entry * ct_val.max(1e-12) > lev_notional && entry > 0.0 {
            qty = lev_notional / entry / ct_val.max(1e-12);
        }
        if lot > 0.0 {
            qty = (qty / lot).floor() * lot;
        }
        qty.max(0.0)
    }

    pub fn stop_too_tight(entry: f64, invalidation: f64, tick_sz: f64) -> bool {
        (entry - invalidation).abs() + 1e-12 < tick_sz
    }

    pub fn side_ok_for_reduce(pos_qty: f64, side: Side) -> bool {
        match side {
            Side::Sell => pos_qty > 0.0,
            Side::Buy => pos_qty < 0.0,
        }
    }
}
