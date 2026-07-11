//! UI renderer globals for legacy UI callbacks.
//!
//! C++ parity: the original engine uses a singleton pointer for the display
//! device, which is naturally re-entrant. Rust wraps it in an RwLock for
//! thread safety, but the single-threaded UI draw path calls back into
//! functions that also acquire the lock. The thread-local active pointer
//! mirrors the C++ singleton semantics during the draw traversal.

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
    static ACTIVE_UI_RENDERER_PTR: std::cell::Cell<Option<*mut UIRenderer>> = const { std::cell::Cell::new(None) };
}

/// Set the active UI renderer pointer during draw traversal.
/// Call with `Some(&mut *renderer)` before entering `wm.draw_all()`.
/// Call with `None` after draw_all completes.
pub fn set_active_ui_renderer(renderer: Option<&mut UIRenderer>) {
    ACTIVE_UI_RENDERER_PTR.with(|cell| {
        cell.set(renderer.map(|r| r as *mut UIRenderer));
    });
}

/// Obtain a mutable reference to the UI renderer.
/// During draw traversal (`flush_ui_to_frame`), the RwLock is already held,
/// so this returns a raw-pointer-based reference instead of deadlocking.
/// Outside of draw traversal, this acquires the write lock normally.
/// Returns `None` if neither path succeeds.
pub fn with_ui_renderer_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut UIRenderer) -> R,
{
    if let Some(ptr) = ACTIVE_UI_RENDERER_PTR.with(|cell| cell.get()) {
        let renderer = unsafe { &mut *ptr };
        return Some(f(renderer));
    }

    with_ui_renderer(|arc| {
        let mut guard = arc.write().ok()?;
        Some(f(&mut guard))
    })?
}
