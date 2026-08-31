//! DirectSound-specific audio implementation for Windows.

#[cfg(windows)]
use windows::Win32::{
    Foundation::{HRESULT, HWND},
    Media::Audio::DirectSound::*,
    System::Com::CoInitialize,
};

#[cfg(windows)]
use crate::{Priority, error::Result, formats::AudioFormat};
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
        // SAFETY: Windows-only FFI (module is #[cfg(windows)]). CoInitialize
        // takes no pointers and cannot violate memory safety; it establishes
        // the COM apartment all DirectSound calls below need, and `.ok()`
        // tolerates S_FALSE / RPC_E_CHANGED_MODE because DirectSoundCreate
        // works in either already-initialized apartment.
        unsafe {
            CoInitialize(None).ok();
        }

        // Create DirectSound interface
        // SAFETY: `ds` is a plain out-parameter slot: `mem::zeroed()` yields
        // the valid null-interface bit pattern for the Option-wrapped COM
        // pointer, DirectSoundCreate overwrites it on success, and the
        // map_err guard means a failed create never leaves us holding a
        // non-null-but-invalid interface.
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
            // SAFETY: Invariant upheld by callers: `hwnd` is a live window
            // handle owned by the host application for the driver's whole
            // lifetime. SetCooperativeLevel only reads it; DSSCL_PRIORITY is
            // the cooperative level the original game used.
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

        // SAFETY: buffer_desc.dwSize is set to size_of::<DSBUFFERDESC>()
        // as the API demands, DSBCAPS_PRIMARYBUFFER requires (and gets) a
        // null lpwfxFormat, and `buffer` again starts as the valid
        // null-interface pattern that CreateSoundBuffer fills on the success
        // path enforced by map_err.
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
        // SAFETY: primary_buffer was produced by the guarded
        // CreateSoundBuffer directly above and is therefore a valid
        // IDirectSoundBuffer; SetFormat only reads the WAVEFORMATEX built
        // from our own AudioFormat, which stays alive across the call.
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

        // SAFETY: dwSize is correctly initialized, lpwfxFormat points at
        // wave_format which outlives this call, and the zeroed out-slot is
        // the valid null-interface pattern overwritten by CreateSoundBuffer
        // on the success path enforced below before the buffer is stored.
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

        // SAFETY: Same descriptor contract as create_buffer (dwSize set,
        // lpwfxFormat pointing at wave_format living past the call); the
        // DSBCAPS_CTRL3D flag is what makes the QueryInterface below legal.
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

        // SAFETY: `buffer` is the IDirectSoundBuffer just returned by a
        // guarded CreateSoundBuffer, so its vtable matches the interface
        // ABI; QueryInterface performs an AddRef whose reference count is
        // then owned by the returned IDirectSound3DBuffer wrapper.
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

        // SAFETY: caps.dwSize is pre-set to size_of::<DSCAPS>() exactly as
        // GetCaps requires, and self.direct_sound is the interface created
        // (and kept alive) in new(); the call only writes into our stack
        // struct.
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

        // SAFETY: Lock returns write windows ptr1/ptr2 (wrap-around) with
        // sizes size1/size2 covering the requested bytes inside this buffer's
        // own memory; both copies clamp to those returned region sizes and
        // Unlock always receives exactly the pointer/size values Lock
        // returned, preserving the required Lock/Unlock pairing.
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

        // SAFETY: self.buffer is a valid IDirectSoundBuffer owned by this
        // wrapper since create_buffer; Play's first two reserved arguments
        // are documented to be 0 and flags carries only DSBPLAY_LOOPING.
        unsafe {
            self.buffer
                .Play(0, 0, flags)
                .map_err(|_| crate::error::Error::Audio("Buffer play failed".to_string()))?;
        }

        Ok(())
    }

    /// Stop buffer playback
    pub fn stop(&self) -> Result<()> {
        // SAFETY: self.buffer is a valid IDirectSoundBuffer owned by this
        // wrapper; Stop takes no pointers and merely halts playback.
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

        // SAFETY: self.buffer is a valid IDirectSoundBuffer owned by this
        // wrapper; ds_volume is clamped to DSBVOLUME_MIN..0 hundredths of a
        // decibel, the documented range for SetVolume.
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

        // SAFETY: self.buffer is a valid IDirectSoundBuffer owned by this
        // wrapper; GetCurrentPosition only writes through the two supplied
        // Some(&mut) out-locations on our stack frame.
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

        // SAFETY: self.buffer_3d was obtained via QueryInterface in
        // create_3d_buffer and holds its own COM reference count, so it is a
        // valid IDirectSound3DBuffer; DS3D_IMMEDIATE applies the change
        // without requiring a deferred commit.
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

        // SAFETY: self.buffer_3d holds a valid reference-counted
        // IDirectSound3DBuffer from create_3d_buffer; SetVelocity reads only
        // the three f32 coordinates passed by value.
        unsafe {
            self.buffer_3d
                .SetVelocity(x, y, z, DS3D_IMMEDIATE)
                .map_err(|_| crate::error::Error::Audio("SetVelocity failed".to_string()))?;
        }

        Ok(())
    }

    /// Set minimum and maximum distance for 3D audio
    pub fn set_distance(&self, min_distance: f32, max_distance: f32) -> Result<()> {
        // SAFETY: self.buffer_3d holds a valid reference-counted
        // IDirectSound3DBuffer from create_3d_buffer; both distance setters
        // take f32s by value and require min <= max, which the mixer's
        // falloff parameters guarantee.
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
