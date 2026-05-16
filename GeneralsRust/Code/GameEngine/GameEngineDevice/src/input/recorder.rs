//! Input recording and playback system for replays

use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{InputError, InputEvent, Result};

/// A single frame of input with timing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFrame {
    /// Timestamp relative to recording start
    pub timestamp: Duration,

    /// Input events that occurred at this timestamp
    pub events: Vec<InputEvent>,
}

/// Input recording metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    /// Recording name
    pub name: String,

    /// Recording creation time
    pub created: std::time::SystemTime,

    /// Total duration
    pub duration: Duration,

    /// Number of frames
    pub frame_count: usize,

    /// Number of events
    pub event_count: usize,

    /// Application version
    pub version: String,

    /// Custom metadata
    pub custom: std::collections::HashMap<String, String>,
}

/// Complete input recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRecording {
    /// Recording metadata
    pub metadata: RecordingMetadata,

    /// Recorded input frames
    pub frames: Vec<InputFrame>,
}

impl InputRecording {
    /// Create a new empty recording
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            metadata: RecordingMetadata {
                name: name.into(),
                created: std::time::SystemTime::now(),
                duration: Duration::ZERO,
                frame_count: 0,
                event_count: 0,
                version: env!("CARGO_PKG_VERSION").to_string(),
                custom: std::collections::HashMap::new(),
            },
            frames: Vec::new(),
        }
    }

    /// Get total duration
    pub fn duration(&self) -> Duration {
        self.metadata.duration
    }

    /// Get event count
    pub fn event_count(&self) -> usize {
        self.metadata.event_count
    }

    /// Get frame count
    pub fn frame_count(&self) -> usize {
        self.metadata.frame_count
    }

    /// Load recording from file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let recording: Self = serde_json::from_str(&content)
            .map_err(|e| InputError::RecordingError(e.to_string()))?;
        Ok(recording)
    }

    /// Save recording to file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| InputError::RecordingError(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Compress recording (remove redundant frames)
    pub fn compress(&mut self) {
        // Remove empty frames
        self.frames.retain(|f| !f.events.is_empty());

        // Update metadata
        self.metadata.frame_count = self.frames.len();
        self.metadata.event_count = self.frames.iter().map(|f| f.events.len()).sum();

        if let Some(last_frame) = self.frames.last() {
            self.metadata.duration = last_frame.timestamp;
        }
    }
}

/// Playback mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackMode {
    /// Play once and stop
    Once,

    /// Loop continuously
    Loop,

    /// Play in reverse
    Reverse,
}

/// Input recorder for capturing and playing back input
pub struct InputRecorder {
    /// Current recording
    recording: Option<InputRecording>,

    /// Recording state
    is_recording: bool,

    /// Recording start time
    recording_start: Option<std::time::Instant>,

    /// Frame buffer for current recording
    frame_buffer: VecDeque<InputFrame>,

    /// Playback state
    is_playing: bool,

    /// Playback mode
    playback_mode: PlaybackMode,

    /// Current playback frame index
    playback_frame: usize,

    /// Playback start time
    playback_start: Option<std::time::Instant>,

    /// Playback speed multiplier
    playback_speed: f32,
}

impl InputRecorder {
    /// Create a new input recorder
    pub fn new() -> Self {
        Self {
            recording: None,
            is_recording: false,
            recording_start: None,
            frame_buffer: VecDeque::new(),
            is_playing: false,
            playback_mode: PlaybackMode::Once,
            playback_frame: 0,
            playback_start: None,
            playback_speed: 1.0,
        }
    }

    /// Start recording
    pub fn start(&mut self) {
        if self.is_recording {
            return;
        }

        self.recording = Some(InputRecording::new("Recording"));
        self.is_recording = true;
        self.recording_start = Some(std::time::Instant::now());
        self.frame_buffer.clear();
    }

    /// Stop recording
    pub fn stop(&mut self) {
        if !self.is_recording {
            return;
        }

        self.is_recording = false;

        // Finalize recording
        if let Some(recording) = &mut self.recording {
            // Add remaining frames from buffer
            recording.frames.extend(self.frame_buffer.drain(..));

            // Update metadata
            recording.compress();
        }

        self.recording_start = None;
    }

