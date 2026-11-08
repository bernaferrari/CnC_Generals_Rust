//! Blit helpers (ported from WWLib blit.cpp/h).

use crate::blitter::{Blitter, RLEBlitter};
use crate::buff::Buffer;
use crate::point::Point2D;
use crate::surface::{Rect, Surface};

pub fn buffer_size(surface: &Surface, width: i32, height: i32) -> i32 {
    width * height * surface.bytes_per_pixel() as i32
}

pub fn to_buffer(surface: &Surface, rect: Rect, buffer: &mut Buffer) -> bool {
    if !rect.is_valid() {
        return false;
    }
    let Some(buf) = buffer.as_mut_slice() else {
        return false;
    };
    let bytes_per_pixel = surface.bytes_per_pixel();
    let stride = surface.stride();
    let mut offset = 0usize;
    let lock = match surface.lock(Point2D::new(0, 0)) {
        Ok(lock) => lock,
        Err(_) => return false,
    };

    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            // SAFETY: [Category 3 — dangling / Category 10 — OOB] pointer comes from the held SurfaceLock's bounds-checked get_pixel_ptr; exactly bytes_per_pixel are copied.
            unsafe {
                if let Some(ptr) = lock.get_pixel_ptr(Point2D::new(x, y)) {
                    let dest = &mut buf[offset..offset + bytes_per_pixel];
                    dest.copy_from_slice(std::slice::from_raw_parts(ptr, bytes_per_pixel));
                } else {
                    return false;
                }
            }
            offset += bytes_per_pixel;
        }
        let _ = stride;
    }

    true
}

pub fn from_buffer(surface: &mut Surface, rect: Rect, buffer: &Buffer) -> bool {
    if !rect.is_valid() {
        return false;
    }
    let Some(buf) = buffer.as_slice() else {
        return false;
    };
    let bytes_per_pixel = surface.bytes_per_pixel();
    let mut offset = 0usize;
    let lock = match surface.lock(Point2D::new(0, 0)) {
        Ok(lock) => lock,
        Err(_) => return false,
    };

    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            // SAFETY: [Category 3 — dangling / Category 10 — OOB] pointer comes from the held SurfaceLock's bounds-checked get_pixel_ptr; exactly bytes_per_pixel are copied.
            unsafe {
                if let Some(ptr) = lock.get_pixel_ptr(Point2D::new(x, y)) {
                    let src = &buf[offset..offset + bytes_per_pixel];
                    std::slice::from_raw_parts_mut(ptr, bytes_per_pixel).copy_from_slice(src);
                } else {
                    return false;
                }
            }
            offset += bytes_per_pixel;
        }
    }

    true
}

pub fn bit_blit(
    dest: &mut Surface,
    destrect: Rect,
    source: &Surface,
    sourcerect: Rect,
    blitter: &dyn Blitter,
) -> bool {
    let dcliprect = dest.get_rect();
    let scliprect = source.get_rect();
    bit_blit_clipped(
        dest, dcliprect, destrect, source, scliprect, sourcerect, blitter,
    )
}

/// Blit between distinct surfaces. For a self-copy use [`bit_blit_within`].
pub fn bit_blit_clipped(
    dest: &mut Surface,
    dcliprect: Rect,
    drect: Rect,
    source: &Surface,
    scliprect: Rect,
    srect: Rect,
    blitter: &dyn Blitter,
) -> bool {
    let Some((dx, dy, sx, sy, width, height)) =
        ordinary_geometry(dest, dcliprect, drect, source, scliprect, srect, blitter)
    else { return false };
    let (ds, ss) = (dest.stride(), source.stride());
    let (dp, sp) = (blitter.dest_pixel_size(), blitter.source_pixel_size());
    let source_lock = match source.lock(Point2D::new(0, 0)) {
        Ok(lock) => lock,
        Err(_) => return false,
    };
    let source_bytes = source_lock.bytes();
    let Some(dest_bytes) = dest.rle_buffer_mut() else { return false };
    for row in 0..height {
        let d = (dy + row) * ds + dx * dp;
        let s = (sy + row) * ss + sx * sp;
        if !blitter.blit_forward(&mut dest_bytes[d..d + width * dp],
                                 &source_bytes[s..s + width * sp], width) {
            return false;
        }
    }
    true
}

