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

#[inline]
pub fn ggetm(src: *const u8, bytes: usize) -> u32 {
    unsafe {
        match bytes {
            1 => *src as u32,
            2 => ((*src as u32) << 8) | (*src.add(1) as u32),
            3 => ((*src as u32) << 16) | ((*src.add(1) as u32) << 8) | (*src.add(2) as u32),
            4 => {
                ((*src as u32) << 24)
                    | ((*src.add(1) as u32) << 16)
                    | ((*src.add(2) as u32) << 8)
                    | (*src.add(3) as u32)
            }
            _ => 0,
        }
    }
}

#[inline]
pub fn ggeti(src: *const u8, bytes: usize) -> u32 {
    unsafe {
        match bytes {
            1 => *src as u32,
            2 => (*src as u32) | ((*src.add(1) as u32) << 8),
            3 => (*src as u32) | ((*src.add(1) as u32) << 8) | ((*src.add(2) as u32) << 16),
            4 => {
                (*src as u32)
                    | ((*src.add(1) as u32) << 8)
                    | ((*src.add(2) as u32) << 16)
                    | ((*src.add(3) as u32) << 24)
            }
            _ => 0,
        }
    }
}

#[inline]
pub fn gputm(dst: *mut u8, data: u32, bytes: usize) {
    unsafe {
        match bytes {
            1 => *dst = data as u8,
            2 => {
                *dst = (data >> 8) as u8;
                *dst.add(1) = data as u8;
            }
            3 => {
                *dst = (data >> 16) as u8;
                *dst.add(1) = (data >> 8) as u8;
                *dst.add(2) = data as u8;
            }
            4 => {
                *dst = (data >> 24) as u8;
                *dst.add(1) = (data >> 16) as u8;
                *dst.add(2) = (data >> 8) as u8;
                *dst.add(3) = data as u8;
            }
            _ => {}
        }
    }
}

#[inline]
pub fn gputi(dst: *mut u8, data: u32, bytes: usize) {
    unsafe {
        match bytes {
            1 => *dst = data as u8,
            2 => {
                *dst = data as u8;
                *dst.add(1) = (data >> 8) as u8;
            }
            3 => {
                *dst = data as u8;
                *dst.add(1) = (data >> 8) as u8;
                *dst.add(2) = (data >> 16) as u8;
            }
            4 => {
                *dst = data as u8;
                *dst.add(1) = (data >> 8) as u8;
                *dst.add(2) = (data >> 16) as u8;
                *dst.add(3) = (data >> 24) as u8;
            }
            _ => {}
        }
    }
}
