////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// AudioSystem - Enhanced audio management system
// This file mirrors the enhanced audio features from C++ implementation
// Provides 3D audio, event management, and faction-specific audio

use fastrand;
use glam::Vec3;
use log::{debug, error, info, warn};
use rodio::{Decoder, Sink, Source, SpatialSink};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Cursor;
use std::time::Duration;

use crate::assets::archive::ArchiveFileSystem;
use crate::assets::audio::{AudioManager, SendSyncWrapper};
use crate::game_logic::ObjectId;

enum InstanceSink {
    Stereo(SendSyncWrapper<Sink>),
    Spatial(SendSyncWrapper<SpatialSink>),
}

impl InstanceSink {
    fn stop(&self) {
        match self {
            InstanceSink::Stereo(sink) => sink.get().stop(),
            InstanceSink::Spatial(sink) => sink.get().stop(),
        }
    }

    fn empty(&self) -> bool {
        match self {
            InstanceSink::Stereo(sink) => sink.get().empty(),
            InstanceSink::Spatial(sink) => sink.get().empty(),
        }
    }

    fn is_paused(&self) -> bool {
        match self {
            InstanceSink::Stereo(sink) => sink.get().is_paused(),
            InstanceSink::Spatial(sink) => sink.get().is_paused(),
        }
    }

    fn set_volume(&self, value: f32) {
        match self {
            InstanceSink::Stereo(sink) => sink.get().set_volume(value),
            InstanceSink::Spatial(sink) => sink.get().set_volume(value),
        }
    }

    fn set_speed(&self, value: f32) {
        match self {
            InstanceSink::Stereo(sink) => sink.get().set_speed(value),
            InstanceSink::Spatial(sink) => sink.get().set_speed(value),
        }
    }

    fn set_emitter_position(&self, position: Vec3) {
        if let InstanceSink::Spatial(sink) = self {
            sink.get()
                .set_emitter_position([position.x, position.y, position.z]);
        }
    }

    fn set_ear_positions(&self, left: Vec3, right: Vec3) {
        if let InstanceSink::Spatial(sink) = self {
            sink.get().set_left_ear_position([left.x, left.y, left.z]);
            sink.get()
                .set_right_ear_position([right.x, right.y, right.z]);
        }
    }
}

/// Audio event types matching C&C categories (mirrors C++ AudioEventType enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioEventType {
    // Unit voices and acknowledgments
    UnitSelect,
    UnitMove,
    UnitAttack,
    UnitDie,
    UnitCreate,
    UnitPromotion,

    // Weapon sounds
    WeaponFire,
    WeaponHit,
    WeaponReload,
    ShellEject,
    MissileTrail,

    // Explosion and impact sounds
    ExplosionSmall,
    ExplosionMedium,
    ExplosionLarge,
    ImpactMetal,
    ImpactConcrete,
    ImpactDirt,

    // Building sounds
    BuildingConstruct,
    BuildingComplete,
    BuildingDestroy,
    BuildingPowerUp,
    BuildingPowerDown,

    // Ambient and environment
    EngineIdle,
    TreadMovement,
    HelicopterRotor,
    JetEngine,
    WaterSplash,

    // UI and interface
    ButtonClick,
    ButtonHover,
    MenuTransition,
    AlarmWarning,

    // Special effects
    ElectricZap,
    LaserCharge,
    ShieldHit,
    Teleport,

    // Music and themes
    BackgroundMusic,
    VictoryMusic,
    DefeatMusic,
    SuspenseMusic,
}

/// Faction-specific audio sets (matches C++ Faction enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Faction {
    USA,
    China,
    GLA,
    Neutral,
}

/// Audio priority levels (matches C++ AudioPriority enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AudioPriority {
    Background = 0,
    Ambient = 1,
    Interface = 2,
    Unit = 3,
    Weapon = 4,
    Explosion = 5,
    Critical = 6,
    AlwaysPlay = 7,
}

/// 3D audio properties (matches C++ Audio3DProperties structure)
#[derive(Debug, Clone)]
pub struct Audio3DProperties {
    pub position: Vec3,
    pub velocity: Vec3,
    pub max_distance: f32,
    pub rolloff_factor: f32,
    pub doppler_factor: f32,
    pub cone_inner_angle: f32,
    pub cone_outer_angle: f32,
    pub cone_outer_gain: f32,
    pub direction: Vec3,
}

