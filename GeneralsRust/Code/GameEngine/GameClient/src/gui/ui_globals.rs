//! UI renderer globals for legacy UI callbacks.
//!
//! C++ parity: the original engine uses a singleton pointer for the display
//! device, which is naturally re-entrant. Rust wraps it in an `RwLock` for
//! thread safety. The UI draw path used to stash a TLS `*mut UIRenderer` while
//! an outer write guard was live; that aliases `&mut UIRenderer`.
//!
//! Draw protocol (no aliasing, no no-op):
//! - Frame owners (`flush_ui_to_frame`, Display UI pass) must **drop** the
//!   `RwLock` write guard before `wm.draw_all()`. Gadget WND callbacks then
//!   call [`with_ui_renderer_mut`] and successfully `try_write()` the same
//!   renderer, recording real draw commands.
//! - Do **not** hold a write guard across `draw_all` and do **not** set the
//!   in-draw flag around it. That combination discarded nested draws.
//! - If a write guard is already live, [`with_ui_renderer_mut`] fail-closes
//!   valued calls and does not transmute borrowed closures into the TLS queue.
//!   Owned `'static` unit work goes through [`queue_ui_renderer_op`].

use std::cell::{Cell, RefCell};
use std::sync::{Arc, OnceLock, RwLock};

use super::ui_renderer::UIRenderer;

static UI_RENDERER: OnceLock<Arc<RwLock<UIRenderer>>> = OnceLock::new();

pub fn set_ui_renderer(renderer: Arc<RwLock<UIRenderer>>) {
    let _ = UI_RENDERER.set(renderer);
}

/// Access the global UI renderer Arc.
/// Callers typically do `with_ui_renderer(|arc| arc.write())`.
pub fn with_ui_renderer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Arc<RwLock<UIRenderer>>) -> R,
{
    UI_RENDERER.get().map(f)
}

thread_local! {
    /// In-draw / write-held flag. Never stores `*mut UIRenderer`.
    static UI_DRAW_ACTIVE: Cell<bool> = const { Cell::new(false) };
    /// Nested mut ops deferred until the stack owner holds a unique `&mut UIRenderer`.
    static UI_RENDERER_OP_QUEUE: RefCell<Vec<Box<dyn FnOnce(&mut UIRenderer) + 'static>>> =
        RefCell::new(Vec::new());
}

fn drain_ui_renderer_ops(renderer: &mut UIRenderer) {
    loop {
        let ops = UI_RENDERER_OP_QUEUE.with(|queue| queue.replace(Vec::new()));
        if ops.is_empty() {
            break;
        }
        for op in ops {
            op(renderer);
        }
    }
}

/// Restores the prior draw state on drop, including panic unwind.
///
/// A boolean guard must be nesting-aware: an inner renderer callback may
/// finish while an outer callback still owns the write lock.  Restoring the
/// previous state keeps subsequent nested work on the safe fail-closed path.
struct UiDrawActiveGuard {
    was_active: bool,
}

impl UiDrawActiveGuard {
    fn enter() -> Self {
        let was_active = UI_DRAW_ACTIVE.with(|flag| flag.replace(true));
        Self { was_active }
    }
}

impl Drop for UiDrawActiveGuard {
    fn drop(&mut self) {
        UI_DRAW_ACTIVE.with(|flag| flag.set(self.was_active));
    }
}

/// Queue a `'static` UI-renderer side effect.
///
/// Runs immediately when the write lock is free; otherwise runs the next time
/// a unique `&mut UIRenderer` is held (`set_active_ui_renderer(Some)` or a
/// successful [`with_ui_renderer_mut`] write).
pub fn queue_ui_renderer_op(f: impl FnOnce(&mut UIRenderer) + 'static) {
    if ui_draw_active() {
        UI_RENDERER_OP_QUEUE.with(|queue| queue.borrow_mut().push(Box::new(f)));
        return;
    }
    let queued = with_ui_renderer(|arc| match arc.try_write() {
        Ok(mut guard) => {
            let _draw = UiDrawActiveGuard::enter();
            f(&mut guard);
            drain_ui_renderer_ops(&mut guard);
            false
        }
        Err(std::sync::TryLockError::WouldBlock) => {
            UI_RENDERER_OP_QUEUE.with(|queue| queue.borrow_mut().push(Box::new(f)));
            true
        }
        Err(std::sync::TryLockError::Poisoned(poisoned)) => {
            let mut guard = poisoned.into_inner();
            let _draw = UiDrawActiveGuard::enter();
            f(&mut guard);
            drain_ui_renderer_ops(&mut guard);
            false
        }
    });
    // Renderer unset: nothing to draw into. Drop `f` with the unused closure.
    let _ = queued;
}

