//
// Project:    Generals
//
// File name:  GameClient/video_player.rs
//
// Created:    Ported from C++
//
// Description: Video player interface and implementation for video playback
//
// Original C++ source:
//   /GeneralsMD/Code/GameEngine/Include/GameClient/VideoPlayer.h
//   /GeneralsMD/Code/GameEngine/Source/GameClient/VideoPlayer.cpp
//
//----------------------------------------------------------------------------

use crate::GameClient::video_stream::{VideoStream, VideoStreamInterface};
use base_types::{Bool, Int};
use std::sync::{Arc, Mutex};

//----------------------------------------------------------------------------
// Video Struct
//----------------------------------------------------------------------------

/// Video metadata structure
///
/// Matches C++ VideoPlayer.h Video struct (lines 43-48)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Video {
    /// Filename on disk
    pub filename: String,

    /// Internal reference name
    pub internal_name: String,

    /// Comment for WorldBuilder
    pub comment_for_wb: String,
}

impl Video {
    /// Create a new Video metadata entry
    pub fn new(filename: String, internal_name: String, comment_for_wb: String) -> Self {
        Video {
            filename,
            internal_name,
            comment_for_wb,
        }
    }

    /// Create an empty Video
    pub fn empty() -> Self {
        Video {
            filename: String::new(),
            internal_name: String::new(),
            comment_for_wb: String::new(),
        }
    }
}

//----------------------------------------------------------------------------
// FieldParse
//----------------------------------------------------------------------------

/// Field parse information for INI parsing
///
/// Matches C++ VideoPlayer.cpp m_videoFieldParseTable (lines 474-479)
#[derive(Debug, Clone)]
pub struct FieldParse {
    /// Field name in INI file
    pub field_name: &'static str,

    /// Parse function type (represented as string for now)
    pub parse_type: &'static str,

    /// Additional data (unused, for C++ compatibility)
    pub data: usize,

    /// Offset into Video struct
    pub offset: usize,
}

/// Video field parse table
///
/// Matches C++ VideoPlayer.cpp m_videoFieldParseTable
pub const VIDEO_FIELD_PARSE_TABLE: &[FieldParse] = &[
    FieldParse {
        field_name: "Filename",
        parse_type: "AsciiString",
        data: 0,
        offset: 0, // offset to m_filename
    },
    FieldParse {
        field_name: "Comment",
        parse_type: "AsciiString",
        data: 0,
        offset: 0, // offset to m_commentForWB
    },
];

//----------------------------------------------------------------------------
// VideoPlayerInterface Trait
//----------------------------------------------------------------------------

/// Interface for video playback
///
/// Matches C++ VideoPlayer.h VideoPlayerInterface (lines 193-225)
pub trait VideoPlayerInterface {
    /// Initialize video playback
    ///
    /// Matches C++ VideoPlayerInterface::init
    fn init(&mut self);

    /// Reset video playback
    ///
    /// Matches C++ VideoPlayerInterface::reset
    fn reset(&mut self);

    /// Services all video tasks. Should be called frequently
    ///
    /// Matches C++ VideoPlayerInterface::update
    fn update(&mut self);

    /// Close down player
    ///
    /// Matches C++ VideoPlayerInterface::deinit
    fn deinit(&mut self);

    /// Should be called when application loses focus
    ///
    /// Matches C++ VideoPlayerInterface::loseFocus
    fn lose_focus(&mut self);

    /// Should be called when application regains focus
    ///
    /// Matches C++ VideoPlayerInterface::regainFocus
    fn regain_focus(&mut self);

    /// Open video file for playback
    ///
    /// Matches C++ VideoPlayerInterface::open
    fn open(&mut self, movie_title: String) -> Option<Box<dyn VideoStreamInterface>>;

    /// Load video file into memory for playback
    ///
    /// Matches C++ VideoPlayerInterface::load
    fn load(&mut self, movie_title: String) -> Option<Box<dyn VideoStreamInterface>>;

