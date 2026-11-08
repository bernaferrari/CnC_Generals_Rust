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

pub fn bit_blit_clipped(
    dest: &mut Surface,
    dcliprect: Rect,
    mut drect: Rect,
    source: &Surface,
    scliprect: Rect,
    mut srect: Rect,
    blitter: &dyn Blitter,
) -> bool {
    let mut overlapped = false;
    let dest_stride = dest.stride() as i32;
    let source_stride = source.stride() as i32;
    let dest_bpp = dest.bytes_per_pixel() as i32;
    let (dbuffer, sbuffer, drect, srect, lock_guard) = match prep_for_blit(
        dest,
        dcliprect,
        drect,
        source,
        scliprect,
        srect,
        &mut overlapped,
    ) {
        Some(data) => data,
        None => return false,
    };

    let _locks = lock_guard;

    if drect.width * dest_bpp == dest_stride && dest_stride == source_stride {
        let length = (srect.height * srect.width).min(drect.height * drect.width);
        if overlapped {
            blitter.blit_backward(dbuffer, sbuffer, length);
        } else {
            blitter.blit_forward(dbuffer, sbuffer, length);
        }
        return true;
    }

    let mut sstride = source_stride;
    let mut dstride = dest_stride;
    let mut sbuffer = sbuffer;
    let mut dbuffer = dbuffer;
    if overlapped {
        sstride = -sstride;
        dstride = -dstride;
        // SAFETY: [Category 10 — OOB] repositions to the last source/dest row within the locked surfaces before a backward overlap-safe pass.
        unsafe {
            sbuffer = sbuffer.offset(((srect.height - 1) * source_stride) as isize) as *mut u8;
            dbuffer = dbuffer.offset(((drect.height - 1) * dest_stride) as isize) as *mut u8;
        }
    }

    let height = srect.height.min(drect.height);
    for _ in 0..height {
        if overlapped {
            blitter.blit_backward(dbuffer, sbuffer, srect.width);
        } else {
            blitter.blit_forward(dbuffer, sbuffer, srect.width);
        }
        // SAFETY: [Category 10 — OOB] repositions to the last source/dest row within the locked surfaces before a backward overlap-safe pass.
        unsafe {
            dbuffer = dbuffer.offset(dstride as isize) as *mut u8;
            sbuffer = sbuffer.offset(sstride as isize);
        }
    }

    true
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

struct PrepLocks<'a> {
    _dest_lock: crate::surface::SurfaceLock<'a>,
    _source_lock: crate::surface::SurfaceLock<'a>,
}

fn prep_for_blit<'a>(
    dest: &'a mut Surface,
    dcliprect: Rect,
    mut drect: Rect,
    source: &'a Surface,
    scliprect: Rect,
    mut srect: Rect,
    overlapped: &mut bool,
) -> Option<(*mut u8, *const u8, Rect, Rect, PrepLocks<'a>)> {
    *overlapped = false;
    if !drect.is_valid() || !dcliprect.is_valid() || !srect.is_valid() || !scliprect.is_valid() {
        return None;
    }

    if !blit_clip(&mut drect, dcliprect, &mut srect, scliprect) {
        return None;
    }

    if std::ptr::eq(dest, source) && srect.overlaps(drect) {
        if srect.y < drect.y || (srect.y == drect.y && srect.x < drect.x) {
            *overlapped = true;
        }
    }

    let dest_lock = dest.lock(Point2D::new(0, 0)).ok()?;
    let source_lock = source.lock(Point2D::new(0, 0)).ok()?;
    let dpoint = Point2D::new(dcliprect.x + drect.x, dcliprect.y + drect.y);
    let spoint = Point2D::new(scliprect.x + srect.x, scliprect.y + srect.y);
    // SAFETY: [Category 3 — dangling / Category 10 — OOB] pointer comes from the held SurfaceLock's bounds-checked get_pixel_ptr; exactly bytes_per_pixel are copied.
    let dbuffer = unsafe { dest_lock.get_pixel_ptr(dpoint)? };
    // SAFETY: [Category 3 — dangling / Category 10 — OOB] pointer comes from the held SurfaceLock's bounds-checked get_pixel_ptr; exactly bytes_per_pixel are copied.
    let sbuffer = unsafe { source_lock.get_pixel_ptr(spoint)? };

    Some((
        dbuffer,
        sbuffer as *const u8,
        drect,
        srect,
        PrepLocks {
            _dest_lock: dest_lock,
            _source_lock: source_lock,
        },
    ))
}

#[cfg(test)]
mod rle_tests {
    use super::*;
    use crate::rlerle::RLEBlitTransXlat;
    use crate::surface::PixelFormat;

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