impl Default for Audio3DProperties {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            max_distance: 100.0,
            rolloff_factor: 1.0,
            doppler_factor: 1.0,
            cone_inner_angle: 360.0,
            cone_outer_angle: 360.0,
            cone_outer_gain: 1.0,
            direction: Vec3::Y,
        }
    }
}

/// Audio event definition (matches C++ AudioEvent class)
#[derive(Debug, Clone)]
pub struct AudioEvent {
    pub event_type: AudioEventType,
    pub file_paths: Vec<String>, // Multiple files for variation
    pub faction: Option<Faction>,
    pub priority: AudioPriority,
    pub volume: f32,
    pub pitch_min: f32,
    pub pitch_max: f32,
    pub is_3d: bool,
    pub is_looping: bool,
    pub fade_in_time: f32,
    pub fade_out_time: f32,
    pub min_replay_delay: f32, // Minimum time between replays
    pub max_concurrent: u32,   // Maximum concurrent instances
    pub audio_3d: Option<Audio3DProperties>,
}

impl AudioEvent {
    /// Create new AudioEvent (matches C++ AudioEvent constructor)
    pub fn new(event_type: AudioEventType, file_path: String) -> Self {
        Self {
            event_type,
            file_paths: vec![file_path],
            faction: None,
            priority: AudioPriority::Unit,
            volume: 1.0,
            pitch_min: 1.0,
            pitch_max: 1.0,
            is_3d: false,
            is_looping: false,
            fade_in_time: 0.0,
            fade_out_time: 0.0,
            min_replay_delay: 0.0,
            max_concurrent: 1,
            audio_3d: None,
        }
    }

    /// Add variation files (matches C++ withVariation)
    pub fn with_variation(mut self, additional_files: Vec<String>) -> Self {
        self.file_paths.extend(additional_files);
        self
    }

    /// Set faction (matches C++ withFaction)
    pub fn with_faction(mut self, faction: Faction) -> Self {
        self.faction = Some(faction);
        self
    }

    /// Set priority (matches C++ withPriority)
    pub fn with_priority(mut self, priority: AudioPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Enable 3D audio (matches C++ with3D)
    pub fn with_3d(mut self, properties: Audio3DProperties) -> Self {
        self.is_3d = true;
        self.audio_3d = Some(properties);
        self
    }

    /// Set pitch variation (matches C++ withPitchVariation)
    pub fn with_pitch_variation(mut self, min: f32, max: f32) -> Self {
        self.pitch_min = min;
        self.pitch_max = max;
        self
    }

    /// Enable looping (matches C++ looping)
    pub fn looping(mut self) -> Self {
        self.is_looping = true;
        self
    }

    /// Get random file from variations (matches C++ getRandomFile)
    pub fn get_random_file(&self) -> Option<&String> {
        if self.file_paths.is_empty() {
            return None;
        }
        let index = fastrand::usize(0..self.file_paths.len());
        Some(&self.file_paths[index])
    }

    /// Get random pitch within range (matches C++ getRandomPitch)
    pub fn get_random_pitch(&self) -> f32 {
        if self.pitch_min == self.pitch_max {
            self.pitch_min
        } else {
            fastrand::f32() * (self.pitch_max - self.pitch_min) + self.pitch_min
        }
    }
}

/// Active audio instance (matches C++ AudioInstance class)
pub struct AudioInstance {
    sink: InstanceSink,
    pub event_type: AudioEventType,
    pub object_id: Option<ObjectId>,
    pub start_time_seconds: f32,
    pub properties_3d: Option<Audio3DProperties>,
    pub priority: AudioPriority,
    pub is_looping: bool,
    pub base_volume: f32,
}

/// AudioSystem - Enhanced audio manager (matches C++ AudioSystem class)
/// Builds on the base AudioManager with advanced features
pub struct EnhancedAudioManager {
    base_audio: AudioManager,
    audio_events: HashMap<AudioEventType, AudioEvent>,
    faction_events: HashMap<(AudioEventType, Faction), AudioEvent>,
    active_instances: Vec<AudioInstance>,
    listener_position: Vec3,
    listener_velocity: Vec3,
    listener_orientation: (Vec3, Vec3), // Forward and Up vectors
    master_volume: f32,
    music_volume: f32,
    sfx_volume: f32,
    voice_volume: f32,
    last_play_times: HashMap<AudioEventType, f32>,
    max_audio_instances: usize,
    current_time_seconds: f32,
}

impl EnhancedAudioManager {
    /// Initialize AudioSystem (matches C++ AudioSystem constructor)
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let base_audio = AudioManager::new()?;

