use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

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

#[derive(Debug, Clone)]
struct AnimSoundList {
    bone_name: Option<String>,
    entries: Vec<AnimSoundInfo>,
}

impl Default for AnimSoundList {
    fn default() -> Self {
        Self {
            bone_name: Some("root".to_owned()),
            entries: Vec::new(),
        }
    }
}

#[derive(Default)]
struct AnimatedSoundMgrState {
    animation_map: HashMap<String, Arc<AnimSoundList>>,
    sound_library: Option<Arc<dyn SoundLibraryBridge>>,
}

/// Host-owned animated-sound manager (replaces process STATE).
#[derive(Default)]
pub struct AnimatedSoundMgr {
    pub(crate) state: Mutex<AnimatedSoundMgrState>,
}

impl AnimatedSoundMgr {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(AnimatedSoundMgrState::default()),
        }
    }
}

impl std::fmt::Debug for AnimatedSoundMgr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AnimatedSoundMgr")
    }
}

fn parse_animation_map(
    state: &mut AnimatedSoundMgrState,
    text: &str,
) {
    let mut lists: HashMap<String, (AnimSoundList, HashSet<String>)> = HashMap::new();
    let mut section: Option<String> = None;
    // INIClass keeps the first entry for a duplicate key and enumerates in
    // source order. configparser's map instead discards both properties.
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            if let Some(end) = line.find(']') {
                section = Some(line[1..end].trim().to_owned());
            }
            continue;
        }
        let Some(section) = &section else { continue };
        let line = line.split(';').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        let (list, keys) = lists.entry(section.clone()).or_default();
        if !keys.insert(key.to_owned()) {
            continue;
        }
        if key == "BoneName" {
            list.bone_name = Some(value.to_owned());
            continue;
        }

        let params: Vec<&str> = value
            .split(',')
            .filter(|s| !s.is_empty())
            .collect();

        if params.len() < 2 {
            continue;
        }

        // atoi consumes an optional sign and decimal digit prefix; clamp
        // overflow rather than depending on the C runtime's undefined result.
        let token = params[0].trim_start();
        let (negative, digits) = match token.as_bytes().first() {
            Some(b'-') => (true, &token[1..]),
            Some(b'+') => (false, &token[1..]),
            _ => (false, token),
        };
        let magnitude = digits.bytes().take_while(u8::is_ascii_digit).fold(0_i64, |n, digit| {
            n.saturating_mul(10).saturating_add(i64::from(digit - b'0'))
        });
        let frame = (if negative { -magnitude } else { magnitude })
            .clamp(i32::MIN as i64, i32::MAX as i64) as f32;
        let sound_name = params[1].trim().to_owned();
        let is_2d = params.iter().skip(2).any(|token| token.contains("2D"));
        let is_stop = params.iter().skip(2).any(|token| token.contains("STOP"));

        list.entries.push(AnimSoundInfo {
            frame,
            sound_name,
            is_2d,
            is_stop,
        });
    }

    for (section, (list, _)) in lists {
        if !list.entries.is_empty() {
            state.animation_map.insert(section.to_ascii_uppercase(), Arc::new(list));
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
pub fn initialize<P: AsRef<Path>>(mgr: &AnimatedSoundMgr, ini_path: Option<P>) -> W3DResult<()> {
    let mut state = mgr.state.lock().map_err(|_| W3DError::Unknown)?;

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

    let text = String::from_utf8_lossy(&ini_bytes);
    load_animation_map_from_text(text.to_string())?;
    parse_animation_map(&mut state, &text);
    Ok(())
}

/// Initialize the animated sound manager from already-resolved INI bytes.
pub fn initialize_from_bytes(mgr: &AnimatedSoundMgr, bytes: &[u8], source_name: &str) -> W3DResult<()> {
    let mut state = mgr.state.lock().map_err(|_| W3DError::Unknown)?;

    if !state.animation_map.is_empty() {
        return Ok(());
    }

    let text = String::from_utf8_lossy(bytes);
    load_animation_map_from_text(text.to_string()).map_err(
        |err| {
            W3DError::InvalidParameter(format!(
                "failed to parse animated sound metadata from {}: {}",
                source_name, err
            ))
        },
    )?;

    parse_animation_map(&mut state, &text);
    Ok(())
}

/// Release internal resources.
pub fn shutdown(mgr: &AnimatedSoundMgr) {
    if let Ok(mut state) = mgr.state.lock() {
        state.animation_map.clear();
        state.sound_library = None;
    }
}

/// Install the bridge to the active sound library.
pub fn set_sound_library(mgr: &AnimatedSoundMgr, library: Arc<dyn SoundLibraryBridge>) {
    if let Ok(mut state) = mgr.state.lock() {
        state.sound_library = Some(library);
    }
}

/// Retrieve the embedded sound bone name for the provided animation, if one exists.
pub fn embedded_sound_bone(mgr: &AnimatedSoundMgr, anim: &HAnimClass) -> Option<String> {
    mgr.state.lock().ok().and_then(|state| {
        let key = anim.get_name().to_ascii_uppercase();
        state
            .animation_map
            .get(&key)
            .and_then(|list| list.bone_name.clone())
    })
}

fn find_list<'a>(state: &'a AnimatedSoundMgrState, anim: &HAnimClass) -> Option<&'a Arc<AnimSoundList>> {
    let key = anim.get_name().to_ascii_uppercase();
    state.animation_map.get(&key)
}

