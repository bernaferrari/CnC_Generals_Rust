//
// Project:    Generals
//
// File name:  GameClient/video_stream.rs
//
// Created:    Ported from C++
//
// Description: Video stream interface and implementation for video playback
//
// Original C++ source:
//   /GeneralsMD/Code/GameEngine/Include/GameClient/VideoPlayer.h
//   /GeneralsMD/Code/GameEngine/Source/GameClient/VideoPlayer.cpp (lines 342-471)
//
//----------------------------------------------------------------------------

use crate::GameClient::video_buffer::VideoBuffer;
use base_types::{Bool, Int};

//----------------------------------------------------------------------------
// VideoStreamInterface Trait
//----------------------------------------------------------------------------

/// Video stream interface trait
///
/// Matches C++ VideoPlayer.h VideoStreamInterface abstract class
pub trait VideoStreamInterface {
    /// Returns next open stream
    ///
    /// Matches C++ VideoStreamInterface::next
    fn next(&self) -> Option<&dyn VideoStreamInterface>;

    /// Returns next open stream (mutable)
    ///
    /// Matches C++ VideoStreamInterface::next
    fn next_mut(&mut self) -> Option<&mut dyn VideoStreamInterface>;

    /// Update stream
    ///
    /// Matches C++ VideoStreamInterface::update
    fn update(&mut self);

    /// Close and free stream
    ///
    /// Matches C++ VideoStreamInterface::close
    ///
    /// Note: In Rust, this consumes self rather than using delete
    fn close(self: Box<Self>);

    /// Is the frame ready to be displayed
    ///
    /// Matches C++ VideoStreamInterface::isFrameReady
    fn is_frame_ready(&self) -> Bool;

    /// Decompress current frame
    ///
    /// Matches C++ VideoStreamInterface::frameDecompress
    fn frame_decompress(&mut self);

    /// Render current frame into buffer
    ///
    /// Matches C++ VideoStreamInterface::frameRender
    fn frame_render(&mut self, buffer: &mut dyn VideoBuffer);

    /// Advance to next frame
    ///
    /// Matches C++ VideoStreamInterface::frameNext
    fn frame_next(&mut self);

    /// Returns zero based index of current frame
    ///
    /// Matches C++ VideoStreamInterface::frameIndex
    fn frame_index(&self) -> Int;

    /// Returns the total number of frames in the stream
    ///
    /// Matches C++ VideoStreamInterface::frameCount
    fn frame_count(&self) -> Int;

    /// Go to the specified frame index
    ///
    /// Matches C++ VideoStreamInterface::frameGoto
    fn frame_goto(&mut self, index: Int);

    /// Return the height of the video
    ///
    /// Matches C++ VideoStreamInterface::height
    fn height(&self) -> Int;

    /// Return the width of the video
    ///
    /// Matches C++ VideoStreamInterface::width
    fn width(&self) -> Int;
}

//----------------------------------------------------------------------------
// VideoStream - Default Implementation
//----------------------------------------------------------------------------

/// Default video stream implementation
///
/// Matches C++ VideoPlayer.h VideoStream class
pub struct VideoStream {
    /// Next open stream (linked list)
    next: Option<Box<VideoStream>>,
}

impl VideoStream {
    /// Create a new video stream
    ///
    /// Matches C++ VideoStream::VideoStream constructor (lines 344-347)
    pub fn new() -> Self {
        VideoStream { next: None }
    }

    /// Get the next stream in the list
    ///
    /// Internal method for managing the linked list
    pub fn get_next(&self) -> Option<&VideoStream> {
        self.next.as_deref()
    }

    /// Get the next stream in the list (mutable)
    ///
    /// Internal method for managing the linked list
    pub fn get_next_mut(&mut self) -> Option<&mut VideoStream> {
        self.next.as_deref_mut()
    }

    /// Set the next stream in the list
    ///
    /// Internal method for managing the linked list
    pub fn set_next(&mut self, next: Option<Box<VideoStream>>) {
        self.next = next;
    }

    /// Take the next stream, leaving None in its place
    ///
    /// Internal method for managing the linked list
    pub fn take_next(&mut self) -> Option<Box<VideoStream>> {
        self.next.take()
    }
}

impl Default for VideoStream {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoStreamInterface for VideoStream {
    /// Returns next open stream
    ///
    /// Matches C++ VideoStream::next (lines 370-373)
    fn next(&self) -> Option<&dyn VideoStreamInterface> {
        self.next.as_deref().map(|s| s as &dyn VideoStreamInterface)
    }

    /// Returns next open stream (mutable)
    ///
    /// Matches C++ VideoStream::next (lines 370-373)
    fn next_mut(&mut self) -> Option<&mut dyn VideoStreamInterface> {
        self.next
            .as_deref_mut()
            .map(|s| s as &mut dyn VideoStreamInterface)
    }

    /// Update stream
    ///
    /// Matches C++ VideoStream::update (lines 379-381)
    /// Default implementation does nothing
    fn update(&mut self) {
        // Base implementation is empty
    }

