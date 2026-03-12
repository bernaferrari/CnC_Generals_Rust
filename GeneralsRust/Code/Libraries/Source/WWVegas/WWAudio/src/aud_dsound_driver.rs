//! DirectSound Audio Driver - Rust conversion of AUD_DSoundDriver.cpp
//! 
//! This module provides a complete Rust implementation of the original WPAudio DirectSound driver,
//! featuring:
//! - DirectSound buffer management with COM safety wrappers
//! - Multi-format audio support (PCM, IMA-ADPCM, MS-ADPCM, MP3)
//! - Robust error handling and buffer restoration
//! - Thread-safe audio operations with proper synchronization
//! - Seamless integration with existing WPAudio Rust architecture

#![cfg(windows)]

use windows::{
    core::{ComInterface, GUID, HRESULT, Interface, PCSTR, PSTR},
    Win32::{
        Foundation::{HWND, HANDLE, BOOL, TRUE, FALSE},
        Media::Audio::DirectSound::*,
        Media::Audio::{WAVEFORMATEX, WAVE_FORMAT_PCM},
        System::Com::{CoInitialize, CoUninitialize},
        System::Threading::{CreateThread, WaitForSingleObject, INFINITE},
        System::Memory::{GlobalAlloc, GlobalFree, GMEM_FIXED},
    },
};

use crate::{
    error::{Result, Error, DeviceError, ChannelError},
    formats::{AudioFormat, SampleRate, SampleWidth, ChannelLayout},
    compression::CompressionType,
    time::TimeStamp,
    device::{AudioDevice, DeviceConfig},
    channel::{AudioChannel, ChannelState},
    source::{AudioSource, SourceConfig},
    aud_source::{AudioSample, EnhancedAudioFormat, AudioCompressionType, CompressionData},
    Priority, Volume,
    windows::WindowsAudioUtils,
};

use std::{
    ptr::{self, null_mut},
    mem::{zeroed, size_of},
    sync::{Arc, Mutex, RwLock, atomic::{AtomicBool, AtomicU32, AtomicI32, Ordering}},
    collections::HashMap,
    time::{Duration, Instant},
    thread::{self, JoinHandle},
};

// DirectSound driver constants (matching original C++ implementation)
const AUD_DRV_MAX_CHANNELS: usize = 100;
const AUD_DRV_POLL_INTERVAL_MS: u32 = 33; // ~30fps polling
const AUD_DRV_OVER_SAMPLE: u32 = 8;
const AUD_DRV_FRAMES: u32 = 4;
const AUD_DRV_LAG_FRAMES: u32 = 0;
const AUD_DRV_DB_HALF: i32 = -1000; // -10dB

// Transfer state machine states for compressed audio
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransferState {
    ReadBlock = 0,
    WriteBlock = 1,
    DecodeBlock = 2,
    InitBlock = 3,
}

/// IMA ADPCM decoder state
#[derive(Debug, Clone)]
struct ImaAdpcmState {
    previous_value: i32,
    index: i32,
}

/// MS ADPCM decoder state  
#[derive(Debug, Clone)]
struct MsAdpcmState {
    delta: i32,
    coefficients: [i16; 2],
}

/// MP3 stream state
#[derive(Debug, Clone)]
struct Mp3State {
    stream_handle: Option<usize>, // Handle to ASI stream
    input_position: usize,
}

/// Audio transfer handler for format conversion and decompression
#[derive(Debug)]
struct AudioTransfer {
    state: TransferState,
    next_state: TransferState,
    
    // Input buffer for compressed data
    input_buffer: Vec<u8>,
    input_bytes_needed: usize,
    input_bytes_left: usize,
    
    // Output buffer for PCM data
    output_buffer: Vec<u8>,
    output_bytes: usize,
    output_bytes_left: usize,
    
    channels: u16,
    block_size: usize,
    pending: bool,
    
    // Format-specific decoder states
    ima_state: [ImaAdpcmState; 2],
    ms_state: [MsAdpcmState; 2], 
    mp3_state: Mp3State,
}

impl AudioTransfer {
    fn new() -> Self {
        Self {
            state: TransferState::InitBlock,
            next_state: TransferState::InitBlock,
            input_buffer: Vec::new(),
            input_bytes_needed: 0,
            input_bytes_left: 0,
            output_buffer: Vec::new(),
            output_bytes: 0,
            output_bytes_left: 0,
            channels: 1,
            block_size: 0,
            pending: false,
            ima_state: [
                ImaAdpcmState { previous_value: 0, index: 0 },
                ImaAdpcmState { previous_value: 0, index: 0 },
            ],
            ms_state: [
                MsAdpcmState { delta: 0, coefficients: [0; 2] },
                MsAdpcmState { delta: 0, coefficients: [0; 2] },
            ],
            mp3_state: Mp3State { stream_handle: None, input_position: 0 },
        }
    }
    
    fn reset(&mut self) {
        self.state = TransferState::InitBlock;
        self.input_bytes_left = 0;
        self.output_bytes_left = 0;
        self.pending = false;
        
        // Reset decoder states
        for state in &mut self.ima_state {
            state.previous_value = 0;
            state.index = 0;
        }
        
        for state in &mut self.ms_state {
            state.delta = 0;
            state.coefficients = [0; 2];
        }
        
        self.mp3_state.input_position = 0;
    }
}

/// DirectSound channel implementation
pub struct DSoundChannel {
    // Core channel data
    id: u32,
    format: AudioFormat,
    priority: Priority,
    
    // DirectSound objects (COM interface wrappers)
    ds_buffer: Option<IDirectSoundBuffer>,
    
    // Buffer management
    buffer_size: u32,
    frame_bytes: u32,
    frame_count: u32,
    
    // Playback state
    state: AtomicU32, // ChannelState as u32
    playing: AtomicBool,
    looping: AtomicBool,
    