    /// Return the first open/loaded video stream
    ///
    /// Matches C++ VideoPlayerInterface::firstStream
    fn first_stream(&self) -> Option<&dyn VideoStreamInterface>;

    /// Return the first open/loaded video stream (mutable)
    ///
    /// Matches C++ VideoPlayerInterface::firstStream
    fn first_stream_mut(&mut self) -> Option<&mut dyn VideoStreamInterface>;

    /// Close all open streams
    ///
    /// Matches C++ VideoPlayerInterface::closeAllStreams
    fn close_all_streams(&mut self);

    /// Add a video to the list of videos we can play
    ///
    /// Matches C++ VideoPlayerInterface::addVideo
    fn add_video(&mut self, video: Video);

    /// Remove a video from the list of videos we can play
    ///
    /// Matches C++ VideoPlayerInterface::removeVideo
    fn remove_video(&mut self, internal_name: &str);

    /// Retrieve info about the number of videos currently listed
    ///
    /// Matches C++ VideoPlayerInterface::getNumVideos
    fn get_num_videos(&self) -> Int;

    /// Retrieve info about a movie based on internal name
    ///
    /// Matches C++ VideoPlayerInterface::getVideo
    fn get_video_by_name(&self, movie_title: &str) -> Option<&Video>;

    /// Retrieve info about a movie based on index
    ///
    /// Matches C++ VideoPlayerInterface::getVideo
    fn get_video_by_index(&self, index: Int) -> Option<&Video>;

    /// Return the field parse info
    ///
    /// Matches C++ VideoPlayerInterface::getFieldParse
    fn get_field_parse(&self) -> &'static [FieldParse] {
        VIDEO_FIELD_PARSE_TABLE
    }

    /// Notify the video player that they can now ask for an audio handle,
    /// or they need to give theirs up.
    ///
    /// Matches C++ VideoPlayerInterface::notifyVideoPlayerOfNewProvider
    fn notify_video_player_of_new_provider(&mut self, now_has_valid: Bool);
}

//----------------------------------------------------------------------------
// VideoPlayer - Implementation
//----------------------------------------------------------------------------

/// Common video playback code
///
/// Matches C++ VideoPlayer.h VideoPlayer class (lines 236-277)
pub struct VideoPlayer {
    /// List of videos available for playback
    ///
    /// Matches C++ VideoPlayer::mVideosAvailableForPlay
    videos_available_for_play: Vec<Video>,

    /// First stream in the linked list of open streams
    ///
    /// Matches C++ VideoPlayer::m_firstStream
    first_stream: Option<Box<VideoStream>>,
}

impl VideoPlayer {
    /// Create a new VideoPlayer
    ///
    /// Matches C++ VideoPlayer::VideoPlayer constructor (lines 125-128)
    pub fn new() -> Self {
        VideoPlayer {
            videos_available_for_play: Vec::new(),
            first_stream: None,
        }
    }

    /// Remove a stream from the active list
    ///
    /// Matches C++ VideoPlayer::remove (lines 254-276)
    ///
    /// This is called by VideoStream destructor to remove itself from the list
    pub fn remove(&mut self, stream_to_remove: *const VideoStream) {
        if self.first_stream.is_none() {
            return;
        }

        // Check if first stream is the one to remove
        if let Some(ref first) = self.first_stream {
            let first_ptr = first.as_ref() as *const VideoStream;
            if first_ptr == stream_to_remove {
                // Remove first stream
                let mut old_first = self.first_stream.take().unwrap();
                self.first_stream = old_first.take_next();
                return;
            }
        }

        // Search through the list
        let mut current = self.first_stream.as_mut();

        while let Some(stream) = current {
            if let Some(next) = stream.get_next() {
                let next_ptr = next as *const VideoStream;
                if next_ptr == stream_to_remove {
                    // Remove next stream
                    let removed = stream.take_next().unwrap();
                    stream.set_next(removed.get_next().map(|_| stream.take_next().unwrap()));
                    return;
                }
            }

            current = stream.get_next_mut();
        }
    }
}

