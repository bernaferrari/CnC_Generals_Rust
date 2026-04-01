//! Audio device management and hardware abstraction.
//!
//! This module provides the core device layer for WPAudio, handling:
//! - Audio device enumeration and selection  
//! - Device capability detection
//! - Hardware abstraction across platforms
//! - Device lifecycle management
//!
//! Based on the original MilesAudioManager system from C&C Generals.

use crate::handles::Sound2DHandle;
use crate::{
    backend::{BackendKind, BackendManager},
    cache::{AudioCache as SourceCache, CacheConfig},
    error::{DeviceError, Result},
    formats::{AudioFormat, ChannelLayout},
    listener::Listener3D,
    logical::{
        list::{LogicalSoundRegistration, LogicalSoundRegistry},
        LogicalDefinitionManager, LogicalSoundFactory, LogicalSoundFactoryEntry,
        LogicalTypeDefinition,
    },
    logical_listener::LogicalListener,
    mixer::{
        AudioMixer, MixBuffer, MixRenderStats, MixerConfig, MixerEvent, MixerTimelineSnapshot,
        VoicePlaybackState, VoiceStopReason,
    },
    output::CpalOutput,
    save_load::{
        DynamicAudioSaveLoad, SavedMixerVoiceRecord, SavedSoundRecord, StaticAudioSaveLoad,
    },
    sound3d::Sound3D,
    sound_pseudo3d::SoundPseudo3D,
    sound_scene::{LogicalTrigger, SceneSound},
    sound_scene_obj::SoundObjectId,
    thread_pool::queue_delayed_release,
    wwaudio::DriverType3D,
    wwaudio_handles::{make_2d_handle, make_3d_handle, make_stream_handle, WWHandle},
    AudioSource, Priority, SoundState,
};
use directories::ProjectDirs;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Maximum number of audio providers supported
const MAX_PROVIDERS: usize = 64;

/// Maximum number of 2D drivers tracked for preference management
const MAX_DRIVERS_2D: usize = 16;

/// Callback invoked when a playback channel reaches its end-of-stream
pub type EndOfStreamCallback =
    std::sync::Arc<dyn Fn(u32, Option<&crate::AudioSource>) + Send + Sync>;

/// Callback invoked when a textual event is emitted by the audio system
pub type TextEventCallback = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

/// Abstraction for supplying audio files from custom sources (databases, archives, etc.)
pub trait AudioFileFactory: Send + Sync {
    fn open(&self, identifier: &str) -> Result<Vec<u8>>;
}

/// Audio system types matching the original C++ implementation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayingAudioType {
    Sample,
    Sample3D,
    Stream,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayingStatus {
    Playing,
    Stopped,
    Paused,
}

/// Playing audio structure - matches C++ PlayingAudio
#[derive(Debug, Clone)]
pub struct PlayingAudio {
    pub audio_type: PlayingAudioType,
    pub status: PlayingStatus,
    pub handle: Option<u32>, // Simplified handle representation
    pub request_stop: bool,
    pub frames_faded: i32,
}

impl Default for PlayingAudio {
    fn default() -> Self {
        Self {
            audio_type: PlayingAudioType::Invalid,
            status: PlayingStatus::Stopped,
            handle: None,
            request_stop: false,
            frames_faded: 0,
        }
    }
}

/// Provider information - matches C++ ProviderInfo
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: String,
    pub id: u32,
    pub driver_type: u32,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AudioPreferences {
    pub device_name: Option<String>,
    pub preferred_3d_provider: Option<String>,
    pub preferred_2d_driver: Option<String>,
    pub stereo: bool,
    pub bits: u16,
    pub hertz: u32,
    pub sound_enabled: bool,
    pub music_enabled: bool,
    pub sound_volume: f32,
    pub music_volume: f32,
}

impl Default for ProviderInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            id: 0,
            driver_type: DriverType3D::Error as u32,
            is_valid: false,
        }
    }
}

/// Open audio file information - matches C++ OpenAudioFile
pub struct OpenAudioFile {
    pub file_data: Vec<u8>,
    pub open_count: u32,
    pub file_size: u32,
    pub compressed: bool,
}

/// Main audio system interface - based on MilesAudioManager
pub struct AudioSystem {
    devices: Vec<DeviceInfo>,
    config: crate::AudioSystemConfig,

    // Provider management
    providers_3d: Vec<ProviderInfo>,
    provider_count: u32,
    selected_provider: u32,
    last_provider: u32,
    selected_speaker_type: u32,

    // 2D driver management
    driver_2d_list: [Driver2DInfo; MAX_DRIVERS_2D],
    driver_2d_count: u32,
    selected_driver_2d: Option<Driver2DKind>,

    // Preferred settings
    preferred_3d_provider: String,
    preferred_speaker: String,
    preferred_2d_driver: Option<String>,

    // Audio pools
    playing_sounds: Vec<PlayingAudio>,
    playing_3d_sounds: Vec<PlayingAudio>,
    playing_streams: Vec<PlayingAudio>,
    fading_audio: Vec<PlayingAudio>,
    stopped_audio: Vec<PlayingAudio>,

    // Cache and handles
    source_cache: SourceCache,
    num_2d_samples: u32,
    num_3d_samples: u32,
    num_streams: u32,

    // Device state
    is_initialized: bool,
    hardware_accelerated: bool,
    speaker_surround: bool,

    // Global playback state
    playback_rate: u32,
    playback_bits: u16,
    playback_stereo: bool,
    max_2d_samples: u32,
    max_3d_samples: u32,
    max_2d_buffer_size: usize,
    max_3d_buffer_size: usize,
    sound_volume: f32,
    music_volume: f32,
    sound_effects_enabled: bool,
    music_enabled: bool,
    reverb_level: f32,
    reverb_room_type: i32,

    // Scene graph
    sound_scene: crate::SoundScene,

    // Callback registries
    eos_callbacks: Vec<(EndOfStreamCallback, u32)>,
    text_callbacks: Vec<TextEventCallback>,

    // Logical type records
    logical_types: Vec<LogicalTypeRecord>,
    logical_definition_manager: LogicalDefinitionManager,
    logical_sound_factory: LogicalSoundFactory,
    logical_registry: LogicalSoundRegistry,

    // Backend management
    backend_manager: BackendManager,
    selected_backend: BackendKind,

    // Mixer backend
    mixer: Arc<AudioMixer>,
    fallback_buffer: MixBuffer,
    mix_stats: MixRenderStats,
    last_timeline_snapshot: Option<MixerTimelineSnapshot>,
    pending_scene_time_ms: f64,
    cpal_output: Option<CpalOutput>,

    // File factory hook
    file_factory: Option<Arc<dyn AudioFileFactory>>,
    static_save: StaticAudioSaveLoad,
    dynamic_save: DynamicAudioSaveLoad,
    pending_mixer_events: VecDeque<MixerEvent>,
    pending_logical_events: VecDeque<LogicalEvent>,
    pending_voice_restores: VecDeque<SavedMixerVoiceRecord>,
    next_channel_id: u32,
}

/// Audio device representation
pub struct AudioDevice {
    info: DeviceInfo,
    config: DeviceConfig,
    system: *mut AudioSystem, // Reference to parent system
}

// SAFETY: AudioDevice holds a non-owning reference to AudioSystem.
// Access is synchronized through the audio system's internal locking.
unsafe impl Send for AudioDevice {}
unsafe impl Sync for AudioDevice {}

/// Device information and capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub supported_formats: Vec<AudioFormat>,
    pub max_channels: usize,
    pub capabilities: DeviceCapabilities,
}

/// Device configuration
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub format: AudioFormat,
    pub buffer_size: usize,
    pub buffer_count: usize,
    pub low_latency: bool,
}

/// Device capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub hardware_mixing: bool,
    pub hardware_3d: bool,
    pub min_sample_rate: u32,
    pub max_sample_rate: u32,
    pub formats: Vec<String>,
}

/// 2D audio driver descriptor (mirrors legacy WWAudio semantics)
#[derive(Debug, Clone)]
pub struct Driver2DInfo {
    pub name: String,
    pub kind: Driver2DKind,
    pub preferred_format: Option<AudioFormat>,
    pub is_hardware_accelerated: bool,
}

impl Default for Driver2DInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: Driver2DKind::Unknown,
            preferred_format: None,
            is_hardware_accelerated: false,
        }
    }
}

/// Available families of 2D drivers the original engine supported
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver2DKind {
    DirectSound,
    WaveOut,
    Alsa,
    CoreAudio,
    Wasapi,
    Unknown,
}

/// Logical audio type record used by the definition system
#[derive(Debug, Clone)]
pub struct LogicalTypeRecord {
    pub display_name: String,
    pub type_id: i32,
}

#[derive(Debug, Clone)]
pub struct LogicalEvent {
    pub sound_id: SoundObjectId,
    pub listener_id: SoundObjectId,
    pub type_mask: u32,
    pub labels: Vec<String>,
}

fn preferences_path(key: &str) -> Option<PathBuf> {
    ProjectDirs::from("com", "Westwood", "WWAudio").map(|dirs| {
        let mut path = dirs.config_dir().to_path_buf();
        path.push("preferences");
        path.push(format!("{key}.json"));
        path
    })
}

fn ensure_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn write_preferences(path: &Path, prefs: &AudioPreferences) -> std::io::Result<()> {
    ensure_parent(path)?;
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, prefs)?;
    Ok(())
}

fn read_preferences(path: &Path) -> std::io::Result<AudioPreferences> {
    let file = File::open(path)?;
    Ok(serde_json::from_reader(file)?)
}

