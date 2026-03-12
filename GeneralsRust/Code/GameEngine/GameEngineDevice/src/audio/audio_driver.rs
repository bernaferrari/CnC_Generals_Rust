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
        match driver_type {
            DriverType::Kira => {
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
            DriverType::Null => {
                let capabilities = DriverCapabilities {
                    driver_type,
                    name: "Null Audio Driver".to_string(),
                    hardware_acceleration: false,
                    exclusive_mode: false,
                    min_latency_ms: 0.0,
                    max_latency_ms: 1000.0,
                    supported_formats: vec![SampleFormat::F32, SampleFormat::I16],
                    supported_sample_rates: vec![22050, 44100, 48000],
                    max_input_channels: 2,
                    max_output_channels: 2,
                    version: "1.0.0".to_string(),
                };
                Ok(Self {
                    driver_type,
                    capabilities,
                    kira_driver: None,
                    initialized: true,
                })
            }
            _ => {
                let capabilities = DriverCapabilities {
                    driver_type,
                    name: format!("{driver_type:?} Driver"),
                    hardware_acceleration: false,
                    exclusive_mode: false,
                    min_latency_ms: 10.0,
                    max_latency_ms: 100.0,
                    supported_formats: vec![SampleFormat::F32, SampleFormat::I16],
                    supported_sample_rates: vec![22050, 44100, 48000],
                    max_input_channels: 2,
                    max_output_channels: 2,
                    version: "1.0.0".to_string(),
                };
                Ok(Self {
                    driver_type,
                    capabilities,
                    kira_driver: None,
                    initialized: false,
                })
            }
        }
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
        let name = if self.driver_type == DriverType::Kira {
            "Kira Default Audio Device"
        } else {
            "Default Audio Device"
        };
        Ok(vec![AudioDeviceInfo {
            id: format!("{:?}-default", self.driver_type).to_lowercase(),
            name: name.to_string(),
            is_default: true,
            capabilities: self.capabilities.clone(),
        }])
    }

    /// Initialize the driver
    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        if self.driver_type == DriverType::Kira && self.kira_driver.is_none() {
            let driver = KiraAudioDriver::new().await?;
            self.capabilities = build_capabilities(self.driver_type, driver.get_capabilities());
            self.kira_driver = Some(Arc::new(driver));
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

/// Audio stream trait for cross-platform streaming (stub version)
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

/// Stub audio stream implementation
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

    fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn get_latency(&self) -> f32 {
        10.0 // Stub latency
    }
}

fn build_capabilities(
    driver_type: DriverType,
    simple: &SimpleDeviceCapabilities,
) -> DriverCapabilities {
    DriverCapabilities {
        driver_type,
        name: "Kira Modern Audio Driver".to_string(),
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
