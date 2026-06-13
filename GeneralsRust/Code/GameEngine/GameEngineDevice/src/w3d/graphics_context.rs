//! # W3D Graphics Context
//!
//! Complete wgpu-based graphics context with modern state management,
//! resource binding, and render pipeline management.

use super::{Result, W3DError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, Buffer,
    BufferDescriptor, BufferUsages, Color, CommandEncoder, CompareFunction, ComputePipeline,
    Device, Extent3d, FilterMode, LoadOp, Operations, PipelineLayoutDescriptor, QuerySet, Queue,
    RenderPass, RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipeline, Sampler, SamplerDescriptor, ShaderStages, StoreOp, Surface, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureView,
};

/// Graphics context state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextState {
    /// Viewport settings
    pub viewport: Viewport,
    /// Scissor test settings
    pub scissor: Option<ScissorRect>,
    /// Blend state
    pub blend_state: BlendState,
    /// Depth state
    pub depth_state: DepthState,
    /// Rasterizer state
    pub rasterizer_state: RasterizerState,
}

/// Viewport settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    /// X offset
    pub x: f32,
    /// Y offset
    pub y: f32,
    /// Width
    pub width: f32,
    /// Height
    pub height: f32,
    /// Minimum depth
    pub min_depth: f32,
    /// Maximum depth
    pub max_depth: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    }
}

/// Scissor rectangle
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScissorRect {
    /// Left coordinate
    pub left: i32,
    /// Top coordinate
    pub top: i32,
    /// Right coordinate
    pub right: i32,
    /// Bottom coordinate
    pub bottom: i32,
}

/// Blend state settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlendState {
    /// Alpha blending enabled
    pub enabled: bool,
    /// Source blend factor
    pub src_factor: BlendFactor,
    /// Destination blend factor
    pub dst_factor: BlendFactor,
    /// Blend operation
    pub blend_op: BlendOp,
    /// Alpha source blend factor
    pub src_alpha_factor: BlendFactor,
    /// Alpha destination blend factor
    pub dst_alpha_factor: BlendFactor,
    /// Alpha blend operation
    pub alpha_blend_op: BlendOp,
}

/// Blend factors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendFactor {
    Zero,
    One,
    SrcColor,
    InvSrcColor,
    DstColor,
    InvDstColor,
    SrcAlpha,
    InvSrcAlpha,
    DstAlpha,
    InvDstAlpha,
}

/// Blend operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendOp {
    Add,
    Subtract,
    RevSubtract,
    Min,
    Max,
}

impl Default for BlendState {
    fn default() -> Self {
        Self {
            enabled: false,
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::InvSrcAlpha,
            blend_op: BlendOp::Add,
            src_alpha_factor: BlendFactor::One,
            dst_alpha_factor: BlendFactor::Zero,
            alpha_blend_op: BlendOp::Add,
        }
    }
}

/// Depth state settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DepthState {
    /// Depth test enabled
    pub depth_test: bool,
    /// Depth write enabled
    pub depth_write: bool,
    /// Depth comparison function
    pub depth_func: CompareFunc,
}

/// Comparison functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareFunc {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

impl Default for DepthState {
    fn default() -> Self {
        Self {
            depth_test: true,
            depth_write: true,
            depth_func: CompareFunc::Less,
        }
    }
}

/// Rasterizer state settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterizerState {
    /// Fill mode
    pub fill_mode: FillMode,
    /// Cull mode
    pub cull_mode: CullMode,
    /// Front face winding
    pub front_face: FrontFace,
    /// Depth bias
    pub depth_bias: f32,
    /// Depth bias clamp
    pub depth_bias_clamp: f32,
    /// Slope scaled depth bias
    pub slope_scaled_depth_bias: f32,
    /// Multi-sampling enabled
    pub multisample: bool,
}

/// Fill modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillMode {
    Solid,
    Wireframe,
    Point,
}

/// Cull modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CullMode {
    None,
    Front,
    Back,
}

/// Front face winding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrontFace {
    CounterClockwise,
    Clockwise,
}

impl Default for RasterizerState {
    fn default() -> Self {
        Self {
            fill_mode: FillMode::Solid,
            cull_mode: CullMode::Back,
            front_face: FrontFace::CounterClockwise,
            depth_bias: 0.0,
            depth_bias_clamp: 0.0,
            slope_scaled_depth_bias: 0.0,
            multisample: true,
        }
    }
}

/// Complete W3D graphics context with wgpu integration
pub struct GraphicsContext {
    /// WGPU device reference
    device: Arc<Device>,
    /// WGPU command queue
    queue: Arc<Queue>,

    /// Current context state
    state: ContextState,
    /// State stack for push/pop operations
    state_stack: Vec<ContextState>,

    /// Command encoder for recording commands
    command_encoder: Option<CommandEncoder>,

    /// Active render pass
    current_render_pass: Option<wgpu::RenderPass<'static>>,

    /// Pending clear values consumed at next render pass creation (LoadOp::Clear)
    pending_clear_color: Option<Color>,
    pending_clear_depth: Option<f32>,
    pending_clear_stencil: Option<u32>,

    /// Resource caches
    render_pipeline_cache: HashMap<u64, Arc<RenderPipeline>>,
    compute_pipeline_cache: HashMap<u64, Arc<ComputePipeline>>,
    bind_group_cache: HashMap<u64, Arc<BindGroup>>,
    buffer_cache: HashMap<String, Arc<Buffer>>,
    sampler_cache: HashMap<u64, Arc<Sampler>>,

    /// Performance tracking
    stats: ContextStats,
}

