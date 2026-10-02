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

use glam::{Mat3, Quat, Vec3};
use kira::{
    AudioManager, AudioManagerSettings, Decibels, Tween,
    effect::{
        filter::{FilterBuilder, FilterHandle, FilterMode},
        reverb::{ReverbBuilder, ReverbHandle},
    },
    listener::ListenerHandle,
    sound::PlaybackState as KiraPlaybackState,
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
    track::{SpatialTrackBuilder, SpatialTrackHandle, TrackBuilder, TrackHandle},
};

fn kira_amplitude(amp: f64) -> Decibels {
    if amp <= 0.0001 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * (amp as f32).log10())
    }
}
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use thiserror::Error;

use super::{AssetError, AssetHandle};

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
    pub position: Vec3,
    pub velocity: Vec3,
    pub orientation: Vec3,
    pub min_distance: f32,
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
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            orientation: Vec3::new(0.0, 0.0, -1.0),
            min_distance: 25.0,
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
    pub position: Vec3,
    pub velocity: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub gain: f32,
}

impl Default for AudioListener {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            forward: Vec3::new(0.0, 0.0, -1.0),
            up: Vec3::new(0.0, 1.0, 0.0),
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
    pub position: Option<Vec3>,
    pub sound_handle: Option<StaticSoundHandle>,
    pub emitter_handle: Option<SpatialTrackHandle>,
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

/// Kira mixer handles owned outright by the loader.
///
/// Kira types every handle mutator as `&mut self` even though the handles are
/// command writers into the audio thread — the `&mut` is the API's aliasing
/// convention, not a real mutation — so these handles cannot be shared as
/// `&TrackHandle`. They are grouped under the loader's single mixer lock
/// instead of one lock per handle.
struct Mixer {
    /// Keeps the audio backend alive: kira stops output when the manager is
    /// dropped. Playback goes through the track handles below.
    _audio_manager: AudioManager,
    music_track: TrackHandle,
    sfx_track: TrackHandle,
    voice_track: TrackHandle,
    ui_track: TrackHandle,
    sfx_low_pass: FilterHandle,
    sfx_high_pass: FilterHandle,
    sfx_reverb: ReverbHandle,
    spatial_listener: ListenerHandle,
}

impl Mixer {
    fn track_mut(&mut self, asset_type: AudioAssetType) -> &mut TrackHandle {
        match asset_type {
            AudioAssetType::Music => &mut self.music_track,
            AudioAssetType::Voice => &mut self.voice_track,
            AudioAssetType::UI => &mut self.ui_track,
            _ => &mut self.sfx_track,
        }
    }

    /// Play decoded audio on the mixer track for `asset_type`.
    fn play(
        &mut self,
        asset_type: AudioAssetType,
        sound_data: StaticSoundData,
    ) -> Result<StaticSoundHandle, AudioError> {
        self.track_mut(asset_type)
            .play(sound_data)
            .map_err(|e| AudioError::EngineError(format!("Failed to play sound: {}", e)))
    }

    /// Play decoded audio on a fresh spatial track under the main mix.
    fn play_spatial(
        &mut self,
        sound_data: StaticSoundData,
        position: Vec3,
        min_distance: f32,
        max_distance: f32,
    ) -> Result<(StaticSoundHandle, SpatialTrackHandle), AudioError> {
        let listener_id = self.spatial_listener.id();
        let mut spatial = self
            ._audio_manager
            .add_spatial_sub_track(
                listener_id,
                mint::Vector3 {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                },
                SpatialTrackBuilder::new()
                    .distances((min_distance, max_distance))
                    .persist_until_sounds_finish(true),
            )
            .map_err(|e| {
                AudioError::EngineError(format!("Failed to create spatial track: {}", e))
            })?;
        let handle = spatial
            .play(sound_data)
            .map_err(|e| AudioError::EngineError(format!("Failed to play sound: {}", e)))?;
        Ok((handle, spatial))
    }

    fn set_track_volume(&mut self, asset_type: AudioAssetType, volume: f32, master: f32) {
        self.track_mut(asset_type)
            .set_volume(kira_amplitude((volume * master) as f64), Tween::default());
    }

