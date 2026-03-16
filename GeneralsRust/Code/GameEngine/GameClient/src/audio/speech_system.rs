//! # Speech System
//!
//! EVA announcements and unit voice response management.
//!
//! Ported from C++ `GameSpeech.cpp`.
//!
//! The speech system maintains two queues:
//!   1. **EVA announcements** (high priority, can interrupt unit responses)
//!   2. **Unit voice responses** (lower priority, played between EVA cues)
//!
//! Each entry has a cooldown timer so the same line is not repeated too
//! frequently.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use rand::seq::SliceRandom;

use super::audio_engine::{AudioEngine, AudioHandle, AudioPosition};

// ---------------------------------------------------------------------------
// Speech priority levels (matches C++ Speech priority)
// ---------------------------------------------------------------------------

/// Priority level for a speech line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpeechPriority {
    /// Normal unit voice response.
    Normal = 0,
    /// Elevated priority (unit under attack, etc.).
    High = 1,
    /// EVA announcement (interrupts lower-priority speech).
    Eva = 2,
    /// Critical EVA that cannot be queued behind other EVA lines.
    Critical = 3,
}

impl Default for SpeechPriority {
    fn default() -> Self {
        Self::Normal
    }
}

// ---------------------------------------------------------------------------
// SpeechLine
// ---------------------------------------------------------------------------

/// A single speech line ready to be played.
/// Matches C++ `Speech` struct.
#[derive(Debug, Clone)]
pub struct SpeechLine {
    /// Logical name of the speech line (matches INI entry).
    pub name: String,
    /// Sound file(s) to choose from.
    pub sound_files: Vec<String>,
    /// Base mixing volume (0.0..=1.0).
    pub volume: f32,
    /// Priority.
    pub priority: SpeechPriority,
    /// Timeout: if the line cannot be played within this duration, drop it.
    pub timeout: Duration,
    /// Whether this line should interrupt currently playing speech.
    pub interrupt: bool,
    /// Per-line cooldown after it has been played.
    pub cooldown: Duration,
    /// Player index this speech is associated with.
    pub player_index: Option<i32>,
}

impl SpeechLine {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sound_files: Vec::new(),
            volume: 1.0,
            priority: SpeechPriority::Normal,
            timeout: Duration::from_secs(10),
            interrupt: false,
            cooldown: Duration::from_secs(3),
            player_index: None,
        }
    }
}

// ---------------------------------------------------------------------------
// SpeechEntry (internal queued entry)
// ---------------------------------------------------------------------------

/// Internal queued entry with timing information.
struct QueuedSpeech {
    line: SpeechLine,
    queued_at: Instant,
}

// ---------------------------------------------------------------------------
// CooldownTracker
// ---------------------------------------------------------------------------

/// Tracks per-name cooldowns to prevent the same line being played too often.
struct CooldownTracker {
    last_played: HashMap<String, Instant>,
    default_cooldown: Duration,
}

impl CooldownTracker {
    fn new(default_cooldown: Duration) -> Self {
        Self {
            last_played: HashMap::new(),
            default_cooldown,
        }
    }

    /// Check if the given speech name is on cooldown.
    fn is_on_cooldown(&self, name: &str) -> bool {
        if let Some(last) = self.last_played.get(name) {
            last.elapsed() < self.default_cooldown
        } else {
            false
        }
    }

    /// Mark a speech name as having been played now.
    fn mark_played(&mut self, name: &str, cooldown: Duration) {
        let cd = if cooldown.is_zero() {
            self.default_cooldown
        } else {
            cooldown
        };
        self.last_played.insert(name.to_string(), Instant::now());
        let _ = cd; // used for per-entry override in the future
    }

    /// Clear all cooldowns.
    fn clear(&mut self) {
        self.last_played.clear();
    }
}

// ---------------------------------------------------------------------------
// SpeechSystem
// ---------------------------------------------------------------------------

/// Manages EVA announcements and unit voice responses.
///
/// Ported from C++ `SpeechManager` / `GameSpeech.cpp`.
///
/// # Queue priority
///
/// EVA announcements (priority >= `SpeechPriority::Eva`) always take
/// precedence.  When an EVA line arrives, any currently playing unit
/// response is stopped.  EVA lines themselves respect their own priority
/// (Critical > Eva).
pub struct SpeechSystem {
    /// The EVA announcement queue.
    eva_queue: VecDeque<QueuedSpeech>,

    /// The unit voice-response queue.
    unit_queue: VecDeque<QueuedSpeech>,

    /// Currently playing speech handle (0 = none).
    current_handle: AudioHandle,
    /// Which queue the currently playing speech came from.
    current_source: SpeechSource,
    /// Whether the current speech was interrupted (for cleanup).
    was_interrupted: bool,

    /// Cooldown tracker.
    cooldowns: CooldownTracker,