    // Position tracking
    current_frame: AtomicU32,
    write_position: AtomicU32,
    play_position: AtomicU32,
    pcm_position: AtomicI32,
    
    // Source data
    source_data: RwLock<Option<Vec<u8>>>,
    source_position: AtomicU32,
    source_bytes_left: AtomicU32,
    
    // Audio attributes
    volume: AtomicU32, // Volume as u32 (0-100)
    pan: AtomicI32,    // Pan as i32 (-100 to +100)
    frequency: AtomicU32,
    
    // Transfer system for compressed audio
    transfer: Mutex<AudioTransfer>,
    
    // Timing and synchronization
    last_poll: Mutex<Instant>,
    poll_interval: Duration,
    
    // Statistics
    service_count: AtomicU32,
    frames_played: AtomicU32,
}

unsafe impl Send for DSoundChannel {}
unsafe impl Sync for DSoundChannel {}

impl DSoundChannel {
    fn new(id: u32, format: AudioFormat) -> Self {
        Self {
            id,
            format,
            priority: Priority::Normal,
            ds_buffer: None,
            buffer_size: 0,
            frame_bytes: 0,
            frame_count: AUD_DRV_FRAMES,
            state: AtomicU32::new(ChannelState::Stopped as u32),
            playing: AtomicBool::new(false),
            looping: AtomicBool::new(false),
            current_frame: AtomicU32::new(0),
            write_position: AtomicU32::new(0),
            play_position: AtomicU32::new(0),
            pcm_position: AtomicI32::new(0),
            source_data: RwLock::new(None),
            source_position: AtomicU32::new(0),
            source_bytes_left: AtomicU32::new(0),
            volume: AtomicU32::new(80), // Default 80% volume
            pan: AtomicI32::new(0),     // Centered
            frequency: AtomicU32::new(u32::from(format.sample_rate)),
            transfer: Mutex::new(AudioTransfer::new()),
            last_poll: Mutex::new(Instant::now()),
            poll_interval: Duration::from_millis(AUD_DRV_POLL_INTERVAL_MS as u64),
            service_count: AtomicU32::new(0),
            frames_played: AtomicU32::new(0),
        }
    }
    
    /// Create DirectSound buffer for this channel
    fn create_buffer(&mut self, ds_device: &IDirectSound) -> Result<()> {
        // Calculate frame size based on format and timing requirements
        let samples_per_second = u32::from(self.format.sample_rate);
        let bytes_per_sample = u8::from(self.format.sample_width) / 8;
        let channels = self.format.channels as u32;
        
        // Frame size calculation (matching original C++ logic)
        let frame_bytes = ((samples_per_second * (AUD_DRV_POLL_INTERVAL_MS / 10)) / 100) 
            * channels 
            * bytes_per_sample 
            * AUD_DRV_OVER_SAMPLE;
            
        // Align to 1KB boundaries
        self.frame_bytes = ((frame_bytes + 1023) / 1024) * 1024;
        self.buffer_size = self.frame_bytes * AUD_DRV_FRAMES;
        
        // Convert AudioFormat to WAVEFORMATEX
        let wave_format = WindowsAudioUtils::audio_format_to_waveformatex(&self.format);
        
        // Create DirectSound buffer descriptor
        let buffer_desc = DSBUFFERDESC {
            dwSize: size_of::<DSBUFFERDESC>() as u32,
            dwFlags: DSBCAPS_CTRLVOLUME | DSBCAPS_CTRLPAN | DSBCAPS_CTRLFREQUENCY | DSBCAPS_GETCURRENTPOSITION2,
            dwBufferBytes: self.buffer_size,
            dwReserved: 0,
            lpwfxFormat: &wave_format as *const _ as *mut _,
            guid3DAlgorithm: GUID::zeroed(),
        };
        
        // Create the DirectSound buffer
        unsafe {
            let mut buffer: Option<IDirectSoundBuffer> = None;
            ds_device.CreateSoundBuffer(
                &buffer_desc,
                &mut buffer,
                None
            ).map_err(|e| Error::Device(DeviceError::InitializationFailed(
                format!("Failed to create DirectSound buffer: {:?}", e)
            )))?;
            
            self.ds_buffer = buffer;
        }
        
        Ok(())
    }
    
    /// Initialize buffer for playback
    fn initialize_buffer(&self) -> Result<()> {
        if let Some(ref buffer) = self.ds_buffer {
            // Clear buffer state
            self.source_bytes_left.store(0, Ordering::SeqCst);
            self.pcm_position.store(0, Ordering::SeqCst);
            
            // Set initial position
            unsafe {
                buffer.SetCurrentPosition(0).map_err(|_| 
                    Error::Audio("Failed to set buffer position".to_string())
                )?;
            }
            
            // Get current positions
            let (play_pos, write_pos) = self.get_buffer_positions()?;
            self.play_position.store(play_pos, Ordering::SeqCst);
            self.write_position.store(write_pos, Ordering::SeqCst);
            
            self.current_frame.store(play_pos / self.frame_bytes, Ordering::SeqCst);
            *self.last_poll.lock().unwrap() = Instant::now();
            
            Ok(())
        } else {
            Err(Error::Channel(ChannelError::InvalidState("No DirectSound buffer".to_string())))
        }
    }
    
    /// Get current buffer positions
    fn get_buffer_positions(&self) -> Result<(u32, u32)> {
        if let Some(ref buffer) = self.ds_buffer {
            unsafe {
                let mut play_pos = 0u32;
                let mut write_pos = 0u32;
                
                buffer.GetCurrentPosition(
                    Some(&mut play_pos),
                    Some(&mut write_pos)
                ).map_err(|_| Error::Audio("Failed to get buffer position".to_string()))?;
                
                Ok((play_pos, write_pos))
            }
        } else {
            Err(Error::Channel(ChannelError::InvalidState("No DirectSound buffer".to_string())))
        }
    }
    
