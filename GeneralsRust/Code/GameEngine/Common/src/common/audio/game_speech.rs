////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! GameSpeech - Speech and dialog management system
//! Westwood Studios Pacific
//! Converted to Rust

use crate::common::audio::audio_event_rts::{AudioEventRts, TimeOfDay};
use std::collections::{HashMap, VecDeque};
use std::sync::{OnceLock, RwLock};

// Type aliases
pub type AsciiString = String;
pub type Real = f32;
pub type Bool = bool;
pub type Int = i32;
pub type UnsignedInt = u32;
pub type TimeStamp = u64;

// Constants
const BASE_DLG_ID: u32 = 5000;
const BASE_DLG_DIR: &str = "Data\\Audio\\Sounds";
const BASE_DLG_EXT: &str = "wav";
const NUM_DLG_PRIORITIES: usize = 5;

static SPEECH_REGISTRY: OnceLock<RwLock<HashMap<String, Speech>>> = OnceLock::new();

fn speech_registry() -> &'static RwLock<HashMap<String, Speech>> {
    SPEECH_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Information about a line of speech/dialog
#[derive(Debug, Clone)]
pub struct SpeechInfo {
    pub dialog_event: AsciiString,
    pub volume: Real,
    pub priority: Int,
    pub interruptable: Bool,
    pub time_of_day: TimeOfDay,
    pub dialog_files: Vec<AsciiString>,
    pub dialog_files_evening: Vec<AsciiString>,
    pub dialog_files_morning: Vec<AsciiString>,
    pub dialog_files_night: Vec<AsciiString>,
    pub random_start_index: Int,
    pub sequential_start_index: Int,
    pub internal_play_count: Int,
}

impl SpeechInfo {
    pub fn new() -> Self {
        SpeechInfo {
            dialog_event: String::new(),
            volume: 0.5,
            priority: 0,
            interruptable: false,
            time_of_day: TimeOfDay::Afternoon,
            dialog_files: Vec::new(),
            dialog_files_evening: Vec::new(),
            dialog_files_morning: Vec::new(),
            dialog_files_night: Vec::new(),
            random_start_index: 0,
            sequential_start_index: -1,
            internal_play_count: 0,
        }
    }
}

impl Default for SpeechInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// The Speech struct holds all information about a line of dialog
#[derive(Debug, Clone)]
pub struct Speech {
    pub id: u32,
    pub index: Int,
    pub name: AsciiString,
    pub volume: Real,
    pub info: SpeechInfo,
    pub valid: Bool,
    pub priority: Int,
    pub timeout: Int,
    pub interrupt: Int,
}

impl Speech {
    pub fn new() -> Self {
        Speech {
            id: 0,
            index: 0,
            name: String::new(),
            volume: 0.5,
            info: SpeechInfo::new(),
            valid: false,
            priority: 0,
            timeout: 65000,
            interrupt: 0,
        }
    }

    pub fn from_speech_info(speech_info: &SpeechInfo) -> Self {
        let mut speech = Speech::new();
        speech.name = speech_info.dialog_event.clone();
        speech.volume = speech_info.volume;
        speech.timeout = 65000;
        speech.interrupt = if speech_info.interruptable { 1 } else { 0 };
        speech.valid = true;
        speech.priority = speech_info.priority;
        speech.info = speech_info.clone();
        speech
    }
}

impl Default for Speech {
    fn default() -> Self {
        Self::new()
    }
}

/// Flags for speech items
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeechItemFlags {
    None = 0,
    Paused = 0x00000001,
}

/// Internal structure used by the SpeechManager for tracking dialog playback
#[derive(Debug, Clone)]
pub struct SpeechItem {
    pub flags: SpeechItemFlags,
    pub speech: Option<Speech>,
    pub priority: Int,
    pub timeout: TimeStamp,
}

impl SpeechItem {
    pub fn new() -> Self {
        SpeechItem {
            flags: SpeechItemFlags::None,
            speech: None,
            priority: 0,
            timeout: 0,
        }
    }

    pub fn with_speech(speech: Speech, priority: Int, timeout: TimeStamp) -> Self {
        SpeechItem {
            flags: SpeechItemFlags::None,
            speech: Some(speech),
            priority,
            timeout,
        }
    }

    pub fn is_paused(&self) -> Bool {
        matches!(self.flags, SpeechItemFlags::Paused)
    }

    pub fn set_paused(&mut self, paused: Bool) {
        if paused {
            self.flags = SpeechItemFlags::Paused;
        } else {
            self.flags = SpeechItemFlags::None;
        }
    }
}

impl Default for SpeechItem {
    fn default() -> Self {
        Self::new()
    }
}