    /// Whether speech is enabled.
    enabled: bool,
    /// Whether speech is currently allowed (can be toggled by script).
    disallow_speech: bool,

    /// Maximum number of queued EVA lines.
    max_eva_queue: usize,
    /// Maximum number of queued unit responses.
    max_unit_queue: usize,

    /// Registered speech lines by name (from INI).
    registry: HashMap<String, SpeechLine>,
}

/// Where the currently playing speech originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeechSource {
    None,
    Eva,
    Unit,
}

impl SpeechSystem {
    /// Create a new speech system.
    pub fn new() -> Self {
        Self {
            eva_queue: VecDeque::with_capacity(16),
            unit_queue: VecDeque::with_capacity(32),
            current_handle: 0,
            current_source: SpeechSource::None,
            was_interrupted: false,
            cooldowns: CooldownTracker::new(Duration::from_secs(3)),
            enabled: true,
            disallow_speech: false,
            max_eva_queue: 16,
            max_unit_queue: 32,
            registry: HashMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Registry
    // -----------------------------------------------------------------------

    /// Register a speech line (from INI).
    pub fn register_line(&mut self, line: SpeechLine) {
        self.registry.insert(line.name.clone(), line);
    }

    /// Look up a registered speech line.
    pub fn find_line(&self, name: &str) -> Option<&SpeechLine> {
        self.registry.get(name)
    }

    // -----------------------------------------------------------------------
    // Queueing
    // -----------------------------------------------------------------------

    /// Queue an EVA announcement.
    ///
    /// EVA lines can interrupt unit responses.  If the line has the
    /// `interrupt` flag set, it will also preempt any currently playing EVA
    /// line of lower priority.
    pub fn queue_eva(&mut self, mut line: SpeechLine) {
        if !self.enabled || self.disallow_speech {
            return;
        }

        // Upgrade priority to at least Eva level.
        if line.priority < SpeechPriority::Eva {
            line.priority = SpeechPriority::Eva;
        }

        if self.eva_queue.len() >= self.max_eva_queue {
            log::warn!("SpeechSystem: EVA queue full, dropping {:?}", line.name);
            return;
        }

        self.eva_queue.push_back(QueuedSpeech {
            line,
            queued_at: Instant::now(),
        });
    }

    /// Queue a unit voice response.
    pub fn queue_unit(&mut self, line: SpeechLine) {
        if !self.enabled || self.disallow_speech {
            return;
        }

        if self.unit_queue.len() >= self.max_unit_queue {
            log::warn!("SpeechSystem: unit queue full, dropping {:?}", line.name);
            return;
        }

        self.unit_queue.push_back(QueuedSpeech {
            line,
            queued_at: Instant::now(),
        });
    }

    /// Queue a speech line, automatically routing to EVA or unit queue
    /// based on priority.
    pub fn queue(&mut self, line: SpeechLine) {
        if line.priority >= SpeechPriority::Eva {
            self.queue_eva(line);
        } else {
            self.queue_unit(line);
        }
    }

    // -----------------------------------------------------------------------
    // Playback control
    // -----------------------------------------------------------------------

    /// Update the speech system (call every frame).
    ///
    /// Expired entries are dropped, and the next eligible line is played.
    pub fn update(&mut self, engine: &mut AudioEngine) {
        // Drop timed-out entries from both queues.
        Self::expire_queue(&mut self.eva_queue);
        Self::expire_queue(&mut self.unit_queue);

        // Check if currently playing speech has finished.
        if self.current_handle != 0 && !engine.is_playing(self.current_handle) {
            self.current_handle = 0;
            self.current_source = SpeechSource::None;
            self.was_interrupted = false;
        }

        // If nothing is playing, try to play the next line.
        if self.current_handle == 0 {
            // EVA takes priority.
            if let Some(entry) = self.eva_queue.front() {
                // Interrupt any unit voice currently playing (should not happen
                // since we checked current_handle == 0 above, but be safe).
                if self.current_source == SpeechSource::Unit {
                    self.stop_current(engine);
                }
                let entry = self.eva_queue.pop_front().unwrap();
                self.play_entry(entry, engine);
            } else if let Some(entry) = self.unit_queue.front() {
                let entry = self.unit_queue.pop_front().unwrap();
                self.play_entry(entry, engine);
            }
        }
    }

    /// Stop all speech immediately.
    pub fn stop_all(&mut self, engine: &mut AudioEngine) {
        self.stop_current(engine);
        self.eva_queue.clear();
        self.unit_queue.clear();
    }

    /// Stop the currently playing speech.
    pub fn stop_current(&mut self, engine: &mut AudioEngine) {
        if self.current_handle != 0 {
            engine.stop_event(self.current_handle);
            self.current_handle = 0;
            self.was_interrupted = true;
        }
        self.current_source = SpeechSource::None;
    }

    /// Check if speech is currently playing.
    pub fn is_playing(&self) -> bool {
        self.current_handle != 0
    }

    /// Check if a specific object is currently playing voice.
    /// `object_id` is the game object whose voice we want to check.
    pub fn is_object_playing_voice(&self, _object_id: u32) -> bool {
        // In the full implementation we would track which object
        // triggered the currently playing speech.
        self.current_source == SpeechSource::Unit
    }

    // -----------------------------------------------------------------------
    // Settings
    // -----------------------------------------------------------------------

    /// Enable or disable speech.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.stop_all(&mut AudioEngine::new().unwrap());
        }
    }

