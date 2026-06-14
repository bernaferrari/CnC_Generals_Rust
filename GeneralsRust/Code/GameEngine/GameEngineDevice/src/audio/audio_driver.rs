//! # Audio Driver - Kira-based wrapper
//!
//! Provides a cross-platform audio driver API backed by the modern Kira driver.

use super::{AudioDeviceError, Result};
use super::{KiraAudioDriver, SimpleDeviceCapabilities};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Audio driver types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriverType {
    /// Modern Kira-based driver (replaces CPAL)
    Kira,

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
        // Always use Kira for modern audio
        Self::Kira
    }
}

impl DriverType {
    fn uses_kira_backend(self) -> bool {
        match self {
            DriverType::Kira => true,
            #[cfg(target_os = "windows")]
            DriverType::Wasapi | DriverType::DirectSound => true,
            #[cfg(target_os = "linux")]
            DriverType::Alsa | DriverType::PulseAudio => true,
            #[cfg(target_os = "macos")]
            DriverType::CoreAudio => true,
            DriverType::Null => false,
        }
    }

    fn default_device_id(self) -> &'static str {
        match self {
            DriverType::Kira => "kira-system-default",
            #[cfg(target_os = "windows")]
            DriverType::Wasapi => "wasapi-system-default",
            #[cfg(target_os = "windows")]
            DriverType::DirectSound => "directsound-system-default",
            #[cfg(target_os = "linux")]
            DriverType::Alsa => "alsa-system-default",
            #[cfg(target_os = "linux")]
            DriverType::PulseAudio => "pulseaudio-system-default",
            #[cfg(target_os = "macos")]
            DriverType::CoreAudio => "coreaudio-system-default",
            DriverType::Null => "null-audio",
        }
    }

    fn default_device_name(self) -> &'static str {
        match self {
            DriverType::Null => "Null Audio Driver",
            DriverType::Kira => "System Default Audio Output",
            #[cfg(target_os = "windows")]
            DriverType::Wasapi => "System Default WASAPI Output",
            #[cfg(target_os = "windows")]
            DriverType::DirectSound => "System Default DirectSound Output",
            #[cfg(target_os = "linux")]
            DriverType::Alsa => "System Default ALSA Output",
            #[cfg(target_os = "linux")]
            DriverType::PulseAudio => "System Default PulseAudio Output",
            #[cfg(target_os = "macos")]
            DriverType::CoreAudio => "System Default CoreAudio Output",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            DriverType::Kira => "Kira Modern Audio Driver",
            #[cfg(target_os = "windows")]
            DriverType::Wasapi => "Kira WASAPI Audio Driver",
            #[cfg(target_os = "windows")]
            DriverType::DirectSound => "Kira DirectSound Compatibility Driver",
            #[cfg(target_os = "linux")]
            DriverType::Alsa => "Kira ALSA Audio Driver",
            #[cfg(target_os = "linux")]
            DriverType::PulseAudio => "Kira PulseAudio Audio Driver",
            #[cfg(target_os = "macos")]
            DriverType::CoreAudio => "Kira CoreAudio Audio Driver",
            DriverType::Null => "Null Audio Driver",
        }
    }
}

/// Sample format enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleFormat {
    I16,
    I32,
    F32,
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
    /// Device capabilities
    pub capabilities: DriverCapabilities,
}

/// Cross-platform audio driver
#[derive(Debug)]
pub struct AudioDriver {
    /// Driver type
    driver_type: DriverType,
    /// Driver capabilities
    capabilities: DriverCapabilities,
    /// Kira driver instance
    kira_driver: Option<Arc<KiraAudioDriver>>,
    /// Is initialized
    initialized: bool,
}

impl AudioDriver {
    /// Create a new audio driver
    pub async fn new(driver_type: DriverType) -> Result<Self> {
        if driver_type.uses_kira_backend() {
            let driver = KiraAudioDriver::new().await?;
            let capabilities = build_capabilities(driver_type, driver.get_capabilities());
            let initialized = driver.is_initialized();
            Ok(Self {
                driver_type,
                capabilities,
                kira_driver: Some(Arc::new(driver)),
                initialized,
            })
        } else {
            match driver_type {
                DriverType::Null => {
                    let capabilities = build_null_capabilities(driver_type);
                    Ok(Self {
                        driver_type,
                        capabilities,
                        kira_driver: None,
                        initialized: true,
                    })
                }
                _ => {
                    let driver = KiraAudioDriver::new().await?;
                    let capabilities = build_capabilities(driver_type, driver.get_capabilities());
                    let initialized = driver.is_initialized();
                    Ok(Self {
                        driver_type,
                        capabilities,
                        kira_driver: Some(Arc::new(driver)),
                        initialized,
                    })
                }
            }
        }
    }