impl AudioSystem {
    /// Create a new audio system with default configuration
    pub async fn new() -> Result<Self> {
        Self::new_with_config(crate::AudioSystemConfig::default()).await
    }

    /// Create a new audio system with custom configuration - matches MilesAudioManager constructor
    pub async fn new_with_config(config: crate::AudioSystemConfig) -> Result<Self> {
        info!("Initializing audio system with Miles-compatible interface");

        let config_snapshot = config.clone();
        let cache_config = CacheConfig {
            max_size_bytes: config_snapshot.cache_size_bytes,
            max_items: config_snapshot.max_cache_items,
            block_size: config_snapshot.cache_block_size,
            preload_threshold: config_snapshot.cache_block_size.saturating_mul(4),
        };

        let playback_rate = u32::from(config_snapshot.default_format.sample_rate);
        let playback_bits = u8::from(config_snapshot.default_format.sample_width) as u16;
        let playback_stereo = config_snapshot.default_format.channels > 1;
        let mixer = Arc::new(AudioMixer::new(MixerConfig {
            sample_rate: playback_rate,
            channels: config_snapshot.default_format.channels,
            buffer_frames: config_snapshot.mixer_buffer_frames,
        }));

        let mut system = Self {
            devices: Vec::new(),
            config,
            providers_3d: vec![ProviderInfo::default(); MAX_PROVIDERS],
            provider_count: 0,
            selected_provider: 0,
            last_provider: 0,
            selected_speaker_type: 0,
            driver_2d_list: std::array::from_fn(|_| Driver2DInfo::default()),
            driver_2d_count: 0,
            selected_driver_2d: None,
            preferred_3d_provider: String::new(),
            preferred_speaker: String::new(),
            preferred_2d_driver: None,
            playing_sounds: Vec::new(),
            playing_3d_sounds: Vec::new(),
            playing_streams: Vec::new(),
            fading_audio: Vec::new(),
            stopped_audio: Vec::new(),
            source_cache: SourceCache::new(cache_config),
            num_2d_samples: 0,
            num_3d_samples: 0,
            num_streams: 0,
            is_initialized: false,
            hardware_accelerated: false,
            speaker_surround: false,
            playback_rate,
            playback_bits,
            playback_stereo,
            max_2d_samples: config_snapshot.max_2d_samples,
            max_3d_samples: config_snapshot.max_3d_samples,
            max_2d_buffer_size: config_snapshot.max_2d_buffer_bytes,
            max_3d_buffer_size: config_snapshot.max_3d_buffer_bytes,
            sound_volume: config_snapshot.default_sound_volume,
            music_volume: config_snapshot.default_music_volume,
            sound_effects_enabled: config_snapshot.sound_effects_enabled,
            music_enabled: config_snapshot.music_enabled,
            reverb_level: config_snapshot.default_reverb_level,
            reverb_room_type: config_snapshot.default_reverb_room_type,
            sound_scene: crate::SoundScene::new(),
            eos_callbacks: Vec::new(),
            text_callbacks: Vec::new(),
            logical_types: Vec::new(),
            backend_manager: BackendManager::new(config_snapshot.default_format),
            selected_backend: BackendKind::Software,
            mixer: Arc::clone(&mixer),
            fallback_buffer: MixBuffer::new(
                config_snapshot.default_format.channels,
                config_snapshot.mixer_buffer_frames,
                u32::from(config_snapshot.default_format.sample_rate),
            ),
            mix_stats: MixRenderStats::default(),
            last_timeline_snapshot: Some(mixer.timeline_snapshot()),
            pending_scene_time_ms: 0.0,
            cpal_output: None,
            file_factory: None,
            static_save: StaticAudioSaveLoad::default(),
            dynamic_save: DynamicAudioSaveLoad::default(),
            pending_mixer_events: VecDeque::new(),
            pending_logical_events: VecDeque::new(),
            pending_voice_restores: VecDeque::new(),
            next_channel_id: 1,
            logical_definition_manager: LogicalDefinitionManager::new(),
            logical_sound_factory: LogicalSoundFactory::new(),
            logical_registry: LogicalSoundRegistry::new(),
        };

        // Build provider list - matches C++ buildProviderList()
        system.build_provider_list().await?;
        system.build_driver_list();

        // Initialize audio device
        system.init().await?;

        Ok(system)
    }

    /// Initialize the audio system - matches C++ MilesAudioManager::init()
    pub async fn init(&mut self) -> Result<()> {
        info!("Initializing audio system core");

        // Reset all audio lists
        self.playing_sounds.clear();
        self.playing_3d_sounds.clear();
        self.playing_streams.clear();
        self.fading_audio.clear();
        self.stopped_audio.clear();

        // Reset counters
        self.num_2d_samples = 0;
        self.num_3d_samples = 0;
        self.num_streams = 0;
        self.next_channel_id = 1;
        self.pending_scene_time_ms = 0.0;
        self.last_timeline_snapshot = Some(self.mixer.timeline_snapshot());
        self.fallback_buffer = MixBuffer::new(
            self.config.default_format.channels,
            self.config.mixer_buffer_frames,
            u32::from(self.config.default_format.sample_rate),
        );

        self.ensure_output_stream()?;

        // Create default device if none exist
        if self.devices.is_empty() {
            self.create_default_device()?;
        }

        self.is_initialized = true;
        info!("Audio system initialized successfully");

        Ok(())
    }

    fn ensure_output_stream(&mut self) -> Result<()> {
        if self.cpal_output.is_none() {
            let output = CpalOutput::new(
                &self.config.default_format,
                self.config.mixer_buffer_frames,
                Arc::clone(&self.mixer),
            )?;
            self.cpal_output = Some(output);
        }
        Ok(())
    }

    pub(crate) fn allocate_channel_id(&mut self) -> u32 {
        let id = self.next_channel_id;
        self.next_channel_id = self.next_channel_id.wrapping_add(1).max(1);
        id
    }

    /// Shutdown audio system, freeing the active devices and cached state
    pub async fn shutdown(&mut self) {
        if !self.is_initialized {
            return;
        }

        self.close_2d_device();
        self.playing_sounds.clear();
        self.playing_3d_sounds.clear();
        self.playing_streams.clear();
        self.fading_audio.clear();
        self.stopped_audio.clear();
        self.sound_scene.flush_scene();
        if let Err(err) = self.source_cache.clear().await {
            warn!("Failed to clear audio cache during shutdown: {err:?}");
        }
        self.cpal_output = None;
        self.next_channel_id = 1;
        self.pending_scene_time_ms = 0.0;
        self.last_timeline_snapshot = Some(self.mixer.timeline_snapshot());
        self.fallback_buffer = MixBuffer::new(
            self.config.default_format.channels,
            self.config.mixer_buffer_frames,
            u32::from(self.config.default_format.sample_rate),
        );
        self.is_initialized = false;
    }

    /// Public wrapper matching WWAudio::Build_3D_Driver_List
    pub async fn build_3d_driver_list(&mut self) -> Result<()> {
        self.build_provider_list().await
    }

    /// Build list of available audio providers - matches C++ buildProviderList()
    async fn build_provider_list(&mut self) -> Result<()> {
        info!("Building audio provider list");

        let mut providers = Vec::new();
        for descriptor in self.backend_manager.providers() {
            if descriptor.capabilities.hardware_accelerated && !self.hardware_accelerated {
                continue;
            }

            providers.push(ProviderInfo {
                name: descriptor.name.to_string(),
                id: descriptor.provider_type as u32,
                driver_type: descriptor.provider_type as u32,
                is_valid: descriptor.capabilities.supports_3d,
            });
        }

        self.provider_count = providers.len() as u32;
        for slot in self.providers_3d.iter_mut() {
            *slot = ProviderInfo::default();
        }
        for (index, provider) in providers.into_iter().enumerate().take(MAX_PROVIDERS) {
            self.providers_3d[index] = provider;
        }
        if self.provider_count > 0 {
            self.selected_provider = self.selected_provider.min(self.provider_count - 1);
            if let Some(descriptor) = self
                .backend_manager
                .providers()
                .get(self.selected_provider as usize)
            {
                self.selected_backend = descriptor.kind;
            }
        }

        info!("Found {} audio providers", self.provider_count);
        Ok(())
    }

    /// Equivalent to WWAudio::Free_3D_Driver_List
    pub fn free_3d_driver_list(&mut self) {
        self.providers_3d.fill(ProviderInfo::default());
        self.provider_count = 0;
        self.selected_provider = 0;
        self.last_provider = 0;
    }

    /// Create a default audio device
    ///
    /// Matches C++ behavior of providing a fallback device
    /// when no specific hardware is available or detected
    fn create_default_device(&mut self) -> Result<()> {
        let device_info = DeviceInfo {
            id: "default".to_string(),
            name: "Default Audio Device".to_string(),
            is_default: true,
            supported_formats: vec![
                AudioFormat::default(),
                // Add common format variations
                AudioFormat {
                    sample_rate: crate::formats::SampleRate::Hz22050,
                    ..AudioFormat::default()
                },
                AudioFormat {
                    sample_rate: crate::formats::SampleRate::Hz48000,
                    ..AudioFormat::default()
                },
            ],
            max_channels: 8,
            capabilities: DeviceCapabilities {
                hardware_mixing: self.hardware_accelerated,
                hardware_3d: self.hardware_accelerated,
                min_sample_rate: 8000,
                max_sample_rate: 48000,
                formats: vec![
                    "PCM16".to_string(),
                    "PCM24".to_string(),
                    "PCM32".to_string(),
                ],
            },
        };

        self.devices.push(device_info);
        info!("Created default audio device");
        Ok(())
    }

