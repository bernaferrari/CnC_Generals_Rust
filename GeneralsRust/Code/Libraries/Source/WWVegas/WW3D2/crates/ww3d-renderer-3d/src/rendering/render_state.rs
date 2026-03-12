//! Render state caching system
//!
//! Minimizes GPU state changes by caching current state and only applying
//! changes when necessary. This is critical for performance as GPU state
//! changes are expensive operations.

use crate::rendering::shader_system::shader::{
    AlphaTestType, CullModeType, DepthCompareType, DepthMaskType, DstBlendFuncType, FogFuncType,
    ShaderClass, SrcBlendFuncType,
};

/// Handle to a shader (for state tracking)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderHandle(pub u32);

/// Handle to a texture (for state tracking)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u64);

/// Blend mode state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendMode {
    pub src_blend: SrcBlendFuncType,
    pub dst_blend: DstBlendFuncType,
    pub enabled: bool,
}

impl BlendMode {
    /// Create opaque blend mode (no blending)
    pub fn opaque() -> Self {
        Self {
            src_blend: SrcBlendFuncType::One,
            dst_blend: DstBlendFuncType::Zero,
            enabled: false,
        }
    }

    /// Create alpha blend mode
    pub fn alpha() -> Self {
        Self {
            src_blend: SrcBlendFuncType::SrcAlpha,
            dst_blend: DstBlendFuncType::InvSrcAlpha,
            enabled: true,
        }
    }

    /// Create additive blend mode
    pub fn additive() -> Self {
        Self {
            src_blend: SrcBlendFuncType::One,
            dst_blend: DstBlendFuncType::One,
            enabled: true,
        }
    }

    /// Create from shader settings
    pub fn from_shader(shader: &ShaderClass) -> Self {
        let src_blend = shader.get_src_blend_func();
        let dst_blend = shader.get_dst_blend_func();
        let enabled = !(matches!(src_blend, SrcBlendFuncType::One)
            && matches!(dst_blend, DstBlendFuncType::Zero));

        Self {
            src_blend,
            dst_blend,
            enabled,
        }
    }
}

/// Depth state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepthMode {
    pub compare: DepthCompareType,
    pub write_enabled: bool,
}

impl DepthMode {
    /// Create default depth mode (test and write enabled)
    pub fn default() -> Self {
        Self {
            compare: DepthCompareType::Lequal,
            write_enabled: true,
        }
    }

    /// Create from shader settings
    pub fn from_shader(shader: &ShaderClass) -> Self {
        Self {
            compare: shader.get_depth_compare(),
            write_enabled: shader.get_depth_mask() == DepthMaskType::Enable,
        }
    }
}

/// Culling state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CullMode {
    pub enabled: bool,
    pub cull_backface: bool,
}

impl CullMode {
    /// Create default cull mode (backface culling enabled)
    pub fn default() -> Self {
        Self {
            enabled: true,
            cull_backface: true,
        }
    }

    /// Create from shader settings
    pub fn from_shader(shader: &ShaderClass) -> Self {
        Self {
            enabled: shader.get_cull_mode() == CullModeType::Enable,
            cull_backface: true, // WW3D always culls backfaces when enabled
        }
    }
}

/// Alpha test state
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaTest {
    pub enabled: bool,
    pub reference: f32,
}

impl AlphaTest {
    /// Create disabled alpha test
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            reference: 0.0,
        }
    }

    /// Create from shader settings
    pub fn from_shader(shader: &ShaderClass) -> Self {
        Self {
            enabled: shader.get_alpha_test() == AlphaTestType::Enable,
            reference: 0.5, // Standard reference value
        }
    }
}

/// Fog state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FogMode {
    pub enabled: bool,
}

impl FogMode {
    /// Create from shader settings
    pub fn from_shader(shader: &ShaderClass) -> Self {
        Self {
            enabled: shader.get_fog_func() == FogFuncType::Enable,
        }
    }
}

/// Complete render state
///
/// This struct caches all GPU state that affects rendering. By tracking
/// the current state, we can avoid redundant state changes which are
/// expensive on the GPU.
#[derive(Debug)]
pub struct RenderState {
    // Shader state
    pub current_shader: Option<ShaderHandle>,

    // Texture state (8 stages to match DX8/WGPU limits)
    pub current_textures: [Option<TextureHandle>; 8],

    // Blend state
    pub current_blend_mode: Option<BlendMode>,

    // Depth state
    pub current_depth_mode: Option<DepthMode>,

    // Cull state
    pub current_cull_mode: Option<CullMode>,

    // Alpha test state
    pub current_alpha_test: Option<AlphaTest>,

    // Fog state
    pub current_fog_mode: Option<FogMode>,

