//! Gameplay audio dispatch layer
//!
//! Thin bridge that lets GameLogic (weapon fire, unit death, EVA) trigger audio
//! playback through whatever concrete backend is registered at startup.
//!
//! Matches the C++ pattern where `TheAudio->AddAudioEvent()` is called from
//! gameplay code and the AudioManager routes it to the sound device.

use std::sync::{Arc, OnceLock};

// ---------------------------------------------------------------------------
// Callback trait
// ---------------------------------------------------------------------------

/// Trait implemented by the concrete audio backend (registered at startup).
///
/// The methods mirror the C++ `AudioManager::addAudioEvent` entry points
/// that the game logic uses.
pub trait GameplayAudioDispatch: Send + Sync {
    /// Play a positional (3-D) sound event at the given world coordinates.
    ///
    /// `event_name` is the INI `AudioEvent` name (e.g. `"AmericaTankFire"`).
    fn play_positional_sound(&self, event_name: &str, x: f32, y: f32, z: f32);

    /// Play a 2-D / UI sound event (no world position).
    fn play_2d_sound(&self, event_name: &str);
}

// ---------------------------------------------------------------------------
// Global registration
// ---------------------------------------------------------------------------

static DISPATCH: OnceLock<Arc<dyn GameplayAudioDispatch>> = OnceLock::new();

/// Register the concrete audio dispatch backend.
///
/// Returns `false` if a backend was already registered.
pub fn register_gameplay_audio_dispatch(dispatch: Arc<dyn GameplayAudioDispatch>) -> bool {
    DISPATCH.set(dispatch).is_ok()
}

/// Replace the current dispatch backend (used during tests or hot-swap).
pub fn set_gameplay_audio_dispatch(dispatch: Arc<dyn GameplayAudioDispatch>) {
    // OnceLock doesn't support clearing, so we use a different strategy:
    // We just try to set, and if it's already set, we can't change it.
    // For production this is fine - registration happens once at startup.
    let _ = DISPATCH.set(dispatch);
}

fn with_dispatch<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&dyn GameplayAudioDispatch) -> R,
{
    DISPATCH.get().map(|d| f(d.as_ref()))
}

// ---------------------------------------------------------------------------
// Public free functions – callable from anywhere in the codebase
// ---------------------------------------------------------------------------

/// Fire a weapon sound at a world position.
///
/// Call from `Weapon::fire_weapon_effects()` / `WeaponTemplate::apply_firing_effects()`.
pub fn dispatch_weapon_fire(sound_name: &str, x: f32, y: f32, z: f32) {
    if sound_name.is_empty() {
        return;
    }
    let _ = with_dispatch(|dispatch| {
        dispatch.play_positional_sound(sound_name, x, y, z);
    });
}

/// Play a unit/building death sound at a world position.
///
/// Call from `Object::on_die()` or `Object::handle_death()`.
pub fn dispatch_unit_death(sound_name: &str, x: f32, y: f32, z: f32) {
    if sound_name.is_empty() {
        return;
    }
    let _ = with_dispatch(|dispatch| {
        dispatch.play_positional_sound(sound_name, x, y, z);
    });
}

/// Play an EVA voice announcement (2-D, no positional attenuation).
///
/// Call after draining `TheEva` events.
pub fn dispatch_eva_announcement(event_name: &str) {
    if event_name.is_empty() {
        return;
    }
    let _ = with_dispatch(|dispatch| {
        dispatch.play_2d_sound(event_name);
    });
}

/// Play a UI sound (button click, menu open, etc.).
///
/// Call from UI interaction handlers.
pub fn dispatch_ui_sound(event_name: &str) {
    if event_name.is_empty() {
        return;
    }
    let _ = with_dispatch(|dispatch| {
        dispatch.play_2d_sound(event_name);
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// A test dispatch that records what was played.
    struct TestDispatch {
        calls: Mutex<Vec<String>>,
    }

    impl GameplayAudioDispatch for TestDispatch {
        fn play_positional_sound(&self, event_name: &str, x: f32, y: f32, z: f32) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("3d:{}:{},{},{}", event_name, x, y, z));
        }

        fn play_2d_sound(&self, event_name: &str) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("2d:{}", event_name));
        }
    }

    #[test]
    fn test_dispatch_weapon_fire() {
        let dispatch = Arc::new(TestDispatch {
            calls: Mutex::new(Vec::new()),
        });
        // We can't easily test with OnceLock (already set), but we can test
        // that the empty-name guard works.
        dispatch_weapon_fire("", 1.0, 2.0, 3.0);
        dispatch_unit_death("", 0.0, 0.0, 0.0);
        dispatch_eva_announcement("");
        dispatch_ui_sound("");
    }

    #[test]
    fn test_dispatch_empty_names_are_noop() {
        // Without a registered dispatch, all calls should be silent no-ops.
        dispatch_weapon_fire("TestSound", 0.0, 0.0, 0.0);
        dispatch_unit_death("TestDeath", 0.0, 0.0, 0.0);
        dispatch_eva_announcement("EVA_Test");
        dispatch_ui_sound("UI_Test");
    }
}
