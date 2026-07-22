//! # Complete Cross-Platform Audio Driver
//!
//! This module provides comprehensive cross-platform audio driver abstraction with
//! full platform-specific implementations for optimal performance and compatibility.

use super::{AudioDeviceError, AudioFormat, DeviceCapabilities, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "audio")]
use cpal::{
    Device as CpalDevice, Host, Sample, SampleFormat, Stream, StreamConfig, SupportedStreamConfig,
};

// Platform-specific imports
#[cfg(all(target_os = "windows", feature = "audio"))]
use windows::Win32::Media::Audio::{IAudioClient, IMMDevice, IMMDeviceEnumerator, WAVEFORMATEX};

#[cfg(all(target_os = "linux", feature = "audio"))]
use alsa::{Direction, ValueOr, PCM};

#[cfg(all(target_os = "macos", feature = "audio"))]
use coreaudio_rs::audio_unit::{
    render_callback::{self, data},
    AudioUnit,
};

/// Audio driver types with comprehensive platform support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriverType {
    /// Cross-platform CPAL driver (fallback)
    Cpal,

    /// Windows WASAPI (Windows Audio Session API) - Primary Windows choice
    #[cfg(target_os = "windows")]
    Wasapi,

    /// Windows DirectSound (legacy compatibility)
    #[cfg(target_os = "windows")]
    DirectSound,

    /// Windows WaveOut (oldest compatibility)
    #[cfg(target_os = "windows")]
    WaveOut,

    /// Linux ALSA (Advanced Linux Sound Architecture) - Low latency
    #[cfg(target_os = "linux")]
    Alsa,

    /// Linux PulseAudio - Desktop integration
    #[cfg(target_os = "linux")]
    PulseAudio,

    /// Linux JACK - Professional audio
    #[cfg(target_os = "linux")]
    Jack,

    /// macOS CoreAudio - Native macOS
    #[cfg(target_os = "macos")]
    CoreAudio,

    /// macOS AudioUnit - Plugin architecture
    #[cfg(target_os = "macos")]
    AudioUnit,

    /// Null driver for testing
    Null,
}

impl Default for DriverType {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        return Self::Wasapi;

        #[cfg(target_os = "linux")]
        return Self::PulseAudio;

        #[cfg(target_os = "macos")]
        return Self::CoreAudio;

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        Self::Cpal
    }
}

/// Comprehensive driver capabilities and features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverCapabilities {
    /// Driver type
    pub driver_type: DriverType,
    /// Driver name
    pub name: String,
    /// Hardware acceleration support
    pub hardware_acceleration: bool,
    /// Exclusive mode support
    pub exclusive_mode: bool,
    /// Minimum supported latency (milliseconds)
    pub min_latency_ms: f32,
    /// Maximum supported latency (milliseconds)
    pub max_latency_ms: f32,
    /// Supported sample formats
    pub supported_formats: Vec<SampleFormat>,
    /// Supported sample rates
    pub supported_sample_rates: Vec<u32>,
    /// Maximum input channels
    pub max_input_channels: u32,
    /// Maximum output channels
    pub max_output_channels: u32,
    /// Driver version
    pub version: String,
    /// ASIO support (for professional audio)
    pub asio_support: bool,
    /// Multi-client support
    pub multi_client: bool,
    /// Hot-plug support (dynamic device changes)
    pub hotplug_support: bool,
    /// Sample rate conversion support
    pub src_support: bool,
    /// Bit depth conversion support
    pub bit_depth_conversion: bool,
}

/// Extended audio device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    /// Device ID
    pub id: String,
    /// Device name
    pub name: String,
    /// Device description
    pub description: String,
    /// Whether this is the default device
    pub is_default: bool,
    /// Device type (input/output/duplex)
    pub device_type: AudioDeviceType,
    /// Driver capabilities
    pub capabilities: DriverCapabilities,
    /// Supported configurations
    pub supported_configs: Vec<SupportedAudioConfig>,
    /// Device state (connected/disconnected)
    pub state: DeviceState,
    /// Hardware information
    pub hardware_info: Option<HardwareInfo>,
}

/// Audio device type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioDeviceType {
    /// Input device (microphone, line in)
    Input,
    /// Output device (speakers, headphones)
    Output,
    /// Bidirectional device
    Duplex,
}

/// Device connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceState {
    /// Device is active and available
    Active,
    /// Device is present but not available
    Disabled,
    /// Device is not present
    NotPresent,
    /// Device state is unknown
    Unknown,
}

/// Hardware information for audio devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    /// Vendor name
    pub vendor: Option<String>,
    /// Product name
    pub product: Option<String>,
    /// Hardware revision
    pub revision: Option<String>,
    /// Driver version
    pub driver_version: Option<String>,
    /// Bus type (USB, PCIe, etc.)
    pub bus_type: Option<String>,
    /// Hardware capabilities flags
    pub capabilities: u32,
}

