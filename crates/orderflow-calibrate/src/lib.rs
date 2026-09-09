//! Stage 9 — fill-number interfaces. Do not complete calibration here.
//! Replay + shadow stats only. Never hand-write textbook 400% into source.

pub mod fill;
pub mod promote;
pub mod stats;

pub use fill::{fill_report, FillReport, FillStep, SlotStatus};
pub use promote::promote_fill;
pub use stats::{summarize_journal, ShadowStats};

pub const WIRED: bool = true;
/// This plan period does not finish filling numbers.
pub const COMPLETE: bool = false;

/// SOL bucket → record vs armed (still parallel) → liquidity/session → SUI → OOS.
pub const FILL_ORDER: &[FillStep] = &[
    FillStep::SolBucket,
    FillStep::RecordVsArmed,
    FillStep::LiquiditySession,
    FillStep::SuiTable,
    FillStep::OutOfSample,
];
