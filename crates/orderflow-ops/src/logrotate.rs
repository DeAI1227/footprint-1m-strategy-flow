//! Size-based log rotation. Keep N files. No secrets in the rotator itself.

use orderflow_domain::OpsConfig;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct LogRotator {
    path: PathBuf,
    max_bytes: u64,
    keep: u32,
}

impl LogRotator {
    pub fn new(path: impl AsRef<Path>, cfg: &OpsConfig) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            max_bytes: cfg.log_max_bytes,
            keep: cfg.log_keep,
        }
    }

    pub fn append_line(&self, line: &str) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.maybe_rotate()?;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{line}")?;
        Ok(())
    }

    fn maybe_rotate(&self) -> std::io::Result<()> {
        let meta = match std::fs::metadata(&self.path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        if meta.len() < self.max_bytes {
            return Ok(());
        }
        let keep = self.keep.max(1);
        let oldest = Self::rotated_path(&self.path, keep);
        if oldest.exists() {
            std::fs::remove_file(&oldest)?;
        }
        for i in (1..keep).rev() {
            let src = Self::rotated_path(&self.path, i);
            let dst = Self::rotated_path(&self.path, i + 1);
            if src.exists() {
                std::fs::rename(src, dst)?;
            }
        }
        std::fs::rename(&self.path, Self::rotated_path(&self.path, 1))?;
        Ok(())
    }

    fn rotated_path(path: &Path, n: u32) -> PathBuf {
        let mut p = path.as_os_str().to_os_string();
        p.push(format!(".{n}"));
        PathBuf::from(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rotates_when_over_max() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("app.log");
        let cfg = OpsConfig {
            log_max_bytes: 20,
            log_keep: 2,
            ..OpsConfig::default()
        };
        let rot = LogRotator::new(&path, &cfg);
        rot.append_line("aaaaaaaaaaaaaaaaaaaa").unwrap();
        rot.append_line("bbbbbbbbbbbbbbbbbbbb").unwrap();
        let mut rotated = path.as_os_str().to_os_string();
        rotated.push(".1");
        assert!(PathBuf::from(rotated).exists());
        assert!(path.exists());
    }
}