    fn apply_environment(&mut self, environment: &AudioEnvironment) {
        let low_cutoff = environment.low_pass_cutoff.clamp(20.0, 20000.0);
        let high_cutoff = environment.high_pass_cutoff.clamp(20.0, 20000.0);
        let low_mix = if low_cutoff < 19950.0 { 1.0 } else { 0.0 };
        let high_mix = if high_cutoff > 25.0 { 1.0 } else { 0.0 };

        let _ = self
            .sfx_low_pass
            .set_cutoff(low_cutoff as f64, Tween::default());
        let _ = self.sfx_low_pass.set_mix(low_mix, Tween::default());
        let _ = self
            .sfx_high_pass
            .set_cutoff(high_cutoff as f64, Tween::default());
        let _ = self.sfx_high_pass.set_mix(high_mix, Tween::default());

        let feedback = (environment.reverb_time / 5.0).clamp(0.0, 0.95);
        let damping = environment.reverb_decay.clamp(0.0, 1.0);
        let mix = environment.reverb_density.clamp(0.0, 1.0);
        let _ = self
            .sfx_reverb
            .set_feedback(feedback as f64, Tween::default());
        let _ = self
            .sfx_reverb
            .set_damping(damping as f64, Tween::default());
        let _ = self.sfx_reverb.set_mix(mix, Tween::default());
    }
}

/// Loaded audio: asset metadata, the path index used for cache hits, and the
/// decoded samples playback consumes. One cell so a load publishes all three
/// maps together.
struct AudioStore {
    assets: HashMap<AssetHandle, Arc<AudioAsset>>,
    index: HashMap<PathBuf, AssetHandle>,
    sound_data: HashMap<AssetHandle, StaticSoundData>,
}

impl AudioStore {
    fn new() -> Self {
        Self {
            assets: HashMap::new(),
            index: HashMap::new(),
            sound_data: HashMap::new(),
        }
    }
}

/// Complete Audio System
pub struct AudioLoader {
    /// Kira mixer handles (tracks, filter/reverb effects, spatial listener and
    /// the backend itself).
    // THREAD: audio work runs on tokio worker threads — assets/audio_bridge
    // spawns one task per sound on the process runtime — while the game thread
    // polls `is_sound_playing` (audio_bridge `is_playing`) and load tasks
    // publish decoded assets. Those tasks are the only genuinely concurrent
    // users of this struct, and this is the one lock they must share.
    mixer: Mutex<Mixer>,

    /// Loaded assets: metadata, path index and decoded sound data.
    // THREAD: producer→consumer handoff — load tasks write, play tasks read.
    store: RwLock<AudioStore>,

    /// Live playback instances keyed by instance id.
    // THREAD: play/stop/pause run on tokio audio tasks, `is_sound_playing` is
    // polled on the game thread.
    active_instances: RwLock<HashMap<u64, AudioInstance>>,

    /// Cumulative telemetry counters.
    // THREAD: incremented by concurrent load/play tasks, read by `get_stats`.
    stats: RwLock<AudioStats>,

    /// Monotonic instance-id source (`fetch_add` — no lock needed).
    instance_counter: AtomicU64,

    // 3D audio system — kira's own `ListenerHandle` is the live copy.
    listener: AudioListener,

    // Environmental effects
    current_environment: AudioEnvironment,
    environments: HashMap<String, AudioEnvironment>,

    // Configuration
    config: AudioConfig,
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

