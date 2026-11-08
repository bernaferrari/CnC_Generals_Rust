//! Bounded blitter interfaces and implementations (ported from WWLib blitter.h).
//!
//! Multibyte pixels use native byte order, matching the original typed stores.

/// A row blitter. Invalid row lengths and short lookup tables return false;
/// pixels written before a short lookup table is encountered remain written.
pub trait Blitter {
    /// Number of source bytes per pixel, or zero when the source is unused.
    fn source_pixel_size(&self) -> usize;
    fn dest_pixel_size(&self) -> usize;
    /// Process a single complete pixel. Returns false for an invalid table entry.
    fn blit_pixel(&self, dest: &mut [u8], source: &[u8]) -> bool;

    fn blit_forward(&self, dest: &mut [u8], source: &[u8], length: usize) -> bool {
        self.blit_row(dest, source, length, false)
    }

    fn blit_backward(&self, dest: &mut [u8], source: &[u8], length: usize) -> bool {
        self.blit_row(dest, source, length, true)
    }

    #[doc(hidden)]
    fn blit_row(&self, dest: &mut [u8], source: &[u8], length: usize, backward: bool) -> bool {
        let (s, d) = (self.source_pixel_size(), self.dest_pixel_size());
        if d == 0 || length.checked_mul(s).map_or(true, |n| n > source.len())
            || length.checked_mul(d).map_or(true, |n| n > dest.len()) {
            return false;
        }
        for n in 0..length {
            let i = if backward { length - 1 - n } else { n };
            if !self.blit_pixel(&mut dest[i*d..(i+1)*d], &source[i*s..(i+1)*s]) {
                return false;
            }
        }
        true
    }

    /// Blit regions of one buffer in the indicated direction, including overlap.
    /// The source pixel is read before the destination pixel is modified.
    fn blit_within(&self, buffer: &mut [u8], dest: usize, source: usize, length: usize, backward: bool) -> bool {
        let (s, d) = (self.source_pixel_size(), self.dest_pixel_size());
        if s > 4 || d == 0
            || length.checked_mul(s).and_then(|n| source.checked_add(n)).map_or(true, |end| end > buffer.len())
            || length.checked_mul(d).and_then(|n| dest.checked_add(n)).map_or(true, |end| end > buffer.len()) {
            return false;
        }
        for n in 0..length {
            let i = if backward { length - 1 - n } else { n };
            let mut pixel = [0u8; 4];
            pixel[..s].copy_from_slice(&buffer[source+i*s..source+(i+1)*s]);
            if !self.blit_pixel(&mut buffer[dest+i*d..dest+(i+1)*d], &pixel[..s]) {
                return false;
            }
        }
        true
    }
}

/// Decodes one bounded RLE row into native-endian destination pixels.
pub trait RLEBlitter {
    /// Destination pixel width in bytes.
    fn pixel_size(&self) -> usize;
    /// Returns false on truncated input, insufficient output, or a short table.
    /// Pixels decoded before an error remain written.
    fn blit(&self, dest: &mut [u8], source: &[u8], length: usize, leadskip: usize) -> bool;
}

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
            fn read(bytes: &[u8]) -> Self { <$t>::from_ne_bytes(bytes.try_into().unwrap()) }
            fn write(self, bytes: &mut [u8]) { bytes.copy_from_slice(&self.to_ne_bytes()); }
            fn index(self) -> usize { self as usize }
        }
    };
}
pixel!(u8, 1);
pixel!(u16, 2);
pixel!(u32, 4);

macro_rules! plain {
    ($name:ident, $size:expr) => {
        pub struct $name;
        impl Blitter for $name {
            fn source_pixel_size(&self) -> usize { $size }
            fn dest_pixel_size(&self) -> usize { $size }
            fn blit_pixel(&self, dest: &mut [u8], source: &[u8]) -> bool {
                match (dest.get_mut(..$size), source.get(..$size)) {
                    (Some(d), Some(s)) => { d.copy_from_slice(s); true },
                    _ => false,
                }
            }
            fn blit_row(&self, dest: &mut [u8], source: &[u8], length: usize, _: bool) -> bool {
                let Some(bytes) = length.checked_mul($size) else { return false };
                match (dest.get_mut(..bytes), source.get(..bytes)) {
                    (Some(d), Some(s)) => { d.copy_from_slice(s); true },
                    _ => false,
                }
            }
            fn blit_within(&self, buffer: &mut [u8], dest: usize, source: usize, length: usize, _: bool) -> bool {
                let Some(bytes) = length.checked_mul($size) else { return false };
                let Some(src_end) = source.checked_add(bytes) else { return false };
                let Some(dst_end) = dest.checked_add(bytes) else { return false };
                if src_end > buffer.len() || dst_end > buffer.len() { return false; }
                buffer.copy_within(source..src_end, dest);
                true
            }
        }
    };
}
plain!(BlitPlainU8, 1);
plain!(BlitPlainU16, 2);
plain!(BlitPlainU32, 4);

