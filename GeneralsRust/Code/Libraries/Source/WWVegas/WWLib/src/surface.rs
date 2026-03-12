//! Surface graphics handling utilities for 2D graphics operations.
//!
//! This module provides surface abstractions for handling 2D graphics surfaces
//! used in game rendering and UI. It includes support for different pixel formats,
//! color depths, surface creation, copying, manipulation, and drawing operations.
//!
//! # Features
//!
//! - Safe buffer handling for pixel data operations
//! - Support for multiple pixel formats and color depths
//! - Surface creation, copying, and manipulation
//! - Blitting operations with transparency support
//! - Drawing operations (pixels, lines, rectangles, fills)
//! - Memory locking for direct pixel access
//! - Comprehensive error handling
//! - Integration with Point2D for coordinate operations
//!
//! # Examples
//!
//! ```rust
//! use wwlib_rust::surface::{Surface, PixelFormat, SurfaceError};
//! use wwlib_rust::point::Point2D;
//!
//! // Create a new surface
//! let mut surface = Surface::new(640, 480, PixelFormat::RGB24)?;
//!
//! // Fill with a color
//! surface.fill(0xFF0000)?; // Red
//!
//! // Draw a pixel
//! surface.put_pixel(Point2D::new(100, 100), 0x00FF00)?; // Green pixel
//!
//! // Draw a line
//! surface.draw_line(
//!     Point2D::new(0, 0),
//!     Point2D::new(100, 100),
//!     0x0000FF // Blue line
//! )?;
//!
//! # Ok::<(), SurfaceError>(())
//! ```

use crate::point::Point2D;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::sync::{Arc, Mutex};

/// Errors that can occur during surface operations.
#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceError {
    /// Invalid surface dimensions (width or height is zero or negative)
    InvalidDimensions,
    /// Coordinates are outside the surface bounds
    OutOfBounds,
    /// Invalid color value for the current pixel format
    InvalidColor,
    /// Memory allocation failed
    AllocationFailed,
    /// Surface is already locked
    AlreadyLocked,
    /// Surface is not locked when lock is required
    NotLocked,
    /// Invalid rectangle dimensions
    InvalidRectangle,
    /// Unsupported pixel format
    UnsupportedPixelFormat,
    /// Buffer underflow/overflow during operation
    BufferError,
}

impl Display for SurfaceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            SurfaceError::InvalidDimensions => write!(f, "Invalid surface dimensions"),
            SurfaceError::OutOfBounds => write!(f, "Coordinates out of bounds"),
            SurfaceError::InvalidColor => write!(f, "Invalid color value"),
            SurfaceError::AllocationFailed => write!(f, "Memory allocation failed"),
            SurfaceError::AlreadyLocked => write!(f, "Surface is already locked"),
            SurfaceError::NotLocked => write!(f, "Surface is not locked"),
            SurfaceError::InvalidRectangle => write!(f, "Invalid rectangle"),
            SurfaceError::UnsupportedPixelFormat => write!(f, "Unsupported pixel format"),
            SurfaceError::BufferError => write!(f, "Buffer operation error"),
        }
    }
}

impl std::error::Error for SurfaceError {}

/// Type alias for surface operation results.
pub type SurfaceResult<T> = Result<T, SurfaceError>;

/// Supported pixel formats for surfaces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PixelFormat {
    /// 8-bit palette indexed color
    Palette8,
    /// 16-bit RGB (5:6:5)
    RGB16,
    /// 24-bit RGB (8:8:8)
    RGB24,
    /// 32-bit RGBA (8:8:8:8)
    RGBA32,
}

impl PixelFormat {
    /// Get the number of bytes per pixel for this format.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::PixelFormat;
    ///
    /// assert_eq!(PixelFormat::Palette8.bytes_per_pixel(), 1);
    /// assert_eq!(PixelFormat::RGB16.bytes_per_pixel(), 2);
    /// assert_eq!(PixelFormat::RGB24.bytes_per_pixel(), 3);
    /// assert_eq!(PixelFormat::RGBA32.bytes_per_pixel(), 4);
    /// ```
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Palette8 => 1,
            PixelFormat::RGB16 => 2,
            PixelFormat::RGB24 => 3,
            PixelFormat::RGBA32 => 4,
        }
    }

    /// Check if this format supports transparency.
    pub fn supports_transparency(self) -> bool {
        matches!(self, PixelFormat::RGBA32)
    }
}

/// A rectangle in 2D space, used for clipping and area operations.
///
/// This is equivalent to the TRect template class from the original C++ code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// X coordinate of upper left corner
    pub x: i32,
    /// Y coordinate of upper left corner  
    pub y: i32,
    /// Width of rectangle
    pub width: i32,
    /// Height of rectangle
    pub height: i32,
}

impl Rect {
    /// Create a new rectangle.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::Rect;
    ///
    /// let rect = Rect::new(10, 20, 100, 50);
    /// assert_eq!(rect.x, 10);
    /// assert_eq!(rect.y, 20);
    /// assert_eq!(rect.width, 100);
    /// assert_eq!(rect.height, 50);
    /// ```
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Create a rectangle from a point and dimensions.
    pub fn from_point(point: Point2D, width: i32, height: i32) -> Self {
        Rect::new(point.x, point.y, width, height)
    }

