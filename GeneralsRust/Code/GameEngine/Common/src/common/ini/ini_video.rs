////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_video.rs
//! Author: John McDonald, February 2002 (Converted to Rust)
//! Desc:   Parsing Video INI entries

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::common::ascii_string::AsciiString;
/// Result type for video parsing operations
pub type VideoResult<T> = Result<T, VideoError>;

/// Errors that can occur during video parsing
#[derive(Debug, Clone, PartialEq)]
pub enum VideoError {
    InvalidName,
    InvalidPath,
    ParseError(String),
    PlayerError(String),
    NotFound,
    AlreadyExists,
}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoError::InvalidName => write!(f, "Invalid video name"),
            VideoError::InvalidPath => write!(f, "Invalid video path"),
            VideoError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            VideoError::PlayerError(msg) => write!(f, "Video player error: {}", msg),
            VideoError::NotFound => write!(f, "Video not found"),
            VideoError::AlreadyExists => write!(f, "Video already exists"),
        }
    }
}

impl std::error::Error for VideoError {}

/// Video playback modes
#[derive(Debug, Clone, PartialEq)]
pub enum VideoPlaybackMode {
    Fullscreen,
    Windowed,
    InGame,
    Cutscene,
    Background,
    Custom(String),
}

impl VideoPlaybackMode {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "fullscreen" => Self::Fullscreen,
            "windowed" => Self::Windowed,
            "ingame" => Self::InGame,
            "cutscene" => Self::Cutscene,
            "background" => Self::Background,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Fullscreen => "Fullscreen",
            Self::Windowed => "Windowed",
            Self::InGame => "InGame",
            Self::Cutscene => "Cutscene",
            Self::Background => "Background",
            Self::Custom(name) => name,
        }
    }
}

/// Video codec formats
#[derive(Debug, Clone, PartialEq)]
pub enum VideoCodec {
    MPEG,
    AVI,
    WMV,
    MOV,
    MP4,
    BIK,     // Bink video (common in games)
    SMACKER, // Smacker video
    Custom(String),
}

impl VideoCodec {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "mpg" | "mpeg" => Self::MPEG,
            "avi" => Self::AVI,
            "wmv" => Self::WMV,
            "mov" => Self::MOV,
            "mp4" => Self::MP4,
            "bik" => Self::BIK,
            "smk" => Self::SMACKER,
            _ => Self::Custom(ext.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::MPEG => "MPEG",
            Self::AVI => "AVI",
            Self::WMV => "WMV",
            Self::MOV => "MOV",
            Self::MP4 => "MP4",
            Self::BIK => "BIK",
            Self::SMACKER => "SMACKER",
            Self::Custom(name) => name,
        }
    }
}

/// Video definition
#[derive(Debug, Clone)]
pub struct Video {
    pub internal_name: AsciiString,
    pub display_name: AsciiString,
    pub file_path: AsciiString,
    pub description: AsciiString,
    pub codec: VideoCodec,
    pub playback_mode: VideoPlaybackMode,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f32,
    pub duration: f32, // In seconds
    pub is_looping: bool,
    pub can_skip: bool,
    pub auto_start: bool,
    pub volume: f32, // 0.0 to 1.0
    pub subtitle_file: AsciiString,
    pub trigger_events: Vec<AsciiString>,
    pub properties: HashMap<String, String>,
}

impl Video {
    pub fn new(internal_name: AsciiString) -> Self {
        Self {
            internal_name,
            display_name: AsciiString::from(""),
            file_path: AsciiString::from(""),
            description: AsciiString::from(""),
            codec: VideoCodec::MPEG,
            playback_mode: VideoPlaybackMode::Fullscreen,
            width: 640,
            height: 480,
            frame_rate: 30.0,
            duration: 0.0,
            is_looping: false,
            can_skip: true,
            auto_start: false,
            volume: 1.0,
            subtitle_file: AsciiString::from(""),
            trigger_events: Vec::new(),
            properties: HashMap::new(),
        }
    }

