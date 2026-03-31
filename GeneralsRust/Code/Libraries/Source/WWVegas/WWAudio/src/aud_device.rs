//! Audio Device Management Module
//! 
//! Provides comprehensive audio device abstraction and management functionality
//! for the WPAudio system. This is a Rust conversion of the original C++ code
//! from AUD_Device.cpp.
//!
//! # Features
//! 
//! - Cross-platform audio device enumeration and initialization
//! - Audio channel management with priority-based allocation
//! - Real-time audio attribute updates and mixing
//! - Hardware abstraction layer for different audio systems
//! - Thread-safe device and channel access
//! - Comprehensive error handling with Result types
//!
//! # Examples
//!
//! ```rust
//! use wp_audio::device::*;
//! use wp_audio::AudioFormat;
//! 
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize the audio system
//! AudioSystem::setup()?;
//! 
//! // Load an audio system (e.g., DirectSound)
//! let system_master = AudioSystemMaster::new("DirectSound")?;
//! let system = AudioSystem::load_system(system_master)?;
//! 
//! // Open the default audio device
//! let device = AudioDevice::open(AUDIO_DEVICE_DEFAULT, None)?;
//! 
//! // Create audio channels
//! let channel = device.create_channel()?;
//! 
//! // Service all devices (call regularly in main loop)
//! AudioSystem::service_all_devices();
//! 
//! // Cleanup
//! device.close()?;
//! AudioSystem::close_down();
//! # Ok(())
//! # }
//! ```

use crate::{
    AudioFormat, AudioAttribs, AudioChannel, AudioChannelType,
    error::{Result, AudioError},
    list::{ListNode, ListHead},
    lock::{Lock, LockGuard},
    time::{TimeStamp, AudioGetTime},
    memory::AudioMemFree,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock, atomic::{AtomicBool, AtomicU32, Ordering}},
    time::{Duration, Instant},
};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
use cpal::{Device, Host, StreamConfig, SupportedStreamConfig};

// Constants from the original C++ code
const AUDIO_DEFAULT_SERVICE_INTERVAL: Duration = Duration::from_millis(33); // ~30 FPS
const VOLUME_SLACK: u32 = 10; // AUDIO_LEVEL_MAX/10
const SECONDS: fn(u64) -> Duration = Duration::from_secs;

/// Default audio device identifier
pub const AUDIO_DEVICE_DEFAULT: i32 = 0;

/// Audio system initialization state
static AUDIO_INITIALIZED: AtomicBool = AtomicBool::new(false);
static AUDIO_EXCLUSIVE_SYSTEM_LOADED: AtomicBool = AtomicBool::new(false);

/// Global audio data structure
struct AudioData {
    system_list: Arc<Mutex<Vec<Arc<AudioSystem>>>>,
    dev_list: Arc<Mutex<Vec<Arc<AudioDevice>>>>,
    dev_list_access: Lock,
    std_channel_attribs: AudioAttribs,
}

static AUDIO_DATA: std::sync::OnceLock<AudioData> = std::sync::OnceLock::new();

/// Audio system flags
#[derive(Debug, Clone, Copy)]
pub struct AudioSystemFlags(u32);

impl AudioSystemFlags {
    pub const LOADED: Self = Self(0x01);
    pub const EXCLUSIVE: Self = Self(0x02);
    
    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
    
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
    
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

/// Audio device flags
#[derive(Debug, Clone, Copy)]
pub struct AudioDeviceFlags(u32);

impl AudioDeviceFlags {
    pub const DONT_SERVICE: Self = Self(0x01);
    
    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Audio service information for performance monitoring
#[derive(Debug, Clone)]
pub struct AudioServiceInfo {
    service_interval: Duration,
    must_service_interval: Duration,
    longest_reset: Duration,
    last_service_time: Instant,
    last_interval: Duration,
    longest_interval: Duration,
    count: u64,
    last_count: u64,
    period_interval: Duration,
    longest_interval_for_period: Duration,
    period_start: Instant,
    anim_pos: usize,
    miss_count: u64,
}

impl AudioServiceInfo {
    /// Initialize service info with default values
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            service_interval: Duration::ZERO,
            must_service_interval: Duration::ZERO,
            longest_reset: Duration::ZERO,
            last_service_time: now,
            last_interval: Duration::ZERO,
            longest_interval: Duration::ZERO,
            count: 0,
            last_count: 0,
            period_interval: Duration::from_secs(5),
            longest_interval_for_period: Duration::ZERO,
            period_start: now,
            anim_pos: 0,
            miss_count: 0,
        }
    }
    
