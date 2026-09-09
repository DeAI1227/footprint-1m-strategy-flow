//! Promote observation → live. Always refused on repo params.

use orderflow_domain::{live_open_allowed, AppConfig, ArmedRatePolicy, LiveDenied, Mode};

/// Observation freeze is not a live authorization. Parallel 300∥400 cannot promote.
pub fn promote_fill(mode: Mode, cfg: &AppConfig) -> Result<(), LiveDenied> {
    let g = &cfg.runtime.calibration;
    if g.observation_frozen && !g.out_of_sample_validated {
        return Err(LiveDenied::ParamsNotCalibrated);
    }
    if cfg.sol.armed_rate_policy == ArmedRatePolicy::Parallel
        || cfg.sui.armed_rate_policy == ArmedRatePolicy::Parallel
    {
        return Err(LiveDenied::ArmedRateStillParallel);
    }
    live_open_allowed(mode, cfg)?;
    Err(LiveDenied::ExecNotWired)
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
    fn frozen_observation_cannot_promote() {
        let cfg = cfg();
        assert!(cfg.runtime.calibration.observation_frozen);
        assert_eq!(
            promote_fill(Mode::Live, &cfg).unwrap_err(),
            LiveDenied::ParamsNotCalibrated
        );
        assert_eq!(
            promote_fill(Mode::Shadow, &cfg).unwrap_err(),
            LiveDenied::ParamsNotCalibrated
        );
    }

    #[test]
    fn even_unfrozen_parallel_cannot_promote() {
        let mut cfg = cfg();
        cfg.runtime.calibration.observation_frozen = false;
        assert_eq!(
            promote_fill(Mode::Live, &cfg).unwrap_err(),
            LiveDenied::ArmedRateStillParallel
        );
    }
}
