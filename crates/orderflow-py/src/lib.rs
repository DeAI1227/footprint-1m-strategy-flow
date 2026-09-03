use pyo3::prelude::*;

use orderflow_domain::{
    boot_decision, json_log, live_allowed as domain_live_allowed, AppConfig, FrozenSnapshot, Mode,
    VERSION,
};

/// Dummy frozen 1m snapshot. Real fields land when the Rust footprint engine is wired.
#[pyclass(name = "FrozenBar1mSnapshot")]
struct FrozenBar1mSnapshot {
    inner: FrozenSnapshot,
}

#[pymethods]
impl FrozenBar1mSnapshot {
    #[new]
    fn new() -> Self {
        Self {
            inner: FrozenSnapshot::dummy(),
        }
    }

    fn wired(&self) -> bool {
        self.inner.wired
    }

    fn schema_version(&self) -> u32 {
        self.inner.schema_version
    }
}

/// Always false in stage 0. Observation freeze is not live authorization.
#[pyfunction]
fn live_allowed() -> bool {
    domain_live_allowed()
}

#[pyfunction]
fn version() -> &'static str {
    VERSION
}

/// Return a JSON object: ok, event, live_gate, reason, message.
#[pyfunction]
fn boot(config_dir: &str, mode: &str) -> PyResult<String> {
    let mode = Mode::parse(mode).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;
    let cfg = AppConfig::load(std::path::Path::new(config_dir))
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?;
    let d = boot_decision(mode, &cfg);
    let level = if d.ok { "info" } else { "error" };
    Ok(json_log(level, &d))
}

#[pymodule]
fn orderflow_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(boot, m)?)?;
    m.add_function(wrap_pyfunction!(live_allowed, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_class::<FrozenBar1mSnapshot>()?;
    m.add("school", "footprint")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn live_allowed_is_false() {
        assert!(!orderflow_domain::live_allowed());
        assert_eq!(orderflow_domain::VERSION, "0.0.0");
        assert!(!orderflow_domain::FrozenSnapshot::dummy().wired);
    }
}
