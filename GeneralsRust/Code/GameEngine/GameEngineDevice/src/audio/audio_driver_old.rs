//! # Cross-Platform Audio Driver
//!
//! This module provides cross-platform audio driver abstraction supporting:
//! - Windows: WASAPI, DirectSound
//! - Linux: ALSA, PulseAudio  
//! - macOS: CoreAudio
//!
//! The driver layer provides low-level hardware access while maintaining a unified interface.

use super::{AudioDeviceError, Result, AudioFormat, DeviceCapabilities};
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};

// CPAL replaced with Kira-based modern audio - no longer needed
// #[cfg(feature = "audio")]
// use cpal::{Device, Host, SupportedStreamConfig, StreamConfig, Sample, SampleFormat};

/// Audio driver types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriverType {
    /// Cross-platform CPAL driver
    Cpal,
    
    /// Windows WASAPI (Windows Audio Session API)
    #[cfg(target_os = "windows")]
    Wasapi,
    
    /// Windows DirectSound (legacy)
    #[cfg(target_os = "windows")]
    DirectSound,
    
    /// Linux ALSA (Advanced Linux Sound Architecture)
    #[cfg(target_os = "linux")]
    Alsa,
    
    /// Linux PulseAudio
    #[cfg(target_os = "linux")]  
    PulseAudio,
    
    /// macOS CoreAudio
    #[cfg(target_os = "macos")]
    CoreAudio,
    
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

/// Driver capabilities and features
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
    /// Minimum supported latency
    pub min_latency_ms: f32,
    /// Maximum supported latency  
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
}

/// Audio device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    /// Device ID
    pub id: String,
    /// Device name
    pub name: String,
    /// Whether this is the default device
    pub is_default: bool,
    /// Device type (input/output)
    pub device_type: AudioDeviceType,
    /// Driver capabilities
    pub capabilities: DriverCapabilities,
    /// Supported configurations
    pub supported_configs: Vec<SupportedAudioConfig>,
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

/// Supported audio configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedAudioConfig {
    /// Audio format
    pub format: AudioFormat,
    /// Minimum buffer size
    pub min_buffer_size: u32,
    /// Maximum buffer size  
    pub max_buffer_size: u32,
    /// Default buffer size
    pub default_buffer_size: u32,
}

/// Audio callback function signature
pub type AudioCallback = dyn Fn(&mut [f32], &AudioCallbackInfo) + Send + Sync;

/// Information passed to audio callback
#[derive(Debug, Clone)]
pub struct AudioCallbackInfo {
    /// Current timestamp
    pub timestamp: std::time::Instant,
    /// Buffer size in frames
    pub buffer_size: usize,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
    /// Callback invocation count
    pub callback_count: u64,
}

/// Cross-platform audio driver
pub struct AudioDriver {
    /// Driver type
    driver_type: DriverType,
    
    /// CPAL host (if using CPAL)
    #[cfg(feature = "audio")]
    cpal_host: Option<Host>,
    
    /// Current audio device
    #[cfg(feature = "audio")]
    current_device: Option<Device>,
    
    /// Driver capabilities
    capabilities: DriverCapabilities,
    
    /// Available devices
    available_devices: Vec<AudioDeviceInfo>,
    