    // Statistics
    pub shader_changes: u64,
    pub texture_changes: u64,
    pub blend_changes: u64,
    pub depth_changes: u64,
    pub cull_changes: u64,
    pub total_state_changes: u64,
}

impl RenderState {
    /// Create a new render state with no cached state
    pub fn new() -> Self {
        Self {
            current_shader: None,
            current_textures: [None; 8],
            current_blend_mode: None,
            current_depth_mode: None,
            current_cull_mode: None,
            current_alpha_test: None,
            current_fog_mode: None,
            shader_changes: 0,
            texture_changes: 0,
            blend_changes: 0,
            depth_changes: 0,
            cull_changes: 0,
            total_state_changes: 0,
        }
    }

    /// Set shader state (only applies if different from current)
    pub fn set_shader(&mut self, shader: ShaderHandle) -> bool {
        if self.current_shader != Some(shader) {
            self.current_shader = Some(shader);
            self.shader_changes += 1;
            self.total_state_changes += 1;
            true // State changed
        } else {
            false // State unchanged
        }
    }

    /// Set texture for a specific stage (only applies if different)
    pub fn set_texture(&mut self, stage: usize, texture: TextureHandle) -> bool {
        if stage >= self.current_textures.len() {
            return false;
        }

        if self.current_textures[stage] != Some(texture) {
            self.current_textures[stage] = Some(texture);
            self.texture_changes += 1;
            self.total_state_changes += 1;
            true
        } else {
            false
        }
    }

    /// Clear texture for a specific stage
    pub fn clear_texture(&mut self, stage: usize) -> bool {
        if stage >= self.current_textures.len() {
            return false;
        }

        if self.current_textures[stage].is_some() {
            self.current_textures[stage] = None;
            self.texture_changes += 1;
            self.total_state_changes += 1;
            true
        } else {
            false
        }
    }

    /// Set blend mode (only applies if different)
    pub fn set_blend_mode(&mut self, blend_mode: BlendMode) -> bool {
        if self.current_blend_mode != Some(blend_mode) {
            self.current_blend_mode = Some(blend_mode);
            self.blend_changes += 1;
            self.total_state_changes += 1;
            true
        } else {
            false
        }
    }

    /// Set depth mode (only applies if different)
    pub fn set_depth_mode(&mut self, depth_mode: DepthMode) -> bool {
        if self.current_depth_mode != Some(depth_mode) {
            self.current_depth_mode = Some(depth_mode);
            self.depth_changes += 1;
            self.total_state_changes += 1;
            true
        } else {
            false
        }
    }

    /// Set cull mode (only applies if different)
    pub fn set_cull_mode(&mut self, cull_mode: CullMode) -> bool {
        if self.current_cull_mode != Some(cull_mode) {
            self.current_cull_mode = Some(cull_mode);
            self.cull_changes += 1;
            self.total_state_changes += 1;
            true
        } else {
            false
        }
    }

    /// Set alpha test (only applies if different)
    pub fn set_alpha_test(&mut self, alpha_test: AlphaTest) -> bool {
        // Use approximate equality for float comparison
        let changed = match self.current_alpha_test {
            Some(current) => {
                current.enabled != alpha_test.enabled
                    || (current.reference - alpha_test.reference).abs() > 0.001
            }
            None => true,
        };

        if changed {
            self.current_alpha_test = Some(alpha_test);
            self.total_state_changes += 1;
            true
        } else {
            false
        }
    }

    /// Set fog mode (only applies if different)
    pub fn set_fog_mode(&mut self, fog_mode: FogMode) -> bool {
        if self.current_fog_mode != Some(fog_mode) {
            self.current_fog_mode = Some(fog_mode);
            self.total_state_changes += 1;
            true
        } else {
            false
        }
    }

    /// Apply complete shader state (convenience method)
    ///
    /// This examines the shader and applies all relevant state changes.
    /// Returns the number of state changes made.
    pub fn apply_shader_state(&mut self, shader: &ShaderClass) -> usize {
        let mut changes = 0;

        let shader_handle = ShaderHandle(shader.get_bits());
        if self.set_shader(shader_handle) {
            changes += 1;
        }

        let blend_mode = BlendMode::from_shader(shader);
        if self.set_blend_mode(blend_mode) {
            changes += 1;
        }

        let depth_mode = DepthMode::from_shader(shader);
        if self.set_depth_mode(depth_mode) {
            changes += 1;
        }

        let cull_mode = CullMode::from_shader(shader);
        if self.set_cull_mode(cull_mode) {
            changes += 1;
        }

        let alpha_test = AlphaTest::from_shader(shader);
        if self.set_alpha_test(alpha_test) {
            changes += 1;
        }

        let fog_mode = FogMode::from_shader(shader);
        if self.set_fog_mode(fog_mode) {
            changes += 1;
        }

        changes
    }