    /// Set service interval
    pub fn set_interval(&mut self, interval: Duration) {
        self.service_interval = interval;
        self.must_service_interval = interval;
        self.longest_reset = interval * 10;
    }
    
    /// Set must-service interval
    pub fn set_must_service_interval(&mut self, interval: Duration) {
        self.must_service_interval = interval;
    }
    
    /// Set reset interval
    pub fn set_reset_interval(&mut self, interval: Duration) {
        self.longest_reset = interval;
    }
    
    /// Check if service is needed
    pub fn service_needed(&self, now: Instant) -> bool {
        let interval = now.duration_since(self.last_service_time);
        interval >= self.service_interval
    }
    
    /// Perform service and update statistics
    pub fn service_perform(&mut self, now: Instant) {
        let interval = now.duration_since(self.last_service_time);
        
        self.last_interval = interval;
        self.count += 1;
        
        // Reset longest interval if it's extremely long
        if self.longest_interval > self.longest_reset {
            self.longest_interval = Duration::ZERO;
        }
        
        if interval > self.longest_interval {
            self.longest_interval = interval;
        }
        
        if interval > self.must_service_interval {
            self.miss_count += 1;
        }
        
        if interval > self.longest_interval_for_period {
            self.longest_interval_for_period = interval;
        }
        
        self.last_service_time = now;
    }
    
    /// Generate debug string for service info
    pub fn dump(&mut self) -> String {
        static ANIM_CHARS: &[char] = &['|', '/', '-', '\\'];
        let now = Instant::now();
        
        if self.count != self.last_count {
            self.anim_pos = (self.anim_pos + 1) % ANIM_CHARS.len();
            self.last_count = self.count;
        }
        
        if self.period_start.elapsed() > self.period_interval {
            self.longest_interval_for_period = self.service_interval;
            self.period_start = now;
        }
        
        format!("{:05}ms ({:04},{:04},{:04},~{:02}) {} ",
            self.service_interval.as_millis(),
            self.last_interval.as_millis(),
            self.longest_interval_for_period.as_millis(),
            self.longest_interval.as_millis(),
            self.miss_count,
            ANIM_CHARS[self.anim_pos]
        )
    }
}

/// Audio system master - represents a driver/backend type
#[derive(Debug, Clone)]
pub struct AudioSystemMaster {
    pub name: String,
    pub flags: AudioSystemFlags,
    pub properties: u32,
    pub stamp: u32, // For validation
    
    // Driver function pointers would be here in C++
    // In Rust, we'll use trait objects or enums
    #[cfg(target_os = "windows")]
    pub host: Option<cpal::Host>,
}

impl AudioSystemMaster {
    pub const STAMP_SYSTEM_MASTER: u32 = 0xAUDI0SYS;
    pub const PROP_EXCLUSIVE: u32 = 0x01;
    
    /// Create a new audio system master
    pub fn new(name: impl Into<String>) -> Result<Self> {
        Ok(Self {
            name: name.into(),
            flags: AudioSystemFlags(0),
            properties: 0,
            stamp: Self::STAMP_SYSTEM_MASTER,
            #[cfg(target_os = "windows")]
            host: None,
        })
    }
    
    /// Load this system master
    pub fn load(&mut self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.host = Some(cpal::default_host());
        }
        
        self.flags.insert(AudioSystemFlags::LOADED);
        Ok(())
    }
    
    /// Unload this system master
    pub fn unload(&mut self) {
        self.flags.remove(AudioSystemFlags::LOADED);
        #[cfg(target_os = "windows")]
        {
            self.host = None;
        }
    }
}

/// Audio system - represents a loaded audio backend
#[derive(Debug)]
pub struct AudioSystem {
    pub master: Arc<Mutex<AudioSystemMaster>>,
    pub num_units: usize,
    pub units: Vec<Option<Arc<AudioDevice>>>,
    pub lock: Lock,
    
    // Platform-specific data
    #[cfg(target_os = "windows")]
    pub devices: Vec<Device>,
}

impl AudioSystem {
    /// Initialize the audio system
    pub fn setup() -> Result<()> {
        if AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return Err(AudioError::AlreadyInitialized);
        }