    /// Check if the rectangle is valid (has positive dimensions).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::Rect;
    ///
    /// assert!(Rect::new(0, 0, 100, 50).is_valid());
    /// assert!(!Rect::new(0, 0, 0, 50).is_valid());
    /// assert!(!Rect::new(0, 0, 100, -10).is_valid());
    /// ```
    pub fn is_valid(self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Get the area of the rectangle.
    pub fn area(self) -> i32 {
        self.width * self.height
    }

    /// Check if a point lies within the rectangle.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::Rect;
    /// use wwlib_rust::point::Point2D;
    ///
    /// let rect = Rect::new(10, 10, 100, 50);
    /// assert!(rect.contains_point(Point2D::new(50, 30)));
    /// assert!(!rect.contains_point(Point2D::new(5, 30)));
    /// ```
    pub fn contains_point(self, point: Point2D) -> bool {
        point.x >= self.x
            && point.x < self.x + self.width
            && point.y >= self.y
            && point.y < self.y + self.height
    }

    /// Check if two rectangles overlap.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::Rect;
    ///
    /// let rect1 = Rect::new(0, 0, 100, 100);
    /// let rect2 = Rect::new(50, 50, 100, 100);
    /// let rect3 = Rect::new(200, 200, 50, 50);
    ///
    /// assert!(rect1.overlaps(rect2));
    /// assert!(!rect1.overlaps(rect3));
    /// ```
    pub fn overlaps(self, other: Rect) -> bool {
        self.x < other.x + other.width
            && self.y < other.y + other.height
            && self.x + self.width > other.x
            && self.y + self.height > other.y
    }

    /// Calculate the intersection of two rectangles.
    /// Returns None if they don't intersect.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::Rect;
    ///
    /// let rect1 = Rect::new(0, 0, 100, 100);
    /// let rect2 = Rect::new(50, 50, 100, 100);
    ///
    /// if let Some(intersection) = rect1.intersect(rect2) {
    ///     assert_eq!(intersection, Rect::new(50, 50, 50, 50));
    /// }
    /// ```
    pub fn intersect(self, other: Rect) -> Option<Rect> {
        if !self.is_valid() || !other.is_valid() {
            return None;
        }

        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);

        if left < right && top < bottom {
            Some(Rect::new(left, top, right - left, bottom - top))
        } else {
            None
        }
    }

    /// Calculate the union of two rectangles.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::Rect;
    ///
    /// let rect1 = Rect::new(0, 0, 50, 50);
    /// let rect2 = Rect::new(25, 25, 50, 50);
    /// let union = rect1.union(rect2);
    /// assert_eq!(union, Rect::new(0, 0, 75, 75));
    /// ```
    pub fn union(self, other: Rect) -> Rect {
        if !self.is_valid() {
            return other;
        }
        if !other.is_valid() {
            return self;
        }

        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);

        Rect::new(left, top, right - left, bottom - top)
    }

    /// Get the corner points of the rectangle.
    pub fn corners(self) -> (Point2D, Point2D, Point2D, Point2D) {
        let top_left = Point2D::new(self.x, self.y);
        let top_right = Point2D::new(self.x + self.width - 1, self.y);
        let bottom_left = Point2D::new(self.x, self.y + self.height - 1);
        let bottom_right = Point2D::new(self.x + self.width - 1, self.y + self.height - 1);

        (top_left, top_right, bottom_left, bottom_right)
    }
}

/// Lock handle for direct surface access.
/// When dropped, automatically unlocks the surface.
pub struct SurfaceLock<'a> {
    surface: &'a Surface,
    data: *mut u8,
    stride: usize,
    offset: Point2D,
}

impl<'a> SurfaceLock<'a> {
    /// Get a pointer to pixel data at the given coordinates.
    /// Returns None if coordinates are out of bounds.
    ///
    /// # Safety
    ///
    /// The returned pointer is only valid while the lock is held.
    /// Caller must ensure proper bounds checking and pixel format alignment.
    pub unsafe fn get_pixel_ptr(&self, point: Point2D) -> Option<*mut u8> {
        if point.x < 0
            || point.y < 0
            || point.x >= self.surface.width as i32
            || point.y >= self.surface.height as i32
        {
            return None;
        }

        let adjusted_x = point.x - self.offset.x;
        let adjusted_y = point.y - self.offset.y;

        if adjusted_x < 0 || adjusted_y < 0 {
            return None;
        }

        let offset = (adjusted_y as usize * self.stride)
            + (adjusted_x as usize * self.surface.pixel_format.bytes_per_pixel());

        unsafe { Some(self.data.add(offset)) }
    }

    /// Get the stride (bytes per row) of the locked surface.
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Get the raw data pointer.
    ///
    /// # Safety
    ///
    /// Direct access to pixel data - caller must ensure proper bounds checking.
    pub unsafe fn data_ptr(&self) -> *mut u8 {
        self.data
    }
}

impl<'a> Drop for SurfaceLock<'a> {
    fn drop(&mut self) {
        // Unlock the surface when the lock goes out of scope
        if let Err(e) = self.surface.unlock() {
            eprintln!("Warning: Failed to unlock surface: {}", e);
        }
    }
}

