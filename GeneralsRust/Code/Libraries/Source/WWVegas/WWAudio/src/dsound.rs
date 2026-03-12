//! DirectSound-specific audio implementation for Windows.

#[cfg(windows)]
use windows::Win32::{
    Foundation::{HRESULT, HWND},
    Media::Audio::DirectSound::*,
    System::Com::CoInitialize,
};

#[cfg(windows)]
use crate::{error::Result, formats::AudioFormat, Priority};
#[cfg(windows)]
use std::ptr;

/// DirectSound audio driver implementation
#[cfg(windows)]
pub struct DirectSoundDriver {
    direct_sound: IDirectSound,
    primary_buffer: Option<IDirectSoundBuffer>,
    format: AudioFormat,
    initialized: bool,
}

/// DirectSound buffer wrapper
#[cfg(windows)]
pub struct DirectSoundBuffer {
    buffer: IDirectSoundBuffer,
    format: AudioFormat,
    size: u32,
}

/// DirectSound 3D buffer for positional audio
#[cfg(windows)]
pub struct DirectSound3DBuffer {
    buffer: IDirectSoundBuffer,
    buffer_3d: IDirectSound3DBuffer,
    position: [f32; 3],
    velocity: [f32; 3],
}

#[cfg(windows)]
impl DirectSoundDriver {
    /// Create new DirectSound driver
    pub fn new(hwnd: Option<HWND>) -> Result<Self> {
        // Initialize COM
        unsafe {
            CoInitialize(None).ok();
        }

        // Create DirectSound interface
        let direct_sound = unsafe {
            let mut ds = std::mem::zeroed();
            DirectSoundCreate(None, &mut ds, None).map_err(|e| {
                crate::error::Error::Device(crate::error::DeviceError::InitializationFailed(
                    format!("DirectSoundCreate failed: {:?}", e),
                ))
            })?;
            ds
        };

        // Set cooperative level
        if let Some(hwnd) = hwnd {
            unsafe {
                direct_sound
                    .SetCooperativeLevel(hwnd, DSSCL_PRIORITY)
                    .map_err(|e| {
                        crate::error::Error::Device(
                            crate::error::DeviceError::InitializationFailed(format!(
                                "SetCooperativeLevel failed: {:?}",
                                e
                            )),
                        )
                    })?;
            }
        }

        Ok(Self {
            direct_sound,
            primary_buffer: None,
            format: AudioFormat::default(),
            initialized: false,
        })
    }

    /// Initialize DirectSound with specific format
    pub fn initialize(&mut self, format: AudioFormat) -> Result<()> {
        self.format = format;

        // Create primary buffer
        let buffer_desc = DSBUFFERDESC {
            dwSize: std::mem::size_of::<DSBUFFERDESC>() as u32,
            dwFlags: DSBCAPS_PRIMARYBUFFER,
            dwBufferBytes: 0,
            dwReserved: 0,
            lpwfxFormat: ptr::null_mut(),
            guid3DAlgorithm: Default::default(),
        };

        let primary_buffer = unsafe {
            let mut buffer = std::mem::zeroed();
            self.direct_sound
                .CreateSoundBuffer(&buffer_desc, &mut buffer, None)
                .map_err(|e| {
                    crate::error::Error::Device(crate::error::DeviceError::InitializationFailed(
                        format!("CreateSoundBuffer failed: {:?}", e),
                    ))
                })?;
            buffer
        };

        // Set primary buffer format
        let wave_format = crate::windows::WindowsAudioUtils::audio_format_to_waveformatex(&format);
        unsafe {
            primary_buffer.SetFormat(&wave_format).map_err(|e| {
                crate::error::Error::Device(crate::error::DeviceError::UnsupportedFormat)
            })?;
        }

        self.primary_buffer = Some(primary_buffer);
        self.initialized = true;

        Ok(())
    }