    /// Record an input event
    pub fn record_event(&mut self, event: &InputEvent) {
        if !self.is_recording {
            return;
        }

        let timestamp = if let Some(start) = self.recording_start {
            start.elapsed()
        } else {
            Duration::ZERO
        };

        // Check if we have a frame for this timestamp
        if let Some(frame) = self.frame_buffer.back_mut() {
            // If timestamp matches, add to existing frame
            if (frame.timestamp.as_millis() as i64 - timestamp.as_millis() as i64).abs() < 16 {
                // Within 16ms (one frame at 60fps)
                frame.events.push(event.clone());
                return;
            }
        }

        // Create new frame
        let frame = InputFrame {
            timestamp,
            events: vec![event.clone()],
        };

        self.frame_buffer.push_back(frame);

        // Flush buffer if it gets too large
        if self.frame_buffer.len() > 1000 {
            if let Some(recording) = &mut self.recording {
                recording
                    .frames
                    .extend(self.frame_buffer.drain(..500).collect::<Vec<_>>());
            }
        }
    }

    /// Check if recording
    pub fn is_recording(&self) -> bool {
        self.is_recording
    }

    /// Get current recording
    pub fn get_recording(&self) -> Option<&InputRecording> {
        self.recording.as_ref()
    }

    /// Take ownership of recording
    pub fn take_recording(&mut self) -> Option<InputRecording> {
        self.recording.take()
    }

    /// Replace the current recording with an already constructed one.
    pub fn set_recording(&mut self, recording: InputRecording) {
        self.recording = Some(recording);
        self.is_playing = false;
        self.playback_frame = 0;
        self.playback_start = None;
    }

    /// Load a recording
    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let recording = InputRecording::load(path)?;
        self.set_recording(recording);
        Ok(())
    }

    /// Save current recording
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        if let Some(recording) = &self.recording {
            recording.save(path)?;
        } else {
            return Err(InputError::RecordingError("No recording available".into()));
        }
        Ok(())
    }

    /// Start playback
    pub fn start_playback(&mut self, mode: PlaybackMode) -> Result<()> {
        if self.recording.is_none() {
            return Err(InputError::PlaybackError("No recording loaded".into()));
        }

        self.is_playing = true;
        self.playback_mode = mode;
        self.playback_frame = match self.playback_mode {
            PlaybackMode::Reverse => self
                .recording
                .as_ref()
                .map(|recording| recording.frames.len().saturating_sub(1))
                .unwrap_or(0),
            _ => 0,
        };
        self.playback_start = Some(std::time::Instant::now());

        Ok(())
    }

    /// Stop playback
    pub fn stop_playback(&mut self) {
        self.is_playing = false;
        self.playback_frame = 0;
        self.playback_start = None;
    }

    /// Check if playing back
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Set playback speed
    pub fn set_playback_speed(&mut self, speed: f32) {
        self.playback_speed = speed.max(0.1).min(10.0);
    }

    /// Get playback speed
    pub fn playback_speed(&self) -> f32 {
        self.playback_speed
    }

    /// Get next playback event(s) if ready
    pub fn get_playback_event(&mut self, current_time: Duration) -> Option<InputEvent> {
        if !self.is_playing {
            return None;
        }

        let recording = self.recording.as_ref()?;

        if recording.frames.is_empty() {
            self.stop_playback();
            return None;
        }

        // Calculate playback time
        let playback_elapsed = self.playback_start?.elapsed();
        let adjusted_time = playback_elapsed.mul_f32(self.playback_speed);
        let duration = if recording.metadata.duration > Duration::ZERO {
            recording.metadata.duration
        } else {
            recording
                .frames
                .last()
                .map(|frame| frame.timestamp)
                .unwrap_or(Duration::ZERO)
        };

        if matches!(self.playback_mode, PlaybackMode::Reverse) {
            let target_time = duration
                .checked_sub(adjusted_time)
                .unwrap_or(Duration::ZERO);

            while self.playback_frame < recording.frames.len() {
                let frame = &recording.frames[self.playback_frame];

                if frame.timestamp >= target_time {
                    if let Some(event) = frame.events.first() {
                        let event = event.clone();
                        if self.playback_frame == 0 {
                            self.stop_playback();
                            return Some(event);
                        }
                        self.playback_frame -= 1;
                        return Some(event);
                    }

                    if self.playback_frame == 0 {
                        self.stop_playback();
                        break;
                    }
                    self.playback_frame -= 1;
                } else {
                    break;
                }
            }

            if adjusted_time >= duration {
                self.stop_playback();
            }

            let _ = current_time;
            return None;
        }

        // Find frames that should be played
        while self.playback_frame < recording.frames.len() {
            let frame = &recording.frames[self.playback_frame];

            if frame.timestamp <= adjusted_time {
                // This frame should be played
                if let Some(event) = frame.events.first() {
                    // Return first event of this frame
                    // (Multiple events per frame would need queue)
                    let event = event.clone();
                    self.playback_frame += 1;
                    return Some(event);
                }
                self.playback_frame += 1;
            } else {
                // Not time yet
                break;
            }
        }

        // Check if playback finished
        if self.playback_frame >= recording.frames.len() {
            match self.playback_mode {
                PlaybackMode::Once => {
                    self.stop_playback();
                }
                PlaybackMode::Loop => {
                    self.playback_frame = 0;
                    self.playback_start = Some(std::time::Instant::now());
                }
                PlaybackMode::Reverse => {}
            }
        }

        None
    }

    /// Get playback progress (0.0 to 1.0)
    pub fn playback_progress(&self) -> f32 {
        if let Some(recording) = &self.recording {
            if recording.frames.is_empty() {
                return 0.0;
            }
            self.playback_frame as f32 / recording.frames.len() as f32
        } else {
            0.0
        }
    }

    /// Seek to specific time in playback
    pub fn seek(&mut self, time: Duration) -> Result<()> {
        if !self.is_playing {
            return Err(InputError::PlaybackError("Not currently playing".into()));
        }

        let recording = self
            .recording
            .as_ref()
            .ok_or_else(|| InputError::PlaybackError("No recording loaded".into()))?;

        if matches!(self.playback_mode, PlaybackMode::Reverse) {
            for (i, frame) in recording.frames.iter().enumerate().rev() {
                if frame.timestamp <= time {
                    self.playback_frame = i;
                    self.playback_start =
                        Some(std::time::Instant::now() - time.mul_f32(1.0 / self.playback_speed));
                    return Ok(());
                }
            }

            self.playback_frame = 0;
            return Ok(());
        }

        // Find frame index for target time
        for (i, frame) in recording.frames.iter().enumerate() {
            if frame.timestamp >= time {
                self.playback_frame = i;
                self.playback_start =
                    Some(std::time::Instant::now() - time.mul_f32(1.0 / self.playback_speed));
                return Ok(());
            }
        }

        // If we didn't find a frame, go to end
        self.playback_frame = recording.frames.len();
        Ok(())
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.stop();
        self.stop_playback();
        self.recording = None;
        self.frame_buffer.clear();
    }
}