    /// Get the field parse table for this video
    pub fn get_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        vec![
            ("DisplayName", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("FilePath", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("Description", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("PlaybackMode", |value| {
                Ok(Box::new(VideoPlaybackMode::from_string(value)) as Box<dyn std::any::Any>)
            }),
            ("Width", |value| {
                value
                    .parse::<u32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse width: {}", e))
            }),
            ("Height", |value| {
                value
                    .parse::<u32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse height: {}", e))
            }),
            ("FrameRate", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse frame rate: {}", e))
            }),
            ("Duration", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse duration: {}", e))
            }),
            ("IsLooping", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse looping: {}", e))
            }),
            ("CanSkip", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse can skip: {}", e))
            }),
            ("AutoStart", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse auto start: {}", e))
            }),
            ("Volume", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v.clamp(0.0, 1.0)) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse volume: {}", e))
            }),
            ("SubtitleFile", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("TriggerEvents", |value| {
                let events: Vec<AsciiString> = value
                    .split_whitespace()
                    .map(|s| AsciiString::from(s))
                    .collect();
                Ok(Box::new(events) as Box<dyn std::any::Any>)
            }),
        ]
    }

    /// Update video from properties
    pub fn update_from_properties(&mut self, properties: &HashMap<String, String>) {
        for (key, value) in properties {
            match key.as_str() {
                "DisplayName" => {
                    self.display_name = AsciiString::from(value);
                }
                "FilePath" => {
                    self.file_path = AsciiString::from(value);
                    // Auto-detect codec from file extension
                    if let Some(ext) = self.file_path.as_str().split('.').last() {
                        self.codec = VideoCodec::from_extension(ext);
                    }
                }
                "Description" => {
                    self.description = AsciiString::from(value);
                }
                "PlaybackMode" => {
                    self.playback_mode = VideoPlaybackMode::from_string(value);
                }
                "Width" => {
                    if let Ok(width) = value.parse::<u32>() {
                        self.width = width;
                    }
                }
                "Height" => {
                    if let Ok(height) = value.parse::<u32>() {
                        self.height = height;
                    }
                }
                "FrameRate" => {
                    if let Ok(rate) = value.parse::<f32>() {
                        self.frame_rate = rate;
                    }
                }
                "Duration" => {
                    if let Ok(duration) = value.parse::<f32>() {
                        self.duration = duration;
                    }
                }
                "IsLooping" => {
                    if let Ok(looping) = parse_bool(value) {
                        self.is_looping = looping;
                    }
                }
                "CanSkip" => {
                    if let Ok(can_skip) = parse_bool(value) {
                        self.can_skip = can_skip;
                    }
                }
                "AutoStart" => {
                    if let Ok(auto_start) = parse_bool(value) {
                        self.auto_start = auto_start;
                    }
                }
                "Volume" => {
                    if let Ok(volume) = value.parse::<f32>() {
                        self.volume = volume.clamp(0.0, 1.0);
                    }
                }
                "SubtitleFile" => {
                    self.subtitle_file = AsciiString::from(value);
                }
                "TriggerEvents" => {
                    self.trigger_events = value
                        .split_whitespace()
                        .map(|s| AsciiString::from(s))
                        .collect();
                }
                _ => {
                    // Store unknown properties
                    self.properties.insert(key.clone(), value.clone());
                }
            }
        }
    }

    pub fn get_internal_name(&self) -> &AsciiString {
        &self.internal_name
    }

    pub fn is_valid(&self) -> bool {
        !self.internal_name.is_empty() && !self.file_path.is_empty()
    }

    pub fn get_aspect_ratio(&self) -> f32 {
        if self.height > 0 {
            self.width as f32 / self.height as f32
        } else {
            4.0 / 3.0 // Default aspect ratio
        }
    }

    pub fn has_audio(&self) -> bool {
        self.volume > 0.0
    }

    pub fn has_subtitles(&self) -> bool {
        !self.subtitle_file.is_empty()
    }

    pub fn should_trigger_event(&self, event: &AsciiString) -> bool {
        self.trigger_events.contains(event)
    }
}

/// Video player - manages and plays video files
#[derive(Debug)]
pub struct VideoPlayer {
    videos: HashMap<String, Video>,
    video_order: Vec<String>,
    current_video: Option<String>,
    is_playing: bool,
    is_paused: bool,
    playback_position: f32, // Current position in seconds
}

impl VideoPlayer {
    pub fn new() -> Self {
        Self {
            videos: HashMap::new(),
            video_order: Vec::new(),
            current_video: None,
            is_playing: false,
            is_paused: false,
            playback_position: 0.0,
        }
    }

