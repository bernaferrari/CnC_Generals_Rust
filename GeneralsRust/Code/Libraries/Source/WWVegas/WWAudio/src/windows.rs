//! Windows-specific audio implementations.

#[cfg(windows)]
use std::ffi::CStr;
#[cfg(windows)]
use windows::core::PCSTR;
#[cfg(windows)]
use windows::Win32::Foundation::{BOOL, HWND};
#[cfg(windows)]
use windows::Win32::Media::Audio::{
    DirectSound::{
        DirectSoundCreate, DirectSoundEnumerateA, IDirectSound, DSCAPS, DSCAPS_CERTIFIED,
        DSCAPS_EMULDRIVER, DSSCL_PRIORITY,
    },
    WAVEFORMATEX, WAVE_FORMAT_PCM,
};

#[cfg(windows)]
use crate::{error::Result, formats::AudioFormat};

/// Windows audio device implementation
#[cfg(windows)]
pub struct WindowsAudioDevice {
    direct_sound: Option<IDirectSound>,
    format: AudioFormat,
}

/// Windows-specific device capabilities
#[cfg(windows)]
#[derive(Debug, Clone)]
pub struct WindowsDeviceCapabilities {
    pub supports_hardware_mixing: bool,
    pub supports_3d_audio: bool,
    pub max_hardware_channels: u32,
    pub driver_version: String,
}

#[cfg(windows)]
impl WindowsAudioDevice {
    /// Create new Windows audio device
    pub fn new() -> Result<Self> {
        // Initialize DirectSound
        let direct_sound = unsafe {
            let mut ds = std::mem::zeroed();
            DirectSoundCreate(None, &mut ds, None).map_err(|e| {
                crate::error::Error::Device(crate::error::DeviceError::InitializationFailed(
                    format!("DirectSound init failed: {:?}", e),
                ))
            })?;
            Some(ds)
        };

        Ok(Self {
            direct_sound,
            format: AudioFormat::default(),
        })
    }

    /// Get Windows-specific capabilities
    pub fn get_windows_capabilities(&self) -> Result<WindowsDeviceCapabilities> {
        let ds = self
            .direct_sound
            .as_ref()
            .ok_or(crate::error::Error::Device(
                crate::error::DeviceError::NotFound,
            ))?;

        let mut caps = unsafe { std::mem::zeroed::<DSCAPS>() };
        caps.dwSize = std::mem::size_of::<DSCAPS>() as u32;

        unsafe {
            ds.GetCaps(&mut caps).map_err(|e| {
                crate::error::Error::Device(crate::error::DeviceError::InitializationFailed(
                    format!("DirectSound capability query failed: {e:?}"),
                ))
            })?;
        }

        let driver_version = if (caps.dwFlags & DSCAPS_CERTIFIED) != 0 {
            "Certified driver"
        } else if (caps.dwFlags & DSCAPS_EMULDRIVER) != 0 {
            "Emulated driver"
        } else {
            "Unknown driver"
        };

        Ok(WindowsDeviceCapabilities {
            supports_hardware_mixing: caps.dwMaxHwMixingAllBuffers > 0,
            supports_3d_audio: caps.dwMaxHw3DAllBuffers > 0,
            max_hardware_channels: caps.dwMaxHwMixingAllBuffers,
            driver_version: driver_version.to_string(),
        })
    }

    /// Set cooperative level
    pub fn set_cooperative_level(&self, hwnd: isize) -> Result<()> {
        if let Some(ref ds) = self.direct_sound {
            unsafe {
                // Set cooperative level for DirectSound
                ds.SetCooperativeLevel(HWND(hwnd), DSSCL_PRIORITY)
                    .map_err(|e| {
                        crate::error::Error::Device(
                            crate::error::DeviceError::InitializationFailed(format!(
                                "Failed to set cooperative level: {e:?}"
                            )),
                        )
                    })?;
            }
        }
        Ok(())
    }
}

/// Windows audio utilities
#[cfg(windows)]
pub struct WindowsAudioUtils;

#[cfg(windows)]
impl WindowsAudioUtils {
    /// Convert AudioFormat to WAVEFORMATEX
    pub fn audio_format_to_waveformatex(format: &AudioFormat) -> WAVEFORMATEX {
        WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: format.channels,
            nSamplesPerSec: u32::from(format.sample_rate),
            nAvgBytesPerSec: format.bytes_per_second(),
            nBlockAlign: format.bytes_per_frame() as u16,
            wBitsPerSample: u8::from(format.sample_width) as u16,
            cbSize: 0,
        }
    }

    /// Get Windows audio device names
    pub fn enumerate_windows_devices() -> Result<Vec<String>> {
        unsafe extern "system" fn enum_callback(
            _guid: *mut windows::core::GUID,
            description: PCSTR,
            _module: PCSTR,
            context: *mut std::ffi::c_void,
        ) -> BOOL {
            if context.is_null() {
                return BOOL(0);
            }

            let devices = unsafe { &mut *(context as *mut Vec<String>) };

            if !description.is_null() {
                // SAFETY: The pointer is valid for the duration of the callback.
                let desc = unsafe { CStr::from_ptr(description.as_ptr() as *const i8) }
                    .to_string_lossy()
                    .into_owned();
                devices.push(desc);
            } else {
                devices.push("Primary Sound Driver".to_string());
            }

            BOOL(1)
        }

        let mut devices = Vec::new();

        unsafe {
            let context = &mut devices as *mut Vec<String> as *const std::ffi::c_void;
            DirectSoundEnumerateA(Some(enum_callback), Some(context)).map_err(|e| {
                crate::error::Error::Device(crate::error::DeviceError::InitializationFailed(
                    format!("DirectSound enumeration failed: {e:?}"),
                ))
            })?;
        }

        if devices.is_empty() {
            devices.push("Primary Sound Driver".to_string());
        }

        Ok(devices)
    }

    /// Get default Windows audio device
    pub fn get_default_device() -> Result<String> {
        let mut devices = Self::enumerate_windows_devices()?;
        Ok(devices
            .drain(..)
            .next()
            .unwrap_or_else(|| "Primary Sound Driver".to_string()))
    }
}

// Stub implementations for non-Windows platforms
#[cfg(not(windows))]
pub struct WindowsAudioDevice;

#[cfg(not(windows))]
impl WindowsAudioDevice {
    pub fn new() -> Result<Self> {
        Err(crate::error::Error::Audio(
            "Windows audio not available on this platform".to_string(),
        ))
    }
}

#[cfg(not(windows))]
pub struct WindowsAudioUtils;

#[cfg(not(windows))]
impl WindowsAudioUtils {
    pub fn enumerate_windows_devices() -> Result<Vec<String>> {
        Ok(vec![])
    }
}