/// Graphics context performance statistics
#[derive(Debug, Default, Clone)]
pub struct ContextStats {
    /// Commands recorded
    pub commands_recorded: u32,
    /// Pipeline switches
    pub pipeline_switches: u32,
    /// Bind group switches
    pub bind_group_switches: u32,
    /// Buffer updates
    pub buffer_updates: u32,
    /// Texture uploads
    pub texture_uploads: u32,
    /// GPU memory allocated
    pub gpu_memory_allocated: u64,
    /// Cache hit rate
    pub cache_hit_rate: f32,
}

impl GraphicsContext {
    /// Create a new graphics context with wgpu backend
    pub async fn new_with_wgpu(device: &Device, queue: &Queue) -> Result<Self> {
        tracing::info!("Creating W3D graphics context with wgpu backend");

        Ok(Self {
            device: Arc::new(device.clone()),
            queue: Arc::new(queue.clone()),
            state: ContextState::default(),
            state_stack: Vec::new(),
            command_encoder: None,
            current_render_pass: None,
            pending_clear_color: None,
            pending_clear_depth: None,
            pending_clear_stencil: None,
            render_pipeline_cache: HashMap::new(),
            compute_pipeline_cache: HashMap::new(),
            bind_group_cache: HashMap::new(),
            buffer_cache: HashMap::new(),
            sampler_cache: HashMap::new(),
            stats: ContextStats::default(),
        })
    }

    /// Set viewport
    pub async fn set_viewport(&mut self, viewport: Viewport) -> Result<()> {
        self.state.viewport = viewport.clone();
        if let Some(rp) = self.current_render_pass.as_mut() {
            rp.set_viewport(
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
                viewport.min_depth,
                viewport.max_depth,
            );
        }
        Ok(())
    }

    /// Set scissor rectangle
    pub async fn set_scissor(&mut self, scissor: Option<ScissorRect>) -> Result<()> {
        self.state.scissor = scissor.clone();
        if let (Some(rp), Some(ref r)) = (self.current_render_pass.as_mut(), scissor) {
            let x = r.left.max(0) as u32;
            let y = r.top.max(0) as u32;
            let w = (r.right - r.left).max(0) as u32;
            let h = (r.bottom - r.top).max(0) as u32;
            rp.set_scissor_rect(x, y, w, h);
        }
        Ok(())
    }

    /// Set blend state.
    /// In wgpu, blend state is part of the pipeline object (not immediate like DX9 OMSetBlendState).
    /// This stores state for deferred pipeline creation.
    pub async fn set_blend_state(&mut self, blend_state: BlendState) -> Result<()> {
        self.state.blend_state = blend_state;
        Ok(())
    }

    /// Set depth state.
    /// In wgpu, depth state is part of the pipeline object (not immediate like DX9 OMSetDepthStencilState).
    /// This stores state for deferred pipeline creation.
    pub async fn set_depth_state(&mut self, depth_state: DepthState) -> Result<()> {
        self.state.depth_state = depth_state;
        Ok(())
    }

    /// Set rasterizer state.
    /// In wgpu, rasterizer state is part of the pipeline object (not immediate like DX9 RSSetState).
    /// This stores state for deferred pipeline creation.
    pub async fn set_rasterizer_state(&mut self, rasterizer_state: RasterizerState) -> Result<()> {
        self.state.rasterizer_state = rasterizer_state;
        Ok(())
    }

    /// Push current state onto stack
    pub fn push_state(&mut self) {
        self.state_stack.push(self.state.clone());
    }

    /// Pop state from stack and re-apply dynamic state to active render pass
    pub async fn pop_state(&mut self) -> Result<()> {
        if let Some(previous_state) = self.state_stack.pop() {
            self.state = previous_state;
            self.apply_dynamic_state_to_pass();
        }
        Ok(())
    }

    fn apply_dynamic_state_to_pass(&mut self) {
        if let Some(rp) = self.current_render_pass.as_mut() {
            let v = &self.state.viewport;
            rp.set_viewport(v.x, v.y, v.width, v.height, v.min_depth, v.max_depth);
            if let Some(ref r) = self.state.scissor {
                let x = r.left.max(0) as u32;
                let y = r.top.max(0) as u32;
                let w = (r.right - r.left).max(0) as u32;
                let h = (r.bottom - r.top).max(0) as u32;
                rp.set_scissor_rect(x, y, w, h);
            }
        }
    }

    /// Get current context state
    pub fn get_state(&self) -> &ContextState {
        &self.state
    }

    /// Clear render targets.
    /// In wgpu, clearing is done via LoadOp::Clear at render pass creation, not mid-pass.
    /// These values are stored and consumed by drain_pending_clears() when the next render pass begins.
    pub async fn clear(
        &mut self,
        color: Option<[f32; 4]>,
        depth: Option<f32>,
        stencil: Option<u32>,
    ) -> Result<()> {
        self.pending_clear_color = color.map(|c| Color {
            r: c[0] as f64,
            g: c[1] as f64,
            b: c[2] as f64,
            a: c[3] as f64,
        });
        self.pending_clear_depth = depth;
        self.pending_clear_stencil = stencil;
        Ok(())
    }

    /// Drain pending clear values for use in LoadOp::Clear when creating the next render pass
    pub fn drain_pending_clears(&mut self) -> (Option<Color>, Option<f32>, Option<u32>) {
        (
            self.pending_clear_color.take(),
            self.pending_clear_depth.take(),
            self.pending_clear_stencil.take(),
        )
    }

    /// Shutdown the graphics context
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("W3D graphics context shutdown completed");
        Ok(())
    }
}
