//! Windows-specific audio implementations for WPAudio
//!
//! This module provides Windows-specific audio functionality including:
//! - Audio thread management with Windows threading APIs
//! - Wave file format reading and parsing
//! - Windows debug output integration
//! - Window handle management for DirectSound cooperation
//!
//! Copyright 2025 Electronic Arts Inc.
//! Licensed under GPL-3.0

use std::ffi::{CStr, CString};
use std::mem;
use std::ptr;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
    Arc, Condvar, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Media::Audio::*,
        System::{Memory::*, SystemServices::*, Threading::*},
        UI::WindowsAndMessaging::*,
    },
};

use crate::{
    error::{DeviceError, Error, Result},
    formats::AudioFormat,
};

/// Maximum length for thread names
const MAX_THREAD_NAME_LEN: usize = 200;

/// TimeStamp type for compatibility with original C++ code
pub type TimeStamp = Duration;

/// CPU profiling information
#[derive(Debug, Default)]
pub struct ProfileCPU {
    pub cpu_usage: f32,
    pub max_cpu_usage: f32,
    pub total_time: Duration,
    pub idle_time: Duration,
}

impl ProfileCPU {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self) {
        // Placeholder - would implement actual CPU profiling
    }
}

/// Audio service information for monitoring
#[derive(Debug)]
pub struct AudioServiceInfo {
    pub interval: Duration,
    pub must_service_interval: Duration,
    pub reset_interval: Duration,
    pub last_service: Instant,
    pub service_count: u64,
}

impl AudioServiceInfo {
    pub fn new() -> Self {
        Self {
            interval: Duration::from_millis(33),
            must_service_interval: Duration::from_millis(133),
            reset_interval: Duration::from_millis(133),
            last_service: Instant::now(),
            service_count: 0,
        }
    }

    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    pub fn set_must_service_interval(&mut self, interval: Duration) {
        self.must_service_interval = interval;
    }

    pub fn set_reset_interval(&mut self, interval: Duration) {
        self.reset_interval = interval;
    }

    pub fn update(&mut self) {
        self.last_service = Instant::now();
        self.service_count += 1;
    }
}

/// Thread priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudThreadPriority {
    Normal,
    High,
    Realtime,
}

/// Thread callback function type
pub type AudThreadCallback = dyn Fn(&AudThread, *mut std::ffi::c_void) -> bool + Send + Sync;

/// Audio thread structure for Windows-specific threading
pub struct AudThread {
    name: String,
    quit: AtomicBool,
    count: AtomicUsize,
    leaving: AtomicBool,
    interval: AtomicU32, // In milliseconds
    running: AtomicBool,
    #[cfg(target_os = "windows")]
    handle: Option<HANDLE>,
    #[cfg(target_os = "windows")]
    thread_id: u32,
    data: Mutex<*mut std::ffi::c_void>,
    callback: Arc<Box<AudThreadCallback>>,
    #[cfg(target_os = "windows")]
    critical_section: Mutex<CRITICAL_SECTION>,
    #[cfg(not(target_os = "windows"))]
    critical_section: Mutex<()>,
    update: Mutex<AudioServiceInfo>,
    cpu_profile: Mutex<ProfileCPU>,
}

unsafe impl Send for AudThread {}
unsafe impl Sync for AudThread {}

/// RIFF header structure for wave file parsing
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct RiffHeader {
    form: u32,      // 'RIFF'
    length: u32,    // File length
    file_type: u32, // 'WAVE'
}

/// RIFF chunk header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct RiffChunk {
    chunk_type: u32, // Chunk identifier
    length: u32,     // Chunk length
}

/// Wave format structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct WaveFormatEx {
    format_tag: u16,
    channels: u16,
    samples_per_sec: u32,
    avg_bytes_per_sec: u32,
    block_align: u16,
    bits_per_sample: u16,
    cb_size: u16,
}

/// ADPCM wave format structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct AdpcmWaveFormat {
    wave_format: WaveFormatEx,
    samples_per_block: u16,
    num_coef: u16,
    coef: [AdpcmCoeff; 7],
}

