//! Comprehensive Sound Effects Management System
//!
//! This module provides advanced sound effect management including:
//! - Intelligent sound prioritization and culling
//! - Dynamic sound variation and randomization
//! - Sound pools for performance optimization
//! - Distance-based LOD (Level of Detail) for sound effects
//! - Combat and environmental sound management
//! - Real-time parameter modulation
//! - Sound effect categories and tagging

use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use rand::{thread_rng, Rng};
use smallvec::SmallVec;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use crate::common::audio::{
    AsciiString, AudioAssetManager, AudioEventRts, AudioHandle, AudioPriority, AudioType, Bool,
    Coord3D, Real, TimeOfDay, UnsignedInt,
};

use crate::common::audio::assets::{AudioData, CachePriority, LoadOptions};
use crate::common::audio::spatial::{Position3D, SpatialAudioProcessor, SpatialSource};

/// Maximum number of simultaneous sound effects
pub const MAX_SOUND_EFFECTS: usize = 128;

/// Maximum number of sounds per category
pub const MAX_SOUNDS_PER_CATEGORY: usize = 32;

/// Sound effect distance LOD thresholds
pub const NEAR_DISTANCE: f32 = 25.0;
pub const MEDIUM_DISTANCE: f32 = 100.0;
pub const FAR_DISTANCE: f32 = 500.0;

/// Sound effect cooldown time (prevents spam)
pub const DEFAULT_COOLDOWN_MS: u64 = 100;

/// Sound effect categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundCategory {
    /// User interface sounds
    UI,
    /// Weapon fire sounds
    Weapons,
    /// Explosion sounds
    Explosions,
    /// Vehicle sounds (engines, movement)
    Vehicles,
    /// Infantry sounds (footsteps, voices)
    Infantry,
    /// Building sounds (construction, destruction)
    Buildings,
    /// Environmental sounds (wind, water, etc.)
    Environment,
    /// Ambient background sounds
    Ambient,
    /// Music and musical stingers
    Music,
    /// Voice acting and speech
    Voice,
    /// Special effects (magic, sci-fi, etc.)
    Special,
}

impl SoundCategory {
    /// Get default priority for this category
    pub fn default_priority(&self) -> AudioPriority {
        match self {
            Self::UI => AudioPriority::High,
            Self::Weapons => AudioPriority::High,
            Self::Explosions => AudioPriority::High,
            Self::Voice => AudioPriority::High,
            Self::Vehicles => AudioPriority::Normal,
            Self::Infantry => AudioPriority::Normal,
            Self::Buildings => AudioPriority::Normal,
            Self::Special => AudioPriority::Normal,
            Self::Environment => AudioPriority::Low,
            Self::Ambient => AudioPriority::Low,
            Self::Music => AudioPriority::Low,
        }
    }

    /// Get maximum simultaneous sounds for this category
    pub fn max_simultaneous(&self) -> usize {
        match self {
            Self::UI => 8,
            Self::Weapons => 16,
            Self::Explosions => 12,
            Self::Voice => 4,
            Self::Vehicles => 8,
            Self::Infantry => 12,
            Self::Buildings => 6,
            Self::Special => 8,
            Self::Environment => 8,
            Self::Ambient => 4,
            Self::Music => 2,
        }
    }

    /// Check if this category supports 3D positioning
    pub fn supports_3d(&self) -> bool {
        !matches!(self, Self::UI | Self::Music)
    }
}

/// Sound variation parameters
#[derive(Debug, Clone)]
pub struct SoundVariation {
    /// Volume variation range (-1.0 to 1.0)
    pub volume_variation: f32,
    /// Pitch variation range (-1.0 to 1.0)
    pub pitch_variation: f32,
    /// Random delay before playing (0-1000ms)
    pub delay_variation_ms: u32,
    /// Random selection from multiple files
    pub file_variations: Vec<String>,
    /// Probability of playing (0.0 - 1.0)
    pub play_probability: f32,
}