    /// Get the field parse table for video definitions
    pub fn get_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        // Return a generic field parse table that can be used by any Video instance
        vec![
            ("DisplayName", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("FilePath", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
        ]
    }

    /// Add a video to the player
    pub fn add_video(&mut self, video: Video) {
        let name = video.internal_name.as_str().to_string();
        if !self.videos.contains_key(&name) {
            self.video_order.push(name.clone());
        }
        self.videos.insert(name, video);
    }

    /// Find a video by name
    pub fn find_video(&self, name: &AsciiString) -> Option<&Video> {
        self.videos.get(name.as_str())
    }

    /// Find a mutable video by name
    pub fn find_video_mut(&mut self, name: &AsciiString) -> Option<&mut Video> {
        self.videos.get_mut(name.as_str())
    }

    /// Play a video
    pub fn play_video(&mut self, name: &AsciiString) -> VideoResult<()> {
        let name_string = name.as_str().to_string();

        // Check video validity first without borrowing
        let video_exists_and_valid = if let Some(video) = self.find_video(name) {
            video.is_valid()
        } else {
            return Err(VideoError::NotFound);
        };

        if !video_exists_and_valid {
            return Err(VideoError::InvalidPath);
        }

        self.current_video = Some(name_string);
        self.is_playing = true;
        self.is_paused = false;
        self.playback_position = 0.0;

        if let Some(video) = self.find_video(name) {
            println!(
                "Playing video: {} ({})",
                video.display_name.as_str(),
                video.file_path.as_str()
            );
        }
        Ok(())
    }

    /// Stop current video
    pub fn stop_video(&mut self) {
        self.current_video = None;
        self.is_playing = false;
        self.is_paused = false;
        self.playback_position = 0.0;
    }

    /// Pause current video
    pub fn pause_video(&mut self) {
        if self.is_playing {
            self.is_paused = true;
        }
    }

    /// Resume current video
    pub fn resume_video(&mut self) {
        if self.is_playing && self.is_paused {
            self.is_paused = false;
        }
    }

    /// Get current video
    pub fn get_current_video(&self) -> Option<&Video> {
        if let Some(ref name) = self.current_video {
            self.videos.get(name)
        } else {
            None
        }
    }

    /// Check if a video is currently playing
    pub fn is_playing(&self) -> bool {
        self.is_playing && !self.is_paused
    }

    /// Get all video names
    pub fn get_video_names(&self) -> Vec<&String> {
        self.video_order
            .iter()
            .filter(|name| self.videos.contains_key(*name))
            .collect()
    }

    /// Get videos by playback mode
    pub fn get_videos_by_mode(&self, mode: &VideoPlaybackMode) -> Vec<&Video> {
        self.video_order
            .iter()
            .filter_map(|name| self.videos.get(name))
            .filter(|v| &v.playback_mode == mode)
            .collect()
    }

    /// Remove a video
    pub fn remove_video(&mut self, name: &AsciiString) -> bool {
        // Stop video if it's currently playing
        if let Some(ref current) = self.current_video {
            if current == name.as_str() {
                self.stop_video();
            }
        }

        let removed = self.videos.remove(name.as_str()).is_some();
        if removed {
            self.video_order
                .retain(|existing| existing != name.as_str());
        }
        removed
    }

    /// Clear all videos
    pub fn clear(&mut self) {
        self.stop_video();
        self.videos.clear();
        self.video_order.clear();
    }

    /// Get video count
    pub fn get_video_count(&self) -> usize {
        self.videos.len()
    }

    /// Update playback position (called by game loop)
    pub fn update(&mut self, delta_time: f32) {
        if self.is_playing && !self.is_paused {
            self.playback_position += delta_time;

            // Check if video has ended
            if let Some(video) = self.get_current_video() {
                if self.playback_position >= video.duration && video.duration > 0.0 {
                    if video.is_looping {
                        self.playback_position = 0.0;
                    } else {
                        self.stop_video();
                    }
                }
            }
        }
    }
}

impl Default for VideoPlayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Global video player instance
static VIDEO_PLAYER: OnceCell<Arc<Mutex<VideoPlayer>>> = OnceCell::new();

/// Initialize the global video player
pub fn initialize_video_player() {
    if VIDEO_PLAYER.get().is_none() {
        let _ = VIDEO_PLAYER.set(Arc::new(Mutex::new(VideoPlayer::new())));
    } else if let Some(player) = VIDEO_PLAYER.get() {
        if let Ok(mut guard) = player.lock() {
            *guard = VideoPlayer::new();
        }
    }
}

/// Get a reference to the global video player
pub fn get_video_player() -> Option<MutexGuard<'static, VideoPlayer>> {
    VIDEO_PLAYER
        .get()
        .map(|player| player.lock().expect("VideoPlayer mutex poisoned"))
}

/// Parse a boolean value from string
pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

/// INI parsing functions for videos
pub struct IniVideo;

