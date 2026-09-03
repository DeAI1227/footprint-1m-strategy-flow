//! Stage 0 stub. OKX private execution is stage 7.
//! Live opens are impossible even if someone flips toml flags.

use orderflow_domain::{live_open_allowed, AppConfig, LiveDenied, Mode};

pub const WIRED: bool = false;

pub fn submit_live_open(mode: Mode, cfg: &AppConfig) -> Result<(), LiveDenied> {
    live_open_allowed(mode, cfg)?;
    Err(LiveDenied::ExecNotWired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn submit_live_open_never_succeeds_on_repo_params() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("params");
        let cfg = AppConfig::load(&root).unwrap();
        assert_eq!(
            submit_live_open(Mode::Live, &cfg).unwrap_err(),
            LiveDenied::ParamsNotCalibrated
        );
        assert_eq!(
            submit_live_open(Mode::Shadow, &cfg).unwrap_err(),
            LiveDenied::ExecNotWired
        );
        assert!(!WIRED);
    }
}