        AUDIO_DATA.get_or_init(|| AudioData {
            system_list: Arc::new(Mutex::new(Vec::new())),
            dev_list: Arc::new(Mutex::new(Vec::new())),
            dev_list_access: Lock::new(),
            std_channel_attribs: AudioAttribs::new(),
        });
        
        // Initialize audio timer
        crate::time::init_audio_timer();
        
        AUDIO_INITIALIZED.store(true, Ordering::SeqCst);
        
        println!("Initializing audio module");
        Ok(())
    }
    
    /// Close down the audio system
    pub fn close_down() {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return;
        }
        
        Self::destroy_all_devices();
        Self::unload_all_systems();
        
        println!("WPAudio system has been shut down");
        AUDIO_INITIALIZED.store(false, Ordering::SeqCst);
    }
    
    /// Service all audio devices (call regularly)
    pub fn service_all_devices() {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return;
        }
        
        let audio_data = AUDIO_DATA.get().unwrap();
        
        if audio_data.dev_list_access.try_lock().is_err() {
            return;
        }
        
        let devices = audio_data.dev_list.lock().unwrap();
        for device in devices.iter() {
            if !device.flags.contains(AudioDeviceFlags::DONT_SERVICE) {
                let _ = device.service();
            }
        }
    }
    
    /// Load an audio system
    pub fn load_system(mut master: AudioSystemMaster) -> Result<Arc<Self>> {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return Err(AudioError::NotInitialized);
        }
        
        if master.flags.contains(AudioSystemFlags::LOADED) {
            return Err(AudioError::AlreadyLoaded);
        }
        
        if AUDIO_EXCLUSIVE_SYSTEM_LOADED.load(Ordering::SeqCst) {
            return Err(AudioError::ExclusiveSystemLoaded);
        }
        
        println!("Loading driver system for {}", master.name);
        
        master.load()?;
        
        let system = Arc::new(Self::create_system(Arc::new(Mutex::new(master)))?);
        
        let audio_data = AUDIO_DATA.get().unwrap();
        audio_data.system_list.lock().unwrap().push(system.clone());
        
        {
            let master = system.master.lock().unwrap();
            if (master.properties & AudioSystemMaster::PROP_EXCLUSIVE) != 0 {
                AUDIO_EXCLUSIVE_SYSTEM_LOADED.store(true, Ordering::SeqCst);
            }
        }
        
        Ok(system)
    }
    
    /// Unload an audio system
    pub fn unload_system(system: Arc<Self>) -> Result<()> {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return Err(AudioError::NotInitialized);
        }
        
        if system.lock.is_locked() {
            return Err(AudioError::SystemInUse);
        }
        
        let audio_data = AUDIO_DATA.get().unwrap();
        let mut systems = audio_data.system_list.lock().unwrap();
        
        // Remove from list
        systems.retain(|s| !Arc::ptr_eq(s, &system));
        
        // Unload the master
        {
            let mut master = system.master.lock().unwrap();
            master.unload();
        }
        
        AUDIO_EXCLUSIVE_SYSTEM_LOADED.store(false, Ordering::SeqCst);
        
        Ok(())
    }
    
    /// Unload all audio systems
    pub fn unload_all_systems() {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return;
        }
        
        let audio_data = AUDIO_DATA.get().unwrap();
        let systems = audio_data.system_list.lock().unwrap().clone();
        
        for system in systems {
            let _ = Self::unload_system(system);
        }
    }
    
    /// Get first audio system
    pub fn first_system() -> Option<Arc<Self>> {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return None;
        }
        
        let audio_data = AUDIO_DATA.get().unwrap();
        let systems = audio_data.system_list.lock().unwrap();
        systems.first().cloned()
    }
    
    /// Get number of available devices across all systems
    pub fn number_of_devices() -> usize {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return 0;
        }
        
        let audio_data = AUDIO_DATA.get().unwrap();
        let systems = audio_data.system_list.lock().unwrap();
        
        systems.iter().map(|sys| sys.num_units).sum()
    }
    
    /// Destroy all audio devices
    pub fn destroy_all_devices() {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return;
        }
        
        let audio_data = AUDIO_DATA.get().unwrap();
        let _guard = audio_data.dev_list_access.lock();
        
        let devices = audio_data.dev_list.lock().unwrap().clone();
        for device in devices {
            device.destroy();
        }
    }
    
    /// Create a new audio system
    fn create_system(master: Arc<Mutex<AudioSystemMaster>>) -> Result<Self> {
        let num_units = Self::detect_num_units(&master)?;
        
        Ok(Self {
            master,
            num_units,
            units: vec![None; num_units],
            lock: Lock::new(),
            #[cfg(target_os = "windows")]
            devices: Vec::new(),
        })
    }
    
    /// Detect number of units for this system
    fn detect_num_units(_master: &Arc<Mutex<AudioSystemMaster>>) -> Result<usize> {
        #[cfg(target_os = "windows")]
        {
            // Use cpal to detect number of audio devices
            let host = cpal::default_host();
            let devices = host.output_devices()
                .map_err(|_| AudioError::DeviceEnumerationFailed)?
                .count();
            Ok(devices.max(1)) // At least one device
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // Default implementation for other platforms
            Ok(1)
        }
    }
    
    /// Map device unit number to system and system unit
    fn map_device_unit(unit: i32) -> Option<(Arc<Self>, usize)> {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return None;
        }
        
        let audio_data = AUDIO_DATA.get().unwrap();
        let systems = audio_data.system_list.lock().unwrap();
        
        let mut remaining_unit = unit as usize;
        
        for system in systems.iter() {
            if remaining_unit < system.num_units {
                return Some((system.clone(), remaining_unit));
            }
            remaining_unit -= system.num_units;
        }
        
        None
    }
}

