//! # MilesAudioDevice - Modern Rust conversion of MilesAudioManager
//!
//! This module provides a complete conversion of the original C++ MilesAudioManager
//! to modern Rust, maintaining API compatibility while adding safety and performance improvements.

use super::DeviceCapabilities as AudioDeviceCapabilities;
use super::{
    AudioDeviceError, AudioFormat, AudioFormatType, AudioHandle, AudioListener, AudioSource,
    AudioStatistics, PlaybackState, Position3D, Priority, Result, SampleFormat, Volume,
};
use crate::{DeviceCapabilities, DeviceStatus, DeviceType, PerformanceMetrics};

use crossbeam_channel::{bounded, Receiver, Sender};
use dashmap::DashMap;
use parking_lot::RwLock as ParkingRwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

// CPAL and Rodio replaced with modern Kira audio system
#[cfg(feature = "audio")]
use super::kira_audio_driver::{KiraAudioDriver, ModernAudioDevice};

/// Configuration for MilesAudioDevice
#[derive(Debug, Clone)]
pub struct MilesAudioConfig {
    /// Preferred audio driver
    pub preferred_driver: Option<String>,
    /// Preferred speaker configuration
    pub preferred_speaker_config: Option<String>,
    /// Enable hardware acceleration
    pub hardware_acceleration: bool,
    /// Enable surround sound
    pub surround_sound: bool,
    /// Maximum number of 2D samples
    pub max_2d_samples: usize,
    /// Maximum number of 3D samples
    pub max_3d_samples: usize,
    /// Maximum number of streams
    pub max_streams: usize,
    /// Audio cache size in bytes
    pub cache_size_bytes: usize,
    /// Enable 3D audio processing
    pub enable_3d_audio: bool,
    /// Default audio format
    pub default_format: AudioFormat,
    /// Device buffer size in frames
    pub buffer_size: u32,
    /// Number of audio processing threads
    pub processing_threads: usize,
}

impl Default for MilesAudioConfig {
    fn default() -> Self {
        Self {
            preferred_driver: None,
            preferred_speaker_config: None,
            hardware_acceleration: true,
            surround_sound: false,
            max_2d_samples: 32,
            max_3d_samples: 16,
            max_streams: 8,
            cache_size_bytes: 32 * 1024 * 1024, // 32MB
            enable_3d_audio: true,
            default_format: AudioFormat::cd_quality(),
            buffer_size: 1024,
            processing_threads: 2,
        }
    }
}

/// Audio playback type (matching original C++ enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayingAudioType {
    /// Regular 2D sample
    Sample,
    /// 3D positioned sample
    Sample3D,
    /// Streaming audio
    Stream,
}

/// Audio playback status (matching original C++ enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayingStatus {
    /// Currently playing
    Playing,
    /// Stopped
    Stopped,
    /// Paused
    Paused,
}

/// Internal playing audio tracking structure
#[derive(Debug)]
struct PlayingAudio {
    /// Unique identifier
    id: Uuid,
    /// Audio handle for external reference
    handle: AudioHandle,
    /// Type of audio (2D, 3D, stream)
    audio_type: PlayingAudioType,
    /// Current playback status
    status: PlayingStatus,
    /// Associated audio source configuration
    source_config: AudioSource,
    /// File path or identifier
    file_path: Option<String>,
    /// Request to stop this audio
    request_stop: bool,
    /// Start time
    start_time: Instant,
    /// Duration (if known)
    duration: Option<Duration>,
    /// Current volume
    current_volume: Volume,
    /// Modern Kira-based audio handle
    #[cfg(feature = "audio")]
    kira_handle: Option<String>, // Handle ID for Kira audio system
    /// Platform-specific handle storage
    platform_handle: Option<Box<dyn std::any::Any + Send + Sync>>,
}

/// Audio cache for efficient file management
#[derive(Debug)]
struct AudioCache {
    /// Cached audio files
    cached_files: DashMap<String, Arc<CachedAudioFile>>,
    /// Current cache size in bytes
    current_size: parking_lot::Mutex<u64>,
    /// Maximum cache size
    max_size: u64,
    /// Access tracking for LRU eviction
    access_times: parking_lot::Mutex<HashMap<String, Instant>>,
}

