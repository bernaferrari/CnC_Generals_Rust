//
// Project:    Generals
//
// File name:  GameClient/video_buffer.rs
//
// Created:    Ported from C++
//
// Description: Video buffer interface for rendering video streams
//
// Original C++ source: /GeneralsMD/Code/GameEngine/Include/GameClient/VideoPlayer.h
//
//----------------------------------------------------------------------------

use base_types::{Bool, Real, UnsignedInt};

//----------------------------------------------------------------------------
// VideoBuffer Type Enumeration
//----------------------------------------------------------------------------

/// Buffer pixel format types
///
/// Matches C++ VideoPlayer.h VideoBuffer::Type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VideoBufferType {
    Unknown = 0,
    R8G8B8 = 1,
    X8R8G8B8 = 2,
    R5G6B5 = 3,
    X1R5G5B5 = 4,
}

impl VideoBufferType {
    pub const NUM_TYPES: usize = 5;

    /// Validate and normalize buffer type
    /// Matches C++ VideoBuffer::VideoBuffer constructor validation
    pub fn validate(format: u32) -> Self {
        if format >= Self::NUM_TYPES as u32 {
            VideoBufferType::Unknown
        } else {
            match format {
                0 => VideoBufferType::Unknown,
                1 => VideoBufferType::R8G8B8,
                2 => VideoBufferType::X8R8G8B8,
                3 => VideoBufferType::R5G6B5,
                4 => VideoBufferType::X1R5G5B5,
                _ => VideoBufferType::Unknown,
            }
        }
    }
}

//----------------------------------------------------------------------------
// RectClass
//----------------------------------------------------------------------------

/// Rectangle class for video rendering
///
/// Matches C++ WWMath/rect.h RectClass
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectClass {
    pub x1: Real,
    pub y1: Real,
    pub x2: Real,
    pub y2: Real,
}

impl RectClass {
    /// Create a new rectangle
    pub fn new(x1: Real, y1: Real, x2: Real, y2: Real) -> Self {
        RectClass { x1, y1, x2, y2 }
    }

    /// Set rectangle coordinates
    /// Matches C++ RectClass::Set
    pub fn set(&mut self, x1: Real, y1: Real, x2: Real, y2: Real) {
        self.x1 = x1;
        self.y1 = y1;
        self.x2 = x2;
        self.y2 = y2;
    }
}

impl Default for RectClass {
    fn default() -> Self {
        RectClass::new(0.0, 0.0, 0.0, 0.0)
    }
}

//----------------------------------------------------------------------------
// VideoBuffer Trait
//----------------------------------------------------------------------------

/// Video buffer interface trait
///
/// The VideoPlayer uses this buffer abstraction in order to be able to
/// render a video stream.
///
/// Matches C++ VideoPlayer.h VideoBuffer abstract class
pub trait VideoBuffer {
    /// Allocate buffer
    ///
    /// Matches C++ VideoBuffer::allocate
    fn allocate(&mut self, width: UnsignedInt, height: UnsignedInt) -> Bool;

    /// Free the buffer
    ///
    /// Matches C++ VideoBuffer::free
    fn free(&mut self);

    /// Returns memory pointer to start of buffer
    ///
    /// Matches C++ VideoBuffer::lock
    ///
    /// # Safety
    ///
    /// The returned pointer is only valid until unlock() is called
    fn lock(&mut self) -> *mut u8;

    /// Release buffer
    ///
    /// Matches C++ VideoBuffer::unlock
    fn unlock(&mut self);

    /// Is the buffer valid to use
    ///
    /// Matches C++ VideoBuffer::valid
    fn valid(&self) -> Bool;

    /// Get X pixel offset to draw into
    ///
    /// Matches C++ VideoBuffer::xPos
    fn x_pos(&self) -> UnsignedInt;

    /// Get Y pixel offset to draw into
    ///
    /// Matches C++ VideoBuffer::yPos
    fn y_pos(&self) -> UnsignedInt;