impl Default for SoundVariation {
    fn default() -> Self {
        Self {
            volume_variation: 0.1,
            pitch_variation: 0.05,
            delay_variation_ms: 0,
            file_variations: Vec::new(),
            play_probability: 1.0,
        }
    }
}

/// Sound effect descriptor
#[derive(Debug, Clone)]
pub struct SoundEffectDescriptor {
    /// Unique sound ID
    pub id: String,
    /// Sound category
    pub category: SoundCategory,
    /// Base file path
    pub file_path: String,
    /// Base volume (0.0 - 2.0)
    pub volume: f32,
    /// Base pitch (0.5 - 2.0)
    pub pitch: f32,
    /// Priority level
    pub priority: AudioPriority,
    /// Loop flag
    pub looping: bool,
    /// 3D sound parameters
    pub min_distance: f32,
    pub max_distance: f32,
    pub rolloff_factor: f32,
    /// Variation parameters
    pub variation: SoundVariation,
    /// Cooldown time in milliseconds
    pub cooldown_ms: u64,
    /// Tags for filtering and organization
    pub tags: Vec<String>,
    /// Custom properties
    pub properties: HashMap<String, String>,
}

impl SoundEffectDescriptor {
    pub fn new(id: String, category: SoundCategory, file_path: String) -> Self {
        Self {
            id,
            category,
            file_path,
            volume: 1.0,
            pitch: 1.0,
            priority: category.default_priority(),
            looping: false,
            min_distance: 1.0,
            max_distance: 100.0,
            rolloff_factor: 1.0,
            variation: SoundVariation::default(),
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            tags: Vec::new(),
            properties: HashMap::new(),
        }
    }

    /// Apply random variation to base parameters
    pub fn apply_variation(&self, rng: &mut impl Rng) -> (f32, f32, Duration) {
        let volume_factor = if self.variation.volume_variation > 0.0 {
            1.0 + rng.gen_range(-self.variation.volume_variation..self.variation.volume_variation)
        } else {
            1.0
        };

        let pitch_factor = if self.variation.pitch_variation > 0.0 {
            1.0 + rng.gen_range(-self.variation.pitch_variation..self.variation.pitch_variation)
        } else {
            1.0
        };

        let delay = if self.variation.delay_variation_ms > 0 {
            Duration::from_millis(rng.gen_range(0..self.variation.delay_variation_ms) as u64)
        } else {
            Duration::ZERO
        };

        (
            (self.volume * volume_factor).clamp(0.0, 2.0),
            (self.pitch * pitch_factor).clamp(0.1, 4.0),
            delay,
        )
    }

    /// Get random file variation
    pub fn get_random_file(&self, rng: &mut impl Rng) -> &String {
        if !self.variation.file_variations.is_empty() {
            let index = rng.gen_range(0..self.variation.file_variations.len());
            &self.variation.file_variations[index]
        } else {
            &self.file_path
        }
    }

    /// Check if sound should play based on probability
    pub fn should_play(&self, rng: &mut impl Rng) -> bool {
        rng.gen::<f32>() < self.variation.play_probability
    }
}

/// Active sound effect instance
#[derive(Debug)]
pub struct ActiveSoundEffect {
    /// Audio handle
    pub handle: AudioHandle,
    /// Sound descriptor ID
    pub descriptor_id: String,
    /// Category
    pub category: SoundCategory,
    /// Priority
    pub priority: AudioPriority,
    /// Start time
    pub start_time: Instant,
    /// Expected duration (if known)
    pub duration: Option<Duration>,
    /// 3D position (if applicable)
    pub position: Option<Position3D>,
    /// Current volume
    pub volume: f32,
    /// Distance to listener (for LOD)
    pub distance_to_listener: f32,
    /// Last update time
    pub last_update: Instant,
    /// Custom properties
    pub properties: HashMap<String, f32>,
}