impl Default for VideoPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoPlayerInterface for VideoPlayer {
    /// Initialize video playback code
    ///
    /// Matches C++ VideoPlayer::init (lines 148-155)
    fn init(&mut self) {
        // Load video configuration from INI files
        // In C++, this loads Data\INI\Default\Video.ini and Data\INI\Video.ini
        // For now, we have a placeholder that can be extended with actual INI loading
    }

    /// Reset video playback
    ///
    /// Matches C++ VideoPlayer::reset (lines 169-172)
    fn reset(&mut self) {
        self.close_all_streams();
    }

    /// Services all audio tasks. Should be called frequently
    ///
    /// Matches C++ VideoPlayer::update (lines 178-189)
    fn update(&mut self) {
        // Update all streams in the linked list
        let mut current = self.first_stream.as_mut();

        while let Some(stream) = current {
            stream.update();
            current = stream.get_next_mut();
        }
    }

    /// Close down player
    ///
    /// Matches C++ VideoPlayer::deinit (lines 161-163)
    fn deinit(&mut self) {
        // Default implementation is empty
    }

    /// Should be called when application loses focus
    ///
    /// Matches C++ VideoPlayer::loseFocus (lines 195-198)
    fn lose_focus(&mut self) {
        // Default implementation is empty
    }

    /// Should be called when application regains focus
    ///
    /// Matches C++ VideoPlayer::regainFocus (lines 203-206)
    fn regain_focus(&mut self) {
        // Default implementation is empty
    }

    /// Open video file for playback
    ///
    /// Matches C++ VideoPlayer::open (lines 213-216)
    ///
    /// Returns None in base implementation (should be overridden by subclasses)
    fn open(&mut self, _movie_title: String) -> Option<Box<dyn VideoStreamInterface>> {
        None
    }

    /// Load video file into memory for playback
    ///
    /// Matches C++ VideoPlayer::load (lines 222-225)
    ///
    /// Returns None in base implementation (should be overridden by subclasses)
    fn load(&mut self, _movie_title: String) -> Option<Box<dyn VideoStreamInterface>> {
        None
    }

    /// Return the first open/loaded video stream
    ///
    /// Matches C++ VideoPlayer::firstStream (lines 231-234)
    fn first_stream(&self) -> Option<&dyn VideoStreamInterface> {
        self.first_stream
            .as_deref()
            .map(|s| s as &dyn VideoStreamInterface)
    }

    /// Return the first open/loaded video stream (mutable)
    ///
    /// Matches C++ VideoPlayer::firstStream (lines 231-234)
    fn first_stream_mut(&mut self) -> Option<&mut dyn VideoStreamInterface> {
        self.first_stream
            .as_deref_mut()
            .map(|s| s as &mut dyn VideoStreamInterface)
    }

    /// Close all open streams
    ///
    /// Matches C++ VideoPlayer::closeAllStreams (lines 240-248)
    fn close_all_streams(&mut self) {
        while let Some(stream) = self.first_stream.take() {
            // Take the next stream before closing current
            self.first_stream = stream.get_next().map(|_| stream);
            // stream is dropped here, calling close
        }
    }

    /// Add a video to the list of videos we can play
    ///
    /// Matches C++ VideoPlayer::addVideo (lines 281-292)
    fn add_video(&mut self, video: Video) {
        // Check if video with same internal name already exists
        for existing in &mut self.videos_available_for_play {
            if existing.internal_name == video.internal_name {
                // Update existing entry
                *existing = video;
                return;
            }
        }

        // Add new entry
        self.videos_available_for_play.push(video);
    }

    /// Remove a video from the list of videos we can play
    ///
    /// Matches C++ VideoPlayer::removeVideo (lines 297-305)
    fn remove_video(&mut self, internal_name: &str) {
        self.videos_available_for_play
            .retain(|v| v.internal_name != internal_name);
    }