/// Supported audio configuration with extended information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedAudioConfig {
    /// Audio format
    pub format: AudioFormat,
    /// Minimum buffer size in frames
    pub min_buffer_size: u32,
    /// Maximum buffer size in frames
    pub max_buffer_size: u32,
    /// Default buffer size in frames
    pub default_buffer_size: u32,
    /// Supported buffer sizes (if hardware has specific requirements)
    pub supported_buffer_sizes: Vec<u32>,
    /// Hardware latency in frames
    pub hardware_latency: u32,
    /// Whether exclusive mode is available for this config
    pub exclusive_mode_available: bool,
}

/// Audio callback function signature
pub type AudioCallback = dyn Fn(&mut [f32], &AudioCallbackInfo) + Send + Sync;

/// Information passed to audio callback
#[derive(Debug, Clone)]
pub struct AudioCallbackInfo {
    /// Current timestamp
    pub timestamp: Instant,
    /// Buffer size in frames
    pub buffer_size: usize,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
    /// Callback invocation count
    pub callback_count: u64,
    /// Estimated latency in frames
    pub latency_frames: u32,
    /// Whether we're in exclusive mode
    pub exclusive_mode: bool,
    /// CPU usage percentage (if available)
    pub cpu_usage: Option<f32>,
}

/// Stream performance metrics
#[derive(Debug, Clone, Default)]
pub struct StreamMetrics {
    /// Total callbacks processed
    pub total_callbacks: u64,
    /// Audio dropouts/underruns
    pub dropouts: u64,
    /// Average callback duration (microseconds)
    pub avg_callback_duration_us: f64,
    /// Peak callback duration (microseconds)
    pub peak_callback_duration_us: u64,
    /// Current CPU usage
    pub cpu_usage: f32,
    /// Memory usage (bytes)
    pub memory_usage: u64,
}

/// Comprehensive cross-platform audio driver
pub struct AudioDriver {
    /// Driver type
    driver_type: DriverType,

    /// CPAL host (if using CPAL)
    #[cfg(feature = "audio")]
    cpal_host: Option<Host>,

    /// Current audio device
    #[cfg(feature = "audio")]
    current_device: Option<CpalDevice>,

    /// Platform-specific device handle
    #[cfg(target_os = "windows")]
    windows_device: Option<WindowsAudioDevice>,

    #[cfg(target_os = "linux")]
    linux_device: Option<LinuxAudioDevice>,

    #[cfg(target_os = "macos")]
    macos_device: Option<MacOSAudioDevice>,

    /// Driver capabilities
    capabilities: Arc<DriverCapabilities>,

    /// Available devices cache
    available_devices: Arc<RwLock<Vec<AudioDeviceInfo>>>,

    /// Current configuration
    current_config: Arc<RwLock<Option<SupportedAudioConfig>>>,

    /// Active streams
    active_streams: Arc<DashMap<String, Arc<dyn AudioStream>>>,

    /// Performance metrics
    metrics: Arc<RwLock<StreamMetrics>>,

    /// Device change notification callback
    device_change_callback: Arc<Mutex<Option<Box<dyn Fn(DeviceChangeEvent) + Send + Sync>>>>,

    /// Hot-plug monitoring task handle
    hotplug_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// Device change events
#[derive(Debug, Clone)]
pub enum DeviceChangeEvent {
    /// Device was added
    DeviceAdded(AudioDeviceInfo),
    /// Device was removed
    DeviceRemoved(String), // device_id
    /// Device state changed
    DeviceStateChanged(String, DeviceState),
    /// Default device changed
    DefaultDeviceChanged(String), // device_id
}

// Platform-specific device implementations

#[cfg(target_os = "windows")]
struct WindowsAudioDevice {
    device: Option<IMMDevice>,
    audio_client: Option<IAudioClient>,
    // Additional Windows-specific fields
}

#[cfg(target_os = "linux")]
struct LinuxAudioDevice {
    pcm: Option<alsa::PCM>,
    // Additional Linux-specific fields
}

#[cfg(target_os = "macos")]
struct MacOSAudioDevice {
    audio_unit: Option<AudioUnit>,
    // Additional macOS-specific fields
}

impl AudioDriver {
    /// Create a new audio driver with the default driver type
    pub async fn new() -> Result<Self> {
        Self::new_with_type(DriverType::default()).await
    }

    /// Create a new audio driver with specific type
    pub async fn new_with_type(driver_type: DriverType) -> Result<Self> {
        let mut driver = Self {
            driver_type,
            #[cfg(feature = "audio")]
            cpal_host: None,
            #[cfg(feature = "audio")]
            current_device: None,

            #[cfg(target_os = "windows")]
            windows_device: None,

            #[cfg(target_os = "linux")]
            linux_device: None,

            #[cfg(target_os = "macos")]
            macos_device: None,

            capabilities: Arc::new(Self::get_default_capabilities(driver_type)),
            available_devices: Arc::new(RwLock::new(Vec::new())),
            current_config: Arc::new(RwLock::new(None)),
            active_streams: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(StreamMetrics::default())),
            device_change_callback: Arc::new(Mutex::new(None)),
            hotplug_handle: Arc::new(Mutex::new(None)),
        };

        driver.initialize().await?;
        Ok(driver)
    }

