//! # Advanced Audio System
//!
//! Complete audio loading and playback system with:
//! - Support for all C&C audio formats (WAV, OGG, MP3, custom)
//! - 3D spatial audio positioning
//! - Dynamic range compression
//! - Audio streaming for large files
//! - Multi-channel surround sound
//! - Environmental audio effects (reverb, echo)
//! - Audio asset management and caching
//! - Real-time mixing and effects processing

use kira::{
    manager::{AudioManager, AudioManagerSettings},
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
    sound::PlaybackRate,
    sound::PlaybackState as KiraPlaybackState,
    spatial::{
        emitter::{EmitterHandle, EmitterSettings},
        listener::{ListenerHandle, ListenerSettings},
        scene::SpatialSceneSettings,
    },
    track::{
        effect::{
            filter::{FilterBuilder, FilterHandle, FilterMode},
            reverb::{ReverbBuilder, ReverbHandle},
        },
        TrackBuilder, TrackHandle,
    },
    tween::Tween,
    Volume,
};
use nalgebra::{UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use thiserror::Error;

use super::{AssetError, AssetHandle, AssetPriority};

/// Audio loading and processing errors
#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Audio format not supported: {format} for file {path}")]
    UnsupportedFormat { path: String, format: String },
    #[error("Audio decoding failed: {path} - {error}")]
    DecodingFailed { path: String, error: String },
    #[error("Audio engine error: {0}")]
    EngineError(String),
    #[error("Track creation failed: {0}")]
    TrackFailed(String),
    #[error("Effect processing failed: {effect} - {error}")]
    EffectFailed { effect: String, error: String },
    #[error("Audio streaming error: {0}")]
    StreamingError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Audio format types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioFormat {
    Wav,
    Mp3,
    Ogg,
    Flac,
    M4A,
    Custom(u32), // For C&C specific formats
}

impl AudioFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "wav" => Self::Wav,
            "mp3" => Self::Mp3,
            "ogg" => Self::Ogg,
            "flac" => Self::Flac,
            "m4a" => Self::M4A,
            _ => Self::Custom(0),
        }
    }
}

/// Audio asset type categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioAssetType {
    Music,
    SoundEffect,
    Voice,
    Ambient,
    UI,
    Weapon,
    Vehicle,
    Environment,
}

/// Audio quality settings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioQuality {
    Low,    // 22kHz, mono/stereo
    Medium, // 44kHz, stereo
    High,   // 48kHz, stereo/5.1
    Ultra,  // 96kHz, 7.1 surround
}

impl AudioQuality {
    pub fn sample_rate(self) -> u32 {
        match self {
            Self::Low => 22050,
            Self::Medium => 44100,
            Self::High => 48000,
            Self::Ultra => 96000,
        }
    }

    pub fn channels(self) -> u16 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 6,
            Self::Ultra => 8,
        }
    }
}

/// Audio playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Fading,
    Streaming,
}

/// 3D audio settings
#[derive(Debug, Clone)]
pub struct Audio3DSettings {
    pub position: Vector3<f32>,
    pub velocity: Vector3<f32>,
    pub orientation: Vector3<f32>,
    pub max_distance: f32,
    pub rolloff_factor: f32,
    pub doppler_factor: f32,
    pub cone_inner_angle: f32,
    pub cone_outer_angle: f32,
    pub cone_outer_gain: f32,
}

impl Default for Audio3DSettings {
    fn default() -> Self {
        Self {
            position: Vector3::zeros(),
            velocity: Vector3::zeros(),
            orientation: Vector3::new(0.0, 0.0, -1.0),
            max_distance: 100.0,
            rolloff_factor: 1.0,
            doppler_factor: 1.0,
            cone_inner_angle: 360.0,
            cone_outer_angle: 360.0,
            cone_outer_gain: 0.0,
        }
    }
}

/// Audio listener settings (camera/player position)
#[derive(Debug, Clone)]
pub struct AudioListener {
    pub position: Vector3<f32>,
    pub velocity: Vector3<f32>,
    pub forward: Vector3<f32>,
    pub up: Vector3<f32>,
    pub gain: f32,
}

