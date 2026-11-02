//! WWUtil — GeneralsMD/Code/Libraries/Source/WWVegas/Wwutil/
//!
//! Faithful ports of `cMathUtil` and `cMiscUtil`. No process-wide mutable state.

pub mod mathutil;
pub mod miscutil;

/// Epsilon value for misc utility comparisons (matches C++ `MISCUTIL_EPSILON`).
pub const MISCUTIL_EPSILON: f32 = 0.0001;

pub use mathutil::{MathUtil, PdfRng, PI_1, PI_2};
pub use miscutil::MiscUtil;