/// Audio device - represents an opened audio output device
#[derive(Debug)]
pub struct AudioDevice {
    pub unit: i32,
    pub system_unit: usize,
    pub system: Arc<AudioSystem>,
    pub lock: Lock,
    pub flags: AudioDeviceFlags,
    pub max_channels: usize,
    pub channels: AtomicU32,
    pub default_format: AudioFormat,
    pub format: AudioFormat,
    pub attribs: AudioAttribs,
    pub group_attribs: Option<Arc<AudioAttribs>>,
    pub channel_list: Arc<Mutex<Vec<Arc<AudioChannel>>>>,
    pub attribs_list: Arc<Mutex<Vec<Arc<AudioAttribs>>>>,
    pub channel_access: Lock,
    pub attribs_update: Arc<Mutex<AudioServiceInfo>>,
    pub mixer_update: Arc<Mutex<AudioServiceInfo>>,
    
    // Performance metrics
    pub frames: AtomicU32,
    pub over_sample: AtomicU32,
    pub frame_lag: AtomicU32,
    
    // Platform-specific device handle
    #[cfg(target_os = "windows")]
    pub cpal_device: Option<Device>,
    #[cfg(target_os = "windows")]
    pub stream_config: Option<StreamConfig>,
}

impl AudioDevice {
    /// Open an audio device
    pub fn open(unit: i32, format: Option<&AudioFormat>) -> Result<Arc<Self>> {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return Err(AudioError::NotInitialized);
        }
        
        let unit = if unit == AUDIO_DEVICE_DEFAULT { 0 } else { unit };
        
        // Map unit number to system and system unit
        let (system, system_unit) = AudioSystem::map_device_unit(unit)
            .ok_or(AudioError::NoSuchDevice)?;
        
        // Check if device is already open
        if let Some(existing) = &system.units[system_unit] {
            let _guard = existing.lock.lock();
            return Ok(existing.clone());
        }
        
        // Create new device
        let device = Arc::new(Self::create_device(system.clone(), system_unit, format)?);
        device.system.units[system_unit] = Some(device.clone());
        
        // Add to global device list
        let audio_data = AUDIO_DATA.get().unwrap();
        audio_data.dev_list.lock().unwrap().push(device.clone());
        