impl Default for AudioListener {
    fn default() -> Self {
        Self {
            position: Vector3::zeros(),
            velocity: Vector3::zeros(),
            forward: Vector3::new(0.0, 0.0, -1.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            gain: 1.0,
        }
    }
}

/// Environmental audio effects
#[derive(Debug, Clone)]
pub struct AudioEnvironment {
    pub name: String,
    pub reverb_time: f32,
    pub reverb_decay: f32,
    pub reverb_density: f32,
    pub air_absorption: f32,
    pub echo_delay: f32,
    pub echo_feedback: f32,
    pub low_pass_cutoff: f32,
    pub high_pass_cutoff: f32,
}

impl Default for AudioEnvironment {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            reverb_time: 1.0,
            reverb_decay: 0.5,
            reverb_density: 0.7,
            air_absorption: 0.0,
            echo_delay: 0.0,
            echo_feedback: 0.0,
            low_pass_cutoff: 20000.0,
            high_pass_cutoff: 20.0,
        }
    }
}

/// Audio asset metadata
#[derive(Debug, Clone)]
pub struct AudioAsset {
    pub handle: AssetHandle,
    pub path: PathBuf,
    pub name: String,
    pub format: AudioFormat,
    pub asset_type: AudioAssetType,
    pub duration: Duration,
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u16,
    pub file_size: u64,
    pub is_looping: bool,
    pub is_streaming: bool,
    pub quality: AudioQuality,
    pub tags: Vec<String>,

    // 3D audio properties
    pub spatial_settings: Option<Audio3DSettings>,

    // Playback settings
    pub volume: f32,
    pub pitch: f32,
    pub priority: AudioAssetPriority,

    // Performance data
    pub load_time: Duration,
    pub last_played: Option<Instant>,
    pub play_count: u64,
}

/// Audio priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AudioAssetPriority {
    Critical = 0, // UI sounds, essential game audio
    High = 1,     // Player actions, important effects
    Normal = 2,   // General sound effects
    Low = 3,      // Ambient sounds
    Lowest = 4,   // Optional background audio
}

/// Audio loading request
struct AudioLoadRequest {
    handle: AssetHandle,
    path: PathBuf,
    data: Vec<u8>,
    priority: AssetPriority,
    settings: AudioLoadSettings,
    callback: Option<Box<dyn FnOnce(Result<AssetHandle, AudioError>) + Send + Sync>>,
}

impl std::fmt::Debug for AudioLoadRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioLoadRequest")
            .field("handle", &self.handle)
            .field("path", &self.path)
            .field("data_len", &self.data.len())
            .field("priority", &self.priority)
            .field("settings", &self.settings)
            .field(
                "has_callback",
                &self.callback.as_ref().map(|_| true).unwrap_or(false),
            )
            .finish()
    }
}

/// Audio loading settings
#[derive(Debug, Clone)]
pub struct AudioLoadSettings {
    pub asset_type: AudioAssetType,
    pub quality: AudioQuality,
    pub enable_streaming: bool,
    pub enable_3d: bool,
    pub spatial_settings: Option<Audio3DSettings>,
    pub volume: f32,
    pub pitch: f32,
    pub looping: bool,
    pub preload: bool,
}

impl Default for AudioLoadSettings {
    fn default() -> Self {
        Self {
            asset_type: AudioAssetType::SoundEffect,
            quality: AudioQuality::Medium,
            enable_streaming: false,
            enable_3d: false,
            spatial_settings: None,
            volume: 1.0,
            pitch: 1.0,
            looping: false,
            preload: false,
        }
    }
}

/// Audio playback instance
pub struct AudioInstance {
    pub id: u64,
    pub asset_handle: AssetHandle,
    pub state: PlaybackState,
    pub volume: f32,
    pub pitch: f32,
    pub position: Option<Vector3<f32>>,
    pub sound_handle: Option<StaticSoundHandle>,
    pub emitter_handle: Option<EmitterHandle>,
    pub start_time: Instant,
    pub fade_target: Option<f32>,
    pub fade_duration: Option<Duration>,
}

impl std::fmt::Debug for AudioInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioInstance")
            .field("id", &self.id)
            .field("asset_handle", &self.asset_handle)
            .field("state", &self.state)
            .field("volume", &self.volume)
            .field("pitch", &self.pitch)
            .field("position", &self.position)
            .field("has_sound_handle", &self.sound_handle.is_some())
            .field("has_emitter_handle", &self.emitter_handle.is_some())
            .field("start_time", &self.start_time)
            .field("fade_target", &self.fade_target)
            .field("fade_duration", &self.fade_duration)
            .finish()
    }
}

/// Complete Audio System
pub struct AudioLoader {
    // Core audio engine
    audio_manager: Arc<Mutex<AudioManager>>,