    /// Build a driver directly from an existing Kira backend.
    pub fn from_kira_backend(driver_type: DriverType, driver: KiraAudioDriver) -> Result<Self> {
        if !driver_type.uses_kira_backend() {
            return Err(AudioDeviceError::InitializationFailed(format!(
                "{driver_type:?} cannot be backed by Kira"
            )));
        }

        let capabilities = build_capabilities(driver_type, driver.get_capabilities());
        let initialized = driver.is_initialized();
        Ok(Self {
            driver_type,
            capabilities,
            kira_driver: Some(Arc::new(driver)),
            initialized,
        })
    }

    /// Get driver capabilities
    pub fn capabilities(&self) -> &DriverCapabilities {
        &self.capabilities
    }

    /// Check if driver is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Enumerate available audio devices
    pub async fn enumerate_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        if !self.initialized {
            return Err(AudioDeviceError::InitializationFailed(format!(
                "{:?} driver is not initialized",
                self.driver_type
            )));
        }

        match self.driver_type {
            driver_type if driver_type.uses_kira_backend() => {
                if self.kira_driver.is_none() {
                    return Err(AudioDeviceError::InitializationFailed(format!(
                        "{driver_type:?} is initialized without an audio backend"
                    )));
                }

                Ok(vec![AudioDeviceInfo {
                    id: driver_type.default_device_id().to_string(),
                    name: driver_type.default_device_name().to_string(),
                    is_default: true,
                    capabilities: self.capabilities.clone(),
                }])
            }
            DriverType::Null => Ok(vec![AudioDeviceInfo {
                id: "null-audio".to_string(),
                name: "Null Audio Driver".to_string(),
                is_default: true,
                capabilities: self.capabilities.clone(),
            }]),
            _ => Err(AudioDeviceError::InitializationFailed(format!(
                "{:?} has no available audio backend",
                self.driver_type
            ))),
        }
    }

    /// Initialize the driver
    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        if self.driver_type.uses_kira_backend() && self.kira_driver.is_none() {
            let driver = KiraAudioDriver::new().await?;
            self.capabilities = build_capabilities(self.driver_type, driver.get_capabilities());
            self.kira_driver = Some(Arc::new(driver));
        } else if self.driver_type != DriverType::Null && self.kira_driver.is_none() {
            return Err(AudioDeviceError::InitializationFailed(format!(
                "{:?} has no available audio backend",
                self.driver_type
            )));
        }
        self.initialized = true;
        log::info!("Audio driver initialized: {:?}", self.driver_type);
        Ok(())
    }

    /// Shutdown the driver
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(driver) = self.kira_driver.take() {
            driver.shutdown().await?;
        }
        self.initialized = false;
        log::info!("Audio driver shutdown: {:?}", self.driver_type);
        Ok(())
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

    /// Resume the audio stream
    fn resume(&self) -> Result<()>;

    /// Check if the stream is running
    fn is_running(&self) -> bool;

    /// Get stream latency in milliseconds
    fn get_latency(&self) -> f32;

    /// Set stream volume
    fn set_volume(&self, volume: f32) -> Result<()>;

    /// Set stream pitch/speed
    fn set_pitch(&self, pitch: f32) -> Result<()>;
}

/// Kira-backed audio stream implementation
#[cfg(feature = "audio")]
pub struct KiraAudioStream {
    is_running: std::sync::atomic::AtomicBool,
    volume_bits: std::sync::atomic::AtomicU32,
    pitch_bits: std::sync::atomic::AtomicU32,
    latency_ms: f32,
}

#[cfg(feature = "audio")]
impl KiraAudioStream {
    pub fn new() -> Self {
        Self {
            is_running: std::sync::atomic::AtomicBool::new(false),
            volume_bits: std::sync::atomic::AtomicU32::new(1.0f32.to_bits()),
            pitch_bits: std::sync::atomic::AtomicU32::new(1.0f32.to_bits()),
            latency_ms: 5.0,
        }
    }
}

