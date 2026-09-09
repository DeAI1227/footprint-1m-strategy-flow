//! Disk watermark. Inject free bytes in tests; unknown space does not false-trip.

use orderflow_domain::OpsConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskWatermarkStatus {
    Ok,
    Below,
    Unknown,
}

pub struct DiskWatermark {
    min_free: u64,
}

impl DiskWatermark {
    pub fn new(cfg: &OpsConfig) -> Self {
        Self {
            min_free: cfg.disk_min_free_bytes,
        }
    }

    pub fn check_injected(&self, free_bytes: Option<u64>) -> DiskWatermarkStatus {
        match free_bytes {
            None => DiskWatermarkStatus::Unknown,
            Some(n) if n < self.min_free => DiskWatermarkStatus::Below,
            Some(_) => DiskWatermarkStatus::Ok,
        }
    }

    pub fn check_path(&self, path: &std::path::Path) -> DiskWatermarkStatus {
        match available_bytes(path) {
            Some(n) => self.check_injected(Some(n)),
            None => DiskWatermarkStatus::Unknown,
        }
    }
}

fn available_bytes(path: &std::path::Path) -> Option<u64> {
    let p = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    let out = std::process::Command::new("df")
        .args(["-Pk", p.to_str()?])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().nth(1)?;
    let avail = line.split_whitespace().nth(3)?;
    let kb: u64 = avail.parse().ok()?;
    Some(kb.saturating_mul(1024))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_does_not_trip() {
        let w = DiskWatermark::new(&OpsConfig::default());
        assert_eq!(w.check_injected(None), DiskWatermarkStatus::Unknown);
    }

    #[test]
    fn below_trips() {
        let w = DiskWatermark {
            min_free: 1_000_000,
        };
        assert_eq!(w.check_injected(Some(10)), DiskWatermarkStatus::Below);
        assert_eq!(w.check_injected(Some(2_000_000)), DiskWatermarkStatus::Ok);
    }
}