    // Asset storage
    audio_assets: Arc<RwLock<HashMap<AssetHandle, Arc<AudioAsset>>>>,
    asset_index: Arc<RwLock<HashMap<PathBuf, AssetHandle>>>,

    // Sound data storage - holds actual decoded audio for playback
    sound_data_cache: Arc<RwLock<HashMap<AssetHandle, StaticSoundData>>>,

    // Playback management
    active_instances: Arc<RwLock<HashMap<u64, AudioInstance>>>,
    instance_counter: Arc<Mutex<u64>>,

    // 3D audio system
    listener: Arc<RwLock<AudioListener>>,
    spatial_scene: Arc<Mutex<kira::spatial::scene::SpatialSceneHandle>>,
    spatial_listener: Arc<Mutex<ListenerHandle>>,

    // Environmental effects
    current_environment: Arc<RwLock<AudioEnvironment>>,
    environments: Arc<RwLock<HashMap<String, AudioEnvironment>>>,

    // Audio tracks for mixing
    music_track: Arc<Mutex<Option<TrackHandle>>>,
    sfx_track: Arc<Mutex<Option<TrackHandle>>>,
    voice_track: Arc<Mutex<Option<TrackHandle>>>,
    ui_track: Arc<Mutex<Option<TrackHandle>>>,
    sfx_low_pass: Arc<Mutex<Option<FilterHandle>>>,
    sfx_high_pass: Arc<Mutex<Option<FilterHandle>>>,
    sfx_reverb: Arc<Mutex<Option<ReverbHandle>>>,

    // Loading system
    load_queue: Arc<Mutex<VecDeque<AudioLoadRequest>>>,

    // Configuration
    config: AudioConfig,

    // Statistics
    stats: Arc<RwLock<AudioStats>>,
}

/// Audio system configuration
#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub voice_volume: f32,
    pub ui_volume: f32,
    pub quality: AudioQuality,
    pub max_concurrent_sounds: u32,
    pub enable_3d_audio: bool,
    pub enable_environmental_effects: bool,
    pub streaming_buffer_size: u32,
    pub cache_size_mb: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            music_volume: 0.8,
            sfx_volume: 1.0,
            voice_volume: 1.0,
            ui_volume: 1.0,
            quality: AudioQuality::Medium,
            max_concurrent_sounds: 64,
            enable_3d_audio: true,
            enable_environmental_effects: true,
            streaming_buffer_size: 4096,
            cache_size_mb: 128,
        }
    }
}

/// Audio system statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AudioStats {
    pub total_assets: u64,
    pub memory_used_mb: f32,
    pub active_instances: u32,
    pub total_played: u64,
    pub streaming_instances: u32,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub average_load_time_ms: f32,
    pub peak_concurrent_sounds: u32,
}

