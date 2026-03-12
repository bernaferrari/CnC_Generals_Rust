//! Blitter interfaces and implementations (ported from WWLib blitter.h/blitblit.h).

use std::ptr;

pub trait Blitter {
    fn blit_forward(&self, dest: *mut u8, source: *const u8, length: i32);
    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32);
}

pub trait RLEBlitter {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32);
}

pub struct BlitPlainU8;
pub struct BlitPlainU16;
pub struct BlitPlainU32;

impl Blitter for BlitPlainU8 {
    fn blit_forward(&self, dest: *mut u8, source: *const u8, length: i32) {
        unsafe {
            ptr::copy_nonoverlapping(source, dest, length as usize);
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        unsafe {
            ptr::copy(source, dest, length as usize);
        }
    }
}

impl Blitter for BlitPlainU16 {
    fn blit_forward(&self, dest: *mut u8, source: *const u8, length: i32) {
        unsafe {
            ptr::copy_nonoverlapping(source as *const u16, dest as *mut u16, length as usize);
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        unsafe {
            ptr::copy(source as *const u16, dest as *mut u16, length as usize);
        }
    }
}

impl Blitter for BlitPlainU32 {
    fn blit_forward(&self, dest: *mut u8, source: *const u8, length: i32) {
        unsafe {
            ptr::copy_nonoverlapping(source as *const u32, dest as *mut u32, length as usize);
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        unsafe {
            ptr::copy(source as *const u32, dest as *mut u32, length as usize);
        }
    }
}

pub struct BlitTransU8;
pub struct BlitTransU16;
pub struct BlitTransU32;

impl Blitter for BlitTransU8 {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    *dest = color;
                }
                dest = dest.add(1);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl Blitter for BlitTransU16 {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *(source as *const u16);
                if color != 0 {
                    *(dest as *mut u16) = color;
                }
                dest = dest.add(2);
                source = source.add(2);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl Blitter for BlitTransU32 {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *(source as *const u32);
                if color != 0 {
                    *(dest as *mut u32) = color;
                }
                dest = dest.add(4);
                source = source.add(4);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

pub struct BlitPlainXlat<'a, T> {
    translate: &'a [T],
}

impl<'a, T> BlitPlainXlat<'a, T> {
    pub fn new(translate: &'a [T]) -> Self {
        Self { translate }
    }
}

impl<'a> Blitter for BlitPlainXlat<'a, u8> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                *dest = self.translate[color as usize];
                dest = dest.add(1);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitPlainXlat<'a, u16> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source as usize;
                *(dest as *mut u16) = self.translate[color];
                dest = dest.add(2);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitPlainXlat<'a, u32> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source as usize;
                *(dest as *mut u32) = self.translate[color];
                dest = dest.add(4);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

pub struct BlitTransXlat<'a, T> {
    translate: &'a [T],
}

impl<'a, T> BlitTransXlat<'a, T> {
    pub fn new(translate: &'a [T]) -> Self {
        Self { translate }
    }
}

impl<'a> Blitter for BlitTransXlat<'a, u8> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    *dest = self.translate[color as usize];
                }
                dest = dest.add(1);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransXlat<'a, u16> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    *(dest as *mut u16) = self.translate[color as usize];
                }
                dest = dest.add(2);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransXlat<'a, u32> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    *(dest as *mut u32) = self.translate[color as usize];
                }
                dest = dest.add(4);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

pub struct BlitTransRemapXlat<'a, T> {
    remap: &'a [u8],
    translate: &'a [T],
}

impl<'a, T> BlitTransRemapXlat<'a, T> {
    pub fn new(remap: &'a [u8], translate: &'a [T]) -> Self {
        Self { remap, translate }
    }
}

impl<'a> Blitter for BlitTransRemapXlat<'a, u8> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let mapped = self.remap[color as usize] as usize;
                    *dest = self.translate[mapped];
                }
                dest = dest.add(1);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransRemapXlat<'a, u16> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let mapped = self.remap[color as usize] as usize;
                    *(dest as *mut u16) = self.translate[mapped];
                }
                dest = dest.add(2);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransRemapXlat<'a, u32> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let mapped = self.remap[color as usize] as usize;
                    *(dest as *mut u32) = self.translate[mapped];
                }
                dest = dest.add(4);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

pub struct BlitTransZRemapXlat<'a, T> {
    remap: &'a [u8],
    translate: &'a [T],
}

pub struct BlitTransDarken<T> {
    mask: T,
}

impl<T> BlitTransDarken<T> {
    pub fn new(mask: T) -> Self {
        Self { mask }
    }
}

impl Blitter for BlitTransDarken<u16> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let value = *(dest as *mut u16);
                    *(dest as *mut u16) = (value >> 1) & self.mask;
                }
                dest = dest.add(2);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl Blitter for BlitTransDarken<u32> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let value = *(dest as *mut u32);
                    *(dest as *mut u32) = (value >> 1) & self.mask;
                }
                dest = dest.add(4);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

pub struct BlitTransRemapDest<'a, T> {
    remap: &'a [T],
}

impl<'a, T> BlitTransRemapDest<'a, T> {
    pub fn new(remap: &'a [T]) -> Self {
        Self { remap }
    }
}