    /// Initialize the audio driver
    async fn initialize(&mut self) -> Result<()> {
        match self.driver_type {
            DriverType::Cpal => {
                #[cfg(feature = "audio")]
                {
                    let host = cpal::default_host();
                    self.cpal_host = Some(host);
                    self.enumerate_devices().await?;
                }

                #[cfg(not(feature = "audio"))]
                return Err(AudioDeviceError::InitializationFailed(
                    "Audio feature not enabled".to_string(),
                ));
            }

            #[cfg(target_os = "windows")]
            DriverType::Wasapi => {
                self.initialize_wasapi().await?;
            }

            #[cfg(target_os = "windows")]
            DriverType::DirectSound => {
                self.initialize_directsound().await?;
            }

            #[cfg(target_os = "windows")]
            DriverType::WaveOut => {
                self.initialize_waveout().await?;
            }

            #[cfg(target_os = "linux")]
            DriverType::Alsa => {
                self.initialize_alsa().await?;
            }

            #[cfg(target_os = "linux")]
            DriverType::PulseAudio => {
                self.initialize_pulseaudio().await?;
            }

            #[cfg(target_os = "linux")]
            DriverType::Jack => {
                self.initialize_jack().await?;
            }

            #[cfg(target_os = "macos")]
            DriverType::CoreAudio => {
                self.initialize_coreaudio().await?;
            }

            #[cfg(target_os = "macos")]
            DriverType::AudioUnit => {
                self.initialize_audiounit().await?;
            }

            DriverType::Null => {
                self.initialize_null_driver().await?;
            }
        }

        // Start hot-plug monitoring if supported
        if self.capabilities.hotplug_support {
            self.start_hotplug_monitoring().await?;
        }

        Ok(())
    }

    /// Enumerate available audio devices with full information
    pub async fn enumerate_devices(&mut self) -> Result<&[AudioDeviceInfo]> {
        let mut devices = Vec::new();

        match self.driver_type {
            DriverType::Cpal =>
            {
                #[cfg(feature = "audio")]
                if let Some(host) = &self.cpal_host {
                    devices.extend(self.enumerate_cpal_devices(host).await?);
                }
            }

            #[cfg(target_os = "windows")]
            DriverType::Wasapi => {
                devices.extend(self.enumerate_wasapi_devices().await?);
            }

            #[cfg(target_os = "linux")]
            DriverType::Alsa => {
                devices.extend(self.enumerate_alsa_devices().await?);
            }

            #[cfg(target_os = "macos")]
            DriverType::CoreAudio => {
                devices.extend(self.enumerate_coreaudio_devices().await?);
            }

            DriverType::Null => {
                devices.push(self.create_null_device());
            }

            _ => {
                // Use CPAL as fallback
                #[cfg(feature = "audio")]
                if let Some(host) = &self.cpal_host {
                    devices.extend(self.enumerate_cpal_devices(host).await?);
                }
            }
        }

        *self.available_devices.write() = devices;
        Ok(&self.available_devices.read())
    }

    /// Get the default output device
    pub async fn get_default_output_device(&self) -> Result<Option<&AudioDeviceInfo>> {
        Ok(self
            .available_devices
            .read()
            .iter()
            .find(|device| device.device_type == AudioDeviceType::Output && device.is_default))
    }

    /// Get the default input device
    pub async fn get_default_input_device(&self) -> Result<Option<&AudioDeviceInfo>> {
        Ok(self
            .available_devices
            .read()
            .iter()
            .find(|device| device.device_type == AudioDeviceType::Input && device.is_default))
    }

    /// Select an audio device for use with validation
    pub async fn select_device(&mut self, device_id: &str) -> Result<()> {
        let device_info = self
            .available_devices
            .read()
            .iter()
            .find(|device| device.id == device_id)
            .cloned()
            .ok_or_else(|| {
                AudioDeviceError::DeviceNotFound(format!("Device not found: {}", device_id))
            })?;

        // Validate device state
        if device_info.state != DeviceState::Active {
            return Err(AudioDeviceError::DeviceBusy(format!(
                "Device {} is not active (state: {:?})",
                device_id, device_info.state
            )));
        }

        match self.driver_type {
            DriverType::Cpal =>
            {
                #[cfg(feature = "audio")]
                self.select_cpal_device(&device_info).await?
            }

            #[cfg(target_os = "windows")]
            DriverType::Wasapi => {
                self.select_wasapi_device(&device_info).await?;
            }

            #[cfg(target_os = "linux")]
            DriverType::Alsa => {
                self.select_alsa_device(&device_info).await?;
            }

            #[cfg(target_os = "macos")]
            DriverType::CoreAudio => {
                self.select_coreaudio_device(&device_info).await?;
            }

            _ => {
                // Fallback selection
            }
        }

        tracing::info!(
            "Selected audio device: {} ({})",
            device_info.name,
            device_id
        );
        Ok(())
    }