/// Color palette for 8-bit indexed color surfaces.
#[derive(Debug, Clone)]
pub struct Palette {
    colors: Vec<u32>, // RGBA values
}

impl Palette {
    /// Create a new palette with the specified number of colors.
    /// Colors are initialized to black.
    pub fn new(size: usize) -> Self {
        Palette {
            colors: vec![0xFF000000; size.min(256)], // Alpha=FF, RGB=000
        }
    }

    /// Set a color in the palette.
    ///
    /// # Arguments
    /// * `index` - Palette index (0-255)
    /// * `color` - RGBA color value
    pub fn set_color(&mut self, index: u8, color: u32) -> SurfaceResult<()> {
        if (index as usize) < self.colors.len() {
            self.colors[index as usize] = color;
            Ok(())
        } else {
            Err(SurfaceError::OutOfBounds)
        }
    }

    /// Get a color from the palette.
    pub fn get_color(&self, index: u8) -> Option<u32> {
        self.colors.get(index as usize).copied()
    }

    /// Get the number of colors in the palette.
    pub fn size(&self) -> usize {
        self.colors.len()
    }
}

/// A 2D graphics surface for drawing operations.
///
/// This struct provides the main interface for graphics operations,
/// equivalent to the abstract Surface class from the original C++ code.
pub struct Surface {
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    buffer: Vec<u8>,
    palette: Option<Palette>,
    is_locked: Arc<Mutex<bool>>,
}

impl Surface {
    /// Create a new surface with the specified dimensions and pixel format.
    ///
    /// # Arguments  
    /// * `width` - Width in pixels
    /// * `height` - Height in pixels  
    /// * `pixel_format` - Pixel format for the surface
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::{Surface, PixelFormat};
    ///
    /// let surface = Surface::new(640, 480, PixelFormat::RGB24)?;
    /// assert_eq!(surface.get_width(), 640);
    /// assert_eq!(surface.get_height(), 480);
    /// # Ok::<(), wwlib_rust::surface::SurfaceError>(())
    /// ```
    pub fn new(width: u32, height: u32, pixel_format: PixelFormat) -> SurfaceResult<Self> {
        if width == 0 || height == 0 {
            return Err(SurfaceError::InvalidDimensions);
        }

        let bytes_per_pixel = pixel_format.bytes_per_pixel();
        let buffer_size = (width as usize) * (height as usize) * bytes_per_pixel;
        let buffer = vec![0u8; buffer_size];

        let palette = if pixel_format == PixelFormat::Palette8 {
            Some(Palette::new(256))
        } else {
            None
        };

        Ok(Surface {
            width,
            height,
            pixel_format,
            buffer,
            palette,
            is_locked: Arc::new(Mutex::new(false)),
        })
    }

    /// Get the width of the surface.
    pub fn get_width(&self) -> u32 {
        self.width
    }

    /// Get the height of the surface.
    pub fn get_height(&self) -> u32 {
        self.height
    }

    /// Get the pixel format of the surface.
    pub fn get_pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    /// Get the number of bytes per pixel.
    pub fn bytes_per_pixel(&self) -> usize {
        self.pixel_format.bytes_per_pixel()
    }

    /// Get the stride (bytes per row) of the surface.
    pub fn stride(&self) -> usize {
        (self.width as usize) * self.bytes_per_pixel()
    }

    /// Get the surface bounds as a rectangle.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::{Surface, PixelFormat, Rect};
    ///
    /// let surface = Surface::new(640, 480, PixelFormat::RGB24)?;
    /// let rect = surface.get_rect();
    /// assert_eq!(rect, Rect::new(0, 0, 640, 480));
    /// # Ok::<(), wwlib_rust::surface::SurfaceError>(())
    /// ```
    pub fn get_rect(&self) -> Rect {
        Rect::new(0, 0, self.width as i32, self.height as i32)
    }

    /// Check if coordinates are within surface bounds.
    pub fn is_point_valid(&self, point: Point2D) -> bool {
        point.x >= 0
            && point.y >= 0
            && (point.x as u32) < self.width
            && (point.y as u32) < self.height
    }

    /// Get a reference to the palette (if this is a palette surface).
    pub fn get_palette(&self) -> Option<&Palette> {
        self.palette.as_ref()
    }

    /// Get a mutable reference to the palette (if this is a palette surface).
    pub fn get_palette_mut(&mut self) -> Option<&mut Palette> {
        self.palette.as_mut()
    }

    /// Check if the surface is currently locked.
    pub fn is_locked(&self) -> bool {
        *self.is_locked.lock().unwrap()
    }

    /// Lock the surface for direct pixel access.
    /// Returns a lock handle that automatically unlocks when dropped.
    ///
    /// # Arguments
    /// * `offset` - Offset point for the lock (usually Point2D::new(0, 0))
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::{Surface, PixelFormat};
    /// use wwlib_rust::point::Point2D;
    ///
    /// let mut surface = Surface::new(100, 100, PixelFormat::RGB24)?;
    /// {
    ///     let lock = surface.lock(Point2D::new(0, 0))?;
    ///     // Direct pixel access through lock
    /// } // Surface automatically unlocked here
    /// # Ok::<(), wwlib_rust::surface::SurfaceError>(())
    /// ```
    pub fn lock(&self, offset: Point2D) -> SurfaceResult<SurfaceLock> {
        {
            let mut locked = self.is_locked.lock().unwrap();
            if *locked {
                return Err(SurfaceError::AlreadyLocked);
            }
            *locked = true;
        }

        Ok(SurfaceLock {
            surface: self,
            data: self.buffer.as_ptr() as *mut u8,
            stride: self.stride(),
            offset,
        })
    }