macro_rules! transparent {
    ($name:ident, $size:expr) => {
        pub struct $name;
        impl Blitter for $name {
            fn source_pixel_size(&self) -> usize { $size }
            fn dest_pixel_size(&self) -> usize { $size }
            fn blit_pixel(&self, dest: &mut [u8], source: &[u8]) -> bool {
                match (dest.get_mut(..$size), source.get(..$size)) {
                    (Some(d), Some(s)) => { if s.iter().any(|&b| b != 0) { d.copy_from_slice(s); } true },
                    _ => false,
                }
            }
        }
    };
}
transparent!(BlitTransU8, 1);
transparent!(BlitTransU16, 2);
transparent!(BlitTransU32, 4);

pub struct BlitPlainXlat<'a, T> { translate: &'a [T] }
impl<'a, T> BlitPlainXlat<'a, T> { pub fn new(translate: &'a [T]) -> Self { Self { translate } } }
pub struct BlitTransXlat<'a, T> { translate: &'a [T] }
impl<'a, T> BlitTransXlat<'a, T> { pub fn new(translate: &'a [T]) -> Self { Self { translate } } }
pub struct BlitTransRemapXlat<'a, T> { remap: &'a [u8], translate: &'a [T] }
impl<'a, T> BlitTransRemapXlat<'a, T> { pub fn new(remap: &'a [u8], translate: &'a [T]) -> Self { Self { remap, translate } } }
pub struct BlitTransZRemapXlat<'a, T> { remap: &'a [u8], translate: &'a [T] }
impl<'a, T> BlitTransZRemapXlat<'a, T> { pub fn new(remap: &'a [u8], translate: &'a [T]) -> Self { Self { remap, translate } } }
pub struct BlitTransRemapDest<'a, T> { remap: &'a [T] }
impl<'a, T> BlitTransRemapDest<'a, T> { pub fn new(remap: &'a [T]) -> Self { Self { remap } } }
pub struct BlitTransDarken<T> { mask: T }
impl<T> BlitTransDarken<T> { pub fn new(mask: T) -> Self { Self { mask } } }
pub struct BlitDarken<T> { mask: T }
impl<T> BlitDarken<T> { pub fn new(mask: T) -> Self { Self { mask } } }
pub struct BlitTransLucent50<'a, T> { translate: &'a [T], mask: T }
impl<'a, T> BlitTransLucent50<'a, T> { pub fn new(translate: &'a [T], mask: T) -> Self { Self { translate, mask } } }
pub struct BlitTransLucent25<'a, T> { translate: &'a [T], mask: T }
impl<'a, T> BlitTransLucent25<'a, T> { pub fn new(translate: &'a [T], mask: T) -> Self { Self { translate, mask } } }
pub struct BlitTransLucent75<'a, T> { translate: &'a [T], mask: T }
impl<'a, T> BlitTransLucent75<'a, T> { pub fn new(translate: &'a [T], mask: T) -> Self { Self { translate, mask } } }

// Indexed-source effects share the same bounded pixel and table handling.
macro_rules! indexed_impl {
    ($name:ident, $t:ty, $this:ident, $value:ident, $dst:ident, $body:block) => {
        impl Blitter for $name<'_, $t> {
            fn source_pixel_size(&self) -> usize { 1 }
            fn dest_pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit_pixel(&self, dest: &mut [u8], source: &[u8]) -> bool {
                let Some(&$value) = source.first() else { return false };
                let Some(dest) = dest.get_mut(..<$t as Pixel>::SIZE) else { return false };
                let $dst = <$t as Pixel>::read(dest);
                let $this = self;
                let result: Option<$t> = $body;
                match result { Some(value) => { value.write(dest); true }, None => false }
            }
        }
    };
}
macro_rules! indexed_blitters {
    ($t:ty) => {
        indexed_impl!(BlitPlainXlat, $t, this, v, _dst, { this.translate.get(v as usize).copied() });
        indexed_impl!(BlitTransXlat, $t, this, v, dst, { if v == 0 { Some(dst) } else { this.translate.get(v as usize).copied() } });
        indexed_impl!(BlitTransRemapXlat, $t, this, v, dst, { if v == 0 { Some(dst) } else { this.remap.get(v as usize).and_then(|&i| this.translate.get(i as usize)).copied() } });
        indexed_impl!(BlitTransZRemapXlat, $t, this, v, dst, { if v == 0 { Some(dst) } else { this.remap.get(v as usize).and_then(|&i| this.translate.get(i as usize)).copied() } });
        indexed_impl!(BlitTransRemapDest, $t, this, v, dst, { if v == 0 { Some(dst) } else { this.remap.get(dst.index()).copied() } });
    };
}
indexed_blitters!(u8);
indexed_blitters!(u16);
indexed_blitters!(u32);

macro_rules! color_blitters {
    ($t:ty) => {
        impl Blitter for BlitTransDarken<$t> {
            fn source_pixel_size(&self) -> usize { 1 }
            fn dest_pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit_pixel(&self, dest: &mut [u8], source: &[u8]) -> bool {
                let Some(&v) = source.first() else { return false };
                let Some(dest) = dest.get_mut(..<$t as Pixel>::SIZE) else { return false };
                if v != 0 { ((<$t as Pixel>::read(dest) >> 1) & self.mask).write(dest); }
                true
            }
        }
        impl Blitter for BlitDarken<$t> {
            fn source_pixel_size(&self) -> usize { 0 }
            fn dest_pixel_size(&self) -> usize { <$t as Pixel>::SIZE }
            fn blit_pixel(&self, dest: &mut [u8], _: &[u8]) -> bool {
                let Some(dest) = dest.get_mut(..<$t as Pixel>::SIZE) else { return false };
                ((<$t as Pixel>::read(dest) >> 1) & self.mask).write(dest);
                true
            }
        }
        indexed_impl!(BlitTransLucent50, $t, this, v, dst, {
            if v == 0 { Some(dst) } else { this.translate.get(v as usize).map(|&s| ((dst >> 1) & this.mask).wrapping_add((s >> 1) & this.mask)) }
        });
        indexed_impl!(BlitTransLucent25, $t, this, v, dst, {
            if v == 0 { Some(dst) } else { this.translate.get(v as usize).map(|&s| { let s = (s >> 2) & this.mask; ((dst >> 2) & this.mask).wrapping_add(s).wrapping_add(s).wrapping_add(s) }) }
        });
        indexed_impl!(BlitTransLucent75, $t, this, v, dst, {
            if v == 0 { Some(dst) } else { this.translate.get(v as usize).map(|&s| { let d = (dst >> 2) & this.mask; d.wrapping_add(d).wrapping_add(d).wrapping_add((s >> 2) & this.mask) }) }
        });
    };
}
color_blitters!(u16);
color_blitters!(u32);

#[cfg(test)]
mod ordinary_tests {
    use super::*;