    /// Create secondary buffer for audio playback
    pub fn create_buffer(&self, size: u32, format: AudioFormat) -> Result<DirectSoundBuffer> {
        if !self.initialized {
            return Err(crate::error::Error::Device(
                crate::error::DeviceError::InitializationFailed(
                    "DirectSound not initialized".to_string(),
                ),
            ));
        }

        let wave_format = crate::windows::WindowsAudioUtils::audio_format_to_waveformatex(&format);

        let buffer_desc = DSBUFFERDESC {
            dwSize: std::mem::size_of::<DSBUFFERDESC>() as u32,
            dwFlags: DSBCAPS_CTRLVOLUME | DSBCAPS_CTRLFREQUENCY | DSBCAPS_GETCURRENTPOSITION2,
            dwBufferBytes: size,
            dwReserved: 0,
            lpwfxFormat: &wave_format as *const _ as *mut _,
            guid3DAlgorithm: Default::default(),
        };

        let buffer = unsafe {
            let mut buffer = std::mem::zeroed();
            self.direct_sound
                .CreateSoundBuffer(&buffer_desc, &mut buffer, None)
                .map_err(|e| {
                    crate::error::Error::Device(crate::error::DeviceError::InitializationFailed(
                        format!("CreateSoundBuffer failed: {:?}", e),
                    ))
                })?;
            buffer
        };

        Ok(DirectSoundBuffer {
            buffer,
            format,
            size,
        })
    }

    /// Create 3D audio buffer
    pub fn create_3d_buffer(&self, size: u32, format: AudioFormat) -> Result<DirectSound3DBuffer> {
        let wave_format = crate::windows::WindowsAudioUtils::audio_format_to_waveformatex(&format);

        let buffer_desc = DSBUFFERDESC {
            dwSize: std::mem::size_of::<DSBUFFERDESC>() as u32,
            dwFlags: DSBCAPS_CTRL3D | DSBCAPS_CTRLVOLUME | DSBCAPS_MUTE3DATMAXDISTANCE,
            dwBufferBytes: size,
            dwReserved: 0,
            lpwfxFormat: &wave_format as *const _ as *mut _,
            guid3DAlgorithm: DS3DALG_DEFAULT,
        };

        let buffer = unsafe {
            let mut buffer = std::mem::zeroed();
            self.direct_sound
                .CreateSoundBuffer(&buffer_desc, &mut buffer, None)
                .map_err(|e| {
                    crate::error::Error::Device(crate::error::DeviceError::InitializationFailed(
                        format!("Create3DSoundBuffer failed: {:?}", e),
                    ))
                })?;
            buffer
        };

        let buffer_3d = unsafe {
            buffer
                .QueryInterface::<IDirectSound3DBuffer>()
                .map_err(|e| {
                    crate::error::Error::Device(crate::error::DeviceError::InitializationFailed(
                        format!("QueryInterface IDirectSound3DBuffer failed: {:?}", e),
                    ))
                })?
        };

        Ok(DirectSound3DBuffer {
            buffer,
            buffer_3d,
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
        })
    }

    /// Get DirectSound capabilities
    pub fn get_capabilities(&self) -> Result<DSCAPS> {
        let mut caps = DSCAPS {
            dwSize: std::mem::size_of::<DSCAPS>() as u32,
            ..Default::default()
        };

        unsafe {
            self.direct_sound.GetCaps(&mut caps).map_err(|e| {
                crate::error::Error::Device(crate::error::DeviceError::InitializationFailed(
                    format!("GetCaps failed: {:?}", e),
                ))
            })?;
        }

        Ok(caps)
    }
}

#[cfg(windows)]
impl DirectSoundBuffer {
    /// Write audio data to buffer
    pub fn write_data(&self, data: &[u8], offset: u32) -> Result<u32> {
        let mut ptr1 = ptr::null_mut();
        let mut size1 = 0;
        let mut ptr2 = ptr::null_mut();
        let mut size2 = 0;

        unsafe {
            self.buffer
                .Lock(
                    offset,
                    data.len() as u32,
                    &mut ptr1,
                    &mut size1,
                    &mut ptr2,
                    &mut size2,
                    0,
                )
                .map_err(|_| crate::error::Error::Audio("Buffer lock failed".to_string()))?;

            // Copy data to first segment
            if !ptr1.is_null() && size1 > 0 {
                let copy_size1 = (size1 as usize).min(data.len());
                ptr::copy_nonoverlapping(data.as_ptr(), ptr1 as *mut u8, copy_size1);
            }

            // Copy data to second segment (if buffer wrapped)
            if !ptr2.is_null() && size2 > 0 && data.len() > size1 as usize {
                let remaining = data.len() - size1 as usize;
                let copy_size2 = (size2 as usize).min(remaining);
                ptr::copy_nonoverlapping(
                    data[size1 as usize..].as_ptr(),
                    ptr2 as *mut u8,
                    copy_size2,
                );
            }

            self.buffer
                .Unlock(ptr1, size1, ptr2, size2)
                .map_err(|_| crate::error::Error::Audio("Buffer unlock failed".to_string()))?;
        }

        Ok(size1 + size2)
    }