    /// Invalidate all state (forces reapplication next time)
    pub fn invalidate(&mut self) {
        self.current_shader = None;
        self.current_textures = [None; 8];
        self.current_blend_mode = None;
        self.current_depth_mode = None;
        self.current_cull_mode = None;
        self.current_alpha_test = None;
        self.current_fog_mode = None;
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.shader_changes = 0;
        self.texture_changes = 0;
        self.blend_changes = 0;
        self.depth_changes = 0;
        self.cull_changes = 0;
        self.total_state_changes = 0;
    }

    /// Get statistics string
    pub fn stats_string(&self) -> String {
        format!(
            "RenderState Stats: {} total changes ({} shader, {} texture, {} blend, {} depth, {} cull)",
            self.total_state_changes,
            self.shader_changes,
            self.texture_changes,
            self.blend_changes,
            self.depth_changes,
            self.cull_changes
        )
    }
}

impl Default for RenderState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render state manager with batching support
///
/// This manager groups objects by state to minimize state changes.
/// It sorts objects by their render state hash before rendering.
pub struct RenderStateBatcher {
    state: RenderState,
    batches: Vec<StateBatch>,
}

/// A batch of objects with the same render state
struct StateBatch {
    shader_bits: u32,
    texture_handles: Vec<TextureHandle>,
    object_count: usize,
}

impl RenderStateBatcher {
    /// Create a new render state batcher
    pub fn new() -> Self {
        Self {
            state: RenderState::new(),
            batches: Vec::new(),
        }
    }

    /// Get the current render state
    pub fn state(&self) -> &RenderState {
        &self.state
    }

    /// Get mutable render state
    pub fn state_mut(&mut self) -> &mut RenderState {
        &mut self.state
    }

    /// Begin a new frame (clears batches, keeps state cache)
    pub fn begin_frame(&mut self) {
        self.batches.clear();
        self.state.reset_stats();
    }

    /// Sort and batch objects by render state
    ///
    /// In a full implementation, this would:
    /// 1. Collect all objects to render
    /// 2. Sort by render state hash
    /// 3. Group into batches with same state
    /// 4. Render batches in order
    pub fn batch_and_render(&mut self) {
        // Sort batches by shader bits to minimize state changes
        self.batches.sort_by_key(|b| b.shader_bits);

        // Render each batch
        for batch in &self.batches {
            // Apply state for this batch
            // (In actual implementation, would render objects here)
            println!("Rendering batch with {} objects", batch.object_count);
        }
    }

    /// Get state change statistics
    pub fn stats(&self) -> String {
        self.state.stats_string()
    }
}

impl Default for RenderStateBatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_state_creation() {
        let state = RenderState::new();
        assert!(state.current_shader.is_none());
        assert_eq!(state.total_state_changes, 0);
    }

    #[test]
    fn test_shader_state_caching() {
        let mut state = RenderState::new();
        let shader = ShaderHandle(12345);

        // First set should change state
        assert!(state.set_shader(shader));
        assert_eq!(state.shader_changes, 1);

        // Second set with same shader should not change
        assert!(!state.set_shader(shader));
        assert_eq!(state.shader_changes, 1);

        // Different shader should change
        let shader2 = ShaderHandle(54321);
        assert!(state.set_shader(shader2));
        assert_eq!(state.shader_changes, 2);
    }

    #[test]
    fn test_texture_state_caching() {
        let mut state = RenderState::new();
        let texture = TextureHandle(999);

        // First set should change state
        assert!(state.set_texture(0, texture));
        assert_eq!(state.texture_changes, 1);

        // Second set with same texture should not change
        assert!(!state.set_texture(0, texture));
        assert_eq!(state.texture_changes, 1);

        // Different texture should change
        let texture2 = TextureHandle(888);
        assert!(state.set_texture(0, texture2));
        assert_eq!(state.texture_changes, 2);
    }

    #[test]
    fn test_blend_mode_equality() {
        let opaque1 = BlendMode::opaque();
        let opaque2 = BlendMode::opaque();
        assert_eq!(opaque1, opaque2);

        let alpha = BlendMode::alpha();
        assert_ne!(opaque1, alpha);
    }

    #[test]
    fn test_state_invalidation() {
        let mut state = RenderState::new();
        state.set_shader(ShaderHandle(123));
        state.set_texture(0, TextureHandle(456));

        assert!(state.current_shader.is_some());
        assert!(state.current_textures[0].is_some());

        state.invalidate();

        assert!(state.current_shader.is_none());
        assert!(state.current_textures[0].is_none());
    }
}