impl ActiveSoundEffect {
    pub fn new(
        handle: AudioHandle,
        descriptor_id: String,
        category: SoundCategory,
        priority: AudioPriority,
    ) -> Self {
        Self {
            handle,
            descriptor_id,
            category,
            priority,
            start_time: Instant::now(),
            duration: None,
            position: None,
            volume: 1.0,
            distance_to_listener: 0.0,
            last_update: Instant::now(),
            properties: HashMap::new(),
        }
    }

    /// Check if this sound effect is still relevant (not too old)
    pub fn is_relevant(&self, max_age: Duration) -> bool {
        self.start_time.elapsed() < max_age
    }

    /// Get age of this sound effect
    pub fn age(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Check if sound has finished playing
    pub fn is_finished(&self) -> bool {
        if let Some(duration) = self.duration {
            self.age() >= duration
        } else {
            false
        }
    }
}

/// Sound effect pool for performance optimization
#[derive(Debug)]
pub struct SoundPool {
    /// Pool name
    pub name: String,
    /// Pre-loaded audio data
    pub audio_data: Vec<Arc<AudioData>>,
    /// Available handles (not currently playing)
    pub available_handles: VecDeque<AudioHandle>,
    /// Currently playing sounds from this pool
    pub active_sounds: HashMap<AudioHandle, Instant>,
    /// Maximum pool size
    pub max_size: usize,
    /// Pool statistics
    pub total_requests: usize,
    pub cache_hits: usize,
}

impl SoundPool {
    pub fn new(name: String, max_size: usize) -> Self {
        Self {
            name,
            audio_data: Vec::with_capacity(max_size),
            available_handles: VecDeque::with_capacity(max_size),
            active_sounds: HashMap::new(),
            max_size,
            total_requests: 0,
            cache_hits: 0,
        }
    }

    /// Preload audio data into the pool
    pub fn preload(&mut self, audio_data: Vec<Arc<AudioData>>) {
        self.audio_data = audio_data;
        // Initialize available handles
        for i in 0..self.audio_data.len().min(self.max_size) {
            self.available_handles.push_back(i as AudioHandle);
        }
    }

    /// Get an available sound from the pool
    pub fn acquire(&mut self) -> Option<AudioHandle> {
        self.total_requests += 1;

        if let Some(handle) = self.available_handles.pop_front() {
            self.active_sounds.insert(handle, Instant::now());
            self.cache_hits += 1;
            Some(handle)
        } else {
            None
        }
    }

    /// Return a sound to the pool
    pub fn release(&mut self, handle: AudioHandle) {
        if self.active_sounds.remove(&handle).is_some() {
            self.available_handles.push_back(handle);
        }
    }