impl Default for InputRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, ModifierKeys};

    #[test]
    fn test_input_frame() {
        let frame = InputFrame {
            timestamp: Duration::from_secs(1),
            events: vec![],
        };

        assert_eq!(frame.timestamp, Duration::from_secs(1));
        assert!(frame.events.is_empty());
    }

    #[test]
    fn test_recording_creation() {
        let recording = InputRecording::new("Test");
        assert_eq!(recording.metadata.name, "Test");
        assert_eq!(recording.frame_count(), 0);
    }

    #[test]
    fn test_recorder() {
        let mut recorder = InputRecorder::new();

        assert!(!recorder.is_recording());

        recorder.start();
        assert!(recorder.is_recording());

        let event = InputEvent::KeyPressed {
            key: KeyCode::A,
            modifiers: ModifierKeys::empty(),
            timestamp: Duration::from_millis(100),
        };

        recorder.record_event(&event);

        recorder.stop();
        assert!(!recorder.is_recording());

        let recording = recorder.get_recording().unwrap();
        assert!(recording.event_count() > 0);
    }

    #[test]
    fn test_playback() {
        let mut recorder = InputRecorder::new();

        // Create a simple recording
        let mut recording = InputRecording::new("Test");
        recording.frames.push(InputFrame {
            timestamp: Duration::from_millis(100),
            events: vec![InputEvent::KeyPressed {
                key: KeyCode::A,
                modifiers: ModifierKeys::empty(),
                timestamp: Duration::from_millis(100),
            }],
        });

        recording.compress();
        recorder.recording = Some(recording);

        assert!(!recorder.is_playing());

        let result = recorder.start_playback(PlaybackMode::Once);
        assert!(result.is_ok());
        assert!(recorder.is_playing());

        recorder.stop_playback();
        assert!(!recorder.is_playing());
    }

    #[test]
    fn test_playback_progress() {
        let mut recorder = InputRecorder::new();

        let mut recording = InputRecording::new("Test");
        for i in 0..10 {
            recording.frames.push(InputFrame {
                timestamp: Duration::from_millis(i * 100),
                events: vec![],
            });
        }

        recorder.recording = Some(recording);
        recorder.playback_frame = 5;

        assert_eq!(recorder.playback_progress(), 0.5);
    }
}