    /// Unlock the surface (called automatically by SurfaceLock drop).
    pub fn unlock(&self) -> SurfaceResult<()> {
        let mut locked = self.is_locked.lock().unwrap();
        if !*locked {
            return Err(SurfaceError::NotLocked);
        }
        *locked = false;
        Ok(())
    }

    /// Fill the entire surface with a color.
    ///
    /// # Arguments
    /// * `color` - Color value in the surface's pixel format
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::{Surface, PixelFormat};
    ///
    /// let mut surface = Surface::new(100, 100, PixelFormat::RGB24)?;
    /// surface.fill(0xFF0000)?; // Fill with red
    /// # Ok::<(), wwlib_rust::surface::SurfaceError>(())
    /// ```
    pub fn fill(&mut self, color: u32) -> SurfaceResult<()> {
        let rect = self.get_rect();
        self.fill_rect(rect, color)
    }

    /// Fill a rectangle with a color.
    ///
    /// # Arguments
    /// * `rect` - Rectangle to fill
    /// * `color` - Color value in the surface's pixel format
    pub fn fill_rect(&mut self, rect: Rect, color: u32) -> SurfaceResult<()> {
        self.fill_rect_clipped(self.get_rect(), rect, color)
    }

    /// Fill a rectangle with a color, clipped to a clipping rectangle.
    ///
    /// # Arguments
    /// * `clip_rect` - Clipping rectangle
    /// * `fill_rect` - Rectangle to fill  
    /// * `color` - Color value in the surface's pixel format
    pub fn fill_rect_clipped(
        &mut self,
        clip_rect: Rect,
        fill_rect: Rect,
        color: u32,
    ) -> SurfaceResult<()> {
        if !clip_rect.is_valid() || !fill_rect.is_valid() {
            return Err(SurfaceError::InvalidRectangle);
        }

        let clipped = match clip_rect.intersect(fill_rect) {
            Some(rect) => rect,
            None => return Ok(()), // No intersection, nothing to draw
        };

        let bytes_per_pixel = self.bytes_per_pixel();
        let stride = self.stride();

        // Convert color to bytes based on pixel format
        let color_bytes = self.color_to_bytes(color)?;

        for y in clipped.y..clipped.y + clipped.height {
            if y < 0 || y >= self.height as i32 {
                continue;
            }

            let row_offset = (y as usize) * stride;
            let start_x = clipped.x.max(0) as usize;
            let end_x = (clipped.x + clipped.width).min(self.width as i32) as usize;

            for x in start_x..end_x {
                let pixel_offset = row_offset + x * bytes_per_pixel;
                if pixel_offset + bytes_per_pixel <= self.buffer.len() {
                    self.buffer[pixel_offset..pixel_offset + bytes_per_pixel]
                        .copy_from_slice(&color_bytes);
                }
            }
        }

        Ok(())
    }

    /// Set a pixel to the specified color.
    ///
    /// # Arguments
    /// * `point` - Pixel coordinates
    /// * `color` - Color value in the surface's pixel format
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::{Surface, PixelFormat};
    /// use wwlib_rust::point::Point2D;
    ///
    /// let mut surface = Surface::new(100, 100, PixelFormat::RGB24)?;
    /// surface.put_pixel(Point2D::new(50, 50), 0x00FF00)?; // Green pixel
    /// # Ok::<(), wwlib_rust::surface::SurfaceError>(())
    /// ```
    pub fn put_pixel(&mut self, point: Point2D, color: u32) -> SurfaceResult<()> {
        if !self.is_point_valid(point) {
            return Err(SurfaceError::OutOfBounds);
        }

        let bytes_per_pixel = self.bytes_per_pixel();
        let pixel_offset =
            (point.y as usize) * self.stride() + (point.x as usize) * bytes_per_pixel;

        if pixel_offset + bytes_per_pixel > self.buffer.len() {
            return Err(SurfaceError::BufferError);
        }

        let color_bytes = self.color_to_bytes(color)?;
        self.buffer[pixel_offset..pixel_offset + bytes_per_pixel].copy_from_slice(&color_bytes);

        Ok(())
    }

    /// Get the color of a pixel.
    ///
    /// # Arguments
    /// * `point` - Pixel coordinates
    ///
    /// # Returns
    /// * Color value in the surface's pixel format
    pub fn get_pixel(&self, point: Point2D) -> SurfaceResult<u32> {
        if !self.is_point_valid(point) {
            return Err(SurfaceError::OutOfBounds);
        }

        let bytes_per_pixel = self.bytes_per_pixel();
        let pixel_offset =
            (point.y as usize) * self.stride() + (point.x as usize) * bytes_per_pixel;

        if pixel_offset + bytes_per_pixel > self.buffer.len() {
            return Err(SurfaceError::BufferError);
        }

        let pixel_bytes = &self.buffer[pixel_offset..pixel_offset + bytes_per_pixel];
        self.bytes_to_color(pixel_bytes)
    }