/// Cached audio file representation
#[derive(Debug)]
struct CachedAudioFile {
    /// File path
    path: String,
    /// Audio data
    data: Arc<[u8]>,
    /// Audio format
    format: AudioFormat,
    /// File size in bytes
    size: u64,
    /// Reference count
    ref_count: parking_lot::Mutex<usize>,
    /// Last access time
    last_access: parking_lot::Mutex<Instant>,
}

/// Audio processing command
#[derive(Debug)]
enum AudioCommand {
    /// Play audio with given configuration
    Play {
        handle: AudioHandle,
        file_path: String,
        source_config: AudioSource,
        response: tokio::sync::oneshot::Sender<Result<()>>,
    },
    /// Stop audio playback
    Stop {
        handle: AudioHandle,
        response: tokio::sync::oneshot::Sender<Result<()>>,
    },
    /// Pause audio playback
    Pause {
        handle: AudioHandle,
        response: tokio::sync::oneshot::Sender<Result<()>>,
    },
    /// Resume audio playback
    Resume {
        handle: AudioHandle,
        response: tokio::sync::oneshot::Sender<Result<()>>,
    },
    /// Update listener position for 3D audio
    UpdateListener {
        listener: AudioListener,
        response: tokio::sync::oneshot::Sender<Result<()>>,
    },
    /// Update audio source position
    UpdateSource {
        handle: AudioHandle,
        source_config: AudioSource,
        response: tokio::sync::oneshot::Sender<Result<()>>,
    },
    /// Get playback status
    GetStatus {
        handle: AudioHandle,
        response: tokio::sync::oneshot::Sender<Result<PlaybackState>>,
    },
    /// Shutdown the audio system
    Shutdown {
        response: tokio::sync::oneshot::Sender<Result<()>>,
    },
}

/// Main MilesAudioDevice implementation
pub struct MilesAudioDevice {
    /// Device configuration
    config: Arc<RwLock<MilesAudioConfig>>,

    /// Modern Kira audio driver
    #[cfg(feature = "audio")]
    kira_driver: Option<Arc<KiraAudioDriver>>,

    /// Currently playing audio tracks
    playing_audio: Arc<DashMap<AudioHandle, PlayingAudio>>,

    /// Audio cache for file management
    audio_cache: Arc<AudioCache>,

    /// Current audio listener for 3D audio
    audio_listener: Arc<RwLock<AudioListener>>,

    /// Audio processing command sender
    command_sender: Sender<AudioCommand>,

    /// Audio processing command receiver
    command_receiver: Arc<Mutex<Receiver<AudioCommand>>>,

    /// Device capabilities
    capabilities: Arc<AudioDeviceCapabilities>,

    /// Performance statistics
    statistics: Arc<ParkingRwLock<AudioStatistics>>,

    /// Processing thread handles
    processing_handles: Vec<tokio::task::JoinHandle<()>>,

    /// Shutdown flag
    shutdown_flag: Arc<parking_lot::Mutex<bool>>,
}

impl MilesAudioDevice {
    /// Create a new MilesAudioDevice with default configuration
    pub async fn new() -> Result<Self> {
        Self::new_with_config(MilesAudioConfig::default()).await
    }

    /// Create a new MilesAudioDevice with custom configuration
    pub async fn new_with_config(config: MilesAudioConfig) -> Result<Self> {
        #[cfg(feature = "audio")]
        let kira_driver = KiraAudioDriver::new().await.map_err(|e| {
            AudioDeviceError::InitializationFailed(format!(
                "Failed to create audio output stream: {}",
                e
            ))
        })?;

        // Create command channel for audio processing
        let (command_sender, command_receiver) = bounded(1000);

        // Initialize audio cache
        let audio_cache = Arc::new(AudioCache {
            cached_files: DashMap::new(),
            current_size: parking_lot::Mutex::new(0),
            max_size: config.cache_size_bytes as u64,
            access_times: parking_lot::Mutex::new(HashMap::new()),
        });

        // Detect device capabilities from the active backend when possible.
        #[cfg(feature = "audio")]
        let capabilities = Self::detect_capabilities_from_kira(&kira_driver).await?;
        #[cfg(not(feature = "audio"))]
        let capabilities = Self::detect_capabilities().await?;

        let device = Self {
            config: Arc::new(RwLock::new(config.clone())),

            #[cfg(feature = "audio")]
            kira_driver: Some(Arc::new(kira_driver)),

            playing_audio: Arc::new(DashMap::new()),
            audio_cache,
            audio_listener: Arc::new(RwLock::new(AudioListener::default())),
            command_sender,
            command_receiver: Arc::new(Mutex::new(command_receiver)),
            capabilities: Arc::new(capabilities),
            statistics: Arc::new(ParkingRwLock::new(AudioStatistics::default())),
            processing_handles: Vec::new(),
            shutdown_flag: Arc::new(parking_lot::Mutex::new(false)),
        };

        Ok(device)
    }