        let mut system = Self {
            base_audio,
            audio_events: HashMap::new(),
            faction_events: HashMap::new(),
            active_instances: Vec::new(),
            listener_position: Vec3::ZERO,
            listener_velocity: Vec3::ZERO,
            listener_orientation: (Vec3::NEG_Z, Vec3::Y), // Forward = -Z, Up = Y
            master_volume: 1.0,
            music_volume: 0.7,
            sfx_volume: 0.8,
            voice_volume: 0.9,
            last_play_times: HashMap::new(),
            max_audio_instances: 64,
            current_time_seconds: 0.0,
        };

        system.load_default_audio_events();
        Ok(system)
    }

    /// Register an audio event (matches C++ registerAudioEvent)
    pub fn register_audio_event(&mut self, event: AudioEvent) {
        if let Some(faction) = event.faction {
            self.faction_events
                .insert((event.event_type, faction), event);
        } else {
            self.audio_events.insert(event.event_type, event);
        }
    }

    /// Play an audio event at a world position (matches C++ playAudioEvent3D)
    pub async fn play_audio_event_3d(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        event_type: AudioEventType,
        position: Vec3,
        object_id: Option<ObjectId>,
        faction: Option<Faction>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.play_audio_event_3d_with_delay(
            archive_system,
            event_type,
            position,
            object_id,
            faction,
            0.0,
        )
        .await
    }

