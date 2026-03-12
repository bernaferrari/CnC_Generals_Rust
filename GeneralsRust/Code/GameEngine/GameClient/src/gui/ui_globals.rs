//! UI renderer globals for legacy UI callbacks.

use std::sync::{Arc, OnceLock, RwLock};

use super::ui_renderer::UIRenderer;

static UI_RENDERER: OnceLock<Arc<RwLock<UIRenderer>>> = OnceLock::new();

pub fn set_ui_renderer(renderer: Arc<RwLock<UIRenderer>>) {
    let _ = UI_RENDERER.set(renderer);
}

pub fn with_ui_renderer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Arc<RwLock<UIRenderer>>) -> R,
{
    UI_RENDERER.get().map(f)
}
