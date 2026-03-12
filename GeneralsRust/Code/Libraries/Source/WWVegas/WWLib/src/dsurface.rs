//! DSurface helpers (partial port of WWLib dsurface.h/cpp).

use crate::palette::PaletteClass;
use crate::rgb::RGBClass;

/// Build a 16-bit hicolor pixel from 8-bit RGB.
pub fn build_hicolor_pixel(red: i32, green: i32, blue: i32) -> u16 {
    let r = ((red.clamp(0, 255) as u16) >> 3) & 0x1F;
    let g = ((green.clamp(0, 255) as u16) >> 2) & 0x3F;
    let b = ((blue.clamp(0, 255) as u16) >> 3) & 0x1F;
    (r << 11) | (g << 5) | b
}

/// Build a 256-entry remap table from palette to 16-bit pixels.
pub fn build_remap_table(table: &mut [u16], palette: &PaletteClass) {
    let len = table.len().min(256);
    for idx in 0..len {
        let color = palette.get_color(idx);
        table[idx] = build_hicolor_pixel(
            color.red() as i32,
            color.green() as i32,
            color.blue() as i32,
        );
    }
}

pub fn get_halfbright_mask() -> u16 {
    build_hicolor_pixel(127, 127, 127)
}

pub fn get_quarterbright_mask() -> u16 {
    build_hicolor_pixel(63, 63, 63)
}

pub fn get_eighthbright_mask() -> u16 {
    build_hicolor_pixel(31, 31, 31)
}

pub fn remap_rgb_to_hicolor(color: &RGBClass) -> u16 {
    build_hicolor_pixel(
        color.red() as i32,
        color.green() as i32,
        color.blue() as i32,
    )
}