    /// Draw a line between two points.
    ///
    /// # Arguments
    /// * `start` - Starting point
    /// * `end` - Ending point  
    /// * `color` - Line color
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::{Surface, PixelFormat};
    /// use wwlib_rust::point::Point2D;
    ///
    /// let mut surface = Surface::new(100, 100, PixelFormat::RGB24)?;
    /// surface.draw_line(
    ///     Point2D::new(10, 10),
    ///     Point2D::new(90, 90),
    ///     0x0000FF // Blue line
    /// )?;
    /// # Ok::<(), wwlib_rust::surface::SurfaceError>(())
    /// ```
    pub fn draw_line(&mut self, start: Point2D, end: Point2D, color: u32) -> SurfaceResult<()> {
        self.draw_line_clipped(self.get_rect(), start, end, color)
    }

    /// Draw a line between two points, clipped to a rectangle.
    ///
    /// # Arguments
    /// * `clip_rect` - Clipping rectangle
    /// * `start` - Starting point
    /// * `end` - Ending point
    /// * `color` - Line color
    pub fn draw_line_clipped(
        &mut self,
        clip_rect: Rect,
        start: Point2D,
        end: Point2D,
        color: u32,
    ) -> SurfaceResult<()> {
        // Bresenham's line algorithm with clipping
        let mut x0 = start.x;
        let mut y0 = start.y;
        let x1 = end.x;
        let y1 = end.y;

        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;

        loop {
            let point = Point2D::new(x0, y0);
            if clip_rect.contains_point(point) {
                if let Err(e) = self.put_pixel(point, color) {
                    // Continue drawing even if some pixels are out of bounds
                    if e != SurfaceError::OutOfBounds {
                        return Err(e);
                    }
                }
            }

            if x0 == x1 && y0 == y1 {
                break;
            }

            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x0 += sx;
            }
            if e2 < dx {
                err += dx;
                y0 += sy;
            }
        }