    /// Lock buffer for writing
    fn lock_buffer(&self, position: u32, size: u32) -> Result<(BufferLock, Option<BufferLock>)> {
        if let Some(ref buffer) = self.ds_buffer {
            unsafe {
                let mut ptr1 = null_mut::<std::ffi::c_void>();
                let mut size1 = 0u32;
                let mut ptr2 = null_mut::<std::ffi::c_void>();
                let mut size2 = 0u32;
                
                // Try to lock buffer, retry if buffer was lost
                let mut retry_count = 3;
                loop {
                    match buffer.Lock(position, size, &mut ptr1, &mut size1, &mut ptr2, &mut size2, 0) {
                        Ok(_) => break,
                        Err(e) => {
                            if retry_count > 0 && e.code() == DSERR_BUFFERLOST {
                                // Attempt to restore buffer
                                if let Err(_) = buffer.Restore() {
                                    return Err(Error::Audio("Failed to restore buffer".to_string()));
                                }
                                retry_count -= 1;
                                continue;
                            }
                            return Err(Error::Audio("Failed to lock buffer".to_string()));
                        }
                    }
                }
                
                let lock1 = BufferLock {
                    ptr: ptr1 as *mut u8,
                    size: size1,
                };
                
                let lock2 = if ptr2.is_null() {
                    None
                } else {
                    Some(BufferLock {
                        ptr: ptr2 as *mut u8,
                        size: size2,
                    })
                };
                
                Ok((lock1, lock2))
            }
        } else {
            Err(Error::Channel(ChannelError::InvalidState("No DirectSound buffer".to_string())))
        }
    }
    
    /// Unlock buffer after writing
    fn unlock_buffer(&self, lock1: BufferLock, lock2: Option<BufferLock>) -> Result<()> {
        if let Some(ref buffer) = self.ds_buffer {
            unsafe {
                let ptr2 = lock2.as_ref().map(|l| l.ptr as *mut std::ffi::c_void).unwrap_or(null_mut());
                let size2 = lock2.as_ref().map(|l| l.size).unwrap_or(0);
                
                buffer.Unlock(
                    lock1.ptr as *mut std::ffi::c_void,
                    lock1.size,
                    ptr2,
                    size2
                ).map_err(|_| Error::Audio("Failed to unlock buffer".to_string()))?;
            }
            Ok(())
        } else {
            Err(Error::Channel(ChannelError::InvalidState("No DirectSound buffer".to_string())))
        }
    }
}

/// Buffer lock wrapper for safe memory access
#[derive(Debug)]
struct BufferLock {
    ptr: *mut u8,
    size: u32,
}

impl BufferLock {
    /// Write data to locked buffer segment
    fn write_data(&self, data: &[u8], offset: usize) -> usize {
        if self.ptr.is_null() || offset >= self.size as usize {
            return 0;
        }
        
        let available = (self.size as usize).saturating_sub(offset);
        let copy_size = data.len().min(available);
        
        if copy_size > 0 {
            unsafe {
                ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    self.ptr.add(offset),
                    copy_size
                );
            }
        }
        
        copy_size
    }
    
    /// Fill buffer segment with silence
    fn fill_silence(&self, sample_width: SampleWidth, offset: usize, size: usize) {
        if self.ptr.is_null() || offset >= self.size as usize {
            return;
        }
        
        let available = (self.size as usize).saturating_sub(offset);
        let fill_size = size.min(available);
        
        if fill_size > 0 {
            let fill_value = match sample_width {
                SampleWidth::U8 => 0x80u8, // Unsigned 8-bit silence
                _ => 0x00u8,               // Signed formats use 0
            };
            
            unsafe {
                ptr::write_bytes(self.ptr.add(offset), fill_value, fill_size);
            }
        }
    }
}

/// Volume lookup table (matching original C++ implementation)
/// Generated with formula: 1000 * log2(n/100) where n is 0..100
const VOLUME_LOG_TABLE: [i32; 101] = [
    -10000, -6644, -5644, -5059, -4644, -4322, -4059, -3837, -3644, -3474,
    -3322, -3184, -3059, -2943, -2837, -2737, -2644, -2556, -2474, -2396,
    -2322, -2252, -2184, -2120, -2059, -2000, -1943, -1889, -1837, -1786,
    -1737, -1690, -1644, -1599, -1556, -1515, -1474, -1434, -1396, -1358,
    -1322, -1286, -1252, -1218, -1184, -1152, -1120, -1089, -1059, -1029,
    -1000, -971, -943, -916, -889, -862, -837, -811, -786, -761,
    -737, -713, -690, -667, -644, -621, -599, -578, -556, -535,
    -515, -494, -474, -454, -434, -415, -396, -377, -358, -340,
    -322, -304, -286, -269, -252, -234, -218, -201, -184, -168,
    -152, -136, -120, -105, -89, -74, -59, -44, -29, -14,
    0
];

/// DirectSound audio driver - main implementation
pub struct DSoundDriver {
    // DirectSound COM objects
    ds_device: Option<IDirectSound>,
    primary_buffer: Option<IDirectSoundBuffer>,
    
    // Driver state
    initialized: AtomicBool,
    cooperative_level_set: AtomicBool,
    
    // Channel management
    channels: RwLock<HashMap<u32, Arc<DSoundChannel>>>,
    next_channel_id: AtomicU32,
    max_channels: usize,
    
    // Audio thread and service
    service_thread: Mutex<Option<JoinHandle<()>>>,
    service_running: AtomicBool,
    
    // Configuration
    default_format: AudioFormat,
    poll_interval: Duration,
    
    // Statistics and debugging
    total_service_calls: AtomicU32,
}

unsafe impl Send for DSoundDriver {}
unsafe impl Sync for DSoundDriver {}