    /// Get cache hit ratio
    pub fn hit_ratio(&self) -> f32 {
        if self.total_requests > 0 {
            self.cache_hits as f32 / self.total_requests as f32
        } else {
            0.0
        }
    }
}

/// Sound effect manager with advanced features
pub struct SoundEffectManager {
    /// Sound effect descriptors
    descriptors: RwLock<HashMap<String, SoundEffectDescriptor>>,
    /// Active sound effects by handle
    active_sounds: RwLock<HashMap<AudioHandle, ActiveSoundEffect>>,
    /// Active sounds by category
    category_sounds: RwLock<HashMap<SoundCategory, Vec<AudioHandle>>>,
    /// Sound pools for performance
    sound_pools: RwLock<HashMap<String, SoundPool>>,
    /// Asset manager
    asset_manager: Arc<AudioAssetManager>,
    /// Spatial audio processor
    spatial_processor: Arc<SpatialAudioProcessor>,
    /// Last played times for cooldown management
    last_played: RwLock<HashMap<String, Instant>>,
    /// Global settings
    master_volume: RwLock<f32>,
    category_volumes: RwLock<HashMap<SoundCategory, f32>>,
    distance_lod_enabled: RwLock<bool>,
    max_distance_lod: RwLock<f32>,
    /// Performance metrics
    total_sounds_played: parking_lot::Mutex<usize>,
    sounds_culled: parking_lot::Mutex<usize>,
    /// Listener position for distance calculations
    listener_position: RwLock<Position3D>,
}

impl SoundEffectManager {
    pub fn new(asset_manager: Arc<AudioAssetManager>) -> Self {
        let spatial_processor = Arc::new(SpatialAudioProcessor::new());

        let mut category_volumes = HashMap::new();
        for category in [
            SoundCategory::UI,
            SoundCategory::Weapons,
            SoundCategory::Explosions,
            SoundCategory::Vehicles,
            SoundCategory::Infantry,
            SoundCategory::Buildings,
            SoundCategory::Environment,
            SoundCategory::Ambient,
            SoundCategory::Music,
            SoundCategory::Voice,
            SoundCategory::Special,
        ] {
            category_volumes.insert(category, 1.0);
        }

        Self {
            descriptors: RwLock::new(HashMap::new()),
            active_sounds: RwLock::new(HashMap::new()),
            category_sounds: RwLock::new(HashMap::new()),
            sound_pools: RwLock::new(HashMap::new()),
            asset_manager,
            spatial_processor,
            last_played: RwLock::new(HashMap::new()),
            master_volume: RwLock::new(1.0),
            category_volumes: RwLock::new(category_volumes),
            distance_lod_enabled: RwLock::new(true),
            max_distance_lod: RwLock::new(500.0),
            total_sounds_played: parking_lot::Mutex::new(0),
            sounds_culled: parking_lot::Mutex::new(0),
            listener_position: RwLock::new(Position3D::default()),
        }
    }

    /// Register a sound effect descriptor
    pub fn register_sound(&self, descriptor: SoundEffectDescriptor) {
        let id = descriptor.id.clone();
        self.descriptors.write().insert(id, descriptor);
    }

    /// Register multiple sound effects from a configuration
    pub fn register_sounds(&self, descriptors: Vec<SoundEffectDescriptor>) {
        let mut desc_map = self.descriptors.write();
        for descriptor in descriptors {
            let id = descriptor.id.clone();
            desc_map.insert(id, descriptor);
        }
    }

    /// Play a sound effect by ID
    pub fn play_sound(
        &self,
        sound_id: &str,
        position: Option<Position3D>,
        volume_multiplier: f32,
    ) -> Result<AudioHandle, Box<dyn std::error::Error>> {
        // Get sound descriptor
        let descriptor = {
            let descriptors = self.descriptors.read();
            descriptors
                .get(sound_id)
                .cloned()
                .ok_or(format!("Sound not found: {}", sound_id))?
        };

        // Check cooldown
        {
            let last_played = self.last_played.read();
            if let Some(last_time) = last_played.get(sound_id) {
                if last_time.elapsed() < Duration::from_millis(descriptor.cooldown_ms) {
                    return Err("Sound is on cooldown".into());
                }
            }
        }

        // Check if should play based on probability
        let mut rng = thread_rng();
        if !descriptor.should_play(&mut rng) {
            return Err("Sound probability check failed".into());
        }

        // Check category limits
        let category_count = {
            let category_sounds = self.category_sounds.read();
            category_sounds
                .get(&descriptor.category)
                .map(|sounds| sounds.len())
                .unwrap_or(0)
        };

        if category_count >= descriptor.category.max_simultaneous() {
            // Try to cull lower priority sounds
            self.cull_sounds_in_category(descriptor.category, descriptor.priority);
        }

        // Apply distance LOD if enabled and position provided
        let should_skip = if let Some(pos) = position {
            self.should_skip_for_distance(&descriptor, &pos)
        } else {
            false
        };

        if should_skip {
            *self.sounds_culled.lock() += 1;
            return Err("Sound culled due to distance LOD".into());
        }

        // Apply variation
        let (volume, pitch, delay) = descriptor.apply_variation(&mut rng);
        let file_path = descriptor.get_random_file(&mut rng);

        // Calculate final volume
        let category_volume = *self
            .category_volumes
            .read()
            .get(&descriptor.category)
            .unwrap_or(&1.0);
        let master_volume = *self.master_volume.read();
        let final_volume = volume * volume_multiplier * category_volume * master_volume;

        // Try to get from sound pool first
        let handle = if let Some(pool_handle) = self.try_get_from_pool(&descriptor.id) {
            pool_handle
        } else {
            // Load and play normally
            self.play_sound_normal(file_path, final_volume, pitch, descriptor.looping, position)?
        };

        // Create active sound effect
        let mut active_sound = ActiveSoundEffect::new(
            handle,
            sound_id.to_string(),
            descriptor.category,
            descriptor.priority,
        );
        active_sound.position = position;
        active_sound.volume = final_volume;

        if let Some(pos) = position {
            active_sound.distance_to_listener = pos.distance_to(&*self.listener_position.read());
        }

        // Add to active sounds
        self.active_sounds.write().insert(handle, active_sound);

        // Add to category tracking
        self.category_sounds
            .write()
            .entry(descriptor.category)
            .or_default()
            .push(handle);

        // Update cooldown
        self.last_played
            .write()
            .insert(sound_id.to_string(), Instant::now());

        // Update statistics
        *self.total_sounds_played.lock() += 1;

        Ok(handle)
    }