    /// Play an audio event at a world position after a delay (used for staged explosions).
    pub async fn play_audio_event_3d_with_delay(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        event_type: AudioEventType,
        position: Vec3,
        object_id: Option<ObjectId>,
        faction: Option<Faction>,
        delay_seconds: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Find the appropriate audio event and clone necessary data
        let event = self.find_audio_event(event_type, faction)?.clone();

        // Check replay delay
        if let Some(&last_play) = self.last_play_times.get(&event_type) {
            if (self.current_time_seconds - last_play) < event.min_replay_delay {
                return Ok(()); // Skip this play
            }
        }

        // Check max concurrent instances
        let current_count = self
            .active_instances
            .iter()
            .filter(|instance| instance.event_type == event_type)
            .count();

        if current_count >= event.max_concurrent as usize {
            // Remove oldest instance of this type
            if let Some(index) = self
                .active_instances
                .iter()
                .position(|instance| instance.event_type == event_type)
            {
                self.active_instances[index].sink.stop();
                self.active_instances.remove(index);
            }
        }

        // Calculate 3D audio properties
        let distance = self.listener_position.distance(position);
        let volume = self.calculate_3d_volume(&event, position, distance);

        if volume < 0.01 {
            return Ok(()); // Too quiet to hear
        }

        // Load and play the audio
        let file_path = match event.get_random_file() {
            Some(file_path) => file_path,
            None => {
                warn!("Audio event {:?} has no file paths, skipping", event_type);
                return Ok(());
            }
        };
        let pitch = event.get_random_pitch();

        match archive_system.open_file(file_path).await {
            Ok(audio_data) => {
                let cursor = Cursor::new(audio_data);
                match Decoder::new(cursor) {
                    Ok(source) => {
                        // Convert to f32 to prevent audio corruption/noise
                        let f32_source = source.convert_samples::<f32>();

                        // Apply looping and pitch modification carefully
                        let final_source: Box<dyn Source<Item = f32> + Send> = if event.is_looping {
                            if (pitch - 1.0).abs() > 0.01 {
                                Box::new(f32_source.repeat_infinite().speed(pitch))
                            } else {
                                Box::new(f32_source.repeat_infinite())
                            }
                        } else if (pitch - 1.0).abs() > 0.01 {
                            Box::new(f32_source.speed(pitch))
                        } else {
                            Box::new(f32_source)
                        };

                        // Create sink and play
                        if let Some(handle) = &self.base_audio.handle {
                            let mut properties = event.audio_3d.clone().unwrap_or_default();
                            properties.position = position;

                            let (listener_forward, listener_up) = self.listener_orientation;
                            let listener_forward = listener_forward.normalize_or_zero();
                            let listener_up = listener_up.normalize_or_zero();
                            let right = listener_forward.cross(listener_up).normalize_or_zero();
                            let ear_offset = 1.0;
                            let left_ear = self.listener_position - right * ear_offset;
                            let right_ear = self.listener_position + right * ear_offset;

                            let sink = SpatialSink::try_new(
                                handle.get(),
                                [position.x, position.y, position.z],
                                [left_ear.x, left_ear.y, left_ear.z],
                                [right_ear.x, right_ear.y, right_ear.z],
                            );
                            match sink {
                                Ok(sink) => {
                                    let sink = SendSyncWrapper::new(sink);
                                    let instance_sink = InstanceSink::Spatial(sink);
                                    instance_sink
                                        .set_volume(volume * self.get_category_volume(event_type));
                                    instance_sink.set_speed(1.0);

                                    if let InstanceSink::Spatial(spatial) = &instance_sink {
                                        let final_source: Box<dyn Source<Item = f32> + Send> =
                                            if delay_seconds > 0.0 {
                                                Box::new(
                                                    final_source.delay(Duration::from_secs_f32(
                                                        delay_seconds,
                                                    )),
                                                )
                                            } else {
                                                final_source
                                            };
                                        spatial.get().append(final_source);
                                    }

                                    let instance = AudioInstance {
                                        sink: instance_sink,
                                        event_type,
                                        object_id,
                                        start_time_seconds: self.current_time_seconds,
                                        properties_3d: Some(properties),
                                        priority: event.priority,
                                        is_looping: event.is_looping,
                                        base_volume: event.volume,
                                    };

                                    self.active_instances.push(instance);
                                    self.last_play_times
                                        .insert(event_type, self.current_time_seconds);

                                    debug!(
                                        "Playing 3D audio event {:?} at {:?}",
                                        event_type, position
                                    );
                                }
                                Err(e) => {
                                    error!("Failed to create audio sink: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to decode audio file {}: {}", file_path, e);
                    }
                }
            }
            Err(e) => {
                warn!("Audio file {} not found: {}", file_path, e);
            }
        }

        Ok(())
    }

    /// Play a 2D audio event (matches C++ playAudioEvent2D)
    pub async fn play_audio_event_2d(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        event_type: AudioEventType,
        volume_override: Option<f32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event = self.find_audio_event(event_type, None)?.clone();
        let base_volume = volume_override.unwrap_or(event.volume);
        let volume = base_volume * self.get_category_volume(event_type);

        let file_path = match event.get_random_file() {
            Some(file_path) => file_path,
            None => {
                warn!("Audio event {:?} has no file paths, skipping", event_type);
                return Ok(());
            }
        };
        let pitch = event.get_random_pitch();

        match archive_system.open_file(file_path).await {
            Ok(audio_data) => {
                let cursor = Cursor::new(audio_data);
                match Decoder::new(cursor) {
                    Ok(source) => {
                        // Convert to f32 to prevent audio corruption/noise
                        let f32_source = source.convert_samples::<f32>();

                        // Apply looping and pitch modification carefully
                        let final_source: Box<dyn Source<Item = f32> + Send> = if event.is_looping {
                            if (pitch - 1.0).abs() > 0.01 {
                                Box::new(f32_source.repeat_infinite().speed(pitch))
                            } else {
                                Box::new(f32_source.repeat_infinite())
                            }
                        } else if (pitch - 1.0).abs() > 0.01 {
                            Box::new(f32_source.speed(pitch))
                        } else {
                            Box::new(f32_source)
                        };

                        if let Some(handle) = &self.base_audio.handle {
                            match Sink::try_new(handle.get()) {
                                Ok(sink) => {
                                    sink.set_volume(volume);
                                    sink.append(final_source);

                                    let instance = AudioInstance {
                                        sink: InstanceSink::Stereo(SendSyncWrapper::new(sink)),
                                        event_type,
                                        object_id: None,
                                        start_time_seconds: self.current_time_seconds,
                                        properties_3d: None,
                                        priority: event.priority,
                                        is_looping: event.is_looping,
                                        base_volume,
                                    };

                                    self.active_instances.push(instance);
                                    debug!("Playing 2D audio event {:?}", event_type);
                                }
                                Err(e) => {
                                    error!("Failed to create 2D audio sink: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to decode audio file {}: {}", file_path, e);
                    }
                }
            }
            Err(e) => {
                warn!("Audio file {} not found: {}", file_path, e);
            }
        }

        Ok(())
    }

    /// Update 3D audio listener position (matches C++ setListenerTransform)
    pub fn set_listener_transform(
        &mut self,
        position: Vec3,
        forward: Vec3,
        up: Vec3,
        velocity: Vec3,
    ) {
        self.listener_position = position;
        self.listener_velocity = velocity;
        self.listener_orientation = (forward.normalize(), up.normalize());

        // Update all 3D audio instances
        self.update_3d_audio();
    }

    /// Update 3D audio calculations (matches C++ update3DAudio)
    fn update_3d_audio(&mut self) {
        let listener_position = self.listener_position;
        let listener_velocity = self.listener_velocity;
        let (listener_forward, listener_up) = self.listener_orientation;
        let listener_forward = listener_forward.normalize_or_zero();
        let listener_up = listener_up.normalize_or_zero();
        let right = listener_forward.cross(listener_up).normalize_or_zero();
        let ear_offset = 1.0;
        let left_ear = listener_position - right * ear_offset;
        let right_ear = listener_position + right * ear_offset;

        let master_volume = self.master_volume;
        let sfx_volume = self.sfx_volume;
        let voice_volume = self.voice_volume;
        let music_volume = self.music_volume;

        for instance in &mut self.active_instances {
            if let Some(properties) = &instance.properties_3d {
                instance.sink.set_emitter_position(properties.position);
                instance.sink.set_ear_positions(left_ear, right_ear);

                let distance = listener_position.distance(properties.position);
                let dir = (properties.position - listener_position).normalize_or_zero();

                // Calculate volume based on distance (matches C++ distance attenuation)
                let volume_scale = if distance <= 1.0 {
                    1.0
                } else if distance >= properties.max_distance {
                    0.0
                } else {
                    (properties.max_distance / distance).powf(properties.rolloff_factor)
                };

                let base_volume = match instance.event_type {
                    AudioEventType::UnitSelect
                    | AudioEventType::UnitMove
                    | AudioEventType::UnitAttack
                    | AudioEventType::UnitDie
                    | AudioEventType::UnitCreate
                    | AudioEventType::UnitPromotion => voice_volume,
                    AudioEventType::BackgroundMusic
                    | AudioEventType::VictoryMusic
                    | AudioEventType::DefeatMusic
                    | AudioEventType::SuspenseMusic => music_volume,
                    _ => sfx_volume,
                } * master_volume;

                instance
                    .sink
                    .set_volume(volume_scale * base_volume * instance.base_volume);

                // Doppler (C++ update3DAudio): modulate playback speed based on relative velocity.
                // Rodio's sink speed is a good match for the per-instance pitch shift.
                let speed_of_sound = 343.0;
                let v_l = listener_velocity
                    .dot(dir)
                    .clamp(-speed_of_sound * 0.5, speed_of_sound * 0.5);
                let v_s = properties
                    .velocity
                    .dot(dir)
                    .clamp(-speed_of_sound * 0.5, speed_of_sound * 0.5);
                let doppler = (speed_of_sound + v_l) / (speed_of_sound + v_s);
                let doppler = 1.0 + (doppler - 1.0) * properties.doppler_factor;
                instance.sink.set_speed(doppler.clamp(0.5, 2.0));
            }
        }
    }

    /// Update the audio system (matches C++ update) - call every frame
    pub fn update(&mut self, _delta_time: f32) {
        // Remove finished audio instances
        self.active_instances.retain(|instance| {
            !instance.sink.empty() && (instance.is_looping || !instance.sink.is_paused())
        });

        // Limit total instances to prevent audio overload
        if self.active_instances.len() > self.max_audio_instances {
            // Sort by priority and age, remove lowest priority oldest sounds
            self.active_instances.sort_by(|a, b| {
                b.priority.cmp(&a.priority).then_with(|| {
                    a.start_time_seconds
                        .partial_cmp(&b.start_time_seconds)
                        .unwrap_or(Ordering::Equal)
                })
            });

            let excess_count = self.active_instances.len() - self.max_audio_instances;
            for _ in 0..excess_count {
                if let Some(instance) = self.active_instances.pop() {
                    instance.sink.stop();
                }
            }
        }

        self.update_3d_audio();
        self.base_audio.update();
    }

    /// Update the audio system with explicit timing data (matches C++ sync clock)
    pub fn update_with_time(&mut self, delta_time: f32, total_time: f32) {
        self.current_time_seconds = total_time;
        self.update(delta_time);
    }

    /// Stop all instances of a specific audio event type (matches C++ stopAudioEvent)
    pub fn stop_audio_event(&mut self, event_type: AudioEventType) {
        for instance in &mut self.active_instances {
            if instance.event_type == event_type {
                instance.sink.stop();
            }
        }

        self.active_instances
            .retain(|instance| instance.event_type != event_type);
    }

    /// Stop all audio associated with an object (matches C++ stopObjectAudio)
    pub fn stop_object_audio(&mut self, object_id: ObjectId) {
        for instance in &mut self.active_instances {
            if instance.object_id == Some(object_id) {
                instance.sink.stop();
            }
        }

        self.active_instances
            .retain(|instance| instance.object_id != Some(object_id));
    }

    /// Set master volume (matches C++ setMasterVolume)
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
        self.base_audio.set_master_volume(self.master_volume);
        self.update_all_volumes();
    }

    /// Set music volume (matches C++ setMusicVolume)
    pub fn set_music_volume(&mut self, volume: f32) {
        self.music_volume = volume.clamp(0.0, 1.0);
        self.base_audio.set_music_volume(self.music_volume);
        self.update_all_volumes();
    }

    /// Set sound effects volume (matches C++ setSFXVolume)
    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.sfx_volume = volume.clamp(0.0, 1.0);
        self.base_audio.set_sfx_volume(self.sfx_volume);
        self.update_all_volumes();
    }

    /// Set voice volume (matches C++ setVoiceVolume)
    pub fn set_voice_volume(&mut self, volume: f32) {
        self.voice_volume = volume.clamp(0.0, 1.0);
        self.update_all_volumes();
    }

    /// Update all volume levels (matches C++ updateAllVolumes)
    fn update_all_volumes(&mut self) {
        let master_volume = self.master_volume;
        let music_volume = self.music_volume;
        let voice_volume = self.voice_volume;
        let sfx_volume = self.sfx_volume;

        for instance in &mut self.active_instances {
            let category_multiplier = match instance.event_type {
                AudioEventType::BackgroundMusic
                | AudioEventType::VictoryMusic
                | AudioEventType::DefeatMusic
                | AudioEventType::SuspenseMusic => music_volume,

                AudioEventType::UnitSelect
                | AudioEventType::UnitMove
                | AudioEventType::UnitAttack
                | AudioEventType::UnitDie
                | AudioEventType::UnitCreate
                | AudioEventType::UnitPromotion => voice_volume,

                _ => sfx_volume,
            };

            let category_volume = category_multiplier * master_volume;
            // 3D instances have distance attenuation handled in `update_3d_audio`, so only apply
            // the user mix there. 2D instances get their per-event base volume scaled here.
            if instance.properties_3d.is_some() {
                instance.sink.set_volume(category_volume);
            } else {
                instance
                    .sink
                    .set_volume(instance.base_volume * category_volume);
            }
        }
    }

    /// Get category volume (matches C++ getCategoryVolume)
    fn get_category_volume(&self, event_type: AudioEventType) -> f32 {
        let category_multiplier = match event_type {
            AudioEventType::BackgroundMusic
            | AudioEventType::VictoryMusic
            | AudioEventType::DefeatMusic
            | AudioEventType::SuspenseMusic => self.music_volume,

            AudioEventType::UnitSelect
            | AudioEventType::UnitMove
            | AudioEventType::UnitAttack
            | AudioEventType::UnitDie
            | AudioEventType::UnitCreate
            | AudioEventType::UnitPromotion => self.voice_volume,

            _ => self.sfx_volume,
        };

        category_multiplier * self.master_volume
    }

    /// Find audio event (matches C++ findAudioEvent)
    fn find_audio_event(
        &self,
        event_type: AudioEventType,
        faction: Option<Faction>,
    ) -> Result<&AudioEvent, Box<dyn std::error::Error>> {
        // Try faction-specific first
        if let Some(faction) = faction {
            if let Some(event) = self.faction_events.get(&(event_type, faction)) {
                return Ok(event);
            }
        }

        // Fall back to generic event
        self.audio_events
            .get(&event_type)
            .ok_or_else(|| format!("Audio event {:?} not found", event_type).into())
    }

    /// Calculate 3D volume (matches C++ calculate3DVolume)
    fn calculate_3d_volume(&self, event: &AudioEvent, _position: Vec3, distance: f32) -> f32 {
        if !event.is_3d {
            return event.volume;
        }

        let properties = match event.audio_3d.as_ref() {
            Some(properties) => properties,
            None => {
                warn!(
                    "3D audio event {:?} missing spatial properties; using raw volume",
                    event.event_type
                );
                return event.volume;
            }
        };

        if distance <= 1.0 {
            return event.volume;
        }

        if distance >= properties.max_distance {
            return 0.0;
        }

        // Simple distance-based attenuation matching C++ behavior
        let volume_scale = (properties.max_distance / distance).powf(properties.rolloff_factor);
        event.volume * volume_scale
    }

    /// Load default audio events (matches C++ loadDefaultAudioEvents)
    fn load_default_audio_events(&mut self) {
        info!("Loading default C&C audio events");

        // === UNIT VOICES ===

        // USA unit voices
        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::UnitSelect,
                "Audio/English/USA/RangerMove.wav".to_string(),
            )
            .with_faction(Faction::USA)
            .with_variation(vec![
                "Audio/English/USA/RangerReady.wav".to_string(),
                "Audio/English/USA/RangerYes.wav".to_string(),
            ])
            .with_priority(AudioPriority::Unit),
        );

        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::UnitMove,
                "Audio/English/USA/RangerMoving.wav".to_string(),
            )
            .with_faction(Faction::USA)
            .with_variation(vec!["Audio/English/USA/RangerMoveOut.wav".to_string()]),
        );

        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::UnitAttack,
                "Audio/English/USA/RangerAttack.wav".to_string(),
            )
            .with_faction(Faction::USA)
            .with_variation(vec!["Audio/English/USA/RangerEngaging.wav".to_string()]),
        );

        // China unit voices
        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::UnitSelect,
                "Audio/English/China/RedGuardReady.wav".to_string(),
            )
            .with_faction(Faction::China)
            .with_variation(vec!["Audio/English/China/RedGuardYes.wav".to_string()]),
        );

        // GLA unit voices
        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::UnitSelect,
                "Audio/English/GLA/RebelReady.wav".to_string(),
            )
            .with_faction(Faction::GLA)
            .with_variation(vec!["Audio/English/GLA/RebelYes.wav".to_string()]),
        );

        // === WEAPON SOUNDS ===

        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::WeaponFire,
                "Audio/Sounds/TankCannonFire.wav".to_string(),
            )
            .with_priority(AudioPriority::Weapon)
            .with_3d(Audio3DProperties {
                max_distance: 150.0,
                rolloff_factor: 1.0,
                ..Default::default()
            })
            .with_pitch_variation(0.9, 1.1),
        );

        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::WeaponFire,
                "Audio/Sounds/MachineGun.wav".to_string(),
            )
            .with_variation(vec![
                "Audio/Sounds/MachineGun2.wav".to_string(),
                "Audio/Sounds/MachineGun3.wav".to_string(),
            ])
            .with_3d(Audio3DProperties {
                max_distance: 80.0,
                ..Default::default()
            })
            .with_pitch_variation(0.95, 1.05),
        );

        // === EXPLOSIONS ===

        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::ExplosionSmall,
                "Audio/Sounds/ExplosionSmall.wav".to_string(),
            )
            .with_variation(vec![
                "Audio/Sounds/ExplosionSmall2.wav".to_string(),
                "Audio/Sounds/ExplosionSmall3.wav".to_string(),
            ])
            .with_priority(AudioPriority::Explosion)
            .with_3d(Audio3DProperties {
                max_distance: 100.0,
                rolloff_factor: 1.2,
                ..Default::default()
            })
            .with_pitch_variation(0.8, 1.2),
        );

        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::ExplosionLarge,
                "Audio/Sounds/ExplosionLarge.wav".to_string(),
            )
            .with_variation(vec!["Audio/Sounds/ExplosionLarge2.wav".to_string()])
            .with_priority(AudioPriority::Explosion)
            .with_3d(Audio3DProperties {
                max_distance: 300.0,
                rolloff_factor: 1.0,
                ..Default::default()
            })
            .with_pitch_variation(0.85, 1.15),
        );

        // === UI SOUNDS ===

        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::ButtonClick,
                "Data/Audio/Sounds/ubutton2.wav".to_string(),
            )
            .with_priority(AudioPriority::Interface)
            .with_pitch_variation(0.95, 1.05),
        );

        self.register_audio_event(
            AudioEvent::new(
                AudioEventType::ButtonHover,
                "Data/Audio/Sounds/uboarder.wav".to_string(),
            )
            .with_priority(AudioPriority::Interface),
        );

        info!(
            "Loaded {} audio events",
            self.audio_events.len() + self.faction_events.len()
        );
    }

    /// Access base audio manager (matches C++ getBaseAudio)
    pub fn get_base_audio(&mut self) -> &mut AudioManager {
        &mut self.base_audio
    }
}