/// Blit between possibly overlapping rectangles of the same surface.
pub fn bit_blit_within(surface: &mut Surface, destrect: Rect, sourcerect: Rect,
                       blitter: &dyn Blitter) -> bool {
    let clip = surface.get_rect();
    bit_blit_within_clipped(surface, clip, destrect, clip, sourcerect, blitter)
}

/// Self-blit with separate source and destination clipping windows.
pub fn bit_blit_within_clipped(surface: &mut Surface, dcliprect: Rect, drect: Rect,
                               scliprect: Rect, srect: Rect, blitter: &dyn Blitter) -> bool {
    let Some((dx, dy, sx, sy, width, height)) =
        ordinary_geometry(surface, dcliprect, drect, surface, scliprect, srect, blitter)
    else { return false };
    let stride = surface.stride();
    let pixel = surface.bytes_per_pixel();
    let backward = dy > sy || (dy == sy && dx > sx);
    let Some(bytes) = surface.rle_buffer_mut() else { return false };
    for n in 0..height {
        let row = if backward { height - 1 - n } else { n };
        let d = (dy + row) * stride + dx * pixel;
        let s = (sy + row) * stride + sx * pixel;
        if !blitter.blit_within(bytes, d, s, width, backward) { return false; }
    }
    true
}

// Coordinates in the public clipping API are relative to their clipping windows.
fn ordinary_geometry(dest: &Surface, dclip: Rect, mut drect: Rect,
                     source: &Surface, sclip: Rect, mut srect: Rect,
                     blitter: &dyn Blitter) -> Option<(usize, usize, usize, usize, usize, usize)> {
    if !dclip.is_valid() || !sclip.is_valid() || !drect.is_valid() || !srect.is_valid()
        || dest.bytes_per_pixel() != blitter.dest_pixel_size()
        || (blitter.source_pixel_size() != 0 && source.bytes_per_pixel() != blitter.source_pixel_size()) {
        return None;
    }
    // Keep all intermediate arithmetic in the legacy clipping routine bounded.
    const LIMIT: i32 = i32::MAX / 8;
    for r in [dclip, sclip, drect, srect] {
        if [r.x, r.y, r.width, r.height].iter().any(|&v| v < -LIMIT || v > LIMIT) {
            return None;
        }
    }
    if !blit_clip(&mut drect, dclip, &mut srect, sclip) { return None; }
    let dx = usize::try_from(dclip.x.checked_add(drect.x)?).ok()?;
    let dy = usize::try_from(dclip.y.checked_add(drect.y)?).ok()?;
    let sx = usize::try_from(sclip.x.checked_add(srect.x)?).ok()?;
    let sy = usize::try_from(sclip.y.checked_add(srect.y)?).ok()?;
    let width = usize::try_from(drect.width.min(srect.width)).ok()?;
    let height = usize::try_from(drect.height.min(srect.height)).ok()?;
    if dx.checked_add(width)? > dest.get_width() as usize
        || dy.checked_add(height)? > dest.get_height() as usize
        || sx.checked_add(width)? > source.get_width() as usize
        || sy.checked_add(height)? > source.get_height() as usize {
        return None;
    }
    Some((dx, dy, sx, sy, width, height))
}

pub fn rle_blit(
    dest: &mut Surface,
    destrect: Rect,
    source: &Surface,
    sourcerect: Rect,
    blitter: &dyn RLEBlitter,
) -> bool {
    let dcliprect = dest.get_rect();
    let scliprect = source.get_rect();
    rle_blit_clipped(
        dest, dcliprect, destrect, source, scliprect, sourcerect, blitter,
    )
}

