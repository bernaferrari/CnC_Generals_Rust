//! ConvertClass implementation (ported from WWLib convert.h).

use crate::blitter::{
    BlitPlainXlat, BlitTransDarken, BlitTransLucent25, BlitTransLucent50, BlitTransLucent75,
    BlitTransRemapDest, BlitTransRemapXlat, BlitTransXlat, BlitTransZRemapXlat, Blitter,
    RLEBlitter,
};
use crate::dsurface;
use crate::hsv::HSVClass;
use crate::palette::PaletteClass;
use crate::rgb::RGBClass;
use crate::rlerle::{
    RLEBlitTransDarken, RLEBlitTransLucent25, RLEBlitTransLucent50, RLEBlitTransLucent75,
    RLEBlitTransRemapDest, RLEBlitTransRemapXlat, RLEBlitTransXlat, RLEBlitTransZRemapXlat,
};
use crate::surface::{PixelFormat, Surface};

/// Shape flags used to select blitters (subset).
pub type ShapeFlagsType = u32;

pub const SHAPE_NORMAL: ShapeFlagsType = 0x0000;
pub const SHAPE_WIN_REL: ShapeFlagsType = 0x0400;
pub const SHAPE_CENTER: ShapeFlagsType = 0x0200;
pub const SHAPE_DARKEN: ShapeFlagsType = 0x0001;
pub const SHAPE_TRANSLUCENT25: ShapeFlagsType = 0x0002;
pub const SHAPE_TRANSLUCENT50: ShapeFlagsType = 0x0004;
pub const SHAPE_TRANSLUCENT75: ShapeFlagsType = 0x0006;
pub const SHAPE_PREDATOR: ShapeFlagsType = 0x0008;
pub const SHAPE_REMAP: ShapeFlagsType = 0x0010;
pub const SHAPE_NOTRANS: ShapeFlagsType = 0x0020;

/// Palette/pixel format conversion helper.
pub struct ConvertClass {
    bbp: usize,
    translator_u8: Vec<u8>,
    translator_u16: Vec<u16>,
    translator_u32: Vec<u32>,
    remap_table: Option<Vec<u8>>,
    shadow_table: Option<Vec<u8>>,
    identity_table: Vec<u8>,
}

impl ConvertClass {
    pub fn new(artpalette: &PaletteClass, screenpalette: &PaletteClass, surface: &Surface) -> Self {
        let bbp = surface.bytes_per_pixel();
        let mut translator_u8 = vec![0u8; 256];
        let mut translator_u16 = vec![0u16; 256];
        let mut translator_u32 = vec![0u32; 256];
        let mut shadow_table = None;
        let identity_table: Vec<u8> = (0u8..=255u8).collect();

        for i in 0..256 {
            let color = artpalette.get_color(i);
            match surface.get_pixel_format() {
                PixelFormat::Palette8 => {
                    let idx = screenpalette.closest_color(color) as u8;
                    translator_u8[i] = idx;
                }
                PixelFormat::RGB16 => {
                    translator_u16[i] = dsurface::remap_rgb_to_hicolor(color);
                }
                PixelFormat::RGB24 => {
                    translator_u32[i] = rgb_to_888(color);
                }
                PixelFormat::RGBA32 => {
                    translator_u32[i] = rgb_to_888a(color);
                }
            }
        }

        if surface.get_pixel_format() == PixelFormat::Palette8 {
            let mut shadow = vec![0u8; 256];
            shadow[0] = 0;
            for idx in 1..256 {
                let hsv: HSVClass = (*artpalette.get_color(idx)).into();
                let mut hsv = hsv;
                hsv.set_value(hsv.value() / 2);
                let rgb: RGBClass = hsv.into();
                shadow[idx] = artpalette.closest_color(&rgb) as u8;
            }
            shadow_table = Some(shadow);
        }

        ConvertClass {
            bbp,
            translator_u8,
            translator_u16,
            translator_u32,
            remap_table: None,
            shadow_table,
            identity_table,
        }
    }

