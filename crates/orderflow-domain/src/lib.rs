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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Venue {
    Okx,
    Binance,
    Bybit,
}

impl Venue {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "okx" => Ok(Self::Okx),
            "binance" => Ok(Self::Binance),
            "bybit" => Ok(Self::Bybit),
            other => Err(format!(
                "unknown venue {other:?}; expected okx|binance|bybit"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Okx => "okx",
            Self::Binance => "binance",
            Self::Bybit => "bybit",
        }
    }
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

/// Floor exchange event time to the 1m bar open `[t, t+60_000)`.
pub fn bar_open_ms(event_ts_ms: i64) -> i64 {
    event_ts_ms - event_ts_ms.rem_euclid(BAR_INTERVAL_MS)
}

/// Normalized public trade. Adapters must emit `taker_buy` / `taker_sell` only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub venue: Venue,
    pub symbol: String,
    #[serde(default)]
    pub trade_id: Option<String>,
    pub event_ts_ms: i64,
    pub recv_ts_ms: i64,
    pub processed_ts_ms: i64,
    pub price: f64,
    pub size: f64,
    pub taker_side: TakerSide,
}

impl Trade {
    pub fn bar_open_ms(&self) -> i64 {
        bar_open_ms(self.event_ts_ms)
    }

    /// Taker buy hits the ask; taker sell hits the bid.
    pub fn is_taker_buy(&self) -> bool {
        matches!(self.taker_side, TakerSide::Buy)
    }
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
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// Taker-sell volume (hits bid).
    pub bid_vol: f64,
    /// Taker-buy volume (hits ask).
    pub ask_vol: f64,
    pub trade_count: u32,
    pub first_trade_ts_ms: i64,
    pub last_trade_ts_ms: i64,
}

impl Bar1m {
    pub fn close_ms(&self) -> i64 {
        self.open_ms + BAR_INTERVAL_MS
    }

    pub fn entries_allowed(&self) -> bool {
        matches!(self.state, BarState::Closed)
    }

    pub fn delta(&self) -> f64 {
        self.ask_vol - self.bid_vol
    }

    pub fn volume(&self) -> f64 {
        self.ask_vol + self.bid_vol
    }

    pub fn into_closed(mut self) -> Self {
        self.state = BarState::Closed;
        self
    }
}

/// Per-venue quality. Do not sum venue volumes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityVector {
    pub late_trade: u32,
    pub out_of_order: u32,
    pub reconnect: u32,
    pub gap_minutes: u32,
    pub trades_seen: u64,
    pub bars_closed: u64,
    pub okx_gap: bool,
    pub binance_gap: bool,
    pub bybit_gap: bool,
    pub okx_book_ok: bool,
    pub binance_book_ok: bool,
    pub bybit_book_ok: bool,
    pub liq_stream_missing: bool,
    /// Private WS seen and healthy. Shadow/sim do not require this to boot.
    #[serde(default)]
    pub private_ok: bool,
}

impl QualityVector {
    /// Queue overflow / stall on one venue. Never used to block the other two.
    pub fn mark_gap(&mut self, venue: Venue) {
        match venue {
            Venue::Okx => self.okx_gap = true,
            Venue::Binance => self.binance_gap = true,
            Venue::Bybit => self.bybit_gap = true,
        }
    }

    pub fn gap(&self, venue: Venue) -> bool {
        match venue {
            Venue::Okx => self.okx_gap,
            Venue::Binance => self.binance_gap,
            Venue::Bybit => self.bybit_gap,
        }
    }

    pub fn set_book_ok(&mut self, venue: Venue, ok: bool) {
        match venue {
            Venue::Okx => self.okx_book_ok = ok,
            Venue::Binance => self.binance_book_ok = ok,
            Venue::Bybit => self.bybit_book_ok = ok,
        }
    }