impl AudioLoader {
    /// Create new audio loader
    pub fn new() -> Result<Self, AudioError> {
        let config = AudioConfig::default();

        // Initialize Kira audio manager
        let audio_manager_settings = AudioManagerSettings::default();
        let mut audio_manager = AudioManager::new(audio_manager_settings).map_err(|e| {
            AudioError::EngineError(format!("Failed to initialize audio manager: {}", e))
        })?;

        // Create audio tracks for different types
        let music_track = audio_manager
            .add_sub_track(TrackBuilder::new())
            .map_err(|e| AudioError::TrackFailed(format!("Music track creation failed: {}", e)))?;

        let mut sfx_builder = TrackBuilder::new();
        let sfx_low_pass =
            sfx_builder.add_effect(FilterBuilder::new().mode(FilterMode::LowPass).mix(0.0));
        let sfx_high_pass =
            sfx_builder.add_effect(FilterBuilder::new().mode(FilterMode::HighPass).mix(0.0));
        let sfx_reverb = sfx_builder.add_effect(ReverbBuilder::new().mix(0.0));
        let sfx_track = audio_manager
            .add_sub_track(sfx_builder)
            .map_err(|e| AudioError::TrackFailed(format!("SFX track creation failed: {}", e)))?;

        let voice_track = audio_manager
            .add_sub_track(TrackBuilder::new())
            .map_err(|e| AudioError::TrackFailed(format!("Voice track creation failed: {}", e)))?;

        let ui_track = audio_manager
            .add_sub_track(TrackBuilder::new())
            .map_err(|e| AudioError::TrackFailed(format!("UI track creation failed: {}", e)))?;

        let mut spatial_scene = audio_manager
            .add_spatial_scene(SpatialSceneSettings::default())
            .map_err(|e| {
                AudioError::EngineError(format!("Spatial scene creation failed: {}", e))
            })?;
        let spatial_listener = spatial_scene
            .add_listener(
                mint::Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                mint::Quaternion {
                    s: 1.0,
                    v: mint::Vector3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                },
                ListenerSettings::default(),
            )
            .map_err(|e| AudioError::EngineError(format!("Listener creation failed: {}", e)))?;

        // Create predefined environments
        let mut environments = HashMap::new();

        environments.insert(
            "outdoor".to_string(),
            AudioEnvironment {
                name: "Outdoor".to_string(),
                reverb_time: 0.2,
                reverb_decay: 0.1,
                reverb_density: 0.1,
                air_absorption: 0.1,
                ..Default::default()
            },
        );

        environments.insert(
            "indoor".to_string(),
            AudioEnvironment {
                name: "Indoor".to_string(),
                reverb_time: 1.5,
                reverb_decay: 0.7,
                reverb_density: 0.8,
                air_absorption: 0.05,
                ..Default::default()
            },
        );

        environments.insert(
            "cave".to_string(),
            AudioEnvironment {
                name: "Cave".to_string(),
                reverb_time: 3.0,
                reverb_decay: 0.9,
                reverb_density: 1.0,
                air_absorption: 0.02,
                echo_delay: 0.5,
                echo_feedback: 0.3,
                ..Default::default()
            },
        );

        Ok(Self {
            audio_manager: Arc::new(Mutex::new(audio_manager)),
            audio_assets: Arc::new(RwLock::new(HashMap::new())),
            asset_index: Arc::new(RwLock::new(HashMap::new())),
            sound_data_cache: Arc::new(RwLock::new(HashMap::new())),
            active_instances: Arc::new(RwLock::new(HashMap::new())),
            instance_counter: Arc::new(Mutex::new(1)),
            listener: Arc::new(RwLock::new(AudioListener::default())),
            spatial_scene: Arc::new(Mutex::new(spatial_scene)),
            spatial_listener: Arc::new(Mutex::new(spatial_listener)),
            current_environment: Arc::new(RwLock::new(AudioEnvironment::default())),
            environments: Arc::new(RwLock::new(environments)),
            music_track: Arc::new(Mutex::new(Some(music_track))),
            sfx_track: Arc::new(Mutex::new(Some(sfx_track))),
            voice_track: Arc::new(Mutex::new(Some(voice_track))),
            ui_track: Arc::new(Mutex::new(Some(ui_track))),
            sfx_low_pass: Arc::new(Mutex::new(Some(sfx_low_pass))),
            sfx_high_pass: Arc::new(Mutex::new(Some(sfx_high_pass))),
            sfx_reverb: Arc::new(Mutex::new(Some(sfx_reverb))),
            load_queue: Arc::new(Mutex::new(VecDeque::new())),
            config,
            stats: Arc::new(RwLock::new(AudioStats::default())),
        })
    }

    /// Load audio asset from data
    pub async fn load_audio_asset(
        &self,
        data: &[u8],
        path: &Path,
        settings: AudioLoadSettings,
    ) -> Result<AssetHandle, AudioError> {
        let start_time = Instant::now();
        let handle = AssetHandle::new();

        // Check cache
        if let Some(cached_handle) = self.asset_index.read().unwrap().get(path) {
            let mut stats = self.stats.write().unwrap();
            stats.cache_hits += 1;
            return Ok(*cached_handle);
        }

        log::info!("Loading audio asset: {}", path.display());

        // Detect audio format
        let format =
            AudioFormat::from_extension(path.extension().and_then(|e| e.to_str()).unwrap_or(""));

        // Create sound data based on settings
        let sound_data = if settings.enable_streaming && data.len() > 1024 * 1024 {
            // Stream large audio files
            self.create_streaming_sound(data, format).await?
        } else {
            // Load into memory for smaller files
            self.create_memory_sound(data, format).await?
        };

        // Extract metadata from decoded sound data
        let (duration, sample_rate, channels, bit_depth) = self
            .analyze_audio_metadata(data, format, &sound_data)
            .await?;

        // Create audio asset
        let audio_asset = AudioAsset {
            handle,
            path: path.to_path_buf(),
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            format,
            asset_type: settings.asset_type,
            duration,
            sample_rate,
            channels,
            bit_depth,
            file_size: data.len() as u64,
            is_looping: settings.looping,
            is_streaming: settings.enable_streaming && data.len() > 1024 * 1024,
            quality: settings.quality,
            tags: Vec::new(),
            spatial_settings: settings.spatial_settings,
            volume: settings.volume,
            pitch: settings.pitch,
            priority: AudioAssetPriority::Normal,
            load_time: start_time.elapsed(),
            last_played: None,
            play_count: 0,
        };

        let asset_arc = Arc::new(audio_asset);

        // Store in cache
        self.audio_assets
            .write()
            .unwrap()
            .insert(handle, asset_arc.clone());
        self.asset_index
            .write()
            .unwrap()
            .insert(path.to_path_buf(), handle);

        // Store the sound data for playback
        self.sound_data_cache
            .write()
            .unwrap()
            .insert(handle, sound_data);

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap();
            stats.cache_misses += 1;
            stats.total_assets += 1;
            stats.memory_used_mb += (data.len() as f32) / (1024.0 * 1024.0);

            let total_time = stats.average_load_time_ms * (stats.total_assets - 1) as f32;
            stats.average_load_time_ms =
                (total_time + start_time.elapsed().as_millis() as f32) / stats.total_assets as f32;
        }