    /// Set the x and y buffer offset
    ///
    /// Matches C++ VideoBuffer::setPos
    fn set_pos(&mut self, x: UnsignedInt, y: UnsignedInt);

    /// Returns pixel width of visible texture
    ///
    /// Matches C++ VideoBuffer::width
    fn width(&self) -> UnsignedInt;

    /// Returns pixel height of visible texture
    ///
    /// Matches C++ VideoBuffer::height
    fn height(&self) -> UnsignedInt;

    /// Returns pixel width of texture
    ///
    /// Matches C++ VideoBuffer::textureWidth
    fn texture_width(&self) -> UnsignedInt;

    /// Returns pixel height of texture
    ///
    /// Matches C++ VideoBuffer::textureHeight
    fn texture_height(&self) -> UnsignedInt;

    /// Returns buffer pitch in bytes
    ///
    /// Matches C++ VideoBuffer::pitch
    fn pitch(&self) -> UnsignedInt;

    /// Returns buffer pixel format
    ///
    /// Matches C++ VideoBuffer::format
    fn format(&self) -> VideoBufferType;

    /// Calculate normalized rectangle coordinates
    ///
    /// Matches C++ VideoBuffer::Rect
    fn rect(&self, x1: Real, y1: Real, x2: Real, y2: Real) -> RectClass {
        let mut rect = RectClass::new(0.0, 0.0, 0.0, 0.0);

        if self.valid() {
            let width = self.width() as Real;
            let height = self.height() as Real;
            let texture_width = self.texture_width() as Real;
            let texture_height = self.texture_height() as Real;

            rect.set(
                (width / texture_width) * x1,
                (height / texture_height) * y1,
                (width / texture_width) * x2,
                (height / texture_height) * y2,
            );
        }

        rect
    }
}

//----------------------------------------------------------------------------
// BaseVideoBuffer - Base implementation
//----------------------------------------------------------------------------

/// Base video buffer implementation
///
/// Provides default storage for VideoBuffer trait implementations.
/// Matches C++ VideoBuffer protected members
#[derive(Debug)]
pub struct BaseVideoBuffer {
    pub x_pos: UnsignedInt,
    pub y_pos: UnsignedInt,
    pub width: UnsignedInt,
    pub height: UnsignedInt,
    pub texture_width: UnsignedInt,
    pub texture_height: UnsignedInt,
    pub pitch: UnsignedInt,
    pub format: VideoBufferType,
}

impl BaseVideoBuffer {
    /// Create a new base video buffer
    ///
    /// Matches C++ VideoBuffer::VideoBuffer constructor
    pub fn new(format: VideoBufferType) -> Self {
        BaseVideoBuffer {
            x_pos: 0,
            y_pos: 0,
            width: 0,
            height: 0,
            texture_width: 0,
            texture_height: 0,
            pitch: 0,
            format,
        }
    }

    /// Free buffer resources
    ///
    /// Matches C++ VideoBuffer::free (lines 113-119)
    pub fn free_base(&mut self) {
        self.width = 0;
        self.height = 0;
        self.texture_width = 0;
        self.texture_height = 0;
    }
}