pub fn rle_blit_clipped(
    dest: &mut Surface,
    dcliprect: Rect,
    mut drect: Rect,
    source: &Surface,
    scliprect: Rect,
    mut srect: Rect,
    blitter: &dyn RLEBlitter,
) -> bool {
    if !blit_clip(&mut drect, dcliprect, &mut srect, scliprect) {
        return false;
    }

    let Some(leftmargin) = srect.x.checked_sub(scliprect.x).and_then(|v| usize::try_from(v).ok()) else {
        return false;
    };
    let Some(topmargin) = srect.y.checked_sub(scliprect.y).and_then(|v| usize::try_from(v).ok()) else {
        return false;
    };
    let (Some(x), Some(y), Ok(width), Ok(height)) = (
        dcliprect.x.checked_add(drect.x).and_then(|v| usize::try_from(v).ok()),
        dcliprect.y.checked_add(drect.y).and_then(|v| usize::try_from(v).ok()),
        usize::try_from(srect.width.min(drect.width)),
        usize::try_from(srect.height.min(drect.height)),
    ) else {
        return false;
    };
    let pixel_size = dest.bytes_per_pixel();
    if pixel_size != blitter.pixel_size() {
        return false;
    }
    let stride = dest.stride();
    let Some(x_offset) = x.checked_mul(pixel_size) else { return false };
    let Some(row_bytes) = width.checked_mul(pixel_size) else { return false };
    if x_offset.checked_add(row_bytes).map_or(true, |end| end > stride) {
        return false;
    }
    let source_lock = match source.lock(Point2D::new(0, 0)) {
        Ok(lock) => lock,
        Err(_) => return false,
    };
    let source_bytes = source_lock.bytes();
    let dest_bytes = match dest.rle_buffer_mut() {
        Some(bytes) => bytes,
        None => return false,
    };

    let mut source_offset = 0usize;
    for row in 0..topmargin.saturating_add(height) {
        let Some(header) = source_bytes.get(source_offset..source_offset.saturating_add(2)) else {
            return false;
        };
        let line_len = u16::from_ne_bytes([header[0], header[1]]) as usize;
        if line_len < 2 {
            return false;
        }
        let Some(end) = source_offset.checked_add(line_len) else { return false };
        let Some(line) = source_bytes.get(source_offset + 2..end) else { return false };
        if row >= topmargin {
            let Some(offset) = y.checked_add(row - topmargin)
                .and_then(|row| row.checked_mul(stride))
                .and_then(|offset| offset.checked_add(x_offset)) else { return false };
            let Some(end) = offset.checked_add(row_bytes) else { return false };
            let Some(target) = dest_bytes.get_mut(offset..end) else { return false };
            if !blitter.blit(target, line, width, leftmargin) {
                return false;
            }
        }
        source_offset = end;
    }
    true
}

fn blit_clip(drect: &mut Rect, dwindow: Rect, srect: &mut Rect, swindow: Rect) -> bool {
    if drect.width == srect.width && drect.height == srect.height {
        if drect.x < 0 {
            let delta = -drect.x;
            srect.x += delta;
            srect.width -= delta;
            drect.width -= delta;
            drect.x = 0;
        }
        if drect.y < 0 {
            let delta = -drect.y;
            srect.y += delta;
            srect.height -= delta;
            drect.height -= delta;
            drect.y = 0;
        }

        let rightspill = (drect.x + drect.width) - dwindow.width;
        if rightspill > 0 {
            srect.width -= rightspill;
            drect.width -= rightspill;
        }
        let bottomspill = (drect.y + drect.height) - dwindow.height;
        if bottomspill > 0 {
            srect.height -= bottomspill;
            drect.height -= bottomspill;
        }

        if srect.x < 0 {
            let delta = -srect.x;
            drect.x += delta;
            srect.width -= delta;
            drect.width -= delta;
            srect.x = 0;
        }
        if srect.y < 0 {
            let delta = -srect.y;
            drect.y += delta;
            srect.height -= delta;
            drect.height -= delta;
            srect.y = 0;
        }

        let rightspill = (srect.x + srect.width) - swindow.width;
        if rightspill > 0 {
            srect.width -= rightspill;
            drect.width -= rightspill;
        }
        let bottomspill = (srect.y + srect.height) - swindow.height;
        if bottomspill > 0 {
            srect.height -= bottomspill;
            drect.height -= bottomspill;
        }
    } else {
        if let Some(intersection) = drect.intersect(dwindow) {
            *drect = intersection;
        } else {
            return false;
        }
        if let Some(intersection) = srect.intersect(swindow) {
            *srect = intersection;
        } else {
            return false;
        }
    }

    drect.is_valid() && srect.is_valid()
}


