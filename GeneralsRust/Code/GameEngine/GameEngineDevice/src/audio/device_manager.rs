//! # Audio Device Manager
//!
//! Manages audio device lifecycle, enumeration, and selection.

use super::{
    AudioDeviceError, AudioDriver, DriverType, MilesAudioConfig, MilesAudioDevice, Result,
};
use crate::{DeviceStatus, DeviceType, PerformanceMetrics};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Audio device information
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
    /// Device capabilities
    pub capabilities: super::DeviceCapabilities,
}

/// Device selection criteria
#[derive(Debug, Clone)]
pub struct DeviceSelection {
    /// Preferred device ID
    pub device_id: Option<String>,
    /// Minimum required capabilities
    pub required_capabilities: Option<super::DeviceCapabilities>,
    /// Prefer hardware acceleration
    pub prefer_hardware_acceleration: bool,
    /// Prefer low latency
    pub prefer_low_latency: bool,
}

impl Default for DeviceSelection {
    fn default() -> Self {
        Self {
            device_id: None,
            required_capabilities: None,
            prefer_hardware_acceleration: true,
            prefer_low_latency: true,
        }
    }
}

/// Audio device manager
#[derive(Clone)]
pub struct DeviceManager {
    /// Current audio device
    current_device: Arc<RwLock<Option<MilesAudioDevice>>>,
    /// Available devices
    available_devices: Arc<RwLock<Vec<AudioDeviceInfo>>>,
    /// Manager configuration
    config: Arc<RwLock<MilesAudioConfig>>,
}

impl DeviceManager {
    /// Create a new device manager
    pub async fn new() -> Result<Self> {
        Self::new_with_config(MilesAudioConfig::default()).await
    }

    /// Create a new device manager with configuration
    pub async fn new_with_config(config: MilesAudioConfig) -> Result<Self> {
        let manager = Self {
            current_device: Arc::new(RwLock::new(None)),
            available_devices: Arc::new(RwLock::new(Vec::new())),
            config: Arc::new(RwLock::new(config)),
        };

        // Initialize and enumerate devices
        manager.enumerate_devices().await?;

        Ok(manager)
    }

    /// Enumerate available audio devices
    pub async fn enumerate_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        let mut driver = AudioDriver::new(DriverType::default()).await?;
        if !driver.is_initialized() {
            driver.initialize().await?;
        }

        let mut devices = driver
            .enumerate_devices()
            .await?
            .into_iter()
            .map(|device| AudioDeviceInfo {
                id: device.id,
                name: device.name.clone(),
                description: format!(
                    "{} (driver: {}, version: {})",
                    device.name, device.capabilities.name, device.capabilities.version
                ),
                is_default: device.is_default,
                capabilities: Self::capabilities_from_driver(&device.capabilities),
            })
            .collect::<Vec<_>>();

        // Keep a deterministic fallback entry when a backend reports no devices.
        if devices.is_empty() {
            devices.push(AudioDeviceInfo {
                id: "default".to_string(),
                name: "Default Audio Device".to_string(),
                description: "No explicit driver devices reported; using default output"
                    .to_string(),
                is_default: true,
                capabilities: super::DeviceCapabilities::default(),
            });
        }

