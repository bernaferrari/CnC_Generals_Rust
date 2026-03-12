//! Sound Effect Manager
//! 
//! This module provides comprehensive sound effect management for the game,
//! including 2D and 3D sound effects, sound pooling, priority management,
//! and integration with the BIG file system for loading C&C audio assets.

use std::collections::{HashMap, VecDeque, BTreeMap};
use std::sync::{Arc, RwLock, Mutex};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::thread;
use std::sync::mpsc::{self, Sender, Receiver};

use rodio::{Decoder, OutputStream, OutputStreamHandle, Source, Sink, SpatialSink};
use symphonia::core::io::MediaSourceStream;

use crate::common::audio::{
    AudioEventRts, AudioEventInfo, AudioHandle, AudioType, AudioPriority,
    audio_3d::{Position3D, SpatialAudioProcessor, Audio3DParams},
    audio_cache::AudioFileCache,
    Coord3D, Real, Bool, Int, UnsignedInt, AsciiString,
};

/// Sound effect categories for organization and volume control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundCategory {
    UI,
    Weapons,
    Explosions,
    Vehicles,
    Infantry,
    Environment,
    Buildings,
    Aircraft,
    Music,
    Speech,
    Ambient,
    Custom(u8),
}

impl SoundCategory {
    /// Get default volume for this category
    pub fn default_volume(&self) -> Real {
        match self {
            SoundCategory::UI => 0.8,
            SoundCategory::Weapons => 0.9,
            SoundCategory::Explosions => 1.0,
            SoundCategory::Vehicles => 0.7,
            SoundCategory::Infantry => 0.6,
            SoundCategory::Environment => 0.5,
            SoundCategory::Buildings => 0.8,
            SoundCategory::Aircraft => 0.8,
            SoundCategory::Music => 0.7,
            SoundCategory::Speech => 0.9,
            SoundCategory::Ambient => 0.4,
            SoundCategory::Custom(_) => 0.5,
        }
    }

    /// Get priority modifier for this category
    pub fn priority_modifier(&self) -> Int {
        match self {
            SoundCategory::UI => 100,
            SoundCategory::Speech => 90,
            SoundCategory::Explosions => 80,
            SoundCategory::Weapons => 70,
            SoundCategory::Aircraft => 60,
            SoundCategory::Vehicles => 50,
            SoundCategory::Infantry => 40,
            SoundCategory::Buildings => 30,
            SoundCategory::Environment => 20,
            SoundCategory::Ambient => 10,
            SoundCategory::Music => 5,
            SoundCategory::Custom(_) => 25,
        }
    }
}

/// Sound variation types for adding randomness
#[derive(Debug, Clone)]
pub enum SoundVariation {
    /// Play sounds in sequence
    Sequential,
    /// Play sounds randomly
    Random,
    /// Play all sounds simultaneously
    Simultaneous,
    /// Play sounds based on weighted probability
    Weighted(Vec<Real>),
}

/// Sound effect descriptor defining how a sound behaves
#[derive(Debug, Clone)]
pub struct SoundEffectDescriptor {
    pub id: String,
    pub category: SoundCategory,
    pub file_paths: Vec<PathBuf>,
    pub variation: SoundVariation,
    pub base_volume: Real,
    pub volume_variance: Real, // ±variance for randomization
    pub pitch_variance: Real,  // ±variance for randomization
    pub base_priority: AudioPriority,
    pub max_instances: usize,  // Maximum concurrent instances
    pub min_interval: Duration, // Minimum time between plays
    pub is_3d: Bool,
    pub min_distance: Real,
    pub max_distance: Real,
    pub rolloff_factor: Real,
    pub interrupt_priority: AudioPriority, // Priority needed to interrupt this sound
    pub fade_in_time: Duration,
    pub fade_out_time: Duration,
    pub loop_count: Int, // 0 = infinite, 1 = play once, >1 = loop N times
    pub preload: Bool,   // Whether to preload this sound into cache
}

impl SoundEffectDescriptor {
    pub fn new(id: String, category: SoundCategory, file_path: String) -> Self {
        Self {
            id,
            category,
            file_paths: vec![PathBuf::from(file_path)],
            variation: SoundVariation::Random,
            base_volume: category.default_volume(),
            volume_variance: 0.1,
            pitch_variance: 0.1,
            base_priority: AudioPriority::Normal,
            max_instances: 5,
            min_interval: Duration::from_millis(100),
            is_3d: false,
            min_distance: 1.0,
            max_distance: 1000.0,
            rolloff_factor: 1.0,
            interrupt_priority: AudioPriority::High,
            fade_in_time: Duration::ZERO,
            fade_out_time: Duration::from_millis(250),
            loop_count: 1,
            preload: false,
        }
    }

