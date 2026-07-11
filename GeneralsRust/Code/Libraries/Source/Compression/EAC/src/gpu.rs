//! GPU-accelerated compression stub (feature-gated).
//! Present so `cargo fmt` can resolve `pub mod gpu` under `gpu_acceleration`.

#![cfg(feature = "gpu_acceleration")]

/// Placeholder GPU compressor — falls back to CPU paths when the feature is enabled
/// but a full GPU path is not implemented for this codec.
pub struct GpuCompressor;

impl GpuCompressor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GpuCompressor {
    fn default() -> Self {
        Self::new()
    }
}