    /// Retrieve info about the number of videos currently listed
    ///
    /// Matches C++ VideoPlayer::getNumVideos (lines 310-313)
    fn get_num_videos(&self) -> Int {
        self.videos_available_for_play.len() as Int
    }

    /// Retrieve info about a movie based on internal name
    ///
    /// Matches C++ VideoPlayer::getVideo (lines 318-326)
    fn get_video_by_name(&self, movie_title: &str) -> Option<&Video> {
        self.videos_available_for_play
            .iter()
            .find(|v| v.internal_name == movie_title)
    }

    /// Retrieve info about a movie based on index
    ///
    /// Matches C++ VideoPlayer::getVideo (lines 331-338)
    fn get_video_by_index(&self, index: Int) -> Option<&Video> {
        if index < 0 || index >= self.videos_available_for_play.len() as Int {
            return None;
        }

        Some(&self.videos_available_for_play[index as usize])
    }

    /// Notify the video player that they can now ask for an audio handle,
    /// or they need to give theirs up.
    ///
    /// Matches C++ VideoPlayer::notifyVideoPlayerOfNewProvider (line 272)
    ///
    /// Default implementation does nothing
    fn notify_video_player_of_new_provider(&mut self, _now_has_valid: Bool) {
        // Default implementation is empty
    }
}

//----------------------------------------------------------------------------
// Global Video Player Instance
//----------------------------------------------------------------------------

use std::sync::OnceLock;

/// Global video player instance
///
/// Matches C++ VideoPlayer.cpp TheVideoPlayer (line 49)
///
/// In C++, this is a raw pointer. In Rust, we use Arc<Mutex<>> for thread-safe
/// shared ownership.
static THE_VIDEO_PLAYER: OnceLock<Arc<Mutex<Option<VideoPlayer>>>> = OnceLock::new();

/// Initialize the global video player
pub fn init_video_player() {
    THE_VIDEO_PLAYER.get_or_init(|| Arc::new(Mutex::new(Some(VideoPlayer::new()))));
}

/// Get a reference to the global video player
pub fn get_video_player() -> Option<Arc<Mutex<Option<VideoPlayer>>>> {
    THE_VIDEO_PLAYER.get().cloned()
}

/// Shutdown the global video player
pub fn shutdown_video_player() {
    if let Some(player) = THE_VIDEO_PLAYER.get() {
        let mut player_guard = player.lock().unwrap();
        *player_guard = None;
    }
}

