////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! GameSounds - Sound effects management system
//! Westwood Studios Pacific
//! Converted to Rust

use crate::common::audio::{
    audio_event_rts::{AudioEventRts, AudioHandle, AudioPriority, Coord3D, ObjectId},
    game_audio::with_sound_playback_hook,
};
use std::sync::{Arc, OnceLock};

// Type aliases
pub type AsciiString = String;
pub type Real = f32;
pub type Bool = bool;
pub type Int = i32;
pub type UnsignedInt = u32;

// Constants for audio control
use super::audio_event_rts::{ST_GLOBAL, ST_SHROUDED, ST_WORLD};
const ST_VOICE: u32 = 0x00000010;
const AC_INTERRUPT: u32 = 0x00000010;
const AP_CRITICAL: AudioPriority = AudioPriority::Critical;
const INVALID_OBJECT_ID: ObjectId = 0xFFFF_FFFF;

/// C++ `SoundManager::canPlayNow` ST_SHROUDED cull (GameSounds.cpp:188-211).
/// Positional, non-ST_GLOBAL, non-AP_CRITICAL events in a non-CLEAR cell are blocked.
pub fn shrouded_positional_event_is_blocked(
    type_field: u32,
    is_positional: bool,
    is_critical: bool,
    position_is_clear: bool,
) -> bool {
    if !is_positional || (type_field & ST_GLOBAL) != 0 || is_critical {
        return false;
    }
    if (type_field & ST_SHROUDED) == 0 {
        return false;
    }
    !position_is_clear
}

// Shroud status enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellShroud {
    Clear,
    Fogged,
    Shrouded,
}

pub trait AudioShroudResolver: Send + Sync {
    fn is_position_visible_to_local_player(&self, position: &Coord3D) -> Bool;
}

static AUDIO_SHROUD_RESOLVER: OnceLock<Arc<dyn AudioShroudResolver>> = OnceLock::new();

pub fn register_audio_shroud_resolver(resolver: Arc<dyn AudioShroudResolver>) -> bool {
    AUDIO_SHROUD_RESOLVER.set(resolver).is_ok()
}

fn with_audio_shroud_resolver<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&dyn AudioShroudResolver) -> R,
{
    AUDIO_SHROUD_RESOLVER
        .get()
        .map(|resolver| f(resolver.as_ref()))
}

/// Sound Manager - handles sound effect playback and management
pub struct SoundManagerImpl {
    // Sample counts and tracking
    num_2d_samples: UnsignedInt,
    num_3d_samples: UnsignedInt,
    num_playing_2d_samples: UnsignedInt,
    num_playing_3d_samples: UnsignedInt,

    // Active sounds tracking
    playing_sounds: Vec<ActiveSound>,

    // Configuration
    listener_position: Coord3D,
    view_radius: Real,
    camera_audible_distance: Real,
}

/// Represents an actively playing sound
#[derive(Debug, Clone)]
pub struct ActiveSound {
    handle: AudioHandle,
    event_name: AsciiString,
    priority: AudioPriority,
    is_3d: Bool,
    position: Option<Coord3D>,
    object_id: ObjectId,
    is_voice: Bool,
}

/// Live-path queries that C++ `SoundManager::canPlayNow` asks of `TheAudio`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayNowAudioQueries {
    pub object_playing_voice: bool,
    pub playing_lower_priority: bool,
    pub playing_already: bool,
    pub violates_limit: bool,
}

impl SoundManagerImpl {
    pub fn new() -> Self {
        SoundManagerImpl {
            num_2d_samples: 0,
            num_3d_samples: 0,
            num_playing_2d_samples: 0,
            num_playing_3d_samples: 0,
            playing_sounds: Vec::new(),
            listener_position: Coord3D::new(),
            view_radius: 1000.0,
            camera_audible_distance: 1000.0,
        }
    }

    /// Initialize the sound system
    pub fn init(&mut self) {
        if self.num_2d_samples == 0 {
            self.num_2d_samples = 16;
        }
        if self.num_3d_samples == 0 {
            self.num_3d_samples = 16;
        }
    }

