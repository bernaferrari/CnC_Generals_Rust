//! CPU surface helper mirroring the legacy `SurfaceClass`.
//!
//! The original engine exposed a DirectX8 surface wrapper that provided
//! pixel-level operations for font extraction, debug overlays, and texture
//! preprocessing.  The Rust port needs the same functionality so that loaders
//! and tooling can faithfully reproduce colour keying, glyph packing, and
//! other CPU-side manipulations before data ever reaches WGPU.

use crate::core::{Error, Result};
use crate::math_utilities::Vector3;
use crate::texture_system::TextureFormat;
use glam::Vec3;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Description of a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceDescription {
    pub format: TextureFormat,
    pub width: u32,
    pub height: u32,
}

/// Rectangular region expressed in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl SurfaceRect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn intersects(self, other: Self) -> bool {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);
        x1 < x2 && y1 < y2
    }
}

#[derive(Debug)]
struct SurfaceStorage {
    desc: SurfaceDescription,
    row_pitch: usize,
    pixels: RwLock<Vec<u8>>,
}

/// CPU surface with reference-counted storage.
#[derive(Clone, Debug)]
pub struct SurfaceClass {
    inner: Arc<SurfaceStorage>,
}

/// Read-only lock over a surface.
pub struct SurfaceLock<'a> {
    row_pitch: usize,
    guard: RwLockReadGuard<'a, Vec<u8>>,
}

impl<'a> SurfaceLock<'a> {
    pub fn pitch(&self) -> usize {
        self.row_pitch
    }

    pub fn pixels(&self) -> &[u8] {
        &self.guard
    }
}

/// Mutable lock over a surface.
pub struct SurfaceLockMut<'a> {
    row_pitch: usize,
    guard: RwLockWriteGuard<'a, Vec<u8>>,
}

impl<'a> SurfaceLockMut<'a> {
    pub fn pitch(&self) -> usize {
        self.row_pitch
    }

    pub fn pixels(&self) -> &[u8] {
        &self.guard
    }

    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.guard
    }
}

impl SurfaceClass {
    /// Create a zeroed surface.
    pub fn new(width: u32, height: u32, format: TextureFormat) -> Result<Self> {
        let row_pitch = row_pitch_for(format, width)
            .ok_or_else(|| Error::Generic("Unsupported texture format for SurfaceClass".into()))?;
        let storage = SurfaceStorage {
            desc: SurfaceDescription {
                format,
                width,
                height,
            },
            row_pitch,
            pixels: RwLock::new(vec![0u8; row_pitch * effective_rows(format, height)]),
        };
        Ok(Self {
            inner: Arc::new(storage),
        })
    }

    /// Create a surface by copying raw pixel data.
    pub fn from_bytes(
        width: u32,
        height: u32,
        format: TextureFormat,
        bytes: &[u8],
    ) -> Result<Self> {
        let row_pitch = row_pitch_for(format, width)
            .ok_or_else(|| Error::Generic("Unsupported texture format for SurfaceClass".into()))?;
        let expected_size = row_pitch * effective_rows(format, height);
        if bytes.len() != expected_size {
            return Err(Error::InvalidData(format!(
                "Surface byte count mismatch (expected {expected_size}, found {})",
                bytes.len()
            )));
        }

        let storage = SurfaceStorage {
            desc: SurfaceDescription {
                format,
                width,
                height,
            },
            row_pitch,
            pixels: RwLock::new(bytes.to_vec()),
        };
        Ok(Self {
            inner: Arc::new(storage),
        })
    }

    /// Accessor for the description.
    pub fn description(&self) -> SurfaceDescription {
        self.inner.desc
    }