        let _guard = device.lock.lock();
        Ok(device)
    }
    
    /// Create a new audio device
    fn create_device(system: Arc<AudioSystem>, system_unit: usize, format: Option<&AudioFormat>) -> Result<Self> {
        let _guard = system.lock.lock();
        
        let mut device = Self {
            unit: 0, // Will be set by caller
            system_unit,
            system,
            lock: Lock::new(),
            flags: AudioDeviceFlags(0),
            max_channels: 32, // Default value
            channels: AtomicU32::new(0),
            default_format: AudioFormat::default(),
            format: AudioFormat::default(),
            attribs: AudioAttribs::new(),
            group_attribs: None,
            channel_list: Arc::new(Mutex::new(Vec::new())),
            attribs_list: Arc::new(Mutex::new(Vec::new())),
            channel_access: Lock::new(),
            attribs_update: Arc::new(Mutex::new(AudioServiceInfo::new())),
            mixer_update: Arc::new(Mutex::new(AudioServiceInfo::new())),
            frames: AtomicU32::new(0),
            over_sample: AtomicU32::new(0),
            frame_lag: AtomicU32::new(0),
            #[cfg(target_os = "windows")]
            cpal_device: None,
            #[cfg(target_os = "windows")]
            stream_config: None,
        };
        
        // Initialize service intervals
        {
            let mut attribs_update = device.attribs_update.lock().unwrap();
            attribs_update.set_interval(AUDIO_DEFAULT_SERVICE_INTERVAL);
            attribs_update.set_must_service_interval(AUDIO_DEFAULT_SERVICE_INTERVAL * 5);
            attribs_update.set_reset_interval(AUDIO_DEFAULT_SERVICE_INTERVAL * 5);
        }
        
        {
            let mut mixer_update = device.mixer_update.lock().unwrap();
            mixer_update.set_interval(AUDIO_DEFAULT_SERVICE_INTERVAL);
        }
        
        // Set default format if provided
        if let Some(fmt) = format {
            device.set_default_format(fmt);
        }
        
        // Open the device with the platform-specific implementation
        device.platform_open()?;
        
        Ok(device)
    }
    
    /// Platform-specific device opening
    #[cfg(target_os = "windows")]
    fn platform_open(&mut self) -> Result<()> {
        let master = self.system.master.lock().unwrap();
        if let Some(host) = &master.host {
            self.cpal_device = host.default_output_device();
            if let Some(device) = &self.cpal_device {
                let supported_configs = device.supported_output_configs()
                    .map_err(|_| AudioError::DeviceOpenFailed)?;
                
                if let Some(config) = supported_configs.into_iter().next() {
                    self.stream_config = Some(config.with_max_sample_rate().config());
                }
            }
        }
        Ok(())
    }
    
    #[cfg(not(target_os = "windows"))]
    fn platform_open(&mut self) -> Result<()> {
        // Default implementation for other platforms
        Ok(())
    }
    
    /// Close the audio device
    pub fn close(&self) -> Result<()> {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return Err(AudioError::NotInitialized);
        }
        
        // Release the lock
        // If no other references exist, the device will be destroyed
        if !self.lock.is_locked() {
            self.destroy();
        }
        
        Ok(())
    }
    
    /// Destroy the audio device
    pub fn destroy(&self) {
        // Remove all attributes
        self.attribs_remove_all();
        
        // Destroy all channels
        let _ = self.destroy_all_channels(AudioChannelType::All);
        
        // Platform-specific cleanup
        self.platform_close();
        
        // Remove from system
        if let Some(device_ref) = &self.system.units[self.system_unit] {
            if Arc::ptr_eq(device_ref, &Arc::new(unsafe { &*(self as *const Self) })) {
                // This is unsafe, but matches the C++ behavior
                // In a real implementation, we'd use proper reference counting
            }
        }
        
        // Remove from global device list
        let audio_data = AUDIO_DATA.get().unwrap();
        let mut devices = audio_data.dev_list.lock().unwrap();
        // Similar issue here - need proper Arc handling
        devices.retain(|d| !std::ptr::eq(d.as_ref(), self));
    }
    
    /// Platform-specific device closing
    #[cfg(target_os = "windows")]
    fn platform_close(&self) {
        // Cleanup would go here
    }
    
    #[cfg(not(target_os = "windows"))]
    fn platform_close(&self) {
        // Default implementation
    }
    
    /// Set default format
    pub fn set_default_format(&mut self, format: &AudioFormat) {
        self.default_format = format.clone();
    }
    
    /// Set device format
    pub fn set_format(&mut self, _format: &AudioFormat) -> Result<()> {
        // Implementation would go here
        Err(AudioError::NotImplemented)
    }
    
    /// Set device volume
    pub fn set_volume(&self, volume: i32) {
        self.attribs.set_volume(volume);
    }
    
    /// Adjust device volume
    pub fn adjust_volume(&self, volume: i32) {
        self.attribs.adjust_volume(volume);
    }
    
    /// Get maximum number of channels
    pub fn max_channels(&self) -> usize {
        self.max_channels
    }
    
    /// Get current number of channels
    pub fn channels(&self) -> u32 {
        self.channels.load(Ordering::SeqCst)
    }
    
    /// Create a new audio channel
    pub fn create_channel(&self) -> Result<Arc<AudioChannel>> {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return Err(AudioError::NotInitialized);
        }
        
        let current_channels = self.channels.load(Ordering::SeqCst);
        if current_channels >= self.max_channels as u32 {
            println!("Maximum of {} channels already open", self.max_channels);
            return Err(AudioError::MaxChannelsReached);
        }
        
        let channel = AudioChannel::create(self)?;
        self.add_channel(channel.clone());
        
        Ok(channel)
    }
    
    /// Create multiple channels
    pub fn create_channels(&self, num: usize) -> usize {
        let mut count = 0;
        for _ in 0..num {
            if self.create_channel().is_ok() {
                count += 1;
            } else {
                break;
            }
        }
        count
    }
    
    /// Add a channel to this device
    pub fn add_channel(&self, channel: Arc<AudioChannel>) {
        self.channels.fetch_add(1, Ordering::SeqCst);
        
        let _guard = self.channel_access.lock();
        self.channel_list.lock().unwrap().push(channel);
    }
    
    /// Remove a channel from this device
    pub fn remove_channel(&self, channel: &AudioChannel) {
        let _guard = self.channel_access.lock();
        let mut channels = self.channel_list.lock().unwrap();
        
        channels.retain(|c| !std::ptr::eq(c.as_ref(), channel));
        self.channels.fetch_sub(1, Ordering::SeqCst);
    }
    
    /// Get a channel of specific type
    pub fn get_channel(&self, channel_type: AudioChannelType) -> Option<Arc<AudioChannel>> {
        let channels = self.channel_list.lock().unwrap();
        
        let mut lowest_priority_playing = i32::MAX;
        let mut lowest_priority_paused = i32::MAX;
        let mut lowest_playing: Option<Arc<AudioChannel>> = None;
        let mut lowest_paused: Option<Arc<AudioChannel>> = None;
        let mut lowest_playing_volume = u32::MAX;
        
        for channel in channels.iter() {
            if channel.get_type() == channel_type || channel_type == AudioChannelType::Any {
                if !channel.is_active() && !channel.is_in_use() {
                    return Some(channel.clone()); // Found a free channel
                }
                
                if channel.is_playing() {
                    let priority = channel.get_priority();
                    let volume = channel.get_volume();
                    
                    if priority < lowest_priority_playing {
                        lowest_priority_playing = priority;
                        lowest_playing = Some(channel.clone());
                        lowest_playing_volume = volume;
                    } else if priority == lowest_priority_playing {
                        if volume < lowest_playing_volume && 
                           lowest_playing_volume.saturating_sub(volume) >= VOLUME_SLACK {
                            lowest_playing = Some(channel.clone());
                            lowest_playing_volume = volume;
                        }
                    }
                } else {
                    let priority = channel.get_priority();
                    if priority < lowest_priority_paused {
                        lowest_priority_paused = priority;
                        lowest_paused = Some(channel.clone());
                    }
                }
            }
        }
        
        // Return the best available channel
        if let Some(paused) = lowest_paused {
            if lowest_priority_paused <= lowest_priority_playing {
                return Some(paused);
            }
        }
        
        lowest_playing
    }
    
    /// Reserve a channel with new type
    pub fn reserve_channel(&self, new_type: AudioChannelType) -> Option<Arc<AudioChannel>> {
        if let Some(channel) = self.get_channel(AudioChannelType::Standard) {
            channel.reserve(new_type);
            Some(channel)
        } else {
            None
        }
    }
    
    /// Destroy all channels of specific type
    pub fn destroy_all_channels(&self, channel_type: AudioChannelType) -> Result<()> {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return Err(AudioError::NotInitialized);
        }
        
        let channels = self.channel_list.lock().unwrap().clone();
        for channel in channels {
            if channel_type == AudioChannelType::All || channel.get_type() == channel_type {
                channel.destroy();
            }
        }
        
        Ok(())
    }
    
    /// Stop all channels of specific type
    pub fn stop_all_channels(&self, channel_type: AudioChannelType) {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return;
        }
        
        let channels = self.channel_list.lock().unwrap();
        for channel in channels.iter() {
            if channel_type == AudioChannelType::All || channel.get_type() == channel_type {
                channel.stop();
            }
        }
    }
    
    /// Pause all channels of specific type
    pub fn pause_all_channels(&self, channel_type: AudioChannelType) {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return;
        }
        
        let channels = self.channel_list.lock().unwrap();
        for channel in channels.iter() {
            if (channel_type == AudioChannelType::All || channel.get_type() == channel_type) 
               && channel.is_playing() {
                channel.pause();
            }
        }
    }
    
    /// Resume all channels of specific type  
    pub fn resume_all_channels(&self, channel_type: AudioChannelType) {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return;
        }
        
        let channels = self.channel_list.lock().unwrap();
        for channel in channels.iter() {
            if (channel_type == AudioChannelType::All || channel.get_type() == channel_type)
               && channel.is_paused() {
                channel.resume();
            }
        }
    }
    
    /// Service this device (call regularly)
    pub fn service(&self) -> Result<()> {
        if self.channel_access.try_lock().is_err() {
            return Err(AudioError::InUse);
        }
        
        let now = Instant::now();
        
        // Check if attributes update is needed
        let needs_service = {
            let attribs_update = self.attribs_update.lock().unwrap();
            attribs_update.service_needed(now)
        };
        
        if !needs_service {
            return Ok(());
        }
        
        // Perform service
        {
            let mut attribs_update = self.attribs_update.lock().unwrap();
            attribs_update.service_perform(now);
        }
        
        // Update device and channel attributes
        self.update_attribs_list();
        self.attribs.update();
        
        let device_changed = self.group_attribs.as_ref()
            .map(|ga| ga.has_changed())
            .unwrap_or(false) || self.attribs.has_changed();
        
        // Update all playing channels
        let channels = self.channel_list.lock().unwrap();
        for channel in channels.iter() {
            if channel.is_playing() && channel.has_sample() {
                channel.update_attribs();
                
                if device_changed || channel.has_attribs_changed() {
                    channel.recalc_attribs();
                    channel.driver_update();
                }
            }
        }
        
        // Mark attributes as used
        self.attribs.mark_used();
        self.mark_attribs_list_used();
        
        Ok(())
    }
    
    /// Add attributes to device
    pub fn attribs_add(&self, attribs: Arc<AudioAttribs>) {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return;
        }
        
        // Check if already exists
        {
            let attribs_list = self.attribs_list.lock().unwrap();
            for existing in attribs_list.iter() {
                if Arc::ptr_eq(existing, &attribs) {
                    return;
                }
            }
        }
        
        let _guard = self.channel_access.lock();
        self.attribs_list.lock().unwrap().push(attribs);
    }
    
    /// Remove attributes from device
    pub fn attribs_remove(&self, attribs: &AudioAttribs) {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return;
        }
        
        let _guard = self.channel_access.lock();
        let mut attribs_list = self.attribs_list.lock().unwrap();
        
        attribs_list.retain(|a| !std::ptr::eq(a.as_ref(), attribs));
    }
    
    /// Remove all attributes from device
    pub fn attribs_remove_all(&self) {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return;
        }
        
        let _guard = self.channel_access.lock();
        self.attribs_list.lock().unwrap().clear();
    }
    
    /// Update all attributes in the list
    fn update_attribs_list(&self) {
        let attribs_list = self.attribs_list.lock().unwrap();
        for attribs in attribs_list.iter() {
            attribs.update();
        }
    }
    
    /// Mark all attributes in list as used
    fn mark_attribs_list_used(&self) {
        let attribs_list = self.attribs_list.lock().unwrap();
        for attribs in attribs_list.iter() {
            attribs.mark_used();
        }
    }
    
    /// Dump device information for debugging
    pub fn dump<F>(&self, print: F, show_names: bool) 
    where
        F: Fn(&str),
    {
        print("Audio Device Dump ---------------------");
        
        let master = self.system.master.lock().unwrap();
        print(&format!("{} ({})", master.name, self.unit));
        
        let mixer_info = self.mixer_update.lock().unwrap().dump();
        print(&format!("mixer: {}", mixer_info));
        
        let attribs_info = self.attribs_update.lock().unwrap().dump();
        print(&format!("level: {}", attribs_info));
        
        print(&format!("Frames: {} Oversamp: {} Lag: {}", 
            self.frames.load(Ordering::SeqCst),
            self.over_sample.load(Ordering::SeqCst),
            self.frame_lag.load(Ordering::SeqCst)
        ));
        
        if show_names {
            print("Chan:TYPE: STATE : FORMAT  :PRI :VOL: NAME");
        } else {
            print("Chan:TYPE: STATE : FORMAT  :PRI :VOL:");
        }
        
        let channels = self.channel_list.lock().unwrap();
        for (i, channel) in channels.iter().enumerate() {
            let state = if channel.is_playing() {
                "PLAYING"
            } else if channel.is_paused() {
                "PAUSED "
            } else {
                "FREE   "
            };
            
            let type_str = match channel.get_type() {
                AudioChannelType::Standard => "STD ",
                AudioChannelType::User(_) => "USER",
                _ => "??? ",
            };
            
            let format_str = channel.format_string();
            let priority = channel.get_priority();
            let volume = channel.get_volume();
            
            if show_names {
                let name = channel.get_sample_name();
                print(&format!("#{:2} :{}:{}:{}:{:4}:{:3}:{:.31}",
                    i, type_str, state, format_str, priority, volume, name));
            } else {
                print(&format!("#{:2} :{}:{}:{}:{:4}:{:3}:",
                    i, type_str, state, format_str, priority, volume));
            }
        }
        
        print("--------------------------------------");
    }
}