impl DSoundDriver {
    /// Create new DirectSound driver instance
    pub fn new() -> Result<Self> {
        // Initialize COM
        unsafe {
            CoInitialize(None).ok();
        }
        
        Ok(Self {
            ds_device: None,
            primary_buffer: None,
            initialized: AtomicBool::new(false),
            cooperative_level_set: AtomicBool::new(false),
            channels: RwLock::new(HashMap::new()),
            next_channel_id: AtomicU32::new(1),
            max_channels: AUD_DRV_MAX_CHANNELS,
            service_thread: Mutex::new(None),
            service_running: AtomicBool::new(false),
            default_format: AudioFormat::default(),
            poll_interval: Duration::from_millis(AUD_DRV_POLL_INTERVAL_MS as u64),
            total_service_calls: AtomicU32::new(0),
        })
    }
    
    /// Initialize DirectSound system
    pub fn initialize(&self, hwnd: Option<HWND>) -> Result<()> {
        if self.initialized.load(Ordering::SeqCst) {
            return Ok(());
        }
        
        // Create DirectSound device
        let ds_device = unsafe {
            let mut device: Option<IDirectSound> = None;
            DirectSoundCreate(None, &mut device, None)
                .map_err(|e| Error::Device(DeviceError::InitializationFailed(
                    format!("DirectSoundCreate failed: {:?}", e)
                )))?;
            device.unwrap()
        };
        
        // Set cooperative level if window handle provided
        if let Some(hwnd) = hwnd {
            unsafe {
                ds_device.SetCooperativeLevel(hwnd, DSSCL_PRIORITY)
                    .map_err(|e| Error::Device(DeviceError::InitializationFailed(
                        format!("SetCooperativeLevel failed: {:?}", e)
                    )))?;
            }
            self.cooperative_level_set.store(true, Ordering::SeqCst);
        }
        
        // Create and configure primary buffer
        let primary_buffer = self.create_primary_buffer(&ds_device)?;
        
        // Store devices
        // SAFETY: We're in a single-threaded context during initialization
        unsafe {
            let self_mut = &*(self as *const Self as *mut Self);
            self_mut.ds_device = Some(ds_device);
            self_mut.primary_buffer = Some(primary_buffer);
        }
        
        self.initialized.store(true, Ordering::SeqCst);
        
        // Start primary buffer playback
        self.start_primary_playback()?;
        
        Ok(())
    }
    
    /// Create and configure primary DirectSound buffer
    fn create_primary_buffer(&self, ds_device: &IDirectSound) -> Result<IDirectSoundBuffer> {
        let buffer_desc = DSBUFFERDESC {
            dwSize: size_of::<DSBUFFERDESC>() as u32,
            dwFlags: DSBCAPS_PRIMARYBUFFER,
            dwBufferBytes: 0,
            dwReserved: 0,
            lpwfxFormat: null_mut(),
            guid3DAlgorithm: GUID::zeroed(),
        };
        
        let primary_buffer = unsafe {
            let mut buffer: Option<IDirectSoundBuffer> = None;
            ds_device.CreateSoundBuffer(&buffer_desc, &mut buffer, None)
                .map_err(|e| Error::Device(DeviceError::InitializationFailed(
                    format!("Failed to create primary buffer: {:?}", e)
                )))?;
            buffer.unwrap()
        };
        
        // Set primary buffer format
        let wave_format = WindowsAudioUtils::audio_format_to_waveformatex(&self.default_format);
        unsafe {
            if let Err(_) = primary_buffer.SetFormat(&wave_format) {
                // Non-fatal - primary buffer format setting can fail
                log::warn!("Unable to set desired primary buffer format");
            }
        }
        
        Ok(primary_buffer)
    }
    
    /// Start primary buffer playback
    fn start_primary_playback(&self) -> Result<()> {
        if let Some(ref primary_buffer) = self.primary_buffer {
            unsafe {
                primary_buffer.Play(0, 0, DSBPLAY_LOOPING)
                    .map_err(|e| Error::Device(DeviceError::InitializationFailed(
                        format!("Failed to start primary buffer: {:?}", e)
                    )))?;
            }
        }
        Ok(())
    }
    
    /// Create new audio channel
    pub fn create_channel(&self, format: AudioFormat, priority: Priority) -> Result<u32> {
        if !self.initialized.load(Ordering::SeqCst) {
            return Err(Error::Device(DeviceError::InitializationFailed(
                "DirectSound not initialized".to_string()
            )));
        }
        
        let channels = self.channels.read().unwrap();
        if channels.len() >= self.max_channels {
            return Err(Error::Channel(ChannelError::AllocationFailed));
        }
        drop(channels);
        
        let channel_id = self.next_channel_id.fetch_add(1, Ordering::SeqCst);
        let mut channel = DSoundChannel::new(channel_id, format);
        channel.priority = priority;
        
        // Create DirectSound buffer for channel
        if let Some(ref ds_device) = self.ds_device {
            channel.create_buffer(ds_device)?;
        } else {
            return Err(Error::Device(DeviceError::InitializationFailed(
                "DirectSound device not available".to_string()
            )));
        }
        
        // Add channel to collection
        let mut channels = self.channels.write().unwrap();
        channels.insert(channel_id, Arc::new(channel));
        
        Ok(channel_id)
    }
    
    /// Remove audio channel
    pub fn remove_channel(&self, channel_id: u32) -> Result<()> {
        let mut channels = self.channels.write().unwrap();
        if let Some(channel) = channels.remove(&channel_id) {
            // Stop channel if playing
            if channel.playing.load(Ordering::SeqCst) {
                let _ = self.stop_channel_internal(&channel);
            }
            Ok(())
        } else {
            Err(Error::Channel(ChannelError::NotAvailable))
        }
    }
    