    /// Acquire a read-only view of the pixels.
    pub fn lock(&self) -> SurfaceLock<'_> {
        SurfaceLock {
            row_pitch: self.inner.row_pitch,
            guard: self.inner.pixels.read().expect("surface lock poisoned"),
        }
    }

    /// Acquire a mutable view of the pixels.
    pub fn lock_mut(&self) -> SurfaceLockMut<'_> {
        SurfaceLockMut {
            row_pitch: self.inner.row_pitch,
            guard: self.inner.pixels.write().expect("surface lock poisoned"),
        }
    }

    /// Zero the surface contents.
    pub fn clear(&self) {
        let mut lock = self.lock_mut();
        lock.pixels_mut().fill(0);
    }

    /// Copy a rectangle from another surface.
    pub fn blit(
        &self,
        dst_origin: (u32, u32),
        src: &SurfaceClass,
        src_rect: SurfaceRect,
    ) -> Result<()> {
        if self.inner.desc.format != src.inner.desc.format {
            return Err(Error::Generic(
                "Surface blit requires matching texture formats".into(),
            ));
        }
        if src_rect.x.checked_add(src_rect.width).unwrap_or(0) > src.inner.desc.width
            || src_rect.y.checked_add(src_rect.height).unwrap_or(0) > src.inner.desc.height
        {
            return Err(Error::InvalidData(
                "Source blit rectangle lies outside the surface bounds".into(),
            ));
        }

        let dst_rect =
            SurfaceRect::new(dst_origin.0, dst_origin.1, src_rect.width, src_rect.height);
        if dst_rect.x.checked_add(dst_rect.width).unwrap_or(0) > self.inner.desc.width
            || dst_rect.y.checked_add(dst_rect.height).unwrap_or(0) > self.inner.desc.height
        {
            return Err(Error::InvalidData(
                "Destination blit rectangle lies outside the surface bounds".into(),
            ));
        }

        let bytes_per_pixel = pixel_stride(self.inner.desc.format)
            .ok_or_else(|| Error::Generic("Compressed surfaces do not support CPU blits".into()))?;

        let src_lock = src.lock();
        let mut dst_lock = self.lock_mut();

        for row in 0..src_rect.height {
            let src_y = (src_rect.y + row) as usize;
            let dst_y = (dst_rect.y + row) as usize;
            let src_offset = src_y * src_lock.pitch() + (src_rect.x as usize * bytes_per_pixel);
            let dst_offset = dst_y * dst_lock.pitch() + (dst_rect.x as usize * bytes_per_pixel);
            let count = (src_rect.width as usize) * bytes_per_pixel;

            let src_slice = &src_lock.pixels()[src_offset..src_offset + count];
            let dst_slice = &mut dst_lock.pixels_mut()[dst_offset..dst_offset + count];
            dst_slice.copy_from_slice(src_slice);
        }

        Ok(())
    }

    /// Copy raw bytes into the surface.
    pub fn copy_from_bytes(&self, bytes: &[u8]) -> Result<()> {
        let expected_size =
            self.inner.row_pitch * effective_rows(self.inner.desc.format, self.inner.desc.height);
        if bytes.len() != expected_size {
            return Err(Error::InvalidData(format!(
                "copy_from_bytes expected {expected_size} bytes but received {}",
                bytes.len()
            )));
        }
        let mut lock = self.lock_mut();
        lock.pixels_mut().copy_from_slice(bytes);
        Ok(())
    }

    /// Draw a single pixel (RGBA order).
    pub fn draw_pixel(&self, x: u32, y: u32, color: [u8; 4]) -> Result<()> {
        ensure_uncompressed(self.inner.desc.format)?;
        if x >= self.inner.desc.width || y >= self.inner.desc.height {
            return Ok(());
        }
        let bytes_per_pixel = pixel_stride(self.inner.desc.format).unwrap();
        let mut lock = self.lock_mut();
        let offset = y as usize * lock.pitch() + x as usize * bytes_per_pixel;
        write_rgba_pixel(
            &mut lock.pixels_mut()[offset..offset + bytes_per_pixel],
            self.inner.desc.format,
            color,
        );
        Ok(())
    }

    /// Draw a horizontal line.
    pub fn draw_hline(&self, y: u32, x1: u32, x2: u32, color: [u8; 4]) -> Result<()> {
        if y >= self.inner.desc.height {
            return Ok(());
        }
        let start = x1.min(x2);
        let mut end = x1.max(x2);
        if start >= self.inner.desc.width {
            return Ok(());
        }
        end = end.min(self.inner.desc.width.saturating_sub(1));
        for x in start..=end {
            self.draw_pixel(x, y, color)?;
        }
        Ok(())
    }

    /// Determine whether an entire column is transparent (alpha == 0).
    pub fn is_transparent_column(&self, column: u32) -> Result<bool> {
        ensure_uncompressed(self.inner.desc.format)?;
        if column >= self.inner.desc.width {
            return Ok(true);
        }
        let bytes_per_pixel = pixel_stride(self.inner.desc.format).unwrap();
        let lock = self.lock();
        for y in 0..self.inner.desc.height {
            let offset = y as usize * lock.pitch() + column as usize * bytes_per_pixel;
            let pixel = read_rgba_pixel(
                &lock.pixels()[offset..offset + bytes_per_pixel],
                self.inner.desc.format,
            );
            if pixel[3] != 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Compute the bounding box of all pixels with non-zero alpha.
    pub fn find_nonzero_alpha_bounds(&self) -> Result<Option<SurfaceRect>> {
        ensure_uncompressed(self.inner.desc.format)?;
        let bytes_per_pixel = pixel_stride(self.inner.desc.format).unwrap();
        let lock = self.lock();

        let mut min_x = self.inner.desc.width;
        let mut min_y = self.inner.desc.height;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        let mut found = false;

        for y in 0..self.inner.desc.height {
            for x in 0..self.inner.desc.width {
                let offset = y as usize * lock.pitch() + x as usize * bytes_per_pixel;
                let pixel = read_rgba_pixel(
                    &lock.pixels()[offset..offset + bytes_per_pixel],
                    self.inner.desc.format,
                );
                if pixel[3] != 0 {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                    found = true;
                }
            }
        }

        if found {
            Ok(Some(SurfaceRect::new(
                min_x,
                min_y,
                max_x - min_x + 1,
                max_y - min_y + 1,
            )))
        } else {
            Ok(None)
        }
    }

    /// Return a copy of the surface bytes.
    pub fn create_copy(&self) -> Vec<u8> {
        self.lock().pixels().to_vec()
    }

    /// Apply an HSV hue shift (legacy behaviour approximation).
    pub fn hue_shift(&self, hsv_shift: Vector3) -> Result<()> {
        ensure_uncompressed(self.inner.desc.format)?;
        let mut lock = self.lock_mut();
        let pitch = lock.pitch();
        let bytes_per_pixel = pixel_stride(self.inner.desc.format).unwrap();

        for y in 0..self.inner.desc.height {
            for x in 0..self.inner.desc.width {
                let offset = y as usize * pitch + x as usize * bytes_per_pixel;
                let pixel_slice = &mut lock.pixels_mut()[offset..offset + bytes_per_pixel];
                let mut rgba =
                    read_rgba_pixel(pixel_slice, self.inner.desc.format).map(|v| v as f32 / 255.0);
                let mut hsv = rgb_to_hsv(Vec3::new(rgba[0], rgba[1], rgba[2]));
                hsv.x = (hsv.x + hsv_shift.x).fract();
                hsv.y = (hsv.y + hsv_shift.y).clamp(0.0, 1.0);
                hsv.z = (hsv.z + hsv_shift.z).clamp(0.0, 1.0);
                let rgb = hsv_to_rgb(hsv);
                rgba[0] = rgb.x;
                rgba[1] = rgb.y;
                rgba[2] = rgb.z;
                let packed = [
                    (rgba[0] * 255.0).round() as u8,
                    (rgba[1] * 255.0).round() as u8,
                    (rgba[2] * 255.0).round() as u8,
                    pixel_slice_alpha(pixel_slice, self.inner.desc.format),
                ];
                write_rgba_pixel(pixel_slice, self.inner.desc.format, packed);
            }
        }
        Ok(())
    }

    /// Check if all colour channels are identical for every pixel.
    pub fn is_monochrome(&self) -> Result<bool> {
        ensure_uncompressed(self.inner.desc.format)?;
        let bytes_per_pixel = pixel_stride(self.inner.desc.format).unwrap();
        let lock = self.lock();
        for chunk in lock.pixels().chunks_exact(bytes_per_pixel) {
            let rgba = read_rgba_pixel(chunk, self.inner.desc.format);
            if !(rgba[0] == rgba[1] && rgba[1] == rgba[2]) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn row_pitch_for(format: TextureFormat, width: u32) -> Option<usize> {
    Some(match format {
        TextureFormat::Rgba8Unorm
        | TextureFormat::Rgba8UnormSrgb
        | TextureFormat::Bgra8Unorm
        | TextureFormat::Bgra8UnormSrgb => (width as usize) * 4,
        TextureFormat::R5G6B5 | TextureFormat::A1R5G5B5 | TextureFormat::A4R4G4B4 => {
            (width as usize) * 2
        }
        TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc4RUnorm => {
            ((width + 3) / 4).max(1) as usize * 8
        }
        TextureFormat::Bc2RgbaUnorm
        | TextureFormat::Bc3RgbaUnorm
        | TextureFormat::Bc5RgUnorm
        | TextureFormat::Bc6hRgbUfloat
        | TextureFormat::Bc7RgbaUnorm => ((width + 3) / 4).max(1) as usize * 16,
    })
}

fn effective_rows(format: TextureFormat, height: u32) -> usize {
    match format {
        TextureFormat::Rgba8Unorm
        | TextureFormat::Rgba8UnormSrgb
        | TextureFormat::Bgra8Unorm
        | TextureFormat::Bgra8UnormSrgb
        | TextureFormat::R5G6B5
        | TextureFormat::A1R5G5B5
        | TextureFormat::A4R4G4B4 => height as usize,
        TextureFormat::Bc1RgbaUnorm
        | TextureFormat::Bc2RgbaUnorm
        | TextureFormat::Bc3RgbaUnorm
        | TextureFormat::Bc4RUnorm
        | TextureFormat::Bc5RgUnorm
        | TextureFormat::Bc6hRgbUfloat
        | TextureFormat::Bc7RgbaUnorm => ((height + 3) / 4).max(1) as usize,
    }
}

fn pixel_stride(format: TextureFormat) -> Option<usize> {
    match format {
        TextureFormat::Rgba8Unorm
        | TextureFormat::Rgba8UnormSrgb
        | TextureFormat::Bgra8Unorm
        | TextureFormat::Bgra8UnormSrgb => Some(4),
        TextureFormat::R5G6B5 | TextureFormat::A1R5G5B5 | TextureFormat::A4R4G4B4 => Some(2),
        _ => None,
    }
}

fn ensure_uncompressed(format: TextureFormat) -> Result<()> {
    if pixel_stride(format).is_none() {
        return Err(Error::Generic(
            "Operation requires an uncompressed surface format".into(),
        ));
    }
    Ok(())
}

fn read_rgba_pixel(data: &[u8], format: TextureFormat) -> [u8; 4] {
    match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => {
            [data[0], data[1], data[2], data[3]]
        }
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => {
            [data[2], data[1], data[0], data[3]]
        }
        TextureFormat::R5G6B5 => {
            let value = u16::from_le_bytes([data[0], data[1]]);
            let r = expand_5_to_8(((value >> 11) & 0x1F) as u8);
            let g = expand_6_to_8(((value >> 5) & 0x3F) as u8);
            let b = expand_5_to_8((value & 0x1F) as u8);
            [r, g, b, 255]
        }
        TextureFormat::A1R5G5B5 => {
            let value = u16::from_le_bytes([data[0], data[1]]);
            let r = expand_5_to_8(((value >> 10) & 0x1F) as u8);
            let g = expand_5_to_8(((value >> 5) & 0x1F) as u8);
            let b = expand_5_to_8((value & 0x1F) as u8);
            let a = if (value & 0x8000) != 0 { 255 } else { 0 };
            [r, g, b, a]
        }
        TextureFormat::A4R4G4B4 => {
            let value = u16::from_le_bytes([data[0], data[1]]);
            let r = expand_4_to_8(((value >> 8) & 0x0F) as u8);
            let g = expand_4_to_8(((value >> 4) & 0x0F) as u8);
            let b = expand_4_to_8((value & 0x0F) as u8);
            let a = expand_4_to_8(((value >> 12) & 0x0F) as u8);
            [r, g, b, a]
        }
        _ => [0, 0, 0, 0],
    }
}

fn write_rgba_pixel(dest: &mut [u8], format: TextureFormat, rgba: [u8; 4]) {
    match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => {
            dest[0] = rgba[0];
            dest[1] = rgba[1];
            dest[2] = rgba[2];
            dest[3] = rgba[3];
        }
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => {
            dest[0] = rgba[2];
            dest[1] = rgba[1];
            dest[2] = rgba[0];
            dest[3] = rgba[3];
        }
        TextureFormat::R5G6B5 => {
            let r = compress_5(rgba[0]);
            let g = compress_6(rgba[1]);
            let b = compress_5(rgba[2]);
            let packed = ((r as u16) << 11) | ((g as u16) << 5) | b as u16;
            dest[..2].copy_from_slice(&packed.to_le_bytes());
        }
        TextureFormat::A1R5G5B5 => {
            let r = compress_5(rgba[0]);
            let g = compress_5(rgba[1]);
            let b = compress_5(rgba[2]);
            let a = if rgba[3] >= 128 { 1u16 } else { 0u16 };
            let packed = (a << 15) | ((r as u16) << 10) | ((g as u16) << 5) | b as u16;
            dest[..2].copy_from_slice(&packed.to_le_bytes());
        }
        TextureFormat::A4R4G4B4 => {
            let r = compress_4(rgba[0]);
            let g = compress_4(rgba[1]);
            let b = compress_4(rgba[2]);
            let a = compress_4(rgba[3]);
            let packed = ((a as u16) << 12) | ((r as u16) << 8) | ((g as u16) << 4) | b as u16;
            dest[..2].copy_from_slice(&packed.to_le_bytes());
        }
        _ => {}
    }
}

fn pixel_slice_alpha(pixel: &[u8], format: TextureFormat) -> u8 {
    match format {
        TextureFormat::Rgba8Unorm
        | TextureFormat::Rgba8UnormSrgb
        | TextureFormat::Bgra8Unorm
        | TextureFormat::Bgra8UnormSrgb => pixel[3],
        TextureFormat::R5G6B5 => 255,
        TextureFormat::A1R5G5B5 => {
            let value = u16::from_le_bytes([pixel[0], pixel[1]]);
            if (value & 0x8000) != 0 {
                255
            } else {
                0
            }
        }
        TextureFormat::A4R4G4B4 => {
            let value = u16::from_le_bytes([pixel[0], pixel[1]]);
            expand_4_to_8(((value >> 12) & 0x0F) as u8)
        }
        _ => 255,
    }
}

fn expand_5_to_8(value: u8) -> u8 {
    (value << 3) | (value >> 2)
}

fn expand_6_to_8(value: u8) -> u8 {
    (value << 2) | (value >> 4)
}

fn expand_4_to_8(value: u8) -> u8 {
    (value << 4) | value
}

fn compress_5(value: u8) -> u8 {
    (value >> 3) & 0x1F
}

fn compress_6(value: u8) -> u8 {
    (value >> 2) & 0x3F
}

fn compress_4(value: u8) -> u8 {
    (value >> 4) & 0x0F
}

fn rgb_to_hsv(rgb: Vec3) -> Vec3 {
    let max = rgb.max_element();
    let min = rgb.min_element();
    let delta = max - min;

    let mut hue = 0.0;
    if delta > f32::EPSILON {
        if (max - rgb.x).abs() < f32::EPSILON {
            hue = (rgb.y - rgb.z) / delta;
        } else if (max - rgb.y).abs() < f32::EPSILON {
            hue = 2.0 + (rgb.z - rgb.x) / delta;
        } else {
            hue = 4.0 + (rgb.x - rgb.y) / delta;
        }
        hue /= 6.0;
        if hue < 0.0 {
            hue += 1.0;
        }
    }

    let saturation = if max > 0.0 { delta / max } else { 0.0 };
    Vec3::new(hue, saturation, max)
}

fn hsv_to_rgb(hsv: Vec3) -> Vec3 {
    let h = hsv.x * 6.0;
    let s = hsv.y;
    let v = hsv.z;

    if s <= f32::EPSILON {
        return Vec3::splat(v);
    }

    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    match i as u32 % 6 {
        0 => Vec3::new(v, t, p),
        1 => Vec3::new(q, v, p),
        2 => Vec3::new(p, v, t),
        3 => Vec3::new(p, q, v),
        4 => Vec3::new(t, p, v),
        _ => Vec3::new(v, p, q),
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn surface_clear_and_draw() {
        let surface = SurfaceClass::new(4, 4, TextureFormat::Rgba8Unorm).unwrap();
        surface.draw_pixel(1, 1, [255, 0, 0, 255]).unwrap();
        let lock = surface.lock();
        assert_eq!(lock.pixels()[1 * lock.pitch() + 1 * 4], 255);
    }

    #[test]
    fn alpha_bounds_detection() {
        let surface = SurfaceClass::new(2, 2, TextureFormat::Rgba8Unorm).unwrap();
        surface.draw_pixel(1, 0, [10, 20, 30, 255]).unwrap();
        let bounds = surface.find_nonzero_alpha_bounds().unwrap();
        assert_eq!(bounds, Some(SurfaceRect::new(1, 0, 1, 1)));
    }

    #[test]
    fn writes_r5g6b5_pixel() {
        let surface = SurfaceClass::new(1, 1, TextureFormat::R5G6B5).unwrap();
        surface.draw_pixel(0, 0, [128, 64, 192, 255]).unwrap();
        let lock = surface.lock();
        let rgba = read_rgba_pixel(&lock.pixels()[0..2], TextureFormat::R5G6B5);
        assert!((rgba[0] as i32 - 128).abs() <= 8);
        assert!((rgba[1] as i32 - 64).abs() <= 8);
        assert!((rgba[2] as i32 - 192).abs() <= 8);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn writes_a1r5g5b5_pixel() {
        let surface = SurfaceClass::new(1, 1, TextureFormat::A1R5G5B5).unwrap();
        surface.draw_pixel(0, 0, [200, 100, 50, 255]).unwrap();
        let lock = surface.lock();
        let rgba = read_rgba_pixel(&lock.pixels()[0..2], TextureFormat::A1R5G5B5);
        assert_eq!(rgba[3], 255);
        assert!((rgba[0] as i32 - 200).abs() <= 12);
        assert!((rgba[1] as i32 - 100).abs() <= 12);
        assert!((rgba[2] as i32 - 50).abs() <= 12);
    }

    #[test]
    fn writes_a4r4g4b4_pixel() {
        let surface = SurfaceClass::new(1, 1, TextureFormat::A4R4G4B4).unwrap();
        surface.draw_pixel(0, 0, [255, 0, 128, 64]).unwrap();
        let lock = surface.lock();
        let rgba = read_rgba_pixel(&lock.pixels()[0..2], TextureFormat::A4R4G4B4);
        assert!((rgba[0] as i32 - 255).abs() <= 16);
        assert!(rgba[1] <= 16);
        assert!((rgba[2] as i32 - 128).abs() <= 16);
        assert!((rgba[3] as i32 - 64).abs() <= 16);
    }
}
