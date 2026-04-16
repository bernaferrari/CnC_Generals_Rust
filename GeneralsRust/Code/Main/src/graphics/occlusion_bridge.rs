// Lightweight, optional bridge to the OcclusionStencilPipeline integration.
// This module provides a safe stub that can be replaced with the actual
// OcclusionStencilPipeline wiring. It is behind a feature flag to avoid
// affecting builds that do not enable occlusion integration yet.
use wgpu::{Device, TextureFormat};

/// Simple bridge holding initialization state for the occlusion pipeline.
pub struct OcclusionBridge {
    initialized: bool,
}

impl OcclusionBridge {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    pub fn init(&mut self, _device: &Device, _color_format: TextureFormat) {
        // In a full wiring, this would create pipelines, buffers, etc.
        self.initialized = true;
    }

    pub fn is_ready(&self) -> bool {
        self.initialized
    }

    // Placeholder hook for rendering the shadow pass. In the actual wiring this would
    // encode the shadow volume passes and the shadow mask pass.
    pub fn render_shadow_pass_placeholder(&self) {
        // No-op in bridge stub
    }
}