    #[test]
    fn ordinary_blitters_preserve_pixels_and_overlap() {
        let mut row = [1, 2, 0, 4, 5];
        assert!(BlitTransU8.blit_within(&mut row, 1, 0, 4, true));
        assert_eq!(row, [1, 1, 2, 4, 4]);
        assert!(BlitPlainU8.blit_within(&mut row, 0, 1, 4, false));
        assert_eq!(row, [1, 2, 4, 4, 4]);

        let table = [0u16, 0x1234, 0x5678];
        let mut out = [0xaa; 6];
        assert!(BlitTransXlat::new(&table).blit_forward(&mut out, &[1, 0, 2], 3));
        assert_eq!(&out[..2], &0x1234u16.to_ne_bytes());
        assert_eq!(&out[2..4], &[0xaa, 0xaa]);
        assert_eq!(&out[4..], &0x5678u16.to_ne_bytes());
        assert!(!BlitTransXlat::new(&table).blit_forward(&mut out, &[3], 1));
    }
}

#[cfg(test)]
mod effects_tests {
    use super::*;

    #[test]
    fn typed_transparency_is_native_endian_and_unaligned() {
        let mut out = [0xaau8; 9];
        let source = [0, 0, 0x34, 0x12];
        assert!(BlitTransU16.blit_forward(&mut out[1..5], &source, 2));
        assert_eq!(&out[1..5], &[0xaa, 0xaa, 0x34, 0x12]);
        let source32 = [0, 0, 0, 0, 1, 2, 3, 4];
        assert!(BlitTransU32.blit_backward(&mut out[1..9], &source32, 2));
        assert_eq!(&out[1..9], &[0xaa, 0xaa, 0x34, 0x12, 1, 2, 3, 4]);
    }

