//! Append-only JSONL journal of **closed** 1m bars. Never rewrite past lines.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use orderflow_domain::{Bar1m, QualityVector};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ClosedBarRecord<'a> {
    pub event: &'static str,
    pub bar: &'a Bar1m,
    pub quality_snapshot: &'a QualityVector,
}

pub struct JsonlJournal {
    path: std::path::PathBuf,
}

impl JsonlJournal {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .map_err(|e| format!("create {}: {e}", path.display()))?;
        Ok(Self { path })
    }

    pub fn open_append(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("open {}: {e}", path.display()))?;
        Ok(Self { path })
    }

    pub fn append_closed(&self, bar: &Bar1m, quality: &QualityVector) -> Result<(), String> {
        use orderflow_domain::BarState;
        if !matches!(bar.state, BarState::Closed) {
            return Err("journal only accepts Closed bars".into());
        }
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("open {}: {e}", self.path.display()))?;
        let rec = ClosedBarRecord {
            event: "bar_closed",
            bar,
            quality_snapshot: quality,
        };
        let line = serde_json::to_string(&rec).map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn append_json<T: Serialize>(&self, rec: &T) -> Result<(), String> {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("open {}: {e}", self.path.display()))?;
        let line = serde_json::to_string(rec).map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderflow_domain::{Bar1m, BarState, QualityVector, Venue};

    #[test]
    fn journal_rejects_forming() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bars.jsonl");
        let j = JsonlJournal::create(&path).unwrap();
        let forming = Bar1m {
            venue: Venue::Okx,
            symbol: "SOL".into(),
            open_ms: 0,
            state: BarState::Forming,
            open: 1.0,
            high: 1.0,
            low: 1.0,
            close: 1.0,
            bid_vol: 0.0,
            ask_vol: 0.0,
            trade_count: 0,
            first_trade_ts_ms: 0,
            last_trade_ts_ms: 0,
        };
        assert!(j
            .append_closed(&forming, &QualityVector::default())
            .is_err());
    }
}
