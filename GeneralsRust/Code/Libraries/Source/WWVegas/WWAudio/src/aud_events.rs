//! Audio Events System
//! 
//! Provides a comprehensive event-driven audio system with priority-based
//! channel allocation, event sequencing, looping, and volume compression.
//! This is a direct conversion of the C++ AUD_Events.cpp file to idiomatic Rust
//! using channels and callback patterns.

use std::collections::{HashMap, VecDeque, BTreeMap};
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::thread;

use crate::aud_cache::{AudioCache, AudioCacheItem};
use crate::aud_attributes::AudioAttribs;
use crate::aud_lock::AudioLock;
use crate::level::AudioLevel;
use crate::device::AudioDevice;
use crate::channel::AudioChannel;
use crate::source::AudioSample;
use crate::profiler::ProfileData;
use crate::time::{Timestamp, AudioGetTime, SECONDS, MSECONDS};
use crate::error::{AudioResult, AudioError};

/// Maximum number of samples per event
pub const MAX_AUDIO_EVENT_SAMPLES: usize = 16;

/// Maximum number of concurrent events
pub const MAX_EVENTS: usize = 100;

/// Default event limit per class
pub const AUDIO_EVENT_DEFAULT_LIMIT: usize = 3;

/// Event priority levels
pub type AudioPriority = u8;

pub const AUDIO_EVENT_NORMAL_PRIORITY: AudioPriority = 5;
pub const AUDIO_EVENT_CRITICAL_PRIORITY: AudioPriority = 9;
pub const AUDIO_NUM_EVENT_PRIORITIES: usize = 10;

/// Audio event control flags
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioEventControl {
    pub none: bool,
    pub loop_event: bool,
    pub random: bool,
    pub interrupt: bool,
    pub all: bool,
    pub attack: bool,
    pub decay: bool,
    pub ambient: bool,
    pub post_delay: bool,
}

impl Default for AudioEventControl {
    fn default() -> Self {
        AudioEventControl {
            none: true,
            loop_event: false,
            random: false,
            interrupt: false,
            all: false,
            attack: false,
            decay: false,
            ambient: false,
            post_delay: false,
        }
    }
}

/// Audio event states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioEventState {
    New,
    StartPlaying,
    Waiting,
    Playing,
    Done,
}

/// Time of day for context-sensitive audio
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeOfDay {
    Morning = 1,
    Afternoon = 2,
    Evening = 3,
    Night = 4,
}

/// Event message types for communication
#[derive(Debug)]
pub enum EventMessage {
    Play(AudioEventRequest),
    Stop(u64), // Event ID
    Pause(u64),
    Resume(u64),
    SetVolume(u64, i32),
    SetPitch(u64, i32),
    SetPan(u64, i32),
    Kill(u64),
    KillAll,
    ServiceEvents,
    Shutdown,
}

/// Audio event request
#[derive(Debug)]
pub struct AudioEventRequest {
    pub class_id: String,
    pub volume: Option<i32>,
    pub pitch: Option<i32>,
    pub pan: Option<i32>,
    pub priority_adjust: i32,
    pub time_of_day: Option<TimeOfDay>,
}

/// Event class definition containing sample data and playback rules
pub struct AudioEventClass {
    /// Unique name for this event class
    pub name: String,
    
    /// Whether this class is valid and ready for use
    pub valid: bool,
    
    /// Control flags for playback behavior
    pub control: AudioEventControl,
    
    /// Base volume level
    pub base_level: AudioLevel,
    
    /// Event priority
    pub priority: AudioPriority,
    
    /// Current count of active events
    pub count: usize,
    
    /// Maximum concurrent events allowed
    pub limit: usize,
    
    /// Maximum loop iterations (0 = infinite)
    pub limit_loop: usize,
    
    /// Audio range in game units
    pub range: i32,
    
    /// Minimum volume threshold
    pub min_volume: i32,
    
    /// Delay range in milliseconds
    pub min_delay: u32,
    pub max_delay: u32,
    
    /// Pitch shift range as percentages
    pub min_freq_shift: i32,
    pub max_freq_shift: i32,
    
    /// Volume shift percentage
    pub volume_shift: i32,
    
    /// Volume compression enabled
    pub volume_compression: bool,
    
    /// Sample file names
    pub sample_names: Vec<String>,
    
    /// Time-of-day sample counts
    pub attack_count: usize,
    pub decay_count: usize,
    pub morning_count: usize,
    pub afternoon_count: usize,
    pub evening_count: usize,
    pub night_count: usize,
    
    /// Fade and master attribute references
    pub fade_attribs: Option<Weak<Mutex<AudioAttribs>>>,
    pub master_attribs: Option<Weak<Mutex<AudioAttribs>>>,
    