    #[test]
    fn indexed_remapping_darken_and_lucency_preserve_transparency() {
        let translate = [0u16, 0x4000, 0x1234];
        let remap = [0u8, 2, 1];
        let mut out = [0u8; 4];
        out[..2].copy_from_slice(&0x6000u16.to_ne_bytes());
        out[2..].copy_from_slice(&0x6000u16.to_ne_bytes());
        assert!(BlitTransRemapXlat::new(&remap, &translate).blit_forward(&mut out, &[1, 0], 2));
        assert_eq!(&out[..2], &0x1234u16.to_ne_bytes());
        assert_eq!(&out[2..], &0x6000u16.to_ne_bytes());
        assert!(BlitTransZRemapXlat::new(&remap, &translate).blit_forward(&mut out, &[2], 1));
        assert_eq!(&out[..2], &0x4000u16.to_ne_bytes());
        assert!(BlitTransDarken::new(0x7fffu16).blit_forward(&mut out, &[1, 0], 2));
        assert_eq!(&out[..2], &0x2000u16.to_ne_bytes());
        assert_eq!(&out[2..], &0x6000u16.to_ne_bytes());

        for (blitter, expected) in [
            (&BlitTransLucent25::new(&translate, 0xffffu16) as &dyn Blitter, 0x4800u16),
            (&BlitTransLucent50::new(&translate, 0xffffu16) as &dyn Blitter, 0x5000u16),
            (&BlitTransLucent75::new(&translate, 0xffffu16) as &dyn Blitter, 0x5800u16),
        ] {
            let mut pixel = 0x6000u16.to_ne_bytes();
            assert!(blitter.blit_forward(&mut pixel, &[1], 1));
            assert_eq!(u16::from_ne_bytes(pixel), expected);
        }
        let table32 = [0u32, 0x40000000];
        let mut pixel32 = 0x60000000u32.to_ne_bytes();
        assert!(BlitTransLucent50::new(&table32, 0xffffffff).blit_forward(&mut pixel32, &[1], 1));
        assert_eq!(u32::from_ne_bytes(pixel32), 0x50000000);
    }

    #[test]
    fn short_rows_tables_and_overflow_are_rejected() {
        let mut out = [9u8; 4];
        assert!(!BlitPlainU16.blit_forward(&mut out, &[1, 2], 2));
        assert_eq!(out, [9; 4]);
        assert!(!BlitPlainU32.blit_within(&mut out, usize::MAX, 0, 1, true));
        assert!(!BlitPlainU32.blit_forward(&mut out, &[0; 4], usize::MAX));
        assert!(!BlitTransRemapXlat::new(&[0u8], &[0u8]).blit_forward(&mut out, &[1], 1));
        assert!(!BlitTransRemapDest::new(&[0u8]).blit_forward(&mut out, &[1], 1));
        assert_eq!(out, [9; 4]);
    }

    #[test]
    fn multibyte_plain_copy_handles_both_overlap_directions() {
        let mut pixels = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        assert!(BlitPlainU32.blit_within(&mut pixels, 4, 0, 2, true));
        assert_eq!(pixels, [1, 2, 3, 4, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(BlitPlainU32.blit_within(&mut pixels, 0, 4, 2, false));
        assert_eq!(pixels, [1, 2, 3, 4, 5, 6, 7, 8, 5, 6, 7, 8]);
    }
}