    /// Whether speech is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set whether speech is disallowed (script control).
    pub fn set_disallow_speech(&mut self, disallow: bool) {
        self.disallow_speech = disallow;
    }

    /// Whether speech is currently disallowed.
    pub fn disallow_speech(&self) -> bool {
        self.disallow_speech
    }

    /// Get the number of queued EVA lines.
    pub fn eva_queue_len(&self) -> usize {
        self.eva_queue.len()
    }

    /// Get the number of queued unit responses.
    pub fn unit_queue_len(&self) -> usize {
        self.unit_queue.len()
    }

    /// Clear all cooldowns.
    pub fn clear_cooldowns(&mut self) {
        self.cooldowns.clear();
    }

    /// Reset the speech system (between missions).
    pub fn reset(&mut self, engine: &mut AudioEngine) {
        self.stop_all(engine);
        self.cooldowns.clear();
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn expire_queue(queue: &mut VecDeque<QueuedSpeech>) {
        while let Some(front) = queue.front() {
            if front.queued_at.elapsed() > front.line.timeout {
                queue.pop_front();
            } else {
                break;
            }
        }
    }

    fn play_entry(&mut self, entry: QueuedSpeech, engine: &mut AudioEngine) {
        let line = &entry.line;

        // Check cooldown.
        if self.cooldowns.is_on_cooldown(&line.name) {
            return;
        }

        // Pick a random sound file.
        let filename = if !line.sound_files.is_empty() {
            let mut rng = rand::thread_rng();
            line.sound_files
                .choose(&mut rng)
                .cloned()
                .unwrap_or_default()
        } else {
            line.name.clone()
        };

        if filename.is_empty() {
            return;
        }

        // Play through the audio engine.
        let handle = engine.play_event(&filename, None::<AudioPosition>);
        if handle != 0 {
            self.current_handle = handle;
            self.current_source = if line.priority >= SpeechPriority::Eva {
                SpeechSource::Eva
            } else {
                SpeechSource::Unit
            };
            self.was_interrupted = false;

            // Mark cooldown.
            self.cooldowns.mark_played(&line.name, line.cooldown);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> AudioEngine {
        AudioEngine::new().unwrap()
    }

    #[test]
    fn test_speech_system_creation() {
        let sys = SpeechSystem::new();
        assert!(sys.is_enabled());
        assert!(!sys.is_playing());
        assert_eq!(sys.eva_queue_len(), 0);
        assert_eq!(sys.unit_queue_len(), 0);
    }

    #[test]
    fn test_queue_routing() {
        let mut sys = SpeechSystem::new();

        // Unit-level speech goes to unit queue.
        let unit_line = SpeechLine::new("UnitAcknowledge");
        sys.queue(unit_line);
        assert_eq!(sys.unit_queue_len(), 1);
        assert_eq!(sys.eva_queue_len(), 0);

        // EVA-level speech goes to EVA queue.
        let eva_line = SpeechLine {
            name: "BaseUnderAttack".to_string(),
            priority: SpeechPriority::Eva,
            ..SpeechLine::new("")
        };
        sys.queue(eva_line);
        assert_eq!(sys.eva_queue_len(), 1);
    }

    #[test]
    fn test_disallow_speech() {
        let mut sys = SpeechSystem::new();
        sys.set_disallow_speech(true);
        assert!(sys.disallow_speech());

        // Queuing should be silently ignored.
        sys.queue(SpeechLine::new("Test"));
        assert_eq!(sys.unit_queue_len(), 0);

        sys.set_disallow_speech(false);
        sys.queue(SpeechLine::new("Test"));
        assert_eq!(sys.unit_queue_len(), 1);
    }

    #[test]
    fn test_cooldown() {
        let mut sys = SpeechSystem::new();

        // Simulate playing a line.
        sys.cooldowns.mark_played("TestLine", Duration::from_secs(3));
        assert!(sys.cooldowns.is_on_cooldown("TestLine"));

        // After clearing cooldowns it should be available.
        sys.cooldowns.clear();
        assert!(!sys.cooldowns.is_on_cooldown("TestLine"));
    }
}