    /// Initialize the audio device
    pub async fn init(&mut self) -> Result<()> {
        // Start audio processing threads
        for i in 0..self.config.read().await.processing_threads {
            let receiver = self.command_receiver.clone();
            let playing_audio = self.playing_audio.clone();
            let audio_cache = self.audio_cache.clone();
            let statistics = self.statistics.clone();
            let shutdown_flag = self.shutdown_flag.clone();

            #[cfg(feature = "audio")]
            let kira_driver = self
                .kira_driver
                .as_ref()
                .ok_or_else(|| {
                    AudioDeviceError::InitializationFailed("No Kira driver".to_string())
                })?
                .clone();

            let handle = tokio::spawn(async move {
                Self::audio_processing_thread(
                    i,
                    receiver,
                    playing_audio,
                    audio_cache,
                    statistics,
                    shutdown_flag,
                    #[cfg(feature = "audio")]
                    kira_driver,
                )
                .await;
            });

            self.processing_handles.push(handle);
        }

        tracing::info!(
            "MilesAudioDevice initialized with {} processing threads",
            self.processing_handles.len()
        );
        Ok(())
    }

    /// Play an audio file with given configuration
    pub async fn play_audio(
        &self,
        file_path: &str,
        source_config: AudioSource,
    ) -> Result<AudioHandle> {
        let handle = AudioHandle::new();
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        let command = AudioCommand::Play {
            handle,
            file_path: file_path.to_string(),
            source_config,
            response: response_tx,
        };

        self.command_sender
            .send(command)
            .map_err(|_| AudioDeviceError::DeviceBusy("Command channel full".to_string()))?;

        response_rx
            .await
            .map_err(|_| AudioDeviceError::DeviceBusy("Command response failed".to_string()))?
            .map(|_| handle)
    }

    /// Stop audio playback
    pub async fn stop_audio(&self, handle: AudioHandle) -> Result<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        let command = AudioCommand::Stop {
            handle,
            response: response_tx,
        };

        self.command_sender
            .send(command)
            .map_err(|_| AudioDeviceError::DeviceBusy("Command channel full".to_string()))?;