    /// Current configuration
    current_config: Option<SupportedAudioConfig>,
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
            capabilities: Self::get_default_capabilities(driver_type),
            available_devices: Vec::new(),
            current_config: None,
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
                return Err(AudioDeviceError::InitializationFailed("Audio feature not enabled".to_string()));
            }
            
            #[cfg(target_os = "windows")]
            DriverType::Wasapi => {
                self.initialize_wasapi().await?;
            }
            
            #[cfg(target_os = "windows")]
            DriverType::DirectSound => {
                self.initialize_directsound().await?;
            }
            
            #[cfg(target_os = "linux")]
            DriverType::Alsa => {
                self.initialize_alsa().await?;
            }
            
            #[cfg(target_os = "linux")]
            DriverType::PulseAudio => {
                self.initialize_pulseaudio().await?;
            }
            
            #[cfg(target_os = "macos")]
            DriverType::CoreAudio => {
                self.initialize_coreaudio().await?;
            }
            
            DriverType::Null => {
                // Null driver for testing - no initialization needed
            }
        }
        
        Ok(())
    }
    
    /// Enumerate available audio devices
    pub async fn enumerate_devices(&mut self) -> Result<&[AudioDeviceInfo]> {
        self.available_devices.clear();
        
        match self.driver_type {
            DriverType::Cpal => {
                #[cfg(feature = "audio")]
                if let Some(host) = &self.cpal_host {
                    // Enumerate output devices
                    let output_devices = host.output_devices()
                        .map_err(|e| AudioDeviceError::DeviceNotFound(format!("Failed to enumerate output devices: {}", e)))?;
                    
                    for (index, device) in output_devices.enumerate() {
                        let device_info = self.create_device_info_from_cpal(device, AudioDeviceType::Output, index).await?;
                        self.available_devices.push(device_info);
                    }
                    
                    // Enumerate input devices
                    let input_devices = host.input_devices()
                        .map_err(|e| AudioDeviceError::DeviceNotFound(format!("Failed to enumerate input devices: {}", e)))?;
                    
                    for (index, device) in input_devices.enumerate() {
                        let device_info = self.create_device_info_from_cpal(device, AudioDeviceType::Input, index + 1000).await?;
                        self.available_devices.push(device_info);
                    }
                }
            }
            
            _ => {
                // Platform-specific device enumeration
                self.enumerate_platform_devices().await?;
            }
        }
        
        Ok(&self.available_devices)
    }
    
    /// Get the default output device
    pub async fn get_default_output_device(&self) -> Result<Option<&AudioDeviceInfo>> {
        Ok(self.available_devices.iter()
            .find(|device| device.device_type == AudioDeviceType::Output && device.is_default))
    }
    
    /// Get the default input device
    pub async fn get_default_input_device(&self) -> Result<Option<&AudioDeviceInfo>> {
        Ok(self.available_devices.iter()
            .find(|device| device.device_type == AudioDeviceType::Input && device.is_default))
    }
    
    /// Select an audio device for use
    pub async fn select_device(&mut self, device_id: &str) -> Result<()> {
        let device_info = self.available_devices.iter()
            .find(|device| device.id == device_id)
            .ok_or_else(|| AudioDeviceError::DeviceNotFound(format!("Device not found: {}", device_id)))?;
        
        match self.driver_type {
            DriverType::Cpal => {
                #[cfg(feature = "audio")]
                if let Some(host) = &self.cpal_host {
                    // Find and select the CPAL device
                    let devices = match device_info.device_type {
                        AudioDeviceType::Output => host.output_devices(),
                        AudioDeviceType::Input => host.input_devices(),
                        AudioDeviceType::Duplex => host.output_devices(), // Prefer output for duplex
                    }.map_err(|e| AudioDeviceError::DeviceNotFound(format!("Failed to enumerate devices: {}", e)))?;
                    
                    for device in devices {
                        if let Ok(name) = device.name() {
                            if name == device_info.name {
                                self.current_device = Some(device);
                                break;
                            }
                        }
                    }
                }
            }
            
            _ => {
                // Platform-specific device selection
                self.select_platform_device(device_id).await?;
            }
        }
        
        Ok(())
    }
    
    /// Create an audio stream with the specified configuration
    pub async fn create_stream(
        &self,
        config: &SupportedAudioConfig,
        callback: Arc<AudioCallback>,
    ) -> Result<Box<dyn AudioStream>> {
        match self.driver_type {
            DriverType::Cpal => {
                #[cfg(feature = "audio")]
                {
                    self.create_cpal_stream(config, callback).await
                }
                
                #[cfg(not(feature = "audio"))]
                Err(AudioDeviceError::InitializationFailed("Audio feature not enabled".to_string()))
            }
            
            _ => {
                self.create_platform_stream(config, callback).await
            }
        }
    }
    
    /// Get driver capabilities
    pub fn get_capabilities(&self) -> &DriverCapabilities {
        &self.capabilities
    }
    
    /// Get available devices
    pub fn get_available_devices(&self) -> &[AudioDeviceInfo] {
        &self.available_devices
    }
    
    // Platform-specific helper methods
    
    #[cfg(feature = "audio")]
    async fn create_device_info_from_cpal(
        &self,
        device: Device,
        device_type: AudioDeviceType,
        index: usize,
    ) -> Result<AudioDeviceInfo> {
        let name = device.name()
            .map_err(|e| AudioDeviceError::DeviceNotFound(format!("Failed to get device name: {}", e)))?;
        
        let supported_configs = device.supported_output_configs()
            .map_err(|e| AudioDeviceError::FormatNotSupported(format!("Failed to get supported configs: {}", e)))?
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
                }
            })
            .collect();
        
        Ok(AudioDeviceInfo {
            id: format!("{}_{}", self.driver_type as u8, index),
            name,
            is_default: index == 0, // Simplified - first device is default
            device_type,
            capabilities: self.capabilities.clone(),
            supported_configs,
        })
    }
    
    #[cfg(feature = "audio")]
    async fn create_cpal_stream(
        &self,
        config: &SupportedAudioConfig,
        callback: Arc<AudioCallback>,
    ) -> Result<Box<dyn AudioStream>> {
        let device = self.current_device.as_ref()
            .ok_or_else(|| AudioDeviceError::DeviceNotFound("No device selected".to_string()))?;
        
        let stream_config = StreamConfig {
            channels: config.format.channels,
            sample_rate: cpal::SampleRate(config.format.sample_rate),
            buffer_size: cpal::BufferSize::Fixed(config.default_buffer_size),
        };
        
        let mut callback_count = 0u64;
        let callback_clone = callback.clone();
        
        let stream = device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let info = AudioCallbackInfo {
                    timestamp: std::time::Instant::now(),
                    buffer_size: data.len(),
                    sample_rate: stream_config.sample_rate.0,
                    channels: stream_config.channels,
                    callback_count,
                };
                
                callback_clone(data, &info);
                callback_count += 1;
            },
            move |err| {
                tracing::error!("Audio stream error: {}", err);
            },
            None,
        ).map_err(|e| AudioDeviceError::InitializationFailed(format!("Failed to create stream: {}", e)))?;
        
        Ok(Box::new(CpalAudioStream::new(stream)))
    }
    
    async fn enumerate_platform_devices(&mut self) -> Result<()> {
        // Platform-specific device enumeration would go here
        // For now, create a dummy device for each platform
        match self.driver_type {
            #[cfg(target_os = "windows")]
            DriverType::Wasapi => {
                self.available_devices.push(AudioDeviceInfo {
                    id: "wasapi_default".to_string(),
                    name: "Default WASAPI Device".to_string(),
                    is_default: true,
                    device_type: AudioDeviceType::Output,
                    capabilities: self.capabilities.clone(),
                    supported_configs: vec![
                        SupportedAudioConfig {
                            format: AudioFormat::cd_quality(),
                            min_buffer_size: 256,
                            max_buffer_size: 8192,
                            default_buffer_size: 1024,
                        }
                    ],
                });
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    async fn select_platform_device(&mut self, _device_id: &str) -> Result<()> {
        // Platform-specific device selection
        Ok(())
    }
    
    async fn create_platform_stream(
        &self,
        _config: &SupportedAudioConfig,
        _callback: Arc<AudioCallback>,
    ) -> Result<Box<dyn AudioStream>> {
        // Platform-specific stream creation
        Err(AudioDeviceError::InitializationFailed("Platform-specific streams not yet implemented".to_string()))
    }
    
    // Platform-specific initialization methods
    
    #[cfg(target_os = "windows")]
    async fn initialize_wasapi(&mut self) -> Result<()> {
        // WASAPI initialization
        self.capabilities = DriverCapabilities {
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
            version: "1.0".to_string(),
        };
        Ok(())
    }
    
    #[cfg(target_os = "windows")]
    async fn initialize_directsound(&mut self) -> Result<()> {
        // DirectSound initialization
        self.capabilities = DriverCapabilities {
            driver_type: DriverType::DirectSound,
            name: "DirectSound".to_string(),
            hardware_acceleration: true,
            exclusive_mode: false,
            min_latency_ms: 10.0,
            max_latency_ms: 200.0,
            supported_formats: vec![SampleFormat::I16],
            supported_sample_rates: vec![22050, 44100, 48000],
            max_input_channels: 2,
            max_output_channels: 8,
            version: "8.0".to_string(),
        };
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    async fn initialize_alsa(&mut self) -> Result<()> {
        // ALSA initialization
        self.capabilities = DriverCapabilities {
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
        };
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    async fn initialize_pulseaudio(&mut self) -> Result<()> {
        // PulseAudio initialization
        self.capabilities = DriverCapabilities {
            driver_type: DriverType::PulseAudio,
            name: "PulseAudio".to_string(),
            hardware_acceleration: false,
            exclusive_mode: false,
            min_latency_ms: 5.0,
            max_latency_ms: 200.0,
            supported_formats: vec![SampleFormat::I16, SampleFormat::F32],
            supported_sample_rates: vec![44100, 48000],
            max_input_channels: 8,
            max_output_channels: 8,
            version: "15.0".to_string(),
        };
        Ok(())
    }
    
    #[cfg(target_os = "macos")]
    async fn initialize_coreaudio(&mut self) -> Result<()> {
        // CoreAudio initialization
        self.capabilities = DriverCapabilities {
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
        };
        Ok(())
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
}

/// CPAL-based audio stream implementation
#[cfg(feature = "audio")]
struct CpalAudioStream {
    stream: cpal::Stream,
    is_running: std::sync::atomic::AtomicBool,
}

#[cfg(feature = "audio")]
impl CpalAudioStream {
    fn new(stream: cpal::Stream) -> Self {
        Self {
            stream,
            is_running: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[cfg(feature = "audio")]
impl AudioStream for CpalAudioStream {
    fn start(&self) -> Result<()> {
        self.stream.play()
            .map_err(|e| AudioDeviceError::InitializationFailed(format!("Failed to start stream: {}", e)))?;
        self.is_running.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
    
    fn stop(&self) -> Result<()> {
        self.stream.pause()
            .map_err(|e| AudioDeviceError::InitializationFailed(format!("Failed to stop stream: {}", e)))?;
        self.is_running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
    
    fn pause(&self) -> Result<()> {
        self.stream.pause()
            .map_err(|e| AudioDeviceError::InitializationFailed(format!("Failed to pause stream: {}", e)))?;
        self.is_running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
    
    fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    fn get_latency(&self) -> f32 {
        // Simplified latency calculation - would need platform-specific implementation
        10.0
    }
}