impl IniVideo {
    /// Parse video definition - equivalent to INI::parseVideoDefinition
    pub fn parse_video_definition(internal_name: AsciiString) -> VideoResult<()> {
        // Validate name
        if internal_name.is_empty() {
            return Err(VideoError::InvalidName);
        }

        // Initialize video player if needed
        initialize_video_player();

        // Create video
        let video = Video::new(internal_name.clone());

        // Add to player
        if let Some(mut player) = get_video_player() {
            player.add_video(video);
            println!("Parsing video definition for: {}", internal_name.as_str());
        } else {
            return Err(VideoError::PlayerError(
                "Video player not initialized".to_string(),
            ));
        }

        // In the original C++, this would call:
        // ini->initFromINI(&video, TheVideoPlayer->getFieldParse());
        // TheVideoPlayer->addVideo(&video);

        Ok(())
    }

    /// Parse a complete video block from INI data
    pub fn parse_video_block(
        internal_name: AsciiString,
        properties: HashMap<String, String>,
    ) -> VideoResult<Video> {
        // Validate name
        if internal_name.is_empty() {
            return Err(VideoError::InvalidName);
        }

        // Create video
        let mut video = Video::new(internal_name);

        // Update video from properties
        video.update_from_properties(&properties);

        // Validate video
        if !video.is_valid() {
            return Err(VideoError::ParseError(
                "Invalid video configuration".to_string(),
            ));
        }

        Ok(video)
    }

    /// Register a video
    pub fn register_video(video: Video) -> VideoResult<()> {
        initialize_video_player();

        let mut player = get_video_player()
            .ok_or_else(|| VideoError::PlayerError("Player not initialized".to_string()))?;

        player.add_video(video);
        Ok(())
    }

    /// Find a video by name
    pub fn find_video_by_name(name: &AsciiString) -> Option<Video> {
        if let Some(player) = get_video_player() {
            player.find_video(name).cloned()
        } else {
            None
        }
    }

    /// Play a video
    pub fn play_video(name: &AsciiString) -> VideoResult<()> {
        initialize_video_player();

        let mut player = get_video_player()
            .ok_or_else(|| VideoError::PlayerError("Player not initialized".to_string()))?;

        player.play_video(name)
    }

    /// Validate video name format
    pub fn validate_name(name: &AsciiString) -> bool {
        !name.is_empty() && name.len() < 128 // Reasonable length limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_playback_mode_parsing() {
        assert_eq!(
            VideoPlaybackMode::from_string("fullscreen"),
            VideoPlaybackMode::Fullscreen
        );
        assert_eq!(
            VideoPlaybackMode::from_string("CUTSCENE"),
            VideoPlaybackMode::Cutscene
        );
        assert_eq!(
            VideoPlaybackMode::from_string("CustomMode"),
            VideoPlaybackMode::Custom("CustomMode".to_string())
        );
    }

    #[test]
    fn test_video_codec_detection() {
        assert_eq!(VideoCodec::from_extension("avi"), VideoCodec::AVI);
        assert_eq!(VideoCodec::from_extension("BIK"), VideoCodec::BIK);
        assert_eq!(
            VideoCodec::from_extension("unknown"),
            VideoCodec::Custom("unknown".to_string())
        );
    }

    #[test]
    fn test_video_creation() {
        let name = AsciiString::from("TestVideo");
        let video = Video::new(name.clone());

        assert_eq!(video.internal_name, name);
        assert_eq!(video.width, 640);
        assert_eq!(video.height, 480);
        assert!(!video.is_looping);
        assert!(video.can_skip);
        assert!(!video.file_path.is_empty() || !video.is_valid()); // Should be invalid without file path
    }

    #[test]
    fn test_video_player() {
        let mut player = VideoPlayer::new();
        let name = AsciiString::from("TestVideo");

        // Create and add video
        let mut video = Video::new(name.clone());
        video.file_path = AsciiString::from("test.avi");
        video.duration = 60.0;
        video.display_name = AsciiString::from("Test Video");

        player.add_video(video);

        // Find video
        let found = player.find_video(&name);
        assert!(found.is_some());
        assert_eq!(found.unwrap().duration, 60.0);

        // Play video
        let result = player.play_video(&name);
        assert!(result.is_ok());
        assert!(player.is_playing());
        assert!(!player.is_paused);

        // Pause and resume
        player.pause_video();
        assert!(!player.is_playing());
        player.resume_video();
        assert!(player.is_playing());

        // Count videos
        assert_eq!(player.get_video_count(), 1);
    }

    #[test]
    fn test_video_player_preserves_cpp_vector_order() {
        let mut player = VideoPlayer::new();
        player.add_video(Video::new(AsciiString::from("Intro")));
        player.add_video(Video::new(AsciiString::from("Campaign")));
        player.add_video(Video::new(AsciiString::from("Credits")));

        assert_eq!(
            player
                .get_video_names()
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["Intro", "Campaign", "Credits"]
        );
    }

