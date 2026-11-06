//! One wgpu render pass shared by consecutive scene draws (renderer plan R12).
//!
//! C++ `RTS3DScene::Flush` issues terrain, decals, meshes, shadows, water,
//! sorted translucents and particles into one D3D8 scene. wgpu passes load and
//! store the whole colour + depth/stencil attachment, which costs a full tile
//! round trip per pass on tile-based GPUs. Scene work therefore records into
//! one open pass on the frame's colour and depth/stencil targets; the pass ends
//! only when someone needs the encoder itself (copies, passes on other
//! targets) or the frame ends.
//!
//! Every hand-out resets the dynamic state a new pass starts with (full-target
//! viewport and scissor, stencil reference 0, blend constant 0), so a draw sees
//! the same state whether it shares the pass or would have opened its own.

use std::sync::Arc;

/// Colour and depth/stencil attachments of a frame's scene pass.
#[derive(Clone)]
pub struct ScenePassTargets {
    pub color: Arc<wgpu::TextureView>,
    pub depth: Option<Arc<wgpu::TextureView>>,
    /// The depth target has a stencil aspect.
    pub stencil: bool,
    /// Attachment size in pixels.
    pub size: (u32, u32),
}

/// Attachment clears for the frame's first scene pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneClear {
    pub color: wgpu::Color,
    pub depth: f32,
}

/// The open scene pass of one encoder, plus how many passes it opened.
#[derive(Default)]
pub struct ScenePass {
    open: Option<wgpu::RenderPass<'static>>,
    opened: u32,
}

impl std::fmt::Debug for ScenePass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScenePass")
            .field("open", &self.open.is_some())
            .field("opened", &self.opened)
            .finish()
    }
}

impl ScenePass {
    /// Borrow the open scene pass, opening one on `encoder` when none is open.
    ///
    /// `clear` applies only when it forces a new pass: an open pass is ended
    /// and a cleared one begins. The first pass of a frame (`opened == 0`)
    /// clears the stencil to 0, so stencil counts start clean whoever opens
    /// first; later passes load everything.
    pub fn get(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        targets: &ScenePassTargets,
        clear: Option<SceneClear>,
    ) -> &mut wgpu::RenderPass<'static> {
        if clear.is_some() {
            self.end();
        }
        let first = self.opened == 0;
        let pass = match self.open.take() {
            Some(pass) => pass,
            None => {
                self.opened = self.opened.saturating_add(1);
                Self::begin(encoder, targets, clear, first)
            }
        };
        let pass = self.open.insert(pass);
        let (width, height) = targets.size;
        pass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, width, height);
        pass.set_stencil_reference(0);
        pass.set_blend_constant(wgpu::Color::TRANSPARENT);
        pass
    }

    /// End the open pass, if any. The encoder is usable again afterwards.
    pub fn end(&mut self) {
        self.open = None;
    }

    /// Whether a pass is open.
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Passes opened so far.
    pub fn opened(&self) -> u32 {
        self.opened
    }

    fn begin(
        encoder: &mut wgpu::CommandEncoder,
        targets: &ScenePassTargets,
        clear: Option<SceneClear>,
        first: bool,
    ) -> wgpu::RenderPass<'static> {
        let color_load = clear.map_or(wgpu::LoadOp::Load, |clear| wgpu::LoadOp::Clear(clear.color));
        let depth_load = clear.map_or(wgpu::LoadOp::Load, |clear| wgpu::LoadOp::Clear(clear.depth));
        let stencil_load = if first {
            wgpu::LoadOp::Clear(0)
        } else {
            wgpu::LoadOp::Load
        };
        let depth_stencil_attachment =
            targets
                .depth
                .as_deref()
                .map(|view| wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(stored(depth_load)),
                    stencil_ops: targets.stencil.then(|| stored(stencil_load)),
                });
        encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: targets.color.as_ref(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: stored(color_load),
                })],
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime()
    }
}

fn stored<V>(load: wgpu::LoadOp<V>) -> wgpu::Operations<V> {
    wgpu::Operations {
        load,
        store: wgpu::StoreOp::Store,
    }
}

/// A target scene draws record into: a shared pass for draws, the encoder for
/// everything else (using the encoder ends the shared pass).
pub trait SceneRecorder {
    /// The shared scene pass (see [`ScenePass::get`]).
    fn scene_pass(&mut self) -> &mut wgpu::RenderPass<'static>;
    /// The encoder, after ending any open scene pass.
    fn encoder(&mut self) -> &mut wgpu::CommandEncoder;
}

/// [`SceneRecorder`] over a borrowed encoder, for callers outside a
/// `ww3d_engine::RenderFrame`.
pub struct EncoderScene<'a> {
    encoder: &'a mut wgpu::CommandEncoder,
    targets: ScenePassTargets,
    pass: ScenePass,
}

impl<'a> EncoderScene<'a> {
    pub fn new(encoder: &'a mut wgpu::CommandEncoder, targets: ScenePassTargets) -> Self {
        Self {
            encoder,
            targets,
            pass: ScenePass::default(),
        }
    }

    /// Passes opened so far.
    pub fn opened(&self) -> u32 {
        self.pass.opened()
    }
}

impl SceneRecorder for EncoderScene<'_> {
    fn scene_pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        self.pass.get(self.encoder, &self.targets, None)
    }

    fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.pass.end();
        self.encoder
    }
}
