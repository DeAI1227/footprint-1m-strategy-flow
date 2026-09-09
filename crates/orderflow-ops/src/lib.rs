//! Stage 8 — Tokyo ops. No secrets. No HTTP. Live still gated by params.

pub mod crash;
pub mod disk;
pub mod funding;
pub mod health;
pub mod logrotate;
pub mod missed;
pub mod reconnect;
pub mod replay;
pub mod spec;

pub use crash::{CrashFuse, CrashState};
pub use disk::{DiskWatermark, DiskWatermarkStatus};
pub use funding::funding_black_at;
pub use health::{HealthReport, OpsCheck};
pub use logrotate::LogRotator;
pub use missed::{MissedBarDetector, MissedBarEvent};
pub use reconnect::ReconnectPolicy;
pub use replay::run_spec_replay;
pub use spec::{SpecChange, SpecSnapshot, SpecWatch};

pub const WIRED: bool = true;

/// Conceptual Tokyo region. Not a live AWS call.
pub const REGION: &str = "ap-northeast-1";
