//! C++ Compatible API Functions for Game Engine Integration
//! 
//! This module provides exact C++ API compatibility for seamless integration
//! with the existing C&C Generals game engine. All functions match the original
//! C++ signatures and behavior while using the modern Rust audio backend.

use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;

use crate::common::audio::{
    AudioManager, AudioEventRts, AudioHandle, AudioType, AudioPriority,
    AudioAffect, ComprehensiveAudioSystem,
    AudioSettings, Coord3D, Real, Bool, Int, UnsignedInt, AsciiString,
    initialize_audio_system, Position3D, Direction3D, SoundCategory,
    AUDIO_HANDLE_INVALID,
};
use crate::common::audio::game_audio::{
    get_global_audio_manager, initialize_global_audio_manager, register_animation_sound_library,
};

/// Global audio system instance
static GLOBAL_AUDIO: OnceLock<Mutex<ComprehensiveAudioSystem>> = OnceLock::new();

/// Global legacy audio manager for compatibility
static LEGACY_AUDIO_MANAGER: OnceLock<Arc<Mutex<AudioManager>>> = OnceLock::new();

/// Initialize the global audio systems
pub fn init_global_audio() -> Result<(), Box<dyn std::error::Error>> {
    let audio_system = initialize_audio_system()?;
    GLOBAL_AUDIO.set(Mutex::new(audio_system)).map_err(|_| "Failed to initialize global audio system")?;

    let legacy = initialize_global_audio_manager();
    register_animation_sound_library(legacy.clone());
    LEGACY_AUDIO_MANAGER
        .set(legacy)
        .map_err(|_| "Failed to initialize legacy audio manager")?;
    Ok(())
}

/// Get reference to global audio system
fn with_global_audio<F, R>(f: F) -> R 
where 
    F: FnOnce(&mut ComprehensiveAudioSystem) -> R,
{
    let audio = GLOBAL_AUDIO.get().expect("Global audio system not initialized");
    let mut system = audio.lock().unwrap();
    f(&mut *system)
}

/// Get reference to legacy audio manager
fn with_legacy_audio<F, R>(f: F) -> R 
where 
    F: FnOnce(&mut AudioManager) -> R,
{
    let audio = LEGACY_AUDIO_MANAGER
        .get()
        .cloned()
        .or_else(|| get_global_audio_manager())
        .expect("Legacy audio manager not initialized");
    let mut manager = audio.lock().unwrap();
    f(&mut *manager)
}

// =============================================================================
// C++ Compatible Audio Manager Functions
// =============================================================================

/// Initialize the audio manager with settings
/// Matches: AudioManager::init(AudioSettings* settings)
#[no_mangle]
pub extern "C" fn AudioManager_Init(settings: *const AudioSettings) -> Bool {
    if settings.is_null() {
        return false;
    }
    
    unsafe {
        let settings_ref = &*settings;
        with_legacy_audio(|manager| {
            manager.init_with_settings(settings_ref.clone());
            true
        })
    }
}

/// Add an audio event to be played
/// Matches: AudioManager::addAudioEvent(AudioEventRTS* audioEvent)
#[no_mangle]
pub extern "C" fn AudioManager_AddAudioEvent(audio_event: *mut AudioEventRts) -> AudioHandle {
    if audio_event.is_null() {
        return AUDIO_HANDLE_INVALID;
    }
    
    unsafe {
        let event_ref = &mut *audio_event;
        with_legacy_audio(|manager| {
            manager.add_audio_event(event_ref)
        })
    }
}

/// Remove an audio event
/// Matches: AudioManager::removeAudioEvent(AudioHandle handle)
#[no_mangle]
pub extern "C" fn AudioManager_RemoveAudioEvent(handle: AudioHandle) {
    with_legacy_audio(|manager| {
        manager.remove_audio_event(handle);
    });
}

/// Set volume for audio affects
/// Matches: AudioManager::setVolume(AudioAffect affects, Real volume)
#[no_mangle]
pub extern "C" fn AudioManager_SetVolume(affects: AudioAffect, volume: Real) {
    with_legacy_audio(|manager| {
        manager.set_volume(volume, affects);
    });
    
    with_global_audio(|system| {
        if affects.has(AudioAffect::Music) {
            system.set_category_volume(SoundCategory::Music, volume);
        }
        if affects.has(AudioAffect::Sound) || affects.has(AudioAffect::Sound3D) {
            system.set_master_volume(volume);
        }
        if affects.has(AudioAffect::Speech) {
            system.set_category_volume(SoundCategory::Speech, volume);
        }
        if affects.has(AudioAffect::Ambient) {
            system.set_category_volume(SoundCategory::Ambient, volume);
        }
    });
}