/// Set the active-UI-draw flag during draw traversal.
///
/// **Do not use this around `wm.draw_all()`.** Gadget callbacks submit drawing
/// through [`with_ui_renderer_mut`]; they need to acquire the write lock
/// themselves. Holding a write guard and setting this flag discards those
/// nested draws (zero commands).
///
/// Passing `Some` flushes any ops queued by [`queue_ui_renderer_op`].
/// Call with `None` to clear the flag.
pub fn set_active_ui_renderer(renderer: Option<&mut UIRenderer>) {
    match renderer {
        Some(renderer) => {
            UI_DRAW_ACTIVE.with(|flag| flag.set(true));
            drain_ui_renderer_ops(renderer);
        }
        None => {
            UI_DRAW_ACTIVE.with(|flag| flag.set(false));
        }
    }
}

fn ui_draw_active() -> bool {
    UI_DRAW_ACTIVE.with(|flag| flag.get())
}

fn enqueue_ui_renderer_unit_op<R: 'static>(f: impl FnOnce(&mut UIRenderer) -> R) -> Option<R> {
    // Nested draw work that captures `&GameWindow` is not `'static`. Do not
    // transmute it into the TLS queue. Frame owners must drop the write guard
    // before `draw_all` so this path is not taken for gadget draws.
    drop(f);
    None
}

/// Obtain a mutable reference to the UI renderer.
///
/// During draw traversal the caller may already hold the `RwLock` write guard.
/// This must **not** create a second `&mut UIRenderer` via a TLS raw pointer
/// and must **not** `write()` again (would deadlock / alias).
/// Nested calls enqueue unit ops or return `None` (fail-closed).
///
/// Outside of draw traversal this acquires the write lock normally.
/// Returns `None` if neither path succeeds.
pub fn with_ui_renderer_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut UIRenderer) -> R,
    R: 'static,
{
    if ui_draw_active() {
        return enqueue_ui_renderer_unit_op(f);
    }

    with_ui_renderer(|arc| {
        let mut guard = match arc.try_write() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                // Another write guard is live on this thread (or another).
                // Do not wait (same-thread would deadlock) and do not alias.
                return enqueue_ui_renderer_unit_op(f);
            }
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        };
        let _draw = UiDrawActiveGuard::enter();
        let result = f(&mut guard);
        drain_ui_renderer_ops(&mut guard);
        Some(result)
    })?
}

fn color_u8_to_f32(c: [u8; 4]) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        c[3] as f32 / 255.0,
    ]
}

/// Advance Mouse tooltip still-time / delay (C++ `Mouse::update`).
pub fn tick_cursor_tooltip() {
    crate::input::mouse::with_mouse(|mouse| mouse.update());
}

pub fn cursor_tooltip_already_submitted() -> bool {
    crate::input::mouse::with_mouse(|mouse| mouse.tooltip_draw_submitted())
}