        let spatial_listener = audio_manager
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
            mixer: Mutex::new(Mixer {
                _audio_manager: audio_manager,
                music_track,
                sfx_track,
                voice_track,
                ui_track,
                sfx_low_pass,
                sfx_high_pass,
                sfx_reverb,
                spatial_listener,
            }),
            store: RwLock::new(AudioStore::new()),
            active_instances: RwLock::new(HashMap::new()),
            stats: RwLock::new(AudioStats::default()),
            instance_counter: AtomicU64::new(1),
            listener: AudioListener::default(),
            current_environment: AudioEnvironment::default(),
            environments,
            config,
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
        if let Some(cached_handle) = {
            let store = self.store.read().unwrap_or_else(|e| e.into_inner());
            store.index.get(path).copied()
        } {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.cache_hits += 1;
            return Ok(cached_handle);
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

        // Publish metadata, path index and decoded data as one step.
        {
            let mut store = self.store.write().unwrap_or_else(|e| e.into_inner());
            store.assets.insert(handle, asset_arc);
            store.index.insert(path.to_path_buf(), handle);
            store.sound_data.insert(handle, sound_data);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
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
        StaticSoundData::from_cursor(cursor).map_err(|e| {
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
        StaticSoundData::from_cursor(cursor).map_err(|e| AudioError::DecodingFailed {
            path: "memory_sound".to_string(),
            error: format!("Failed to create memory sound: {}", e),
        })
    }

    /// Play audio asset
    pub async fn play_sound(
        &self,
        asset_handle: AssetHandle,
        volume: Option<f32>,
        pitch: Option<f32>,
        position: Option<Vec3>,
    ) -> Result<u64, AudioError> {
        // Look the asset and its decoded data up in one pass.
        let (asset, sound_data, instance_id) = {
            let store = self.store.read().unwrap_or_else(|e| e.into_inner());
            let asset = store
                .assets
                .get(&asset_handle)
                .cloned()
                .ok_or_else(|| AudioError::EngineError("Asset not found".to_string()))?;

            // Generate instance ID
            let instance_id = self.instance_counter.fetch_add(1, Ordering::Relaxed) + 1;

            let sound_data = store
                .sound_data
                .get(&asset_handle)
                .cloned()
                .ok_or_else(|| AudioError::EngineError("Sound data not found".to_string()))?;
            (asset, sound_data, instance_id)
        };

        // Apply volume and play
        let final_volume = volume.unwrap_or(asset.volume) * self.config.master_volume;
        let settings = StaticSoundSettings::new()
            .volume(kira_amplitude(final_volume as f64))
            .playback_rate(pitch.unwrap_or(asset.pitch) as f64);
        let sound_data = sound_data.with_settings(settings);

        let (sound_handle, emitter_handle) = if let Some(position) = position {
            let spatial_settings = asset.spatial_settings.clone().unwrap_or_default();
            let min_distance = spatial_settings.min_distance.max(1.0);
            let max_distance = spatial_settings.max_distance.max(min_distance);
            let mut mixer = self.mixer.lock().unwrap_or_else(|e| e.into_inner());
            mixer
                .play_spatial(sound_data, position, min_distance, max_distance)
                .map(|(handle, emitter)| (handle, Some(emitter)))?
        } else {
            let asset_type = asset.asset_type;
            let mut mixer = self.mixer.lock().unwrap_or_else(|e| e.into_inner());
            (mixer.play(asset_type, sound_data)?, None)
        };

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
            .unwrap_or_else(|e| e.into_inner())
            .insert(instance_id, instance);

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.active_instances += 1;
            stats.total_played += 1;
            stats.peak_concurrent_sounds = stats.peak_concurrent_sounds.max(stats.active_instances);
        }

        log::debug!("Playing sound: {} (instance {})", asset.name, instance_id);
        Ok(instance_id)
    }

    /// Stop playing sound instance
    pub fn stop_sound(&self, instance_id: u64) -> Result<(), AudioError> {
        if let Some(mut instance) = self
            .active_instances
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&instance_id)
        {
            instance.state = PlaybackState::Stopped;

            if let Some(mut handle) = instance.sound_handle.take() {
                handle.stop(Tween::default());
            }

            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.active_instances = stats.active_instances.saturating_sub(1);

            log::debug!("Stopped sound instance: {}", instance_id);
            Ok(())
        } else {
            Err(AudioError::EngineError(
                "Sound instance not found".to_string(),
            ))
        }
    }

    /// Pause a playing sound instance.
    pub fn pause_sound(&self, instance_id: u64) -> Result<(), AudioError> {
        if let Some(instance) = self
            .active_instances
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&instance_id)
        {
            instance.state = PlaybackState::Paused;
            if let Some(handle) = instance.sound_handle.as_mut() {
                handle.pause(Tween::default());
            }
            log::debug!("Paused sound instance: {}", instance_id);
            Ok(())
        } else {
            Err(AudioError::EngineError(
                "Sound instance not found".to_string(),
            ))
        }
    }

    /// Resume a paused sound instance.
    pub fn resume_sound(&self, instance_id: u64) -> Result<(), AudioError> {
        if let Some(instance) = self
            .active_instances
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&instance_id)
        {
            instance.state = PlaybackState::Playing;
            if let Some(handle) = instance.sound_handle.as_mut() {
                handle.resume(Tween::default());
            }
            log::debug!("Resumed sound instance: {}", instance_id);
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
            .unwrap_or_else(|e| e.into_inner())
            .get(&instance_id)
            .and_then(|instance| instance.sound_handle.as_ref())
            .map(|handle| {
                let state = handle.state();
                state == KiraPlaybackState::Playing || state == KiraPlaybackState::Paused
            })
            .unwrap_or(false)
    }

    /// Update the gain of a playing sound instance.
    pub fn set_sound_volume(&self, instance_id: u64, volume: f32) -> Result<(), AudioError> {
        let mut instances = self
            .active_instances
            .write()
            .unwrap_or_else(|e| e.into_inner());
        let instance = instances
            .get_mut(&instance_id)
            .ok_or_else(|| AudioError::EngineError("Sound instance not found".to_string()))?;

        let volume = volume.clamp(0.0, 1.0);
        instance.volume = volume;

        if let Some(handle) = instance.sound_handle.as_mut() {
            handle.set_volume(
                kira_amplitude((volume * self.config.master_volume) as f64),
                Tween::default(),
            );
        }

        Ok(())
    }

    /// Update 3D listener position
    pub fn update_listener(&mut self, position: Vec3, forward: Vec3, up: Vec3) {
        self.listener.position = position;
        self.listener.forward = forward;
        self.listener.up = up;

        // THREAD: kira's `ListenerHandle` is only reachable behind the mixer
        // lock; the listener may also be moved from a tokio audio task.
        if let Ok(mut mixer) = self.mixer.lock() {
            let _ = mixer.spatial_listener.set_position(
                mint::Vector3 {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                },
                Tween::default(),
            );
            if let Some(orientation) = Self::listener_orientation(forward, up) {
                let _ = mixer
                    .spatial_listener
                    .set_orientation(orientation, Tween::default());
            }
        }
    }