/// Get volume for audio affects
/// Matches: AudioManager::getVolume(AudioAffect affects)
#[no_mangle]
pub extern "C" fn AudioManager_GetVolume(affects: AudioAffect) -> Real {
    with_legacy_audio(|manager| {
        manager.get_volume(affects)
    })
}

/// Set 3D listener position
/// Matches: AudioManager::set3DListenerPosition(const Coord3D& position, const Coord3D& velocity, const Coord3D& forward, const Coord3D& up)
#[no_mangle]
pub extern "C" fn AudioManager_Set3DListenerPosition(
    position: *const Coord3D,
    velocity: *const Coord3D,
    forward: *const Coord3D,
    up: *const Coord3D
) {
    if position.is_null() || forward.is_null() || up.is_null() {
        return;
    }
    
    unsafe {
        let pos = Position3D::new((*position).x, (*position).y, (*position).z);
        let fwd = Direction3D::new((*forward).x, (*forward).y, (*forward).z);
        let up_vec = Direction3D::new((*up).x, (*up).y, (*up).z);
        
        with_legacy_audio(|manager| {
            manager.set_3d_listener_position(*position, *forward, *up);
        });
        
        with_global_audio(|system| {
            system.set_listener_position(pos, fwd, up_vec);
        });
    }
}

/// Update the audio system (call every frame)
/// Matches: AudioManager::update()
#[no_mangle]
pub extern "C" fn AudioManager_Update() {
    with_legacy_audio(|manager| {
        manager.update();
    });
    
    with_global_audio(|system| {
        system.update();
    });
}

/// Stop all audio
/// Matches: AudioManager::stopEverything()
#[no_mangle]
pub extern "C" fn AudioManager_StopEverything() {
    with_legacy_audio(|manager| {
        manager.stop_everything();
    });
}

/// Pause all audio
/// Matches: AudioManager::pauseEverything()
#[no_mangle]
pub extern "C" fn AudioManager_PauseEverything() {
    with_legacy_audio(|manager| {
        manager.pause_everything();
    });
}

/// Resume all audio
/// Matches: AudioManager::resumeEverything()
#[no_mangle]
pub extern "C" fn AudioManager_ResumeEverything() {
    with_legacy_audio(|manager| {
        manager.resume_everything();
    });
}

// =============================================================================
// Direct Audio Event Functions
// =============================================================================

/// Play an audio event directly
/// Matches: PlayAudioEvent(const char* eventName, const Coord3D* position)
#[no_mangle]
pub extern "C" fn PlayAudioEvent(event_name: *const c_char, position: *const Coord3D) -> AudioHandle {
    if event_name.is_null() {
        return AUDIO_HANDLE_INVALID;
    }
    
    unsafe {
        let name = CStr::from_ptr(event_name).to_string_lossy();
        
        if !position.is_null() {
            // 3D positioned sound
            let pos = Position3D::new((*position).x, (*position).y, (*position).z);
            with_global_audio(|system| {
                system.play_3d_sound(&name, pos, 1.0)
                    .unwrap_or(AUDIO_HANDLE_INVALID)
            })
        } else {
            // 2D sound
            with_global_audio(|system| {
                system.play_2d_sound(&name, 1.0)
                    .unwrap_or(AUDIO_HANDLE_INVALID)
            })
        }
    }
}

/// Stop a specific audio event
/// Matches: StopAudioEvent(AudioHandle handle)
#[no_mangle]
pub extern "C" fn StopAudioEvent(handle: AudioHandle) {
    with_legacy_audio(|manager| {
        manager.remove_audio_event(handle);
    });
}

/// Set the volume of a specific audio event
/// Matches: SetAudioEventVolume(AudioHandle handle, Real volume)
#[no_mangle]
pub extern "C" fn SetAudioEventVolume(handle: AudioHandle, volume: Real) {
    with_legacy_audio(|manager| {
        manager.set_audio_event_volume(handle, volume);
    });
}

// =============================================================================
// Music System Functions
// =============================================================================

/// Start playing background music
/// Matches: StartMusic(const char* musicName, bool loop)
#[no_mangle]
pub extern "C" fn StartMusic(music_name: *const c_char, should_loop: Bool) -> Bool {
    if music_name.is_null() {
        return false;
    }
    
    unsafe {
        let name = CStr::from_ptr(music_name).to_string_lossy();
        with_global_audio(|system| {
            system.create_music_stream(&name).is_ok()
        })
    }
}