    /// Play buffer
    pub fn play(&self, looping: bool) -> Result<()> {
        let flags = if looping { DSBPLAY_LOOPING } else { 0 };

        unsafe {
            self.buffer
                .Play(0, 0, flags)
                .map_err(|_| crate::error::Error::Audio("Buffer play failed".to_string()))?;
        }

        Ok(())
    }

    /// Stop buffer playback
    pub fn stop(&self) -> Result<()> {
        unsafe {
            self.buffer
                .Stop()
                .map_err(|_| crate::error::Error::Audio("Buffer stop failed".to_string()))?;
        }

        Ok(())
    }

    /// Set volume (0-100)
    pub fn set_volume(&self, volume: crate::Volume) -> Result<()> {
        // DirectSound volume is in hundredths of decibels (negative values)
        let ds_volume = if volume == 0 {
            DSBVOLUME_MIN
        } else {
            let linear = (volume as f32) / 100.0;
            let db = 20.0 * linear.log10();
            (db * 100.0) as i32
        };

        unsafe {
            self.buffer
                .SetVolume(ds_volume)
                .map_err(|_| crate::error::Error::Audio("SetVolume failed".to_string()))?;
        }

        Ok(())
    }

    /// Get current play position
    pub fn get_position(&self) -> Result<(u32, u32)> {
        let mut play_pos = 0;
        let mut write_pos = 0;

        unsafe {
            self.buffer
                .GetCurrentPosition(Some(&mut play_pos), Some(&mut write_pos))
                .map_err(|_| crate::error::Error::Audio("GetCurrentPosition failed".to_string()))?;
        }

        Ok((play_pos, write_pos))
    }
}

#[cfg(windows)]
impl DirectSound3DBuffer {
    /// Set 3D position
    pub fn set_position(&mut self, x: f32, y: f32, z: f32) -> Result<()> {
        self.position = [x, y, z];

        unsafe {
            self.buffer_3d
                .SetPosition(x, y, z, DS3D_IMMEDIATE)
                .map_err(|_| crate::error::Error::Audio("SetPosition failed".to_string()))?;
        }

        Ok(())
    }

    /// Set 3D velocity
    pub fn set_velocity(&mut self, x: f32, y: f32, z: f32) -> Result<()> {
        self.velocity = [x, y, z];

        unsafe {
            self.buffer_3d
                .SetVelocity(x, y, z, DS3D_IMMEDIATE)
                .map_err(|_| crate::error::Error::Audio("SetVelocity failed".to_string()))?;
        }

        Ok(())
    }

    /// Set minimum and maximum distance for 3D audio
    pub fn set_distance(&self, min_distance: f32, max_distance: f32) -> Result<()> {
        unsafe {
            self.buffer_3d
                .SetMinDistance(min_distance, DS3D_IMMEDIATE)
                .map_err(|_| crate::error::Error::Audio("SetMinDistance failed".to_string()))?;

            self.buffer_3d
                .SetMaxDistance(max_distance, DS3D_IMMEDIATE)
                .map_err(|_| crate::error::Error::Audio("SetMaxDistance failed".to_string()))?;
        }

        Ok(())
    }
}

// Stub implementations for non-Windows platforms
#[cfg(not(windows))]
pub struct DirectSoundDriver;

#[cfg(not(windows))]
pub struct DirectSoundBuffer;

#[cfg(not(windows))]
pub struct DirectSound3DBuffer;

#[cfg(not(windows))]
impl DirectSoundDriver {
    pub fn new(_hwnd: Option<isize>) -> Result<Self> {
        Err(crate::error::Error::Audio(
            "DirectSound not available on this platform".to_string(),
        ))
    }
}