    pub fn convert_pixel(&self, pixel: u8) -> u32 {
        let idx = pixel as usize;
        match self.bbp {
            1 => self.translator_u8[idx] as u32,
            2 => self.translator_u16[idx] as u32,
            _ => self.translator_u32[idx],
        }
    }

    pub fn bytes_per_pixel(&self) -> usize {
        self.bbp
    }

    pub fn get_translate_table(&self) -> &[u8] {
        &self.translator_u8
    }

    pub fn get_translate_table_16(&self) -> &[u16] {
        &self.translator_u16
    }

    pub fn get_translate_table_32(&self) -> &[u32] {
        &self.translator_u32
    }

    pub fn set_remap(&mut self, remap: Option<&[u8]>) {
        self.remap_table = remap.map(|data| data.to_vec());
    }

    pub fn remap_table(&self) -> Option<&[u8]> {
        self.remap_table.as_deref()
    }

    pub fn blitter_from_flags(&self, flags: ShapeFlagsType) -> Box<dyn Blitter + '_> {
        let remap = self.remap_table.as_deref().unwrap_or(&self.identity_table);
        let shadow = self.shadow_table.as_deref().unwrap_or(&self.translator_u8);

        if (flags & SHAPE_REMAP) != 0 {
            return match self.bbp {
                1 => Box::new(BlitTransZRemapXlat::new(remap, &self.translator_u8)),
                2 => Box::new(BlitTransZRemapXlat::new(remap, &self.translator_u16)),
                _ => Box::new(BlitTransZRemapXlat::new(remap, &self.translator_u32)),
            };
        }

        match flags & (SHAPE_TRANSLUCENT25 | SHAPE_TRANSLUCENT50 | SHAPE_TRANSLUCENT75) {
            SHAPE_TRANSLUCENT25 => {
                return match self.bbp {
                    1 => Box::new(BlitTransRemapXlat::new(shadow, &self.translator_u8)),
                    2 => Box::new(BlitTransLucent25::new(
                        &self.translator_u16,
                        dsurface::get_quarterbright_mask(),
                    )),
                    _ => Box::new(BlitTransLucent25::new(
                        &self.translator_u32,
                        mask_quarter(self.bbp),
                    )),
                };
            }
            SHAPE_TRANSLUCENT50 => {
                return match self.bbp {
                    1 => Box::new(BlitTransRemapXlat::new(shadow, &self.translator_u8)),
                    2 => Box::new(BlitTransLucent50::new(
                        &self.translator_u16,
                        dsurface::get_halfbright_mask(),
                    )),
                    _ => Box::new(BlitTransLucent50::new(
                        &self.translator_u32,
                        mask_half(self.bbp),
                    )),
                };
            }
            SHAPE_TRANSLUCENT75 => {
                return match self.bbp {
                    1 => Box::new(BlitTransRemapXlat::new(shadow, &self.translator_u8)),
                    2 => Box::new(BlitTransLucent75::new(
                        &self.translator_u16,
                        dsurface::get_quarterbright_mask(),
                    )),
                    _ => Box::new(BlitTransLucent75::new(
                        &self.translator_u32,
                        mask_quarter(self.bbp),
                    )),
                };
            }
            _ => {}
        }

        if (flags & SHAPE_DARKEN) != 0 {
            return match self.bbp {
                1 => Box::new(BlitTransRemapDest::new(shadow)),
                2 => Box::new(BlitTransDarken::new(dsurface::get_halfbright_mask())),
                _ => Box::new(BlitTransDarken::new(mask_half(self.bbp))),
            };
        }

        if (flags & SHAPE_NOTRANS) != 0 {
            return match self.bbp {
                1 => Box::new(BlitPlainXlat::new(&self.translator_u8)),
                2 => Box::new(BlitPlainXlat::new(&self.translator_u16)),
                _ => Box::new(BlitPlainXlat::new(&self.translator_u32)),
            };
        }

