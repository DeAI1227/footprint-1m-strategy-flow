//! Crash circuit breaker. Persist N crashes in T seconds → stop. Do not loop-hit API.

use orderflow_domain::OpsConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrashState {
    pub timestamps_ms: Vec<i64>,
    pub tripped: bool,
    pub tripped_at_ms: Option<i64>,
}

impl Default for CrashState {
    fn default() -> Self {
        Self {
            timestamps_ms: Vec::new(),
            tripped: false,
            tripped_at_ms: None,
        }
    }
}

pub struct CrashFuse {
    path: PathBuf,
    window_s: u64,
    burst: u32,
    state: CrashState,
}

impl CrashFuse {
    pub fn load(path: impl AsRef<Path>, cfg: &OpsConfig) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let state = if path.exists() {
            let raw =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))?
        } else {
            CrashState::default()
        };
        Ok(Self {
            path,
            window_s: u64::from(cfg.crash_window_s),
            burst: cfg.crash_burst,
            state,
        })
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_string_pretty(&self.state).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(tmp, &self.path).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn tripped(&self) -> bool {
        self.state.tripped
    }

    pub fn record_crash(&mut self, now_ms: i64) -> Result<bool, String> {
        if self.state.tripped {
            return Ok(true);
        }
        let window_ms = (self.window_s as i64) * 1000;
        self.state.timestamps_ms.push(now_ms);
        self.state
            .timestamps_ms
            .retain(|t| now_ms - *t <= window_ms);
        if self.state.timestamps_ms.len() as u32 >= self.burst {
            self.state.tripped = true;
            self.state.tripped_at_ms = Some(now_ms);
        }
        self.save()?;
        Ok(self.state.tripped)
    }

    /// Manual clear only. Never auto-clear on start unless ops.clear_crash_on_start.
    pub fn clear(&mut self) -> Result<(), String> {
        self.state = CrashState::default();
        self.save()
    }

    pub fn maybe_clear_on_start(&mut self, cfg: &OpsConfig) -> Result<(), String> {
        if cfg.clear_crash_on_start {
            self.clear()?;
        }
        Ok(())
    }

    pub fn state(&self) -> &CrashState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn cfg() -> OpsConfig {
        OpsConfig {
            crash_window_s: 120,
            crash_burst: 5,
            clear_crash_on_start: false,
            ..OpsConfig::default()
        }
    }

    #[test]
    fn five_in_window_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("crash.json");
        let mut fuse = CrashFuse::load(&path, &cfg()).unwrap();
        for i in 0..4 {
            assert!(!fuse.record_crash(1_000 + i * 1_000).unwrap());
        }
        assert!(fuse.record_crash(6_000).unwrap());
        let again = CrashFuse::load(&path, &cfg()).unwrap();
        assert!(again.tripped());
    }

    #[test]
    fn start_does_not_auto_clear() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("crash.json");
        let mut fuse = CrashFuse::load(&path, &cfg()).unwrap();
        for i in 0..5 {
            fuse.record_crash(i as i64 * 1000).unwrap();
        }
        let mut again = CrashFuse::load(&path, &cfg()).unwrap();
        again.maybe_clear_on_start(&cfg()).unwrap();
        assert!(again.tripped());
    }
}