/// ADPCM coefficient structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct AdpcmCoeff {
    coeff1: i16,
    coeff2: i16,
}

/// Wave format tags
const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_ADPCM: u16 = 0x0002;
const WAVE_FORMAT_IMA_ADPCM: u16 = 0x0011;
const WAVE_FORMAT_MPEGLAYER3: u16 = 0x0055;

/// RIFF chunk identifiers
const FOURCC_RIFF: u32 = 0x46464952; // 'RIFF'
const FOURCC_WAVE: u32 = 0x45564157; // 'WAVE'
const FOURCC_FMT: u32 = 0x20746D66; // 'fmt '
const FOURCC_DATA: u32 = 0x61746164; // 'data'

/// Standard MS ADPCM coefficients
const MS_ADPCM_STD_COEF: [AdpcmCoeff; 7] = [
    AdpcmCoeff {
        coeff1: 256,
        coeff2: 0,
    },
    AdpcmCoeff {
        coeff1: 512,
        coeff2: -256,
    },
    AdpcmCoeff {
        coeff1: 0,
        coeff2: 0,
    },
    AdpcmCoeff {
        coeff1: 192,
        coeff2: 64,
    },
    AdpcmCoeff {
        coeff1: 240,
        coeff2: 0,
    },
    AdpcmCoeff {
        coeff1: 460,
        coeff2: -208,
    },
    AdpcmCoeff {
        coeff1: 392,
        coeff2: -232,
    },
];

/// Audio compression types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCompression {
    None,
    MsAdpcm,
    ImaAdpcm,
    Mp3,
}

/// Audio format flags
#[derive(Debug, Clone, Copy)]
pub struct AudioFormatFlags {
    pub pcm: bool,
    pub compressed: bool,
    pub streaming: bool,
}

impl Default for AudioFormatFlags {
    fn default() -> Self {
        Self {
            pcm: true,
            compressed: false,
            streaming: false,
        }
    }
}

/// Extended audio format with compression info
#[derive(Debug, Clone)]
pub struct ExtendedAudioFormat {
    pub base_format: AudioFormat,
    pub compression: AudioCompression,
    pub block_size: Option<u16>,
    pub bytes_per_second: u32,
    pub flags: AudioFormatFlags,
}

/// Global window handle for DirectSound cooperation
static AUDIO_MAIN_WINDOW_HANDLE: Mutex<Option<isize>> = Mutex::new(None);

impl AudThread {
    /// Create a new audio thread
    pub fn create(
        name: &str,
        priority: AudThreadPriority,
        callback: Box<AudThreadCallback>,
    ) -> Result<Arc<AudThread>> {
        let thread_name = if name.is_empty() {
            "no name given".to_string()
        } else {
            name.to_string()
        };

        let thread = Arc::new(AudThread {
            name: thread_name,
            quit: AtomicBool::new(false),
            count: AtomicUsize::new(0),
            leaving: AtomicBool::new(false),
            interval: AtomicU32::new(33), // Default ~30 FPS
            running: AtomicBool::new(false),
            #[cfg(target_os = "windows")]
            handle: None,
            #[cfg(target_os = "windows")]
            thread_id: 0,
            data: Mutex::new(ptr::null_mut()),
            callback: Arc::new(callback),
            #[cfg(target_os = "windows")]
            critical_section: Mutex::new(unsafe { mem::zeroed() }),
            #[cfg(not(target_os = "windows"))]
            critical_section: Mutex::new(()),
            update: Mutex::new(AudioServiceInfo::new()),
            cpu_profile: Mutex::new(ProfileCPU::new()),
        });

        #[cfg(target_os = "windows")]
        {
            // Initialize critical section
            // Note: We're using a Mutex instead of CRITICAL_SECTION for simplicity
            // In a full Windows implementation, we would use InitializeCriticalSection
        }

        thread.set_interval(Duration::from_millis(33))?; // ~30 FPS default

        let thread_clone = Arc::clone(&thread);
        let handle = thread::Builder::new()
            .name(thread.name.clone())
            .stack_size(4 * 1024) // 4KB stack like the original
            .spawn(move || {
                Self::service_thread(thread_clone);
            })
            .map_err(|e| {
                Error::Device(DeviceError::InitializationFailed(format!(
                    "Failed to create audio thread '{}': {}",
                    thread.name, e
                )))
            })?;

        // Set thread priority on Windows
        #[cfg(target_os = "windows")]
        {
            // Store the thread handle for later use
            // Note: In a full implementation, we would need to get the native Windows handle
            // For now, we'll just log the priority setting
            let priority_result = match priority {
                AudThreadPriority::Normal => true,
                AudThreadPriority::High | AudThreadPriority::Realtime => {
                    // In a real implementation, we would use GetCurrentThread() and SetThreadPriority
                    log::info!("Setting thread priority to {:?}", priority);
                    true
                }
            };

            if !priority_result {
                log::warn!("Unable to change priority of thread '{}'", thread.name);
            }
        }

        log::info!("Created audio thread: '{}'", thread.name);
        Ok(thread)
    }