/// Speaker interface - handles playback of speech for a specific priority level
pub struct Speaker {
    pub name: AsciiString,
    pub priority: Int,
    pub paused: Int,
    pub delay: TimeStamp,
    pub delay_time: TimeStamp,
    pub current_speech: Option<SpeechItem>,
    pub pending: VecDeque<SpeechItem>,
    pub buffer_time: Int,
}

impl Speaker {
    pub fn new() -> Self {
        Speaker {
            name: "Unnamed".to_string(),
            priority: 0,
            paused: 0,
            delay: 0,
            delay_time: 300,
            current_speech: None,
            pending: VecDeque::new(),
            buffer_time: 3000,
        }
    }

    pub fn init(&mut self, name: &str, priority: Int, buffer_time: Int, delay: Int) {
        self.deinit();
        self.name = name.to_string();
        self.priority = priority;
        self.delay_time = delay as TimeStamp;
        self.buffer_time = buffer_time;

        // In the original, this would create audio streamers
        // For now, we'll just track the configuration
    }

    pub fn deinit(&mut self) {
        self.stop();
        // Clean up any audio resources
    }

    /// Submit speech to play
    pub fn say_speech(&mut self, speech: Speech, priority: Int, timeout: Int, interrupt: Int) {
        if !speech.valid {
            return;
        }

        // Check if we're already saying this or going to say it
        if self.is_saying(&speech) || self.is_going_to_say(&speech) {
            return;
        }

        let timeout_val = if timeout == 0 {
            speech.timeout
        } else {
            timeout
        };
        let priority_val = if priority == 0 {
            speech.priority
        } else {
            priority
        };
        let interrupt_val = if interrupt == 0 {
            speech.interrupt
        } else {
            interrupt
        };

        let mut item = SpeechItem::with_speech(speech, priority_val, timeout_val as TimeStamp);

        if self.paused > 0 {
            item.set_paused(true);
        }

        // Calculate actual timeout
        if timeout_val != 0 {
            item.timeout = if self.paused > 0 {
                timeout_val as TimeStamp // Store relative timeout for paused state
            } else {
                self.get_current_time() + (timeout_val as TimeStamp)
            };
        }

        // Insert based on priority
        let insert_pos = self
            .pending
            .iter()
            .position(|existing| existing.priority < item.priority)
            .unwrap_or(self.pending.len());
        self.pending.insert(insert_pos, item);

        // Handle interruption
        if interrupt_val != 0 {
            if let Some(current) = &self.current_speech {
                if current.priority < priority_val {
                    // Stop current speech
                    self.current_speech = None;
                    self.delay = 0;
                }
            }
        }
    }

    /// Submit speech by name
    pub fn say_name(&mut self, speech_name: &str, priority: Int, timeout: Int, interrupt: Int) {
        let resolved = speech_registry()
            .read()
            .ok()
            .and_then(|registry| registry.get(speech_name).cloned());
        if let Some(speech) = resolved {
            self.say_speech(speech, priority, timeout, interrupt);
        }
    }

    pub fn set_priority(&mut self, priority: Int) {
        self.priority = priority;
    }

    pub fn get_priority(&self) -> Int {
        self.priority
    }

    pub fn set_delay(&mut self, delay: Int) {
        self.delay = delay as TimeStamp;
    }

    pub fn get_delay(&self) -> Int {
        self.delay as Int
    }

    pub fn set_buffering(&mut self, buffer_time: Int) {
        self.buffer_time = buffer_time;
    }

    pub fn get_buffering(&self) -> Int {
        self.buffer_time
    }

    pub fn pause(&mut self) {
        if self.paused == 0 {
            // Pause audio stream if playing

            let now = self.get_current_time();
            for item in &mut self.pending {
                if !item.is_paused() {
                    item.set_paused(true);
                    if item.timeout != 0 {
                        item.timeout = item.timeout.saturating_sub(now);
                    }
                }
            }
        }
        self.paused += 1;
    }

    pub fn resume(&mut self) {
        if self.paused == 1 {
            let now = self.get_current_time();

            for item in &mut self.pending {
                if item.is_paused() {
                    item.set_paused(false);
                    if item.timeout != 0 {
                        item.timeout = now + item.timeout;
                    }
                }
            }

            // Resume audio stream if paused
        }

        self.paused -= 1;
        if self.paused < 0 {
            self.paused = 0;
        }
    }

    pub fn stop(&mut self) {
        // Stop audio stream
        self.flush();
        self.current_speech = None;
    }