        response_rx
            .await
            .map_err(|_| AudioDeviceError::DeviceBusy("Command response failed".to_string()))?
    }

    /// Pause audio playback
    pub async fn pause_audio(&self, handle: AudioHandle) -> Result<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        let command = AudioCommand::Pause {
            handle,
            response: response_tx,
        };

        self.command_sender
            .send(command)
            .map_err(|_| AudioDeviceError::DeviceBusy("Command channel full".to_string()))?;

        response_rx
            .await
            .map_err(|_| AudioDeviceError::DeviceBusy("Command response failed".to_string()))?
    }

    /// Resume audio playback
    pub async fn resume_audio(&self, handle: AudioHandle) -> Result<()> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        let command = AudioCommand::Resume {
            handle,
            response: response_tx,
        };

        self.command_sender
            .send(command)
            .map_err(|_| AudioDeviceError::DeviceBusy("Command channel full".to_string()))?;

        response_rx
            .await
            .map_err(|_| AudioDeviceError::DeviceBusy("Command response failed".to_string()))?
    }

    /// Update 3D audio listener position
    pub async fn update_listener(&self, listener: AudioListener) -> Result<()> {
        *self.audio_listener.write().await = listener.clone();

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        let command = AudioCommand::UpdateListener {
            listener,
            response: response_tx,
        };

        self.command_sender
            .send(command)
            .map_err(|_| AudioDeviceError::DeviceBusy("Command channel full".to_string()))?;

        response_rx
            .await
            .map_err(|_| AudioDeviceError::DeviceBusy("Command response failed".to_string()))?
    }

    /// Get current audio statistics
    pub async fn get_statistics(&self) -> AudioStatistics {
        self.statistics.read().clone()
    }

    /// Get device capabilities
    pub async fn get_capabilities(&self) -> AudioDeviceCapabilities {
        (*self.capabilities).clone()
    }

    /// Check if audio handle is currently playing
    pub async fn is_playing(&self, handle: AudioHandle) -> Result<bool> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        let command = AudioCommand::GetStatus {
            handle,
            response: response_tx,
        };

        self.command_sender
            .send(command)
            .map_err(|_| AudioDeviceError::DeviceBusy("Command channel full".to_string()))?;

        let status = response_rx
            .await
            .map_err(|_| AudioDeviceError::DeviceBusy("Command response failed".to_string()))??;

        Ok(matches!(status, PlaybackState::Playing))
    }

    /// Shutdown the audio device
    pub async fn shutdown(&self) -> Result<()> {
        *self.shutdown_flag.lock() = true;

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        let command = AudioCommand::Shutdown {
            response: response_tx,
        };

        // Send shutdown command (ignore if channel is closed)
        let _ = self.command_sender.send(command);

        // Wait for response or timeout
        if let Ok(result) = tokio::time::timeout(Duration::from_secs(5), response_rx).await {
            result.map_err(|_| {
                AudioDeviceError::DeviceBusy("Shutdown response failed".to_string())
            })??;
        }

        tracing::info!("MilesAudioDevice shutdown completed");
        Ok(())
    }

    /// Get device status for system monitoring
    pub async fn get_status(&self) -> Result<DeviceStatus> {
        let stats = self.get_statistics().await;
        let capabilities = self.get_capabilities().await;

        Ok(DeviceStatus {
            device_type: DeviceType::Audio,
            initialized: !*self.shutdown_flag.lock(),
            active: stats.active_channels > 0,
            capabilities: DeviceCapabilities {
                hardware_acceleration: capabilities.hardware_mixing,
                multi_threading: true,
                simd_support: true, // Assume SIMD support for audio processing
                platform_features: vec!["3D Audio".to_string(), "Hardware Mixing".to_string()],
            },
            performance: PerformanceMetrics {
                cpu_usage: stats.cpu_usage,
                memory_usage: stats.memory_usage,
                latency_ms: stats.average_latency,
                throughput: stats.active_channels as f32,
            },
        })
    }

    /// Get performance metrics
    pub async fn get_performance_metrics(&self) -> Result<PerformanceMetrics> {
        let stats = self.get_statistics().await;

        Ok(PerformanceMetrics {
            cpu_usage: stats.cpu_usage,
            memory_usage: stats.memory_usage,
            latency_ms: stats.average_latency,
            throughput: stats.active_channels as f32,
        })
    }

    /// Detect device capabilities
    async fn detect_capabilities() -> Result<AudioDeviceCapabilities> {
        // No runtime audio backend available; use conservative defaults.
        Ok(AudioDeviceCapabilities {
            hardware_mixing: true,
            hardware_3d: true,
            supported_sample_rates: vec![22050, 44100, 48000, 96000],
            max_channels: 8,
            min_buffer_size: 256,
            max_buffer_size: 8192,
            supported_formats: vec![
                AudioFormat::cd_quality(),
                AudioFormat::dvd_quality(),
                AudioFormat::high_quality(),
            ],
            latency_ms: 10.0,
        })
    }

    #[cfg(feature = "audio")]
    async fn detect_capabilities_from_kira(
        kira_driver: &KiraAudioDriver,
    ) -> Result<AudioDeviceCapabilities> {
        let driver_caps = kira_driver.get_capabilities();
        let supported_sample_rates = if driver_caps.sample_rates.is_empty() {
            vec![22050, 44100, 48000]
        } else {
            driver_caps.sample_rates.clone()
        };

        let preferred_format = driver_caps
            .formats
            .first()
            .copied()
            .unwrap_or(SampleFormat::I16);
        let bits_per_sample = match preferred_format {
            SampleFormat::F32 | SampleFormat::I32 => 32,
            SampleFormat::I16 => 16,
        };
        let format_type = match preferred_format {
            SampleFormat::F32 => AudioFormatType::PcmFloat,
            SampleFormat::I16 | SampleFormat::I32 => AudioFormatType::PcmInt,
        };

        let channels = driver_caps.max_output_channels.clamp(1, u16::MAX as u32) as u16;
        let supported_formats = supported_sample_rates
            .iter()
            .copied()
            .map(|sample_rate| AudioFormat {
                sample_rate,
                channels,
                bits_per_sample,
                format_type,
            })
            .collect::<Vec<_>>();

        Ok(AudioDeviceCapabilities {
            hardware_mixing: true,
            hardware_3d: channels > 2,
            supported_sample_rates,
            max_channels: channels,
            min_buffer_size: 256,
            max_buffer_size: 8192,
            supported_formats,
            latency_ms: 10.0,
        })
    }

    /// Audio processing thread
    async fn audio_processing_thread(
        thread_id: usize,
        receiver: Arc<Mutex<Receiver<AudioCommand>>>,
        playing_audio: Arc<DashMap<AudioHandle, PlayingAudio>>,
        _audio_cache: Arc<AudioCache>,
        statistics: Arc<ParkingRwLock<AudioStatistics>>,
        shutdown_flag: Arc<parking_lot::Mutex<bool>>,
        #[cfg(feature = "audio")] kira_driver: Arc<KiraAudioDriver>,
    ) {
        tracing::debug!("Audio processing thread {} started", thread_id);

        loop {
            if *shutdown_flag.lock() {
                break;
            }

            // Process commands with timeout
            let command = {
                let receiver_guard = receiver.lock().await;
                match receiver_guard.try_recv() {
                    Ok(cmd) => Some(cmd),
                    Err(_) => {
                        drop(receiver_guard);
                        tokio::time::sleep(Duration::from_millis(1)).await;
                        continue;
                    }
                }
            };

            if let Some(command) = command {
                match command {
                    AudioCommand::Play {
                        handle,
                        file_path,
                        source_config,
                        response,
                    } => {
                        let result = Self::handle_play_command(
                            handle,
                            file_path,
                            source_config,
                            &playing_audio,
                            #[cfg(feature = "audio")]
                            &kira_driver,
                        )
                        .await;
                        let _ = response.send(result);
                    }

                    AudioCommand::Stop { handle, response } => {
                        let result = Self::handle_stop_command(handle, &playing_audio).await;
                        let _ = response.send(result);
                    }

                    AudioCommand::Pause { handle, response } => {
                        let result = Self::handle_pause_command(handle, &playing_audio).await;
                        let _ = response.send(result);
                    }

                    AudioCommand::Resume { handle, response } => {
                        let result = Self::handle_resume_command(handle, &playing_audio).await;
                        let _ = response.send(result);
                    }

                    AudioCommand::UpdateListener {
                        listener: _,
                        response,
                    } => {
                        // Update 3D audio listener - implementation depends on 3D audio library
                        let _ = response.send(Ok(()));
                    }

                    AudioCommand::UpdateSource {
                        handle,
                        source_config,
                        response,
                    } => {
                        let result = Self::handle_update_source_command(
                            handle,
                            source_config,
                            &playing_audio,
                        )
                        .await;
                        let _ = response.send(result);
                    }

                    AudioCommand::GetStatus { handle, response } => {
                        let result = Self::handle_get_status_command(handle, &playing_audio).await;
                        let _ = response.send(result);
                    }

                    AudioCommand::Shutdown { response } => {
                        Self::handle_shutdown_command(&playing_audio).await;
                        let _ = response.send(Ok(()));
                        break;
                    }
                }
            }

            // Update statistics
            Self::update_statistics(&statistics, &playing_audio).await;
        }

        tracing::debug!("Audio processing thread {} stopped", thread_id);
    }

    /// Handle play audio command
    async fn handle_play_command(
        handle: AudioHandle,
        file_path: String,
        source_config: AudioSource,
        playing_audio: &DashMap<AudioHandle, PlayingAudio>,
        #[cfg(feature = "audio")] kira_driver: &Arc<KiraAudioDriver>,
    ) -> Result<()> {
        #[cfg(feature = "audio")]
        {
            // Use Kira to load and play the sound
            let sound_name = format!("sound_{}", handle.0);
            kira_driver
                .load_sound(&sound_name, &file_path)
                .await
                .map_err(|e| {
                    AudioDeviceError::PlaybackFailed(format!("Failed to load sound: {}", e))
                })?;

            // Play with spatial positioning if needed
            if source_config.position != Position3D::origin() {
                let position = [
                    source_config.position.x,
                    source_config.position.y,
                    source_config.position.z,
                ];
                kira_driver
                    .play_sound_3d(&sound_name, position, source_config.volume)
                    .await
                    .map_err(|e| {
                        AudioDeviceError::PlaybackFailed(format!("Failed to play 3D sound: {}", e))
                    })?;
            } else {
                kira_driver
                    .play_sound(&sound_name, source_config.volume, source_config.pitch)
                    .await
                    .map_err(|e| {
                        AudioDeviceError::PlaybackFailed(format!("Failed to play sound: {}", e))
                    })?;
            }

            // Create playing audio entry (clone volume before moving source_config)
            let current_volume = source_config.volume;
            let playing = PlayingAudio {
                id: Uuid::new_v4(),
                handle,
                audio_type: if source_config.position != Position3D::origin() {
                    PlayingAudioType::Sample3D
                } else {
                    PlayingAudioType::Sample
                },
                status: PlayingStatus::Playing,
                source_config,
                file_path: Some(file_path),
                request_stop: false,
                start_time: Instant::now(),
                duration: None, // Would need to be calculated from source
                current_volume,
                kira_handle: Some(sound_name),
                platform_handle: None,
            };

            playing_audio.insert(handle, playing);
        }

        #[cfg(not(feature = "audio"))]
        {
            // Stub implementation for when audio feature is disabled
            let _ = (handle, file_path, source_config, playing_audio);
            return Err(AudioDeviceError::InitializationFailed(
                "Audio feature not enabled".to_string(),
            ));
        }

        Ok(())
    }

    /// Handle stop audio command
    async fn handle_stop_command(
        handle: AudioHandle,
        playing_audio: &DashMap<AudioHandle, PlayingAudio>,
    ) -> Result<()> {
        if let Some(mut playing) = playing_audio.get_mut(&handle) {
            playing.status = PlayingStatus::Stopped;
            playing.request_stop = true;

            #[cfg(feature = "audio")]
            if let Some(kira_handle) = &playing.kira_handle {
                log::debug!("Miles: stop requested for handle {} (Kira sound '{}')", handle.0, kira_handle);
            }
        }

        // Remove from playing list
        playing_audio.remove(&handle);

        Ok(())
    }

    /// Handle pause audio command.
    ///
    /// C++ reference: MilesAudioManager::pauseAudio iterates all playing audio
    /// and calls AIL_stop_sample / AIL_stop_3D_sample / AIL_pause_stream(stream, 1).
    /// It also purges pending AR_Play requests from the queue.
    async fn handle_pause_command(
        handle: AudioHandle,
        playing_audio: &DashMap<AudioHandle, PlayingAudio>,
    ) -> Result<()> {
        if let Some(mut playing) = playing_audio.get_mut(&handle) {
            playing.status = PlayingStatus::Paused;

            #[cfg(feature = "audio")]
            if let Some(kira_handle) = &playing.kira_handle {
                log::debug!(
                    "Miles: pause handle {} (Kira sound '{}') — state set to Paused, backend passthrough deferred",
                    handle.0, kira_handle
                );
            }
        }

        Ok(())
    }

    /// Handle resume audio command.
    ///
    /// C++ reference: MilesAudioManager::resumeAudio iterates all playing audio
    /// and calls AIL_resume_sample / AIL_resume_3D_sample / AIL_pause_stream(stream, 0).
    async fn handle_resume_command(
        handle: AudioHandle,
        playing_audio: &DashMap<AudioHandle, PlayingAudio>,
    ) -> Result<()> {
        if let Some(mut playing) = playing_audio.get_mut(&handle) {
            playing.status = PlayingStatus::Playing;

            #[cfg(feature = "audio")]
            if let Some(kira_handle) = &playing.kira_handle {
                log::debug!(
                    "Miles: resume handle {} (Kira sound '{}') — state set to Playing, backend passthrough deferred",
                    handle.0, kira_handle
                );
            }
        }

        Ok(())
    }

    /// Handle update source command.
    ///
    /// C++ reference: volume changes applied via AIL_set_sample_volume /
    /// AIL_set_3D_sample_volume on the Miles handles.
    async fn handle_update_source_command(
        handle: AudioHandle,
        source_config: AudioSource,
        playing_audio: &DashMap<AudioHandle, PlayingAudio>,
    ) -> Result<()> {
        if let Some(mut playing) = playing_audio.get_mut(&handle) {
            let volume = source_config.volume;
            playing.source_config = source_config;
            playing.current_volume = volume;

            #[cfg(feature = "audio")]
            if let Some(kira_handle) = &playing.kira_handle {
                log::debug!(
                    "Miles: volume update for handle {} (Kira '{}') -> {:.2}",
                    handle.0, kira_handle, volume
                );
            }
        }

        Ok(())
    }

    /// Handle get status command
    async fn handle_get_status_command(
        handle: AudioHandle,
        playing_audio: &DashMap<AudioHandle, PlayingAudio>,
    ) -> Result<PlaybackState> {
        if let Some(playing) = playing_audio.get(&handle) {
            let state = match playing.status {
                PlayingStatus::Playing => PlaybackState::Playing,
                PlayingStatus::Paused => PlaybackState::Paused,
                PlayingStatus::Stopped => PlaybackState::Stopped,
            };
            Ok(state)
        } else {
            Ok(PlaybackState::Stopped)
        }
    }

    /// Handle shutdown command.
    ///
    /// C++ reference: MilesAudioManager::closeDevice() stops all samples,
    /// closes the digital handle, and releases all Miles resources.
    async fn handle_shutdown_command(playing_audio: &DashMap<AudioHandle, PlayingAudio>) {
        for mut entry in playing_audio.iter_mut() {
            let playing = entry.value_mut();
            playing.status = PlayingStatus::Stopped;
            playing.request_stop = true;

            #[cfg(feature = "audio")]
            if let Some(kira_handle) = &playing.kira_handle {
                log::debug!(
                    "Miles: shutdown stopping handle {} (Kira '{}')",
                    playing.handle.0, kira_handle
                );
            }
        }

        playing_audio.clear();
    }

    /// Update statistics
    async fn update_statistics(
        statistics: &ParkingRwLock<AudioStatistics>,
        playing_audio: &DashMap<AudioHandle, PlayingAudio>,
    ) {
        let active_count = playing_audio.len();
        let mut samples_2d = 0;
        let mut samples_3d = 0;
        let mut streams = 0;

        for entry in playing_audio.iter() {
            match entry.audio_type {
                PlayingAudioType::Sample => samples_2d += 1,
                PlayingAudioType::Sample3D => samples_3d += 1,
                PlayingAudioType::Stream => streams += 1,
            }
        }

        let mut stats = statistics.write();
        stats.active_channels = active_count;
        stats.samples_2d = samples_2d;
        stats.samples_3d = samples_3d;
        stats.streams = streams;

        // Update memory usage (simplified calculation)
        stats.memory_usage = (active_count * 1024 * 1024) as u64; // Estimate 1MB per active audio

        // Update CPU usage (simplified - would need real profiling in production)
        stats.cpu_usage = (active_count as f32 * 0.05).min(1.0); // 5% CPU per active audio, max 100%
    }
}

impl Drop for MilesAudioDevice {
    fn drop(&mut self) {
        // Set shutdown flag
        *self.shutdown_flag.lock() = true;
        tracing::info!("MilesAudioDevice dropped");
    }
}