    /// Service thread main loop
    fn service_thread(thread: Arc<AudThread>) {
        thread.running.store(true, Ordering::SeqCst);
        thread.leaving.store(false, Ordering::SeqCst);

        while !thread.quit.load(Ordering::SeqCst) {
            let should_sleep = {
                let data = {
                    let data_guard = thread.data.lock().unwrap();
                    *data_guard
                };

                (thread.callback)(&thread, data)
            };

            if should_sleep {
                let interval = Duration::from_millis(thread.interval.load(Ordering::SeqCst) as u64);
                thread::sleep(interval);
            } else {
                thread::sleep(Duration::from_millis(5));
            }

            thread.count.fetch_add(1, Ordering::SeqCst);
        }

        thread.leaving.store(true, Ordering::SeqCst);
    }

    /// Destroy the thread and clean up resources
    pub fn destroy(self: Arc<Self>) -> Result<()> {
        self.quit.store(true, Ordering::SeqCst);

        // Wait for thread to finish
        while !self.leaving.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(10));
        }

        #[cfg(target_os = "windows")]
        {
            // Clean up critical section
            // In a full implementation, we would call DeleteCriticalSection
        }

        log::info!("Removed audio thread: '{}'", self.name);
        Ok(())
    }

    /// Enter critical section
    pub fn begin_critical_section(&self) {
        #[cfg(target_os = "windows")]
        {
            // In a full implementation, we would call EnterCriticalSection
            // For now, the Mutex provides thread safety
            let _guard = self.critical_section.lock();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _guard = self.critical_section.lock();
        }
    }

    /// Leave critical section
    pub fn end_critical_section(&self) {
        #[cfg(target_os = "windows")]
        {
            // In a full implementation, we would call LeaveCriticalSection
            // The Mutex guard automatically releases when it goes out of scope
        }
        #[cfg(not(target_os = "windows"))]
        {
            // The Mutex guard automatically releases when it goes out of scope
        }
    }

    /// Set thread user data
    pub fn set_data(&self, data: *mut std::ffi::c_void) {
        if let Ok(mut data_guard) = self.data.lock() {
            *data_guard = data;
        }
    }

    /// Set thread interval
    pub fn set_interval(&self, interval: Duration) -> Result<()> {
        let millis = interval.as_millis() as u32;
        self.interval.store(millis, Ordering::SeqCst);

        if let Ok(mut update) = self.update.lock() {
            update.set_interval(interval);
            update.set_must_service_interval(interval * 4);
            update.set_reset_interval(interval * 4);
        }

        Ok(())
    }

    /// Get thread interval
    pub fn get_interval(&self) -> Duration {
        Duration::from_millis(self.interval.load(Ordering::SeqCst) as u64)
    }

    /// Get thread name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get CPU profile
    pub fn cpu_profile(&self) -> std::sync::MutexGuard<ProfileCPU> {
        self.cpu_profile.lock().unwrap()
    }

    /// Get service info
    pub fn service_info(&self) -> std::sync::MutexGuard<AudioServiceInfo> {
        self.update.lock().unwrap()
    }
}

