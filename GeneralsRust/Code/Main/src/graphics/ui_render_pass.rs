//! UI Render Pass — bridges the GUI gadget/window system to WGPU rendering.
//!
//! PARITY_NOTE: In C++ SAGE, the GUI draw dispatch chain is:
//!   GameClient::update() → WinInstance::draw() → GadgetGameWindow::draw()
//!   → per-gadget draw callbacks (W3DGadgetPushButtonDraw, etc.)
//!   → DisplayString::draw() → WW3D Device StretchRect/DrawLine primitives
//!
//! In Rust, gadget draw callbacks queue commands into the UIRenderer (immediate-mode
//! batching). This module flushes those commands into a WGPU render pass after the 3D scene.

use log::{error, info, trace, warn};
use ww3d_renderer_3d::RendererResult;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock, RwLockWriteGuard};

use game_client::gui::ui_renderer::UIRenderer;

static UI_FLUSH_CALL_COUNT: AtomicU32 = AtomicU32::new(0);
static UI_FLUSH_ZERO_CMD_LOGGED: AtomicU32 = AtomicU32::new(0);
static UI_FLUSH_POISON_RECOVERY_COUNT: AtomicU32 = AtomicU32::new(0);

/// Acquire the UI lock without permanently bricking presentation after a
/// caught callback panic.  The caller must reset/discard any open UI frame
/// before rendering again; the recovery is logged and counted rather than
/// silently treating a poisoned renderer as healthy.
fn write_or_recover_ui_lock<'a, T>(
    lock: &'a RwLock<T>,
    stage: &'static str,
) -> (RwLockWriteGuard<'a, T>, bool) {
    match lock.write() {
        Ok(guard) => (guard, false),
        Err(poisoned) => {
            let recovery = UI_FLUSH_POISON_RECOVERY_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
            error!(
                "UI renderer lock was poisoned {stage}; recovering and resetting the affected UI frame (recovery #{recovery})"
            );
            let guard = poisoned.into_inner();
            lock.clear_poison();
            (guard, true)
        }
    }
}

/// The lifecycle surface needed by [`UiFrameCleanup`].
///
/// Kept intentionally small so the cleanup protocol can be exercised without
/// constructing a live WGPU device in a unit test.
trait UiFrameLifecycle {
    fn is_frame_open(&self) -> bool;
    fn end_frame(&mut self);
}

impl UiFrameLifecycle for UIRenderer {
    fn is_frame_open(&self) -> bool {
        Self::is_frame_open(self)
    }

    fn end_frame(&mut self) {
        Self::end_frame(self);
    }
}

/// Ends an open UI frame if a WND callback unwinds out of the overlay pass.
///
/// The frame owner deliberately drops its `RwLockWriteGuard` before
/// `WindowManager::draw_all`, so this guard owns only an `Arc` and reacquires
/// the lock during cleanup.  That keeps callbacks alias-free while ensuring a
/// panic cannot leave commands to leak into a later frame.
struct UiFrameCleanup<T: UiFrameLifecycle> {
    renderer: Arc<RwLock<T>>,
    armed: bool,
}

impl<T: UiFrameLifecycle> UiFrameCleanup<T> {
    fn new(renderer: Arc<RwLock<T>>) -> Self {
        Self {
            renderer,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl<T: UiFrameLifecycle> Drop for UiFrameCleanup<T> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }

        let (mut renderer, _) =
            write_or_recover_ui_lock(self.renderer.as_ref(), "while unwinding an overlay frame");
        if renderer.is_frame_open() {
            warn!("UI overlay exited before completion; discarding the unfinished UI frame");
            renderer.end_frame();
        }
    }
}

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

    // Begin Main's overlay frame, then **drop** the write guard before gadget
    // draw.  Presentation-shell UI may already have queued commands before
    // this post-scene callback; begin_overlay_frame preserves those commands.
    // WND callbacks submit through `with_ui_renderer_mut`; they must be able
    // to `try_write()` this same renderer. Holding the guard (or setting the
    // in-draw flag) used to discard those nested ops — menus drew nothing.
    {
        let (mut renderer, _) = write_or_recover_ui_lock(&renderer_arc, "before WND draw");
        renderer.begin_overlay_frame();
    }
    let mut frame_cleanup = UiFrameCleanup::new(renderer_arc.clone());

    let root_count = game_client::gui::window_manager::with_window_manager(|wm| {
        let roots = wm.root_window_count();
        wm.draw_all();
        roots
    });

    let (mut renderer, recovered_after_wnd_draw) =
        write_or_recover_ui_lock(&renderer_arc, "after WND draw");
    if recovered_after_wnd_draw {
        renderer.end_frame();
        frame_cleanup.disarm();
        return Err(ww3d_renderer_3d::Error::GenericError(
            "UI renderer recovered from a poisoned WND callback lock; discarded partial UI frame"
                .into(),
        ));
    }
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
        frame_cleanup.disarm();
        return Ok(());
    }

    let render_result = {
        let color_view = frame.color_view_arc();
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
        renderer.render(&mut ui_pass)
    };
    renderer.end_frame();
    frame_cleanup.disarm();

    if let Err(err) = render_result {
        warn!("UI render pass failed: {err}");
        return Err(ww3d_renderer_3d::Error::GenericError(err.to_string()));
    }

    trace!(
        "UI render pass flushed {} commands ({}x{})",
        had_draw_commands,
        renderer.screen_size().0,
        renderer.screen_size().1,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_ui_lock_is_recovered_and_unpoisoned() {
        let lock = RwLock::new(7_u32);
        let _ = std::panic::catch_unwind(|| {
            let _guard = lock.write().expect("test lock initially available");
            panic!("intentional UI callback panic");
        });
        assert!(lock.is_poisoned());

        let (mut guard, recovered) = write_or_recover_ui_lock(&lock, "in test");
        assert!(recovered);
        *guard = 9;
        drop(guard);

        assert!(
            !lock.is_poisoned(),
            "recovery must allow later frames to acquire a normal UI lock"
        );
        assert_eq!(*lock.read().expect("unpoisoned lock"), 9);
    }

    #[derive(Default)]
    struct TestUiFrame {
        open: bool,
        end_calls: usize,
    }

    impl UiFrameLifecycle for TestUiFrame {
        fn is_frame_open(&self) -> bool {
            self.open
        }

        fn end_frame(&mut self) {
            self.open = false;
            self.end_calls += 1;
        }
    }

    #[test]
    fn unfinished_ui_frame_is_cleaned_during_panic_unwind() {
        let frame = Arc::new(RwLock::new(TestUiFrame {
            open: true,
            end_calls: 0,
        }));
        let frame_for_panic = frame.clone();

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _cleanup = UiFrameCleanup::new(frame_for_panic);
            panic!("simulated WND callback unwind");
        }));
        assert!(panic.is_err());

        let frame = frame
            .read()
            .expect("cleanup must not poison its frame lock");
        assert!(!frame.open);
        assert_eq!(frame.end_calls, 1);
    }
}