    /// Stop a sound effect
    pub fn stop_sound(&self, handle: AudioHandle) {
        if let Some(active_sound) = self.active_sounds.write().remove(&handle) {
            if let Some(category_sounds) =
                self.category_sounds.write().get_mut(&active_sound.category)
            {
                category_sounds.retain(|&h| h != handle);
            }

            self.return_to_pool(&active_sound.descriptor_id, handle);
        }
    }

    /// Stop all sounds in a category
    pub fn stop_category(&self, category: SoundCategory) {
        let handles_to_stop: Vec<AudioHandle> = {
            let category_sounds = self.category_sounds.read();
            category_sounds
                .get(&category)
                .map(|sounds| sounds.clone())
                .unwrap_or_default()
        };

        for handle in handles_to_stop {
            self.stop_sound(handle);
        }
    }

    /// Set master volume
    pub fn set_master_volume(&self, volume: f32) {
        *self.master_volume.write() = volume.clamp(0.0, 2.0);
        self.update_all_volumes();
    }

    /// Set category volume
    pub fn set_category_volume(&self, category: SoundCategory, volume: f32) {
        self.category_volumes
            .write()
            .insert(category, volume.clamp(0.0, 2.0));
        self.update_category_volumes(category);
    }

    /// Set listener position for distance calculations
    pub fn set_listener_position(&self, position: Position3D) {
        *self.listener_position.write() = position;
        self.update_distance_lod();
    }

    /// Enable/disable distance LOD
    pub fn set_distance_lod_enabled(&self, enabled: bool) {
        *self.distance_lod_enabled.write() = enabled;
    }

    /// Create a sound pool for performance optimization
    pub fn create_sound_pool(&self, pool_name: String, sound_ids: Vec<String>, max_size: usize) {
        let mut pool = SoundPool::new(pool_name.clone(), max_size);

        let mut audio_data = Vec::new();
        for sound_id in sound_ids {
            if let Some(descriptor) = self.descriptors.read().get(&sound_id) {
                let data = self
                    .asset_manager
                    .load_audio(&descriptor.file_path, LoadOptions::default());
                if let Ok(data) = data {
                    audio_data.push(data);
                }
            }
        }

        pool.preload(audio_data);
        self.sound_pools.write().insert(pool_name, pool);
    }