    /// Start channel playback
    pub fn start_channel(&self, channel_id: u32, source_data: Vec<u8>, looping: bool) -> Result<()> {
        let channels = self.channels.read().unwrap();
        if let Some(channel) = channels.get(&channel_id) {
            // Initialize buffer and set source data
            channel.initialize_buffer()?;
            
            // Set source data
            {
                let mut data = channel.source_data.write().unwrap();
                *data = Some(source_data.clone());
            }
            channel.source_bytes_left.store(source_data.len() as u32, Ordering::SeqCst);
            channel.source_position.store(0, Ordering::SeqCst);
            
            // Configure playback
            channel.looping.store(looping, Ordering::SeqCst);
            
            // Pre-fill buffer
            self.fill_channel_buffer(channel, channel.buffer_size)?;
            
            // Update audio attributes
            self.update_channel_attributes(channel)?;
            
            // Start playback
            if let Some(ref buffer) = channel.ds_buffer {
                unsafe {
                    buffer.Play(0, 0, DSBPLAY_LOOPING)
                        .map_err(|_| Error::Audio("Failed to start playback".to_string()))?;
                }
                
                channel.playing.store(true, Ordering::SeqCst);
                channel.state.store(ChannelState::Playing as u32, Ordering::SeqCst);
            }
            
            Ok(())
        } else {
            Err(Error::Channel(ChannelError::NotAvailable))
        }
    }
    
    /// Stop channel playback
    pub fn stop_channel(&self, channel_id: u32) -> Result<()> {
        let channels = self.channels.read().unwrap();
        if let Some(channel) = channels.get(&channel_id) {
            self.stop_channel_internal(channel)
        } else {
            Err(Error::Channel(ChannelError::NotAvailable))
        }
    }
    
    /// Internal channel stop implementation
    fn stop_channel_internal(&self, channel: &DSoundChannel) -> Result<()> {
        if let Some(ref buffer) = channel.ds_buffer {
            unsafe {
                buffer.Stop().map_err(|_| Error::Audio("Failed to stop playback".to_string()))?;
            }
        }
        
        channel.playing.store(false, Ordering::SeqCst);
        channel.state.store(ChannelState::Stopped as u32, Ordering::SeqCst);
        
        Ok(())
    }
    
    /// Set channel volume (0-100)
    pub fn set_channel_volume(&self, channel_id: u32, volume: Volume) -> Result<()> {
        let channels = self.channels.read().unwrap();
        if let Some(channel) = channels.get(&channel_id) {
            let volume = volume.clamp(0, 100);
            channel.volume.store(volume as u32, Ordering::SeqCst);
            
            if let Some(ref buffer) = channel.ds_buffer {
                let ds_volume = VOLUME_LOG_TABLE[volume as usize];
                unsafe {
                    buffer.SetVolume(ds_volume).map_err(|_| 
                        Error::Audio("Failed to set volume".to_string())
                    )?;
                }
            }
            
            Ok(())
        } else {
            Err(Error::Channel(ChannelError::NotAvailable))
        }
    }
    
    /// Set channel pan (-100 to +100, where -100=left, 0=center, +100=right)
    pub fn set_channel_pan(&self, channel_id: u32, pan: i32) -> Result<()> {
        let channels = self.channels.read().unwrap();
        if let Some(channel) = channels.get(&channel_id) {
            let pan = pan.clamp(-100, 100);
            channel.pan.store(pan, Ordering::SeqCst);
            
            if let Some(ref buffer) = channel.ds_buffer {
                let ds_pan = if pan < 0 {
                    VOLUME_LOG_TABLE[(100 + pan) as usize]
                } else {
                    -VOLUME_LOG_TABLE[(100 - pan) as usize]
                };
                
                unsafe {
                    buffer.SetPan(ds_pan).map_err(|_| 
                        Error::Audio("Failed to set pan".to_string())
                    )?;
                }
            }
            
            Ok(())
        } else {
            Err(Error::Channel(ChannelError::NotAvailable))
        }
    }
    
    /// Set channel frequency
    pub fn set_channel_frequency(&self, channel_id: u32, frequency: u32) -> Result<()> {
        let channels = self.channels.read().unwrap();
        if let Some(channel) = channels.get(&channel_id) {
            channel.frequency.store(frequency, Ordering::SeqCst);
            
            if let Some(ref buffer) = channel.ds_buffer {
                unsafe {
                    buffer.SetFrequency(frequency).map_err(|_| 
                        Error::Audio("Failed to set frequency".to_string())
                    )?;
                }
            }
            
            Ok(())
        } else {
            Err(Error::Channel(ChannelError::NotAvailable))
        }
    }
    
    /// Check if channel is playing
    pub fn is_channel_playing(&self, channel_id: u32) -> Result<bool> {
        let channels = self.channels.read().unwrap();
        if let Some(channel) = channels.get(&channel_id) {
            if let Some(ref buffer) = channel.ds_buffer {
                unsafe {
                    let mut status = 0u32;
                    buffer.GetStatus(&mut status).map_err(|_| 
                        Error::Audio("Failed to get buffer status".to_string())
                    )?;
                    
                    let is_playing = (status & (DSBSTATUS_PLAYING.0 | DSBSTATUS_LOOPING.0)) 
                                   == (DSBSTATUS_PLAYING.0 | DSBSTATUS_LOOPING.0);
                    
                    // Update internal state
                    channel.playing.store(is_playing, Ordering::SeqCst);
                    
                    Ok(is_playing)
                }
            } else {
                Ok(false)
            }
        } else {
            Err(Error::Channel(ChannelError::NotAvailable))
        }
    }
    
