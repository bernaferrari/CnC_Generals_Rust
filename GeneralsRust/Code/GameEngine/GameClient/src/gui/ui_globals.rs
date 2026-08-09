//! UI renderer globals for legacy UI callbacks.
//!
//! C++ parity: the original engine uses a singleton pointer for the display
//! device, which is naturally re-entrant. Rust wraps it in an `RwLock` for
//! thread safety. The UI draw path used to stash a TLS `*mut UIRenderer` while
//! an outer write guard was live; that aliases `&mut UIRenderer`.
//!
//! Re-entry protocol:
//! - `set_active_ui_renderer(Some(_))` sets a **non-aliased in-draw flag only**
//!   (the pointer is not stored).
//! - While the flag is set, `with_ui_renderer_mut` does not take another write
//!   guard and does not reconstruct `&mut UIRenderer` from a raw pointer.
//!   Unit (`R = ()`) ops are queued; valued calls fail-closed as `None`.
//! - The stack owner flushes the queue the next time it holds a unique
//!   `&mut UIRenderer` (`set_active_ui_renderer(Some)` or a successful write).

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

/// Set the active-UI-draw flag during draw traversal.
///
/// Call with `Some(&mut *renderer)` before entering `wm.draw_all()` if the
/// caller will keep a write guard live across gadget callbacks. Only a
/// boolean is stored — the `&mut UIRenderer` is **not** retained.
///
/// Passing `Some` also flushes any ops queued by a previous nested
/// `with_ui_renderer_mut` onto `renderer`.
/// Call with `None` after draw_all completes.
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
    use std::any::TypeId;
    use std::mem;

    if TypeId::of::<R>() != TypeId::of::<()>() {
        // Valued nested calls cannot be queued. Fail-closed.
        drop(f);
        return None;
    }

    // SAFETY: same scoped-queue invariant as `with_window_manager`: the stack
    // owner drains before returning to *its* caller. Unit UI draw callbacks
    // that capture short-lived borrows must be `'static` (owned rects / `Arc`s).
    let op: Box<dyn FnOnce(&mut UIRenderer) + 'static> = unsafe {
        mem::transmute::<
            Box<dyn FnOnce(&mut UIRenderer) + '_>,
            Box<dyn FnOnce(&mut UIRenderer) + 'static>,
        >(Box::new(move |renderer| {
            let _: R = f(renderer);
        }))
    };
    UI_RENDERER_OP_QUEUE.with(|queue| queue.borrow_mut().push(op));
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
        UI_DRAW_ACTIVE.with(|flag| flag.set(true));
        let result = f(&mut guard);
        drain_ui_renderer_ops(&mut guard);
        UI_DRAW_ACTIVE.with(|flag| flag.set(false));
        Some(result)
    })?
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
    fn with_ui_renderer_mut_queues_unit_op_when_draw_active() {
        set_ui_draw_active_for_test(true);
        let ran = Cell::new(false);
        let queued = with_ui_renderer_mut(|_| {
            ran.set(true);
        });
        assert!(queued.is_none());
        assert!(
            !ran.get(),
            "unit op is queued, not executed under the live outer write"
        );
        let queued_len = UI_RENDERER_OP_QUEUE.with(|queue| queue.borrow().len());
        set_ui_draw_active_for_test(false);
        assert_eq!(queued_len, 1);
    }
}