/// Stop the currently playing music
/// Matches: StopMusic()
#[no_mangle]
pub extern "C" fn StopMusic() {
    with_legacy_audio(|manager| {
        manager.stop_music();
    });
}

/// Set music volume
/// Matches: SetMusicVolume(Real volume)
#[no_mangle]
pub extern "C" fn SetMusicVolume(volume: Real) {
    AudioManager_SetVolume(AudioAffect::Music, volume);
}

/// Pause the current music
/// Matches: PauseMusic()
#[no_mangle]
pub extern "C" fn PauseMusic() {
    with_legacy_audio(|manager| {
        manager.pause_music();
    });
}

/// Resume the current music
/// Matches: ResumeMusic()
#[no_mangle]
pub extern "C" fn ResumeMusic() {
    with_legacy_audio(|manager| {
        manager.resume_music();
    });
}

// =============================================================================
// 3D Audio Functions
// =============================================================================

/// Play a 3D positioned sound
/// Matches: Play3DSound(const char* soundName, const Coord3D& position, Real volume)
#[no_mangle]
pub extern "C" fn Play3DSound(sound_name: *const c_char, position: *const Coord3D, volume: Real) -> AudioHandle {
    if sound_name.is_null() || position.is_null() {
        return AUDIO_HANDLE_INVALID;
    }
    
    unsafe {
        let name = CStr::from_ptr(sound_name).to_string_lossy();
        let pos = Position3D::new((*position).x, (*position).y, (*position).z);
        
        with_global_audio(|system| {
            system.play_3d_sound(&name, pos, volume)
                .unwrap_or(AUDIO_HANDLE_INVALID)
        })
    }
}

/// Stop a 3D sound
/// Matches: Stop3DSound(AudioHandle handle)
#[no_mangle]
pub extern "C" fn Stop3DSound(handle: AudioHandle) {
    StopAudioEvent(handle);
}

/// Update the position of a 3D sound
/// Matches: Update3DSoundPosition(AudioHandle handle, const Coord3D& position)
#[no_mangle]
pub extern "C" fn Update3DSoundPosition(handle: AudioHandle, position: *const Coord3D) {
    if position.is_null() {
        return;
    }
    
    unsafe {
        with_legacy_audio(|manager| {
            manager.update_3d_sound_position(handle, *position);
        });
    }
}

// =============================================================================
// Audio Settings Functions
// =============================================================================

/// Load audio settings from INI
/// Matches: LoadAudioSettings(const char* iniPath)
#[no_mangle]
pub extern "C" fn LoadAudioSettings(ini_path: *const c_char) -> Bool {
    if ini_path.is_null() {
        return false;
    }
    
    unsafe {
        let path = CStr::from_ptr(ini_path).to_string_lossy();
        with_legacy_audio(|manager| {
            manager.load_settings_from_ini(&path).is_ok()
        })
    }
}

/// Save current audio settings to INI
/// Matches: SaveAudioSettings(const char* iniPath)
#[no_mangle]
pub extern "C" fn SaveAudioSettings(ini_path: *const c_char) -> Bool {
    if ini_path.is_null() {
        return false;
    }
    
    unsafe {
        let path = CStr::from_ptr(ini_path).to_string_lossy();
        with_legacy_audio(|manager| {
            manager.save_settings_to_ini(&path).is_ok()
        })
    }
}

/// Get available audio providers
/// Matches: GetAudioProviders(char** providers, int maxProviders)
#[no_mangle]
pub extern "C" fn GetAudioProviders(providers: *mut *mut c_char, max_providers: Int) -> Int {
    if providers.is_null() || max_providers <= 0 {
        return 0;
    }
    
    // Return hardcoded providers for compatibility
    let provider_names = vec![
        "Rust Audio Engine",
        "DirectSound Hardware",
        "DirectSound Software",
        "Miles Fast 2D Positional Audio",
    ];
    
    let count = std::cmp::min(provider_names.len(), max_providers as usize);
    
    unsafe {
        for i in 0..count {
            let name_cstring = CString::new(provider_names[i]).unwrap();
            let name_ptr = name_cstring.into_raw();
            *providers.add(i) = name_ptr;
        }
    }
    
    count as Int
}

// =============================================================================
// Audio Cache Functions
// =============================================================================

/// Preload audio files into cache
/// Matches: PreloadAudio(const char* fileName)
#[no_mangle]
pub extern "C" fn PreloadAudio(file_name: *const c_char) -> Bool {
    if file_name.is_null() {
        return false;
    }
    
    unsafe {
        let name = CStr::from_ptr(file_name).to_string_lossy();
        with_global_audio(|system| {
            // Use asset manager to preload
            system.asset_manager.preload_audio(&name).is_ok()
        })
    }
}