#[cfg(test)]
mod rle_tests {
    use super::*;
    use crate::rlerle::RLEBlitTransXlat;
    use crate::surface::PixelFormat;


    #[test]
    fn ordinary_surface_blit_clips_and_self_overlap() {
        use crate::blitter::{BlitPlainU8, BlitTransU8};
        let mut source = Surface::new(4, 2, PixelFormat::Palette8).unwrap();
        source.rle_buffer_mut().unwrap().copy_from_slice(&[1, 2, 3, 4, 5, 0, 7, 8]);
        let mut dest = Surface::new(4, 2, PixelFormat::Palette8).unwrap();
        dest.rle_buffer_mut().unwrap().fill(9);
        assert!(bit_blit(&mut dest, Rect::new(-1, 0, 4, 2), &source,
                         Rect::new(0, 0, 4, 2), &BlitTransU8));
        assert_eq!(dest.rle_buffer_mut().unwrap(), &[2, 3, 4, 9, 9, 7, 8, 9]);
        assert!(bit_blit_within(&mut source, Rect::new(1, 0, 3, 2),
                                Rect::new(0, 0, 3, 2), &BlitPlainU8));
        assert_eq!(source.rle_buffer_mut().unwrap(), &[1, 1, 2, 3, 5, 5, 0, 7]);
        assert!(bit_blit_within(&mut source, Rect::new(0, 1, 4, 1),
                                Rect::new(0, 0, 4, 1), &BlitPlainU8));
        assert_eq!(source.rle_buffer_mut().unwrap(), &[1, 1, 2, 3, 1, 1, 2, 3]);
    }

    #[test]
    fn transparent_self_blit_preserves_pixels_in_both_directions() {
        use crate::blitter::BlitTransU8;
        let mut surface = Surface::new(5, 2, PixelFormat::Palette8).unwrap();
        surface.rle_buffer_mut().unwrap().copy_from_slice(&[1, 0, 2, 3, 4, 5, 6, 0, 7, 8]);
        assert!(bit_blit_within(&mut surface, Rect::new(1, 0, 4, 2),
                                Rect::new(0, 0, 4, 2), &BlitTransU8));
        assert_eq!(surface.rle_buffer_mut().unwrap(), &[1, 1, 2, 2, 3, 5, 5, 6, 7, 7]);
        assert!(bit_blit_within(&mut surface, Rect::new(0, 0, 4, 2),
                                Rect::new(1, 0, 4, 2), &BlitTransU8));
        assert_eq!(surface.rle_buffer_mut().unwrap(), &[1, 2, 2, 3, 3, 5, 6, 7, 7, 7]);
    }

    #[test]
    fn transparent_self_blit_uses_bottom_up_rows() {
        use crate::blitter::BlitTransU8;
        let mut surface = Surface::new(3, 3, PixelFormat::Palette8).unwrap();
        surface.rle_buffer_mut().unwrap().copy_from_slice(&[1, 0, 3, 4, 5, 0, 7, 8, 9]);
        assert!(bit_blit_within(&mut surface, Rect::new(0, 1, 3, 2),
                                Rect::new(0, 0, 3, 2), &BlitTransU8));
        assert_eq!(surface.rle_buffer_mut().unwrap(), &[1, 0, 3, 1, 5, 3, 4, 5, 9]);
    }

