//! WWUtil compatibility crate
//!
//! Port of GeneralsMD/Code/Libraries/Source/WWVegas/Wwutil/
//! Provides math and miscellaneous utility functions used throughout the engine.

pub mod mathutil;
pub mod miscutil;

/// Epsilon value for misc utility comparisons (matches C++ MISCUTIL_EPSILON)
pub const MISCUTIL_EPSILON: f32 = 0.0001f32;

// Re-export for convenience
pub use mathutil::*;
pub use miscutil::*;