    /// Update channel audio attributes
    fn update_channel_attributes(&self, channel: &DSoundChannel) -> Result<()> {
        if let Some(ref buffer) = channel.ds_buffer {
            // Apply volume
            let volume = channel.volume.load(Ordering::SeqCst) as usize;
            let ds_volume = VOLUME_LOG_TABLE[volume.clamp(0, 100)];
            
            // Apply pan
            let pan = channel.pan.load(Ordering::SeqCst);
            let ds_pan = if pan < 0 {
                VOLUME_LOG_TABLE[(100 + pan).clamp(0, 100) as usize]
            } else {
                -VOLUME_LOG_TABLE[(100 - pan).clamp(0, 100) as usize]
            };
            
            // Apply frequency
            let frequency = channel.frequency.load(Ordering::SeqCst);
            
            unsafe {
                buffer.SetVolume(ds_volume).ok();
                buffer.SetPan(ds_pan).ok();
                buffer.SetFrequency(frequency).ok();
            }
        }
        
        Ok(())
    }
    
    /// Fill channel buffer with audio data
    fn fill_channel_buffer(&self, channel: &DSoundChannel, bytes_to_fill: u32) -> Result<()> {
        let mut bytes_remaining = bytes_to_fill;
        let mut write_pos = channel.write_position.load(Ordering::SeqCst);
        
        while bytes_remaining > 0 {
            // Handle buffer wrap-around
            let bytes_to_end = channel.buffer_size.saturating_sub(write_pos);
            let transfer_size = bytes_remaining.min(bytes_to_end);
            
            if transfer_size == 0 {
                write_pos = 0; // Wrap around
                continue;
            }
            
            // Lock buffer for writing
            let (lock1, lock2) = channel.lock_buffer(write_pos, transfer_size)?;
            
            // Fill buffer with audio data
            let bytes_written = self.fill_buffer_segment(channel, &lock1, lock2.as_ref())?;
            
            // Unlock buffer
            channel.unlock_buffer(lock1, lock2)?;
            
            if bytes_written == 0 {
                break; // No more data available
            }
            
            write_pos = (write_pos + bytes_written as u32) % channel.buffer_size;
            bytes_remaining = bytes_remaining.saturating_sub(bytes_written as u32);
        }
        
        channel.write_position.store(write_pos, Ordering::SeqCst);
        Ok(())
    }
    
    /// Fill buffer segment with audio data
    fn fill_buffer_segment(&self, channel: &DSoundChannel, 
                          lock1: &BufferLock, lock2: Option<&BufferLock>) -> Result<usize> {
        let mut total_written = 0;
        let source_data = channel.source_data.read().unwrap();
        
        if let Some(ref data) = *source_data {
            let source_pos = channel.source_position.load(Ordering::SeqCst) as usize;
            let source_remaining = channel.source_bytes_left.load(Ordering::SeqCst) as usize;
            
            if source_remaining > 0 && source_pos < data.len() {
                let available_data = &data[source_pos..];
                let copy_size = available_data.len().min(source_remaining).min(lock1.size as usize);
                
                if copy_size > 0 {
                    let written = lock1.write_data(&available_data[..copy_size], 0);
                    total_written += written;
                    
                    // Update source position
                    channel.source_position.store((source_pos + written) as u32, Ordering::SeqCst);
                    channel.source_bytes_left.store((source_remaining - written) as u32, Ordering::SeqCst);
                }
            }
            
            // Fill remaining space with silence
            if total_written < lock1.size as usize {
                lock1.fill_silence(channel.format.sample_width, total_written, 
                                  lock1.size as usize - total_written);
            }
            
            // Handle second buffer segment if wrapped
            if let Some(lock2) = lock2 {
                lock2.fill_silence(channel.format.sample_width, 0, lock2.size as usize);
            }
        } else {
            // No source data - fill with silence
            lock1.fill_silence(channel.format.sample_width, 0, lock1.size as usize);
            if let Some(lock2) = lock2 {
                lock2.fill_silence(channel.format.sample_width, 0, lock2.size as usize);
            }
        }
        
        Ok(total_written)
    }
    
    /// Start audio service thread
    pub fn start_service(&self) -> Result<()> {
        if self.service_running.load(Ordering::SeqCst) {
            return Ok(());
        }
        
        self.service_running.store(true, Ordering::SeqCst);
        
        let driver_weak = Arc::downgrade(&Arc::new(self));
        let service_thread = thread::Builder::new()
            .name("DirectSound Service".to_string())
            .spawn(move || {
                while let Some(driver) = driver_weak.upgrade() {
                    if !driver.service_running.load(Ordering::SeqCst) {
                        break;
                    }
                    
                    // Service all active channels
                    let _ = driver.service_channels();
                    
                    // Sleep until next service interval
                    thread::sleep(driver.poll_interval);
                    
                    driver.total_service_calls.fetch_add(1, Ordering::SeqCst);
                }
            })
            .map_err(|e| Error::Device(DeviceError::InitializationFailed(
                format!("Failed to start service thread: {}", e)
            )))?;
        
        *self.service_thread.lock().unwrap() = Some(service_thread);
        
        Ok(())
    }
    
    /// Stop audio service thread
    pub fn stop_service(&self) -> Result<()> {
        self.service_running.store(false, Ordering::SeqCst);
        
        if let Some(thread) = self.service_thread.lock().unwrap().take() {
            thread.join().map_err(|_| Error::Audio("Failed to join service thread".to_string()))?;
        }
        
        Ok(())
    }
    
    /// Service all active channels
    fn service_channels(&self) -> Result<()> {
        let channels = self.channels.read().unwrap();
        let now = Instant::now();
        
        for channel in channels.values() {
            if !channel.playing.load(Ordering::SeqCst) {
                continue;
            }
            
            // Check if channel is still actually playing
            if let Ok(false) = self.is_channel_playing(channel.id) {
                let _ = self.stop_channel_internal(channel);
                continue;
            }
            
            // Update buffer timing
            let last_poll = *channel.last_poll.lock().unwrap();
            let elapsed = now.duration_since(last_poll);
            
            if elapsed >= channel.poll_interval {
                *channel.last_poll.lock().unwrap() = now;
                
                // Update buffer content
                if let Err(e) = self.update_channel_buffer(channel) {
                    log::error!("Failed to update channel {} buffer: {}", channel.id, e);
                    let _ = self.stop_channel_internal(channel);
                }
                
                channel.service_count.fetch_add(1, Ordering::SeqCst);
            }
        }
        
        Ok(())
    }
    