/// Convenience functions for common audio operations (matches C++ helper functions)
impl EnhancedAudioManager {
    /// Play weapon fire sound with realistic positioning (matches C++ playWeaponFire)
    pub async fn play_weapon_fire(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        position: Vec3,
        _weapon_type: &str,
    ) {
        let _ = self
            .play_audio_event_3d(
                archive_system,
                AudioEventType::WeaponFire,
                position,
                None,
                None,
            )
            .await;
    }

    /// Play explosion with appropriate size (matches C++ playExplosion)
    pub async fn play_explosion(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        position: Vec3,
        size: f32, // 0.0 = small, 1.0+ = large
    ) {
        let event_type = if size < 0.5 {
            AudioEventType::ExplosionSmall
        } else if size < 1.5 {
            AudioEventType::ExplosionMedium
        } else {
            AudioEventType::ExplosionLarge
        };

        let _ = self
            .play_audio_event_3d(archive_system, event_type, position, None, None)
            .await;
    }

    /// Play UI button sound (matches C++ playButtonClick)
    pub async fn play_button_click(&mut self, archive_system: &mut ArchiveFileSystem) {
        let _ = self
            .play_audio_event_2d(archive_system, AudioEventType::ButtonClick, None)
            .await;
    }

    /// Play faction music (matches C++ playFactionMusic)
    pub async fn play_faction_music(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        faction: Faction,
    ) {
        let track_candidates = match faction {
            Faction::USA => vec![
                "Data/Audio/Tracks/USA_10.mp3",
                "Data/Audio/Tracks/USA_11.mp3",
                "Music/USA01.mp3", // Fallback
            ],
            Faction::China => vec![
                "Data/Audio/Tracks/CHI_10.mp3",
                "Data/Audio/Tracks/CHI_11.mp3",
                "Data/Audio/Tracks/C_Chix01.mp3",
                "Music/China01.mp3", // Fallback
            ],
            Faction::GLA => vec![
                "Data/Audio/Tracks/GLA_10.mp3",
                "Data/Audio/Tracks/GLA_11.mp3",
                "Music/GLA01.mp3", // Fallback
            ],
            Faction::Neutral => vec![
                "Data/Audio/Tracks/USA_10.mp3", // Default to USA music
                "Music/Music01.mp3",            // Fallback
            ],
        };

        // Try each track until we find one that exists
        for track_name in &track_candidates {
            if archive_system.does_file_exist(track_name) {
                if let Err(e) = self
                    .base_audio
                    .play_background_music(archive_system, track_name)
                    .await
                {
                    warn!("Failed to play faction music {}: {}", track_name, e);
                } else {
                    info!("Playing faction music for {:?}: {}", faction, track_name);
                    return;
                }
            }
        }

        warn!("No faction music found for {:?}", faction);
    }

    /// Play unit acknowledgment sound (matches C++ playUnitAcknowledgment)
    pub async fn play_unit_acknowledgment(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        _faction: Faction,
        _unit_type: &str,
    ) {
        if let Err(e) = self
            .play_audio_event_2d(
                archive_system,
                AudioEventType::UnitSelect,
                Some(self.voice_volume),
            )
            .await
        {
            debug!("Failed to play unit acknowledgment: {}", e);
        }
    }

    /// Play random Command & Conquer music track (matches C++ playRandomCNCMusic)
    pub async fn play_random_cnc_music(&mut self, archive_system: &mut ArchiveFileSystem) {
        if let Err(e) = self.base_audio.play_random_cnc_music(archive_system).await {
            warn!("Failed to play random C&C music: {}", e);
        }
    }
}