    pub fn cancel(&mut self, speech: &Speech) {
        self.pending.retain(|item| {
            if let Some(item_speech) = &item.speech {
                item_speech.name != speech.name
            } else {
                true
            }
        });
    }

    pub fn has_said(&self, speech: &Speech) -> Bool {
        !self.is_going_to_say(speech) && !self.is_saying(speech)
    }

    pub fn is_going_to_say(&self, speech: &Speech) -> Bool {
        self.pending.iter().any(|item| {
            if let Some(item_speech) = &item.speech {
                item_speech.name == speech.name
            } else {
                false
            }
        })
    }

    pub fn is_talking(&self) -> Bool {
        self.current_speech.is_some()
    }

    pub fn saying(&self) -> Option<&Speech> {
        self.current_speech.as_ref()?.speech.as_ref()
    }

    pub fn update(&mut self) {
        // Service the speaker - check for completed audio, start new audio, etc.
        let now = self.get_current_time();

        // Remove expired items
        self.remove_expired_items(now);

        // In the C++ path, current speech is released once streamer playback completes.
        // Without a bound streamer in this compatibility layer, advance once delay expires.
        if self.current_speech.is_some() && self.delay <= now {
            self.current_speech = None;
        }

        // Start next speech if current one is done and delay has passed
        if self.current_speech.is_none() && self.paused == 0 && self.delay <= now {
            if let Some(next_item) = self.pending.pop_front() {
                self.current_speech = Some(next_item);
                self.delay = now + self.delay_time;
            }
        }
    }

    fn flush(&mut self) {
        self.pending.clear();
    }

    fn remove_expired_items(&mut self, now: TimeStamp) {
        self.pending
            .retain(|item| item.timeout == 0 || item.timeout >= now);
    }

    fn is_saying(&self, speech: &Speech) -> Bool {
        if let Some(current) = &self.current_speech {
            if let Some(current_speech) = &current.speech {
                return current_speech.name == speech.name;
            }
        }
        false
    }

    fn get_current_time(&self) -> TimeStamp {
        // In the original, this would call AudioGetTime()
        // For now, use system time
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as TimeStamp
    }
}

impl Default for Speaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Main speech management system
pub struct SpeechManager {
    volume: Real,
    on: Bool,
    count: Int,

    speakers: Vec<Speaker>,
    speeches: HashMap<String, Speech>,
    speech_names: Vec<AsciiString>,
    temporary_speeches: Vec<Speech>,
}

impl SpeechManager {
    pub fn new() -> Self {
        SpeechManager {
            volume: 0.5,
            on: false,
            count: 0,
            speakers: Vec::new(),
            speeches: HashMap::new(),
            speech_names: Vec::new(),
            temporary_speeches: Vec::new(),
        }
    }

    /// Initialize the speech system
    pub fn init(&mut self) -> Bool {
        self.deinit();

        // Create speakers for different priority levels
        for i in 0..NUM_DLG_PRIORITIES {
            let speaker_name = format!("Priority{}", i + 1);
            let mut speaker = Speaker::new();
            speaker.init(&speaker_name, (i + 1) as Int, 3000, 300);
            self.speakers.push(speaker);
        }

        self.on = true;
        true
    }

    /// Deinitialize the speech system
    pub fn deinit(&mut self) {
        self.count = 0;
        self.speeches.clear();
        self.speech_names.clear();
        if let Ok(mut registry) = speech_registry().write() {
            registry.clear();
        }

        for speaker in &mut self.speakers {
            speaker.deinit();
        }
        self.speakers.clear();

        self.stop();
    }

    /// Update the speech system
    pub fn update(&mut self) {
        for speaker in &mut self.speakers {
            speaker.update();
        }
    }

    pub fn reset(&mut self) {
        // Reset speech system state
    }

    pub fn lose_focus(&mut self) {
        // Handle application losing focus
    }

    pub fn regain_focus(&mut self) {
        // Handle application regaining focus
    }

    pub fn num_speeches(&self) -> Int {
        self.count
    }

