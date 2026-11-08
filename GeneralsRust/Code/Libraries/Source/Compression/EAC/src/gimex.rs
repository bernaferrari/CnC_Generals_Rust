//! GIMEX API definitions (ported from gimex.h).

use core::ffi::c_void;

pub const GIMEX_VERSION: i32 = 346;
pub const GIMEX_PATCH: i32 = 0;

pub type GCHANNEL = u8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ARGB {
    pub b: GCHANNEL,
    pub g: GCHANNEL,
    pub r: GCHANNEL,
    pub a: GCHANNEL,
}

pub type GPOS = i64;

pub const GIMEX_FRAMENAME_SIZE: usize = 512;
pub const GIMEX_COMMENT_SIZE: usize = 1024;
pub const GIMEX_COLOURTBL_SIZE: usize = 256;
pub const GIMEX_HOTSPOTTBL_SIZE: usize = 1024;
pub const GIMEX_HOTSPOTTBL_VALUES: usize = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GINFO {
    pub signature: i32,
    pub size: i32,
    pub version: i32,
    pub framenum: i32,
    pub width: i32,
    pub height: i32,
    pub bpp: i32,
    pub originalbpp: i32,
    pub startcolour: i32,
    pub numcolours: i32,
    pub colourtbl: [ARGB; GIMEX_COLOURTBL_SIZE],
    pub subtype: i32,
    pub packed: i32,
    pub quality: i32,
    pub framesize: i32,
    pub alphabits: i32,
    pub redbits: i32,
    pub greenbits: i32,
    pub bluebits: i32,
    pub centerx: i32,
    pub centery: i32,
    pub defaultx: i32,
    pub defaulty: i32,
    pub numhotspots: i32,
    pub framename: [u8; GIMEX_FRAMENAME_SIZE],
    pub comment: [u8; GIMEX_COMMENT_SIZE],
    pub hotspottbl: [[i32; GIMEX_HOTSPOTTBL_VALUES]; GIMEX_HOTSPOTTBL_SIZE],
    pub dpi: f32,
    pub fps: f32,
    pub reserved: [i32; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GINSTANCE {
    pub signature: i32,
    pub size: i32,
    pub frames: i32,
    pub framenum: i32,
    pub gstream: *mut GSTREAM,
    pub gref: *mut c_void,
}

pub const MAXMACTYPES: usize = 8;
pub const MAXEXTENSIONS: usize = 8;

pub const GIMEX_EXTENSION_SIZE: usize = 8;
pub const GIMEX_AUTHORSTR_SIZE: usize = 32;
pub const GIMEX_VERSIONSTR_SIZE: usize = 8;
pub const GIMEX_SHORTTYPESTR_SIZE: usize = 8;
pub const GIMEX_WORDTYPESTR_SIZE: usize = 16;
pub const GIMEX_LONGTYPESTR_SIZE: usize = 32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GABOUT {
    pub signature: i32,
    pub size: i32,
    pub version: i32,
    pub canimport: u32,
    pub canexport: u32,
    pub importpacked: u32,
    pub exportpacked: u32,
    pub import8: u32,
    pub export8: u32,
    pub import32: u32,
    pub export32: u32,
    pub multiframe: u32,
    pub multifile: u32,
    pub multisize: u32,
    pub framebuffer: u32,
    pub external: u32,
    pub usesfile: u32,
    pub globalpalette: u32,
    pub greyscale: u32,
    pub startcolour: u32,
    pub dotsubtype: u32,
    pub resizable: u32,
    pub reserved2: u32,
    pub reserved3: u32,
    pub importstream: u32,
    pub exportstream: u32,
    pub movie: u32,
    pub mipmap: u32,
    pub font: u32,
    pub obsolete: u32,
    pub file64: u32,
    pub firstextension: u32,
    pub pad: u32,
    pub maxcolours: i32,
    pub maxframename: i32,
    pub defaultquality: i32,
    pub mactype: [i32; MAXMACTYPES],
    pub extensions: [[u8; GIMEX_EXTENSION_SIZE]; MAXEXTENSIONS],
    pub authorstr: [u8; GIMEX_AUTHORSTR_SIZE],
    pub versionstr: [u8; GIMEX_VERSIONSTR_SIZE],
    pub shorttypestr: [u8; GIMEX_SHORTTYPESTR_SIZE],
    pub wordtypestr: [u8; GIMEX_WORDTYPESTR_SIZE],
    pub longtypestr: [u8; GIMEX_LONGTYPESTR_SIZE],
    pub maxalphabits: u32,
    pub maxredbits: u32,
    pub maxgreenbits: u32,
    pub maxbluebits: u32,
    pub maxwidth: u32,
    pub maxheight: u32,
    pub alignwidth: u32,
    pub alignheight: u32,
    pub pad2: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GBITMAP {
    pub ginfo: *mut GINFO,
    pub image: *mut c_void,
    pub rowbytes: i32,
}

#[repr(C)]
pub struct GSTREAM {
    _private: [u8; 0],
}

#[repr(C)]
pub struct GFUNCTIONS {
    pub gimex_about: Option<extern "C" fn() -> *mut GABOUT>,
    pub gimex_is: Option<extern "C" fn(g: *mut GSTREAM) -> i32>,
    pub gimex_open: Option<
        extern "C" fn(
            gx: *mut *mut GINSTANCE,
            g: *mut GSTREAM,
            pathname: *const i8,
            framecountflag: bool,
        ) -> i32,
    >,
    pub gimex_info: Option<extern "C" fn(gx: *mut GINSTANCE, framenum: i32) -> *mut GINFO>,
    pub gimex_read: Option<
        extern "C" fn(gx: *mut GINSTANCE, ginfo: *const GINFO, dest: *mut i8, rowbytes: i32) -> i32,
    >,
    pub gimex_close: Option<extern "C" fn(gx: *mut GINSTANCE) -> i32>,
    pub gimex_wopen: Option<
        extern "C" fn(
            gx: *mut *mut GINSTANCE,
            g: *mut GSTREAM,
            pathname: *const i8,
            numframes: i32,
        ) -> i32,
    >,
    pub gimex_write: Option<
        extern "C" fn(
            gx: *mut GINSTANCE,
            ginfo: *const GINFO,
            source: *const i8,
            rowbytes: i32,
        ) -> i32,
    >,
    pub gimex_wclose: Option<extern "C" fn(gx: *mut GINSTANCE) -> i32>,
}

pub const GIMEX_FRAMECOUNT: bool = true;
pub const GIMEX_NOFRAMECOUNT: bool = false;

/// Reads 1–4 big-endian bytes. Returns `None` for an invalid width or a short slice.
#[inline]
pub fn ggetm(src: &[u8], bytes: usize) -> Option<u32> {
    if !(1..=4).contains(&bytes) {
        return None;
    }
    let src = src.get(..bytes)?;
    let mut value = [0; 4];
    value[4 - bytes..].copy_from_slice(src);
    Some(u32::from_be_bytes(value))
}

/// Reads 1–4 little-endian bytes. Returns `None` for an invalid width or a short slice.
#[inline]
pub fn ggeti(src: &[u8], bytes: usize) -> Option<u32> {
    if !(1..=4).contains(&bytes) {
        return None;
    }
    let src = src.get(..bytes)?;
    let mut value = [0; 4];
    value[..bytes].copy_from_slice(src);
    Some(u32::from_le_bytes(value))
}

/// Writes the low 1–4 bytes in big-endian order.
/// Returns `false` without modifying the slice for an invalid width or a short slice.
#[inline]
pub fn gputm(dst: &mut [u8], data: u32, bytes: usize) -> bool {
    if !(1..=4).contains(&bytes) {
        return false;
    }
    let Some(dst) = dst.get_mut(..bytes) else {
        return false;
    };
    dst.copy_from_slice(&data.to_be_bytes()[4 - bytes..]);
    true
}

/// Writes the low 1–4 bytes in little-endian order.
/// Returns `false` without modifying the slice for an invalid width or a short slice.
#[inline]
pub fn gputi(dst: &mut [u8], data: u32, bytes: usize) -> bool {
    if !(1..=4).contains(&bytes) {
        return false;
    }
    let Some(dst) = dst.get_mut(..bytes) else {
        return false;
    };
    dst.copy_from_slice(&data.to_le_bytes()[..bytes]);
    true
}

#[cfg(test)]
mod tests {
    use super::{ggeti, ggetm, gputi, gputm};

    #[test]
    fn endian_operations_cover_all_widths() {
        let big = [0x12, 0x34, 0x56, 0x78, 0xaa];
        let little = [0x78, 0x56, 0x34, 0x12, 0xaa];
        let big_values = [0x12, 0x1234, 0x123456, 0x12345678];
        let little_values = [0x78, 0x5678, 0x345678, 0x12345678];
        for bytes in 1..=4 {
            assert_eq!(ggetm(&big, bytes), Some(big_values[bytes - 1]));
            assert_eq!(ggeti(&little, bytes), Some(little_values[bytes - 1]));
            let mut out = [0xaa; 5];
            assert!(gputm(&mut out, big_values[bytes - 1], bytes));
            assert_eq!(&out[..bytes], &big[..bytes]);
            assert!(out[bytes..].iter().all(|&b| b == 0xaa));
            let mut out = [0xaa; 5];
            assert!(gputi(&mut out, little_values[bytes - 1], bytes));
            assert_eq!(&out[..bytes], &little[..bytes]);
            assert!(out[bytes..].iter().all(|&b| b == 0xaa));
        }
    }

    #[test]
    fn writes_truncate_high_bytes() {
        let mut out = [0; 3];
        assert!(gputm(&mut out, 0x12345678, 3));
        assert_eq!(out, [0x34, 0x56, 0x78]);
        assert!(gputi(&mut out, 0x12345678, 3));
        assert_eq!(out, [0x78, 0x56, 0x34]);
    }

    #[test]
    fn invalid_widths_and_short_buffers_are_rejected_without_writes() {
        for bytes in [0, 1, 2, 3, 4, 5, usize::MAX] {
            for len in 0..=4 {
                if (1..=4).contains(&bytes) && len >= bytes {
                    continue;
                }
                let src = [0x12; 4];
                assert_eq!(ggetm(&src[..len], bytes), None);
                assert_eq!(ggeti(&src[..len], bytes), None);
                let mut dst = [0xaa; 4];
                assert!(!gputm(&mut dst[..len], u32::MAX, bytes));
                assert_eq!(dst, [0xaa; 4]);
                assert!(!gputi(&mut dst[..len], u32::MAX, bytes));
                assert_eq!(dst, [0xaa; 4]);
            }
        }
    }
}