    /// Close and free stream
    ///
    /// Matches C++ VideoStream::close (lines 387-390)
    /// In Rust, this consumes self and calls Drop
    fn close(self: Box<Self>) {
        // VideoStream destructor will be called automatically
        // which removes from player list
        drop(self);
    }

    /// Is the frame ready to be displayed
    ///
    /// Matches C++ VideoStream::isFrameReady (lines 396-399)
    /// Default implementation always returns true
    fn is_frame_ready(&self) -> Bool {
        true
    }

    /// Decompress current frame
    ///
    /// Matches C++ VideoStream::frameDecompress (lines 405-408)
    /// Default implementation does nothing
    fn frame_decompress(&mut self) {
        // Base implementation is empty
    }

    /// Render current frame into buffer
    ///
    /// Matches C++ VideoStream::frameRender (lines 414-417)
    /// Default implementation does nothing
    fn frame_render(&mut self, _buffer: &mut dyn VideoBuffer) {
        // Base implementation is empty
    }

    /// Advance to next frame
    ///
    /// Matches C++ VideoStream::frameNext (lines 423-426)
    /// Default implementation does nothing
    fn frame_next(&mut self) {
        // Base implementation is empty
    }

    /// Returns zero based index of current frame
    ///
    /// Matches C++ VideoStream::frameIndex (lines 432-435)
    /// Default implementation returns 0
    fn frame_index(&self) -> Int {
        0
    }

    /// Returns the total number of frames in the stream
    ///
    /// Matches C++ VideoStream::frameCount (lines 441-444)
    /// Default implementation returns 0
    fn frame_count(&self) -> Int {
        0
    }

    /// Go to the specified frame index
    ///
    /// Matches C++ VideoStream::frameGoto (lines 450-453)
    /// Default implementation does nothing
    fn frame_goto(&mut self, _index: Int) {
        // Base implementation is empty
    }

    /// Return the height of the video
    ///
    /// Matches C++ VideoStream::height (lines 459-462)
    /// Default implementation returns 0
    fn height(&self) -> Int {
        0
    }

    /// Return the width of the video
    ///
    /// Matches C++ VideoStream::width (lines 468-471)
    /// Default implementation returns 0
    fn width(&self) -> Int {
        0
    }
}

//----------------------------------------------------------------------------
// Unit Tests
//----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_stream_creation() {
        let stream = VideoStream::new();
        assert!(stream.next.is_none());
    }

    #[test]
    fn test_video_stream_default_values() {
        let stream = VideoStream::new();
        assert_eq!(stream.is_frame_ready(), true);
        assert_eq!(stream.frame_index(), 0);
        assert_eq!(stream.frame_count(), 0);
        assert_eq!(stream.height(), 0);
        assert_eq!(stream.width(), 0);
    }

    #[test]
    fn test_video_stream_next() {
        let mut stream1 = VideoStream::new();
        let stream2 = Box::new(VideoStream::new());

        stream1.set_next(Some(stream2));
        assert!(stream1.get_next().is_some());
    }

    #[test]
    fn test_video_stream_interface_methods() {
        let mut stream = VideoStream::new();

        // Test update (should not panic)
        stream.update();

        // Test frame operations (should not panic)
        stream.frame_decompress();
        stream.frame_next();
        stream.frame_goto(5);

        // Test default return values
        assert_eq!(stream.is_frame_ready(), true);
        assert_eq!(stream.frame_index(), 0);
        assert_eq!(stream.frame_count(), 0);
        assert_eq!(stream.height(), 0);
        assert_eq!(stream.width(), 0);
    }

    #[test]
    fn test_video_stream_linked_list() {
        let mut stream1 = VideoStream::new();
        let mut stream2 = VideoStream::new();
        let stream3 = VideoStream::new();

        // Build linked list: stream1 -> stream2 -> stream3
        stream2.set_next(Some(Box::new(stream3)));
        stream1.set_next(Some(Box::new(stream2)));

        // Verify chain
        assert!(stream1.get_next().is_some());
        assert!(stream1.get_next().unwrap().get_next().is_some());
    }

    #[test]
    fn test_video_stream_take_next() {
        let mut stream1 = VideoStream::new();
        let stream2 = Box::new(VideoStream::new());

        stream1.set_next(Some(stream2));
        assert!(stream1.get_next().is_some());

        let taken = stream1.take_next();
        assert!(taken.is_some());
        assert!(stream1.get_next().is_none());
    }

    #[test]
    fn test_video_stream_close() {
        let stream = Box::new(VideoStream::new());
        // Close should consume the stream
        stream.close();
        // If we get here without panic, the test passes
    }

    #[test]
    fn test_video_stream_trait_object() {
        let stream: Box<dyn VideoStreamInterface> = Box::new(VideoStream::new());

        // Test that trait methods work through trait object
        assert_eq!(stream.is_frame_ready(), true);
        assert_eq!(stream.frame_index(), 0);
        assert_eq!(stream.frame_count(), 0);
        assert_eq!(stream.height(), 0);
        assert_eq!(stream.width(), 0);
    }
}