    pub fn get_speech_info<'a>(&self, speech: &'a Speech) -> Option<&'a SpeechInfo> {
        Some(&speech.info)
    }

    pub fn get_speech_index(&self, speech: &Speech) -> Int {
        speech.index
    }

    pub fn get_speech_id(&self, speech: &Speech) -> u32 {
        speech.id
    }

    pub fn get_speech_by_name(&self, name: &str) -> Option<&Speech> {
        self.speeches.get(name)
    }

    pub fn get_speech_by_id(&self, id: u32) -> Option<&Speech> {
        self.speeches.values().find(|speech| speech.id == id)
    }

    pub fn get_speech_by_index(&self, index: Int) -> Option<&Speech> {
        if index >= 0 && (index as usize) < self.speech_names.len() {
            let name = &self.speech_names[index as usize];
            self.speeches.get(name)
        } else {
            None
        }
    }

    pub fn get_speech_name<'a>(&self, speech: &'a Speech) -> &'a str {
        &speech.name
    }

    pub fn fade_in(&mut self) {
        // Fade all speech in
    }

    pub fn fade_out(&mut self) {
        // Fade all speech out
    }

    pub fn fade(&mut self, fade_value: Real) {
        // Fade all speech by a specified amount
    }

    pub fn is_fading(&self) -> Bool {
        // Return whether any speech is in the process of fading
        false
    }

    pub fn wait_for_fade(&mut self) {
        // Wait for all fading to finish
    }

    pub fn set_volume(&mut self, new_volume: Real) {
        self.volume = new_volume.clamp(0.0, 1.0);
    }

    pub fn get_volume(&self) -> Real {
        self.volume
    }

    /// Create a new speaker
    pub fn create_speaker(
        &mut self,
        name: &str,
        priority: Int,
        buffer_time: Int,
        delay: Int,
    ) -> usize {
        let mut speaker = Speaker::new();
        speaker.init(name, priority, buffer_time, delay);
        self.speakers.push(speaker);
        self.speakers.len() - 1 // Return index
    }

    pub fn add_new_speech(&mut self, speech_to_add: &SpeechInfo) -> Option<&Speech> {
        let entry_key = speech_to_add.dialog_event.clone();

        if !self.speeches.contains_key(&entry_key) {
            self.count += 1;
            self.speech_names.push(speech_to_add.dialog_event.clone());
        }

        let mut new_speech = Speech::from_speech_info(speech_to_add);
        new_speech.id = (self.count as u32) + BASE_DLG_ID;
        new_speech.index = self.count - 1;

        self.speeches.insert(entry_key.clone(), new_speech);
        if let Some(speech) = self.speeches.get(&entry_key).cloned() {
            if let Ok(mut registry) = speech_registry().write() {
                registry.insert(entry_key.clone(), speech);
            }
        }
        self.speeches.get(&entry_key)
    }

    pub fn add_temporary_dialog(&mut self, temporary_speech: Speech) -> Option<&Speech> {
        self.temporary_speeches.push(temporary_speech);
        if let Some(speech) = self.temporary_speeches.last().cloned() {
            if let Ok(mut registry) = speech_registry().write() {
                registry.insert(speech.name.clone(), speech);
            }
        }
        self.temporary_speeches.last()
    }

    /// Stop all speech
    pub fn stop(&mut self) {
        for speaker in &mut self.speakers {
            speaker.stop();
        }
    }

    pub fn pause(&mut self) {
        for speaker in &mut self.speakers {
            speaker.pause();
        }
    }

    pub fn resume(&mut self) {
        for speaker in &mut self.speakers {
            speaker.resume();
        }
    }

    pub fn turn_off(&mut self) {
        self.stop();
        self.on = false;
    }

    pub fn turn_on(&mut self) {
        self.on = true;
    }

    pub fn wait_to_stop(&mut self, _milliseconds: Int) -> Bool {
        // Return when all speech has completely stopped playing
        true
    }

    /// Attempt to add a speech to the speech queue
    pub fn say(&mut self, speech_to_say: &Speech) -> Bool {
        if self.speakers.is_empty() {
            return false;
        }

        let speaker_index = (speech_to_say.priority as usize).min(self.speakers.len() - 1);
        if let Some(speaker) = self.speakers.get_mut(speaker_index) {
            speaker.say_speech(
                speech_to_say.clone(),
                speech_to_say.priority,
                speech_to_say.timeout,
                speech_to_say.interrupt,
            );
            return true;
        }

        false
    }

    /// Attempt to find and add a speech to the speech queue
    pub fn say_by_name(&mut self, speech_name: &str) -> Bool {
        if let Some(speech) = self.get_speech_by_name(speech_name).cloned() {
            self.say(&speech)
        } else {
            false
        }
    }

    pub fn is_on(&self) -> Bool {
        self.on
    }

    /// Add a dialog event
    pub fn add_dialog_event(
        &mut self,
        event_rts: &AudioEventRts,
        speech: Option<&Speech>,
        return_event: Option<&mut AudioEventRts>,
    ) {
        let use_speech = if let Some(s) = speech {
            s.clone()
        } else if let Some(s) = self.get_speech_by_name(event_rts.get_event_name()) {
            s.clone()
        } else {
            return;
        };

        self.say(&use_speech);

        if let Some(ret_event) = return_event {
            *ret_event = event_rts.clone();
        }
    }

    /// Remove a dialog event
    pub fn remove_dialog_event(
        &mut self,
        event_rts: &AudioEventRts,
        event_to_use: Option<&mut AudioEventRts>,
    ) {
        let use_speech = if let Some(s) = self.get_speech_by_name(event_rts.get_event_name()) {
            s.clone()
        } else {
            return;
        };

        if let Some(event) = event_to_use {
            *event = event_rts.clone();
        }

        // Cancel the speech from all speakers
        for speaker in &mut self.speakers {
            speaker.cancel(&use_speech);
        }
    }

    /// Get filename for playing from an audio event
    pub fn get_filename_for_play_from_audio_event(
        &self,
        event_to_get_from: &AudioEventRts,
    ) -> AsciiString {
        if event_to_get_from.get_event_name().is_empty() {
            return String::new();
        }

        if let Some(speech) = self.get_speech_by_name(event_to_get_from.get_event_name()) {
            self.get_filename_for_play(speech)
        } else {
            String::new()
        }
    }

    /// Get filename for playing a specific speech
    pub fn get_filename_for_play(&self, speech: &Speech) -> AsciiString {
        let speech_info = &speech.info;

        let regular_samples = speech_info.dialog_files.len();
        let evening_samples = speech_info.dialog_files_evening.len();
        let morning_samples = speech_info.dialog_files_morning.len();
        let night_samples = speech_info.dialog_files_night.len();
        let num_samples = regular_samples + evening_samples + morning_samples + night_samples;

        if num_samples == 0 {
            return String::new();
        }

        // Select a random sample
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let sound_to_play = rng.gen_range(0..num_samples);

        let filename = if sound_to_play < regular_samples {
            &speech_info.dialog_files[sound_to_play]
        } else if sound_to_play < regular_samples + evening_samples {
            &speech_info.dialog_files_evening[sound_to_play - regular_samples]
        } else if sound_to_play < regular_samples + evening_samples + morning_samples {
            &speech_info.dialog_files_morning[sound_to_play - regular_samples - evening_samples]
        } else {
            &speech_info.dialog_files_night
                [sound_to_play - regular_samples - evening_samples - morning_samples]
        };

        // Generate full path
        let localized = filename.starts_with('$');
        let clean_filename = if localized { &filename[1..] } else { filename };

        if clean_filename.is_empty() {
            return String::new();
        }

        let local_dir = if localized { "english\\" } else { "" };
        format!(
            "{}\\{}{}.{}",
            BASE_DLG_DIR, local_dir, clean_filename, BASE_DLG_EXT
        )
    }
}

