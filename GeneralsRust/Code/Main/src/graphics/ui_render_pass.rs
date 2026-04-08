//! UI Render Pass — bridges the GUI gadget/window system to WGPU rendering.
//!
//! PARITY_NOTE: In C++ SAGE, the GUI draw dispatch chain is:
//!   GameClient::update() → WinInstance::draw() → GadgetGameWindow::draw()
//!   → per-gadget draw callbacks (W3DGadgetPushButtonDraw, etc.)
//!   → DisplayString::draw() → WW3D Device StretchRect/DrawLine primitives
//!
//! In Rust, gadget draw callbacks queue commands into the UIRenderer (immediate-mode
//! batching). This module flushes those commands into a WGPU render pass after the 3D scene.

use log::{trace, warn};
use ww3d_renderer_3d::RendererResult;

/// Flush all queued UI draw commands into the given WGPU render frame.
///
/// PARITY_NOTE: Equivalent to C++ SAGE post-scene 2D overlay pass where
/// W3DDevice::StretchRect, DrawLine, and font rasterization are issued after
/// the 3D scene render. Orthographic projection uses screen-space coordinates
/// (0,0 at top-left, Y increasing downward).
pub fn flush_ui_to_frame(frame: &mut ww3d_engine::RenderFrame) -> RendererResult<()> {
    let renderer_arc = match game_client::gui::ui_globals::with_ui_renderer(|r| r.clone()) {
        Some(arc) => arc,
        None => return Ok(()),
    };

    let mut renderer = match renderer_arc.write() {
        Ok(r) => r,
        Err(_) => {
            warn!("UI render pass skipped: renderer lock poisoned");
            return Ok(());
        }
    };

    renderer.begin_frame();

    // PARITY_NOTE: Matches C++ WinRepaint() z-order (BELOW → normal → ABOVE → modal).
    let had_draw_commands = game_client::gui::window_manager::with_window_manager(|wm| {
        wm.draw_all();
        renderer.queued_draw_command_count()
    });

    if had_draw_commands == 0 {
        renderer.end_frame();
        return Ok(());
    }

    let color_view = frame.color_view_arc();
    {
        let encoder = frame.encoder();
        let mut ui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("UI overlay pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view.as_ref(),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if let Err(err) = renderer.render(&mut ui_pass) {
            warn!("UI render pass failed: {err}");
        }
    }

    renderer.end_frame();

    trace!(
        "UI render pass flushed {} commands ({}x{})",
        had_draw_commands,
        renderer.screen_size().0,
        renderer.screen_size().1,
    );

    Ok(())
}