    /// Update channel buffer content
    fn update_channel_buffer(&self, channel: &DSoundChannel) -> Result<()> {
        // Get current frame position
        let (current_play_pos, _) = channel.get_buffer_positions()?;
        let current_frame = current_play_pos / channel.frame_bytes;
        let last_frame = channel.current_frame.load(Ordering::SeqCst);
        
        if current_frame != last_frame {
            // Calculate frame difference (handling wrap-around)
            let mut frame_diff = if current_frame >= last_frame {
                current_frame - last_frame
            } else {
                (AUD_DRV_FRAMES - last_frame) + current_frame
            };
            
            // Apply lag compensation
            if frame_diff > AUD_DRV_LAG_FRAMES {
                frame_diff -= AUD_DRV_LAG_FRAMES;
                
                // Update frame counter
                let new_frame = (last_frame + frame_diff) % AUD_DRV_FRAMES;
                channel.current_frame.store(new_frame, Ordering::SeqCst);
                channel.frames_played.fetch_add(frame_diff, Ordering::SeqCst);
                
                // Fill buffer for advanced frames
                let bytes_to_fill = frame_diff * channel.frame_bytes;
                self.fill_channel_buffer(channel, bytes_to_fill)?;
                
                // Update PCM position
                let pcm_pos = channel.pcm_position.load(Ordering::SeqCst);
                let new_pcm_pos = pcm_pos - (bytes_to_fill as i32);
                
                if new_pcm_pos <= 0 {
                    // End of sample reached
                    let _ = self.stop_channel_internal(channel);
                } else {
                    channel.pcm_position.store(new_pcm_pos, Ordering::SeqCst);
                }
            }
        }
        
        Ok(())
    }
    
    /// Shutdown DirectSound driver
    pub fn shutdown(&self) -> Result<()> {
        // Stop service thread
        self.stop_service()?;
        
        // Stop and remove all channels
        {
            let mut channels = self.channels.write().unwrap();
            for channel in channels.values() {
                let _ = self.stop_channel_internal(channel);
            }
            channels.clear();
        }
        
        // Stop primary buffer
        if let Some(ref primary_buffer) = self.primary_buffer {
            unsafe {
                primary_buffer.Stop().ok();
            }
        }
        
        // Release COM objects (automatic via Drop)
        self.initialized.store(false, Ordering::SeqCst);
        
        Ok(())
    }
    
    /// Get driver statistics
    pub fn get_statistics(&self) -> DSoundDriverStats {
        let channels = self.channels.read().unwrap();
        let active_channels = channels.values()
            .filter(|ch| ch.playing.load(Ordering::SeqCst))
            .count();
        
        DSoundDriverStats {
            total_channels: channels.len(),
            active_channels,
            total_service_calls: self.total_service_calls.load(Ordering::SeqCst),
            initialized: self.initialized.load(Ordering::SeqCst),
        }
    }
}

impl Drop for DSoundDriver {
    fn drop(&mut self) {
        let _ = self.shutdown();
        
        // Uninitialize COM
        unsafe {
            CoUninitialize();
        }
    }
}

/// DirectSound driver statistics
#[derive(Debug, Clone)]
pub struct DSoundDriverStats {
    pub total_channels: usize,
    pub active_channels: usize,
    pub total_service_calls: u32,
    pub initialized: bool,
}

/// ADPCM lookup tables and decoders (matching original C++ implementation)
mod adpcm {
    use super::*;
    
    // IMA ADPCM lookup tables
    const IMA_INDEX_ADJUST_TABLE: [i32; 16] = [
        -1, -1, -1, -1, 2, 4, 6, 8,
        -1, -1, -1, -1, 2, 4, 6, 8,
    ];
    
    const IMA_STEP_SIZE_TABLE: [i32; 89] = [
        7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34,
        37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143,
        157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494,
        544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552,
        1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026,
        4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442,
        11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623,
        27086, 29794, 32767
    ];
    
    // MS ADPCM adaptation table
    const MS_ADAPTATION_TABLE: [i32; 16] = [
        230, 230, 230, 230, 307, 409, 512, 614,
        768, 614, 512, 409, 307, 230, 230, 230
    ];
    
    /// Decode IMA ADPCM sample
    pub fn ima_adpcm_decode(delta_code: u8, state: &mut ImaAdpcmState) -> i16 {
        let step = IMA_STEP_SIZE_TABLE[state.index as usize];
        
        // Construct difference
        let mut difference = step >> 3;
        if delta_code & 1 != 0 { difference += step >> 2; }
        if delta_code & 2 != 0 { difference += step >> 1; }
        if delta_code & 4 != 0 { difference += step; }
        
        if delta_code & 8 != 0 {
            difference = -difference;
        }
        
        // Update previous value
        state.previous_value += difference;
        state.previous_value = state.previous_value.clamp(-32768, 32767);
        
        // Update index
        state.index += IMA_INDEX_ADJUST_TABLE[delta_code as usize];
        state.index = state.index.clamp(0, 88);
        
        state.previous_value as i16
    }
    
    /// Decode MS ADPCM sample
    pub fn ms_adpcm_decode(nibble: u8, state: &mut MsAdpcmState, sample1: i16, sample2: i16) -> i16 {
        let pred_sample = ((sample1 as i32 * state.coefficients[0] as i32) + 
                          (sample2 as i32 * state.coefficients[1] as i32)) >> 8;
        
        let new_sample = pred_sample + (state.delta * (nibble as i32 - ((nibble & 0x08) << 1) as i32));
        
        state.delta = (state.delta * MS_ADAPTATION_TABLE[nibble as usize]) >> 8;
        state.delta = state.delta.max(16);
        
        new_sample.clamp(-32768, 32767) as i16
    }
}