    pub fn with_3d(mut self, min_distance: Real, max_distance: Real) -> Self {
        self.is_3d = true;
        self.min_distance = min_distance;
        self.max_distance = max_distance;
        self
    }

    pub fn with_variations(mut self, file_paths: Vec<String>) -> Self {
        self.file_paths = file_paths.into_iter().map(PathBuf::from).collect();
        self
    }

    pub fn with_volume(mut self, volume: Real, variance: Real) -> Self {
        self.base_volume = volume.clamp(0.0, 1.0);
        self.volume_variance = variance.clamp(0.0, 1.0);
        self
    }

    pub fn with_pitch_variance(mut self, variance: Real) -> Self {
        self.pitch_variance = variance.clamp(0.0, 2.0);
        self
    }

    pub fn with_priority(mut self, priority: AudioPriority) -> Self {
        self.base_priority = priority;
        self
    }

    pub fn with_max_instances(mut self, max_instances: usize) -> Self {
        self.max_instances = max_instances.max(1);
        self
    }

    pub fn with_looping(mut self, loop_count: Int) -> Self {
        self.loop_count = loop_count;
        self
    }

    pub fn with_preload(mut self, preload: Bool) -> Self {
        self.preload = preload;
        self
    }
}

/// Active sound effect instance
#[derive(Debug)]
pub struct ActiveSoundEffect {
    pub handle: AudioHandle,
    pub descriptor_id: String,
    pub sink: Option<Arc<Mutex<Sink>>>,
    pub spatial_sink: Option<Arc<Mutex<SpatialSink>>>,
    pub start_time: Instant,
    pub position: Option<Position3D>,
    pub volume: Real,
    pub pitch: Real,
    pub priority: Int,
    pub fade_start: Option<Instant>,
    pub fade_duration: Duration,
    pub target_volume: Real,
    pub is_fading: Bool,
    pub loop_remaining: Int,
}

impl ActiveSoundEffect {
    pub fn new_2d(
        handle: AudioHandle,
        descriptor_id: String,
        sink: Sink,
        volume: Real,
        pitch: Real,
        priority: Int,
    ) -> Self {
        Self {
            handle,
            descriptor_id,
            sink: Some(Arc::new(Mutex::new(sink))),
            spatial_sink: None,
            start_time: Instant::now(),
            position: None,
            volume,
            pitch,
            priority,
            fade_start: None,
            fade_duration: Duration::ZERO,
            target_volume: volume,
            is_fading: false,
            loop_remaining: 1,
        }
    }

    pub fn new_3d(
        handle: AudioHandle,
        descriptor_id: String,
        spatial_sink: SpatialSink,
        position: Position3D,
        volume: Real,
        pitch: Real,
        priority: Int,
    ) -> Self {
        Self {
            handle,
            descriptor_id,
            sink: None,
            spatial_sink: Some(Arc::new(Mutex::new(spatial_sink))),
            start_time: Instant::now(),
            position: Some(position),
            volume,
            pitch,
            priority,
            fade_start: None,
            fade_duration: Duration::ZERO,
            target_volume: volume,
            is_fading: false,
            loop_remaining: 1,
        }
    }

    pub fn is_playing(&self) -> bool {
        if let Some(sink) = &self.sink {
            !sink.lock().unwrap().empty()
        } else if let Some(spatial_sink) = &self.spatial_sink {
            !spatial_sink.lock().unwrap().empty()
        } else {
            false
        }
    }

    pub fn stop(&self) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().stop();
        } else if let Some(spatial_sink) = &self.spatial_sink {
            spatial_sink.lock().unwrap().stop();
        }
    }

    pub fn pause(&self) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().pause();
        } else if let Some(spatial_sink) = &self.spatial_sink {
            spatial_sink.lock().unwrap().pause();
        }
    }

    pub fn resume(&self) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().play();
        } else if let Some(spatial_sink) = &self.spatial_sink {
            spatial_sink.lock().unwrap().play();
        }
    }

    pub fn set_volume(&self, volume: Real) {
        if let Some(sink) = &self.sink {
            sink.lock().unwrap().set_volume(volume);
        } else if let Some(spatial_sink) = &self.spatial_sink {
            spatial_sink.lock().unwrap().set_volume(volume);
        }
    }

    pub fn update_3d_position(&self, position: Position3D) {
        if let Some(spatial_sink) = &self.spatial_sink {
            let pos_array: [f32; 3] = position.into();
            spatial_sink.lock().unwrap().set_emitter_position(pos_array);
        }
    }

    pub fn start_fade(&mut self, target_volume: Real, duration: Duration) {
        self.fade_start = Some(Instant::now());
        self.fade_duration = duration;
        self.target_volume = target_volume;
        self.is_fading = true;
    }

    pub fn update_fade(&mut self) {
        if let Some(fade_start) = self.fade_start {
            let elapsed = fade_start.elapsed();
            if elapsed >= self.fade_duration {
                // Fade complete
                self.is_fading = false;
                self.fade_start = None;
                self.volume = self.target_volume;
                self.set_volume(self.volume);
                
                if self.target_volume == 0.0 {
                    self.stop();
                }
            } else {
                // Update fade progress
                let progress = elapsed.as_secs_f32() / self.fade_duration.as_secs_f32();
                let current_volume = self.volume + (self.target_volume - self.volume) * progress;
                self.set_volume(current_volume);
            }
        }
    }
}