    /// Post-process loading - called after the audio system is fully initialized
    pub fn post_process_load(&mut self) {
        self.update_sample_counts_from_hardware();
    }

    /// Update the sound system - called each frame
    pub fn update(&mut self) {
        // Update sound positions, remove completed sounds, etc.
        self.cleanup_completed_sounds();
    }

    /// Reset the sound system
    pub fn reset(&mut self) {
        self.num_playing_2d_samples = 0;
        self.num_playing_3d_samples = 0;
        self.playing_sounds.clear();
    }

    /// Called when the application loses focus
    pub fn lose_focus(&mut self) {
        for sound in &self.playing_sounds {
            let _ = with_sound_playback_hook(|hook| hook.pause(sound.handle));
        }
    }

    /// Called when the application regains focus
    pub fn regain_focus(&mut self) {
        for sound in &self.playing_sounds {
            let _ = with_sound_playback_hook(|hook| hook.resume(sound.handle));
        }
    }

    /// Set the listener position for 3D audio calculations
    pub fn set_listener_position(&mut self, position: &Coord3D) {
        self.listener_position = *position;
    }

    /// Set the radius of the view from the center of the screen in world coordinate units
    pub fn set_view_radius(&mut self, view_radius: Real) {
        self.view_radius = view_radius;
    }

    /// Set the camera audible distance
    pub fn set_camera_audible_distance(&mut self, audible_distance: Real) {
        self.camera_audible_distance = audible_distance;
    }

    /// Get the camera audible distance
    pub fn get_camera_audible_distance(&self) -> Real {
        self.camera_audible_distance
    }

    /// Notify that a 2D sample has started playing
    pub fn notify_of_2d_sample_start(&mut self) {
        self.num_playing_2d_samples += 1;
    }

    /// Notify that a 3D sample has started playing
    pub fn notify_of_3d_sample_start(&mut self) {
        self.num_playing_3d_samples += 1;
    }

    /// Notify that a 2D sample has finished playing
    pub fn notify_of_2d_sample_completion(&mut self) {
        if self.num_playing_2d_samples > 0 {
            self.num_playing_2d_samples -= 1;
        }
    }

    /// Notify that a 3D sample has finished playing
    pub fn notify_of_3d_sample_completion(&mut self) {
        if self.num_playing_3d_samples > 0 {
            self.num_playing_3d_samples -= 1;
        }
    }

    /// Get the number of available 2D sample slots
    pub fn get_available_samples(&self) -> Int {
        (self.num_2d_samples as Int) - (self.num_playing_2d_samples as Int)
    }

    /// Get the number of available 3D sample slots
    pub fn get_available_3d_samples(&self) -> Int {
        (self.num_3d_samples as Int) - (self.num_playing_3d_samples as Int)
    }

    /// Get filename for playing from an audio event
    pub fn get_filename_for_play_from_audio_event(
        &self,
        event_to_get_from: &AudioEventRts,
    ) -> AsciiString {
        event_to_get_from
            .resolve_filename()
            .unwrap_or_else(|| event_to_get_from.get_event_name().to_string())
    }

    /// Determine if a sound can be played now based on various criteria
    pub fn can_play_now(&self, event: &mut AudioEventRts) -> Bool {
        self.can_play_now_checked(event, &PlayNowAudioQueries::default())
    }