    /// Check if audio system is properly initialized and ready for use
    ///
    /// Matches C++ WWAudio.cpp:2021 (Is_Disabled check)
    pub fn is_audio_initialized(&self) -> bool {
        self.is_initialized && self.cpal_output.is_some()
    }

    /// Get audio device information by ID
    pub fn get_device_info(&self, device_id: &str) -> Option<&DeviceInfo> {
        self.devices.iter().find(|d| d.id == device_id)
    }

    /// Get default audio device information
    pub fn get_default_device_info(&self) -> Option<&DeviceInfo> {
        self.devices
            .iter()
            .find(|d| d.is_default)
            .or_else(|| self.devices.first())
    }

    /// Enumerate available audio devices
    pub async fn enumerate_devices(&self) -> Result<&[DeviceInfo]> {
        debug!("Enumerating {} available audio devices", self.devices.len());
        Ok(&self.devices)
    }

    /// Open an audio device - matches C++ openDevice()
    pub async fn open_device(&self, device_id: Option<&str>) -> Result<AudioDevice> {
        let device_info = if let Some(id) = device_id {
            self.devices
                .iter()
                .find(|d| d.id == id)
                .ok_or_else(|| DeviceError::NotFound)
                .map_err(crate::error::Error::from)?
        } else {
            self.devices
                .iter()
                .find(|d| d.is_default)
                .or_else(|| self.devices.first())
                .ok_or_else(|| DeviceError::NotFound)
                .map_err(crate::error::Error::from)?
        };

        info!("Opening audio device: {}", device_info.name);

        Ok(AudioDevice {
            info: device_info.clone(),
            config: DeviceConfig::default(),
            system: self as *const _ as *mut _,
        })
    }

    /// Load an audio source from file, leveraging the internal cache for reuse
    pub async fn load_source(&self, path: &str) -> Result<AudioSource> {
        info!("Loading audio source from: {}", path);

        if let Some(cached) = self.source_cache.get(path).await? {
            return Ok((*cached).clone());
        }

        let source = AudioSource::from_file(path).await?;
        let hosted = Arc::new(source.clone());
        self.source_cache
            .put(path.to_string(), hosted, Priority::Normal)
            .await?;

        Ok(source)
    }

    /// Get provider count - matches C++ getProviderCount()
    pub fn get_provider_count(&self) -> u32 {
        self.provider_count
    }

    /// Get provider name - matches C++ getProviderName()
    pub fn get_provider_name(&self, provider_num: u32) -> String {
        if provider_num < self.provider_count {
            self.providers_3d[provider_num as usize].name.clone()
        } else {
            String::new()
        }
    }

    /// Select audio provider - matches C++ selectProvider()
    pub fn select_provider(&mut self, provider_index: u32) {
        self.select_3d_provider(provider_index);
    }

    /// Set hardware acceleration - matches C++ setHardwareAccelerated()
    pub fn set_hardware_accelerated(&mut self, accelerated: bool) {
        self.hardware_accelerated = accelerated;
        info!(
            "Hardware acceleration: {}",
            if accelerated { "enabled" } else { "disabled" }
        );
    }

    /// Set surround sound - matches C++ setSpeakerSurround()  
    pub fn set_speaker_surround(&mut self, surround: bool) {
        self.speaker_surround = surround;
        info!(
            "Surround sound: {}",
            if surround { "enabled" } else { "disabled" }
        );
    }

    /// Update audio system - matches C++ update()
    pub async fn update(&mut self) -> Result<()> {
        if !self.is_initialized {
            return Ok(());
        }

        self.restore_pending_mixer_voices().await;

        self.mixer.tick(Duration::from_millis(16));

        let mut snapshot = None;

        if let Some(output) = self.cpal_output.as_ref() {
            if let Some(metrics) = output.drain_metrics() {
                self.mix_stats = metrics.stats;
                snapshot = Some(metrics.snapshot);
            }
        }

        if snapshot.is_none() && self.cpal_output.is_none() {
            let desired_frames = self.config.mixer_buffer_frames;
            let desired_channels = self.config.default_format.channels;
            let desired_rate: u32 = self.config.default_format.sample_rate.into();

            if self.fallback_buffer.channels != desired_channels {
                self.fallback_buffer.channels = desired_channels;
            }
            if self.fallback_buffer.sample_rate != desired_rate {
                self.fallback_buffer.sample_rate = desired_rate;
            }
            if self.fallback_buffer.frames != desired_frames
                || self.fallback_buffer.data.len()
                    != desired_frames.saturating_mul(desired_channels as usize)
            {
                self.fallback_buffer.frames = desired_frames;
                self.fallback_buffer.data.resize(
                    desired_frames.saturating_mul(desired_channels as usize),
                    0.0,
                );
            }

            self.mix_stats = self.mixer.render_into(&mut self.fallback_buffer);
            snapshot = Some(self.mixer.timeline_snapshot());
        }

        if let Some(snapshot) = snapshot {
            if let Some(previous) = self.last_timeline_snapshot {
                let frame_delta = snapshot
                    .current_frame
                    .saturating_sub(previous.current_frame);
                if frame_delta > 0 && snapshot.sample_rate > 0 {
                    let delta_ms = (frame_delta as f64 / snapshot.sample_rate as f64) * 1000.0;
                    self.pending_scene_time_ms += delta_ms;
                }
            }

            self.last_timeline_snapshot = Some(snapshot);
        }

        self.pump_scene_updates();
        self.process_mixer_events();
        self.collect_logical_events();

        // Process playing audio lists - matches C++ processing functions
        self.process_playing_list();
        self.process_fading_list();
        self.process_stopped_list();

        Ok(())
    }

    /// Process playing audio list - matches C++ processPlayingList()
    fn process_playing_list(&mut self) {
        // Update status of currently playing sounds
        self.playing_sounds
            .retain(|audio| audio.status == PlayingStatus::Playing);
        self.playing_3d_sounds
            .retain(|audio| audio.status == PlayingStatus::Playing);
        self.playing_streams
            .retain(|audio| audio.status == PlayingStatus::Playing);
    }

    /// Frame update helper mirroring WWAudio::On_Frame_Update
    pub async fn on_frame_update(&mut self, milliseconds: u32) -> Result<()> {
        self.pending_scene_time_ms += f64::from(milliseconds);
        self.update().await
    }

    /// Process fading audio list - matches C++ processFadingList()
    fn process_fading_list(&mut self) {
        for audio in &mut self.fading_audio {
            audio.frames_faded += 1;

            // If fading is complete, move to stopped list
            if audio.frames_faded >= 30 {
                // 30 frames to complete fade
                audio.status = PlayingStatus::Stopped;
            }
        }

        // Move completed fades to stopped list
        let mut completed_fades = Vec::new();
        self.fading_audio.retain(|audio| {
            if audio.status == PlayingStatus::Stopped {
                completed_fades.push(audio.clone());
                false
            } else {
                true
            }
        });

        self.stopped_audio.extend(completed_fades);
    }

    /// Access the current system configuration used to bootstrap devices
    pub fn configuration(&self) -> &crate::AudioSystemConfig {
        &self.config
    }

    /// Update the preferred speaker name used for device enumeration
    pub fn set_preferred_speaker(&mut self, speaker: impl Into<String>) {
        self.preferred_speaker = speaker.into();
    }

    /// Retrieve the preferred speaker label
    pub fn preferred_speaker(&self) -> &str {
        &self.preferred_speaker
    }

    /// Update the preferred 3D audio provider label
    pub fn set_preferred_3d_provider(&mut self, provider: impl Into<String>) {
        self.preferred_3d_provider = provider.into();
    }

    /// Retrieve the preferred 3D audio provider name
    pub fn preferred_3d_provider(&self) -> &str {
        &self.preferred_3d_provider
    }

    /// Set the current speaker type identifier
    pub fn set_selected_speaker_type(&mut self, value: u32) {
        self.selected_speaker_type = value;
    }

    /// Get the currently selected speaker type identifier
    pub fn selected_speaker_type(&self) -> u32 {
        self.selected_speaker_type
    }

    /// Process stopped audio list - matches C++ processStoppedList()
    fn process_stopped_list(&mut self) {
        // Clean up stopped audio
        self.stopped_audio.clear();
    }

    fn process_mixer_events(&mut self) {
        for event in self.mixer.drain_events() {
            if let MixerEvent::VoiceStopped {
                descriptor, reason, ..
            } = &event
            {
                if *reason == VoiceStopReason::Completed {
                    self.emit_eos_callbacks(Some(descriptor.source.as_ref()));
                }
            }
            self.pending_mixer_events.push_back(event);
        }
    }

    fn collect_logical_events(&mut self) {
        let triggers: Vec<LogicalTrigger> = self.sound_scene.collect_logical_sounds(None);
        for trigger in triggers {
            let mut labels = Vec::new();
            if let Some(registration) = self.logical_registration(trigger.sound_id) {
                if let Some(display) = registration.display.as_ref() {
                    labels.push(display.clone());
                }
            }
            labels.extend(self.describe_logical_mask(trigger.type_mask));

            if labels.is_empty() {
                self.fire_text_event(&format!("logical_sound_{}", trigger.sound_id));
            } else {
                for label in &labels {
                    self.fire_text_event(label);
                }
            }
            self.pending_logical_events.push_back(LogicalEvent {
                sound_id: trigger.sound_id,
                listener_id: trigger.listener_id,
                type_mask: trigger.type_mask,
                labels,
            });
        }
    }