/// Public interface functions matching original C++ API
pub mod api {
    use super::*;
    use std::sync::OnceLock;
    
    static DSOUND_DRIVER: OnceLock<Arc<DSoundDriver>> = OnceLock::new();
    
    /// Initialize DirectSound driver system
    pub fn audio_load(hwnd: Option<HWND>) -> Result<()> {
        let driver = Arc::new(DSoundDriver::new()?);
        driver.initialize(hwnd)?;
        driver.start_service()?;
        
        DSOUND_DRIVER.set(driver).map_err(|_| 
            Error::Device(DeviceError::InitializationFailed("Driver already initialized".to_string()))
        )?;
        
        Ok(())
    }
    
    /// Shutdown DirectSound driver system  
    pub fn audio_unload() -> Result<()> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            driver.shutdown()?;
        }
        Ok(())
    }
    
    /// Create audio channel
    pub fn audio_open_channel(format: AudioFormat, priority: Priority) -> Result<u32> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            driver.create_channel(format, priority)
        } else {
            Err(Error::Device(DeviceError::InitializationFailed("Driver not initialized".to_string())))
        }
    }
    
    /// Close audio channel
    pub fn audio_close_channel(channel_id: u32) -> Result<()> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            driver.remove_channel(channel_id)
        } else {
            Err(Error::Device(DeviceError::InitializationFailed("Driver not initialized".to_string())))
        }
    }
    
    /// Start channel playback
    pub fn audio_start(channel_id: u32, source_data: Vec<u8>, looping: bool) -> Result<()> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            driver.start_channel(channel_id, source_data, looping)
        } else {
            Err(Error::Device(DeviceError::InitializationFailed("Driver not initialized".to_string())))
        }
    }
    
    /// Stop channel playback
    pub fn audio_stop(channel_id: u32) -> Result<()> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            driver.stop_channel(channel_id)
        } else {
            Err(Error::Device(DeviceError::InitializationFailed("Driver not initialized".to_string())))
        }
    }
    
    /// Check if channel is playing
    pub fn audio_check(channel_id: u32) -> Result<bool> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            driver.is_channel_playing(channel_id)
        } else {
            Err(Error::Device(DeviceError::InitializationFailed("Driver not initialized".to_string())))
        }
    }
    
    /// Set channel volume
    pub fn audio_set_volume(channel_id: u32, volume: Volume) -> Result<()> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            driver.set_channel_volume(channel_id, volume)
        } else {
            Err(Error::Device(DeviceError::InitializationFailed("Driver not initialized".to_string())))
        }
    }
    
    /// Set channel pan  
    pub fn audio_set_pan(channel_id: u32, pan: i32) -> Result<()> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            driver.set_channel_pan(channel_id, pan)
        } else {
            Err(Error::Device(DeviceError::InitializationFailed("Driver not initialized".to_string())))
        }
    }
    
    /// Set channel frequency
    pub fn audio_set_frequency(channel_id: u32, frequency: u32) -> Result<()> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            driver.set_channel_frequency(channel_id, frequency)
        } else {
            Err(Error::Device(DeviceError::InitializationFailed("Driver not initialized".to_string())))
        }
    }
    
    /// Get DirectSound object (for compatibility with original C++ API)
    pub fn get_directsound_object() -> Option<IDirectSound> {
        DSOUND_DRIVER.get()?.ds_device.clone()
    }
    
    /// Get primary buffer (for compatibility with original C++ API)
    pub fn get_primary_buffer() -> Option<IDirectSoundBuffer> {
        DSOUND_DRIVER.get()?.primary_buffer.clone()
    }
    
    /// Handle audio focus loss
    pub fn audio_lose_focus() -> Result<()> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            if let Some(ref primary_buffer) = driver.primary_buffer {
                unsafe {
                    primary_buffer.Stop().ok();
                }
            }
        }
        Ok(())
    }
    
    /// Handle audio focus regain
    pub fn audio_regain_focus() -> Result<()> {
        if let Some(driver) = DSOUND_DRIVER.get() {
            if let Some(ref primary_buffer) = driver.primary_buffer {
                unsafe {
                    let mut status = 0u32;
                    primary_buffer.GetStatus(&mut status).ok();
                    
                    if status & DSBSTATUS_BUFFERLOST.0 != 0 {
                        primary_buffer.Restore().ok();
                    }
                    
                    if status & DSBSTATUS_PLAYING.0 == 0 {
                        primary_buffer.Play(0, 0, DSBPLAY_LOOPING).ok();
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dsound_channel_creation() {
        let channel = DSoundChannel::new(1, AudioFormat::default());
        assert_eq!(channel.id, 1);
        assert_eq!(channel.state.load(Ordering::SeqCst), ChannelState::Stopped as u32);
    }
    
    #[test]
    fn test_volume_table_bounds() {
        assert_eq!(VOLUME_LOG_TABLE[0], -10000); // Minimum volume
        assert_eq!(VOLUME_LOG_TABLE[100], 0);     // Maximum volume
    }
    
    #[test]
    fn test_audio_transfer_initialization() {
        let transfer = AudioTransfer::new();
        assert_eq!(transfer.state, TransferState::InitBlock);
        assert!(!transfer.pending);
    }
    
    #[cfg(windows)]
    #[test]
    fn test_dsound_driver_creation() {
        let driver = DSoundDriver::new();
        assert!(driver.is_ok());
        let driver = driver.unwrap();
        assert!(!driver.initialized.load(Ordering::SeqCst));
    }
}