    /// C++ `SoundManager::canPlayNow` (GameSounds.cpp:174-286).
    pub fn can_play_now_checked(
        &self,
        event: &mut AudioEventRts,
        queries: &PlayNowAudioQueries,
    ) -> Bool {
        let event_info = match event.get_audio_event_info() {
            Some(info) => info,
            None => return false,
        };

        if event.is_positional_audio()
            && (event_info.type_field & ST_GLOBAL) == 0
            && event_info.priority != AP_CRITICAL
        {
            if let Some(pos) = event.get_current_position() {
                let mut distance = self.listener_position;
                distance.sub(pos);

                if distance.length() >= event_info.max_distance {
                    return false;
                }

                if shrouded_positional_event_is_blocked(
                    event_info.type_field,
                    true,
                    event_info.priority == AP_CRITICAL,
                    with_audio_shroud_resolver(|resolver| {
                        resolver.is_position_visible_to_local_player(pos)
                    })
                    .unwrap_or(true),
                ) {
                    return false;
                }
            }
        }

        if self.violates_voice(event, queries.object_playing_voice) {
            return self.is_interrupting(event);
        }
        if queries.violates_limit {
            return false;
        }
        if self.is_interrupting(event) {
            return true;
        }

        if event.is_positional_audio() {
            if (self.num_playing_3d_samples as Int) < (self.num_3d_samples as Int) {
                return true;
            }
        } else if (self.num_playing_2d_samples as Int) < (self.num_2d_samples as Int) {
            return true;
        }

        if queries.playing_lower_priority {
            return true;
        }

        if self.is_interrupting(event) {
            return queries.playing_already;
        }

        false
    }

    /// Check if playing this sound would violate voice restrictions
    fn violates_voice(&self, event: &AudioEventRts, object_playing_voice: Bool) -> Bool {
        if let Some(event_info) = event.get_audio_event_info() {
            if (event_info.type_field & ST_VOICE) != 0 {
                let object_id = event.get_object_id();
                if object_id != 0 && object_id != INVALID_OBJECT_ID {
                    return object_playing_voice;
                }
            }
        }
        false
    }

    #[allow(dead_code)]
    fn violates_limit(&self, event: &mut AudioEventRts) -> Bool {
        let Some(event_info) = event.get_audio_event_info() else {
            return false;
        };

        if event_info.limit <= 0 {
            return false;
        }
        let limit = event_info.limit;
        let interrupting = (event_info.control & AC_INTERRUPT) != 0;
        let event_name = event.get_event_name().to_string();

        let mut total_playing_count = 0;
        for sound in &self.playing_sounds {
            if sound.event_name == event_name {
                if total_playing_count == 0 {
                    event.set_handle_to_kill(sound.handle);
                }
                total_playing_count += 1;
            }
        }

        if interrupting {
            if total_playing_count < limit {
                event.set_handle_to_kill(0);
                return false;
            }
            return false;
        }

        if total_playing_count < limit {
            event.set_handle_to_kill(0);
            return false;
        }

        true
    }

    /// Check if this is an interrupting sound
    fn is_interrupting(&self, event: &AudioEventRts) -> Bool {
        if let Some(event_info) = event.get_audio_event_info() {
            (event_info.control & AC_INTERRUPT) != 0
        } else {
            false
        }
    }

    /// Check if we can kill lower priority sounds to make room
    fn can_kill_lower_priority(&self, event: &AudioEventRts) -> Bool {
        let event_priority = event.get_audio_priority();
        for sound in &self.playing_sounds {
            if sound.priority < event_priority {
                return true;
            }
        }
        false
    }

    fn can_interrupt_existing(&self, event: &AudioEventRts) -> Bool {
        let event_name = event.get_event_name();
        for sound in &self.playing_sounds {
            if sound.event_name == event_name {
                return true;
            }
        }
        false
    }

    /// Remove completed sounds from our tracking list
    fn cleanup_completed_sounds(&mut self) {
        let mut completed_2d = 0u32;
        let mut completed_3d = 0u32;

        self.playing_sounds.retain(|sound| {
            let still_playing =
                with_sound_playback_hook(|hook| hook.is_playing(sound.handle)).unwrap_or(true);
            if !still_playing {
                if sound.is_3d {
                    completed_3d += 1;
                } else {
                    completed_2d += 1;
                }
            }
            still_playing
        });

        self.num_playing_2d_samples = self.num_playing_2d_samples.saturating_sub(completed_2d);
        self.num_playing_3d_samples = self.num_playing_3d_samples.saturating_sub(completed_3d);
    }