    /// Update 3D sound position
    pub fn update_sound_position(
        &self,
        instance_id: u64,
        position: Vec3,
    ) -> Result<(), AudioError> {
        if let Some(instance) = self
            .active_instances
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&instance_id)
        {
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
    pub fn set_environment(&mut self, environment_name: &str) -> Result<(), AudioError> {
        let Some(environment) = self.environments.get(environment_name).cloned() else {
            return Err(AudioError::EffectFailed {
                effect: environment_name.to_string(),
                error: "Environment not found".to_string(),
            });
        };

        self.current_environment = environment.clone();

        // THREAD: the effect handles live behind the mixer lock (see
        // `Self::mixer`) because play tasks read the same block.
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.apply_environment(&environment);
        }
        log::info!("Set audio environment: {}", environment_name);
        Ok(())
    }

    /// Update audio system (call every frame)
    pub fn update(&self) -> Result<(), AudioError> {
        let mut finished_instances = Vec::new();
        {
            let instances = self
                .active_instances
                .read()
                .unwrap_or_else(|e| e.into_inner());
            for (id, instance) in instances.iter() {
                let state = instance.sound_handle.as_ref().map(|handle| handle.state());
                let finished = state
                    .map(|s| s != KiraPlaybackState::Playing && s != KiraPlaybackState::Paused)
                    .unwrap_or(true);
                if finished {
                    finished_instances.push(*id);
                }
            }
        }

        if !finished_instances.is_empty() {
            let mut instances = self
                .active_instances
                .write()
                .unwrap_or_else(|e| e.into_inner());
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());

            for id in finished_instances {
                instances.remove(&id);
                stats.active_instances = stats.active_instances.saturating_sub(1);
            }
        }

        Ok(())
    }

    /// Get audio statistics
    pub fn get_stats(&self) -> AudioStats {
        let stats = self.stats.read().unwrap_or_else(|e| e.into_inner());
        let mut result = stats.clone();
        result.active_instances = self
            .active_instances
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .len() as u32;
        result
    }

    fn listener_orientation(forward: Vec3, up: Vec3) -> Option<mint::Quaternion<f32>> {
        if forward.length_squared() < 0.0001 || up.length_squared() < 0.0001 {
            return None;
        }
        let forward_norm = forward.normalize();
        let up_norm = up.normalize();
        if !forward_norm.is_finite() || !up_norm.is_finite() {
            return None;
        }
        let z = forward_norm;
        let x = up_norm.cross(z).normalize();
        let y = z.cross(x);
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let rotation = Quat::from_mat3(&Mat3::from_cols(x, y, z));
        Some(mint::Quaternion {
            s: rotation.w,
            v: mint::Vector3 {
                x: rotation.x,
                y: rotation.y,
                z: rotation.z,
            },
        })
    }

    /// Push the configured category volumes onto the mixer tracks.
    fn apply_track_volumes(&self) {
        let master = self.config.master_volume;
        let music = self.config.music_volume;
        let sfx = self.config.sfx_volume;
        let voice = self.config.voice_volume;
        let ui = self.config.ui_volume;

        // THREAD: track handles live behind the mixer lock (see `Self::mixer`).
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.set_track_volume(AudioAssetType::Music, music, master);
            mixer.set_track_volume(AudioAssetType::SoundEffect, sfx, master);
            mixer.set_track_volume(AudioAssetType::Voice, voice, master);
            mixer.set_track_volume(AudioAssetType::UI, ui, master);
        }
    }

    /// Set master volume
    pub fn set_master_volume(&mut self, volume: f32) {
        self.config.master_volume = volume.clamp(0.0, 1.0);
        self.apply_track_volumes();
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
        let category_volume = match category {
            AudioAssetType::Music => self.config.music_volume,
            AudioAssetType::Voice => self.config.voice_volume,
            AudioAssetType::UI => self.config.ui_volume,
            _ => self.config.sfx_volume,
        };

        // THREAD: track handles live behind the mixer lock (see `Self::mixer`).
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.set_track_volume(category, category_volume, master);
        }
    }

    /// Cleanup audio resources
    pub fn cleanup(&self) {
        // Stop all active instances
        let instance_ids: Vec<u64> = self
            .active_instances
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
        for id in instance_ids {
            let _ = self.stop_sound(id);
        }

        // Clear all caches including sound data
        {
            let mut store = self.store.write().unwrap_or_else(|e| e.into_inner());
            store.assets.clear();
            store.index.clear();
            store.sound_data.clear();
        }

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