    /// Update all active sounds (call each frame)
    pub fn update(&self) {
        self.cull_finished_sounds();
        self.update_distance_lod();
        self.update_spatial_audio();
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> SoundEffectStats {
        let active_count = self.active_sounds.read().len();
        let total_played = *self.total_sounds_played.lock();
        let culled_count = *self.sounds_culled.lock();

        let mut category_counts = HashMap::new();
        for (category, sounds) in self.category_sounds.read().iter() {
            category_counts.insert(*category, sounds.len());
        }

        SoundEffectStats {
            active_sounds: active_count,
            total_sounds_played: total_played,
            sounds_culled: culled_count,
            category_counts,
            pool_hit_ratios: self.get_pool_hit_ratios(),
        }
    }

    // Private helper methods

    fn should_skip_for_distance(
        &self,
        descriptor: &SoundEffectDescriptor,
        position: &Position3D,
    ) -> bool {
        if !*self.distance_lod_enabled.read() {
            return false;
        }

        let listener_pos = self.listener_position.read();
        let distance = position.distance_to(&*listener_pos);
        let max_distance = *self.max_distance_lod.read();

        // Skip if beyond maximum distance
        if distance > max_distance {
            return true;
        }

        // Apply category-specific distance culling
        match descriptor.category {
            SoundCategory::UI => false, // Never cull UI sounds
            SoundCategory::Weapons => distance > FAR_DISTANCE,
            SoundCategory::Explosions => distance > FAR_DISTANCE,
            SoundCategory::Voice => distance > MEDIUM_DISTANCE,
            _ => distance > descriptor.max_distance,
        }
    }

    fn cull_sounds_in_category(&self, category: SoundCategory, new_priority: AudioPriority) {
        let handles_to_cull: Vec<AudioHandle> = {
            let active_sounds = self.active_sounds.read();
            let mut candidates = Vec::new();

            for (handle, sound) in active_sounds.iter() {
                if sound.category == category && sound.priority < new_priority {
                    candidates.push(*handle);
                }
            }

            // Sort by priority (lowest first) and age (oldest first)
            candidates.sort_by(|&a, &b| {
                let sound_a = active_sounds.get(&a).unwrap();
                let sound_b = active_sounds.get(&b).unwrap();
                sound_a
                    .priority
                    .cmp(&sound_b.priority)
                    .then(sound_b.age().cmp(&sound_a.age()))
            });

            // Take only what we need to free space
            let max_sounds = category.max_simultaneous();
            if candidates.len() > max_sounds / 2 {
                candidates.truncate(max_sounds / 2);
            }

            candidates
        };

        for handle in handles_to_cull {
            self.stop_sound(handle);
            *self.sounds_culled.lock() += 1;
        }
    }

    fn try_get_from_pool(&self, sound_id: &str) -> Option<AudioHandle> {
        // Try to get from sound pool if available
        let mut pools = self.sound_pools.write();
        if let Some(pool) = pools.get_mut(sound_id) {
            pool.acquire()
        } else {
            None
        }
    }

    fn return_to_pool(&self, sound_id: &str, handle: AudioHandle) {
        let mut pools = self.sound_pools.write();
        if let Some(pool) = pools.get_mut(sound_id) {
            pool.release(handle);
        }
    }

    fn play_sound_normal(
        &self,
        file_path: &str,
        volume: f32,
        pitch: f32,
        looping: bool,
        position: Option<Position3D>,
    ) -> Result<AudioHandle, Box<dyn std::error::Error>> {
        use std::sync::atomic::{AtomicU32, Ordering};

        static NEXT_HANDLE: AtomicU32 = AtomicU32::new(10000);

        let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);

        #[cfg(feature = "audio")]
        {
            use rodio::{Decoder, Sink, Source, SpatialSink};
            use std::io::Cursor;

            let audio_data = self
                .asset_manager
                .load_audio(file_path, LoadOptions::default())?;

            let cursor: Cursor<Vec<u8>> = match audio_data.as_ref() {
                crate::common::audio::AudioData::Compressed { data, .. } => {
                    Cursor::new(data.clone())
                }
                crate::common::audio::AudioData::Loaded { samples, metadata } => {
                    use hound::WavWriter;
                    let spec = hound::WavSpec {
                        channels: metadata.channels,
                        sample_rate: metadata.sample_rate,
                        bits_per_sample: 32,
                        sample_format: hound::SampleFormat::Float,
                    };
                    let mut buf = Vec::new();
                    {
                        let mut writer = WavWriter::new(std::io::Cursor::new(&mut buf), spec)?;
                        for &sample in samples {
                            writer.write_sample(sample)?;
                        }
                        writer.finalize()?;
                    }
                    Cursor::new(buf)
                }
                crate::common::audio::AudioData::Streaming { file_path: fp, .. } => {
                    let data = std::fs::read(fp)?;
                    Cursor::new(data)
                }
            };
            let source = Decoder::new(cursor)?;

            let final_source: Box<dyn Source<Item = f32> + Send> = if looping {
                if (pitch - 1.0).abs() > 0.01 {
                    Box::new(source.repeat_infinite().speed(pitch))
                } else {
                    Box::new(source.repeat_infinite())
                }
            } else if (pitch - 1.0).abs() > 0.01 {
                Box::new(source.speed(pitch))
            } else {
                Box::new(source)
            };

            if let Some(pos) = position {
                let (stream, stream_handle) = rodio::OutputStream::try_default()?;
                let spatial_sink = SpatialSink::try_new(
                    &stream_handle,
                    [pos.x, pos.y, pos.z],
                    [-0.1, 0.0, 0.0],
                    [0.1, 0.0, 0.0],
                )?;
                spatial_sink.set_volume(volume.clamp(0.0, 1.0));
                spatial_sink.append(final_source);
                std::mem::forget(stream);
            } else {
                let (stream, stream_handle) = rodio::OutputStream::try_default()?;
                let sink = Sink::try_new(&stream_handle)?;
                sink.set_volume(volume.clamp(0.0, 1.0));
                sink.append(final_source);
                std::mem::forget(stream);
            }
        }

        Ok(handle)
    }

