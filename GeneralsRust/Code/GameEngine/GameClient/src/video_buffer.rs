//! Video buffer interface for rendering video streams.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

type Bool = bool;
type Real = f32;
type UnsignedInt = u32;

const MAX_SOFTWARE_TEXTURE_DIMENSION: UnsignedInt = 4096;

/// Buffer pixel format types
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectClass {
    pub x1: Real,
    pub y1: Real,
    pub x2: Real,
    pub y2: Real,
}

impl RectClass {
    pub fn new(x1: Real, y1: Real, x2: Real, y2: Real) -> Self {
        RectClass { x1, y1, x2, y2 }
    }

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

/// Video buffer interface trait.
pub trait VideoBuffer: Send {
    fn allocate(&mut self, width: UnsignedInt, height: UnsignedInt) -> Bool;
    fn free(&mut self);
    fn lock(&mut self) -> *mut u8;
    fn unlock(&mut self);
    fn valid(&self) -> Bool;
    fn x_pos(&self) -> UnsignedInt;
    fn y_pos(&self) -> UnsignedInt;
    fn set_pos(&mut self, x: UnsignedInt, y: UnsignedInt);
    fn width(&self) -> UnsignedInt;
    fn height(&self) -> UnsignedInt;
    fn texture_width(&self) -> UnsignedInt;
    fn texture_height(&self) -> UnsignedInt;
    fn pitch(&self) -> UnsignedInt;
    fn format(&self) -> VideoBufferType;

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

#[derive(Clone)]
pub struct VideoBufferHandle {
    inner: Arc<Mutex<dyn VideoBuffer + Send>>,
}

impl VideoBufferHandle {
    pub fn new<B: VideoBuffer + Send + 'static>(buffer: B) -> Self {
        Self {
            inner: Arc::new(Mutex::new(buffer)),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, dyn VideoBuffer + Send + 'static> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for VideoBufferHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VideoBufferHandle").finish()
    }
}

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

    pub fn free_base(&mut self) {
        self.width = 0;
        self.height = 0;
        self.texture_width = 0;
        self.texture_height = 0;
    }
}

#[derive(Debug)]
pub struct SoftwareVideoBuffer {
    base: BaseVideoBuffer,
    data: Vec<u8>,
}

impl SoftwareVideoBuffer {
    pub fn new(format: VideoBufferType) -> Self {
        Self {
            base: BaseVideoBuffer::new(format),
            data: Vec::new(),
        }
    }

    fn bytes_per_pixel(format: VideoBufferType) -> UnsignedInt {
        match format {
            VideoBufferType::R8G8B8 => 3,
            VideoBufferType::X8R8G8B8 => 4,
            VideoBufferType::R5G6B5 | VideoBufferType::X1R5G5B5 => 2,
            VideoBufferType::Unknown => 0,
        }
    }

    fn validate_texture_size(
        width: UnsignedInt,
        height: UnsignedInt,
    ) -> (UnsignedInt, UnsignedInt) {
        let mut texture_width = next_power_of_two_at_least_one(width);
        let mut texture_height = next_power_of_two_at_least_one(height);

        texture_width = texture_width.min(MAX_SOFTWARE_TEXTURE_DIMENSION);
        texture_height = texture_height.min(MAX_SOFTWARE_TEXTURE_DIMENSION);

        if texture_width > texture_height {
            while texture_height != 0 && texture_width / texture_height > 8 {
                texture_height = texture_height.saturating_mul(2);
            }
        } else {
            while texture_width != 0 && texture_height / texture_width > 8 {
                texture_width = texture_width.saturating_mul(2);
            }
        }

        (
            texture_width.min(MAX_SOFTWARE_TEXTURE_DIMENSION),
            texture_height.min(MAX_SOFTWARE_TEXTURE_DIMENSION),
        )
    }
}

fn next_power_of_two_at_least_one(value: UnsignedInt) -> UnsignedInt {
    let mut power = 1;
    while power < value {
        power = power.saturating_mul(2);
    }
    power
}

impl VideoBuffer for SoftwareVideoBuffer {
    fn allocate(&mut self, width: UnsignedInt, height: UnsignedInt) -> Bool {
        self.free();

        let bpp = Self::bytes_per_pixel(self.base.format);
        if bpp == 0 {
            return false;
        }
        let (texture_width, texture_height) = Self::validate_texture_size(width, height);
        let Some(size) = texture_width
            .checked_mul(texture_height)
            .and_then(|pixels| pixels.checked_mul(bpp))
            .map(|bytes| bytes as usize)
        else {
            return false;
        };
        self.data = vec![0; size];
        self.base.width = width;
        self.base.height = height;
        self.base.texture_width = texture_width;
        self.base.texture_height = texture_height;
        self.base.pitch = texture_width * bpp;
        true
    }

    fn free(&mut self) {
        self.data.clear();
        self.base.free_base();
    }

    fn lock(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }

    fn unlock(&mut self) {}

    fn valid(&self) -> Bool {
        !self.data.is_empty()
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
    fn software_video_buffer_validates_texture_size_like_w3d() {
        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8);
        assert!(buffer.allocate(300, 200));

        assert_eq!(buffer.width(), 300);
        assert_eq!(buffer.height(), 200);
        assert_eq!(buffer.texture_width(), 512);
        assert_eq!(buffer.texture_height(), 256);
        assert_eq!(buffer.pitch(), 512 * 4);

        let rect = buffer.rect(0.0, 0.0, 1.0, 1.0);
        assert_eq!(rect, RectClass::new(0.0, 0.0, 300.0 / 512.0, 200.0 / 256.0));
    }

    #[test]
    fn software_video_buffer_balances_extreme_aspect_ratio_like_w3d() {
        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::R5G6B5);
        assert!(buffer.allocate(1, 100));

        assert_eq!(buffer.texture_width(), 16);
        assert_eq!(buffer.texture_height(), 128);
        assert_eq!(buffer.pitch(), 16 * 2);
    }

    #[test]
    fn failed_allocate_frees_existing_buffer_like_w3d() {
        let mut buffer = SoftwareVideoBuffer::new(VideoBufferType::X8R8G8B8);
        assert!(buffer.allocate(64, 64));
        assert!(buffer.valid());

        buffer.base.format = VideoBufferType::Unknown;
        assert!(!buffer.allocate(64, 64));

        assert!(!buffer.valid());
        assert_eq!(buffer.width(), 0);
        assert_eq!(buffer.height(), 0);
        assert_eq!(buffer.texture_width(), 0);
        assert_eq!(buffer.texture_height(), 0);
    }
}