/// Sound instance tracking and limiting
#[derive(Debug)]
struct SoundInstanceTracker {
    instances: HashMap<String, Vec<AudioHandle>>,
    last_play_time: HashMap<String, Instant>,
    handle_to_descriptor: HashMap<AudioHandle, String>,
}

impl SoundInstanceTracker {
    fn new() -> Self {
        Self {
            instances: HashMap::new(),
            last_play_time: HashMap::new(),
            handle_to_descriptor: HashMap::new(),
        }
    }

    fn can_play(&self, descriptor_id: &str, descriptor: &SoundEffectDescriptor) -> bool {
        // Check minimum interval
        if let Some(last_time) = self.last_play_time.get(descriptor_id) {
            if last_time.elapsed() < descriptor.min_interval {
                return false;
            }
        }

        // Check max instances
        if let Some(handles) = self.instances.get(descriptor_id) {
            handles.len() < descriptor.max_instances
        } else {
            true
        }
    }

    fn add_instance(&mut self, descriptor_id: String, handle: AudioHandle) {
        self.instances.entry(descriptor_id.clone()).or_insert_with(Vec::new).push(handle);
        self.last_play_time.insert(descriptor_id.clone(), Instant::now());
        self.handle_to_descriptor.insert(handle, descriptor_id);
    }

    fn remove_instance(&mut self, handle: AudioHandle) -> Option<String> {
        if let Some(descriptor_id) = self.handle_to_descriptor.remove(&handle) {
            if let Some(handles) = self.instances.get_mut(&descriptor_id) {
                handles.retain(|&h| h != handle);
                if handles.is_empty() {
                    self.instances.remove(&descriptor_id);
                }
            }
            Some(descriptor_id)
        } else {
            None
        }
    }

    fn get_instance_count(&self, descriptor_id: &str) -> usize {
        self.instances.get(descriptor_id).map(|v| v.len()).unwrap_or(0)
    }

    fn get_lowest_priority_handle(&self, descriptor_id: &str, active_sounds: &HashMap<AudioHandle, ActiveSoundEffect>) -> Option<AudioHandle> {
        if let Some(handles) = self.instances.get(descriptor_id) {
            handles.iter()
                .filter_map(|&handle| active_sounds.get(&handle).map(|sound| (handle, sound.priority)))
                .min_by_key(|(_, priority)| *priority)
                .map(|(handle, _)| handle)
        } else {
            None
        }
    }
}

/// Sound pool for managing audio resources efficiently
pub struct SoundPool {
    available_sinks: VecDeque<Sink>,
    available_spatial_sinks: VecDeque<SpatialSink>,
    stream_handle: Arc<OutputStreamHandle>,
    max_2d_sinks: usize,
    max_3d_sinks: usize,
}

impl SoundPool {
    pub fn new(stream_handle: Arc<OutputStreamHandle>, max_2d_sinks: usize, max_3d_sinks: usize) -> Self {
        let mut pool = Self {
            available_sinks: VecDeque::new(),
            available_spatial_sinks: VecDeque::new(),
            stream_handle,
            max_2d_sinks,
            max_3d_sinks,
        };

        // Pre-create some sinks for better performance
        pool.refill_pools();
        pool
    }

    pub fn get_2d_sink(&mut self) -> Result<Sink, String> {
        if let Some(sink) = self.available_sinks.pop_front() {
            Ok(sink)
        } else {
            Sink::try_new(&*self.stream_handle)
                .map_err(|e| format!("Failed to create 2D sink: {}", e))
        }
    }

    pub fn get_3d_sink(&mut self) -> Result<SpatialSink, String> {
        if let Some(sink) = self.available_spatial_sinks.pop_front() {
            Ok(sink)
        } else {
            SpatialSink::try_new(&*self.stream_handle, [0.0, 0.0, 0.0], [1.0, 0.0], [-1.0, 0.0])
                .map_err(|e| format!("Failed to create 3D sink: {}", e))
        }
    }

    pub fn return_2d_sink(&mut self, sink: Sink) {
        if self.available_sinks.len() < self.max_2d_sinks {
            // Clear any remaining audio and add to pool
            // Note: In practice, you'd want to ensure the sink is properly cleared
            self.available_sinks.push_back(sink);
        }
        // Otherwise, let it drop and be garbage collected
    }