/// Trigger sounds for the animation if the frame range crosses sound markers.
pub fn trigger_sound(
    mgr: &AnimatedSoundMgr,
    anim: &HAnimClass,
    previous_frame: f32,
    current_frame: f32,
    transform: &Mat4,
) -> f32 {
    let (sound_list, library) = {
        let state = match mgr.state.lock() {
            Ok(guard) => guard,
            Err(_) => return previous_frame,
        };
        let Some(sound_list) = find_list(&state, anim) else {
            return previous_frame;
        };
        let Some(library) = &state.sound_library else {
            return previous_frame;
        };
        (Arc::clone(sound_list), Arc::clone(library))
    };
    // A callback may shut down or reconfigure the manager. Keep this invocation's
    // list and library alive without holding the mutex across external calls.

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
pub fn has_embedded_sounds(mgr: &AnimatedSoundMgr, anim: &HAnimClass) -> bool {
    mgr.state
        .lock()
        .map(|state| find_list(&state, anim).is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingLibrary {
        calls: Mutex<Vec<String>>,
    }

    impl SoundLibraryBridge for RecordingLibrary {
        fn play_3d_audio(&self, name: &str, _: &Mat4) -> W3DResult<()> {
            self.calls.lock().unwrap().push(format!("3d:{name}"));
            Ok(())
        }

        fn play_2d_audio(&self, name: &str) -> W3DResult<()> {
            self.calls.lock().unwrap().push(format!("2d:{name}"));
            Ok(())
        }

        fn stop_audio(&self, name: &str) -> W3DResult<()> {
            self.calls.lock().unwrap().push(format!("stop:{name}"));
            Ok(())
        }
    }

    #[test]
    fn trigger_uses_authored_order_and_returns_last_triggered_frame() {
        let mgr = AnimatedSoundMgr::new();
        initialize_from_bytes(&mgr, b"[TestAnim]\nEntry1=20, Loop, 2D\nEntry2=10, Loop, STOP\n", "inline").unwrap();
        let library = Arc::new(RecordingLibrary::default());
        set_sound_library(&mgr, library.clone());
        let anim = HAnimClass::new("TestAnim", "", 30, 30.0);

        assert_eq!(trigger_sound(&mgr, &anim, 0.0, 20.0, &Mat4::IDENTITY), 10.0);
        assert_eq!(*library.calls.lock().unwrap(), ["2d:Loop", "stop:Loop"]);
    }

    #[test]
    fn authored_values_use_integer_frames_case_sensitive_flags_and_first_duplicate() {
        let mgr = AnimatedSoundMgr::new();
        let ini = b"[TestAnim]\n\
            first=+12.9, One, extra2Dflag\n\
            first=3, Replaced, STOP\n\
            second= 15frames, Two, stop\n\
            third=16, Three, preSTOPpost, 2D\n";
        initialize_from_bytes(&mgr, ini, "inline").unwrap();
        let library = Arc::new(RecordingLibrary::default());
        set_sound_library(&mgr, library.clone());
        let anim = HAnimClass::new("TestAnim", "", 30, 30.0);

        assert_eq!(embedded_sound_bone(&mgr, &anim).as_deref(), Some("root"));
        assert_eq!(trigger_sound(&mgr, &anim, 0.0, 16.0, &Mat4::IDENTITY), 16.0);
        assert_eq!(*library.calls.lock().unwrap(), ["2d:One", "3d:Two", "stop:Three"]);
    }

    #[test]
    fn bone_only_section_does_not_create_sound_list() {
        let mgr = AnimatedSoundMgr::new();
        initialize_from_bytes(&mgr, b"[TestAnim]\nBoneName=Turret\n", "inline").unwrap();
        let anim = HAnimClass::new("TestAnim", "", 30, 30.0);
        assert!(!has_embedded_sounds(&mgr, &anim));
        assert_eq!(embedded_sound_bone(&mgr, &anim), None);
    }

    #[test]
    fn callback_can_reenter_manager_without_deadlocking() {
        struct ReentrantLibrary(Arc<AnimatedSoundMgr>);
        impl SoundLibraryBridge for ReentrantLibrary {
            fn play_3d_audio(&self, _: &str, _: &Mat4) -> W3DResult<()> {
                shutdown(&self.0);
                Ok(())
            }
            fn play_2d_audio(&self, _: &str) -> W3DResult<()> { unreachable!() }
            fn stop_audio(&self, _: &str) -> W3DResult<()> { unreachable!() }
        }

        let mgr = Arc::new(AnimatedSoundMgr::new());
        initialize_from_bytes(&mgr, b"[TestAnim]\nEntry=1, Fire\n", "inline").unwrap();
        set_sound_library(&mgr, Arc::new(ReentrantLibrary(mgr.clone())));
        let anim = HAnimClass::new("TestAnim", "", 30, 30.0);
        assert_eq!(trigger_sound(&mgr, &anim, 0.0, 2.0, &Mat4::IDENTITY), 1.0);
        assert!(!has_embedded_sounds(&mgr, &anim));
    }

    #[test]
    fn initialize_from_bytes_loads_embedded_sound_definitions() {
        let mgr = AnimatedSoundMgr::new();
        shutdown(&mgr);

        let ini = br#"
[TestAnim]
BoneName = Turret
Entry1 = 10, TankFire, 2D
Entry2 = 20, TankLoop, STOP
"#;

        initialize_from_bytes(&mgr, ini, "inline").expect("animated sound bytes should parse");

        let state = mgr.state.lock().expect("animated sound state poisoned");
        let list = state
            .animation_map
            .get("TESTANIM")
            .expect("animation entry should exist");
        assert_eq!(list.bone_name.as_deref(), Some("Turret"));
        assert_eq!(list.entries.len(), 2);
        assert!(list.entries[0].is_2d);
        assert!(list.entries[1].is_stop);
    }
}