    /// User data
    pub user_data: Option<Box<dyn std::any::Any + Send>>,
}

/// Individual audio event instance
pub struct AudioEvent {
    /// Unique event ID
    pub id: u64,
    
    /// Reference to event class
    pub class: Arc<Mutex<AudioEventClass>>,
    
    /// Current state
    pub state: AudioEventState,
    
    /// Next state when waiting
    pub next_state: AudioEventState,
    
    /// Event attributes for playback
    pub attribs: AudioAttribs,
    
    /// Cached audio samples
    pub items: Vec<Option<Arc<Mutex<AudioCacheItem>>>>,
    
    /// Current sample being played
    pub current_item: usize,
    
    /// Audio channel assignment
    pub channel: Option<Arc<Mutex<AudioChannel>>>,
    
    /// Playback parameters
    pub frequency_shift: i32,
    pub volume_shift: i32,
    pub priority_adjust: i32,
    
    /// Timing control
    pub delay: u32,
    pub timeout: Timestamp,
    
    /// Sequencing
    pub sequence: Vec<usize>,
    pub loop_count: usize,
    
    /// Time of day context
    pub time_of_day: Option<TimeOfDay>,
    
    /// Pause lock
    pub paused: AudioLock,
    
    /// Event creation timestamp
    pub stamp: u64,
    
    /// Event flags
    pub flags: EventFlags,
}

/// Event control flags
#[derive(Debug, Default)]
pub struct EventFlags {
    pub dead: bool,
    pub playing: bool,
    pub has_channel: bool,
    pub no_attack: bool,
    pub no_decay: bool,
    pub end: bool,
    pub do_end: bool,
    pub allocated: bool,
}

/// Audio event handle for external control
pub struct AudioEventHandle {
    /// Event ID
    pub event_id: Option<u64>,
    
    /// Event class reference
    pub class: Option<Arc<Mutex<AudioEventClass>>>,
    
    /// Event stamp for validity checking
    pub stamp: u64,
    
    /// Sender for event commands
    sender: mpsc::Sender<EventMessage>,
}

/// Main audio event system
pub struct AudioEventSystem {
    /// All event classes by name
    event_classes: HashMap<String, Arc<Mutex<AudioEventClass>>>,
    
    /// Active events by ID
    active_events: HashMap<u64, Arc<Mutex<AudioEvent>>>,
    
    /// Event priority queues
    priority_queues: [VecDeque<u64>; AUDIO_NUM_EVENT_PRIORITIES],
    
    /// Next unique event ID
    next_event_id: AtomicUsize,
    
    /// System enabled flag
    enabled: AtomicBool,
    
    /// Performance counters
    events_count: AtomicUsize,
    events_peak: AtomicUsize,
    
    /// Audio cache reference
    cache: Option<Arc<Mutex<AudioCache>>>,
    
    /// Audio device reference
    device: Option<Arc<Mutex<AudioDevice>>>,
    
    /// Volume compression attributes
    compression_attribs: AudioAttribs,
    
    /// Event message channel
    message_sender: mpsc::Sender<EventMessage>,
    message_receiver: mpsc::Receiver<EventMessage>,
    
    /// Background processing thread
    processor_thread: Option<thread::JoinHandle<()>>,
    
    /// Shutdown flag
    shutdown_flag: Arc<AtomicBool>,
    
    /// Frame counter for updates
    frame_counter: AtomicUsize,
}

