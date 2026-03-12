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

    let leftmargin = srect.x - scliprect.x;
    let mut topmargin = srect.y - scliprect.y;

    let dpoint = Point2D::new(dcliprect.x + drect.x, dcliprect.y + drect.y);
    let mut dbuffer = match dest.lock(Point2D::new(0, 0)) {
        Ok(lock) => lock,
        Err(_) => return false,
    };
    let mut sbuffer = match source.lock(Point2D::new(0, 0)) {
        Ok(lock) => lock,
        Err(_) => return false,
    };

    let mut dptr = unsafe {
        dbuffer
            .get_pixel_ptr(dpoint)
            .unwrap_or(std::ptr::null_mut())
    };
    if dptr.is_null() {
        return false;
    }

    let mut sptr = unsafe { sbuffer.data_ptr() };
    if sptr.is_null() {
        return false;
    }

    unsafe {
        while topmargin > 0 {
            let line_len = *(sptr as *const u16) as i32;
            sptr = sptr.add(line_len as usize);
            topmargin -= 1;
        }
    }

    let dstride = dest.stride() as i32;
    let height = srect.height.min(drect.height);
    for _ in 0..height {
        unsafe {
            let line_len = *(sptr as *const u16) as i32;
            let line_data = sptr.add(2);
            blitter.blit(dptr, line_data, srect.width, leftmargin);
            sptr = sptr.add(line_len as usize);
            dptr = dptr.offset(dstride as isize);
        }
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
    let dbuffer = unsafe { dest_lock.get_pixel_ptr(dpoint)? };
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