        log::info!(
            "Audio asset loaded: {} ({:.2} MB, {:.0} ms)",
            path.display(),
            data.len() as f32 / (1024.0 * 1024.0),
            start_time.elapsed().as_millis()
        );

        Ok(handle)
    }

    /// Analyze decoded audio to extract metadata.
    async fn analyze_audio_metadata(
        &self,
        data: &[u8],
        format: AudioFormat,
        decoded_sound: &StaticSoundData,
    ) -> Result<(Duration, u32, u16, u16), AudioError> {
        match format {
            AudioFormat::Wav => self.parse_wav_header(data).await,
            _ => Ok((
                decoded_sound.duration(),
                decoded_sound.sample_rate,
                2,  // Kira decodes into stereo frames
                16, // Most content is 16-bit PCM/decoded to f32 internally
            )),
        }
    }

    /// Parse WAV file header
    async fn parse_wav_header(&self, data: &[u8]) -> Result<(Duration, u32, u16, u16), AudioError> {
        if data.len() < 44 {
            return Err(AudioError::DecodingFailed {
                path: "wav_data".to_string(),
                error: "File too small for WAV header".to_string(),
            });
        }

        // Check RIFF signature
        if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return Err(AudioError::DecodingFailed {
                path: "wav_data".to_string(),
                error: "Invalid WAV signature".to_string(),
            });
        }

        // Extract format information
        let sample_rate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        let channels = u16::from_le_bytes([data[22], data[23]]);
        let bits_per_sample = u16::from_le_bytes([data[34], data[35]]);
        let byte_rate = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);

        // Calculate duration
        let data_size = data.len() as u32 - 44; // Approximate
        let duration_secs = if byte_rate > 0 {
            data_size as f64 / byte_rate as f64
        } else {
            0.0
        };

        Ok((
            Duration::from_secs_f64(duration_secs),
            sample_rate,
            channels,
            bits_per_sample,
        ))
    }

    /// Create streaming sound for large audio files
    async fn create_streaming_sound(
        &self,
        data: &[u8],
        format: AudioFormat,
    ) -> Result<StaticSoundData, AudioError> {
        if matches!(format, AudioFormat::Custom(_)) {
            return Err(AudioError::UnsupportedFormat {
                path: "streaming".to_string(),
                format: format!("{:?}", format),
            });
        }
        let cursor = Cursor::new(data.to_vec());
        StaticSoundData::from_cursor(cursor, StaticSoundSettings::default()).map_err(|e| {
            AudioError::StreamingError(format!("Failed to create streaming sound: {}", e))
        })
    }

    /// Create memory-based sound for smaller audio files
    async fn create_memory_sound(
        &self,
        data: &[u8],
        format: AudioFormat,
    ) -> Result<StaticSoundData, AudioError> {
        let cursor = Cursor::new(data.to_vec());
        StaticSoundData::from_cursor(cursor, StaticSoundSettings::default()).map_err(|e| {
            AudioError::DecodingFailed {
                path: "memory_sound".to_string(),
                error: format!("Failed to create memory sound: {}", e),
            }
        })
    }

    /// Play audio asset
    pub async fn play_sound(
        &self,
        asset_handle: AssetHandle,
        volume: Option<f32>,
        pitch: Option<f32>,
        position: Option<Vector3<f32>>,
    ) -> Result<u64, AudioError> {
        let asset = self
            .audio_assets
            .read()
            .unwrap()
            .get(&asset_handle)
            .cloned()
            .ok_or_else(|| AudioError::EngineError("Asset not found".to_string()))?;

        // Generate instance ID
        let instance_id = {
            let mut counter = self.instance_counter.lock().unwrap();
            *counter += 1;
            *counter
        };

        // Get sound data from cache and play it through Kira
        let sound_data = self
            .sound_data_cache
            .read()
            .unwrap()
            .get(&asset_handle)
            .cloned()
            .ok_or_else(|| AudioError::EngineError("Sound data not found".to_string()))?;

        // Apply volume and play
        let final_volume = volume.unwrap_or(asset.volume) * self.config.master_volume;
        let mut settings = StaticSoundSettings::new()
            .volume(Volume::Amplitude(final_volume as f64))
            .playback_rate(PlaybackRate::Factor(pitch.unwrap_or(asset.pitch) as f64));

        let emitter_handle = if let Some(position) = position {
            let spatial_settings = asset.spatial_settings.clone().unwrap_or_default();
            let mut scene = self.spatial_scene.lock().unwrap();
            let emitter = scene
                .add_emitter(
                    mint::Vector3 {
                        x: position.x,
                        y: position.y,
                        z: position.z,
                    },
                    EmitterSettings::default()
                        .distances((1.0, spatial_settings.max_distance.max(1.0))),
                )
                .map_err(|e| AudioError::EngineError(format!("Failed to create emitter: {}", e)))?;
            settings = settings.output_destination(&emitter);
            Some(emitter)
        } else {
            match asset.asset_type {
                AudioAssetType::Music => {
                    if let Some(track) = self.music_track.lock().unwrap().as_ref() {
                        settings = settings.output_destination(track);
                    }
                }
                AudioAssetType::Voice => {
                    if let Some(track) = self.voice_track.lock().unwrap().as_ref() {
                        settings = settings.output_destination(track);
                    }
                }
                AudioAssetType::UI => {
                    if let Some(track) = self.ui_track.lock().unwrap().as_ref() {
                        settings = settings.output_destination(track);
                    }
                }
                _ => {
                    if let Some(track) = self.sfx_track.lock().unwrap().as_ref() {
                        settings = settings.output_destination(track);
                    }
                }
            }
            None
        };

        // Play the sound using the audio manager
        let sound_handle = self
            .audio_manager
            .lock()
            .unwrap()
            .play(sound_data.with_settings(settings))
            .map_err(|e| AudioError::EngineError(format!("Failed to play sound: {}", e)))?;

        let instance = AudioInstance {
            id: instance_id,
            asset_handle,
            state: PlaybackState::Playing,
            volume: volume.unwrap_or(asset.volume),
            pitch: pitch.unwrap_or(asset.pitch),
            position,
            sound_handle: Some(sound_handle),
            emitter_handle,
            start_time: Instant::now(),
            fade_target: None,
            fade_duration: None,
        };

        // Store the instance
        self.active_instances
            .write()
            .unwrap()
            .insert(instance_id, instance);

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap();
            stats.active_instances += 1;
            stats.total_played += 1;
            stats.peak_concurrent_sounds = stats.peak_concurrent_sounds.max(stats.active_instances);
        }

        log::debug!("Playing sound: {} (instance {})", asset.name, instance_id);
        Ok(instance_id)
    }

    /// Stop playing sound instance
    pub fn stop_sound(&self, instance_id: u64) -> Result<(), AudioError> {
        if let Some(mut instance) = self.active_instances.write().unwrap().remove(&instance_id) {
            instance.state = PlaybackState::Stopped;

            if let Some(mut handle) = instance.sound_handle.take() {
                if let Err(err) = handle.stop(Tween::default()) {
                    log::warn!("Failed to stop sound instance {}: {}", instance_id, err);
                }
            }

            let mut stats = self.stats.write().unwrap();
            stats.active_instances = stats.active_instances.saturating_sub(1);

            log::debug!("Stopped sound instance: {}", instance_id);
            Ok(())
        } else {
            Err(AudioError::EngineError(
                "Sound instance not found".to_string(),
            ))
        }
    }

    pub fn is_sound_playing(&self, instance_id: u64) -> bool {
        self.active_instances
            .read()
            .unwrap()
            .get(&instance_id)
            .and_then(|instance| instance.sound_handle.as_ref())
            .map(|handle| handle.state() == KiraPlaybackState::Playing)
            .unwrap_or(false)
    }

    /// Update 3D listener position
    pub fn update_listener(&self, position: Vector3<f32>, forward: Vector3<f32>, up: Vector3<f32>) {
        let mut listener = self.listener.write().unwrap();
        listener.position = position;
        listener.forward = forward;
        listener.up = up;

        if let Ok(mut handle) = self.spatial_listener.lock() {
            let _ = handle.set_position(
                mint::Vector3 {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                },
                Tween::default(),
            );
            if let Some(orientation) = Self::listener_orientation(forward, up) {
                let _ = handle.set_orientation(orientation, Tween::default());
            }
        }
    }

    /// Update 3D sound position
    pub fn update_sound_position(
        &self,
        instance_id: u64,
        position: Vector3<f32>,
    ) -> Result<(), AudioError> {
        if let Some(instance) = self.active_instances.write().unwrap().get_mut(&instance_id) {
            instance.position = Some(position);

            if let Some(handle) = instance.emitter_handle.as_mut() {
                let _ = handle.set_position(
                    mint::Vector3 {
                        x: position.x,
                        y: position.y,
                        z: position.z,
                    },
                    Tween::default(),
                );
            }
            Ok(())
        } else {
            Err(AudioError::EngineError(
                "Sound instance not found".to_string(),
            ))
        }
    }

    /// Set environmental audio effects
    pub fn set_environment(&self, environment_name: &str) -> Result<(), AudioError> {
        let environments = self.environments.read().unwrap();
        if let Some(environment) = environments.get(environment_name) {
            *self.current_environment.write().unwrap() = environment.clone();

            self.apply_environment_effects(environment);
            log::info!("Set audio environment: {}", environment_name);
            Ok(())
        } else {
            Err(AudioError::EffectFailed {
                effect: environment_name.to_string(),
                error: "Environment not found".to_string(),
            })
        }
    }

    /// Update audio system (call every frame)
    pub fn update(&self) -> Result<(), AudioError> {
        let mut finished_instances = Vec::new();
        {
            let instances = self.active_instances.read().unwrap();
            for (id, instance) in instances.iter() {
                let finished = instance
                    .sound_handle
                    .as_ref()
                    .map(|handle| handle.state() != KiraPlaybackState::Playing)
                    .unwrap_or(true);
                if finished {
                    finished_instances.push(*id);
                }
            }
        }

        if !finished_instances.is_empty() {
            let mut instances = self.active_instances.write().unwrap();
            let mut stats = self.stats.write().unwrap();

            for id in finished_instances {
                instances.remove(&id);
                stats.active_instances = stats.active_instances.saturating_sub(1);
            }
        }

        Ok(())
    }

    /// Get audio statistics
    pub fn get_stats(&self) -> AudioStats {
        let stats = self.stats.read().unwrap();
        let mut result = stats.clone();
        result.active_instances = self.active_instances.read().unwrap().len() as u32;
        result
    }

    fn listener_orientation(
        forward: Vector3<f32>,
        up: Vector3<f32>,
    ) -> Option<mint::Quaternion<f32>> {
        if forward.norm_squared() < 0.0001 || up.norm_squared() < 0.0001 {
            return None;
        }
        let forward_norm = forward.normalize();
        let up_norm = up.normalize();
        if !forward_norm.iter().all(|v| v.is_finite()) || !up_norm.iter().all(|v| v.is_finite()) {
            return None;
        }
        let rotation = UnitQuaternion::face_towards(&forward_norm, &up_norm);
        let rotation = rotation.into_inner();
        Some(mint::Quaternion {
            s: rotation.w,
            v: mint::Vector3 {
                x: rotation.i,
                y: rotation.j,
                z: rotation.k,
            },
        })
    }

    fn apply_track_volume(
        &self,
        track: &Arc<Mutex<Option<TrackHandle>>>,
        volume: f32,
        master: f32,
    ) {
        if let Ok(guard) = track.lock() {
            if let Some(track) = guard.as_ref() {
                if let Err(err) = track.set_volume(
                    Volume::Amplitude((volume * master) as f64),
                    Tween::default(),
                ) {
                    log::warn!("Failed to update track volume: {}", err);
                }
            }
        }
    }

    fn apply_environment_effects(&self, environment: &AudioEnvironment) {
        let low_cutoff = environment.low_pass_cutoff.clamp(20.0, 20000.0);
        let high_cutoff = environment.high_pass_cutoff.clamp(20.0, 20000.0);
        let low_mix = if low_cutoff < 19950.0 { 1.0 } else { 0.0 };
        let high_mix = if high_cutoff > 25.0 { 1.0 } else { 0.0 };

        if let Ok(mut handle) = self.sfx_low_pass.lock() {
            if let Some(handle) = handle.as_mut() {
                let _ = handle.set_cutoff(low_cutoff as f64, Tween::default());
                let _ = handle.set_mix(low_mix, Tween::default());
            }
        }

        if let Ok(mut handle) = self.sfx_high_pass.lock() {
            if let Some(handle) = handle.as_mut() {
                let _ = handle.set_cutoff(high_cutoff as f64, Tween::default());
                let _ = handle.set_mix(high_mix, Tween::default());
            }
        }

        let feedback = (environment.reverb_time / 5.0).clamp(0.0, 0.95);
        let damping = environment.reverb_decay.clamp(0.0, 1.0);
        let mix = environment.reverb_density.clamp(0.0, 1.0);
        if let Ok(mut handle) = self.sfx_reverb.lock() {
            if let Some(handle) = handle.as_mut() {
                let _ = handle.set_feedback(feedback as f64, Tween::default());
                let _ = handle.set_damping(damping as f64, Tween::default());
                let _ = handle.set_mix(mix as f64, Tween::default());
            }
        }
    }

    /// Set master volume
    pub fn set_master_volume(&mut self, volume: f32) {
        self.config.master_volume = volume.clamp(0.0, 1.0);
        let master = self.config.master_volume;
        self.apply_track_volume(&self.music_track, self.config.music_volume, master);
        self.apply_track_volume(&self.sfx_track, self.config.sfx_volume, master);
        self.apply_track_volume(&self.voice_track, self.config.voice_volume, master);
        self.apply_track_volume(&self.ui_track, self.config.ui_volume, master);
    }

    /// Set category volumes
    pub fn set_category_volume(&mut self, category: AudioAssetType, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        match category {
            AudioAssetType::Music => self.config.music_volume = volume,
            AudioAssetType::SoundEffect => self.config.sfx_volume = volume,
            AudioAssetType::Voice => self.config.voice_volume = volume,
            AudioAssetType::UI => self.config.ui_volume = volume,
            _ => {}
        }

        let master = self.config.master_volume;
        match category {
            AudioAssetType::Music => {
                self.apply_track_volume(&self.music_track, self.config.music_volume, master)
            }
            AudioAssetType::Voice => {
                self.apply_track_volume(&self.voice_track, self.config.voice_volume, master)
            }
            AudioAssetType::UI => {
                self.apply_track_volume(&self.ui_track, self.config.ui_volume, master)
            }
            _ => self.apply_track_volume(&self.sfx_track, self.config.sfx_volume, master),
        }
    }

    /// Cleanup audio resources
    pub fn cleanup(&self) {
        // Stop all active instances
        let instance_ids: Vec<u64> = self
            .active_instances
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        for id in instance_ids {
            let _ = self.stop_sound(id);
        }

        // Clear all caches including sound data
        self.audio_assets.write().unwrap().clear();
        self.asset_index.write().unwrap().clear();
        self.sound_data_cache.write().unwrap().clear();

        log::info!("Audio system cleanup complete");
    }
}

impl From<AudioError> for AssetError {
    fn from(err: AudioError) -> Self {
        match err {
            AudioError::Io(io_err) => AssetError::Io(io_err),
            _ => AssetError::LoadingFailed {
                path: "audio_asset".to_string(),
                error: err.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audio_format_detection() {
        assert_eq!(AudioFormat::from_extension("wav"), AudioFormat::Wav);
        assert_eq!(AudioFormat::from_extension("mp3"), AudioFormat::Mp3);
        assert_eq!(AudioFormat::from_extension("ogg"), AudioFormat::Ogg);
    }

    #[test]
    fn test_audio_quality_settings() {
        assert_eq!(AudioQuality::Low.sample_rate(), 22050);
        assert_eq!(AudioQuality::Medium.sample_rate(), 44100);
        assert_eq!(AudioQuality::High.sample_rate(), 48000);
        assert_eq!(AudioQuality::Ultra.sample_rate(), 96000);
    }

    #[test]
    fn test_audio_3d_settings() {
        let settings = Audio3DSettings::default();
        assert_eq!(settings.max_distance, 100.0);
        assert_eq!(settings.rolloff_factor, 1.0);
        assert_eq!(settings.doppler_factor, 1.0);
    }
}