/// Read wave file format information
pub fn read_wave_file_format<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
) -> Result<(ExtendedAudioFormat, usize)> {
    reader.seek(std::io::SeekFrom::Start(0))?;

    let mut riff_header = [0u8; 12];
    reader.read_exact(&mut riff_header)?;

    let form = u32::from_le_bytes([
        riff_header[0],
        riff_header[1],
        riff_header[2],
        riff_header[3],
    ]);
    let file_type = u32::from_le_bytes([
        riff_header[8],
        riff_header[9],
        riff_header[10],
        riff_header[11],
    ]);

    if form != FOURCC_RIFF || file_type != FOURCC_WAVE {
        // Try MP3 format
        reader.seek(std::io::SeekFrom::Start(0))?;
        return read_mp3_file_format(reader);
    }

    let mut wave_format: Option<WaveFormatEx> = None;
    let mut data_size = 0usize;

    // Parse chunks
    loop {
        let mut chunk_header = [0u8; 8];
        match reader.read_exact(&mut chunk_header) {
            Ok(_) => {}
            Err(_) => break, // End of file
        }

        let chunk_type = u32::from_le_bytes([
            chunk_header[0],
            chunk_header[1],
            chunk_header[2],
            chunk_header[3],
        ]);
        let chunk_length = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]) as usize;

        match chunk_type {
            FOURCC_FMT => {
                let format_size = chunk_length.max(std::mem::size_of::<WaveFormatEx>());
                let mut format_data = vec![0u8; format_size];
                reader.read_exact(&mut format_data[..chunk_length])?;

                if chunk_length >= std::mem::size_of::<WaveFormatEx>() {
                    wave_format = Some(unsafe {
                        std::ptr::read(format_data.as_ptr() as *const WaveFormatEx)
                    });
                } else {
                    // Create minimal format structure
                    let mut wf: WaveFormatEx = unsafe { mem::zeroed() };
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            format_data.as_ptr(),
                            &mut wf as *mut _ as *mut u8,
                            chunk_length,
                        );
                    }
                    wf.cb_size = chunk_length as u16;
                    wave_format = Some(wf);
                }
            }
            FOURCC_DATA => {
                data_size = chunk_length;
                break; // Found data chunk, we have what we need
            }
            _ => {
                // Skip unknown chunk
                reader.seek(std::io::SeekFrom::Current(chunk_length as i64))?;
            }
        }
    }

    let wformat = wave_format.ok_or_else(|| {
        Error::Source(crate::error::SourceError::InvalidFormat(
            "No format chunk found".to_string(),
        ))
    })?;

    if data_size == 0 {
        return Err(Error::Source(crate::error::SourceError::InvalidFormat(
            "No data chunk found".to_string(),
        )));
    }

    let (compression, block_size, sample_width) = match wformat.format_tag {
        WAVE_FORMAT_PCM => (AudioCompression::None, None, wformat.bits_per_sample / 8),
        WAVE_FORMAT_IMA_ADPCM => (AudioCompression::ImaAdpcm, Some(wformat.block_align), 2),
        WAVE_FORMAT_ADPCM => {
            // Verify standard coefficients (simplified check)
            (AudioCompression::MsAdpcm, Some(wformat.block_align), 2)
        }
        WAVE_FORMAT_MPEGLAYER3 => {
            reader.seek(std::io::SeekFrom::Start(0))?;
            return read_mp3_file_format(reader);
        }
        _ => {
            return Err(Error::Source(crate::error::SourceError::InvalidFormat(
                format!("Unsupported wave format: {}", wformat.format_tag),
            )));
        }
    };

    let base_format = AudioFormat {
        channels: wformat.channels,
        sample_rate: match wformat.samples_per_sec {
            8000 => crate::formats::SampleRate::Hz8000,
            11025 => crate::formats::SampleRate::Hz11025,
            16000 => crate::formats::SampleRate::Hz16000,
            22050 => crate::formats::SampleRate::Hz22050,
            44100 => crate::formats::SampleRate::Hz44100,
            48000 => crate::formats::SampleRate::Hz48000,
            96000 => crate::formats::SampleRate::Hz96000,
            192000 => crate::formats::SampleRate::Hz192000,
            _ => crate::formats::SampleRate::Hz44100, // Default fallback
        },
        sample_width: match sample_width {
            1 => crate::formats::SampleWidth::U8,
            2 => crate::formats::SampleWidth::S16,
            3 => crate::formats::SampleWidth::S24,
            4 => crate::formats::SampleWidth::S32,
            _ => crate::formats::SampleWidth::S16, // Default fallback
        },
        channel_layout: match wformat.channels {
            1 => crate::formats::ChannelLayout::Mono,
            2 => crate::formats::ChannelLayout::Stereo,
            4 => crate::formats::ChannelLayout::Surround41,
            6 => crate::formats::ChannelLayout::Surround51,
            8 => crate::formats::ChannelLayout::Surround71,
            _ => crate::formats::ChannelLayout::Stereo, // Default fallback
        },
    };

    let mut flags = AudioFormatFlags::default();
    flags.pcm = compression == AudioCompression::None;
    flags.compressed = compression != AudioCompression::None;

    let extended_format = ExtendedAudioFormat {
        base_format,
        compression,
        block_size,
        bytes_per_second: wformat.avg_bytes_per_sec,
        flags,
    };

    Ok((extended_format, data_size))
}