        match self.bbp {
            1 => Box::new(BlitTransXlat::new(&self.translator_u8)),
            2 => Box::new(BlitTransXlat::new(&self.translator_u16)),
            _ => Box::new(BlitTransXlat::new(&self.translator_u32)),
        }
    }

    pub fn rle_blitter_from_flags(&self, flags: ShapeFlagsType) -> Box<dyn RLEBlitter + '_> {
        let remap = self.remap_table.as_deref().unwrap_or(&self.identity_table);
        let shadow = self.shadow_table.as_deref().unwrap_or(&self.translator_u8);

        if (flags & SHAPE_REMAP) != 0 {
            return match self.bbp {
                1 => Box::new(RLEBlitTransZRemapXlat::new(remap, &self.translator_u8)),
                2 => Box::new(RLEBlitTransZRemapXlat::new(remap, &self.translator_u16)),
                _ => Box::new(RLEBlitTransZRemapXlat::new(remap, &self.translator_u32)),
            };
        }

        match flags & (SHAPE_TRANSLUCENT25 | SHAPE_TRANSLUCENT50 | SHAPE_TRANSLUCENT75) {
            SHAPE_TRANSLUCENT25 => {
                return match self.bbp {
                    1 => Box::new(RLEBlitTransRemapXlat::new(shadow, &self.translator_u8)),
                    2 => Box::new(RLEBlitTransLucent25::new(
                        &self.translator_u16,
                        dsurface::get_quarterbright_mask(),
                    )),
                    _ => Box::new(RLEBlitTransLucent25::new(
                        &self.translator_u32,
                        mask_quarter(self.bbp),
                    )),
                };
            }
            SHAPE_TRANSLUCENT50 => {
                return match self.bbp {
                    1 => Box::new(RLEBlitTransRemapXlat::new(shadow, &self.translator_u8)),
                    2 => Box::new(RLEBlitTransLucent50::new(
                        &self.translator_u16,
                        dsurface::get_halfbright_mask(),
                    )),
                    _ => Box::new(RLEBlitTransLucent50::new(
                        &self.translator_u32,
                        mask_half(self.bbp),
                    )),
                };
            }
            SHAPE_TRANSLUCENT75 => {
                return match self.bbp {
                    1 => Box::new(RLEBlitTransRemapXlat::new(shadow, &self.translator_u8)),
                    2 => Box::new(RLEBlitTransLucent75::new(
                        &self.translator_u16,
                        dsurface::get_quarterbright_mask(),
                    )),
                    _ => Box::new(RLEBlitTransLucent75::new(
                        &self.translator_u32,
                        mask_quarter(self.bbp),
                    )),
                };
            }
            _ => {}
        }

        if (flags & SHAPE_DARKEN) != 0 {
            return match self.bbp {
                1 => Box::new(RLEBlitTransRemapDest::new(shadow)),
                2 => Box::new(RLEBlitTransDarken::new(dsurface::get_halfbright_mask())),
                _ => Box::new(RLEBlitTransDarken::new(mask_half(self.bbp))),
            };
        }

        match self.bbp {
            1 => Box::new(RLEBlitTransXlat::new(&self.translator_u8)),
            2 => Box::new(RLEBlitTransXlat::new(&self.translator_u16)),
            _ => Box::new(RLEBlitTransXlat::new(&self.translator_u32)),
        }
    }
}

fn rgb_to_565(color: &RGBClass) -> u16 {
    let r = (color.red() as u16 >> 3) & 0x1F;
    let g = (color.green() as u16 >> 2) & 0x3F;
    let b = (color.blue() as u16 >> 3) & 0x1F;
    (r << 11) | (g << 5) | b
}

fn rgb_to_888(color: &RGBClass) -> u32 {
    ((color.red() as u32) << 16) | ((color.green() as u32) << 8) | (color.blue() as u32)
}

fn rgb_to_888a(color: &RGBClass) -> u32 {
    (0xFFu32 << 24)
        | ((color.red() as u32) << 16)
        | ((color.green() as u32) << 8)
        | (color.blue() as u32)
}

fn mask_half(bbp: usize) -> u32 {
    match bbp {
        2 => 0x7BEF,
        _ => 0x7F7F7F,
    }
}

fn mask_quarter(bbp: usize) -> u32 {
    match bbp {
        2 => 0x39E7,
        _ => 0x3F3F3F,
    }
}
