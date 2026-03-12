//! RLE blitter implementations (ported from WWLib rlerle.h).

use crate::blitter::RLEBlitter;

fn skip_leading_pixels(mut sptr: *const u8, skipper: i32) -> (*const u8, i32) {
    let mut skip = skipper;
    unsafe {
        while skip > 0 {
            let value = *sptr;
            sptr = sptr.add(1);
            if value == 0 {
                let run = *sptr as i32;
                sptr = sptr.add(1);
                skip -= run;
            } else {
                skip -= 1;
            }
        }
    }
    (sptr, -skip)
}

pub struct RLEBlitTransXlat<'a, T> {
    translate: &'a [T],
}

impl<'a, T> RLEBlitTransXlat<'a, T> {
    pub fn new(translate: &'a [T]) -> Self {
        Self { translate }
    }
}

impl<'a> RLEBlitter for RLEBlitTransXlat<'a, u16> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u16;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    *dptr = self.translate[value as usize];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransXlat<'a, u32> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u32;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    *dptr = self.translate[value as usize];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransXlat<'a, u8> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            unsafe {
                dptr = dptr.add(transcount as usize);
            }
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    *dptr = self.translate[value as usize];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

pub struct RLEBlitTransRemapXlat<'a, T> {
    remap: &'a [u8],
    translate: &'a [T],
}

impl<'a, T> RLEBlitTransRemapXlat<'a, T> {
    pub fn new(remap: &'a [u8], translate: &'a [T]) -> Self {
        Self { remap, translate }
    }
}

impl<'a> RLEBlitter for RLEBlitTransRemapXlat<'a, u16> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u16;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let mapped = self.remap[value as usize] as usize;
                    *dptr = self.translate[mapped];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransRemapXlat<'a, u32> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u32;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let mapped = self.remap[value as usize] as usize;
                    *dptr = self.translate[mapped];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransRemapXlat<'a, u8> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            unsafe {
                dptr = dptr.add(transcount as usize);
            }
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let mapped = self.remap[value as usize] as usize;
                    *dptr = self.translate[mapped];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

pub struct RLEBlitTransZRemapXlat<'a, T> {
    remap: &'a [u8],
    translate: &'a [T],
}

impl<'a, T> RLEBlitTransZRemapXlat<'a, T> {
    pub fn new(remap: &'a [u8], translate: &'a [T]) -> Self {
        Self { remap, translate }
    }
}

impl<'a> RLEBlitter for RLEBlitTransZRemapXlat<'a, u16> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u16;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let mapped = self.remap[value as usize] as usize;
                    *dptr = self.translate[mapped];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransZRemapXlat<'a, u32> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u32;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let mapped = self.remap[value as usize] as usize;
                    *dptr = self.translate[mapped];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransZRemapXlat<'a, u8> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            unsafe {
                dptr = dptr.add(transcount as usize);
            }
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let mapped = self.remap[value as usize] as usize;
                    *dptr = self.translate[mapped];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

pub struct RLEBlitTransRemapDest<'a, T> {
    remap: &'a [T],
}

impl<'a, T> RLEBlitTransRemapDest<'a, T> {
    pub fn new(remap: &'a [T]) -> Self {
        Self { remap }
    }
}

impl<'a> RLEBlitter for RLEBlitTransRemapDest<'a, u16> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u16;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let idx = *dptr as usize;
                    *dptr = self.remap[idx];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransRemapDest<'a, u32> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u32;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let idx = *dptr as usize;
                    *dptr = self.remap[idx];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransRemapDest<'a, u8> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            unsafe {
                dptr = dptr.add(transcount as usize);
            }
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let idx = *dptr as usize;
                    *dptr = self.remap[idx];
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

pub struct RLEBlitTransDarken<T> {
    mask: T,
}

impl<T> RLEBlitTransDarken<T> {
    pub fn new(mask: T) -> Self {
        Self { mask }
    }
}

impl RLEBlitter for RLEBlitTransDarken<u16> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u16;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    *dptr = ((*dptr >> 1) & self.mask) as u16;
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl RLEBlitter for RLEBlitTransDarken<u32> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u32;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    *dptr = ((*dptr >> 1) & self.mask) as u32;
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

pub struct RLEBlitTransLucent50<'a, T> {
    translate: &'a [T],
    mask: T,
}

impl<'a, T> RLEBlitTransLucent50<'a, T> {
    pub fn new(translate: &'a [T], mask: T) -> Self {
        Self { translate, mask }
    }
}

impl<'a> RLEBlitter for RLEBlitTransLucent50<'a, u16> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u16;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let dst = (*dptr >> 1) & self.mask;
                    let src = (self.translate[value as usize] >> 1) & self.mask;
                    *dptr = (dst + src) as u16;
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransLucent50<'a, u32> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u32;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let dst = (*dptr >> 1) & self.mask;
                    let src = (self.translate[value as usize] >> 1) & self.mask;
                    *dptr = (dst + src) as u32;
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

pub struct RLEBlitTransLucent25<'a, T> {
    translate: &'a [T],
    mask: T,
}

impl<'a, T> RLEBlitTransLucent25<'a, T> {
    pub fn new(translate: &'a [T], mask: T) -> Self {
        Self { translate, mask }
    }
}

impl<'a> RLEBlitter for RLEBlitTransLucent25<'a, u16> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u16;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let qsource = (self.translate[value as usize] >> 2) & self.mask;
                    let qdest = (*dptr >> 2) & self.mask;
                    *dptr = (qdest + qsource + qsource + qsource) as u16;
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransLucent25<'a, u32> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u32;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let qsource = (self.translate[value as usize] >> 2) & self.mask;
                    let qdest = (*dptr >> 2) & self.mask;
                    *dptr = (qdest + qsource + qsource + qsource) as u32;
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

pub struct RLEBlitTransLucent75<'a, T> {
    translate: &'a [T],
    mask: T,
}

impl<'a, T> RLEBlitTransLucent75<'a, T> {
    pub fn new(translate: &'a [T], mask: T) -> Self {
        Self { translate, mask }
    }
}

impl<'a> RLEBlitter for RLEBlitTransLucent75<'a, u16> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u16;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let qsource = (self.translate[value as usize] >> 2) & self.mask;
                    let qdest = (*dptr >> 2) & self.mask;
                    *dptr = (qdest + qdest + qdest + qsource) as u16;
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}

impl<'a> RLEBlitter for RLEBlitTransLucent75<'a, u32> {
    fn blit(&self, dest: *mut u8, source: *const u8, length: i32, leadskip: i32) {
        let mut sptr = source;
        let mut dptr = dest as *mut u32;
        let mut remaining = length;
        if leadskip > 0 {
            let (new_sptr, transcount) = skip_leading_pixels(sptr, leadskip);
            sptr = new_sptr;
            dptr = unsafe { dptr.add(transcount as usize) };
            remaining -= transcount;
        }

        unsafe {
            while remaining > 0 {
                let value = *sptr;
                sptr = sptr.add(1);
                if value == 0 {
                    let run = *sptr as i32;
                    sptr = sptr.add(1);
                    remaining -= run;
                    dptr = dptr.add(run as usize);
                } else {
                    let qsource = (self.translate[value as usize] >> 2) & self.mask;
                    let qdest = (*dptr >> 2) & self.mask;
                    *dptr = (qdest + qdest + qdest + qsource) as u32;
                    dptr = dptr.add(1);
                    remaining -= 1;
                }
            }
        }
    }
}