//----------------------------------------------------------------------------
// Unit Tests
//----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_buffer_type_validate() {
        assert_eq!(VideoBufferType::validate(0), VideoBufferType::Unknown);
        assert_eq!(VideoBufferType::validate(1), VideoBufferType::R8G8B8);
        assert_eq!(VideoBufferType::validate(2), VideoBufferType::X8R8G8B8);
        assert_eq!(VideoBufferType::validate(3), VideoBufferType::R5G6B5);
        assert_eq!(VideoBufferType::validate(4), VideoBufferType::X1R5G5B5);
        assert_eq!(VideoBufferType::validate(999), VideoBufferType::Unknown);
    }

    #[test]
    fn test_rect_class() {
        let rect = RectClass::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(rect.x1, 1.0);
        assert_eq!(rect.y1, 2.0);
        assert_eq!(rect.x2, 3.0);
        assert_eq!(rect.y2, 4.0);

        let mut rect2 = RectClass::default();
        rect2.set(5.0, 6.0, 7.0, 8.0);
        assert_eq!(rect2.x1, 5.0);
        assert_eq!(rect2.y1, 6.0);
        assert_eq!(rect2.x2, 7.0);
        assert_eq!(rect2.y2, 8.0);
    }

    #[test]
    fn test_base_video_buffer_creation() {
        let buffer = BaseVideoBuffer::new(VideoBufferType::X8R8G8B8);
        assert_eq!(buffer.format, VideoBufferType::X8R8G8B8);
        assert_eq!(buffer.width, 0);
        assert_eq!(buffer.height, 0);
        assert_eq!(buffer.x_pos, 0);
        assert_eq!(buffer.y_pos, 0);
    }

    #[test]
    fn test_base_video_buffer_free() {
        let mut buffer = BaseVideoBuffer::new(VideoBufferType::R8G8B8);
        buffer.width = 640;
        buffer.height = 480;
        buffer.texture_width = 1024;
        buffer.texture_height = 512;

        buffer.free_base();

        assert_eq!(buffer.width, 0);
        assert_eq!(buffer.height, 0);
        assert_eq!(buffer.texture_width, 0);
        assert_eq!(buffer.texture_height, 0);
    }

    // Mock implementation for testing rect calculation
    struct MockVideoBuffer {
        base: BaseVideoBuffer,
    }

    impl VideoBuffer for MockVideoBuffer {
        fn allocate(&mut self, _width: UnsignedInt, _height: UnsignedInt) -> Bool {
            true
        }
        fn free(&mut self) {
            self.base.free_base();
        }
        fn lock(&mut self) -> *mut u8 {
            std::ptr::null_mut()
        }
        fn unlock(&mut self) {}
        fn valid(&self) -> Bool {
            self.base.width > 0 && self.base.height > 0
        }
        fn x_pos(&self) -> UnsignedInt {
            self.base.x_pos
        }
        fn y_pos(&self) -> UnsignedInt {
            self.base.y_pos
        }
        fn set_pos(&mut self, x: UnsignedInt, y: UnsignedInt) {
            self.base.x_pos = x;
            self.base.y_pos = y;
        }
        fn width(&self) -> UnsignedInt {
            self.base.width
        }
        fn height(&self) -> UnsignedInt {
            self.base.height
        }
        fn texture_width(&self) -> UnsignedInt {
            self.base.texture_width
        }
        fn texture_height(&self) -> UnsignedInt {
            self.base.texture_height
        }
        fn pitch(&self) -> UnsignedInt {
            self.base.pitch
        }
        fn format(&self) -> VideoBufferType {
            self.base.format
        }
    }

    #[test]
    fn test_video_buffer_rect_calculation() {
        let mut buffer = MockVideoBuffer {
            base: BaseVideoBuffer::new(VideoBufferType::X8R8G8B8),
        };

        // Buffer not valid, should return zero rect
        let rect = buffer.rect(0.0, 0.0, 1.0, 1.0);
        assert_eq!(rect.x1, 0.0);
        assert_eq!(rect.y1, 0.0);
        assert_eq!(rect.x2, 0.0);
        assert_eq!(rect.y2, 0.0);

        // Make buffer valid with specific dimensions
        buffer.base.width = 640;
        buffer.base.height = 480;
        buffer.base.texture_width = 1024;
        buffer.base.texture_height = 512;

        // Test rect calculation matches C++ VideoBuffer::Rect logic
        let rect = buffer.rect(0.0, 0.0, 1024.0, 512.0);
        let expected_x2 = (640.0 / 1024.0) * 1024.0;
        let expected_y2 = (480.0 / 512.0) * 512.0;
        assert_eq!(rect.x1, 0.0);
        assert_eq!(rect.y1, 0.0);
        assert_eq!(rect.x2, expected_x2);
        assert_eq!(rect.y2, expected_y2);
    }
}
