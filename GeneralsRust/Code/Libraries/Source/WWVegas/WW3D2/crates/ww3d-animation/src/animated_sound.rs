use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use configparser::ini::Ini;
use glam::Mat4;
use ww3d_core::errors::{W3DError, W3DResult};

use crate::HAnimClass;

/// Bridge trait implemented by the host audio system.
pub trait SoundLibraryBridge: Send + Sync {
    fn play_3d_audio(&self, name: &str, transform: &Mat4) -> W3DResult<()>;
    fn play_2d_audio(&self, name: &str) -> W3DResult<()>;
    fn stop_audio(&self, name: &str) -> W3DResult<()>;
}

#[derive(Debug, Clone)]
struct AnimSoundInfo {
    frame: f32,
    sound_name: String,
    is_2d: bool,
    is_stop: bool,
}

#[derive(Debug, Clone, Default)]
struct AnimSoundList {
    bone_name: Option<String>,
    entries: Vec<AnimSoundInfo>,
}

#[derive(Default)]
struct AnimatedSoundMgrState {
    animation_map: HashMap<String, AnimSoundList>,
    sound_library: Option<Arc<dyn SoundLibraryBridge>>,
}

fn manager_state() -> &'static Mutex<AnimatedSoundMgrState> {
    static STATE: OnceLock<Mutex<AnimatedSoundMgrState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(AnimatedSoundMgrState::default()))
}

fn parse_animation_map(
    state: &mut AnimatedSoundMgrState,
    map: HashMap<String, HashMap<String, Option<String>>>,
) {
    for (section, properties) in map {
        if section.trim().is_empty() {
            continue;
        }

        let mut list = AnimSoundList::default();

        for (key, value) in properties {
            if key.eq_ignore_ascii_case("BoneName") {
                if let Some(bone) = value {
                    list.bone_name = Some(bone.trim().to_string());
                }
                continue;
            }

            let Some(raw_value) = value else { continue };
            let params: Vec<String> = raw_value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if params.len() < 2 {
                continue;
            }

            let frame = params[0].parse::<f32>().unwrap_or(0.0);
            let sound_name = params[1].clone();
            let mut is_2d = false;
            let mut is_stop = false;

            for token in params.iter().skip(2) {
                match token.to_ascii_uppercase().as_str() {
                    "2D" => is_2d = true,
                    "STOP" => is_stop = true,
                    _ => {}
                }
            }

            list.entries.push(AnimSoundInfo {
                frame,
                sound_name,
                is_2d,
                is_stop,
            });
        }

        if !list.entries.is_empty() || list.bone_name.is_some() {
            let upper_name = section.to_ascii_uppercase();
            list.entries.sort_by(|a, b| {
                a.frame
                    .partial_cmp(&b.frame)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            state.animation_map.insert(upper_name, list);
        }
    }
}

fn load_animation_map_from_text(
    ini_text: String,
) -> Result<HashMap<String, HashMap<String, Option<String>>>, W3DError> {
    let mut parser = Ini::new();
    parser.read(ini_text).map_err(|err| {
        W3DError::InvalidParameter(format!("failed to parse animated sound definitions: {err}"))
    })
}

/// Initialize the animated sound manager by parsing `w3danimsound.ini`.
pub fn initialize<P: AsRef<Path>>(ini_path: Option<P>) -> W3DResult<()> {
    let mut state = manager_state().lock().map_err(|_| W3DError::Unknown)?;

    if !state.animation_map.is_empty() {
        return Ok(());
    }

    let default_path = Path::new("w3danimsound.ini").to_path_buf();
    let explicit_path = ini_path.as_ref().map(|p| p.as_ref().to_path_buf());
    let path = explicit_path.clone().unwrap_or(default_path);

    let ini_bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(_) => {
            if explicit_path.is_some() {
                log::warn!(
                    "Animated sound definition file not found at {} – embedded animation sounds disabled",
                    path.display()
                );
            } else {
                log::debug!(
                    "Animated sound definition file not found at {} – embedded animation sounds disabled",
                    path.display()
                );
            }
            return Ok(());
        }
    };

    let map = load_animation_map_from_text(String::from_utf8_lossy(&ini_bytes).into_owned())?;
    parse_animation_map(&mut state, map);
    Ok(())
}

/// Initialize the animated sound manager from already-resolved INI bytes.
pub fn initialize_from_bytes(bytes: &[u8], source_name: &str) -> W3DResult<()> {
    let mut state = manager_state().lock().map_err(|_| W3DError::Unknown)?;

    if !state.animation_map.is_empty() {
        return Ok(());
    }

    let map = load_animation_map_from_text(String::from_utf8_lossy(bytes).into_owned())
        .map_err(|err| {
            W3DError::InvalidParameter(format!(
                "failed to parse animated sound metadata from {}: {}",
                source_name, err
            ))
        })?;

    parse_animation_map(&mut state, map);
    Ok(())
}

/// Release internal resources.
pub fn shutdown() {
    if let Ok(mut state) = manager_state().lock() {
        state.animation_map.clear();
        state.sound_library = None;
    }
}

/// Install the bridge to the active sound library.
pub fn set_sound_library(library: Arc<dyn SoundLibraryBridge>) {
    if let Ok(mut state) = manager_state().lock() {
        state.sound_library = Some(library);
    }
}

/// Retrieve the embedded sound bone name for the provided animation, if one exists.
pub fn embedded_sound_bone(anim: &HAnimClass) -> Option<String> {
    manager_state().lock().ok().and_then(|state| {
        let key = anim.get_name().to_ascii_uppercase();
        state
            .animation_map
            .get(&key)
            .and_then(|list| list.bone_name.clone())
    })
}

fn find_list<'a>(state: &'a AnimatedSoundMgrState, anim: &HAnimClass) -> Option<&'a AnimSoundList> {
    let key = anim.get_name().to_ascii_uppercase();
    state.animation_map.get(&key)
}