    pub fn return_3d_sink(&mut self, sink: SpatialSink) {
        if self.available_spatial_sinks.len() < self.max_3d_sinks {
            // Clear any remaining audio and add to pool
            self.available_spatial_sinks.push_back(sink);
        }
        // Otherwise, let it drop and be garbage collected
    }

    fn refill_pools(&mut self) {
        // Pre-create some 2D sinks
        while self.available_sinks.len() < self.max_2d_sinks / 2 {
            if let Ok(sink) = Sink::try_new(&*self.stream_handle) {
                self.available_sinks.push_back(sink);
            } else {
                break;
            }
        }

        // Pre-create some 3D sinks
        while self.available_spatial_sinks.len() < self.max_3d_sinks / 2 {
            if let Ok(sink) = SpatialSink::try_new(&*self.stream_handle, [0.0, 0.0, 0.0], [1.0, 0.0], [-1.0, 0.0]) {
                self.available_spatial_sinks.push_back(sink);
            } else {
                break;
            }
        }
    }
}

/// Main Sound Effect Manager
pub struct SoundEffectManager {
    // Audio system components
    stream_handle: Arc<OutputStreamHandle>,
    audio_cache: Arc<AudioFileCache>,
    spatial_processor: Option<Arc<SpatialAudioProcessor>>,
    
    // Sound management
    descriptors: RwLock<HashMap<String, SoundEffectDescriptor>>,
    active_sounds: Arc<RwLock<HashMap<AudioHandle, ActiveSoundEffect>>>,
    instance_tracker: RwLock<SoundInstanceTracker>,
    sound_pool: Mutex<SoundPool>,
    
    // Handle generation
    next_handle: Mutex<AudioHandle>,
    
    // Volume controls
    master_volume: RwLock<Real>,
    category_volumes: RwLock<HashMap<SoundCategory, Real>>,
    
    // Listener position for 3D audio
    listener_position: RwLock<Position3D>,
    
    // Statistics
    stats: RwLock<SoundEffectStats>,
    
    // Configuration
    max_concurrent_sounds: usize,
    priority_culling_enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SoundEffectStats {
    pub active_sounds: usize,
    pub total_sounds_played: u64,
    pub sounds_culled: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub memory_used: usize,
    pub avg_latency_ms: Real,
}

impl SoundEffectManager {
    pub fn new(audio_cache: Arc<AudioFileCache>) -> Self {
        // This would normally get the stream handle from the audio system
        let (_, stream_handle) = OutputStream::try_default().expect("Failed to create output stream");
        let stream_handle = Arc::new(stream_handle);
        
        let mut category_volumes = HashMap::new();
        // Initialize default category volumes
        for &category in &[
            SoundCategory::UI, SoundCategory::Weapons, SoundCategory::Explosions,
            SoundCategory::Vehicles, SoundCategory::Infantry, SoundCategory::Environment,
            SoundCategory::Buildings, SoundCategory::Aircraft, SoundCategory::Music,
            SoundCategory::Speech, SoundCategory::Ambient,
        ] {
            category_volumes.insert(category, category.default_volume());
        }

        Self {
            stream_handle: stream_handle.clone(),
            audio_cache,
            spatial_processor: None,
            descriptors: RwLock::new(HashMap::new()),
            active_sounds: Arc::new(RwLock::new(HashMap::new())),
            instance_tracker: RwLock::new(SoundInstanceTracker::new()),
            sound_pool: Mutex::new(SoundPool::new(stream_handle, 32, 16)),
            next_handle: Mutex::new(50000), // Start sound effect handles at 50000
            master_volume: RwLock::new(1.0),
            category_volumes: RwLock::new(category_volumes),
            listener_position: RwLock::new(Position3D::zero()),
            stats: RwLock::new(SoundEffectStats::default()),
            max_concurrent_sounds: 128,
            priority_culling_enabled: true,
        }
    }

    /// Set spatial audio processor for 3D sound processing
    pub fn set_spatial_processor(&mut self, processor: Arc<SpatialAudioProcessor>) {
        self.spatial_processor = Some(processor);
    }

    /// Register sound effect descriptors
    pub fn register_sounds(&self, descriptors: Vec<SoundEffectDescriptor>) {
        let mut desc_map = self.descriptors.write().unwrap();
        
        for descriptor in descriptors {
            // Preload if requested
            if descriptor.preload {
                for file_path in &descriptor.file_paths {
                    // Try to preload into cache
                    if file_path.exists() {
                        self.audio_cache.preload_file(file_path);
                    }
                }
            }
            
            desc_map.insert(descriptor.id.clone(), descriptor);
        }
    }

    /// Register a single sound effect
    pub fn register_sound(&self, descriptor: SoundEffectDescriptor) {
        self.register_sounds(vec![descriptor]);
    }