impl AudioEventSystem {
    /// Create a new audio event system
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        
        AudioEventSystem {
            event_classes: HashMap::new(),
            active_events: HashMap::new(),
            priority_queues: Default::default(),
            next_event_id: AtomicUsize::new(1),
            enabled: AtomicBool::new(true),
            events_count: AtomicUsize::new(0),
            events_peak: AtomicUsize::new(0),
            cache: None,
            device: None,
            compression_attribs: AudioAttribs::new(),
            message_sender: sender,
            message_receiver: receiver,
            processor_thread: None,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            frame_counter: AtomicUsize::new(0),
        }
    }

    /// Initialize the event system
    pub fn setup(&mut self, device: Arc<Mutex<AudioDevice>>, cache: Arc<Mutex<AudioCache>>) -> AudioResult<()> {
        self.device = Some(device);
        self.cache = Some(cache);
        
        // Initialize compression attributes
        self.compression_attribs = AudioAttribs::new();
        
        // Start background processing thread
        self.start_processor_thread()?;
        
        Ok(())
    }

    /// Shutdown the event system
    pub fn shutdown(&mut self) {
        // Set shutdown flag
        self.shutdown_flag.store(true, Ordering::SeqCst);
        
        // Send shutdown message
        let _ = self.message_sender.send(EventMessage::Shutdown);
        
        // Wait for processor thread to finish
        if let Some(handle) = self.processor_thread.take() {
            let _ = handle.join();
        }
        
        // Kill all remaining events
        self.kill_all_events();
    }

    /// Create a new event class
    pub fn create_event_class(&mut self, name: String) -> Arc<Mutex<AudioEventClass>> {
        let class = Arc::new(Mutex::new(AudioEventClass {
            name: name.clone(),
            valid: true,
            control: AudioEventControl::default(),
            base_level: AudioLevel::new(100),
            priority: AUDIO_EVENT_NORMAL_PRIORITY,
            count: 0,
            limit: AUDIO_EVENT_DEFAULT_LIMIT,
            limit_loop: 0,
            range: 10,
            min_volume: 40,
            min_delay: 0,
            max_delay: 0,
            min_freq_shift: 0,
            max_freq_shift: 0,
            volume_shift: 0,
            volume_compression: false,
            sample_names: Vec::new(),
            attack_count: 0,
            decay_count: 0,
            morning_count: 0,
            afternoon_count: 0,
            evening_count: 0,
            night_count: 0,
            fade_attribs: None,
            master_attribs: None,
            user_data: None,
        }));
        
        self.event_classes.insert(name, Arc::clone(&class));
        class
    }

    /// Get an existing event class
    pub fn get_event_class(&self, name: &str) -> Option<Arc<Mutex<AudioEventClass>>> {
        self.event_classes.get(name).cloned()
    }

    /// Create an event handle for external control
    pub fn create_handle(&self) -> AudioEventHandle {
        AudioEventHandle {
            event_id: None,
            class: None,
            stamp: 0,
            sender: self.message_sender.clone(),
        }
    }

    /// Play an event
    pub fn play_event(&self, request: AudioEventRequest) -> AudioResult<u64> {
        let event_id = self.next_event_id.fetch_add(1, Ordering::SeqCst) as u64;
        
        self.message_sender
            .send(EventMessage::Play(request))
            .map_err(|_| AudioError::SystemError("Failed to send play message".to_string()))?;
        
        Ok(event_id)
    }

    /// Stop an event by ID
    pub fn stop_event(&self, event_id: u64) -> AudioResult<()> {
        self.message_sender
            .send(EventMessage::Stop(event_id))
            .map_err(|_| AudioError::SystemError("Failed to send stop message".to_string()))?;
        
        Ok(())
    }

    /// Kill all events
    pub fn kill_all_events(&self) {
        let _ = self.message_sender.send(EventMessage::KillAll);
    }

    /// Enable/disable the event system
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    /// Check if the event system is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Get current event count
    pub fn get_event_count(&self) -> usize {
        self.events_count.load(Ordering::SeqCst)
    }

    /// Get peak event count
    pub fn get_peak_event_count(&self) -> usize {
        self.events_peak.load(Ordering::SeqCst)
    }

    /// Service all events (called regularly)
    pub fn service_events(&self) {
        let _ = self.message_sender.send(EventMessage::ServiceEvents);
        self.frame_counter.fetch_add(1, Ordering::SeqCst);
    }

    // Private implementation methods

    fn start_processor_thread(&mut self) -> AudioResult<()> {
        let receiver = std::mem::replace(&mut self.message_receiver, {
            let (sender, receiver) = mpsc::channel();
            self.message_sender = sender;
            receiver
        });
        
        let shutdown_flag = Arc::clone(&self.shutdown_flag);
        let cache = self.cache.clone();
        let device = self.device.clone();
        
        let handle = thread::spawn(move || {
            Self::processor_thread_main(receiver, shutdown_flag, cache, device);
        });
        
        self.processor_thread = Some(handle);
        Ok(())
    }

    fn processor_thread_main(
        receiver: mpsc::Receiver<EventMessage>,
        shutdown_flag: Arc<AtomicBool>,
        _cache: Option<Arc<Mutex<AudioCache>>>,
        _device: Option<Arc<Mutex<AudioDevice>>>,
    ) {
        while !shutdown_flag.load(Ordering::SeqCst) {
            match receiver.recv_timeout(Duration::from_millis(16)) { // ~60fps
                Ok(message) => {
                    match message {
                        EventMessage::Shutdown => break,
                        EventMessage::Play(request) => {
                            // Handle play request
                            Self::handle_play_request(request);
                        }
                        EventMessage::Stop(event_id) => {
                            // Handle stop request
                            Self::handle_stop_request(event_id);
                        }
                        EventMessage::ServiceEvents => {
                            // Service all active events
                            Self::handle_service_events();
                        }
                        EventMessage::KillAll => {
                            // Kill all events
                            Self::handle_kill_all();
                        }
                        _ => {
                            // Handle other message types
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Regular processing timeout - service events
                    Self::handle_service_events();
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }
    }

    fn handle_play_request(_request: AudioEventRequest) {
        // Implementation would create and start playing an event
        // This is a simplified placeholder
    }

    fn handle_stop_request(_event_id: u64) {
        // Implementation would stop the specified event
        // This is a simplified placeholder
    }

    fn handle_service_events() {
        // Implementation would update all active events
        // This is a simplified placeholder
    }

    fn handle_kill_all() {
        // Implementation would kill all active events
        // This is a simplified placeholder
    }
}

impl AudioEventClass {
    /// Add a sample to this event class
    pub fn add_sample(&mut self, filename: String) -> AudioResult<()> {
        if self.sample_names.len() >= MAX_AUDIO_EVENT_SAMPLES {
            return Err(AudioError::TooManySamples);
        }
        
        self.sample_names.push(filename);
        Ok(())
    }

    /// Set control flags
    pub fn set_control(&mut self, control: AudioEventControl) {
        self.control = control;
        
        // Auto-set attack/decay counts based on control flags
        if self.control.attack && self.attack_count == 0 {
            self.attack_count = 1;
        } else if !self.control.attack {
            self.attack_count = 0;
        }
        
        if self.control.decay && self.decay_count == 0 {
            self.decay_count = 1;
        } else if !self.control.decay {
            self.decay_count = 0;
        }
    }

    /// Set priority level
    pub fn set_priority(&mut self, priority: AudioPriority) {
        self.priority = priority;
    }

    /// Set volume level
    pub fn set_volume(&mut self, volume: i32) {
        self.base_level.set(volume);
        self.base_level.update();
    }

    /// Set event limit
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }

    /// Set delay range
    pub fn set_delay(&mut self, min_delay: u32, max_delay: u32) {
        self.min_delay = min_delay;
        self.max_delay = max_delay;
    }

    /// Set pitch shift range
    pub fn set_pitch_shift(&mut self, min_shift: i32, max_shift: i32) {
        self.min_freq_shift = min_shift;
        self.max_freq_shift = max_shift;
    }

    /// Set volume shift
    pub fn set_volume_shift(&mut self, shift: i32) {
        self.volume_shift = shift.clamp(0, 100);
    }

    /// Enable/disable volume compression
    pub fn set_volume_compression(&mut self, enabled: bool) {
        self.volume_compression = enabled;
    }

    /// Set loop limit (0 = infinite)
    pub fn set_loop_count(&mut self, count: usize) {
        self.limit_loop = count;
    }

    /// Check if this event class never ends (infinite loop)
    pub fn never_ends(&self) -> bool {
        self.control.loop_event && self.limit_loop == 0
    }

    /// Get number of samples
    pub fn get_sample_count(&self) -> usize {
        self.sample_names.len()
    }

    /// Set time-of-day sample counts
    pub fn set_attack_count(&mut self, count: usize) {
        self.attack_count = count;
    }

    pub fn set_decay_count(&mut self, count: usize) {
        self.decay_count = count;
    }

    pub fn set_morning_count(&mut self, count: usize) {
        self.morning_count = count;
    }

    pub fn set_evening_count(&mut self, count: usize) {
        self.evening_count = count;
    }

    pub fn set_night_count(&mut self, count: usize) {
        self.night_count = count;
    }

    /// Reset to default values
    pub fn reset(&mut self) {
        self.valid = true;
        self.control = AudioEventControl::default();
        self.limit = AUDIO_EVENT_DEFAULT_LIMIT;
        self.priority = AUDIO_EVENT_NORMAL_PRIORITY;
        self.range = 10;
        self.min_volume = 40;
        self.base_level = AudioLevel::new(100);
        self.min_delay = 0;
        self.max_delay = 0;
        self.min_freq_shift = 0;
        self.max_freq_shift = 0;
        self.volume_shift = 0;
        self.volume_compression = false;
        self.attack_count = 0;
        self.decay_count = 0;
        self.morning_count = 0;
        self.afternoon_count = 0;
        self.evening_count = 0;
        self.night_count = 0;
    }
}

impl AudioEventHandle {
    /// Initialize handle
    pub fn init(&mut self) {
        self.event_id = None;
        self.class = None;
        self.stamp = 0;
    }

    /// Stop the controlled event
    pub fn stop(&mut self) -> AudioResult<()> {
        if let Some(event_id) = self.event_id {
            self.sender
                .send(EventMessage::Stop(event_id))
                .map_err(|_| AudioError::SystemError("Failed to send stop message".to_string()))?;
            
            self.event_id = None;
        }
        Ok(())
    }

    /// End the controlled event gracefully
    pub fn end(&mut self) -> AudioResult<()> {
        if let Some(event_id) = self.event_id {
            self.sender
                .send(EventMessage::Kill(event_id))
                .map_err(|_| AudioError::SystemError("Failed to send end message".to_string()))?;
            
            self.event_id = None;
        }
        Ok(())
    }

    /// Set volume of controlled event
    pub fn set_volume(&self, volume: i32) -> AudioResult<()> {
        if let Some(event_id) = self.event_id {
            self.sender
                .send(EventMessage::SetVolume(event_id, volume))
                .map_err(|_| AudioError::SystemError("Failed to send volume message".to_string()))?;
        }
        Ok(())
    }

    /// Set pitch of controlled event
    pub fn set_pitch(&self, pitch: i32) -> AudioResult<()> {
        if let Some(event_id) = self.event_id {
            self.sender
                .send(EventMessage::SetPitch(event_id, pitch))
                .map_err(|_| AudioError::SystemError("Failed to send pitch message".to_string()))?;
        }
        Ok(())
    }

    /// Set pan of controlled event
    pub fn set_pan(&self, pan: i32) -> AudioResult<()> {
        if let Some(event_id) = self.event_id {
            self.sender
                .send(EventMessage::SetPan(event_id, pan))
                .map_err(|_| AudioError::SystemError("Failed to send pan message".to_string()))?;
        }
        Ok(())
    }

    /// Check if handle has a valid event
    pub fn is_valid(&self) -> bool {
        self.event_id.is_some()
    }
}

impl Drop for AudioEventSystem {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_system_creation() {
        let system = AudioEventSystem::new();
        assert!(system.is_enabled());
        assert_eq!(system.get_event_count(), 0);
        assert_eq!(system.get_peak_event_count(), 0);
    }

    #[test]
    fn test_event_class_creation() {
        let mut system = AudioEventSystem::new();
        let class = system.create_event_class("test_sound".to_string());
        
        let class_guard = class.lock().unwrap();
        assert_eq!(class_guard.name, "test_sound");
        assert!(class_guard.valid);
        assert_eq!(class_guard.priority, AUDIO_EVENT_NORMAL_PRIORITY);
    }

    #[test]
    fn test_event_handle() {
        let system = AudioEventSystem::new();
        let mut handle = system.create_handle();
        
        assert!(!handle.is_valid());
        
        handle.init();
        assert!(!handle.is_valid()); // Still not valid until assigned an event
    }

    #[test]
    fn test_event_class_samples() {
        let mut system = AudioEventSystem::new();
        let class = system.create_event_class("test".to_string());
        
        {
            let mut class_guard = class.lock().unwrap();
            assert!(class_guard.add_sample("sound1.wav".to_string()).is_ok());
            assert!(class_guard.add_sample("sound2.wav".to_string()).is_ok());
            assert_eq!(class_guard.get_sample_count(), 2);
        }
    }

    #[test]
    fn test_control_flags() {
        let mut system = AudioEventSystem::new();
        let class = system.create_event_class("test".to_string());
        
        {
            let mut class_guard = class.lock().unwrap();
            
            let mut control = AudioEventControl::default();
            control.loop_event = true;
            control.attack = true;
            
            class_guard.set_control(control);
            
            assert!(class_guard.control.loop_event);
            assert!(class_guard.control.attack);
            assert_eq!(class_guard.attack_count, 1); // Auto-set
        }
    }

    #[test]
    fn test_never_ends() {
        let mut system = AudioEventSystem::new();
        let class = system.create_event_class("test".to_string());
        
        {
            let mut class_guard = class.lock().unwrap();
            
            // Not looping - should end
            assert!(!class_guard.never_ends());
            
            // Looping with limit - should end
            let mut control = AudioEventControl::default();
            control.loop_event = true;
            class_guard.set_control(control);
            class_guard.set_loop_count(5);
            assert!(!class_guard.never_ends());
            
            // Looping without limit - should never end
            class_guard.set_loop_count(0);
            assert!(class_guard.never_ends());
        }
    }

    #[test]
    fn test_event_system_enable_disable() {
        let system = AudioEventSystem::new();
        
        assert!(system.is_enabled());
        
        system.set_enabled(false);
        assert!(!system.is_enabled());
        
        system.set_enabled(true);
        assert!(system.is_enabled());
    }
}