/// Iterator for audio systems
pub struct AudioSystemIter {
    systems: Vec<Arc<AudioSystem>>,
    index: usize,
}

impl Iterator for AudioSystemIter {
    type Item = Arc<AudioSystem>;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.systems.len() {
            let system = self.systems[self.index].clone();
            self.index += 1;
            Some(system)
        } else {
            None
        }
    }
}

impl AudioSystemIter {
    /// Create new system iterator
    pub fn new() -> Self {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return Self { systems: Vec::new(), index: 0 };
        }
        
        let audio_data = AUDIO_DATA.get().unwrap();
        let systems = audio_data.system_list.lock().unwrap().clone();
        
        Self { systems, index: 0 }
    }
}

/// Iterator for audio devices
pub struct AudioDeviceIter {
    devices: Vec<Arc<AudioDevice>>,
    index: usize,
}

impl Iterator for AudioDeviceIter {
    type Item = Arc<AudioDevice>;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.devices.len() {
            let device = self.devices[self.index].clone();
            self.index += 1;
            Some(device)
        } else {
            None
        }
    }
}

impl AudioDeviceIter {
    /// Create new device iterator
    pub fn new() -> Self {
        if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
            return Self { devices: Vec::new(), index: 0 };
        }
        
        let audio_data = AUDIO_DATA.get().unwrap();
        let devices = audio_data.dev_list.lock().unwrap().clone();
        
