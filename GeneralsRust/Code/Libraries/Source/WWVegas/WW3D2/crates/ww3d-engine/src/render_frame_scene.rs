//! The frame's shared scene pass (renderer plan R12); see
//! [`ww3d_gpu::scene_pass`].

use crate::RenderFrame;
use ww3d_gpu::scene_pass::{SceneClear, ScenePassTargets, SceneRecorder};

impl RenderFrame {
    fn scene_targets(&self) -> ScenePassTargets {
        ScenePassTargets {
            color: self.color_view_arc(),
            depth: self.depth_view_arc(),
            stencil: self.depth_has_stencil,
            size: self.size(),
        }
    }

    /// The shared scene pass on this frame's colour and depth/stencil targets,
    /// opened (loading the attachments) when none is open. Dynamic state is
    /// reset to a new pass's defaults on every call.
    pub fn scene_pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        let targets = self.scene_targets();
        self.scene_pass.get(&mut self.encoder, &targets, None)
    }

    /// Begin a new shared scene pass that clears colour and depth (the
    /// frame's first scene work). Ends an open scene pass first.
    pub fn clear_scene_pass(&mut self, clear: SceneClear) -> &mut wgpu::RenderPass<'static> {
        let targets = self.scene_targets();
        self.scene_pass.get(&mut self.encoder, &targets, Some(clear))
    }

    /// End the shared scene pass, if open.
    pub fn end_scene_pass(&mut self) {
        self.scene_pass.end();
    }
}

impl SceneRecorder for RenderFrame {
    fn scene_pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        RenderFrame::scene_pass(self)
    }

    fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        RenderFrame::encoder(self)
    }
}