    pub fn book_ok(&self, venue: Venue) -> bool {
        match venue {
            Venue::Okx => self.okx_book_ok,
            Venue::Binance => self.binance_book_ok,
            Venue::Bybit => self.bybit_book_ok,
        }
    }
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
    /// Peers that must agree with OKX when mode is k_of_n. Observation placeholder.
    #[serde(default = "default_resonance_k")]
    pub resonance_k: u32,
    pub calibration: CalibrationGate,
    pub venues: VenuesConfig,
    pub risk: RiskPlaceholder,
    #[serde(default)]
    pub exec: ExecConfig,
    #[serde(default)]
    pub ops: OpsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecConfig {
    /// OKX margin mode. Observation placeholder.
    #[serde(default = "default_td_mode")]
    pub td_mode: String,
    /// Expected OKX posMode. Mismatch blocks new opens.
    #[serde(default = "default_pos_mode")]
    pub pos_mode: String,
    /// Third lock: even if calibration + live flags flip, this stays false in repo.
    #[serde(default)]
    pub live_send: bool,
    #[serde(default = "default_entry_ttl_ms")]
    pub entry_ttl_ms: u64,
    #[serde(default = "default_max_amend")]
    pub max_amend: u32,
    #[serde(default = "default_max_slippage_ticks")]
    pub max_slippage_ticks: u32,
}

impl Default for ExecConfig {
    fn default() -> Self {
        Self {
            td_mode: default_td_mode(),
            pos_mode: default_pos_mode(),
            live_send: false,
            entry_ttl_ms: default_entry_ttl_ms(),
            max_amend: default_max_amend(),
            max_slippage_ticks: default_max_slippage_ticks(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsConfig {
    #[serde(default = "default_crash_window_s")]
    pub crash_window_s: u32,
    #[serde(default = "default_crash_burst")]
    pub crash_burst: u32,
    #[serde(default)]
    pub clear_crash_on_start: bool,
    #[serde(default = "default_log_max_bytes")]
    pub log_max_bytes: u64,
    #[serde(default = "default_log_keep")]
    pub log_keep: u32,
    #[serde(default = "default_disk_min_free")]
    pub disk_min_free_bytes: u64,
    #[serde(default = "default_hot_journal_days")]
    pub hot_journal_days: u32,
}

impl Default for OpsConfig {
    fn default() -> Self {
        Self {
            crash_window_s: default_crash_window_s(),
            crash_burst: default_crash_burst(),
            clear_crash_on_start: false,
            log_max_bytes: default_log_max_bytes(),
            log_keep: default_log_keep(),
            disk_min_free_bytes: default_disk_min_free(),
            hot_journal_days: default_hot_journal_days(),
        }
    }
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
    /// Fraction of equity risked per new open. Observation placeholder.
    #[serde(default = "default_risk_pct")]
    pub risk_pct: f64,
    #[serde(default = "default_symbol_cap")]
    pub symbol_cap_notional: f64,
    #[serde(default = "default_account_cap")]
    pub account_cap_notional: f64,
    #[serde(default = "default_leverage_cap")]
    pub leverage_cap: f64,
    #[serde(default = "default_daily_loss_halt")]
    pub daily_loss_halt: f64,
    #[serde(default = "default_liq_buffer_pct")]
    pub liq_buffer_pct: f64,
    #[serde(default = "default_max_day_trades")]
    pub max_day_trades: u32,
    #[serde(default = "default_equity")]
    pub equity: f64,
    /// Restart must not clear a tripped kill switch unless this is true.
    #[serde(default)]
    pub kill_switch_clear_on_start: bool,
    #[serde(default = "default_reconcile_every_s")]
    pub reconcile_every_s: u32,
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
    /// Consecutive outside POCs after a leave. Week-2 script D used 3. Not calibrated.
    #[serde(default = "default_accept_bars")]
    pub accept_bars: u32,
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

fn default_resonance_k() -> u32 {
    1
}

fn default_accept_bars() -> u32 {
    3
}

fn default_risk_pct() -> f64 {
    0.002
}

fn default_symbol_cap() -> f64 {
    1000.0
}

fn default_account_cap() -> f64 {
    2000.0
}

fn default_leverage_cap() -> f64 {
    3.0
}

fn default_daily_loss_halt() -> f64 {
    50.0
}

fn default_liq_buffer_pct() -> f64 {
    0.05
}

fn default_max_day_trades() -> u32 {
    20
}

fn default_equity() -> f64 {
    10_000.0
}

fn default_reconcile_every_s() -> u32 {
    30
}

fn default_td_mode() -> String {
    "cross".into()
}

fn default_pos_mode() -> String {
    "net_mode".into()
}

fn default_entry_ttl_ms() -> u64 {
    15_000
}

fn default_max_amend() -> u32 {
    2
}

fn default_max_slippage_ticks() -> u32 {
    4
}

fn default_crash_window_s() -> u32 {
    120
}

fn default_crash_burst() -> u32 {
    5
}

fn default_log_max_bytes() -> u64 {
    104_857_600
}

fn default_log_keep() -> u32 {
    7
}

fn default_disk_min_free() -> u64 {
    1_073_741_824
}

fn default_hot_journal_days() -> u32 {
    3
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
        assert_eq!(cfg.sol.accept_bars, 3);
        assert_eq!(cfg.sol.trap_bars, 3);
        assert_eq!(cfg.runtime.resonance_k, 1);
        assert_eq!(cfg.sol.liq_oi_1h_veto_pct, -0.02);
        assert_eq!(cfg.sol.liq_1m_notional_rule, "sample_p95");
        assert_eq!(cfg.sol.funding_hours_utc, vec![0, 8, 16]);
        assert_eq!(cfg.sol.funding_black_window_minutes, 15);
    }

    #[test]
    fn venue_gap_flags_are_independent() {
        let mut q = QualityVector::default();
        q.mark_gap(Venue::Binance);
        assert!(q.gap(Venue::Binance));
        assert!(!q.gap(Venue::Okx));
        assert!(!q.gap(Venue::Bybit));
        q.mark_gap(Venue::Bybit);
        assert!(q.gap(Venue::Bybit));
        assert!(!q.gap(Venue::Okx));
    }

    #[test]
    fn venue_parse_roundtrip() {
        assert_eq!(Venue::parse("OKX").unwrap(), Venue::Okx);
        assert_eq!(Venue::parse("binance").unwrap().as_str(), "binance");
        assert_eq!(Venue::parse("bybit").unwrap(), Venue::Bybit);
        assert!(Venue::parse("deribit").is_err());
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
        assert!((cfg.runtime.risk.risk_pct - 0.002).abs() < 1e-12);
        assert!(!cfg.runtime.risk.kill_switch_clear_on_start);
        assert!(!cfg.runtime.exec.live_send);
        assert_eq!(cfg.runtime.mode_default, Mode::Shadow);
        assert_eq!(cfg.runtime.exec.pos_mode, "net_mode");
        assert_eq!(cfg.runtime.ops.crash_burst, 5);
        assert_eq!(cfg.runtime.ops.crash_window_s, 120);
        assert!(!cfg.runtime.ops.clear_crash_on_start);
        assert_eq!(cfg.runtime.ops.log_keep, 7);
    }

    #[test]
    fn closed_bar_is_immutable_contract() {
        let bar = Bar1m {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            open_ms: 0,
            state: BarState::Closed,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            bid_vol: 1.0,
            ask_vol: 2.0,
            trade_count: 2,
            first_trade_ts_ms: 0,
            last_trade_ts_ms: 1,
        };
        assert!(bar.entries_allowed());
        assert_eq!(bar.close_ms(), BAR_INTERVAL_MS);
        assert_eq!(bar.delta(), 1.0);
        let forming = Bar1m {
            state: BarState::Forming,
            ..bar.clone()
        };
        assert!(!forming.entries_allowed());
        assert_eq!(bar_open_ms(61_000), 60_000);
        assert_eq!(bar_open_ms(59_999), 0);
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
