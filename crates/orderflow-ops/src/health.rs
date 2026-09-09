//! Ops health snapshot. No secrets.

use crate::crash::CrashFuse;
use crate::disk::DiskWatermarkStatus;
use crate::REGION;
use orderflow_domain::{AppConfig, Mode};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub region: String,
    pub mode: String,
    pub calibration_complete: bool,
    pub live_send: bool,
    pub crash_tripped: bool,
    pub disk: String,
    pub funding_black_now: bool,
    pub live_allowed: bool,
    pub copied_price_onto_okx: bool,
}

pub struct OpsCheck;

impl OpsCheck {
    pub fn report(
        cfg: &AppConfig,
        mode: Mode,
        fuse: Option<&CrashFuse>,
        disk: DiskWatermarkStatus,
        now_ms: i64,
    ) -> HealthReport {
        HealthReport {
            region: REGION.to_string(),
            mode: mode.as_str().to_string(),
            calibration_complete: cfg.runtime.calibration.calibration_complete,
            live_send: cfg.runtime.exec.live_send,
            crash_tripped: fuse.map(|f| f.tripped()).unwrap_or(false),
            disk: match disk {
                DiskWatermarkStatus::Ok => "ok".into(),
                DiskWatermarkStatus::Below => "below".into(),
                DiskWatermarkStatus::Unknown => "unknown".into(),
            },
            funding_black_now: crate::funding::funding_black_for_symbol(now_ms, &cfg.sol),
            live_allowed: orderflow_domain::live_allowed(),
            copied_price_onto_okx: false,
        }
    }
}