        *self.available_devices.write().await = devices.clone();
        Ok(devices)
    }

    /// Select and initialize an audio device
    pub async fn select_device(&self, selection: DeviceSelection) -> Result<()> {
        let devices = self.available_devices.read().await;

        // Find the best matching device
        let selected_device = if let Some(device_id) = &selection.device_id {
            devices.iter().find(|d| d.id == *device_id)
        } else {
            // Auto-select best device based on criteria
            devices
                .iter()
                .filter(|d| {
                    if selection.prefer_hardware_acceleration && !d.capabilities.hardware_mixing {
                        return false;
                    }
                    if selection.prefer_low_latency && d.capabilities.latency_ms > 10.0 {
                        return false;
                    }
                    true
                })
                .max_by(|a, b| {
                    // Score devices based on capabilities
                    let score_a = Self::score_device(a, &selection);
                    let score_b = Self::score_device(b, &selection);
                    score_a
                        .partial_cmp(&score_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        };

        let device_info = selected_device.ok_or_else(|| {
            AudioDeviceError::DeviceNotFound("No suitable device found".to_string())
        })?;

        // Create and initialize the audio device
        let config = self.config.read().await.clone();
        let mut audio_device = MilesAudioDevice::new_with_config(config).await?;
        audio_device.init().await?;

        *self.current_device.write().await = Some(audio_device);

        tracing::info!(
            "Selected audio device: {} ({})",
            device_info.name,
            device_info.id
        );
        Ok(())
    }

    /// Get the current audio device
    pub async fn get_current_device(&self) -> Option<MilesAudioDevice> {
        self.current_device.write().await.take()
    }

    /// Get available devices
    pub async fn get_available_devices(&self) -> Vec<AudioDeviceInfo> {
        self.available_devices.read().await.clone()
    }

    /// Get capabilities for the selected device, or the default enumerated device.
    pub async fn get_capabilities(&self) -> super::DeviceCapabilities {
        if let Some(device) = self.current_device.read().await.as_ref() {
            return device.get_capabilities().await;
        }

        let devices = self.available_devices.read().await;
        devices
            .iter()
            .find(|device| device.is_default)
            .or_else(|| devices.first())
            .map(|device| device.capabilities.clone())
            .unwrap_or_default()
    }

    /// Get device status
    pub async fn get_status(&self) -> Result<DeviceStatus> {
        if let Some(device) = self.current_device.read().await.as_ref() {
            device.get_status().await
        } else {
            Ok(DeviceStatus {
                device_type: DeviceType::Audio,
                initialized: false,
                active: false,
                capabilities: Default::default(),
                performance: Default::default(),
            })
        }
    }

    /// Get performance metrics
    pub async fn get_performance_metrics(&self) -> Result<PerformanceMetrics> {
        if let Some(device) = self.current_device.read().await.as_ref() {
            device.get_performance_metrics().await
        } else {
            Ok(Default::default())
        }
    }

    /// Shutdown the device manager
    pub async fn shutdown(&self) -> Result<()> {
        if let Some(device) = self.current_device.write().await.take() {
            device.shutdown().await?;
        }
        Ok(())
    }

    /// Score a device based on selection criteria
    fn score_device(device: &AudioDeviceInfo, selection: &DeviceSelection) -> f32 {
        let mut score = 0.0;

        // Prefer default device
        if device.is_default {
            score += 10.0;
        }

        // Hardware acceleration bonus
        if selection.prefer_hardware_acceleration && device.capabilities.hardware_mixing {
            score += 20.0;
        }

        // 3D audio bonus
        if device.capabilities.hardware_3d {
            score += 15.0;
        }

        // Low latency bonus
        if selection.prefer_low_latency {
            score += 50.0 / device.capabilities.latency_ms.max(1.0);
        }

        // Channel count bonus
        score += device.capabilities.max_channels as f32 * 2.0;

        score
    }

    fn capabilities_from_driver(
        capabilities: &super::audio_driver::DriverCapabilities,
    ) -> super::DeviceCapabilities {
        let mut mapped = super::DeviceCapabilities::default();
        mapped.hardware_mixing = capabilities.hardware_acceleration;
        mapped.hardware_3d = capabilities.max_output_channels > 2;
        mapped.max_channels = capabilities.max_output_channels.min(u16::MAX as u32) as u16;
        if !capabilities.supported_sample_rates.is_empty() {
            mapped.supported_sample_rates = capabilities.supported_sample_rates.clone();
        }
        mapped.latency_ms = capabilities.min_latency_ms.max(0.0);
        mapped
    }
}
