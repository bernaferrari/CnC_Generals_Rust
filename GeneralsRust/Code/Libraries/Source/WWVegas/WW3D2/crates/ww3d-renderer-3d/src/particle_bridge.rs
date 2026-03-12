//! Integration helpers for wiring ww3d-particles into the active renderer.
//!
//! These helpers centralize how particle systems obtain GPU resources so that
//! emitters and buffers no longer need to manage their own device/queue handles.

use crate::core::error::RendererResult;
use crate::pointgr::PointGroupMode;
use crate::Renderer;
use std::sync::Arc;
use ww3d_particles::buffer::{ParticleBuffer, RenderMode};
use ww3d_particles::emitter::ParticleEmitter;
use ww3d_particles::line_renderer::LineGroupRenderer;
use ww3d_particles::manager::ParticleSystemManager;
use ww3d_particles::point_group::{PointGroup, PointMode as ParticlePointMode};
use ww3d_particles::streak::SegmentedLineRenderer;

fn renderer_device_queue() -> RendererResult<(Arc<wgpu::Device>, Arc<wgpu::Queue>)> {
    Renderer::with_global_mut(|renderer| {
        let gpu = renderer.gpu_device();
        Ok((gpu.device_arc(), gpu.queue_arc()))
    })
}

fn ensure_point_group(buffer: &mut ParticleBuffer, mode: PointGroupMode) -> RendererResult<()> {
    if buffer.point_group.is_none() {
        let particle_mode = match mode {
            PointGroupMode::Tris => ParticlePointMode::Triangles,
            PointGroupMode::Quads => ParticlePointMode::Quads,
            PointGroupMode::ScreenSpace => ParticlePointMode::Screenspace,
        };

        let (device, queue) = renderer_device_queue()?;
        let mut group = PointGroup::new(device, queue);
        group.point_mode = particle_mode;
        group.init_gpu_resources();
        buffer.point_group = Some(group);
    }
    Ok(())
}

fn ensure_line_renderer(buffer: &mut ParticleBuffer) -> RendererResult<()> {
    if buffer.line_renderer.is_none() {
        let (device, queue) = renderer_device_queue()?;
        buffer.line_renderer = Some(SegmentedLineRenderer::new(device, queue));
    }
    Ok(())
}

fn ensure_line_group_renderer(buffer: &mut ParticleBuffer) -> RendererResult<()> {
    if buffer.line_group_renderer.is_none() {
        let (device, queue) = renderer_device_queue()?;
        buffer.line_group_renderer = Some(LineGroupRenderer::new(device, queue));
    }
    Ok(())
}

/// Ensure that a particle buffer has all GPU-backed resources configured.
pub fn setup_particle_buffer(buffer: &mut ParticleBuffer) -> RendererResult<()> {
    match buffer.render_mode {
        RenderMode::TriParticles => ensure_point_group(buffer, PointGroupMode::Tris)?,
        RenderMode::QuadParticles => ensure_point_group(buffer, PointGroupMode::Quads)?,
        RenderMode::Line => ensure_line_renderer(buffer)?,
        RenderMode::LineGroup => ensure_line_group_renderer(buffer)?,
    }
    Ok(())
}

/// Configure an emitter so its internal buffer is ready for rendering.
pub fn setup_particle_emitter(emitter: &mut ParticleEmitter) -> RendererResult<()> {
    if let Some(buffer) = emitter.get_buffer_mut() {
        setup_particle_buffer(buffer)?;
    }
    Ok(())
}

/// Add an emitter to the system while automatically wiring GPU resources.
pub fn add_emitter_with_renderer_resources(
    manager: &mut ParticleSystemManager,
    mut emitter: ParticleEmitter,
) -> RendererResult<()> {
    setup_particle_emitter(&mut emitter)?;
    manager.add_emitter(emitter);
    Ok(())
}

/// Construct a particle system manager backed by the renderer's GPU device.
pub fn create_particle_system_manager() -> RendererResult<ParticleSystemManager> {
    let (device, queue) = renderer_device_queue()?;
    Ok(ParticleSystemManager::new(device, queue))
}