    /// Play a sound effect
    pub fn play_sound(&self, sound_id: &str, position: Option<Position3D>, volume_override: Real) 
        -> Result<AudioHandle, Box<dyn std::error::Error>> 
    {
        let descriptors = self.descriptors.read().unwrap();
        let descriptor = descriptors.get(sound_id)
            .ok_or_else(|| format!("Sound descriptor '{}' not found", sound_id))?
            .clone();
        drop(descriptors);

        // Check if we can play this sound
        let instance_tracker = self.instance_tracker.read().unwrap();
        if !instance_tracker.can_play(sound_id, &descriptor) {
            return Err("Sound cannot be played due to instance limits or timing constraints".into());
        }
        drop(instance_tracker);

        // Check concurrent sound limits
        let active_sounds = self.active_sounds.read().unwrap();
        if active_sounds.len() >= self.max_concurrent_sounds {
            if self.priority_culling_enabled {
                // Try to cull lower priority sounds
                drop(active_sounds);
                self.cull_low_priority_sounds(descriptor.base_priority as Int + descriptor.category.priority_modifier())?;
            } else {
                return Err("Too many concurrent sounds playing".into());
            }
        } else {
            drop(active_sounds);
        }

        // Generate audio handle
        let handle = {
            let mut next_handle = self.next_handle.lock().unwrap();
            let handle = *next_handle;
            *next_handle += 1;
            handle
        };

        // Calculate final volume
        let master_volume = *self.master_volume.read().unwrap();
        let category_volumes = self.category_volumes.read().unwrap();
        let category_volume = category_volumes.get(&descriptor.category).copied().unwrap_or(1.0);
        
        let randomized_volume = descriptor.base_volume + 
            (rand::random::<Real>() - 0.5) * 2.0 * descriptor.volume_variance;
        let final_volume = (volume_override * master_volume * category_volume * randomized_volume).clamp(0.0, 1.0);

        // Calculate pitch with randomization
        let randomized_pitch = 1.0 + (rand::random::<Real>() - 0.5) * 2.0 * descriptor.pitch_variance;
        let final_pitch = randomized_pitch.clamp(0.1, 3.0);

        // Select file to play
        let file_path = self.select_file_variation(&descriptor)?;
        
        // Load audio data
        let audio_event = AudioEventRts::with_event_name(sound_id);
        let audio_data = self.audio_cache.open_file(&audio_event)
            .ok_or("Failed to load audio file from cache")?;

        // Create audio source
        let cursor = std::io::Cursor::new((*audio_data).clone());
        let source = Decoder::new(cursor)
            .map_err(|e| format!("Failed to decode audio: {}", e))?;

        // Handle looping
        let final_source = if descriptor.loop_count == 0 {
            // Infinite loop
            Box::new(source.repeat_infinite()) as Box<dyn Source<Item=f32> + Send>
        } else if descriptor.loop_count > 1 {
            // Finite loop
            Box::new(source.take_duration(Duration::from_secs_f32(10.0 * descriptor.loop_count as f32))) // Simplified
        } else {
            // Play once
            Box::new(source)
        };

        let priority = descriptor.base_priority as Int + descriptor.category.priority_modifier();

        // Create and play sound
        let active_sound = if let Some(position) = position {
            // 3D sound
            if !descriptor.is_3d {
                return Err("Trying to play non-3D sound with 3D position".into());
            }

            let mut sound_pool = self.sound_pool.lock().unwrap();
            let mut spatial_sink = sound_pool.get_3d_sink()?;
            
            // Set 3D properties
            let pos_array: [f32; 3] = position.into();
            spatial_sink.set_emitter_position(pos_array);
            spatial_sink.set_left_ear_position([-0.1, 0.0, 0.0]);
            spatial_sink.set_right_ear_position([0.1, 0.0, 0.0]);
            
            // Apply volume and pitch
            spatial_sink.set_volume(final_volume);
            // Note: rodio's SpatialSink doesn't support pitch adjustment directly
            
            spatial_sink.append(final_source);
            spatial_sink.play();

            ActiveSoundEffect::new_3d(handle, sound_id.to_string(), spatial_sink, position, final_volume, final_pitch, priority)
        } else {
            // 2D sound
            let mut sound_pool = self.sound_pool.lock().unwrap();
            let sink = sound_pool.get_2d_sink()?;
            
            sink.set_volume(final_volume);
            sink.append(final_source);
            sink.play();

            ActiveSoundEffect::new_2d(handle, sound_id.to_string(), sink, final_volume, final_pitch, priority)
        };

        // Add to tracking
        {
            let mut instance_tracker = self.instance_tracker.write().unwrap();
            instance_tracker.add_instance(sound_id.to_string(), handle);
        }

        {
            let mut active_sounds = self.active_sounds.write().unwrap();
            active_sounds.insert(handle, active_sound);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_sounds_played += 1;
            stats.active_sounds = active_sounds.len();
        }

        Ok(handle)
    }