/// Trigger sounds for the animation if the frame range crosses sound markers.
pub fn trigger_sound(
    anim: &HAnimClass,
    previous_frame: f32,
    current_frame: f32,
    transform: &Mat4,
) -> f32 {
    let state_guard = match manager_state().lock() {
        Ok(guard) => guard,
        Err(_) => return previous_frame,
    };

    let Some(sound_list) = find_list(&state_guard, anim) else {
        return previous_frame;
    };

    let Some(library) = state_guard.sound_library.clone() else {
        return previous_frame;
    };

    let mut last_frame = previous_frame;

    for entry in &sound_list.entries {
        if previous_frame < entry.frame && current_frame >= entry.frame {
            if entry.is_stop {
                let _ = library.stop_audio(&entry.sound_name);
            } else if entry.is_2d {
                let _ = library.play_2d_audio(&entry.sound_name);
            } else {
                let _ = library.play_3d_audio(&entry.sound_name, transform);
            }
            last_frame = entry.frame;
        }
    }

    last_frame
}

/// Check whether the manager has sound data for the given animation.
pub fn has_embedded_sounds(anim: &HAnimClass) -> bool {
    manager_state()
        .lock()
        .map(|state| find_list(&state, anim).is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_from_bytes_loads_embedded_sound_definitions() {
        shutdown();

        let ini = br#"
[TestAnim]
BoneName = Turret
Entry1 = 10, TankFire, 2D
Entry2 = 20, TankLoop, STOP
"#;

        initialize_from_bytes(ini, "inline").expect("animated sound bytes should parse");

        let state = manager_state().lock().expect("animated sound state poisoned");
        let list = state
            .animation_map
            .get("TESTANIM")
            .expect("animation entry should exist");
        assert_eq!(list.bone_name.as_deref(), Some("Turret"));
        assert_eq!(list.entries.len(), 2);
        assert!(list.entries[0].is_2d);
        assert!(list.entries[1].is_stop);

        drop(state);
        shutdown();
    }
}