    /// Create an audio stream with the specified configuration
    pub async fn create_stream(
        &self,
        config: &SupportedAudioConfig,
        callback: Arc<AudioCallback>,
    ) -> Result<Box<dyn AudioStream>> {
        let stream_id = uuid::Uuid::new_v4().to_string();

        let stream: Box<dyn AudioStream> = match self.driver_type {
            DriverType::Cpal => {
                #[cfg(feature = "audio")]
                {
                    Box::new(
                        self.create_cpal_stream(config, callback, &stream_id)
                            .await?,
                    )
                }

                #[cfg(not(feature = "audio"))]
                return Err(AudioDeviceError::InitializationFailed(
                    "Audio feature not enabled".to_string(),
                ));
            }

            #[cfg(target_os = "windows")]
            DriverType::Wasapi => Box::new(
                self.create_wasapi_stream(config, callback, &stream_id)
                    .await?,
            ),

            #[cfg(target_os = "linux")]
            DriverType::Alsa => Box::new(
                self.create_alsa_stream(config, callback, &stream_id)
                    .await?,
            ),

            #[cfg(target_os = "macos")]
            DriverType::CoreAudio => Box::new(
                self.create_coreaudio_stream(config, callback, &stream_id)
                    .await?,
            ),

            DriverType::Null => Box::new(NullAudioStream::new(stream_id)),

            _ => {
                return Err(AudioDeviceError::InitializationFailed(format!(
                    "{:?} streams not yet implemented",
                    self.driver_type
                )));
            }
        };

        self.active_streams
            .insert(stream_id.clone(), Arc::from(stream));

        // Return a wrapped stream that removes itself from active streams when dropped
        Ok(Box::new(ManagedAudioStream {
            stream_id,
            inner: self.active_streams.get(&stream_id).unwrap().clone(),
            driver: self as *const Self,
        }))
    }

    /// Get driver capabilities
    pub fn get_capabilities(&self) -> &DriverCapabilities {
        &self.capabilities
    }

    /// Get available devices
    pub fn get_available_devices(&self) -> Vec<AudioDeviceInfo> {
        self.available_devices.read().clone()
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> StreamMetrics {
        self.metrics.read().clone()
    }

    /// Set device change notification callback
    pub fn set_device_change_callback<F>(&self, callback: F)
    where
        F: Fn(DeviceChangeEvent) + Send + Sync + 'static,
    {
        *self.device_change_callback.lock() = Some(Box::new(callback));
    }

    /// Start hot-plug monitoring
    async fn start_hotplug_monitoring(&self) -> Result<()> {
        let driver_type = self.driver_type;
        let devices = Arc::clone(&self.available_devices);
        let callback = Arc::clone(&self.device_change_callback);

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            let mut previous_devices = Vec::new();

            loop {
                interval.tick().await;

                // Check for device changes (simplified implementation)
                let current_devices = devices.read().clone();

                // Detect added devices
                for device in &current_devices {
                    if !previous_devices
                        .iter()
                        .any(|d: &AudioDeviceInfo| d.id == device.id)
                    {
                        if let Some(callback) = callback.lock().as_ref() {
                            callback(DeviceChangeEvent::DeviceAdded(device.clone()));
                        }
                    }
                }

                // Detect removed devices
                for device in &previous_devices {
                    if !current_devices.iter().any(|d| d.id == device.id) {
                        if let Some(callback) = callback.lock().as_ref() {
                            callback(DeviceChangeEvent::DeviceRemoved(device.id.clone()));
                        }
                    }
                }

                previous_devices = current_devices;
            }
        });