    fn cull_finished_sounds(&self) {
        let handles_to_remove: Vec<AudioHandle> = {
            let active_sounds = self.active_sounds.read();
            active_sounds
                .iter()
                .filter(|(_, sound)| sound.is_finished())
                .map(|(handle, _)| *handle)
                .collect()
        };

        for handle in handles_to_remove {
            self.stop_sound(handle);
        }
    }

    fn update_distance_lod(&self) {
        if !*self.distance_lod_enabled.read() {
            return;
        }

        let listener_pos = *self.listener_position.read();
        let mut sounds_to_update = Vec::new();

        {
            let mut active_sounds = self.active_sounds.write();
            for (handle, sound) in active_sounds.iter_mut() {
                if let Some(position) = sound.position {
                    let new_distance = position.distance_to(&listener_pos);
                    let old_distance = sound.distance_to_listener;

                    if (new_distance - old_distance).abs() > 5.0 {
                        // Threshold to prevent too frequent updates
                        sound.distance_to_listener = new_distance;
                        sounds_to_update.push((*handle, new_distance));
                    }
                }
            }
        }

        // Apply distance-based volume adjustments
        for (handle, distance) in sounds_to_update {
            self.update_sound_for_distance(handle, distance);
        }
    }

    fn update_sound_for_distance(&self, handle: AudioHandle, distance: f32) {
        // Calculate distance-based volume attenuation
        let attenuation = if distance <= NEAR_DISTANCE {
            1.0
        } else if distance <= MEDIUM_DISTANCE {
            1.0 - (distance - NEAR_DISTANCE) / (MEDIUM_DISTANCE - NEAR_DISTANCE) * 0.3
        } else if distance <= FAR_DISTANCE {
            0.7 - (distance - MEDIUM_DISTANCE) / (FAR_DISTANCE - MEDIUM_DISTANCE) * 0.5
        } else {
            0.2 * (1.0 - (distance - FAR_DISTANCE) / 500.0).max(0.0)
        };

        // Update volume in audio engine
        // self.audio_engine.set_volume(handle, attenuation);
    }