        Self { devices, index: 0 }
    }
}

/// Global function to get standard channel attributes
pub fn audio_std_channel_attribs() -> Option<&'static AudioAttribs> {
    if !AUDIO_INITIALIZED.load(Ordering::SeqCst) {
        return None;
    }
    
    AUDIO_DATA.get().map(|data| &data.std_channel_attribs)
}

// Export key functionality for C-style interface compatibility
pub use AudioSystem as AudioSetUp;
pub use AudioSystem::close_down as AudioCloseDown;
pub use AudioSystem::service_all_devices as AudioServiceAllDevices;
pub use AudioSystem::load_system as AudioLoadSystem;
pub use AudioSystem::unload_system as AudioUnloadSystem;
pub use AudioSystem::unload_all_systems as AudioUnloadAllSystems;
pub use AudioSystem::number_of_devices as AudioNumberOfDevices;
pub use AudioSystem::destroy_all_devices as AudioDestroyAllDevices;
pub use AudioDevice::open as AudioOpenDevice;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_audio_system_setup() {
        AudioSystem::setup().unwrap();
        assert!(AUDIO_INITIALIZED.load(Ordering::SeqCst));
        AudioSystem::close_down();
        assert!(!AUDIO_INITIALIZED.load(Ordering::SeqCst));
    }
    
    #[test] 
    fn test_service_info() {
        let mut service_info = AudioServiceInfo::new();
        let now = Instant::now();
        
        assert!(!service_info.service_needed(now));
        
        service_info.set_interval(Duration::from_millis(10));
        std::thread::sleep(Duration::from_millis(20));
        
        assert!(service_info.service_needed(Instant::now()));
    }
}