        Ok(())
    }

    /// Draw a rectangle outline.
    ///
    /// # Arguments
    /// * `rect` - Rectangle to draw
    /// * `color` - Line color
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::{Surface, PixelFormat, Rect};
    ///
    /// let mut surface = Surface::new(100, 100, PixelFormat::RGB24)?;
    /// surface.draw_rect(Rect::new(10, 10, 50, 30), 0xFF0000)?; // Red rectangle
    /// # Ok::<(), wwlib_rust::surface::SurfaceError>(())
    /// ```
    pub fn draw_rect(&mut self, rect: Rect, color: u32) -> SurfaceResult<()> {
        self.draw_rect_clipped(self.get_rect(), rect, color)
    }

    /// Draw a rectangle outline, clipped to a clipping rectangle.
    ///
    /// # Arguments  
    /// * `clip_rect` - Clipping rectangle
    /// * `rect` - Rectangle to draw
    /// * `color` - Line color
    pub fn draw_rect_clipped(
        &mut self,
        clip_rect: Rect,
        rect: Rect,
        color: u32,
    ) -> SurfaceResult<()> {
        if !rect.is_valid() {
            return Err(SurfaceError::InvalidRectangle);
        }

        let (top_left, top_right, bottom_left, bottom_right) = rect.corners();

        // Draw the four sides of the rectangle
        self.draw_line_clipped(clip_rect, top_left, top_right, color)?;
        self.draw_line_clipped(clip_rect, top_right, bottom_right, color)?;
        self.draw_line_clipped(clip_rect, bottom_right, bottom_left, color)?;
        self.draw_line_clipped(clip_rect, bottom_left, top_left, color)?;

        Ok(())
    }

    /// Blit (copy) from another surface to this surface.
    ///
    /// # Arguments
    /// * `source` - Source surface to copy from
    /// * `transparent` - Whether to treat color 0 as transparent
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::surface::{Surface, PixelFormat};
    ///
    /// let source = Surface::new(50, 50, PixelFormat::RGB24)?;
    /// let mut dest = Surface::new(100, 100, PixelFormat::RGB24)?;
    ///
    /// dest.blit_from(&source, false)?;
    /// # Ok::<(), wwlib_rust::surface::SurfaceError>(())
    /// ```
    pub fn blit_from(&mut self, source: &Surface, transparent: bool) -> SurfaceResult<()> {
        let dest_rect = self.get_rect();
        let src_rect = source.get_rect();
        self.blit_from_rect(dest_rect, source, src_rect, transparent)
    }

    /// Blit from a source rectangle to a destination rectangle.
    ///
    /// # Arguments
    /// * `dest_rect` - Destination rectangle
    /// * `source` - Source surface
    /// * `src_rect` - Source rectangle  
    /// * `transparent` - Whether to treat color 0 as transparent
    pub fn blit_from_rect(
        &mut self,
        dest_rect: Rect,
        source: &Surface,
        src_rect: Rect,
        transparent: bool,
    ) -> SurfaceResult<()> {
        let clip_rect = self.get_rect();
        self.blit_from_rect_clipped(
            clip_rect,
            dest_rect,
            source,
            source.get_rect(),
            src_rect,
            transparent,
        )
    }

    /// Blit with full clipping control.
    ///
    /// # Arguments
    /// * `dest_clip_rect` - Destination clipping rectangle
    /// * `dest_rect` - Destination rectangle
    /// * `source` - Source surface
    /// * `src_clip_rect` - Source clipping rectangle
    /// * `src_rect` - Source rectangle
    /// * `transparent` - Whether to treat color 0 as transparent
    pub fn blit_from_rect_clipped(
        &mut self,
        dest_clip_rect: Rect,
        dest_rect: Rect,
        source: &Surface,
        src_clip_rect: Rect,
        src_rect: Rect,
        transparent: bool,
    ) -> SurfaceResult<()> {
        // Validate rectangles
        if !dest_rect.is_valid() || !src_rect.is_valid() {
            return Err(SurfaceError::InvalidRectangle);
        }

        // Check pixel format compatibility
        if self.pixel_format != source.pixel_format {
            return Err(SurfaceError::UnsupportedPixelFormat);
        }

        // Calculate actual copying region
        let dest_clipped = match dest_clip_rect.intersect(dest_rect) {
            Some(rect) => rect,
            None => return Ok(()), // No intersection
        };

        let src_clipped = match src_clip_rect.intersect(src_rect) {
            Some(rect) => rect,
            None => return Ok(()), // No intersection
        };

        // Calculate the actual copy dimensions
        let copy_width = dest_clipped.width.min(src_clipped.width);
        let copy_height = dest_clipped.height.min(src_clipped.height);

        if copy_width <= 0 || copy_height <= 0 {
            return Ok(());
        }

        let bytes_per_pixel = self.bytes_per_pixel();
        let dest_stride = self.stride();
        let src_stride = source.stride();

        // Perform the blit
        for y in 0..copy_height {
            let dest_y = dest_clipped.y + y;
            let src_y = src_clipped.y + y;

            if dest_y < 0
                || dest_y >= self.height as i32
                || src_y < 0
                || src_y >= source.height as i32
            {
                continue;
            }

            let dest_row_offset = (dest_y as usize) * dest_stride;
            let src_row_offset = (src_y as usize) * src_stride;

            for x in 0..copy_width {
                let dest_x = dest_clipped.x + x;
                let src_x = src_clipped.x + x;

                if dest_x < 0
                    || dest_x >= self.width as i32
                    || src_x < 0
                    || src_x >= source.width as i32
                {
                    continue;
                }

                let dest_pixel_offset = dest_row_offset + (dest_x as usize) * bytes_per_pixel;
                let src_pixel_offset = src_row_offset + (src_x as usize) * bytes_per_pixel;

                // Check bounds
                if dest_pixel_offset + bytes_per_pixel > self.buffer.len()
                    || src_pixel_offset + bytes_per_pixel > source.buffer.len()
                {
                    continue;
                }

                // Copy pixel, handling transparency if requested
                let src_bytes =
                    &source.buffer[src_pixel_offset..src_pixel_offset + bytes_per_pixel];

                if transparent && self.is_transparent_color(src_bytes)? {
                    continue; // Skip transparent pixels
                }

                self.buffer[dest_pixel_offset..dest_pixel_offset + bytes_per_pixel]
                    .copy_from_slice(src_bytes);
            }
        }

        Ok(())
    }

    /// Convert a color value to bytes in the surface's pixel format.
    fn color_to_bytes(&self, color: u32) -> SurfaceResult<Vec<u8>> {
        match self.pixel_format {
            PixelFormat::Palette8 => {
                if color > 255 {
                    return Err(SurfaceError::InvalidColor);
                }
                Ok(vec![color as u8])
            }
            PixelFormat::RGB16 => {
                // Convert 24-bit RGB to 16-bit RGB (5:6:5)
                let r = ((color >> 16) & 0xFF) as u8;
                let g = ((color >> 8) & 0xFF) as u8;
                let b = (color & 0xFF) as u8;

                let r5 = (r >> 3) as u16;
                let g6 = (g >> 2) as u16;
                let b5 = (b >> 3) as u16;

                let rgb16 = (r5 << 11) | (g6 << 5) | b5;
                Ok(vec![rgb16 as u8, (rgb16 >> 8) as u8])
            }
            PixelFormat::RGB24 => {
                let r = ((color >> 16) & 0xFF) as u8;
                let g = ((color >> 8) & 0xFF) as u8;
                let b = (color & 0xFF) as u8;
                Ok(vec![b, g, r]) // BGR format
            }
            PixelFormat::RGBA32 => {
                let a = ((color >> 24) & 0xFF) as u8;
                let r = ((color >> 16) & 0xFF) as u8;
                let g = ((color >> 8) & 0xFF) as u8;
                let b = (color & 0xFF) as u8;
                Ok(vec![b, g, r, a]) // BGRA format
            }
        }
    }

    /// Convert bytes to a color value in the surface's pixel format.
    fn bytes_to_color(&self, bytes: &[u8]) -> SurfaceResult<u32> {
        match self.pixel_format {
            PixelFormat::Palette8 => {
                if bytes.is_empty() {
                    return Err(SurfaceError::BufferError);
                }
                Ok(bytes[0] as u32)
            }
            PixelFormat::RGB16 => {
                if bytes.len() < 2 {
                    return Err(SurfaceError::BufferError);
                }
                let rgb16 = bytes[0] as u16 | ((bytes[1] as u16) << 8);

                let r5 = (rgb16 >> 11) & 0x1F;
                let g6 = (rgb16 >> 5) & 0x3F;
                let b5 = rgb16 & 0x1F;

                let r = (r5 << 3) | (r5 >> 2);
                let g = (g6 << 2) | (g6 >> 4);
                let b = (b5 << 3) | (b5 >> 2);

                Ok(((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
            }
            PixelFormat::RGB24 => {
                if bytes.len() < 3 {
                    return Err(SurfaceError::BufferError);
                }
                Ok(((bytes[2] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[0] as u32))
            }
            PixelFormat::RGBA32 => {
                if bytes.len() < 4 {
                    return Err(SurfaceError::BufferError);
                }
                Ok(((bytes[3] as u32) << 24)
                    | ((bytes[2] as u32) << 16)
                    | ((bytes[1] as u32) << 8)
                    | (bytes[0] as u32))
            }
        }
    }

    /// Check if a pixel is the transparent color (color 0).
    fn is_transparent_color(&self, bytes: &[u8]) -> SurfaceResult<bool> {
        match self.pixel_format {
            PixelFormat::Palette8 => Ok(bytes[0] == 0),
            PixelFormat::RGB16 => Ok(bytes[0] == 0 && bytes[1] == 0),
            PixelFormat::RGB24 => Ok(bytes[0] == 0 && bytes[1] == 0 && bytes[2] == 0),
            PixelFormat::RGBA32 => Ok(bytes[3] == 0), // Transparent if alpha is 0
        }
    }
}

impl Debug for Surface {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Surface")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pixel_format", &self.pixel_format)
            .field("buffer_size", &self.buffer.len())
            .field("has_palette", &self.palette.is_some())
            .field("is_locked", &self.is_locked())
            .finish()
    }
}

// Implementation of Clone for Surface (creates a deep copy)
impl Clone for Surface {
    fn clone(&self) -> Self {
        Surface {
            width: self.width,
            height: self.height,
            pixel_format: self.pixel_format,
            buffer: self.buffer.clone(),
            palette: self.palette.clone(),
            is_locked: Arc::new(Mutex::new(false)), // New surface starts unlocked
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_creation() {
        let surface = Surface::new(640, 480, PixelFormat::RGB24).unwrap();
        assert_eq!(surface.get_width(), 640);
        assert_eq!(surface.get_height(), 480);
        assert_eq!(surface.get_pixel_format(), PixelFormat::RGB24);
        assert_eq!(surface.bytes_per_pixel(), 3);
    }

    #[test]
    fn test_invalid_dimensions() {
        assert!(Surface::new(0, 480, PixelFormat::RGB24).is_err());
        assert!(Surface::new(640, 0, PixelFormat::RGB24).is_err());
    }

    #[test]
    fn test_pixel_format_bytes() {
        assert_eq!(PixelFormat::Palette8.bytes_per_pixel(), 1);
        assert_eq!(PixelFormat::RGB16.bytes_per_pixel(), 2);
        assert_eq!(PixelFormat::RGB24.bytes_per_pixel(), 3);
        assert_eq!(PixelFormat::RGBA32.bytes_per_pixel(), 4);
    }

    #[test]
    fn test_rect_operations() {
        let rect1 = Rect::new(0, 0, 100, 100);
        let rect2 = Rect::new(50, 50, 100, 100);
        let rect3 = Rect::new(200, 200, 50, 50);

        assert!(rect1.is_valid());
        assert!(rect1.overlaps(rect2));
        assert!(!rect1.overlaps(rect3));
        assert!(rect1.contains_point(Point2D::new(50, 50)));
        assert!(!rect1.contains_point(Point2D::new(150, 150)));

        let intersection = rect1.intersect(rect2).unwrap();
        assert_eq!(intersection, Rect::new(50, 50, 50, 50));

        let union = rect1.union(rect2);
        assert_eq!(union, Rect::new(0, 0, 150, 150));
    }

    #[test]
    fn test_palette_operations() {
        let mut palette = Palette::new(256);
        assert_eq!(palette.size(), 256);

        palette.set_color(0, 0xFF000000).unwrap(); // Black
        palette.set_color(1, 0xFFFFFFFF).unwrap(); // White

        assert_eq!(palette.get_color(0), Some(0xFF000000));
        assert_eq!(palette.get_color(1), Some(0xFFFFFFFF));
        assert_eq!(palette.get_color(255), Some(0xFF000000)); // Default
    }

    #[test]
    fn test_surface_fill() {
        let mut surface = Surface::new(100, 100, PixelFormat::RGB24).unwrap();
        surface.fill(0xFF0000).unwrap(); // Fill with red

        let pixel = surface.get_pixel(Point2D::new(50, 50)).unwrap();
        assert_eq!(pixel, 0xFF0000);
    }

    #[test]
    fn test_pixel_operations() {
        let mut surface = Surface::new(100, 100, PixelFormat::RGB24).unwrap();

        // Test put_pixel and get_pixel
        surface.put_pixel(Point2D::new(10, 20), 0x00FF00).unwrap();
        let pixel = surface.get_pixel(Point2D::new(10, 20)).unwrap();
        assert_eq!(pixel, 0x00FF00);

        // Test bounds checking
        assert!(surface.put_pixel(Point2D::new(-1, 20), 0xFF0000).is_err());
        assert!(surface.put_pixel(Point2D::new(100, 20), 0xFF0000).is_err());
    }

    #[test]
    fn test_line_drawing() {
        let mut surface = Surface::new(100, 100, PixelFormat::RGB24).unwrap();

        // Draw a diagonal line
        surface
            .draw_line(Point2D::new(0, 0), Point2D::new(10, 10), 0x0000FF)
            .unwrap();

        // Check that some pixels along the line are set
        let pixel = surface.get_pixel(Point2D::new(0, 0)).unwrap();
        assert_eq!(pixel, 0x0000FF);

        let pixel = surface.get_pixel(Point2D::new(5, 5)).unwrap();
        assert_eq!(pixel, 0x0000FF);
    }

    #[test]
    fn test_rectangle_drawing() {
        let mut surface = Surface::new(100, 100, PixelFormat::RGB24).unwrap();

        let rect = Rect::new(10, 10, 20, 15);
        surface.draw_rect(rect, 0xFF00FF).unwrap();

        // Check corners
        assert_eq!(surface.get_pixel(Point2D::new(10, 10)).unwrap(), 0xFF00FF);
        assert_eq!(surface.get_pixel(Point2D::new(29, 10)).unwrap(), 0xFF00FF);
        assert_eq!(surface.get_pixel(Point2D::new(10, 24)).unwrap(), 0xFF00FF);
        assert_eq!(surface.get_pixel(Point2D::new(29, 24)).unwrap(), 0xFF00FF);
    }

    #[test]
    fn test_surface_lock_unlock() {
        let surface = Surface::new(100, 100, PixelFormat::RGB24).unwrap();

        assert!(!surface.is_locked());

        {
            let _lock = surface.lock(Point2D::new(0, 0)).unwrap();
            assert!(surface.is_locked());

            // Should not be able to lock again
            assert!(surface.lock(Point2D::new(0, 0)).is_err());
        } // Lock should be released here

        assert!(!surface.is_locked());
    }

    #[test]
    fn test_blit_operations() {
        let mut source = Surface::new(50, 50, PixelFormat::RGB24).unwrap();
        source.fill(0xFF0000).unwrap(); // Fill source with red

        let mut dest = Surface::new(100, 100, PixelFormat::RGB24).unwrap();
        dest.fill(0x0000FF).unwrap(); // Fill dest with blue

        // Blit source to dest
        dest.blit_from(&source, false).unwrap();

        // Check that the destination now has red pixels in the copied area
        assert_eq!(dest.get_pixel(Point2D::new(25, 25)).unwrap(), 0xFF0000);
        // And blue pixels outside the copied area
        assert_eq!(dest.get_pixel(Point2D::new(75, 75)).unwrap(), 0x0000FF);
    }

    #[test]
    fn test_color_conversion() {
        let surface = Surface::new(10, 10, PixelFormat::RGB24).unwrap();

        // Test RGB24 color conversion
        let color = 0x123456;
        let bytes = surface.color_to_bytes(color).unwrap();
        assert_eq!(bytes, vec![0x56, 0x34, 0x12]); // BGR format

        let back_color = surface.bytes_to_color(&bytes).unwrap();
        assert_eq!(back_color, color);
    }

    #[test]
    fn test_surface_cloning() {
        let mut original = Surface::new(50, 50, PixelFormat::RGB24).unwrap();
        original.fill(0xFF0000).unwrap();

        let cloned = original.clone();
        assert_eq!(cloned.get_width(), original.get_width());
        assert_eq!(cloned.get_height(), original.get_height());
        assert_eq!(cloned.get_pixel_format(), original.get_pixel_format());

        // Check that pixel data was copied
        assert_eq!(
            cloned.get_pixel(Point2D::new(25, 25)).unwrap(),
            original.get_pixel(Point2D::new(25, 25)).unwrap()
        );
    }

    #[test]
    fn test_transparency_handling() {
        let mut source = Surface::new(10, 10, PixelFormat::RGB24).unwrap();
        source.put_pixel(Point2D::new(5, 5), 0x000000).unwrap(); // Black (transparent)
        source.put_pixel(Point2D::new(6, 6), 0xFF0000).unwrap(); // Red (opaque)

        let mut dest = Surface::new(20, 20, PixelFormat::RGB24).unwrap();
        dest.fill(0x0000FF).unwrap(); // Fill with blue

        // Blit with transparency
        dest.blit_from(&source, true).unwrap();

        // Transparent pixel should not overwrite destination
        assert_eq!(dest.get_pixel(Point2D::new(5, 5)).unwrap(), 0x0000FF); // Still blue
                                                                           // Opaque pixel should overwrite destination
        assert_eq!(dest.get_pixel(Point2D::new(6, 6)).unwrap(), 0xFF0000); // Now red
    }
}
