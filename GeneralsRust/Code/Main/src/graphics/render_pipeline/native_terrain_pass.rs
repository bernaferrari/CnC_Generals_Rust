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
        ScopedNativePass::TerrainGround => {
            let terrain_draw_started = Instant::now();
            let depth_view = frame.depth_view_arc();
            let color_view = frame.color_view_arc();
            let encoder = frame.encoder();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main terrain pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view.as_ref(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: depth_view.as_ref().map(|depth| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view: depth.as_ref(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render_pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);
            render_pass.set_scissor_rect(vp_x as u32, vp_y as u32, vp_w as u32, vp_h as u32);

            terrain_visual.record_chunk_draws(&mut render_pass);
            drop(render_pass);

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
            let Some(depth_view) = frame.depth_view_arc() else {
                return Ok(());
            };
            let color_view = frame.color_view_arc();
            let encoder = frame.encoder();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main water pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view.as_ref(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                // Depth presence was early-returned above.
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view.as_ref(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render_pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);
            render_pass.set_scissor_rect(vp_x as u32, vp_y as u32, vp_w as u32, vp_h as u32);
            terrain_visual.record_water_scene_draws(&mut render_pass);
            drop(render_pass);
            Ok(())
        }
    }
}
