//! Palette utilities (ported from WWLib palette.cpp/h).

use crate::rgb::{RGBClass, BLACK_COLOR};

pub const COLOR_COUNT: usize = 256;

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteClass {
    palette: [RGBClass; COLOR_COUNT],
}

impl PaletteClass {
    pub fn new() -> Self {
        Self {
            palette: [RGBClass::default(); COLOR_COUNT],
        }
    }

    pub fn from_rgb(rgb: RGBClass) -> Self {
        Self {
            palette: [rgb; COLOR_COUNT],
        }
    }

    pub fn from_binary(binary_palette: &[u8]) -> Self {
        let mut palette = [RGBClass::default(); COLOR_COUNT];
        let needed = COLOR_COUNT * 3;
        let copy_len = binary_palette.len().min(needed);
        let mut index = 0;
        while index + 2 < copy_len {
            let entry = index / 3;
            palette[entry] = RGBClass::new(
                binary_palette[index],
                binary_palette[index + 1],
                binary_palette[index + 2],
            );
            index += 3;
        }
        Self { palette }
    }

    pub fn get_color(&self, index: usize) -> &RGBClass {
        &self.palette[index % COLOR_COUNT]
    }

    pub fn get_color_mut(&mut self, index: usize) -> &mut RGBClass {
        &mut self.palette[index % COLOR_COUNT]
    }

    pub fn adjust_to_black(&mut self, ratio: i32) {
        for color in &mut self.palette {
            color.adjust(ratio, &BLACK_COLOR);
        }
    }

    pub fn adjust_to_palette(&mut self, ratio: i32, palette: &PaletteClass) {
        for (index, color) in self.palette.iter_mut().enumerate() {
            color.adjust(ratio, &palette.palette[index]);
        }
    }

    pub fn partial_adjust_to_black(&mut self, ratio: i32, lut: &[u8]) {
        for (index, color) in self.palette.iter_mut().enumerate() {
            if lut.get(index).copied().unwrap_or(0) != 0 {
                color.adjust(ratio, &BLACK_COLOR);
            }
        }
    }

    pub fn partial_adjust_to_palette(&mut self, ratio: i32, palette: &PaletteClass, lut: &[u8]) {
        for (index, color) in self.palette.iter_mut().enumerate() {
            if lut.get(index).copied().unwrap_or(0) != 0 {
                color.adjust(ratio, &palette.palette[index]);
            }
        }
    }

    pub fn closest_color(&self, rgb: &RGBClass) -> usize {
        let mut closest = 0usize;
        let mut value: Option<i32> = None;

        for (index, color) in self.palette.iter().enumerate() {
            let difference = rgb.difference(color);
            if value.map_or(true, |current| difference < current) {
                value = Some(difference);
                closest = index;
            }
        }

        closest
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.palette.as_ptr() as *const u8, COLOR_COUNT * 3) }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(self.palette.as_mut_ptr() as *mut u8, COLOR_COUNT * 3)
        }
    }
}

impl Default for PaletteClass {
    fn default() -> Self {
        Self::new()
    }
}