    /// Stop a playing sound
    pub fn stop_sound(&self, handle: AudioHandle, fade_out: bool) -> bool {
        let mut active_sounds = self.active_sounds.write().unwrap();
        
        if let Some(sound) = active_sounds.get_mut(&handle) {
            if fade_out {
                let descriptors = self.descriptors.read().unwrap();
                let fade_duration = descriptors.get(&sound.descriptor_id)
                    .map(|d| d.fade_out_time)
                    .unwrap_or(Duration::from_millis(250));
                drop(descriptors);
                
                sound.start_fade(0.0, fade_duration);
            } else {
                sound.stop();
                self.cleanup_finished_sound(handle, &mut active_sounds);
            }
            true
        } else {
            false
        }
    }

    /// Pause a playing sound
    pub fn pause_sound(&self, handle: AudioHandle) -> bool {
        let active_sounds = self.active_sounds.read().unwrap();
        if let Some(sound) = active_sounds.get(&handle) {
            sound.pause();
            true
        } else {
            false
        }
    }

    /// Resume a paused sound
    pub fn resume_sound(&self, handle: AudioHandle) -> bool {
        let active_sounds = self.active_sounds.read().unwrap();
        if let Some(sound) = active_sounds.get(&handle) {
            sound.resume();
            true
        } else {
            false
        }
    }

    /// Set master volume for all sound effects
    pub fn set_master_volume(&self, volume: Real) {
        let clamped_volume = volume.clamp(0.0, 1.0);
        *self.master_volume.write().unwrap() = clamped_volume;
        
        // Update all active sounds
        let active_sounds = self.active_sounds.read().unwrap();
        for sound in active_sounds.values() {
            sound.set_volume(sound.volume * clamped_volume);
        }
    }

    /// Set volume for a specific sound category
    pub fn set_category_volume(&self, category: SoundCategory, volume: Real) {
        let clamped_volume = volume.clamp(0.0, 1.0);
        {
            let mut category_volumes = self.category_volumes.write().unwrap();
            category_volumes.insert(category, clamped_volume);
        }

        // Update active sounds in this category
        let descriptors = self.descriptors.read().unwrap();
        let active_sounds = self.active_sounds.read().unwrap();
        
        for sound in active_sounds.values() {
            if let Some(descriptor) = descriptors.get(&sound.descriptor_id) {
                if descriptor.category == category {
                    let master_volume = *self.master_volume.read().unwrap();
                    sound.set_volume(sound.volume * master_volume * clamped_volume);
                }
            }
        }
    }

    /// Update listener position for 3D audio
    pub fn set_listener_position(&self, position: Position3D) {
        *self.listener_position.write().unwrap() = position;
        
        // Update 3D sounds with new listener position
        if let Some(spatial_processor) = &self.spatial_processor {
            let active_sounds = self.active_sounds.read().unwrap();
            
            for sound in active_sounds.values() {
                if let Some(sound_position) = sound.position {
                    if let Some(params) = spatial_processor.calculate_3d_audio_params(sound.handle) {
                        sound.set_volume(sound.volume * params.volume);
                        // In a full implementation, you'd also apply pitch, reverb, etc.
                    }
                }
            }
        }
    }

    /// Update the sound effect manager (call each frame)
    pub fn update(&self) {
        self.update_fading_sounds();
        self.cleanup_finished_sounds();
        self.update_3d_sounds();
        self.update_statistics();
    }

    /// Get current statistics
    pub fn get_stats(&self) -> SoundEffectStats {
        self.stats.read().unwrap().clone()
    }

    /// Stop all sounds in a category
    pub fn stop_category(&self, category: SoundCategory, fade_out: bool) {
        let descriptors = self.descriptors.read().unwrap();
        let active_sounds = self.active_sounds.read().unwrap();
        
        let handles_to_stop: Vec<AudioHandle> = active_sounds.iter()
            .filter(|(_, sound)| {
                descriptors.get(&sound.descriptor_id)
                    .map(|d| d.category == category)
                    .unwrap_or(false)
            })
            .map(|(&handle, _)| handle)
            .collect();
        
        drop(active_sounds);
        drop(descriptors);
        
        for handle in handles_to_stop {
            self.stop_sound(handle, fade_out);
        }
    }

    /// Stop all sounds
    pub fn stop_all_sounds(&self, fade_out: bool) {
        let active_sounds = self.active_sounds.read().unwrap();
        let handles: Vec<AudioHandle> = active_sounds.keys().cloned().collect();
        drop(active_sounds);
        
        for handle in handles {
            self.stop_sound(handle, fade_out);
        }
    }

    /// Get the number of active sounds
    pub fn get_active_sound_count(&self) -> usize {
        self.active_sounds.read().unwrap().len()
    }