    /// Add a sound to our tracking list
    fn track_sound(&mut self, event: &mut AudioEventRts) {
        let (is_voice, object_id) = if let Some(info) = event.get_audio_event_info() {
            (
                (info.type_field & ST_VOICE) != 0,
                if event.get_object_id() != INVALID_OBJECT_ID {
                    event.get_object_id()
                } else {
                    INVALID_OBJECT_ID
                },
            )
        } else {
            (false, INVALID_OBJECT_ID)
        };

        let active_sound = ActiveSound {
            handle: event.get_playing_handle(),
            event_name: event.get_event_name().to_string(),
            priority: event.get_audio_priority(),
            is_3d: event.is_positional_audio(),
            position: event.get_current_position().map(|p| *p),
            object_id,
            is_voice,
        };

        self.playing_sounds.push(active_sound);
    }

    /// Remove a sound from our tracking list
    fn untrack_sound(&mut self, handle: AudioHandle) {
        let mut removed_2d = 0u32;
        let mut removed_3d = 0u32;
        self.playing_sounds.retain(|sound| {
            let keep = sound.handle != handle;
            if !keep {
                if sound.is_3d {
                    removed_3d += 1;
                } else {
                    removed_2d += 1;
                }
            }
            keep
        });

        self.num_playing_2d_samples = self.num_playing_2d_samples.saturating_sub(removed_2d);
        self.num_playing_3d_samples = self.num_playing_3d_samples.saturating_sub(removed_3d);
    }

    /// Get the number of sounds currently playing
    pub fn get_playing_sound_count(&self) -> usize {
        self.playing_sounds.len()
    }

    /// Get information about currently playing sounds
    pub fn get_playing_sounds(&self) -> &[ActiveSound] {
        &self.playing_sounds
    }

    /// Stop all sounds
    pub fn stop_all_sounds(&mut self) {
        self.playing_sounds.clear();
        self.num_playing_2d_samples = 0;
        self.num_playing_3d_samples = 0;
    }

    /// Stop sounds by name
    pub fn stop_sounds_by_name(&mut self, sound_name: &str) {
        let mut removed_2d = 0u32;
        let mut removed_3d = 0u32;
        self.playing_sounds.retain(|sound| {
            let keep = sound.event_name != sound_name;
            if !keep {
                if sound.is_3d {
                    removed_3d += 1;
                } else {
                    removed_2d += 1;
                }
            }
            keep
        });

        self.num_playing_2d_samples = self.num_playing_2d_samples.saturating_sub(removed_2d);
        self.num_playing_3d_samples = self.num_playing_3d_samples.saturating_sub(removed_3d);
    }

    /// Get sample configuration from audio hardware
    fn update_sample_counts_from_hardware(&mut self) {
        if self.num_2d_samples == 0 {
            self.num_2d_samples = 16;
        }
        if self.num_3d_samples == 0 {
            self.num_3d_samples = 16;
        }
        self.num_playing_2d_samples = self.num_playing_2d_samples.min(self.num_2d_samples);
        self.num_playing_3d_samples = self.num_playing_3d_samples.min(self.num_3d_samples);
    }

    pub fn configure_sample_capacity(&mut self, samples_2d: UnsignedInt, samples_3d: UnsignedInt) {
        self.num_2d_samples = samples_2d.max(1);
        self.num_3d_samples = samples_3d.max(1);
        self.num_playing_2d_samples = self.num_playing_2d_samples.min(self.num_2d_samples);
        self.num_playing_3d_samples = self.num_playing_3d_samples.min(self.num_3d_samples);
    }
}

impl Default for SoundManagerImpl {
    fn default() -> Self {
        Self::new()
    }
}

// Implement the trait from game_audio for compatibility
impl super::game_audio::SoundManager for SoundManagerImpl {
    fn add_audio_event(&mut self, mut event: AudioEventRts) -> Result<(), String> {
        if !self.can_play_now(&mut event) {
            return Err("Cannot play sound now".to_string());
        }

        match with_sound_playback_hook(|hook| hook.play(&event)) {
            Some(Ok(())) => {}
            Some(Err(err)) => return Err(format!("Sound backend error: {err}")),
            None => return Err("No sound playback hook registered".to_string()),
        }

        // Track only after backend accept, matching C++ "playing" accounting.
        self.track_sound(&mut event);
        if event.is_positional_audio() {
            self.notify_of_3d_sample_start();
        } else {
            self.notify_of_2d_sample_start();
        }

        Ok(())
    }