    fn pump_scene_updates(&mut self) {
        if self.pending_scene_time_ms < 1.0 {
            return;
        }

        let delta_ms = self.pending_scene_time_ms.floor().min(u32::MAX as f64) as u32;
        if delta_ms == 0 {
            return;
        }

        self.sound_scene.update(delta_ms);
        self.pending_scene_time_ms -= f64::from(delta_ms);
    }

    fn describe_logical_mask(&self, mask: u32) -> Vec<String> {
        if mask == 0 {
            return Vec::new();
        }

        let mut labels = Vec::new();
        for record in &self.logical_types {
            if record.type_id >= 0 {
                let bit = 1u32.checked_shl(record.type_id as u32).unwrap_or(0);
                if bit != 0 && (mask & bit) != 0 {
                    labels.push(record.display_name.clone());
                }
            }
        }

        labels
    }

    /// Enumerate available 3D providers by index
    pub fn provider_info(&self, index: u32) -> Option<&ProviderInfo> {
        self.providers_3d.get(index as usize)
    }

    /// Enumerate available 2D drivers
    pub fn driver_2d_info(&self, index: u32) -> Option<&Driver2DInfo> {
        if index >= self.driver_2d_count {
            return None;
        }
        self.driver_2d_list.get(index as usize)
    }

    pub fn current_2d_driver(&self) -> Option<&Driver2DInfo> {
        self.selected_driver_2d.and_then(|kind| {
            self.driver_2d_list
                .iter()
                .take(self.driver_2d_count as usize)
                .find(|info| info.kind == kind)
        })
    }

    pub fn current_3d_provider(&self) -> Option<&ProviderInfo> {
        self.providers_3d.get(self.selected_provider as usize)
    }

    pub fn selected_provider_index(&self) -> u32 {
        self.selected_provider
    }

    pub fn last_provider_index(&self) -> u32 {
        self.last_provider
    }

    /// Build the 2D driver table (cross-platform redesign of legacy Miles logic)
    pub fn build_driver_list(&mut self) {
        let mut count = 0u32;
        for info in self.driver_2d_list.iter_mut() {
            *info = Driver2DInfo::default();
        }

        for (index, descriptor) in self
            .backend_manager
            .drivers()
            .iter()
            .enumerate()
            .take(MAX_DRIVERS_2D)
        {
            self.driver_2d_list[index] = Driver2DInfo {
                name: descriptor.name.to_string(),
                kind: descriptor.driver_kind,
                preferred_format: descriptor
                    .preferred_format
                    .or(Some(self.config.default_format)),
                is_hardware_accelerated: descriptor.hardware_accelerated,
            };
            count += 1;
        }

        self.driver_2d_count = count;
        if self.driver_2d_count > 0 && self.selected_driver_2d.is_none() {
            self.selected_driver_2d = Some(self.driver_2d_list[0].kind);
            self.preferred_2d_driver = Some(self.driver_2d_list[0].name.clone());
        }
    }

    /// Select a 2D driver by index
    pub fn select_2d_driver(&mut self, index: u32) {
        if (index as usize) < MAX_DRIVERS_2D && index < self.driver_2d_count {
            self.selected_driver_2d = Some(self.driver_2d_list[index as usize].kind);
            self.preferred_2d_driver = Some(self.driver_2d_list[index as usize].name.clone());
            if let Some(descriptor) = self.backend_manager.drivers().get(index as usize) {
                self.selected_backend = descriptor.backend;
            }
        } else {
            warn!("Attempted to select unknown 2D driver index {index}");
        }
    }

    /// Select a 2D driver by name
    pub fn select_2d_driver_by_name(&mut self, name: &str) {
        if let Some((idx, _)) = self
            .driver_2d_list
            .iter()
            .enumerate()
            .take(self.driver_2d_count as usize)
            .find(|(_, info)| info.name.eq_ignore_ascii_case(name))
        {
            self.select_2d_driver(idx as u32);
        } else {
            warn!("2D driver '{name}' not found; keeping existing selection");
        }
    }

