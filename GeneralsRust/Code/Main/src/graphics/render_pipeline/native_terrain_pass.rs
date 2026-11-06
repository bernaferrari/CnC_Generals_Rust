//! Recording borrows this pipeline's owner only during the actual native frame.
use super::*;
use ww3d_renderer_3d::rendering::wgpu_main_renderer::ScopedNativePass;
pub(super) fn record(
    kind: ScopedNativePass,
    terrain_visual: Option<&game_client::terrain::terrain_visual::TerrainVisualImpl>,
    frame: &mut ww3d_engine::RenderFrame,
    viewport: (f32, f32, f32, f32),
    clear_color: wgpu::Color,
) -> RendererResult<()> {
    let terrain_visual = terrain_visual.ok_or_else(|| {
        ww3d_renderer_3d::Error::InvalidOperation("native terrain pass has no receiving map".into())
    })?;
    let (vp_x, vp_y, vp_w, vp_h) = viewport;
    match kind {
        ScopedNativePass::ClientEffects => Err(ww3d_renderer_3d::Error::InvalidOperation(
            "client effects require receiving particle renderer".into(),
        )),
        ScopedNativePass::TerrainGround => {
            let terrain_draw_started = Instant::now();
            // The frame's first scene work: clears colour and depth and opens
            // the shared scene pass the later phases draw into.
            let render_pass = frame.clear_scene_pass(ww3d_gpu::scene_pass::SceneClear {
                color: clear_color,
                depth: 1.0,
            });
            render_pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);
            render_pass.set_scissor_rect(vp_x as u32, vp_y as u32, vp_w as u32, vp_h as u32);

            terrain_visual.record_chunk_draws(render_pass);

            let terrain_draw_elapsed = terrain_draw_started.elapsed();
            if terrain_draw_elapsed >= PROFILE_STEP_LOG_THRESHOLD {
                debug!(
                    "TerrainVisual chunk draw recording took {:?}",
                    terrain_draw_elapsed
                );
            }

            Ok(())
        }
        ScopedNativePass::TerrainWater => {
            if frame.depth_view().is_none() {
                return Ok(());
            }
            let render_pass = frame.scene_pass();
            render_pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);
            render_pass.set_scissor_rect(vp_x as u32, vp_y as u32, vp_w as u32, vp_h as u32);
            terrain_visual.record_water_scene_draws(render_pass);
            Ok(())
        }
    }
}