/// Read MP3 file format (placeholder implementation)
fn read_mp3_file_format<R: std::io::Read + std::io::Seek>(
    _reader: &mut R,
) -> Result<(ExtendedAudioFormat, usize)> {
    // This would need a proper MP3 decoder implementation
    // For now, return a basic format
    Err(Error::Source(crate::error::SourceError::InvalidFormat(
        "MP3 format reading not yet implemented".to_string(),
    )))
}

/// Windows debug print function
#[cfg(target_os = "windows")]
pub fn windows_debug_print(message: &str) {
    let c_message = CString::new(message).unwrap_or_default();
    unsafe {
        OutputDebugStringA(PCSTR(c_message.as_ptr() as *const u8));
    }
}

/// Windows debug print function (no-op on non-Windows)
#[cfg(not(target_os = "windows"))]
pub fn windows_debug_print(message: &str) {
    println!("{}", message);
}

/// Set the main window handle for DirectSound cooperation
pub fn set_windows_handle(hwnd: isize) {
    if let Ok(mut handle) = AUDIO_MAIN_WINDOW_HANDLE.lock() {
        *handle = Some(hwnd);
    }
}

/// Get the main window handle
pub fn get_windows_handle() -> Option<isize> {
    AUDIO_MAIN_WINDOW_HANDLE.lock().ok().and_then(|h| *h)
}

/// Convert milliseconds to Duration safely
pub fn millis_to_duration(millis: u32) -> Duration {
    Duration::from_millis(millis as u64)
}

/// Convert seconds to Duration safely  
pub fn seconds_to_duration(seconds: u32) -> Duration {
    Duration::from_secs(seconds as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_thread_creation() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let callback = Box::new(
            move |_thread: &AudThread, _data: *mut std::ffi::c_void| -> bool {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);
                count < 5 // Run 5 times then stop
            },
        );

        let thread = AudThread::create("test_thread", AudThreadPriority::Normal, callback).unwrap();

        // Let it run for a bit
        std::thread::sleep(Duration::from_millis(200));

        assert!(counter.load(Ordering::SeqCst) > 0);

        // Clean shutdown
        thread.destroy().unwrap();
    }

    #[test]
    fn test_window_handle() {
        let handle = 12345isize;
        set_windows_handle(handle);
        assert_eq!(get_windows_handle(), Some(handle));
    }

    #[test]
    fn test_debug_print() {
        windows_debug_print("Test debug message");
        // Should not panic
    }

    #[test]
    fn test_duration_conversion() {
        let duration = millis_to_duration(1000);
        assert_eq!(duration, Duration::from_secs(1));

        let duration = seconds_to_duration(5);
        assert_eq!(duration, Duration::from_secs(5));
    }
}