    /// Open a 2D device with the requested format
    ///
    /// Matches C++ WWAudio.cpp:178-241 (Open_2D_Device)
    /// Creates platform-specific playback device with fallback support
    pub fn open_2d_device_with_format(&mut self, format: &AudioFormat) -> Result<()> {
        info!("Opening 2D audio device with format: {:?}", format);

        // Store playback settings - matches C++ lines 185-187
        self.playback_rate = u32::from(format.sample_rate);
        self.playback_bits = u8::from(format.sample_width) as u16;
        self.playback_stereo = format.channels > 1;

        // Close existing device first - matches C++ line 196
        self.close_2d_device();

        // Try to recreate the output stream with new format
        // This replaces the Miles Sound System AIL_waveOutOpen call (C++ line 205)
        let recreate_result = CpalOutput::new(
            format,
            self.config.mixer_buffer_frames,
            Arc::clone(&self.mixer),
        );

        match recreate_result {
            Ok(output) => {
                self.cpal_output = Some(output);
                info!("Successfully opened 2D audio device");

                // Allocate handles for the device - matches C++ line 232
                // In the Rust version, channels are allocated on-demand rather than pre-allocated
                debug!("Audio device ready for channel allocation");
                Ok(())
            }
            Err(e) => {
                warn!("Failed to open audio device with requested format: {:?}", e);

                // Try fallback to default format - matches C++ lines 218-228 (WaveOut fallback)
                let default_format = AudioFormat::default();
                if format != &default_format {
                    info!("Attempting fallback to default audio format");
                    match CpalOutput::new(
                        &default_format,
                        self.config.mixer_buffer_frames,
                        Arc::clone(&self.mixer),
                    ) {
                        Ok(output) => {
                            self.cpal_output = Some(output);
                            self.playback_rate = u32::from(default_format.sample_rate);
                            self.playback_bits = u8::from(default_format.sample_width) as u16;
                            self.playback_stereo = default_format.channels > 1;
                            info!("Successfully opened audio device with fallback format");
                            Ok(())
                        }
                        Err(fallback_err) => {
                            warn!("Fallback format also failed: {:?}", fallback_err);
                            Err(e)
                        }
                    }
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Open the default 2D device     
    pub fn open_2d_device_default(&mut self) -> Result<()> {
        self.open_2d_device_with_format(&AudioFormat::default())
    }

    /// Close the current 2D device
    ///
    /// Matches C++ WWAudio.cpp:289-318 (Close_2D_Device)
    /// Releases platform audio resources and frees all associated handles
    pub fn close_2d_device(&mut self) {
        debug!("Closing 2D audio device");

        // Close the audio output stream - matches C++ lines 304-311 (AIL_waveOutClose)
        // This also stops all currently playing audio automatically when the output stream is dropped
        // matching C++ lines 298-299 (Remove_2D_Sound_Handles)
        if self.cpal_output.is_some() {
            self.cpal_output = None;
            debug!("Audio output stream closed, all voices stopped");
        }

        // Release sample handles - matches C++ line 299 (Release_2D_Handles)
        // In the original C++, this freed pre-allocated Miles Sound System sample handles
        // (lines 1327-1336 in WWAudio.cpp).
        // In Rust, channels are allocated on-demand and released when dropped,
        // so we reset the channel ID counter to reclaim the ID space.
        self.next_channel_id = 1;

        debug!("2D audio device resources released");
    }

    /// Select 3D provider by index (legacy parity)
    pub fn select_3d_provider(&mut self, provider_index: u32) {
        if provider_index < self.provider_count {
            self.last_provider = self.selected_provider;
            self.selected_provider = provider_index;
            if let Some(info) = self.providers_3d.get(provider_index as usize) {
                info!("Selected audio provider: {}", info.name);
                let provider_type = DriverType3D::from_raw(info.driver_type);
                if let Some(descriptor) = self.backend_manager.find_provider(provider_type) {
                    self.selected_backend = descriptor.kind;
                }
            }
        } else {
            warn!("Attempted to select invalid 3D provider index {provider_index}");
        }
    }

    /// Select 3D provider by name
    pub fn select_3d_provider_by_name(&mut self, name: &str) {
        if let Some(index) = self.find_3d_provider_by_name(name) {
            self.select_3d_provider(index);
        } else {
            warn!("3D provider '{name}' not found");
        }
    }

    pub fn find_3d_provider_by_type(&self, driver_type: DriverType3D) -> Option<u32> {
        let target = driver_type as u32;
        self.providers_3d
            .iter()
            .enumerate()
            .take(self.provider_count as usize)
            .find(|(_, info)| info.driver_type == target)
            .map(|(idx, _)| idx as u32)
    }

    pub fn emit_eos_callbacks(&self, source: Option<&crate::AudioSource>) {
        for (callback, user_param) in &self.eos_callbacks {
            callback(*user_param, source);
        }
    }

    pub fn select_3d_provider_by_type(&mut self, driver_type: DriverType3D) -> bool {
        if let Some(index) = self.find_3d_provider_by_type(driver_type) {
            self.select_3d_provider(index);
            true
        } else {
            false
        }
    }

    pub fn find_3d_provider_by_name(&self, name: &str) -> Option<u32> {
        self.providers_3d
            .iter()
            .enumerate()
            .take(self.provider_count as usize)
            .find(|(_, info)| info.name.eq_ignore_ascii_case(name))
            .map(|(idx, _)| idx as u32)
    }

    /// Set sound effects master volume (0.0 - 1.0)
    pub fn set_sound_effects_volume(&mut self, volume: f32) {
        self.sound_volume = volume.clamp(0.0, 1.0);
    }

    pub fn sound_effects_volume(&self) -> f32 {
        self.sound_volume
    }

    /// Set music master volume (0.0 - 1.0)
    pub fn set_music_volume(&mut self, volume: f32) {
        self.music_volume = volume.clamp(0.0, 1.0);
    }

    pub fn music_volume(&self) -> f32 {
        self.music_volume
    }

    pub fn allow_sound_effects(&mut self, enabled: bool) {
        self.sound_effects_enabled = enabled;
    }

    pub fn are_sound_effects_enabled(&self) -> bool {
        self.sound_effects_enabled
    }

    pub fn allow_music(&mut self, enabled: bool) {
        self.music_enabled = enabled;
    }

    pub fn is_music_enabled(&self) -> bool {
        self.music_enabled
    }

    /// Configure reverb parameters (mirrors C++ Set_Reverb_Room_Type)
    pub fn set_reverb_room_type(&mut self, room_type: i32) {
        self.reverb_room_type = room_type;
    }

    pub fn reverb_room_type(&self) -> i32 {
        self.reverb_room_type
    }

    pub fn set_reverb_level(&mut self, level: f32) {
        self.reverb_level = level.clamp(0.0, 1.0);
    }

    pub fn reverb_level(&self) -> f32 {
        self.reverb_level
    }

    /// Access the world audio scene graph
    pub fn sound_scene(&self) -> &crate::SoundScene {
        &self.sound_scene
    }

    pub fn sound_scene_mut(&mut self) -> &mut crate::SoundScene {
        &mut self.sound_scene
    }

    /// Save the static audio state to a writer
    pub fn save_static_state<W: Write + Seek>(&mut self, writer: W) -> Result<()> {
        self.static_save.save(&self.sound_scene, writer)
    }

    /// Save the dynamic audio state to a writer
    pub fn save_dynamic_state<W: Write + Seek>(&mut self, writer: W) -> Result<()> {
        let snapshot = self.mixer.timeline_snapshot();
        self.dynamic_save.set_mixer_snapshot(snapshot);
        let voice_snapshot = self.mixer.voice_snapshot();
        let handle_to_sound: HashMap<u32, SoundObjectId> = self
            .sound_scene
            .dynamic_sounds
            .iter()
            .chain(self.sound_scene.static_sounds.iter())
            .filter_map(|sound| sound.miles_handle().map(|handle| (handle, sound.id())))
            .collect();
        let mut voice_records = Vec::with_capacity(voice_snapshot.len());
        for (handle, descriptor) in voice_snapshot {
            let timeline = self.mixer.voice_timeline(handle);
            let sound_id = descriptor
                .handle_id
                .and_then(|id| handle_to_sound.get(&id).copied());
            let record = SavedMixerVoiceRecord::from_mixer(handle, &descriptor, timeline, sound_id);
            voice_records.push(record);
        }
        self.dynamic_save.save(
            &self.sound_scene,
            &self.logical_registry,
            &voice_records,
            writer,
        )
    }

    /// Load static audio state
    pub fn load_static_state<R: Read + Seek>(&mut self, reader: R) -> Result<()> {
        self.static_save.load(reader)?;
        self.apply_static_records();
        Ok(())
    }

    /// Load dynamic audio state
    pub fn load_dynamic_state<R: Read + Seek>(&mut self, reader: R) -> Result<()> {
        self.dynamic_save.load(reader)?;
        self.apply_dynamic_records();
        self.queue_pending_voice_restores();
        Ok(())
    }

    fn instantiate_scene_sound(record: &SavedSoundRecord) -> crate::SceneSound {
        record.instantiate()
    }

    fn apply_static_records(&mut self) {
        self.sound_scene.static_sounds.clear();
        for record in self.static_save.loaded_sounds() {
            let sound = Self::instantiate_scene_sound(record);
            self.sound_scene.add_static_sound(sound);
        }
        self.recalculate_sample_counts();
    }

    fn apply_dynamic_records(&mut self) {
        self.sound_scene.dynamic_sounds.clear();
        for record in self.dynamic_save.loaded_dynamic_sounds() {
            let sound = Self::instantiate_scene_sound(record);
            self.sound_scene.add_dynamic_sound(sound);
        }
        let logical_records = self.dynamic_save.loaded_logical_sounds().to_vec();
        self.sound_scene.logical_sounds.clear();
        self.logical_registry.clear();
        for record in &logical_records {
            let logical = record.instantiate();
            self.sound_scene.add_logical_sound(logical);
        }
        for record in logical_records {
            self.register_logical_sound_factory_entry(
                record.id,
                record.type_mask,
                record.display_name.clone(),
            );
        }
        LogicalListener::set_global_scale(self.dynamic_save.logical_listener_global_scale());
        if let Some(snapshot) = self.dynamic_save.mixer_snapshot() {
            self.mixer.restore_timeline(snapshot);
            self.last_timeline_snapshot = Some(snapshot);
            self.pending_scene_time_ms = 0.0;
        }
        self.recalculate_sample_counts();
        self.sound_scene.update(0);
    }

    pub(crate) fn queue_pending_voice_restores(&mut self) {
        self.pending_voice_restores = self
            .dynamic_save
            .loaded_mixer_voices()
            .iter()
            .cloned()
            .collect();
    }

    async fn restore_pending_mixer_voices(&mut self) {
        if self.pending_voice_restores.is_empty() {
            return;
        }

        let mut remaining = VecDeque::new();
        let mut restored_any = false;

        while let Some(record) = self.pending_voice_restores.pop_front() {
            match self.restore_voice_record(record.clone()).await {
                Ok(true) => restored_any = true,
                Ok(false) => remaining.push_back(record),
                Err(err) => {
                    warn!("Mixer restore: failed to restore voice: {err}");
                }
            }
        }

        self.pending_voice_restores = remaining;

        if restored_any {
            self.recalculate_sample_counts();
            self.sound_scene.update(0);
        }
    }

    async fn restore_voice_record(&mut self, record: SavedMixerVoiceRecord) -> Result<bool> {
        if matches!(record.playback_state, VoicePlaybackState::Completed) {
            return Ok(true);
        }

        let sound_id = match record.sound_id {
            Some(id) => id,
            None => {
                warn!(
                    "Mixer restore: skipped voice {:?} without sound id",
                    record.handle
                );
                return Ok(true);
            }
        };

        if self.sound_scene.find_sound(sound_id).is_none() {
            warn!(
                "Mixer restore: sound {} not present in scene, skipping voice {:?}",
                sound_id, record.handle
            );
            return Ok(true);
        }

        let source_arc = match record.source_identifier.as_deref() {
            Some("<memory>") => {
                if let Some(memory) = record.memory_source.as_ref() {
                    let format = AudioFormat {
                        channels: memory.channels,
                        sample_rate: sample_rate_from_u32(memory.sample_rate),
                        sample_width: sample_width_from_u16(memory.sample_width),
                        channel_layout: channel_layout_from_channels(memory.channels),
                    };
                    match AudioSource::from_memory(memory.data.clone(), format) {
                        Ok(source) => Arc::new(source),
                        Err(err) => {
                            warn!("Mixer restore: unable to rebuild memory source: {err}");
                            return Ok(true);
                        }
                    }
                } else {
                    warn!(
                        "Mixer restore: memory voice {:?} missing data",
                        record.handle
                    );
                    return Ok(true);
                }
            }
            Some(path) if !path.is_empty() => match self.load_source(path).await {
                Ok(source) => Arc::new(source),
                Err(err) => {
                    warn!("Mixer restore: unable to load source {}: {err}", path);
                    return Ok(true);
                }
            },
            _ => {
                warn!(
                    "Mixer restore: voice {:?} missing source identifier",
                    record.handle
                );
                return Ok(true);
            }
        };

        let device = match self.open_device(None).await {
            Ok(device) => device,
            Err(err) => {
                warn!("Mixer restore: no audio device available: {err}");
                return Ok(false);
            }
        };
        let mut channel = match device.create_channel(Priority::Normal) {
            Ok(channel) => channel,
            Err(err) => {
                warn!("Mixer restore: failed to create audio channel: {err}");
                return Ok(false);
            }
        };
        channel.set_handle_id(record.miles_handle_id);

        let listener_snapshot = self.sound_scene.listener.clone();

        let restored = match self.sound_scene.find_sound_mut(sound_id) {
            Some(SceneSound::Audible(sound)) => {
                restore_audible_voice(sound, channel, Arc::clone(&source_arc), &record)
            }
            Some(SceneSound::Sound3D(sound)) => restore_3d_voice(
                sound,
                channel,
                Arc::clone(&source_arc),
                &record,
                &listener_snapshot,
            ),
            Some(SceneSound::Pseudo3D(pseudo)) => restore_pseudo3d_voice(
                pseudo,
                channel,
                Arc::clone(&source_arc),
                &record,
                &listener_snapshot,
            ),
            None => {
                warn!(
                    "Mixer restore: sound {} disappeared before restoration completed",
                    sound_id
                );
                false
            }
        };

        Ok(restored)
    }

    pub(crate) fn recalculate_sample_counts(&mut self) {
        let mut two_d = 0u32;
        let mut three_d = 0u32;
        let mut streams = 0u32;
        for sound in self
            .sound_scene
            .dynamic_sounds
            .iter()
            .chain(self.sound_scene.static_sounds.iter())
        {
            match sound.class_id() {
                crate::SoundClassId::ThreeD => three_d = three_d.saturating_add(1),
                crate::SoundClassId::Pseudo3D => streams = streams.saturating_add(1),
                _ => two_d = two_d.saturating_add(1),
            }
        }
        self.num_2d_samples = two_d;
        self.num_3d_samples = three_d;
        self.num_streams = streams;
    }

    /// Expose playback format parameters
    pub fn playback_rate(&self) -> u32 {
        self.playback_rate
    }

    pub fn playback_bits(&self) -> u16 {
        self.playback_bits
    }

    pub fn playback_is_stereo(&self) -> bool {
        self.playback_stereo
    }

    /// Configure sample pool limits
    pub fn set_max_2d_sample_count(&mut self, count: u32) {
        self.max_2d_samples = count.max(1);
    }

    pub fn max_2d_sample_count(&self) -> u32 {
        self.max_2d_samples
    }

    pub fn available_2d_sample_count(&self) -> u32 {
        self.max_2d_samples.saturating_sub(self.num_2d_samples)
    }

    pub fn set_max_3d_sample_count(&mut self, count: u32) {
        self.max_3d_samples = count.max(1);
    }

    pub fn max_3d_sample_count(&self) -> u32 {
        self.max_3d_samples
    }

    pub fn available_3d_sample_count(&self) -> u32 {
        self.max_3d_samples.saturating_sub(self.num_3d_samples)
    }

    pub fn set_max_2d_buffer_size(&mut self, bytes: usize) {
        self.max_2d_buffer_size = bytes;
    }

    pub fn max_2d_buffer_size(&self) -> usize {
        self.max_2d_buffer_size
    }

    pub fn set_max_3d_buffer_size(&mut self, bytes: usize) {
        self.max_3d_buffer_size = bytes;
    }

    pub fn max_3d_buffer_size(&self) -> usize {
        self.max_3d_buffer_size
    }

    /// Register an end-of-stream callback with optional user param
    pub fn register_eos_callback(&mut self, callback: EndOfStreamCallback, user_param: u32) {
        self.eos_callbacks.push((callback, user_param));
    }

    pub fn unregister_eos_callback(&mut self, callback: &EndOfStreamCallback) {
        self.eos_callbacks
            .retain(|(stored, _)| !std::sync::Arc::ptr_eq(stored, callback));
    }

    pub fn register_text_callback(&mut self, callback: TextEventCallback) {
        self.text_callbacks.push(callback);
    }

    pub fn unregister_text_callback(&mut self, callback: &TextEventCallback) {
        self.text_callbacks
            .retain(|stored| !std::sync::Arc::ptr_eq(stored, callback));
    }

    /// Push a textual event to registered callbacks
    pub fn fire_text_event(&self, text: &str) {
        for handler in &self.text_callbacks {
            handler(text);
        }
    }

    /// Cache-size configuration (kilobytes, mirroring legacy API)
    pub fn set_cache_size_kb(&mut self, kilobytes: usize) {
        self.config.cache_size_bytes = kilobytes.saturating_mul(1024);
        // NOTE: Rebuilding the cache will evict existing entries.
        let cache_config = CacheConfig {
            max_size_bytes: self.config.cache_size_bytes,
            max_items: self.config.max_cache_items,
            block_size: self.config.cache_block_size,
            preload_threshold: self.config.cache_block_size.saturating_mul(4),
        };
        self.source_cache = SourceCache::new(cache_config);
    }

    pub fn cache_size_kb(&self) -> usize {
        self.config.cache_size_bytes / 1024
    }

    pub fn preferences_snapshot(&self) -> AudioPreferences {
        AudioPreferences {
            device_name: self
                .devices
                .iter()
                .find(|d| d.is_default)
                .map(|d| d.name.clone()),
            preferred_3d_provider: self
                .providers_3d
                .get(self.selected_provider as usize)
                .map(|info| info.name.clone()),
            preferred_2d_driver: self.selected_driver_2d.and_then(|kind| {
                self.driver_2d_list
                    .iter()
                    .take(self.driver_2d_count as usize)
                    .find(|info| info.kind == kind)
                    .map(|info| info.name.clone())
            }),
            stereo: self.playback_stereo,
            bits: self.playback_bits,
            hertz: self.playback_rate,
            sound_enabled: self.sound_effects_enabled,
            music_enabled: self.music_enabled,
            sound_volume: self.sound_volume,
            music_volume: self.music_volume,
        }
    }

    pub fn save_preferences_snapshot(key: &str, prefs: &AudioPreferences) -> bool {
        if let Some(path) = preferences_path(key) {
            if let Err(err) = write_preferences(&path, prefs) {
                warn!("Failed to save audio preferences {}: {err}", path.display());
                return false;
            }
            true
        } else {
            false
        }
    }

    pub fn load_preferences_snapshot(key: &str) -> Option<AudioPreferences> {
        let path = preferences_path(key)?;
        match read_preferences(&path) {
            Ok(prefs) => Some(prefs),
            Err(err) => {
                warn!("Failed to read audio preferences {}: {err}", path.display());
                None
            }
        }
    }

    fn apply_preferences(&mut self, prefs: &AudioPreferences) {
        self.set_sound_effects_volume(prefs.sound_volume);
        self.set_music_volume(prefs.music_volume);
        self.allow_sound_effects(prefs.sound_enabled);
        self.allow_music(prefs.music_enabled);

        let format = AudioFormat {
            channels: if prefs.stereo { 2 } else { 1 },
            sample_rate: sample_rate_from_u32(prefs.hertz),
            sample_width: sample_width_from_u16(prefs.bits),
            channel_layout: if prefs.stereo {
                crate::formats::ChannelLayout::Stereo
            } else {
                crate::formats::ChannelLayout::Mono
            },
        };
        let _ = self.open_2d_device_with_format(&format);

        if let Some(driver) = prefs.preferred_2d_driver.as_deref() {
            self.select_2d_driver_by_name(driver);
        }
        if let Some(provider) = prefs.preferred_3d_provider.as_deref() {
            self.select_3d_provider_by_name(provider);
        }
    }

    /// Get current cache memory usage in bytes
    ///
    /// Matches C++ WWAudio.cpp m_CurrentCacheSize tracking
    pub fn current_cache_usage(&self) -> usize {
        self.source_cache.stats().used_size
    }

    /// Get maximum cache memory size in bytes
    ///
    /// Matches C++ WWAudio.cpp m_MaxCacheSize (line 81)
    pub fn max_cache_size(&self) -> usize {
        self.config.cache_size_bytes
    }

    /// Get cache memory usage statistics
    ///
    /// Returns (used_bytes, max_bytes, utilization_percent)
    /// Useful for monitoring audio memory consumption
    pub fn cache_memory_stats(&self) -> (usize, usize, f32) {
        let stats = self.source_cache.stats();
        let max_size = self.config.cache_size_bytes;
        let utilization = if max_size > 0 {
            (stats.used_size as f32 / max_size as f32) * 100.0
        } else {
            0.0
        };
        (stats.used_size, max_size, utilization)
    }

    /// Get total audio system memory allocation estimate
    ///
    /// Includes cache, mixer buffers, and active sound instances
    /// This provides insight into the total memory footprint of the audio system
    pub fn total_memory_estimate(&self) -> usize {
        let cache_size = self.current_cache_usage();
        let mixer_buffer_size = self.config.mixer_buffer_frames
            * self.config.default_format.channels as usize
            * std::mem::size_of::<f32>();
        let fallback_buffer_size = self.fallback_buffer.data.len() * std::mem::size_of::<f32>();

        // Estimate active sound memory (very rough approximation)
        let active_sounds_estimate =
            (self.num_2d_samples + self.num_3d_samples + self.num_streams) as usize * 1024;

        cache_size + mixer_buffer_size + fallback_buffer_size + active_sounds_estimate
    }

    /// Logical type registration (definition system integration)
    pub fn add_logical_type(&mut self, type_id: i32, display_name: impl Into<String>) {
        let display_name = display_name.into();
        if let Some(existing) = self.logical_types.iter_mut().find(|r| r.type_id == type_id) {
            existing.display_name = display_name.clone();
        } else {
            self.logical_types.push(LogicalTypeRecord {
                display_name: display_name.clone(),
                type_id,
            });
        }
        self.logical_definition_manager
            .add_type(type_id, display_name);
    }

    pub fn reset_logical_types(&mut self) {
        self.logical_types.clear();
        self.logical_definition_manager.clear();
        self.logical_registry.clear();
        self.logical_sound_factory.clear();
    }

    pub fn logical_type(&self, index: usize) -> Option<&LogicalTypeRecord> {
        self.logical_types.get(index)
    }

    pub fn logical_type_count(&self) -> usize {
        self.logical_types.len()
    }

    /// Attach a custom file factory (replaces WWAudio::Set_File_Factory)
    pub fn set_file_factory(&mut self, factory: Arc<dyn AudioFileFactory>) {
        self.file_factory = Some(factory);
    }

    pub fn clear_file_factory(&mut self) {
        self.file_factory = None;
    }

    pub fn file_factory(&self) -> Option<&Arc<dyn AudioFileFactory>> {
        self.file_factory.as_ref()
    }

    pub fn queue_delayed_release_object<T>(&self, delay_ms: u64, object: T)
    where
        T: Send + 'static,
    {
        queue_delayed_release(delay_ms, move || drop(object));
    }

    pub fn logical_type_definition(&self, id: i32) -> Option<&LogicalTypeDefinition> {
        self.logical_definition_manager.get(id)
    }

    pub fn register_logical_sound_factory_entry(
        &mut self,
        sound_id: SoundObjectId,
        type_mask: u32,
        display: Option<String>,
    ) {
        self.logical_sound_factory
            .register(sound_id, type_mask, display.clone());
        self.logical_registry.register(sound_id, type_mask, display);
    }

    pub fn logical_factory_entry(
        &self,
        sound_id: SoundObjectId,
    ) -> Option<&LogicalSoundFactoryEntry> {
        self.logical_sound_factory.lookup(sound_id)
    }

    pub fn logical_registration(
        &self,
        sound_id: SoundObjectId,
    ) -> Option<&LogicalSoundRegistration> {
        self.logical_registry.lookup(sound_id)
    }

    pub fn clear_logical_sound_registry(&mut self) {
        self.logical_registry.clear();
    }

    /// Simple helper that loads a file and plays it on a temporary channel
    pub async fn simple_play_2d_sound_effect(
        &mut self,
        path: &str,
        priority: crate::Priority,
    ) -> Result<()> {
        warn!("simple_play_2d_sound_effect is a placeholder and will allocate a temporary channel");
        let device = self.open_device(None).await?;
        let mut channel = device.create_channel(priority)?;
        let source = self.load_source(path).await?;
        channel.play_source(source, false)
    }

    /// Play a raw in-memory sound effect
    pub async fn simple_play_2d_sound_effect_from_memory(
        &mut self,
        identifier: &str,
        data: Vec<u8>,
        format: AudioFormat,
        priority: crate::Priority,
    ) -> Result<()> {
        let device = self.open_device(None).await?;
        let mut channel = device.create_channel(priority)?;
        let source = crate::AudioSource::from_memory(data, format)?;
        self.source_cache
            .put(
                identifier.to_string(),
                std::sync::Arc::new(source.clone()),
                priority,
            )
            .await?;
        channel.play_source(source, false)
    }

    /// Check if a sound is currently cached
    pub async fn is_sound_cached(&self, identifier: &str) -> Result<bool> {
        Ok(self.source_cache.get(identifier).await?.is_some())
    }

    /// Create a sound effect from a file path (stub for parity)
    pub async fn create_sound_effect(&mut self, path: &str) -> Result<crate::AudioSource> {
        self.load_source(path).await
    }

    /// Create a sound effect from an in-memory buffer (stub)
    pub fn create_sound_effect_from_memory(
        &self,
        data: Vec<u8>,
        format: AudioFormat,
    ) -> Result<crate::AudioSource> {
        crate::AudioSource::from_memory(data, format)
    }

    /// Create a 2D audible sound and track it within the scene graph
    pub async fn create_audible_sound(&mut self, path: &str) -> Result<WWHandle> {
        let source = self.load_source(path).await?;
        let id = self.sound_scene.allocate_id();
        let mut audible = crate::AudibleSound::new(id, crate::SoundClassId::TwoD);
        let arc_source = std::sync::Arc::new(source.clone());
        audible.set_source(std::sync::Arc::clone(&arc_source));
        let device = self.open_device(None).await?;
        let channel = device.create_channel(crate::Priority::Normal)?;
        let handle = make_2d_handle(id, channel, arc_source);
        if let WWHandle::Sound2D {
            handle: inner_handle,
            ..
        } = &handle
        {
            audible.set_handle(inner_handle.clone());
        }
        self.sound_scene
            .add_sound(crate::SceneSound::Audible(audible));
        self.num_2d_samples = self.num_2d_samples.saturating_add(1);
        Ok(handle)
    }

    /// Create a basic 3D sound and register it with the scene graph
    pub async fn create_3d_sound(&mut self, path: &str) -> Result<WWHandle> {
        let source = self.load_source(path).await?;
        let id = self.sound_scene.allocate_id();
        let mut sound3d = crate::Sound3D::new(id);
        let arc_source = std::sync::Arc::new(source.clone());
        sound3d.base.set_source(std::sync::Arc::clone(&arc_source));

        let device = self.open_device(None).await?;
        let channel = device.create_channel(crate::Priority::Normal)?;
        let mut sample_handle = crate::handles::Sound2DHandle::new(channel);
        sample_handle.initialize(std::sync::Arc::clone(&arc_source));
        sound3d.base.set_handle(sample_handle.clone());

        let handle = make_3d_handle(
            id,
            sound3d.clone(),
            sample_handle,
            std::sync::Arc::clone(&arc_source),
        );
        self.sound_scene
            .add_sound(crate::SceneSound::Sound3D(sound3d));
        self.num_3d_samples = self.num_3d_samples.saturating_add(1);
        Ok(handle)
    }

    /// Create a pseudo 3D sound that will be spatialised in software
    pub async fn create_pseudo3d_sound(&mut self, path: &str) -> Result<WWHandle> {
        let source = self.load_source(path).await?;
        let id = self.sound_scene.allocate_id();
        let mut pseudo = crate::SoundPseudo3D::new(id);
        let arc_source = std::sync::Arc::new(source.clone());
        pseudo
            .base
            .base
            .set_source(std::sync::Arc::clone(&arc_source));
        let device = self.open_device(None).await?;
        let channel = device.create_channel(crate::Priority::Normal)?;
        let handle = make_stream_handle(id, channel, arc_source);
        if let WWHandle::SoundStream {
            handle: stream_handle,
            ..
        } = &handle
        {
            pseudo.base.base.set_handle(stream_handle.sample_handle());
        }
        self.sound_scene
            .add_sound(crate::SceneSound::Pseudo3D(pseudo));
        self.num_streams = self.num_streams.saturating_add(1);
        Ok(handle)
    }

    /// Get the estimated digital audio CPU utilisation (placeholder)
    pub fn digital_cpu_percent(&self) -> f32 {
        0.0
    }

    /// Determine whether audio output is effectively disabled
    pub fn is_disabled(&self) -> bool {
        !self.sound_effects_enabled && !self.music_enabled
    }

    /// Persist user-facing audio preferences. This replaces the Windows registry calls in the original code.
    pub fn save_preferences(&self) -> bool {
        self.save_preferences_with_key("default")
    }

    pub fn save_preferences_with_key(&self, key: &str) -> bool {
        let prefs = self.preferences_snapshot();
        Self::save_preferences_snapshot(key, &prefs)
    }

    pub fn save_preferences_explicit(&self, key: &str, prefs: &AudioPreferences) -> bool {
        Self::save_preferences_snapshot(key, prefs)
    }

    pub fn load_preferences(&mut self) -> bool {
        self.load_preferences_with_key("default")
    }

    pub fn load_preferences_with_key(&mut self, key: &str) -> bool {
        if let Some(prefs) = Self::load_preferences_snapshot(key) {
            self.apply_preferences(&prefs);
            true
        } else {
            false
        }
    }

    /// Free completed sounds outside the regular update tick
    pub fn free_completed_sounds(&mut self) {
        self.stopped_audio.clear();
    }

    /// Flush the entire sound cache
    pub async fn flush_cache(&self) -> Result<()> {
        self.source_cache.clear().await
    }

    pub fn drain_mixer_events(&mut self) -> Vec<MixerEvent> {
        self.pending_mixer_events.drain(..).collect()
    }

    pub fn drain_logical_events(&mut self) -> Vec<LogicalEvent> {
        self.pending_logical_events.drain(..).collect()
    }
}

fn restore_audible_voice(
    sound: &mut crate::AudibleSound,
    channel: crate::AudioChannel,
    source: Arc<AudioSource>,
    record: &SavedMixerVoiceRecord,
) -> bool {
    let mut handle = Sound2DHandle::new(channel);
    if let Some(miles) = record.miles_handle_id {
        handle.base.set_miles_handle(miles);
    }
    handle.initialize(Arc::clone(&source));
    let handle_clone = handle.clone();

    sound.set_source(source);
    sound.set_handle(handle);
    sound.set_loop_count(record.params.loop_count);
    sound.set_playback_rate(record.params.playback_rate);
    sound.set_culled(record.params.is_culled);
    sound.set_volume(record.params.gain.clamp(0.0, 1.0));

    let pan = (record.params.pan.clamp(-1.0, 1.0) * 1000.0).round() as i32;
    sound.set_pan(pan);

    let frames = record.timeline.position_frames.max(0.0).round() as u64;
    sound.set_current_frame(frames);

    let looping = record.params.loop_count == 0;
    let should_play = matches!(
        record.playback_state,
        VoicePlaybackState::Playing | VoicePlaybackState::Pending
    );

    if should_play {
        if sound.play(looping).is_err() {
            return false;
        }
    } else {
        sound.base.set_state(match record.playback_state {
            VoicePlaybackState::Paused => SoundState::Paused,
            VoicePlaybackState::Completed => SoundState::Stopped,
            _ => SoundState::Playing,
        });
    }

    apply_voice_timeline(&handle_clone, record);

    match record.playback_state {
        VoicePlaybackState::Paused => {
            if should_play {
                let _ = sound.pause();
            } else {
                let _ = handle_clone.pause_sample();
                sound.base.set_state(SoundState::Paused);
            }
        }
        VoicePlaybackState::Completed => {
            if should_play {
                let _ = sound.stop(false);
            } else {
                let _ = handle_clone.stop_sample();
                let _ = sound.stop(false);
            }
            sound.base.set_state(SoundState::Stopped);
        }
        _ => {}
    }
    true
}

fn restore_3d_voice(
    sound: &mut Sound3D,
    channel: crate::AudioChannel,
    source: Arc<AudioSource>,
    record: &SavedMixerVoiceRecord,
    listener: &Listener3D,
) -> bool {
    let mut handle = Sound2DHandle::new(channel);
    if let Some(miles) = record.miles_handle_id {
        handle.base.set_miles_handle(miles);
    }
    handle.initialize(Arc::clone(&source));
    let handle_clone = handle.clone();

    sound.base.set_source(source);
    sound.base.set_handle(handle);
    sound.base.set_loop_count(record.params.loop_count);
    sound.base.set_playback_rate(record.params.playback_rate);
    sound.base.set_culled(record.params.is_culled);
    sound.base.set_volume(record.params.gain.clamp(0.0, 1.0));

    let pan = (record.params.pan.clamp(-1.0, 1.0) * 1000.0).round() as i32;
    sound.base.set_pan(pan);

    sound.set_max_vol_radius(record.params.spatial.min_distance);
    sound.set_dropoff_radius(record.params.spatial.max_distance);
    sound.set_velocity(record.params.spatial.velocity);
    sound.set_listener_transform(record.params.spatial.to_matrix());

    let frames = record.timeline.position_frames.max(0.0).round() as u64;
    sound.base.set_current_frame(frames);

    let looping = record.params.loop_count == 0;
    let should_play = matches!(
        record.playback_state,
        VoicePlaybackState::Playing | VoicePlaybackState::Pending
    );

    if should_play {
        if sound.base.play(looping).is_err() {
            return false;
        }
    } else {
        sound.base.base.set_state(match record.playback_state {
            VoicePlaybackState::Paused => SoundState::Paused,
            VoicePlaybackState::Completed => SoundState::Stopped,
            _ => SoundState::Playing,
        });
    }

    apply_voice_timeline(&handle_clone, record);

    match record.playback_state {
        VoicePlaybackState::Paused => {
            if should_play {
                let _ = sound.base.pause();
            } else {
                let _ = handle_clone.pause_sample();
                sound.base.base.set_state(SoundState::Paused);
            }
        }
        VoicePlaybackState::Completed => {
            if should_play {
                let _ = sound.base.stop(false);
            } else {
                let _ = handle_clone.stop_sample();
                let _ = sound.base.stop(false);
            }
            sound.base.base.set_state(SoundState::Stopped);
        }
        _ => {}
    }

    sound.update_spatial_audio(listener);
    true
}

fn restore_pseudo3d_voice(
    pseudo: &mut SoundPseudo3D,
    channel: crate::AudioChannel,
    source: Arc<AudioSource>,
    record: &SavedMixerVoiceRecord,
    listener: &Listener3D,
) -> bool {
    let mut handle = Sound2DHandle::new(channel);
    if let Some(miles) = record.miles_handle_id {
        handle.base.set_miles_handle(miles);
    }
    handle.initialize(Arc::clone(&source));
    let handle_clone = handle.clone();

    pseudo.base.base.set_source(source);
    pseudo.base.base.set_handle(handle);
    pseudo.base.base.set_loop_count(record.params.loop_count);
    pseudo
        .base
        .base
        .set_playback_rate(record.params.playback_rate);
    pseudo.set_culled(record.params.is_culled);
    pseudo
        .base
        .base
        .set_volume(record.params.gain.clamp(0.0, 1.0));

    let pan = (record.params.pan.clamp(-1.0, 1.0) * 1000.0).round() as i32;
    pseudo.base.base.set_pan(pan);

    pseudo
        .base
        .set_max_vol_radius(record.params.spatial.min_distance);
    pseudo
        .base
        .set_dropoff_radius(record.params.spatial.max_distance);
    pseudo.base.set_velocity(record.params.spatial.velocity);
    pseudo
        .base
        .set_listener_transform(record.params.spatial.to_matrix());

    let frames = record.timeline.position_frames.max(0.0).round() as u64;
    pseudo.base.base.set_current_frame(frames);

    let looping = record.params.loop_count == 0;
    let should_play = matches!(
        record.playback_state,
        VoicePlaybackState::Playing | VoicePlaybackState::Pending
    );

    if should_play {
        if pseudo.base.base.play(looping).is_err() {
            return false;
        }
    } else {
        pseudo
            .base
            .base
            .base
            .set_state(match record.playback_state {
                VoicePlaybackState::Paused => SoundState::Paused,
                VoicePlaybackState::Completed => SoundState::Stopped,
                _ => SoundState::Playing,
            });
    }

    apply_voice_timeline(&handle_clone, record);

    match record.playback_state {
        VoicePlaybackState::Paused => {
            if should_play {
                let _ = pseudo.base.base.pause();
            } else {
                let _ = handle_clone.pause_sample();
                pseudo.base.base.base.set_state(SoundState::Paused);
            }
        }
        VoicePlaybackState::Completed => {
            if should_play {
                let _ = pseudo.base.base.stop(false);
            } else {
                let _ = handle_clone.stop_sample();
                let _ = pseudo.base.base.stop(false);
            }
            pseudo.base.base.base.set_state(SoundState::Stopped);
        }
        _ => {}
    }

    pseudo.update_spatial_audio(listener);
    true
}

fn apply_voice_timeline(handle: &Sound2DHandle, record: &SavedMixerVoiceRecord) {
    let rate = record.timeline.source_rate.max(1);
    let position_ms = ((record.timeline.position_frames / rate as f64) * 1000.0)
        .round()
        .clamp(0.0, u32::MAX as f64) as u32;
    let _ = handle.set_sample_ms_position(position_ms);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audible_sound::AudibleSound,
        save_load::{
            SavedMixerVoiceRecord, SavedVoiceParams, SavedVoiceSpatial, SavedVoiceTimeline,
        },
        sound_types::SoundClassId,
    };

    fn test_source() -> Arc<AudioSource> {
        let format = AudioFormat::default();
        let frame_size = format.bytes_per_frame();
        let data = vec![0u8; frame_size * 32];
        Arc::new(AudioSource::from_memory(data, format).expect("memory source"))
    }

    fn test_channel() -> crate::AudioChannel {
        let mixer = Arc::new(AudioMixer::new(MixerConfig::default()));
        crate::AudioChannel::new(1, Priority::Normal, mixer)
    }

    #[test]
    fn restore_audible_voice_preserves_paused_state() {
        let source = test_source();
        let mut sound = AudibleSound::new(1, SoundClassId::TwoD);
        let record = SavedMixerVoiceRecord {
            playback_state: VoicePlaybackState::Paused,
            params: SavedVoiceParams {
                gain: 0.75,
                pan: 0.1,
                playback_rate: 44_100,
                loop_count: 1,
                start_frame: 220,
                is_culled: false,
                spatial: SavedVoiceSpatial::default(),
            },
            source_identifier: Some("test".into()),
            timeline: SavedVoiceTimeline {
                position_frames: 220.0,
                rendered_frames: 0,
                timeline_origin: 0,
                last_sequence: 0,
                source_rate: 44_100,
            },
            ..SavedMixerVoiceRecord::default()
        };

        let result = restore_audible_voice(&mut sound, test_channel(), source, &record);
        assert!(result);
        assert_eq!(sound.base.state(), SoundState::Paused);
    }

    #[test]
    fn restore_audible_voice_preserves_completed_state() {
        let source = test_source();
        let mut sound = AudibleSound::new(2, SoundClassId::TwoD);
        let record = SavedMixerVoiceRecord {
            playback_state: VoicePlaybackState::Completed,
            params: SavedVoiceParams::default(),
            source_identifier: Some("test".into()),
            ..SavedMixerVoiceRecord::default()
        };

        let result = restore_audible_voice(&mut sound, test_channel(), source, &record);
        assert!(result);
        assert_eq!(sound.base.state(), SoundState::Stopped);
    }
}
impl AudioDevice {
    /// Get device information
    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    /// Retrieve a raw pointer to the owning audio system (for legacy interop)
    pub fn parent_system(&self) -> *mut AudioSystem {
        self.system
    }

    /// Create a new audio channel
    pub fn create_channel(&self, priority: Priority) -> Result<crate::AudioChannel> {
        debug!("Creating audio channel with priority: {:?}", priority);

        let system = unsafe { self.system.as_mut().ok_or(DeviceError::NotInitialized)? };

        system.ensure_output_stream()?;
        let id = system.allocate_channel_id();
        let mixer = Arc::clone(&system.mixer);
        Ok(crate::AudioChannel::new(id, priority, mixer))
    }

    /// Reserve a channel for playback, matching the Miles API.
    pub fn reserve_channel(
        &self,
        channel_type: crate::channel::ChannelType,
    ) -> Result<crate::AudioChannel> {
        let priority = match channel_type {
            crate::channel::ChannelType::Music => Priority::High,
            crate::channel::ChannelType::Voice => Priority::High,
            crate::channel::ChannelType::System => Priority::Critical,
            crate::channel::ChannelType::Ambient => Priority::Normal,
            crate::channel::ChannelType::User => Priority::Normal,
        };

        self.create_channel(priority)
    }

    /// Get current device configuration
    pub fn config(&self) -> &DeviceConfig {
        &self.config
    }
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            format: AudioFormat::default(),
            buffer_size: 1024,
            buffer_count: 4,
            low_latency: false,
        }
    }
}

fn sample_rate_from_u32(hz: u32) -> crate::formats::SampleRate {
    use crate::formats::SampleRate::*;
    match hz {
        8000 => Hz8000,
        11025 => Hz11025,
        16000 => Hz16000,
        22050 => Hz22050,
        44100 => Hz44100,
        48000 => Hz48000,
        96000 => Hz96000,
        192000 => Hz192000,
        _ => Hz44100,
    }
}

fn sample_width_from_u16(bits: u16) -> crate::formats::SampleWidth {
    use crate::formats::SampleWidth::*;
    match bits {
        8 => U8,
        24 => S24,
        32 => S32,
        _ => S16,
    }
}

fn channel_layout_from_channels(channels: u16) -> ChannelLayout {
    match channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        3 => ChannelLayout::Surround21,
        4 => ChannelLayout::Surround41,
        6 => ChannelLayout::Surround51,
        8 => ChannelLayout::Surround71,
        _ => ChannelLayout::Stereo,
    }
}