    /// Check if a specific sound is playing
    pub fn is_sound_playing(&self, handle: AudioHandle) -> bool {
        let active_sounds = self.active_sounds.read().unwrap();
        active_sounds.get(&handle)
            .map(|sound| sound.is_playing())
            .unwrap_or(false)
    }
}

// Private implementation methods
impl SoundEffectManager {
    fn select_file_variation(&self, descriptor: &SoundEffectDescriptor) -> Result<&Path, String> {
        if descriptor.file_paths.is_empty() {
            return Err("No file paths in sound descriptor".to_string());
        }

        let index = match &descriptor.variation {
            SoundVariation::Sequential => {
                // This would need to track sequence state per descriptor
                0 // Simplified
            }
            SoundVariation::Random => {
                rand::random::<usize>() % descriptor.file_paths.len()
            }
            SoundVariation::Simultaneous => {
                // This would need to play all files - for now just return first
                0
            }
            SoundVariation::Weighted(weights) => {
                if weights.len() != descriptor.file_paths.len() {
                    0 // Fallback if weights don't match
                } else {
                    // Select based on weighted probability
                    let total_weight: Real = weights.iter().sum();
                    let mut random = rand::random::<Real>() * total_weight;
                    
                    for (i, &weight) in weights.iter().enumerate() {
                        random -= weight;
                        if random <= 0.0 {
                            return Ok(&descriptor.file_paths[i]);
                        }
                    }
                    0 // Fallback
                }
            }
        };

        Ok(&descriptor.file_paths[index])
    }

    fn cull_low_priority_sounds(&self, required_priority: Int) -> Result<(), Box<dyn std::error::Error>> {
        let mut active_sounds = self.active_sounds.write().unwrap();
        let mut sounds_to_remove = Vec::new();

        // Find sounds with lower priority
        for (&handle, sound) in active_sounds.iter() {
            if sound.priority < required_priority {
                sounds_to_remove.push(handle);
            }
        }

        if sounds_to_remove.is_empty() {
            return Err("Cannot cull any sounds - all have equal or higher priority".into());
        }

        // Sort by priority (lowest first) and remove some
        sounds_to_remove.sort_by_key(|&handle| active_sounds[&handle].priority);
        
        // Remove up to half of the lower priority sounds
        let to_remove = sounds_to_remove.len().min(10); // Don't remove too many at once
        
        for &handle in &sounds_to_remove[..to_remove] {
            if let Some(sound) = active_sounds.remove(&handle) {
                sound.stop();
                self.instance_tracker.write().unwrap().remove_instance(handle);
                
                let mut stats = self.stats.write().unwrap();
                stats.sounds_culled += 1;
            }
        }

        Ok(())
    }

    fn update_fading_sounds(&self) {
        let mut active_sounds = self.active_sounds.write().unwrap();
        let mut finished_sounds = Vec::new();

        for (&handle, sound) in active_sounds.iter_mut() {
            if sound.is_fading {
                sound.update_fade();
                
                // Check if fade to zero is complete
                if sound.target_volume == 0.0 && !sound.is_fading {
                    finished_sounds.push(handle);
                }
            }
        }

        // Remove sounds that finished fading out
        for handle in finished_sounds {
            self.cleanup_finished_sound(handle, &mut active_sounds);
        }
    }

    fn cleanup_finished_sounds(&self) {
        let mut active_sounds = self.active_sounds.write().unwrap();
        let mut finished_sounds = Vec::new();

        for (&handle, sound) in active_sounds.iter() {
            if !sound.is_playing() && !sound.is_fading {
                finished_sounds.push(handle);
            }
        }

        for handle in finished_sounds {
            self.cleanup_finished_sound(handle, &mut active_sounds);
        }
    }

    fn cleanup_finished_sound(&self, handle: AudioHandle, active_sounds: &mut HashMap<AudioHandle, ActiveSoundEffect>) {
        if let Some(sound) = active_sounds.remove(&handle) {
            // Return sink to pool if possible
            let mut sound_pool = self.sound_pool.lock().unwrap();
            
            if let Some(sink) = sound.sink {
                if let Ok(sink) = Arc::try_unwrap(sink) {
                    if let Ok(sink) = sink.into_inner() {
                        sound_pool.return_2d_sink(sink);
                    }
                }
            } else if let Some(spatial_sink) = sound.spatial_sink {
                if let Ok(spatial_sink) = Arc::try_unwrap(spatial_sink) {
                    if let Ok(spatial_sink) = spatial_sink.into_inner() {
                        sound_pool.return_3d_sink(spatial_sink);
                    }
                }
            }

            // Remove from instance tracker
            self.instance_tracker.write().unwrap().remove_instance(handle);
        }
    }