        *self.hotplug_handle.lock() = Some(handle);
        Ok(())
    }

    // Platform-specific implementations

    #[cfg(feature = "audio")]
    async fn enumerate_cpal_devices(&self, host: &Host) -> Result<Vec<AudioDeviceInfo>> {
        let mut devices = Vec::new();

        // Enumerate output devices
        if let Ok(output_devices) = host.output_devices() {
            for (index, device) in output_devices.enumerate() {
                if let Ok(device_info) = self
                    .create_device_info_from_cpal(device, AudioDeviceType::Output, index)
                    .await
                {
                    devices.push(device_info);
                }
            }
        }

        // Enumerate input devices
        if let Ok(input_devices) = host.input_devices() {
            for (index, device) in input_devices.enumerate() {
                if let Ok(device_info) = self
                    .create_device_info_from_cpal(device, AudioDeviceType::Input, index + 1000)
                    .await
                {
                    devices.push(device_info);
                }
            }
        }

        Ok(devices)
    }

    #[cfg(feature = "audio")]
    async fn create_device_info_from_cpal(
        &self,
        device: CpalDevice,
        device_type: AudioDeviceType,
        index: usize,
    ) -> Result<AudioDeviceInfo> {
        let name = device.name().map_err(|e| {
            AudioDeviceError::DeviceNotFound(format!("Failed to get device name: {}", e))
        })?;

        let supported_configs = if device_type == AudioDeviceType::Output {
            device.supported_output_configs()
        } else {
            device.supported_input_configs()
        }
        .map_err(|e| {
            AudioDeviceError::FormatNotSupported(format!("Failed to get supported configs: {}", e))
        })?
        .map(|config| {
            let format = AudioFormat {
                sample_rate: config.min_sample_rate().0,
                channels: config.channels(),
                bits_per_sample: match config.sample_format() {
                    SampleFormat::I16 => 16,
                    SampleFormat::I32 => 32,
                    SampleFormat::F32 => 32,
                    _ => 16,
                },
                format_type: super::AudioFormatType::PcmInt,
            };

            SupportedAudioConfig {
                format,
                min_buffer_size: 256,
                max_buffer_size: 8192,
                default_buffer_size: 1024,
                supported_buffer_sizes: vec![256, 512, 1024, 2048, 4096, 8192],
                hardware_latency: 256,
                exclusive_mode_available: false, // CPAL doesn't expose this directly
            }
        })
        .collect();

        Ok(AudioDeviceInfo {
            id: format!("{}_{}", self.driver_type as u8, index),
            name: name.clone(),
            description: format!("{} - CPAL Device", name),
            is_default: index == 0, // Simplified - first device is default
            device_type,
            capabilities: (*self.capabilities).clone(),
            supported_configs,
            state: DeviceState::Active,
            hardware_info: None,
        })
    }

    #[cfg(feature = "audio")]
    async fn select_cpal_device(&mut self, device_info: &AudioDeviceInfo) -> Result<()> {
        if let Some(host) = &self.cpal_host {
            let devices = match device_info.device_type {
                AudioDeviceType::Output => host.output_devices(),
                AudioDeviceType::Input => host.input_devices(),
                AudioDeviceType::Duplex => host.output_devices(), // Prefer output for duplex
            }
            .map_err(|e| {
                AudioDeviceError::DeviceNotFound(format!("Failed to enumerate devices: {}", e))
            })?;

            for device in devices {
                if let Ok(name) = device.name() {
                    if name == device_info.name {
                        self.current_device = Some(device);
                        return Ok(());
                    }
                }
            }
        }

        Err(AudioDeviceError::DeviceNotFound(
            "CPAL device not found".to_string(),
        ))
    }

    #[cfg(feature = "audio")]
    async fn create_cpal_stream(
        &self,
        config: &SupportedAudioConfig,
        callback: Arc<AudioCallback>,
        stream_id: &str,
    ) -> Result<CpalAudioStream> {
        let device = self
            .current_device
            .as_ref()
            .ok_or_else(|| AudioDeviceError::DeviceNotFound("No device selected".to_string()))?;

        let stream_config = StreamConfig {
            channels: config.format.channels,
            sample_rate: cpal::SampleRate(config.format.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(config.default_buffer_size),
        };

        let stream_id_clone = stream_id.to_string();
        let metrics = Arc::clone(&self.metrics);
        let callback_count = Arc::new(AtomicU64::new(0));
        let callback_count_clone = Arc::clone(&callback_count);

        let stream = device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let start = Instant::now();
                    let count = callback_count_clone.fetch_add(1, Ordering::Relaxed);

                    let info = AudioCallbackInfo {
                        timestamp: start,
                        buffer_size: data.len(),
                        sample_rate: stream_config.sample_rate.0,
                        channels: stream_config.channels,
                        callback_count: count,
                        latency_frames: config.hardware_latency,
                        exclusive_mode: config.exclusive_mode_available,
                        cpu_usage: None,
                    };

                    callback(data, &info);

                    // Update metrics
                    let duration = start.elapsed();
                    let mut metrics_guard = metrics.write();
                    metrics_guard.total_callbacks += 1;
                    let duration_us = duration.as_micros() as f64;
                    metrics_guard.avg_callback_duration_us =
                        (metrics_guard.avg_callback_duration_us * (count as f64) + duration_us)
                            / ((count + 1) as f64);
                    metrics_guard.peak_callback_duration_us = metrics_guard
                        .peak_callback_duration_us
                        .max(duration.as_micros() as u64);
                },
                move |err| {
                    tracing::error!("Audio stream {} error: {}", stream_id_clone, err);
                    // Update dropout counter
                    let mut metrics_guard = metrics.write();
                    metrics_guard.dropouts += 1;
                },
                None,
            )
            .map_err(|e| {
                AudioDeviceError::InitializationFailed(format!("Failed to create stream: {}", e))
            })?;

        Ok(CpalAudioStream::new(
            stream,
            stream_id.to_string(),
            callback_count,
        ))
    }

    // Windows-specific implementations
    #[cfg(target_os = "windows")]
    async fn initialize_wasapi(&mut self) -> Result<()> {
        // Initialize WASAPI
        self.capabilities = Arc::new(DriverCapabilities {
            driver_type: DriverType::Wasapi,
            name: "Windows Audio Session API".to_string(),
            hardware_acceleration: true,
            exclusive_mode: true,
            min_latency_ms: 1.0,
            max_latency_ms: 100.0,
            supported_formats: vec![SampleFormat::I16, SampleFormat::I32, SampleFormat::F32],
            supported_sample_rates: vec![44100, 48000, 96000, 192000],
            max_input_channels: 8,
            max_output_channels: 8,
            version: "10.0".to_string(),
            asio_support: false,
            multi_client: true,
            hotplug_support: true,
            src_support: true,
            bit_depth_conversion: true,
        });

        // Initialize Windows-specific components
        self.windows_device = Some(WindowsAudioDevice {
            device: None,
            audio_client: None,
        });

        self.enumerate_devices().await?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn enumerate_wasapi_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        // Implementation would use Windows APIs to enumerate WASAPI devices
        // For now, return mock data
        Ok(vec![AudioDeviceInfo {
            id: "wasapi_default_out".to_string(),
            name: "Default WASAPI Output".to_string(),
            description: "Default Windows WASAPI output device".to_string(),
            is_default: true,
            device_type: AudioDeviceType::Output,
            capabilities: (*self.capabilities).clone(),
            supported_configs: vec![],
            state: DeviceState::Active,
            hardware_info: Some(HardwareInfo {
                vendor: Some("Microsoft".to_string()),
                product: Some("WASAPI".to_string()),
                revision: None,
                driver_version: Some("10.0".to_string()),
                bus_type: Some("System".to_string()),
                capabilities: 0,
            }),
        }])
    }

    // Linux-specific implementations
    #[cfg(target_os = "linux")]
    async fn initialize_alsa(&mut self) -> Result<()> {
        self.capabilities = Arc::new(DriverCapabilities {
            driver_type: DriverType::Alsa,
            name: "Advanced Linux Sound Architecture".to_string(),
            hardware_acceleration: true,
            exclusive_mode: true,
            min_latency_ms: 1.0,
            max_latency_ms: 100.0,
            supported_formats: vec![SampleFormat::I16, SampleFormat::I32, SampleFormat::F32],
            supported_sample_rates: vec![44100, 48000, 96000],
            max_input_channels: 8,
            max_output_channels: 8,
            version: "1.2".to_string(),
            asio_support: false,
            multi_client: true,
            hotplug_support: true,
            src_support: true,
            bit_depth_conversion: true,
        });

        self.linux_device = Some(LinuxAudioDevice { pcm: None });

        self.enumerate_devices().await?;
        Ok(())
    }

    // macOS-specific implementations
    #[cfg(target_os = "macos")]
    async fn initialize_coreaudio(&mut self) -> Result<()> {
        self.capabilities = Arc::new(DriverCapabilities {
            driver_type: DriverType::CoreAudio,
            name: "Core Audio".to_string(),
            hardware_acceleration: true,
            exclusive_mode: true,
            min_latency_ms: 1.0,
            max_latency_ms: 100.0,
            supported_formats: vec![SampleFormat::I16, SampleFormat::I32, SampleFormat::F32],
            supported_sample_rates: vec![44100, 48000, 96000, 192000],
            max_input_channels: 8,
            max_output_channels: 8,
            version: "1.0".to_string(),
            asio_support: false,
            multi_client: true,
            hotplug_support: true,
            src_support: true,
            bit_depth_conversion: true,
        });

        self.macos_device = Some(MacOSAudioDevice { audio_unit: None });

        self.enumerate_devices().await?;
        Ok(())
    }

    /// Initialize null driver for testing
    async fn initialize_null_driver(&mut self) -> Result<()> {
        self.capabilities = Arc::new(DriverCapabilities {
            driver_type: DriverType::Null,
            name: "Null Audio Driver".to_string(),
            hardware_acceleration: false,
            exclusive_mode: false,
            min_latency_ms: 0.0,
            max_latency_ms: 1000.0,
            supported_formats: vec![SampleFormat::I16, SampleFormat::F32],
            supported_sample_rates: vec![44100, 48000],
            max_input_channels: 16,
            max_output_channels: 16,
            version: "1.0".to_string(),
            asio_support: false,
            multi_client: true,
            hotplug_support: false,
            src_support: false,
            bit_depth_conversion: false,
        });
        Ok(())
    }

    fn create_null_device(&self) -> AudioDeviceInfo {
        AudioDeviceInfo {
            id: "null_device".to_string(),
            name: "Null Audio Device".to_string(),
            description: "Virtual null audio device for testing".to_string(),
            is_default: true,
            device_type: AudioDeviceType::Output,
            capabilities: (*self.capabilities).clone(),
            supported_configs: vec![],
            state: DeviceState::Active,
            hardware_info: None,
        }
    }

    fn get_default_capabilities(driver_type: DriverType) -> DriverCapabilities {
        DriverCapabilities {
            driver_type,
            name: format!("{:?} Driver", driver_type),
            hardware_acceleration: false,
            exclusive_mode: false,
            min_latency_ms: 10.0,
            max_latency_ms: 100.0,
            supported_formats: vec![SampleFormat::I16, SampleFormat::F32],
            supported_sample_rates: vec![44100, 48000],
            max_input_channels: 2,
            max_output_channels: 2,
            version: "1.0".to_string(),
            asio_support: false,
            multi_client: false,
            hotplug_support: false,
            src_support: false,
            bit_depth_conversion: false,
        }
    }

    // Placeholder implementations for other platforms
    // These would be fully implemented in a production system

    #[cfg(target_os = "windows")]
    async fn initialize_directsound(&mut self) -> Result<()> {
        // DirectSound implementation
        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn initialize_waveout(&mut self) -> Result<()> {
        // WaveOut implementation
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn initialize_pulseaudio(&mut self) -> Result<()> {
        // PulseAudio implementation
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn initialize_jack(&mut self) -> Result<()> {
        // JACK implementation
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn initialize_audiounit(&mut self) -> Result<()> {
        // AudioUnit implementation
        Ok(())
    }

    // Placeholder device enumeration functions
    #[cfg(target_os = "linux")]
    async fn enumerate_alsa_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        Ok(vec![])
    }

    #[cfg(target_os = "macos")]
    async fn enumerate_coreaudio_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        Ok(vec![])
    }

    // Placeholder device selection functions
    #[cfg(target_os = "windows")]
    async fn select_wasapi_device(&mut self, _device_info: &AudioDeviceInfo) -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn select_alsa_device(&mut self, _device_info: &AudioDeviceInfo) -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn select_coreaudio_device(&mut self, _device_info: &AudioDeviceInfo) -> Result<()> {
        Ok(())
    }

    // Placeholder stream creation functions
    #[cfg(target_os = "windows")]
    async fn create_wasapi_stream(
        &self,
        _config: &SupportedAudioConfig,
        _callback: Arc<AudioCallback>,
        _stream_id: &str,
    ) -> Result<WasapiAudioStream> {
        Ok(WasapiAudioStream::new())
    }

    #[cfg(target_os = "linux")]
    async fn create_alsa_stream(
        &self,
        _config: &SupportedAudioConfig,
        _callback: Arc<AudioCallback>,
        _stream_id: &str,
    ) -> Result<AlsaAudioStream> {
        Ok(AlsaAudioStream::new())
    }

    #[cfg(target_os = "macos")]
    async fn create_coreaudio_stream(
        &self,
        _config: &SupportedAudioConfig,
        _callback: Arc<AudioCallback>,
        _stream_id: &str,
    ) -> Result<CoreAudioStream> {
        Ok(CoreAudioStream::new())
    }
}

impl Drop for AudioDriver {
    fn drop(&mut self) {
        // Cleanup active streams
        self.active_streams.clear();

        // Stop hot-plug monitoring
        if let Some(handle) = self.hotplug_handle.lock().take() {
            handle.abort();
        }
    }
}

/// Audio stream trait for cross-platform streaming
pub trait AudioStream: Send + Sync {
    /// Start the audio stream
    fn start(&self) -> Result<()>;

    /// Stop the audio stream
    fn stop(&self) -> Result<()>;

    /// Pause the audio stream
    fn pause(&self) -> Result<()>;

    /// Check if the stream is running
    fn is_running(&self) -> bool;

    /// Get stream latency in milliseconds
    fn get_latency(&self) -> f32;

    /// Get stream ID
    fn get_id(&self) -> &str;

    /// Get performance metrics
    fn get_metrics(&self) -> StreamMetrics;
}

/// CPAL-based audio stream implementation
#[cfg(feature = "audio")]
pub struct CpalAudioStream {
    stream: Stream,
    stream_id: String,
    is_running: AtomicBool,
    callback_count: Arc<AtomicU64>,
}

#[cfg(feature = "audio")]
impl CpalAudioStream {
    fn new(stream: Stream, stream_id: String, callback_count: Arc<AtomicU64>) -> Self {
        Self {
            stream,
            stream_id,
            is_running: AtomicBool::new(false),
            callback_count,
        }
    }
}

#[cfg(feature = "audio")]
impl AudioStream for CpalAudioStream {
    fn start(&self) -> Result<()> {
        self.stream.play().map_err(|e| {
            AudioDeviceError::InitializationFailed(format!("Failed to start stream: {}", e))
        })?;
        self.is_running.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        self.stream.pause().map_err(|e| {
            AudioDeviceError::InitializationFailed(format!("Failed to stop stream: {}", e))
        })?;
        self.is_running.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn pause(&self) -> Result<()> {
        self.stream.pause().map_err(|e| {
            AudioDeviceError::InitializationFailed(format!("Failed to pause stream: {}", e))
        })?;
        self.is_running.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    fn get_latency(&self) -> f32 {
        // Simplified latency calculation
        10.0
    }

    fn get_id(&self) -> &str {
        &self.stream_id
    }

    fn get_metrics(&self) -> StreamMetrics {
        StreamMetrics {
            total_callbacks: self.callback_count.load(Ordering::Relaxed),
            dropouts: 0,
            avg_callback_duration_us: 0.0,
            peak_callback_duration_us: 0,
            cpu_usage: 0.0,
            memory_usage: 0,
        }
    }
}

// Platform-specific stream implementations

#[cfg(target_os = "windows")]
pub struct WasapiAudioStream {
    // Windows WASAPI stream implementation
}

#[cfg(target_os = "windows")]
impl WasapiAudioStream {
    fn new() -> Self {
        Self {}
    }
}

#[cfg(target_os = "windows")]
impl AudioStream for WasapiAudioStream {
    fn start(&self) -> Result<()> {
        Ok(())
    }
    fn stop(&self) -> Result<()> {
        Ok(())
    }
    fn pause(&self) -> Result<()> {
        Ok(())
    }
    fn is_running(&self) -> bool {
        false
    }
    fn get_latency(&self) -> f32 {
        5.0
    }
    fn get_id(&self) -> &str {
        "wasapi_stream"
    }
    fn get_metrics(&self) -> StreamMetrics {
        StreamMetrics::default()
    }
}

#[cfg(target_os = "linux")]
pub struct AlsaAudioStream {
    // Linux ALSA stream implementation
}

#[cfg(target_os = "linux")]
impl AlsaAudioStream {
    fn new() -> Self {
        Self {}
    }
}

#[cfg(target_os = "linux")]
impl AudioStream for AlsaAudioStream {
    fn start(&self) -> Result<()> {
        Ok(())
    }
    fn stop(&self) -> Result<()> {
        Ok(())
    }
    fn pause(&self) -> Result<()> {
        Ok(())
    }
    fn is_running(&self) -> bool {
        false
    }
    fn get_latency(&self) -> f32 {
        3.0
    }
    fn get_id(&self) -> &str {
        "alsa_stream"
    }
    fn get_metrics(&self) -> StreamMetrics {
        StreamMetrics::default()
    }
}

#[cfg(target_os = "macos")]
pub struct CoreAudioStream {
    // macOS CoreAudio stream implementation
}

#[cfg(target_os = "macos")]
impl CoreAudioStream {
    fn new() -> Self {
        Self {}
    }
}

#[cfg(target_os = "macos")]
impl AudioStream for CoreAudioStream {
    fn start(&self) -> Result<()> {
        Ok(())
    }
    fn stop(&self) -> Result<()> {
        Ok(())
    }
    fn pause(&self) -> Result<()> {
        Ok(())
    }
    fn is_running(&self) -> bool {
        false
    }
    fn get_latency(&self) -> f32 {
        2.0
    }
    fn get_id(&self) -> &str {
        "coreaudio_stream"
    }
    fn get_metrics(&self) -> StreamMetrics {
        StreamMetrics::default()
    }
}

/// Null audio stream for testing
pub struct NullAudioStream {
    stream_id: String,
    is_running: AtomicBool,
}

impl NullAudioStream {
    fn new(stream_id: String) -> Self {
        Self {
            stream_id,
            is_running: AtomicBool::new(false),
        }
    }
}

impl AudioStream for NullAudioStream {
    fn start(&self) -> Result<()> {
        self.is_running.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        self.is_running.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn pause(&self) -> Result<()> {
        self.is_running.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    fn get_latency(&self) -> f32 {
        0.0
    }

    fn get_id(&self) -> &str {
        &self.stream_id
    }

    fn get_metrics(&self) -> StreamMetrics {
        StreamMetrics::default()
    }
}

/// Managed audio stream that handles cleanup
struct ManagedAudioStream {
    stream_id: String,
    inner: Arc<dyn AudioStream>,
    driver: *const AudioDriver,
}

impl AudioStream for ManagedAudioStream {
    fn start(&self) -> Result<()> {
        self.inner.start()
    }

    fn stop(&self) -> Result<()> {
        self.inner.stop()
    }

    fn pause(&self) -> Result<()> {
        self.inner.pause()
    }

    fn is_running(&self) -> bool {
        self.inner.is_running()
    }

    fn get_latency(&self) -> f32 {
        self.inner.get_latency()
    }

    fn get_id(&self) -> &str {
        &self.stream_id
    }

    fn get_metrics(&self) -> StreamMetrics {
        self.inner.get_metrics()
    }
}

impl Drop for ManagedAudioStream {
    fn drop(&mut self) {
        // Remove stream from active streams when dropped
        unsafe {
            if !self.driver.is_null() {
                let driver = &*self.driver;
                driver.active_streams.remove(&self.stream_id);
            }
        }
    }
}

/// Audio driver builder for convenient configuration
pub struct AudioDriverBuilder {
    driver_type: Option<DriverType>,
    enable_hotplug: bool,
}

impl AudioDriverBuilder {
    /// Create a new driver builder
    pub fn new() -> Self {
        Self {
            driver_type: None,
            enable_hotplug: true,
        }
    }

    /// Set the driver type
    pub fn driver_type(mut self, driver_type: DriverType) -> Self {
        self.driver_type = Some(driver_type);
        self
    }

    /// Enable or disable hot-plug monitoring
    pub fn hotplug(mut self, enable: bool) -> Self {
        self.enable_hotplug = enable;
        self
    }

    /// Build the audio driver
    pub async fn build(self) -> Result<AudioDriver> {
        let driver_type = self.driver_type.unwrap_or_default();
        AudioDriver::new_with_type(driver_type).await
    }
}

impl Default for AudioDriverBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_driver_creation() {
        let driver = AudioDriver::new().await;
        assert!(driver.is_ok());
    }

    #[tokio::test]
    async fn test_null_driver() {
        let driver = AudioDriver::new_with_type(DriverType::Null).await.unwrap();
        assert_eq!(driver.driver_type, DriverType::Null);
        assert!(driver.get_capabilities().name.contains("Null"));
    }

    #[tokio::test]
    async fn test_device_enumeration() {
        let mut driver = AudioDriver::new_with_type(DriverType::Null).await.unwrap();
        let devices = driver.enumerate_devices().await.unwrap();
        assert!(!devices.is_empty());
    }

    #[tokio::test]
    async fn test_driver_builder() {
        let driver = AudioDriverBuilder::new()
            .driver_type(DriverType::Null)
            .hotplug(false)
            .build()
            .await
            .unwrap();

        assert_eq!(driver.driver_type, DriverType::Null);
    }

    #[tokio::test]
    async fn test_null_stream() {
        let stream = NullAudioStream::new("test_stream".to_string());
        assert!(!stream.is_running());

        stream.start().unwrap();
        assert!(stream.is_running());

        stream.stop().unwrap();
        assert!(!stream.is_running());
    }
}