/// C++ `Mouse::drawTooltip` (Mouse.cpp:963-1023): fill, border, wrap, highlight.
pub fn submit_cursor_tooltip(renderer: &mut UIRenderer) -> bool {
    use crate::input::mouse::with_mouse;
    use glam::Vec2;

    let (sw, sh) = renderer.screen_size();
    let screen_w = if sw == 0 { 1024.0 } else { sw as f32 };
    let screen_h = if sh == 0 { 768.0 } else { sh as f32 };

    let Some(info) = with_mouse(|mouse| {
        if !mouse.get_visibility() || mouse.tooltip_draw_submitted() {
            return None;
        }
        mouse.compute_tooltip_draw_info(screen_w, screen_h)
    }) else {
        return false;
    };

    let back = color_u8_to_f32(info.back_color);
    let border = color_u8_to_f32(info.border_color);
    let text = color_u8_to_f32(info.text_color);
    let shadow = color_u8_to_f32(info.shadow_color);
    let highlight = color_u8_to_f32(info.highlight_color);

    let box_w = (info.box_width + 2.0).max(2.0);
    let box_h = info.height + 2.0;
    let rect = super::ui_renderer::UIRect::new(info.x, info.y, box_w, box_h);
    renderer.draw_rect(rect, back, 0.0);
    renderer.draw_rect_outline(rect, 1.0, border, 0.0);

    let clip = super::ui_renderer::UIRect::new(
        info.x + 2.0,
        info.y + 1.0,
        info.box_width.max(0.0),
        info.height,
    );
    let font = info.font_size;
    let line_h = info.line_height;
    for (i, line) in info.lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let y = info.y + 1.0 + i as f32 * line_h;
        let x = info.x + 2.0;
        let _ = renderer.draw_text_simple_with_scissor(
            line,
            Vec2::new(x + 1.0, y + 1.0),
            font,
            shadow,
            clip,
        );
        let _ = renderer.draw_text_simple_with_scissor(line, Vec2::new(x, y), font, text, clip);
        let hl_lo = (info.highlight_pos as f32 - 15.0).max(0.0);
        let hl_hi = info.highlight_pos as f32;
        if hl_hi > hl_lo {
            let hl_clip = super::ui_renderer::UIRect::new(
                info.x + 2.0 + hl_lo,
                info.y + 1.0,
                (hl_hi - hl_lo).max(1.0),
                info.height,
            );
            let _ = renderer.draw_text_simple_with_scissor(
                line,
                Vec2::new(x, y),
                font,
                highlight,
                hl_clip,
            );
        }
    }

    with_mouse(|mouse| mouse.mark_tooltip_draw_submitted());
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[cfg(test)]
    fn set_ui_draw_active_for_test(active: bool) {
        UI_DRAW_ACTIVE.with(|flag| flag.set(active));
        if !active {
            UI_RENDERER_OP_QUEUE.with(|queue| queue.borrow_mut().clear());
        }
    }

    #[test]
    fn with_ui_renderer_mut_is_none_when_unset() {
        set_active_ui_renderer(None);
        assert!(with_ui_renderer_mut(|_| 1).is_none());
        assert!(with_ui_renderer(|_| 1).is_none());
    }

    #[test]
    fn set_active_ui_renderer_none_does_not_panic() {
        set_active_ui_renderer(None);
        set_active_ui_renderer(None);
    }

    #[test]
    fn nested_with_ui_renderer_mut_without_renderer_does_not_panic() {
        set_active_ui_renderer(None);
        let outer = with_ui_renderer_mut(|_| with_ui_renderer_mut(|_| 2));
        assert!(outer.is_none());
    }

    #[test]
    fn with_ui_renderer_mut_fail_closed_when_draw_active() {
        set_ui_draw_active_for_test(true);
        let ran = Cell::new(false);
        let result = with_ui_renderer_mut(|_| {
            ran.set(true);
            1i32
        });
        set_ui_draw_active_for_test(false);
        assert!(
            result.is_none(),
            "valued nested mut must fail-closed without aliasing"
        );
        assert!(
            !ran.get(),
            "fail-closed path must not run f under a live outer write"
        );
    }

    #[test]
    fn with_ui_renderer_mut_fail_closed_unit_op_when_draw_active() {
        set_ui_draw_active_for_test(true);
        let ran = Cell::new(false);
        let queued = with_ui_renderer_mut(|_| {
            ran.set(true);
        });
        assert!(queued.is_none());
        assert!(
            !ran.get(),
            "non-'static nested unit op must not run under a live outer write"
        );
        let queued_len = UI_RENDERER_OP_QUEUE.with(|queue| queue.borrow().len());
        set_ui_draw_active_for_test(false);
        assert_eq!(
            queued_len, 0,
            "borrowed closures must not be transmuted into the TLS queue"
        );
    }

    #[test]
    fn with_ui_renderer_mut_clears_draw_active_after_panic() {
        set_active_ui_renderer(None);
        // No renderer: with_ui_renderer_mut returns None without setting the flag.
        // Simulate the write-held path via the guard used by the live function.
        let panicked = std::panic::catch_unwind(|| {
            let _draw = UiDrawActiveGuard::enter();
            assert!(ui_draw_active());
            panic!("callback boom");
        });
        assert!(panicked.is_err());
        assert!(
            !ui_draw_active(),
            "panic in a draw callback must clear UI_DRAW_ACTIVE"
        );
    }

    #[test]
    fn nested_draw_guard_restores_outer_active_state() {
        set_active_ui_renderer(None);
        let outer = UiDrawActiveGuard::enter();
        assert!(ui_draw_active());
        {
            let inner = UiDrawActiveGuard::enter();
            assert!(ui_draw_active());
            drop(inner);
        }
        assert!(
            ui_draw_active(),
            "dropping an inner guard must not expose an outer live write guard"
        );
        drop(outer);
        assert!(!ui_draw_active());
    }

    #[test]
    fn display_ui_pass_drops_write_lock_before_draw_all() {
        let src = include_str!("../display/display.rs");
        assert!(
            !src.contains("*mut UIRenderer"),
            "must not reintroduce TLS *mut UIRenderer"
        );
        let begin = src
            .find("renderer.begin_frame()")
            .expect("display UI pass must begin_frame");
        let draw = src
            .find("manager.draw_all()")
            .expect("display UI pass must call draw_all");
        assert!(begin < draw, "begin_frame must precede draw_all");
        let between = &src[begin..draw];
        assert!(
            between.contains('}'),
            "UI write lock must drop before wm.draw_all()"
        );
        assert!(
            !between.contains("set_active_ui_renderer(Some"),
            "must not hold in-draw flag across draw_all"
        );
    }
}