    #[test]
    fn clipped_windows_and_indexed_conversion_use_surface_pitch() {
        use crate::blitter::BlitTransXlat;
        let mut source = Surface::new(4, 2, PixelFormat::Palette8).unwrap();
        source.rle_buffer_mut().unwrap().copy_from_slice(&[0, 1, 2, 0, 0, 2, 1, 0]);
        let mut dest = Surface::new(5, 2, PixelFormat::RGB16).unwrap();
        let table = [0u16, 0x1234, 0x5678];
        assert!(bit_blit_clipped(&mut dest, Rect::new(1, 0, 3, 2), Rect::new(0, 0, 2, 2),
                                 &source, Rect::new(1, 0, 3, 2), Rect::new(0, 0, 2, 2),
                                 &BlitTransXlat::new(&table)));
        let bytes = dest.rle_buffer_mut().unwrap();
        assert_eq!(&bytes[2..6], &[0x1234u16.to_ne_bytes(), 0x5678u16.to_ne_bytes()].concat());
        assert_eq!(&bytes[12..16], &[0x5678u16.to_ne_bytes(), 0x1234u16.to_ne_bytes()].concat());
        assert_eq!(&bytes[6..12], &[0; 6]);
    }

    #[test]
    fn source_independent_darken_accepts_a_different_source_format() {
        use crate::blitter::BlitDarken;
        let source = Surface::new(1, 1, PixelFormat::RGBA32).unwrap();
        let mut dest = Surface::new(1, 1, PixelFormat::RGB16).unwrap();
        dest.rle_buffer_mut().unwrap().copy_from_slice(&0x6000u16.to_ne_bytes());
        assert!(bit_blit(&mut dest, Rect::new(0, 0, 1, 1), &source,
                         Rect::new(0, 0, 1, 1), &BlitDarken::new(0x7fffu16)));
        assert_eq!(dest.rle_buffer_mut().unwrap(), &0x3000u16.to_ne_bytes());
    }
    #[test]
    fn rle_surface_blit_clips_and_uses_line_lengths() {
        let mut source = Surface::new(4, 3, PixelFormat::Palette8).unwrap();
        let mut encoded = [0u8; 12];
        encoded[..2].copy_from_slice(&5u16.to_ne_bytes());
        encoded[2..5].copy_from_slice(&[1, 0, 3]);
        encoded[5..7].copy_from_slice(&6u16.to_ne_bytes());
        encoded[7..11].copy_from_slice(&[2, 0, 2, 3]);
        source.rle_buffer_mut().unwrap().copy_from_slice(&encoded);
        let mut dest = Surface::new(4, 2, PixelFormat::Palette8).unwrap();
        dest.rle_buffer_mut().unwrap().fill(99);
        let translate = [0u8, 11, 22, 33];
        assert!(rle_blit(&mut dest, Rect::new(-1, 0, 4, 2), &source,
                         Rect::new(0, 0, 4, 2), &RLEBlitTransXlat::new(&translate)));
        assert_eq!(dest.rle_buffer_mut().unwrap(), &[99, 99, 99, 99, 99, 99, 33, 99]);
    }

    #[test]
    fn rle_surface_blit_rejects_truncated_line_header() {
        let source = Surface::new(1, 1, PixelFormat::Palette8).unwrap();
        let mut dest = Surface::new(1, 1, PixelFormat::Palette8).unwrap();
        assert!(!rle_blit(&mut dest, Rect::new(0, 0, 1, 1), &source,
                          Rect::new(0, 0, 1, 1), &RLEBlitTransXlat::new(&[0u8; 256])));
    }
}