    fn can_play_now(&self, event: &AudioEventRts) -> bool {
        let mut probe = event.clone();
        SoundManagerImpl::can_play_now(self, &mut probe)
    }

    fn can_play_now_checked(
        &self,
        event: &mut AudioEventRts,
        queries: &PlayNowAudioQueries,
    ) -> bool {
        SoundManagerImpl::can_play_now_checked(self, event, queries)
    }

    fn post_process_load(&mut self) {
        SoundManagerImpl::post_process_load(self);
    }

    fn update(&mut self) {
        SoundManagerImpl::update(self);
    }

    fn reset(&mut self) {
        SoundManagerImpl::reset(self);
    }

    fn set_listener_position(&mut self, position: &Coord3D) {
        SoundManagerImpl::set_listener_position(self, position);
    }

    fn configure_sample_capacity(&mut self, samples_2d: UnsignedInt, samples_3d: UnsignedInt) {
        SoundManagerImpl::configure_sample_capacity(self, samples_2d, samples_3d);
    }

    fn notify_of_2d_sample_start(&mut self) {
        self.notify_of_2d_sample_start();
    }

    fn notify_of_3d_sample_start(&mut self) {
        self.notify_of_3d_sample_start();
    }

    fn notify_of_2d_sample_completion(&mut self) {
        self.notify_of_2d_sample_completion();
    }

    fn notify_of_3d_sample_completion(&mut self) {
        self.notify_of_3d_sample_completion();
    }

    fn get_available_samples(&self) -> Int {
        self.get_available_samples()
    }

    fn get_available_3d_samples(&self) -> Int {
        self.get_available_3d_samples()
    }
}

/// Factory function to create a new sound manager
pub fn create_sound_manager() -> Box<dyn super::game_audio::SoundManager> {
    let mut manager = SoundManagerImpl::new();
    manager.init();
    Box::new(manager)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::audio::audio_event_rts::AudioEventRts;

    #[test]
    fn test_sound_manager_creation() {
        let manager = SoundManagerImpl::new();
        assert_eq!(manager.get_available_samples(), 0); // No samples allocated yet
        assert_eq!(manager.get_available_3d_samples(), 0);
    }

    #[test]
    fn test_sound_manager_init() {
        let mut manager = SoundManagerImpl::new();
        manager.init();
        assert_eq!(manager.get_available_samples(), 16);
        assert_eq!(manager.get_available_3d_samples(), 16);
    }

    #[test]
    fn test_sample_tracking() {
        let mut manager = SoundManagerImpl::new();
        manager.init();

        manager.notify_of_2d_sample_start();
        assert_eq!(manager.get_available_samples(), 15);

        manager.notify_of_2d_sample_completion();
        assert_eq!(manager.get_available_samples(), 16);

        manager.notify_of_3d_sample_start();
        assert_eq!(manager.get_available_3d_samples(), 15);

        manager.notify_of_3d_sample_completion();
        assert_eq!(manager.get_available_3d_samples(), 16);
    }

    #[test]
    fn test_listener_position() {
        let mut manager = SoundManagerImpl::new();
        let pos = Coord3D {
            x: 100.0,
            y: 200.0,
            z: 300.0,
        };

        manager.set_listener_position(&pos);
        assert_eq!(manager.listener_position.x, 100.0);
        assert_eq!(manager.listener_position.y, 200.0);
        assert_eq!(manager.listener_position.z, 300.0);
    }

    #[test]
    fn test_view_radius() {
        let mut manager = SoundManagerImpl::new();
        manager.set_view_radius(500.0);
        assert_eq!(manager.view_radius, 500.0);
    }
}