impl Default for SpeechManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Factory function to create a speech interface
pub fn create_speech_interface() -> Box<SpeechManager> {
    Box::new(SpeechManager::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speech_info_creation() {
        let speech_info = SpeechInfo::new();
        assert_eq!(speech_info.volume, 0.5);
        assert_eq!(speech_info.priority, 0);
        assert!(!speech_info.interruptable);
    }

    #[test]
    fn test_speech_creation() {
        let speech_info = SpeechInfo {
            dialog_event: "test_dialog".to_string(),
            volume: 0.8,
            priority: 2,
            interruptable: true,
            ..SpeechInfo::default()
        };

        let speech = Speech::from_speech_info(&speech_info);
        assert_eq!(speech.name, "test_dialog");
        assert_eq!(speech.volume, 0.8);
        assert_eq!(speech.priority, 2);
        assert_eq!(speech.interrupt, 1);
        assert!(speech.valid);
    }

    #[test]
    fn test_speaker_creation() {
        let mut speaker = Speaker::new();
        speaker.init("TestSpeaker", 1, 3000, 300);

        assert_eq!(speaker.name, "TestSpeaker");
        assert_eq!(speaker.priority, 1);
        assert_eq!(speaker.buffer_time, 3000);
        assert_eq!(speaker.delay_time, 300);
    }

    #[test]
    fn test_speech_manager_basic_operations() {
        let mut manager = SpeechManager::new();
        assert!(manager.init());

        assert_eq!(manager.speakers.len(), NUM_DLG_PRIORITIES);
        assert!(manager.is_on());

        let speech_info = SpeechInfo {
            dialog_event: "test_speech".to_string(),
            volume: 0.7,
            priority: 1,
            ..SpeechInfo::default()
        };

        let speech = manager.add_new_speech(&speech_info);
        assert!(speech.is_some());
        assert_eq!(speech.unwrap().name, "test_speech");

        assert_eq!(manager.num_speeches(), 1);
        assert!(manager.get_speech_by_name("test_speech").is_some());
    }
}
