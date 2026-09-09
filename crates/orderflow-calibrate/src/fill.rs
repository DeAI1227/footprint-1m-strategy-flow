//! Fill-number slots. Observation freeze is not calibration complete.

use orderflow_domain::{AppConfig, ArmedRatePolicy, CalibrationStatus};
use serde::Serialize;

use crate::FILL_ORDER;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FillStep {
    SolBucket,
    RecordVsArmed,
    LiquiditySession,
    SuiTable,
    OutOfSample,
}

impl FillStep {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SolBucket => "sol_bucket",
            Self::RecordVsArmed => "record_vs_armed",
            Self::LiquiditySession => "liquidity_session",
            Self::SuiTable => "sui_table",
            Self::OutOfSample => "out_of_sample",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotStatus {
    /// Locked school definition (not a number to fill).
    LockedDefinition,
    /// 21-day observation hypothesis. Not OOS. Not live.
    Observation,
    /// Still waiting on replay / shadow stats. Do not invent a number.
    StillOpen,
    /// Feature present but not evaluated (e.g. script F without L2).
    NotEvaluated,
}

#[derive(Debug, Clone, Serialize)]
pub struct FillSlot {
    pub key: &'static str,
    pub symbol: &'static str,
    pub status: SlotStatus,
    pub note: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct FillReport {
    pub event: &'static str,
    pub wired: bool,
    pub complete: bool,
    pub observation_frozen: bool,
    pub calibration_complete: bool,
    pub out_of_sample_validated: bool,
    pub live_allowed: bool,
    pub chosen_armed_rate: Option<&'static str>,
    pub next_step: &'static str,
    pub fill_order: Vec<&'static str>,
    pub slots: Vec<FillSlot>,
    pub errors: Vec<&'static str>,
    pub promote_allowed: bool,
    pub copied_price_onto_okx: bool,
}

pub fn fill_report(cfg: &AppConfig) -> FillReport {
    let mut errors = Vec::new();
    if (cfg.sol.bucket - cfg.sui.bucket).abs() < f64::EPSILON {
        errors.push("sui_copied_sol_bucket");
    }
    if (cfg.sol.imbalance_rate_dale - cfg.sol.imbalance_rate_valtos).abs() < f64::EPSILON
        || (cfg.sui.imbalance_rate_dale - cfg.sui.imbalance_rate_valtos).abs() < f64::EPSILON
    {
        errors.push("averaged_armed_rate");
    }
    if cfg.runtime.calibration.calibration_complete
        && cfg.runtime.calibration.status == CalibrationStatus::ObservationDraft
    {
        errors.push("complete_flag_on_observation_draft");
    }

    let parallel = cfg.sol.armed_rate_policy == ArmedRatePolicy::Parallel
        && cfg.sui.armed_rate_policy == ArmedRatePolicy::Parallel;
    let chosen = match cfg.sol.armed_rate_policy {
        ArmedRatePolicy::Parallel => None,
        ArmedRatePolicy::Dale300 => Some("dale_300"),
        ArmedRatePolicy::Valtos400 => Some("valtos_400"),
    };

    let slots = vec![
        FillSlot {
            key: "bucket",
            symbol: "SOL",
            status: SlotStatus::Observation,
            note: "0.01 hypothesis; 0.10 rejected; not live",
        },
        FillSlot {
            key: "imbalance_rate_record",
            symbol: "SOL",
            status: SlotStatus::Observation,
            note: "200% display only",
        },
        FillSlot {
            key: "imbalance_rate_dale_valtos",
            symbol: "SOL",
            status: if parallel {
                SlotStatus::StillOpen
            } else {
                SlotStatus::Observation
            },
            note: "300∥400 parallel; do not average to 350%",
        },
        FillSlot {
            key: "min_imbalance_volume_rule",
            symbol: "SOL",
            status: SlotStatus::Observation,
            note: "session nonempty-side p25 both; no frozen SOL lot",
        },
        FillSlot {
            key: "unfinished_is_entry",
            symbol: "SOL",
            status: SlotStatus::LockedDefinition,
            note: "never an entry",
        },
        FillSlot {
            key: "script_g_is_entry",
            symbol: "SOL",
            status: SlotStatus::LockedDefinition,
            note: "never an entry",
        },
        FillSlot {
            key: "script_f",
            symbol: "SOL",
            status: SlotStatus::NotEvaluated,
            note: "needs healthy L2",
        },
        FillSlot {
            key: "bucket",
            symbol: "SUI",
            status: SlotStatus::Observation,
            note: "0.0001 native tick; must not copy SOL 0.01",
        },
        FillSlot {
            key: "imbalance_rate_dale_valtos",
            symbol: "SUI",
            status: if parallel {
                SlotStatus::StillOpen
            } else {
                SlotStatus::Observation
            },
            note: "own table; still parallel",
        },
        FillSlot {
            key: "out_of_sample",
            symbol: "ALL",
            status: SlotStatus::StillOpen,
            note: "observation freeze is not OOS",
        },
    ];

    let next_step = if !parallel {
        FillStep::OutOfSample.as_str()
    } else {
        FillStep::RecordVsArmed.as_str()
    };

    FillReport {
        event: "calibrate_check",
        wired: crate::WIRED,
        complete: crate::COMPLETE,
        observation_frozen: cfg.runtime.calibration.observation_frozen,
        calibration_complete: cfg.runtime.calibration.calibration_complete,
        out_of_sample_validated: cfg.runtime.calibration.out_of_sample_validated,
        live_allowed: orderflow_domain::live_allowed(),
        chosen_armed_rate: chosen,
        next_step,
        fill_order: FILL_ORDER.iter().map(|s| s.as_str()).collect(),
        slots,
        errors,
        promote_allowed: false,
        copied_price_onto_okx: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg() -> AppConfig {
        AppConfig::load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("params"),
        )
        .unwrap()
    }

    #[test]
    fn repo_params_keep_300_and_400_open() {
        let r = fill_report(&cfg());
        assert!(r.wired);
        assert!(!r.complete);
        assert!(r.observation_frozen);
        assert!(!r.calibration_complete);
        assert!(!r.out_of_sample_validated);
        assert!(!r.live_allowed);
        assert!(!r.promote_allowed);
        assert!(r.chosen_armed_rate.is_none());
        assert_eq!(r.next_step, "record_vs_armed");
        assert!(r.errors.is_empty());
        assert_eq!(r.fill_order[0], "sol_bucket");
        assert_eq!(r.fill_order[3], "sui_table");
    }

    #[test]
    fn copying_sui_bucket_is_an_error() {
        let mut c = cfg();
        c.sui.bucket = c.sol.bucket;
        let r = fill_report(&c);
        assert!(r.errors.contains(&"sui_copied_sol_bucket"));
    }

    #[test]
    fn averaging_armed_rate_is_an_error() {
        let mut c = cfg();
        c.sol.imbalance_rate_valtos = c.sol.imbalance_rate_dale;
        let r = fill_report(&c);
        assert!(r.errors.contains(&"averaged_armed_rate"));
    }
}
