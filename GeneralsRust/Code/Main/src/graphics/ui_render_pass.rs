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
static CONTROL_BAR_RETRY_COUNT: AtomicU32 = AtomicU32::new(0);
static CONTROL_BAR_LAST_RETRY_FLUSH: AtomicU32 = AtomicU32::new(0);

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

/// C++ `W3DInGameUI::draw` always `winRepaint()`s the live ControlBar tree.
/// `HideControlBar` at boot (and `show_control_bar` early-return when
/// `ControlBarState.visible` is already true) can leave `ControlBarParent`
/// HIDDEN after InGame enter. Unhide it on every non-shell frame.
fn unhide_control_bar_parent_while_ingame(
    wm: &mut game_client::gui::window_manager::WindowManager,
) {
    if !game_client::gui::callbacks::control_bar_callbacks::is_control_bar_visible() {
        return;
    }
    // ForwardPass only flushes this overlay for the live 3D frame.
    // Do not gate on Shell.is_shell_active — leftover MainMenu can
    // keep that flag true and leave ControlBarParent HIDDEN.
    let Some(parent) = wm.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
    else {
        return;
    };
    if parent.borrow().is_hidden() {
        let _ = parent.borrow_mut().hide(false);
    }
}

/// C++ `winRepaint` always has the ControlBar tree (`InGameUI::createControlBar`
/// + `ShowControlBar`). The Rust InGame enter load can miss; retry on the live
/// `TheWindowManager` with backoff so a later flush can emit draw commands.
fn retry_missing_control_bar_parent_while_ingame() {
    if game_client::gui::get_shell().is_shell_active() {
        return;
    }
    if crate::gameplay_layout::control_bar_parent_is_live() {
        CONTROL_BAR_RETRY_COUNT.store(0, Ordering::Relaxed);
        return;
    }
    let call = UI_FLUSH_CALL_COUNT.load(Ordering::Relaxed);
    let retries = CONTROL_BAR_RETRY_COUNT.load(Ordering::Relaxed);
    let last = CONTROL_BAR_LAST_RETRY_FLUSH.load(Ordering::Relaxed);
    // First miss retries immediately; then 30, 60, 120, 240, 480 frames.
    let interval = if retries == 0 {
        0
    } else {
        30u32.saturating_mul(1u32 << retries.min(4))
    };
    if retries > 0 && call.saturating_sub(last) < interval {
        return;
    }
    CONTROL_BAR_LAST_RETRY_FLUSH.store(call, Ordering::Relaxed);
    let loaded = crate::gameplay_layout::materialise_live_control_bar();
    if loaded {
        CONTROL_BAR_RETRY_COUNT.store(0, Ordering::Relaxed);
        info!(
            "flush_ui_to_frame: ControlBarParent missing after InGame enter; retry load succeeded (flush #{call})"
        );
    } else {
        let n = CONTROL_BAR_RETRY_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        if n <= 8 {
            error!(
                "flush_ui_to_frame: ControlBarParent missing on live WindowManager (retry #{n}, flush #{call}); searched {:?}",
                crate::gameplay_layout::CONTROL_BAR_CANDIDATES
            );
        }
    }
}

pub fn flush_ui_to_frame(frame: &mut ww3d_engine::RenderFrame) -> RendererResult<()> {
    let call = UI_FLUSH_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
    if call < 8 {
        warn!("flush_ui_to_frame #{call} entered");
    }

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
        let (sw, sh) = renderer.screen_size();
        if sw > 0 && sh > 0 {
            game_client::gui::window_manager::with_window_manager(|wm| {
                wm.set_screen_size(sw as i32, sh as i32);
            });
        }
    }
    retry_missing_control_bar_parent_while_ingame();
    // C++ W3DCommandBarBackgroundDraw is on BackgroundMarker. Rust only
    // assigns that callback from ControlBar::update; if update never ticks,
    // draw_all has SEE_THRU markers and queues nothing. Assign here.
    #[cfg(feature = "game_client")]
    {
        game_client::gui::w3d_gadget_draw::ensure_control_bar_wnd_draw_callbacks();
        // C++ ScriptActions::doLetterBoxMode HideControlBar(TRUE) / ShowControlBar(FALSE).
        // flush must not force-show the live ControlBar through a cutscene.
        if game_client::display::display_fx::letterbox_enabled() {
            let _ = game_client::gui::callbacks::control_bar_callbacks::hide_control_bar(true);
        } else if game_client::gui::callbacks::control_bar_callbacks::is_control_bar_visible() {
            let _ = game_client::gui::callbacks::control_bar_callbacks::show_control_bar(true);
        }
    }

    let mut frame_cleanup = UiFrameCleanup::new(renderer_arc.clone());

    let root_count = game_client::gui::window_manager::with_window_manager(|wm| {
        unhide_control_bar_parent_while_ingame(wm);
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

    // C++ W3DMouse::draw calls Mouse::drawTooltip after the cursor (W3DMouse.cpp:565-567).
    // Menu shell never hits InGame HUD; presentation shell may have queued already.
    if !game_client::gui::cursor_tooltip_already_submitted() {
        game_client::gui::tick_cursor_tooltip();
    }
    let _ = game_client::gui::submit_cursor_tooltip(&mut renderer);

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
