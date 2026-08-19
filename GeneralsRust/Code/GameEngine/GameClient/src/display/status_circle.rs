//! C++ `W3DStatusCircle::Render` camera-fade overlay.

use gamelogic::scripting::{get_script_engine, TFade};
use std::sync::Mutex;

/// Fullscreen camera-fade overlay produced by `W3DStatusCircle::Render`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraFadeOverlay {
    pub fade: TFade,
    /// C++ `TheScriptEngine->getFadeValue()` in 0..1.
    pub intensity: f32,
    /// Packed ARGB used as the fullscreen quad vertex color (`255 * intensity`).
    pub diffuse: u32,
}

static LAST_OVERLAY: Mutex<Option<CameraFadeOverlay>> = Mutex::new(None);

/// C++ `W3DStatusCircle::Render` fade branch.
pub fn render_camera_fade() -> Option<CameraFadeOverlay> {
    let overlay = get_script_engine().read().ok().and_then(|guard| {
        let engine = guard.as_ref()?;
        let fade = engine.get_fade();
        if fade == TFade::None {
            return None;
        }
        let intensity = engine.get_fade_value().clamp(0.0, 1.0);
        let channel = (255.0 * intensity) as u32;
        Some(CameraFadeOverlay {
            fade,
            intensity,
            diffuse: (0xff << 24) | (channel << 16) | (channel << 8) | channel,
        })
    });
    if let Ok(mut slot) = LAST_OVERLAY.lock() {
        *slot = overlay;
    }
    overlay
}

/// Last fade overlay computed this frame.
pub fn current_camera_fade() -> Option<CameraFadeOverlay> {
    LAST_OVERLAY.lock().ok().and_then(|slot| *slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_engine_or_none_fade_clears_overlay() {
        let overlay = render_camera_fade();
        if overlay.is_none() {
            assert!(current_camera_fade().is_none());
        }
    }
}
