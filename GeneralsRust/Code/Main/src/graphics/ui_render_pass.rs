//! UI Render Pass — bridges the GUI gadget/window system to WGPU rendering.
//!
//! PARITY_NOTE: In C++ SAGE, the GUI draw dispatch chain is:
//!   GameClient::update() → WinInstance::draw() → GadgetGameWindow::draw()
//!   → per-gadget draw callbacks (W3DGadgetPushButtonDraw, etc.)
//!   → DisplayString::draw() → WW3D Device StretchRect/DrawLine primitives
//!
//! In Rust, gadget draw callbacks queue commands into the UIRenderer (immediate-mode
//! batching). This module flushes those commands into a WGPU render pass after the 3D scene.

use log::{info, trace, warn};
use ww3d_renderer_3d::RendererResult;

use std::sync::atomic::{AtomicU32, Ordering};
static UI_FLUSH_CALL_COUNT: AtomicU32 = AtomicU32::new(0);
static UI_FLUSH_ZERO_CMD_LOGGED: AtomicU32 = AtomicU32::new(0);

pub fn flush_ui_to_frame(frame: &mut ww3d_engine::RenderFrame) -> RendererResult<()> {
    let call = UI_FLUSH_CALL_COUNT.fetch_add(1, Ordering::Relaxed);

    let renderer_arc = match game_client::gui::ui_globals::with_ui_renderer(|r| r.clone()) {
        Some(arc) => arc,
        None => {
            if call < 5 {
                warn!(
                    "flush_ui_to_frame: no UI renderer available (call #{})",
                    call
                );
            }
            return Ok(());
        }
    };

    // Begin the frame, then **drop** the write guard before gadget draw.
    // WND callbacks submit through `with_ui_renderer_mut`; they must be able
    // to `try_write()` this same renderer. Holding the guard (or setting the
    // in-draw flag) used to discard those nested ops — menus drew nothing.
    {
        let mut renderer = match renderer_arc.write() {
            Ok(r) => r,
            Err(_) => {
                warn!("UI render pass: renderer lock poisoned");
                return Err(ww3d_renderer_3d::Error::GenericError(
                    "UI renderer lock poisoned".into(),
                ));
            }
        };
        renderer.begin_frame();
    }

    let root_count = game_client::gui::window_manager::with_window_manager(|wm| {
        let roots = wm.root_window_count();
        wm.draw_all();
        roots
    });

    let mut renderer = match renderer_arc.write() {
        Ok(r) => r,
        Err(_) => {
            warn!("UI render pass: renderer lock poisoned after draw");
            return Err(ww3d_renderer_3d::Error::GenericError(
                "UI renderer lock poisoned".into(),
            ));
        }
    };
    let had_draw_commands = renderer.queued_draw_command_count();

    let should_log = call < 10 || call.is_multiple_of(300);
    if should_log {
        info!(
            "flush_ui_to_frame #{}: root_windows={}, draw_commands={}, screen={}x{}",
            call,
            root_count,
            had_draw_commands,
            renderer.screen_size().0,
            renderer.screen_size().1,
        );
    }

    if had_draw_commands == 0 {
        if UI_FLUSH_ZERO_CMD_LOGGED.fetch_add(1, Ordering::Relaxed) < 5 {
            info!(
                "flush_ui_to_frame: zero draw commands (root_windows={}) — gadget draws queued nothing",
                root_count,
            );
        }
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
            renderer.end_frame();
            return Err(ww3d_renderer_3d::Error::GenericError(err.to_string()));
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