//----------------------------------------------------------------------------
// Unit Tests
//----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_creation() {
        let video = Video::new(
            "test.bik".to_string(),
            "TestVideo".to_string(),
            "Test comment".to_string(),
        );

        assert_eq!(video.filename, "test.bik");
        assert_eq!(video.internal_name, "TestVideo");
        assert_eq!(video.comment_for_wb, "Test comment");
    }

    #[test]
    fn test_video_empty() {
        let video = Video::empty();
        assert!(video.filename.is_empty());
        assert!(video.internal_name.is_empty());
        assert!(video.comment_for_wb.is_empty());
    }

    #[test]
    fn test_video_player_creation() {
        let player = VideoPlayer::new();
        assert_eq!(player.get_num_videos(), 0);
        assert!(player.first_stream().is_none());
    }

    #[test]
    fn test_video_player_add_video() {
        let mut player = VideoPlayer::new();

        let video1 = Video::new(
            "intro.bik".to_string(),
            "Intro".to_string(),
            "Intro video".to_string(),
        );

        player.add_video(video1);
        assert_eq!(player.get_num_videos(), 1);

        let retrieved = player.get_video_by_name("Intro");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().filename, "intro.bik");
    }

    #[test]
    fn test_video_player_add_duplicate() {
        let mut player = VideoPlayer::new();

        let video1 = Video::new(
            "intro.bik".to_string(),
            "Intro".to_string(),
            "First".to_string(),
        );

        let video2 = Video::new(
            "intro2.bik".to_string(),
            "Intro".to_string(),
            "Second".to_string(),
        );

        player.add_video(video1);
        player.add_video(video2);

        // Should have only one video (replaced)
        assert_eq!(player.get_num_videos(), 1);

        let retrieved = player.get_video_by_name("Intro");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().filename, "intro2.bik");
        assert_eq!(retrieved.unwrap().comment_for_wb, "Second");
    }

    #[test]
    fn test_video_player_remove_video() {
        let mut player = VideoPlayer::new();

        let video = Video::new(
            "test.bik".to_string(),
            "Test".to_string(),
            "Comment".to_string(),
        );

        player.add_video(video);
        assert_eq!(player.get_num_videos(), 1);

        player.remove_video("Test");
        assert_eq!(player.get_num_videos(), 0);
        assert!(player.get_video_by_name("Test").is_none());
    }

    #[test]
    fn test_video_player_get_by_index() {
        let mut player = VideoPlayer::new();

        let video1 = Video::new("v1.bik".to_string(), "V1".to_string(), "".to_string());
        let video2 = Video::new("v2.bik".to_string(), "V2".to_string(), "".to_string());

        player.add_video(video1);
        player.add_video(video2);

        assert!(player.get_video_by_index(0).is_some());
        assert!(player.get_video_by_index(1).is_some());
        assert!(player.get_video_by_index(2).is_none());
        assert!(player.get_video_by_index(-1).is_none());
    }

    #[test]
    fn test_video_player_close_all_streams() {
        let mut player = VideoPlayer::new();

        // Add some streams to the player
        let stream1 = Box::new(VideoStream::new());
        let stream2 = Box::new(VideoStream::new());

        player.first_stream = Some(stream1);
        // In real usage, streams would be chained

        player.close_all_streams();
        assert!(player.first_stream().is_none());
    }

    #[test]
    fn test_video_player_init_reset() {
        let mut player = VideoPlayer::new();

        player.init();
        player.reset();

        // Should still be valid after reset
        assert_eq!(player.get_num_videos(), 0);
    }

    #[test]
    fn test_video_player_update() {
        let mut player = VideoPlayer::new();

        // Update should not panic even with no streams
        player.update();

        let stream = Box::new(VideoStream::new());
        player.first_stream = Some(stream);

        // Update should process the stream
        player.update();
    }

    #[test]
    fn test_video_player_focus() {
        let mut player = VideoPlayer::new();

        // These should not panic
        player.lose_focus();
        player.regain_focus();
    }

    #[test]
    fn test_video_player_open_load() {
        let mut player = VideoPlayer::new();

        // Base implementation returns None
        assert!(player.open("test.bik".to_string()).is_none());
        assert!(player.load("test.bik".to_string()).is_none());
    }

    #[test]
    fn test_field_parse_table() {
        assert_eq!(VIDEO_FIELD_PARSE_TABLE.len(), 2);
        assert_eq!(VIDEO_FIELD_PARSE_TABLE[0].field_name, "Filename");
        assert_eq!(VIDEO_FIELD_PARSE_TABLE[1].field_name, "Comment");
    }

    #[test]
    fn test_video_player_interface_get_field_parse() {
        let player = VideoPlayer::new();
        let parse_table = player.get_field_parse();
        assert_eq!(parse_table.len(), 2);
    }

    #[test]
    fn test_global_video_player() {
        init_video_player();

        {
            let player = get_video_player();
            assert!(player.is_some());
            let player_arc = player.unwrap();
            let player_guard = player_arc.lock().unwrap();
            assert!(player_guard.is_some());
        }

        shutdown_video_player();

        {
            let player = get_video_player();
            assert!(player.is_some());
            let player_arc = player.unwrap();
            let player_guard = player_arc.lock().unwrap();
            assert!(player_guard.is_none());
        }
    }
}
