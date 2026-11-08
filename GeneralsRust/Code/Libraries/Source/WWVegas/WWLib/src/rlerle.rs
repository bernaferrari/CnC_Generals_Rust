//! Bounded RLE blitters (ported from WWLib rlerle.h).
//!
//! A nonzero byte is a palette index; zero followed by a count represents
//! that many transparent pixels. Pixels are stored in native byte order, as
//! in the original typed C++ stores, without requiring aligned destinations.

use crate::blitter::RLEBlitter;

trait Pixel: Copy {
    const SIZE: usize;
    fn read(bytes: &[u8]) -> Self;
    fn write(self, bytes: &mut [u8]);
    fn index(self) -> usize;
}

macro_rules! pixel {
    ($t:ty, $n:expr) => {
        impl Pixel for $t {
            const SIZE: usize = $n;
            fn read(bytes: &[u8]) -> Self {
                <$t>::from_ne_bytes(bytes.try_into().unwrap())
            }
            fn write(self, bytes: &mut [u8]) {
                bytes.copy_from_slice(&self.to_ne_bytes());
            }
            fn index(self) -> usize { self as usize }
        }
    };
}
pixel!(u8, 1);
pixel!(u16, 2);
pixel!(u32, 4);

// The source is one bounded compressed row. Decoding stops at the requested
// clipped width; a transparent run may extend beyond either clipping edge.
fn decode<P: Pixel>(dest: &mut [u8], source: &[u8], length: usize, leadskip: usize,
                    mut pixel: impl FnMut(u8, P) -> Option<P>) -> bool {
    let Some(bytes) = length.checked_mul(P::SIZE) else { return false };
    let Some(dest) = dest.get_mut(..bytes) else { return false };
    let mut input = source.iter().copied();
    let mut skip = leadskip;
    let mut position = 0usize;
    while position < length || skip > 0 {
        let Some(value) = input.next() else { return false };
        if value == 0 {
            let Some(run) = input.next() else { return false };
            let run = run as usize;
            if run <= skip {
                skip -= run;
            } else {
                position = position.saturating_add(run - skip).min(length);
                skip = 0;
            }
        } else if skip > 0 {
            skip -= 1;
        } else {
            let offset = position * P::SIZE;
            let out = &mut dest[offset..offset + P::SIZE];
            let Some(value) = pixel(value, P::read(out)) else { return false };
            value.write(out);
            position += 1;
        }
    }
    true
}

pub struct RLEBlitTransXlat<'a, T> { translate: &'a [T] }
impl<'a, T> RLEBlitTransXlat<'a, T> {
    pub fn new(translate: &'a [T]) -> Self { Self { translate } }
}

pub struct RLEBlitTransRemapXlat<'a, T> { remap: &'a [u8], translate: &'a [T] }
impl<'a, T> RLEBlitTransRemapXlat<'a, T> {
    pub fn new(remap: &'a [u8], translate: &'a [T]) -> Self { Self { remap, translate } }
}

pub struct RLEBlitTransZRemapXlat<'a, T> { remap: &'a [u8], translate: &'a [T] }
impl<'a, T> RLEBlitTransZRemapXlat<'a, T> {
    pub fn new(remap: &'a [u8], translate: &'a [T]) -> Self { Self { remap, translate } }
}

pub struct RLEBlitTransRemapDest<'a, T> { remap: &'a [T] }
impl<'a, T> RLEBlitTransRemapDest<'a, T> {
    pub fn new(remap: &'a [T]) -> Self { Self { remap } }
}

pub struct RLEBlitTransDarken<T> { mask: T }
impl<T> RLEBlitTransDarken<T> {
    pub fn new(mask: T) -> Self { Self { mask } }
}

pub struct RLEBlitTransLucent50<'a, T> { translate: &'a [T], mask: T }
impl<'a, T> RLEBlitTransLucent50<'a, T> {
    pub fn new(translate: &'a [T], mask: T) -> Self { Self { translate, mask } }
}
pub struct RLEBlitTransLucent25<'a, T> { translate: &'a [T], mask: T }
impl<'a, T> RLEBlitTransLucent25<'a, T> {
    pub fn new(translate: &'a [T], mask: T) -> Self { Self { translate, mask } }
}
pub struct RLEBlitTransLucent75<'a, T> { translate: &'a [T], mask: T }
impl<'a, T> RLEBlitTransLucent75<'a, T> {
    pub fn new(translate: &'a [T], mask: T) -> Self { Self { translate, mask } }
}

macro_rules! base_blitters {
    ($t:ty) => {
        impl RLEBlitter for RLEBlitTransXlat<'_, $t> {
            fn pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit(&self, dest: &mut [u8], source: &[u8], length: usize, leadskip: usize) -> bool {
                decode::<$t>(dest, source, length, leadskip, |v, _| self.translate.get(v as usize).copied())
            }
        }
        impl RLEBlitter for RLEBlitTransRemapXlat<'_, $t> {
            fn pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit(&self, dest: &mut [u8], source: &[u8], length: usize, leadskip: usize) -> bool {
                decode::<$t>(dest, source, length, leadskip, |v, _| {
                    self.remap.get(v as usize).and_then(|&i| self.translate.get(i as usize)).copied()
                })
            }
        }
        impl RLEBlitter for RLEBlitTransZRemapXlat<'_, $t> {
            fn pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit(&self, dest: &mut [u8], source: &[u8], length: usize, leadskip: usize) -> bool {
                decode::<$t>(dest, source, length, leadskip, |v, _| {
                    self.remap.get(v as usize).and_then(|&i| self.translate.get(i as usize)).copied()
                })
            }
        }
        impl RLEBlitter for RLEBlitTransRemapDest<'_, $t> {
            fn pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit(&self, dest: &mut [u8], source: &[u8], length: usize, leadskip: usize) -> bool {
                decode::<$t>(dest, source, length, leadskip, |_, d| self.remap.get(d.index()).copied())
            }
        }
    }
}
base_blitters!(u8);
base_blitters!(u16);
base_blitters!(u32);