    #[test]
    fn test_video_player_replaces_duplicate_in_place() {
        let mut player = VideoPlayer::new();
        player.add_video(Video::new(AsciiString::from("Intro")));
        let mut replacement = Video::new(AsciiString::from("Intro"));
        replacement.description = AsciiString::from("replacement");
        player.add_video(replacement);
        player.add_video(Video::new(AsciiString::from("Credits")));

        assert_eq!(player.get_video_count(), 2);
        assert_eq!(
            player
                .get_video_names()
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["Intro", "Credits"]
        );
        assert_eq!(
            player
                .find_video(&AsciiString::from("Intro"))
                .expect("video exists")
                .description
                .as_str(),
            "replacement"
        );
    }

    #[test]
    fn test_video_player_mode_filter_preserves_cpp_vector_order() {
        let mut player = VideoPlayer::new();
        let mut intro = Video::new(AsciiString::from("Intro"));
        intro.playback_mode = VideoPlaybackMode::Fullscreen;
        let mut menu = Video::new(AsciiString::from("Menu"));
        menu.playback_mode = VideoPlaybackMode::Windowed;
        let mut credits = Video::new(AsciiString::from("Credits"));
        credits.playback_mode = VideoPlaybackMode::Fullscreen;

        player.add_video(intro);
        player.add_video(menu);
        player.add_video(credits);

        assert_eq!(
            player
                .get_videos_by_mode(&VideoPlaybackMode::Fullscreen)
                .into_iter()
                .map(|video| video.internal_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Intro", "Credits"]
        );
    }

    #[test]
    fn test_video_properties_update() {
        let mut video = Video::new(AsciiString::from("Test"));
        let mut properties = HashMap::new();
        properties.insert("FilePath".to_string(), "test.mp4".to_string());
        properties.insert("Width".to_string(), "1920".to_string());
        properties.insert("Height".to_string(), "1080".to_string());
        properties.insert("IsLooping".to_string(), "true".to_string());
        properties.insert("Volume".to_string(), "0.8".to_string());

        video.update_from_properties(&properties);

        assert_eq!(video.file_path.as_str(), "test.mp4");
        assert_eq!(video.width, 1920);
        assert_eq!(video.height, 1080);
        assert!(video.is_looping);
        assert_eq!(video.volume, 0.8);
        assert!(matches!(video.codec, VideoCodec::MP4));
    }

    #[test]
    fn test_video_aspect_ratio() {
        let mut video = Video::new(AsciiString::from("Test"));
        video.width = 1920;
        video.height = 1080;

        let ratio = video.get_aspect_ratio();
        assert!((ratio - (16.0 / 9.0)).abs() < 0.01);
    }

    #[test]
    fn test_video_events() {
        let mut video = Video::new(AsciiString::from("Test"));
        video.trigger_events = vec![
            AsciiString::from("CUTSCENE_START"),
            AsciiString::from("MISSION_INTRO"),
        ];

        assert!(video.should_trigger_event(&AsciiString::from("CUTSCENE_START")));
        assert!(!video.should_trigger_event(&AsciiString::from("UNKNOWN_EVENT")));
    }

    #[test]
    fn test_video_update_loop() {
        let mut player = VideoPlayer::new();
        let name = AsciiString::from("LoopingVideo");

        let mut video = Video::new(name.clone());
        video.file_path = AsciiString::from("loop.avi");
        video.duration = 5.0;
        video.is_looping = true;

        player.add_video(video);
        player.play_video(&name).unwrap();

        // Simulate time passing
        player.update(6.0); // More than duration

        // Should still be playing due to looping
        assert!(player.is_playing());
        assert!(player.playback_position < 5.0); // Should have reset
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("true"), Ok(true));
        assert_eq!(parse_bool("TRUE"), Ok(true));
        assert_eq!(parse_bool("yes"), Ok(true));
        assert_eq!(parse_bool("1"), Ok(true));

        assert_eq!(parse_bool("false"), Ok(false));
        assert_eq!(parse_bool("FALSE"), Ok(false));
        assert_eq!(parse_bool("no"), Ok(false));
        assert_eq!(parse_bool("0"), Ok(false));

        assert!(parse_bool("invalid").is_err());
    }

    #[test]
    fn test_validate_name() {
        assert!(IniVideo::validate_name(&AsciiString::from("ValidName")));
        assert!(!IniVideo::validate_name(&AsciiString::from("")));
    }
}