#[cfg(feature = "audio")]
impl AudioStream for KiraAudioStream {
    fn start(&self) -> Result<()> {
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn pause(&self) -> Result<()> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn resume(&self) -> Result<()> {
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn get_latency(&self) -> f32 {
        self.latency_ms
    }

    fn set_volume(&self, volume: f32) -> Result<()> {
        self.volume_bits.store(
            volume.clamp(0.0, 1.0).to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
        Ok(())
    }

    fn set_pitch(&self, pitch: f32) -> Result<()> {
        self.pitch_bits.store(
            pitch.clamp(0.1, 10.0).to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
        Ok(())
    }
}

/// Stub audio stream implementation (used when audio feature is disabled)
#[derive(Debug)]
pub struct StubAudioStream {
    is_running: std::sync::atomic::AtomicBool,
}

impl StubAudioStream {
    pub fn new() -> Self {
        Self {
            is_running: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl AudioStream for StubAudioStream {
    fn start(&self) -> Result<()> {
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn pause(&self) -> Result<()> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn resume(&self) -> Result<()> {
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn get_latency(&self) -> f32 {
        10.0
    }

    fn set_volume(&self, _volume: f32) -> Result<()> {
        Ok(())
    }

    fn set_pitch(&self, _pitch: f32) -> Result<()> {
        Ok(())
    }
}

fn build_null_capabilities(driver_type: DriverType) -> DriverCapabilities {
    DriverCapabilities {
        driver_type,
        name: driver_type.display_name().to_string(),
        hardware_acceleration: false,
        exclusive_mode: false,
        min_latency_ms: 0.0,
        max_latency_ms: 1000.0,
        supported_formats: vec![SampleFormat::F32, SampleFormat::I16],
        supported_sample_rates: vec![22050, 44100, 48000],
        max_input_channels: 2,
        max_output_channels: 2,
        version: "1.0.0".to_string(),
    }
}

fn build_capabilities(
    driver_type: DriverType,
    simple: &SimpleDeviceCapabilities,
) -> DriverCapabilities {
    DriverCapabilities {
        driver_type,
        name: driver_type.display_name().to_string(),
        hardware_acceleration: true,
        exclusive_mode: false,
        min_latency_ms: 5.0,
        max_latency_ms: 100.0,
        supported_formats: simple
            .formats
            .iter()
            .map(|f| match f {
                super::SampleFormat::F32 => SampleFormat::F32,
                super::SampleFormat::I16 => SampleFormat::I16,
                super::SampleFormat::I32 => SampleFormat::I32,
            })
            .collect(),
        supported_sample_rates: simple.sample_rates.clone(),
        max_input_channels: simple.max_input_channels,
        max_output_channels: simple.max_output_channels,
        version: simple.version.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn null_driver_enumerates_only_explicit_null_device() {
        let driver = AudioDriver::new(DriverType::Null).await.unwrap();
        let devices = driver.enumerate_devices().await.unwrap();

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, "null-audio");
        assert_eq!(devices[0].capabilities.driver_type, DriverType::Null);
    }

    #[cfg(not(feature = "audio"))]
    #[tokio::test]
    async fn kira_driver_requires_audio_feature() {
        let err = AudioDriver::new(DriverType::Kira).await.unwrap_err();
        assert!(err.to_string().contains("audio feature is disabled"));
    }

    #[cfg(all(target_os = "macos", not(feature = "audio")))]
    #[tokio::test]
    async fn native_coreaudio_driver_reports_disabled_audio_backend() {
        let err = AudioDriver::new(DriverType::CoreAudio).await.unwrap_err();
        assert!(err.to_string().contains("audio feature is disabled"));
    }

    #[cfg(all(target_os = "linux", not(feature = "audio")))]
    #[tokio::test]
    async fn native_pulseaudio_driver_reports_disabled_audio_backend() {
        let err = AudioDriver::new(DriverType::PulseAudio).await.unwrap_err();
        assert!(err.to_string().contains("audio feature is disabled"));
    }

    #[cfg(all(target_os = "windows", not(feature = "audio")))]
    #[tokio::test]
    async fn native_wasapi_driver_reports_disabled_audio_backend() {
        let err = AudioDriver::new(DriverType::Wasapi).await.unwrap_err();
        assert!(err.to_string().contains("audio feature is disabled"));
    }

    #[cfg(all(target_os = "macos", feature = "audio"))]
    #[tokio::test]
    async fn native_coreaudio_driver_enumerates_system_default_device() {
        let driver = AudioDriver::new(DriverType::CoreAudio).await.unwrap();
        let devices = driver.enumerate_devices().await.unwrap();

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, "coreaudio-system-default");
        assert_eq!(devices[0].capabilities.driver_type, DriverType::CoreAudio);
    }
}