    fn update_all_volumes(&self) {
        let master_volume = *self.master_volume.read();
        let category_volumes = self.category_volumes.read();

        for (handle, sound) in self.active_sounds.read().iter() {
            let category_volume = category_volumes.get(&sound.category).unwrap_or(&1.0);
            let final_volume = sound.volume * master_volume * category_volume;

            // Update in audio engine
            // self.audio_engine.set_volume(*handle, final_volume);
        }
    }

    fn update_category_volumes(&self, category: SoundCategory) {
        let master_volume = *self.master_volume.read();
        let category_volume = *self.category_volumes.read().get(&category).unwrap_or(&1.0);

        for (handle, sound) in self.active_sounds.read().iter() {
            if sound.category == category {
                let final_volume = sound.volume * master_volume * category_volume;

                // Update in audio engine
                // self.audio_engine.set_volume(*handle, final_volume);
            }
        }
    }

    fn update_spatial_audio(&self) {
        // Update spatial audio processor with current sounds
        for (handle, sound) in self.active_sounds.read().iter() {
            if let Some(position) = sound.position {
                // Update spatial source in processor
                // self.spatial_processor.update_source(*handle, position);
            }
        }
    }

    fn get_pool_hit_ratios(&self) -> HashMap<String, f32> {
        let pools = self.sound_pools.read();
        pools
            .iter()
            .map(|(name, pool)| (name.clone(), pool.hit_ratio()))
            .collect()
    }
}

/// Sound effect statistics
#[derive(Debug)]
pub struct SoundEffectStats {
    pub active_sounds: usize,
    pub total_sounds_played: usize,
    pub sounds_culled: usize,
    pub category_counts: HashMap<SoundCategory, usize>,
    pub pool_hit_ratios: HashMap<String, f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sound_category_defaults() {
        assert_eq!(SoundCategory::UI.default_priority(), AudioPriority::High);
        assert_eq!(SoundCategory::Weapons.max_simultaneous(), 16);
        assert!(SoundCategory::Weapons.supports_3d());
        assert!(!SoundCategory::UI.supports_3d());
    }

    #[test]
    fn test_sound_variation() {
        let mut variation = SoundVariation::default();
        variation.volume_variation = 0.2;
        variation.pitch_variation = 0.1;
        variation.play_probability = 0.8;

        let mut descriptor = SoundEffectDescriptor::new(
            "test".to_string(),
            SoundCategory::Weapons,
            "test.wav".to_string(),
        );
        descriptor.variation = variation;

        let mut rng = thread_rng();
        let (volume, pitch, _delay) = descriptor.apply_variation(&mut rng);

        assert!(volume >= 0.8 && volume <= 1.2); // Base 1.0 +/- 0.2
        assert!(pitch >= 0.9 && pitch <= 1.1); // Base 1.0 +/- 0.1
    }

    #[test]
    fn test_sound_pool() {
        let mut pool = SoundPool::new("test_pool".to_string(), 5);

        // Initially empty
        assert_eq!(pool.hit_ratio(), 0.0);

        // Preload some data
        pool.preload(vec![]); // Empty for test

        // Try to acquire (will fail since no data)
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn test_active_sound_effect() {
        let active_sound = ActiveSoundEffect::new(
            1,
            "test_sound".to_string(),
            SoundCategory::Weapons,
            AudioPriority::High,
        );

        assert_eq!(active_sound.handle, 1);
        assert_eq!(active_sound.category, SoundCategory::Weapons);
        assert_eq!(active_sound.priority, AudioPriority::High);
        assert!(active_sound.is_relevant(Duration::from_secs(10)));
    }
}