impl<'a> Blitter for BlitTransRemapDest<'a, u8> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let idx = *dest as usize;
                    *dest = self.remap[idx];
                }
                dest = dest.add(1);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransRemapDest<'a, u16> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let idx = *(dest as *mut u16) as usize;
                    *(dest as *mut u16) = self.remap[idx];
                }
                dest = dest.add(2);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransRemapDest<'a, u32> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let idx = *(dest as *mut u32) as usize;
                    *(dest as *mut u32) = self.remap[idx];
                }
                dest = dest.add(4);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

pub struct BlitDarken<T> {
    mask: T,
}

impl<T> BlitDarken<T> {
    pub fn new(mask: T) -> Self {
        Self { mask }
    }
}

impl Blitter for BlitDarken<u16> {
    fn blit_forward(&self, mut dest: *mut u8, _source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let value = *(dest as *mut u16);
                *(dest as *mut u16) = (value >> 1) & self.mask;
                dest = dest.add(2);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl Blitter for BlitDarken<u32> {
    fn blit_forward(&self, mut dest: *mut u8, _source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let value = *(dest as *mut u32);
                *(dest as *mut u32) = (value >> 1) & self.mask;
                dest = dest.add(4);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

pub struct BlitTransLucent50<'a, T> {
    translate: &'a [T],
    mask: T,
}

impl<'a, T> BlitTransLucent50<'a, T> {
    pub fn new(translate: &'a [T], mask: T) -> Self {
        Self { translate, mask }
    }
}

impl<'a> Blitter for BlitTransLucent50<'a, u16> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let dst = (*(dest as *mut u16) >> 1) & self.mask;
                    let src = (self.translate[color as usize] >> 1) & self.mask;
                    *(dest as *mut u16) = dst + src;
                }
                dest = dest.add(2);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransLucent50<'a, u32> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let dst = (*(dest as *mut u32) >> 1) & self.mask;
                    let src = (self.translate[color as usize] >> 1) & self.mask;
                    *(dest as *mut u32) = dst + src;
                }
                dest = dest.add(4);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

pub struct BlitTransLucent25<'a, T> {
    translate: &'a [T],
    mask: T,
}

impl<'a, T> BlitTransLucent25<'a, T> {
    pub fn new(translate: &'a [T], mask: T) -> Self {
        Self { translate, mask }
    }
}

impl<'a> Blitter for BlitTransLucent25<'a, u16> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let qsource = (self.translate[color as usize] >> 2) & self.mask;
                    let qdest = (*(dest as *mut u16) >> 2) & self.mask;
                    *(dest as *mut u16) = qdest + qsource + qsource + qsource;
                }
                dest = dest.add(2);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransLucent25<'a, u32> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let qsource = (self.translate[color as usize] >> 2) & self.mask;
                    let qdest = (*(dest as *mut u32) >> 2) & self.mask;
                    *(dest as *mut u32) = qdest + qsource + qsource + qsource;
                }
                dest = dest.add(4);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

pub struct BlitTransLucent75<'a, T> {
    translate: &'a [T],
    mask: T,
}

impl<'a, T> BlitTransLucent75<'a, T> {
    pub fn new(translate: &'a [T], mask: T) -> Self {
        Self { translate, mask }
    }
}

impl<'a> Blitter for BlitTransLucent75<'a, u16> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let qsource = (self.translate[color as usize] >> 2) & self.mask;
                    let qdest = (*(dest as *mut u16) >> 2) & self.mask;
                    *(dest as *mut u16) = qdest + qdest + qdest + qsource;
                }
                dest = dest.add(2);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransLucent75<'a, u32> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let qsource = (self.translate[color as usize] >> 2) & self.mask;
                    let qdest = (*(dest as *mut u32) >> 2) & self.mask;
                    *(dest as *mut u32) = qdest + qdest + qdest + qsource;
                }
                dest = dest.add(4);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a, T> BlitTransZRemapXlat<'a, T> {
    pub fn new(remap: &'a [u8], translate: &'a [T]) -> Self {
        Self { remap, translate }
    }
}

impl<'a> Blitter for BlitTransZRemapXlat<'a, u8> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        let remap = self.remap;
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let mapped = remap[color as usize] as usize;
                    *dest = self.translate[mapped];
                }
                dest = dest.add(1);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransZRemapXlat<'a, u16> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        let remap = self.remap;
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let mapped = remap[color as usize] as usize;
                    *(dest as *mut u16) = self.translate[mapped];
                }
                dest = dest.add(2);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}

impl<'a> Blitter for BlitTransZRemapXlat<'a, u32> {
    fn blit_forward(&self, mut dest: *mut u8, mut source: *const u8, length: i32) {
        let remap = self.remap;
        for _ in 0..length {
            unsafe {
                let color = *source;
                if color != 0 {
                    let mapped = remap[color as usize] as usize;
                    *(dest as *mut u32) = self.translate[mapped];
                }
                dest = dest.add(4);
                source = source.add(1);
            }
        }
    }

    fn blit_backward(&self, dest: *mut u8, source: *const u8, length: i32) {
        self.blit_forward(dest, source, length);
    }
}
