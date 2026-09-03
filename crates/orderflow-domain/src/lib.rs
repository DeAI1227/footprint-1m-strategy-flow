//! Shared types, TOML config, JSON logs, and the live gate.
//!
//! Stage 0: no WebSocket, no footprint matrix, no orders.
//! Code reads numbers from `params/*.toml`. Do not hard-code 400% here.
//! School is footprint only — no Market Profile / TPO / VWAP / Naked POC / SMC / ICT.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const SCHOOL: &str = "footprint";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FORBIDDEN_SCHOOLS: &[&str] = &[
    "market_profile",
    "tpo",
    "vwap",
    "avwap",
    "naked_poc",
    "smc",
    "ict",
];

/// 1-minute bar on exchange event time `[t, t+60)`.
pub const BAR_INTERVAL_MS: i64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Shadow,
    Sim,
    LiveSmall,
    Live,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "shadow" => Ok(Self::Shadow),
            "sim" => Ok(Self::Sim),
            "live_small" => Ok(Self::LiveSmall),
            "live" => Ok(Self::Live),
            other => Err(format!(
                "unknown mode {other:?}; expected shadow|sim|live_small|live"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shadow => "shadow",
            Self::Sim => "sim",
            Self::LiveSmall => "live_small",
            Self::Live => "live",
        }
    }

    pub fn is_live(self) -> bool {
        matches!(self, Self::Live | Self::LiveSmall)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Venue {
    Okx,
    Binance,
    Bybit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueRole {
    Execution,
    Resonance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakerSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarState {
    Forming,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationStatus {
    /// 21-day observation draft. Not out-of-sample. Not live.
    ObservationDraft,
    OutOfSampleValidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResonanceMode {
    Off,
    KOfN,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmedRatePolicy {
    /// Dale 300% and Valtos 400% both computed. Do not average to 350%.
    Parallel,
    Dale300,
    Valtos400,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImbalanceStyle {
    Diagonal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueAreaScope {
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptFStatus {
    NotEvaluated,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveDenied {
    ParamsNotCalibrated,
    LiveFlagOff,
    ArmedRateStillParallel,
    ExecNotWired,
}

impl LiveDenied {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ParamsNotCalibrated => "params_not_calibrated",
            Self::LiveFlagOff => "live_flag_off",
            Self::ArmedRateStillParallel => "armed_rate_still_parallel",
            Self::ExecNotWired => "exec_not_wired",
        }
    }

    pub fn message_zh(&self) -> &'static str {
        match self {
            Self::ParamsNotCalibrated => "參數未校準：21 日觀察稿不是樣本外驗證，禁止 live",
            Self::LiveFlagOff => "live 旗標關閉，禁止開倉",
            Self::ArmedRateStillParallel => "武裝比率仍是 300∥400 並列，尚未選定，禁止 live",
            Self::ExecNotWired => "執行路徑未接線，即使旗標翻開也下不了單",
        }
    }
}

impl fmt::Display for LiveDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Normalized public trade. Adapters must emit `taker_buy` / `taker_sell` only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub venue: Venue,
    pub symbol: String,
    pub event_ts_ms: i64,
    pub recv_ts_ms: i64,
    pub processed_ts_ms: i64,
    pub price: f64,
    pub size: f64,
    pub taker_side: TakerSide,
}

/// 1m bar on exchange event time `[open_ms, open_ms+60_000)`.
/// Only `Closed` bars may drive entries. `Forming` is cancel/risk only.
/// Closed bars are never rewritten; late trades increment [`QualityVector::late_trade`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar1m {
    pub venue: Venue,
    pub symbol: String,
    pub open_ms: i64,
    pub state: BarState,
}

impl Bar1m {
    pub fn close_ms(&self) -> i64 {
        self.open_ms + BAR_INTERVAL_MS
    }

    pub fn entries_allowed(&self) -> bool {
        matches!(self.state, BarState::Closed)
    }
}

/// Per-venue quality. Do not sum venue volumes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityVector {
    pub late_trade: u32,
    pub okx_gap: bool,
    pub binance_gap: bool,
    pub bybit_gap: bool,
    pub okx_book_ok: bool,
    pub binance_book_ok: bool,
    pub bybit_book_ok: bool,
    pub liq_stream_missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolContract {
    pub symbol: String,
    pub okx_inst_id: String,
    pub binance_symbol: String,
    pub bybit_symbol: String,
    pub ct_val: f64,
    pub tick_sz: f64,
}

/// Dummy frozen 1m snapshot. Real snapshots land when the footprint engine is wired.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrozenSnapshot {
    pub schema_version: u32,
    pub wired: bool,
}

impl FrozenSnapshot {
    pub fn dummy() -> Self {
        Self {
            schema_version: 0,
            wired: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub mode_default: Mode,
    pub school: String,
    pub execution_venue: Venue,
    pub resonance: ResonanceMode,
    pub calibration: CalibrationGate,
    pub venues: VenuesConfig,
    pub risk: RiskPlaceholder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationGate {
    pub status: CalibrationStatus,
    pub live_authorized: bool,
    pub out_of_sample_validated: bool,
    pub calibration_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPlaceholder {
    pub shared_beta_cap_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenuesConfig {
    pub okx: VenueEndpoint,
    pub binance: VenueEndpoint,
    pub bybit: VenueEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueEndpoint {
    pub role: VenueRole,
    pub public_ws: String,
    pub rest: String,
    #[serde(default)]
    pub private_ws: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolParams {
    pub symbol: String,
    pub inst_id: String,
    pub okx_inst_id: String,
    pub binance_symbol: String,
    pub bybit_symbol: String,
    pub ct_val: f64,
    pub tick_sz: f64,
    pub bucket: f64,
    pub min_imbalance_volume_rule: String,
    pub imbalance_style: ImbalanceStyle,
    pub imbalance_rate_record: f64,
    pub imbalance_rate_dale: f64,
    pub imbalance_rate_valtos: f64,
    pub armed_rate_policy: ArmedRatePolicy,
    pub stack_min_levels: u32,
    pub stack_require_bar_direction: bool,
    pub ignore_zero: bool,
    pub value_area_pct: f64,
    pub value_area_scope: ValueAreaScope,
    pub swing_n: u32,
    pub leave_bars: u32,
    pub trap_bars: u32,
    pub unfinished_is_entry: bool,
    pub script_g_is_entry: bool,
    pub script_f: ScriptFStatus,
    pub liq_oi_1h_veto_pct: f64,
    pub liq_1m_notional_rule: String,
    pub funding_hours_utc: Vec<u32>,
    pub funding_black_window_minutes: u32,
    pub resonance: ResonanceMode,
    pub language_runnable: bool,
    pub live_enabled: bool,
    pub calibration_complete: bool,
    pub out_of_sample_validated: bool,
    #[serde(default)]
    pub shadow_only: bool,
}

impl SymbolParams {
    pub fn contract(&self) -> SymbolContract {
        SymbolContract {
            symbol: self.symbol.clone(),
            okx_inst_id: self.okx_inst_id.clone(),
            binance_symbol: self.binance_symbol.clone(),
            bybit_symbol: self.bybit_symbol.clone(),
            ct_val: self.ct_val,
            tick_sz: self.tick_sz,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub runtime: RuntimeConfig,
    pub sol: SymbolParams,
    pub sui: SymbolParams,
    pub config_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct BootDecision {
    pub ok: bool,
    pub mode: Mode,
    pub event: &'static str,
    pub live_gate: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<LiveDenied>,
    pub message: &'static str,
    pub school: &'static str,
    pub execution_venue: Venue,
    pub resonance: ResonanceMode,
    pub calibration_status: CalibrationStatus,
    pub calibration_complete: bool,
}

impl AppConfig {
    pub fn load(config_dir: &Path) -> Result<Self, String> {
        let runtime: RuntimeConfig = load_toml(&config_dir.join("runtime.toml"))?;
        let sol: SymbolParams = load_toml(&config_dir.join("sol.toml"))?;
        let sui: SymbolParams = load_toml(&config_dir.join("sui.toml"))?;
        if runtime.school != SCHOOL {
            return Err(format!(
                "runtime.school must be {SCHOOL:?}, got {:?}",
                runtime.school
            ));
        }
        if runtime.execution_venue != Venue::Okx {
            return Err("execution_venue must be okx".into());
        }
        if (sol.imbalance_rate_dale - sol.imbalance_rate_valtos).abs() < f64::EPSILON {
            return Err("do not collapse Dale 300% and Valtos 400% into one number".into());
        }
        if (sui.imbalance_rate_dale - sui.imbalance_rate_valtos).abs() < f64::EPSILON {
            return Err("do not collapse Dale 300% and Valtos 400% into one number".into());
        }
        if (sui.bucket - sol.bucket).abs() < f64::EPSILON {
            return Err("SUI bucket must not copy SOL bucket".into());
        }
        Ok(Self {
            runtime,
            sol,
            sui,
            config_dir: config_dir.to_path_buf(),
        })
    }
}

fn load_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    toml::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// Hard gate: live / live_small may never open until out-of-sample validation.
/// Stage 0 also refuses because the execution crate is not wired.
/// Observation freeze is not calibration_complete.
pub fn live_open_allowed(mode: Mode, cfg: &AppConfig) -> Result<(), LiveDenied> {
    if !mode.is_live() {
        return Ok(());
    }
    let g = &cfg.runtime.calibration;
    if !g.calibration_complete
        || g.status != CalibrationStatus::OutOfSampleValidated
        || !g.out_of_sample_validated
        || !cfg.sol.calibration_complete
        || !cfg.sol.out_of_sample_validated
    {
        return Err(LiveDenied::ParamsNotCalibrated);
    }
    if !g.live_authorized || !cfg.sol.live_enabled {
        return Err(LiveDenied::LiveFlagOff);
    }
    if cfg.sol.armed_rate_policy == ArmedRatePolicy::Parallel {
        return Err(LiveDenied::ArmedRateStillParallel);
    }
    Err(LiveDenied::ExecNotWired)
}

/// Stage 0: always false. Observation draft cannot authorize live.
pub fn live_allowed() -> bool {
    false
}

pub fn boot_decision(mode: Mode, cfg: &AppConfig) -> BootDecision {
    match live_open_allowed(mode, cfg) {
        Ok(()) if !mode.is_live() => BootDecision {
            ok: true,
            mode,
            event: "boot",
            live_gate: "closed",
            reason: None,
            message: "shadow/sim 已啟動；live 閘門關閉（觀察稿未樣本外驗證）",
            school: SCHOOL,
            execution_venue: cfg.runtime.execution_venue,
            resonance: cfg.runtime.resonance,
            calibration_status: cfg.runtime.calibration.status,
            calibration_complete: cfg.runtime.calibration.calibration_complete,
        },
        Ok(()) => BootDecision {
            ok: false,
            mode,
            event: "live_denied",
            live_gate: "closed",
            reason: Some(LiveDenied::ExecNotWired),
            message: LiveDenied::ExecNotWired.message_zh(),
            school: SCHOOL,
            execution_venue: cfg.runtime.execution_venue,
            resonance: cfg.runtime.resonance,
            calibration_status: cfg.runtime.calibration.status,
            calibration_complete: cfg.runtime.calibration.calibration_complete,
        },
        Err(reason) => BootDecision {
            ok: false,
            mode,
            event: "live_denied",
            live_gate: "closed",
            reason: Some(reason),
            message: reason.message_zh(),
            school: SCHOOL,
            execution_venue: cfg.runtime.execution_venue,
            resonance: cfg.runtime.resonance,
            calibration_status: cfg.runtime.calibration.status,
            calibration_complete: cfg.runtime.calibration.calibration_complete,
        },
    }
}

pub fn json_log(level: &str, decision: &BootDecision) -> String {
    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut v = serde_json::to_value(decision).expect("boot decision json");
    if let Some(obj) = v.as_object_mut() {
        obj.insert("ts".into(), serde_json::Value::String(ts));
        obj.insert("level".into(), serde_json::Value::String(level.into()));
    }
    let line = serde_json::to_string(&v).expect("log line");
    debug_assert!(!line.to_ascii_lowercase().contains("apikey"));
    debug_assert!(!line.to_ascii_lowercase().contains("secret"));
    debug_assert!(!line.to_ascii_lowercase().contains("passphrase"));
    line
}

pub fn default_config_dir() -> PathBuf {
    PathBuf::from("params")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_repo_params() -> AppConfig {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("params");
        AppConfig::load(&root).expect("repo params must load")
    }

    #[test]
    fn shadow_boots_and_live_gate_stays_closed() {
        let cfg = load_repo_params();
        let d = boot_decision(Mode::Shadow, &cfg);
        assert!(d.ok);
        assert_eq!(d.live_gate, "closed");
        assert!(d.reason.is_none());
        assert!(!d.calibration_complete);
        assert!(d.message.contains("live 閘門關閉"));
    }

    #[test]
    fn sim_boots_same_gate() {
        let cfg = load_repo_params();
        assert!(boot_decision(Mode::Sim, &cfg).ok);
    }

    #[test]
    fn live_refused_because_params_not_calibrated() {
        let cfg = load_repo_params();
        let d = boot_decision(Mode::Live, &cfg);
        assert!(!d.ok);
        assert_eq!(d.reason, Some(LiveDenied::ParamsNotCalibrated));
        assert_eq!(d.message, LiveDenied::ParamsNotCalibrated.message_zh());
        assert!(d.message.contains("參數未校準"));
        let d2 = boot_decision(Mode::LiveSmall, &cfg);
        assert_eq!(d2.reason, Some(LiveDenied::ParamsNotCalibrated));
    }

    #[test]
    fn live_allowed_is_always_false_in_stage_0() {
        assert!(!live_allowed());
    }

    #[test]
    fn even_flipped_flags_cannot_skip_exec() {
        let mut cfg = load_repo_params();
        cfg.runtime.calibration.status = CalibrationStatus::OutOfSampleValidated;
        cfg.runtime.calibration.out_of_sample_validated = true;
        cfg.runtime.calibration.calibration_complete = true;
        cfg.runtime.calibration.live_authorized = true;
        cfg.sol.live_enabled = true;
        cfg.sol.calibration_complete = true;
        cfg.sol.out_of_sample_validated = true;
        cfg.sol.armed_rate_policy = ArmedRatePolicy::Dale300;
        let err = live_open_allowed(Mode::Live, &cfg).unwrap_err();
        assert_eq!(err, LiveDenied::ExecNotWired);
    }

    #[test]
    fn does_not_average_300_and_400() {
        let cfg = load_repo_params();
        assert_eq!(cfg.sol.imbalance_rate_dale, 3.0);
        assert_eq!(cfg.sol.imbalance_rate_valtos, 4.0);
        assert_eq!(cfg.sol.armed_rate_policy, ArmedRatePolicy::Parallel);
        assert_eq!(cfg.sol.bucket, 0.01);
        assert_eq!(cfg.sui.bucket, 0.0001);
        assert_ne!(cfg.sol.bucket, cfg.sui.bucket);
        assert_eq!(cfg.sol.imbalance_style, ImbalanceStyle::Diagonal);
        assert_eq!(cfg.sol.value_area_scope, ValueAreaScope::Bar);
        assert_eq!(cfg.sol.script_f, ScriptFStatus::NotEvaluated);
        assert!(!cfg.sol.unfinished_is_entry);
        assert!(!cfg.sol.script_g_is_entry);
        assert_eq!(cfg.sol.swing_n, 5);
        assert_eq!(cfg.sol.leave_bars, 1);
        assert_eq!(cfg.sol.trap_bars, 3);
        assert_eq!(cfg.sol.liq_oi_1h_veto_pct, -0.02);
        assert_eq!(cfg.sol.liq_1m_notional_rule, "sample_p95");
        assert_eq!(cfg.sol.funding_hours_utc, vec![0, 8, 16]);
        assert_eq!(cfg.sol.funding_black_window_minutes, 15);
    }

    #[test]
    fn execution_is_okx_resonance_off() {
        let cfg = load_repo_params();
        assert_eq!(cfg.runtime.execution_venue, Venue::Okx);
        assert_eq!(cfg.runtime.venues.okx.role, VenueRole::Execution);
        assert_eq!(cfg.runtime.venues.binance.role, VenueRole::Resonance);
        assert_eq!(cfg.runtime.venues.bybit.role, VenueRole::Resonance);
        assert_eq!(cfg.runtime.resonance, ResonanceMode::Off);
        assert_eq!(cfg.sol.resonance, ResonanceMode::Off);
        assert_eq!(cfg.sui.resonance, ResonanceMode::Off);
        assert!(!cfg.runtime.calibration.calibration_complete);
        assert!(!cfg.runtime.calibration.out_of_sample_validated);
        assert!(!cfg.runtime.risk.shared_beta_cap_enabled);
    }

    #[test]
    fn closed_bar_is_immutable_contract() {
        let bar = Bar1m {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            open_ms: 0,
            state: BarState::Closed,
        };
        assert!(bar.entries_allowed());
        assert_eq!(bar.close_ms(), BAR_INTERVAL_MS);
        let forming = Bar1m {
            state: BarState::Forming,
            ..bar.clone()
        };
        assert!(!forming.entries_allowed());
    }

    #[test]
    fn json_log_has_no_secrets() {
        let cfg = load_repo_params();
        let line = json_log("info", &boot_decision(Mode::Shadow, &cfg));
        let lower = line.to_ascii_lowercase();
        assert!(!lower.contains("secret"));
        assert!(!lower.contains("apikey"));
        assert!(!lower.contains("passphrase"));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["school"], "footprint");
    }

    #[test]
    fn frozen_snapshot_is_dummy() {
        let s = FrozenSnapshot::dummy();
        assert!(!s.wired);
        assert_eq!(s.schema_version, 0);
    }
}