macro_rules! color_blitters {
    ($t:ty) => {
        impl RLEBlitter for RLEBlitTransDarken<$t> {
            fn pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit(&self, dest: &mut [u8], source: &[u8], length: usize, leadskip: usize) -> bool {
                decode::<$t>(dest, source, length, leadskip, |_, d| Some((d >> 1) & self.mask))
            }
        }
        impl RLEBlitter for RLEBlitTransLucent50<'_, $t> {
            fn pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit(&self, dest: &mut [u8], source: &[u8], length: usize, leadskip: usize) -> bool {
                decode::<$t>(dest, source, length, leadskip, |v, d| {
                    self.translate.get(v as usize).map(|&s| ((d >> 1) & self.mask).wrapping_add((s >> 1) & self.mask))
                })
            }
        }
        impl RLEBlitter for RLEBlitTransLucent25<'_, $t> {
            fn pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit(&self, dest: &mut [u8], source: &[u8], length: usize, leadskip: usize) -> bool {
                decode::<$t>(dest, source, length, leadskip, |v, d| {
                    self.translate.get(v as usize).map(|&s| {
                        let s = (s >> 2) & self.mask;
                        ((d >> 2) & self.mask).wrapping_add(s).wrapping_add(s).wrapping_add(s)
                    })
                })
            }
        }
        impl RLEBlitter for RLEBlitTransLucent75<'_, $t> {
            fn pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit(&self, dest: &mut [u8], source: &[u8], length: usize, leadskip: usize) -> bool {
                decode::<$t>(dest, source, length, leadskip, |v, d| {
                    self.translate.get(v as usize).map(|&s| {
                        let d = (d >> 2) & self.mask;
                        d.wrapping_add(d).wrapping_add(d).wrapping_add((s >> 2) & self.mask)
                    })
                })
            }
        }
    }
}
color_blitters!(u16);
color_blitters!(u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_runs_are_clipped_at_both_edges() {
        let table = [0u8, 11, 22, 33];
        let mut dest = [99; 5];
        assert!(RLEBlitTransXlat::new(&table).blit(&mut dest, &[1, 0, 3, 2, 3, 0, 8], 5, 2));
        assert_eq!(dest, [99, 99, 22, 33, 99]);
    }

    #[test]
    fn translation_and_remap_write_unaligned_native_pixels() {
        let mut table = [0u16; 4];
        table[3] = 0x1234;
        let remap = [0, 3, 0, 0];
        let mut dest = [0xaa; 5];
        assert!(RLEBlitTransRemapXlat::new(&remap, &table).blit(&mut dest[1..], &[1, 0, 1], 2, 0));
        assert_eq!(&dest[1..3], &0x1234u16.to_ne_bytes());
        assert_eq!(&dest[3..], &[0xaa, 0xaa]);
    }
    #[test]
    fn destination_remap_and_lucency_apply_only_to_opaque_pixels() {
        let remap = [8u8, 7, 6, 5];
        let mut indexed = [1, 2, 3];
        assert!(RLEBlitTransRemapDest::new(&remap).blit(&mut indexed, &[1, 0, 1, 2], 3, 0));
        assert_eq!(indexed, [7, 2, 5]);

        let table = [0u16, 0x4000];
        let mut rgb = [0u8; 4];
        rgb[..2].copy_from_slice(&0x2000u16.to_ne_bytes());
        rgb[2..].copy_from_slice(&0x6000u16.to_ne_bytes());
        assert!(RLEBlitTransLucent50::new(&table, 0x7fff).blit(&mut rgb, &[1, 0, 1], 2, 0));
        assert_eq!(&rgb[..2], &0x3000u16.to_ne_bytes());
        assert_eq!(&rgb[2..], &0x6000u16.to_ne_bytes());
    }


    #[test]
    fn truncated_rows_and_short_tables_fail_without_out_of_bounds_access() {
        let mut dest = [9u8; 3];
        let blitter = RLEBlitTransXlat::new(&[0u8, 7]);
        assert!(!blitter.blit(&mut dest, &[1, 0], 3, 0));
        assert_eq!(dest, [7, 9, 9]);
        assert!(!blitter.blit(&mut dest, &[2], 1, 0));
        assert!(!blitter.blit(&mut dest[..1], &[1, 1], 2, 0));
        assert!(!blitter.blit(&mut dest, &[0, 0], 1, 0));
    }
}