    fn update_3d_sounds(&self) {
        if let Some(spatial_processor) = &self.spatial_processor {
            let active_sounds = self.active_sounds.read().unwrap();
            
            for sound in active_sounds.values() {
                if let Some(position) = sound.position {
                    // Update position in spatial processor
                    spatial_processor.update_source_position(sound.handle, position);
                    
                    // Get updated 3D parameters
                    if let Some(params) = spatial_processor.calculate_3d_audio_params(sound.handle) {
                        // Apply volume changes
                        sound.set_volume(sound.volume * params.volume);
                        
                        // Update position in spatial sink
                        sound.update_3d_position(position);
                    }
                }
            }
        }
    }

    fn update_statistics(&self) {
        let active_count = self.active_sounds.read().unwrap().len();
        let (cache_size, _, cache_entries) = self.audio_cache.cache_info();
        
        let mut stats = self.stats.write().unwrap();
        stats.active_sounds = active_count;
        stats.memory_used = cache_size;
        // Other stats would be updated based on actual measurements
    }
}

/// Create a sound effect manager with default settings
pub fn create_sound_effect_manager() -> Result<SoundEffectManager, Box<dyn std::error::Error>> {
    use crate::common::audio::audio_cache::AudioFileCacheBuilder;
    
    let audio_cache = Arc::new(
        AudioFileCacheBuilder::new()
            .max_size(32 * 1024 * 1024) // 32 MB
            .add_search_path("./data/audio/sounds/")
            .add_search_path("./assets/audio/sounds/")
            .build()
    );
    
    Ok(SoundEffectManager::new(audio_cache))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sound_category_defaults() {
        assert_eq!(SoundCategory::UI.default_volume(), 0.8);
        assert_eq!(SoundCategory::Explosions.default_volume(), 1.0);
        assert!(SoundCategory::UI.priority_modifier() > SoundCategory::Ambient.priority_modifier());
    }

    #[test]
    fn test_sound_descriptor_builder() {
        let descriptor = SoundEffectDescriptor::new(
            "test_sound".to_string(),
            SoundCategory::Weapons,
            "weapon_fire.wav".to_string()
        )
        .with_3d(10.0, 500.0)
        .with_volume(0.8, 0.1)
        .with_priority(AudioPriority::High)
        .with_max_instances(3)
        .with_looping(0); // Infinite loop

        assert_eq!(descriptor.id, "test_sound");
        assert_eq!(descriptor.category, SoundCategory::Weapons);
        assert!(descriptor.is_3d);
        assert_eq!(descriptor.min_distance, 10.0);
        assert_eq!(descriptor.max_distance, 500.0);
        assert_eq!(descriptor.base_volume, 0.8);
        assert_eq!(descriptor.volume_variance, 0.1);
        assert_eq!(descriptor.base_priority, AudioPriority::High);
        assert_eq!(descriptor.max_instances, 3);
        assert_eq!(descriptor.loop_count, 0);
    }

    #[test]
    fn test_instance_tracker() {
        let mut tracker = SoundInstanceTracker::new();
        let descriptor = SoundEffectDescriptor::new(
            "test".to_string(),
            SoundCategory::UI,
            "test.wav".to_string()
        ).with_max_instances(2);

        // Should be able to play initially
        assert!(tracker.can_play("test", &descriptor));

        // Add instances
        tracker.add_instance("test".to_string(), 1001);
        tracker.add_instance("test".to_string(), 1002);
        
        assert_eq!(tracker.get_instance_count("test"), 2);
        
        // Should not be able to play more
        assert!(!tracker.can_play("test", &descriptor));

        // Remove one instance
        tracker.remove_instance(1001);
        assert_eq!(tracker.get_instance_count("test"), 1);
        
        // Should be able to play again
        assert!(tracker.can_play("test", &descriptor));
    }

    #[test]
    fn test_sound_variations() {
        let descriptor = SoundEffectDescriptor::new(
            "test".to_string(),
            SoundCategory::Weapons,
            "shot1.wav".to_string()
        ).with_variations(vec![
            "shot1.wav".to_string(),
            "shot2.wav".to_string(),
            "shot3.wav".to_string(),
        ]);

        assert_eq!(descriptor.file_paths.len(), 3);
        assert_eq!(descriptor.variation, SoundVariation::Random);
    }

    #[test]
    fn test_active_sound_effect() {
        // This test would need a real Sink, which requires audio hardware
        // In a real test environment, you'd mock the Sink or test in an environment with audio
    }

    #[test]
    fn test_position3d() {
        let pos = Position3D::new(10.0, 20.0, 30.0);
        let coord: Coord3D = pos.into();
        
        assert_eq!(coord.x, 10.0);
        assert_eq!(coord.y, 20.0);
        assert_eq!(coord.z, 30.0);

        let array: [f32; 3] = pos.into();
        assert_eq!(array, [10.0, 20.0, 30.0]);
    }
}