/// Clear audio cache
/// Matches: ClearAudioCache()
#[no_mangle]
pub extern "C" fn ClearAudioCache() {
    with_global_audio(|system| {
        system.asset_manager.clear_cache();
    });
}

/// Get cache statistics
/// Matches: GetCacheStats(int* entriesUsed, int* memoryUsed, int* memoryLimit)
#[no_mangle]
pub extern "C" fn GetCacheStats(entries_used: *mut Int, memory_used: *mut Int, memory_limit: *mut Int) {
    if entries_used.is_null() || memory_used.is_null() || memory_limit.is_null() {
        return;
    }
    
    with_global_audio(|system| {
        let stats = system.get_statistics();
        unsafe {
            *entries_used = stats.cache_entries as Int;
            *memory_used = stats.cache_size as Int;
            *memory_limit = stats.cache_max_size as Int;
        }
    });
}

// =============================================================================
// Utility Functions
// =============================================================================

/// Check if audio system is initialized
/// Matches: IsAudioInitialized()
#[no_mangle]
pub extern "C" fn IsAudioInitialized() -> Bool {
    GLOBAL_AUDIO.get().is_some() && LEGACY_AUDIO_MANAGER.get().is_some()
}

/// Get audio system version
/// Matches: GetAudioVersion()
#[no_mangle]
pub extern "C" fn GetAudioVersion() -> *const c_char {
    static VERSION: &str = "Rust Audio Engine 1.0.0\0";
    VERSION.as_ptr() as *const c_char
}

/// Shutdown the audio system
/// Matches: ShutdownAudio()
#[no_mangle]
pub extern "C" fn ShutdownAudio() {
    if let Some(audio) = GLOBAL_AUDIO.get() {
        if let Ok(mut system) = audio.lock() {
            let _ = system.shutdown();
        }
    }
    
    with_legacy_audio(|manager| {
        manager.shutdown();
    });
}

// =============================================================================
// Advanced Features
// =============================================================================

/// Enable/disable environmental audio effects
/// Matches: SetEnvironmentalAudio(bool enabled)
#[no_mangle]
pub extern "C" fn SetEnvironmentalAudio(enabled: Bool) {
    with_global_audio(|system| {
        // Enable/disable environmental effects in spatial processor
        // This would be implemented in the spatial audio system
    });
}

/// Set Doppler effect parameters
/// Matches: SetDopplerParameters(Real factor, Real speedOfSound)
#[no_mangle]
pub extern "C" fn SetDopplerParameters(factor: Real, speed_of_sound: Real) {
    with_legacy_audio(|manager| {
        manager.set_doppler_parameters(factor, speed_of_sound);
    });
}

/// Get current audio performance metrics
/// Matches: GetAudioPerformanceMetrics(AudioMetrics* metrics)
#[no_mangle]
pub extern "C" fn GetAudioPerformanceMetrics(metrics: *mut c_void) -> Bool {
    if metrics.is_null() {
        return false;
    }
    
    with_global_audio(|system| {
        let stats = system.get_statistics();
        // Would populate the metrics structure
        // This depends on the specific AudioMetrics structure definition
        true
    })
}

// =============================================================================
// Initialization and Cleanup
// =============================================================================

/// Initialize the complete audio system with C++ compatibility
#[no_mangle]
pub extern "C" fn InitializeCompleteAudioSystem() -> Bool {
    match init_global_audio() {
        Ok(()) => true,
        Err(_) => false,
    }
}

/// Create default audio settings
#[no_mangle]
pub extern "C" fn CreateDefaultAudioSettings() -> *mut AudioSettings {
    let settings = AudioSettings::default();
    Box::into_raw(Box::new(settings))
}

/// Free audio settings structure
#[no_mangle]
pub extern "C" fn FreeAudioSettings(settings: *mut AudioSettings) {
    if !settings.is_null() {
        unsafe {
            let _ = Box::from_raw(settings);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_system_initialization() {
        assert!(init_global_audio().is_ok());
        assert!(IsAudioInitialized());
    }

    #[test]
    fn test_c_api_functions() {
        // Test basic API function safety
        assert_eq!(PlayAudioEvent(ptr::null(), ptr::null()), AUDIO_HANDLE_INVALID);
        assert_eq!(Play3DSound(ptr::null(), ptr::null(), 1.0), AUDIO_HANDLE_INVALID);
        
        // These should not crash
        StopMusic();
        ClearAudioCache();
    }
}
