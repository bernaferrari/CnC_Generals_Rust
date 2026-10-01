//! Palette utilities (ported from WWLib palette.cpp/h).

use crate::rgb::{BLACK_COLOR, RGBClass};

pub const COLOR_COUNT: usize = 256;

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteClass {
    /// 256 RGB triplets, identical layout to `[RGBClass; 256]` (`repr(C)`, 3 bytes).
    palette: [[u8; 3]; COLOR_COUNT],
}

impl PaletteClass {
    pub fn new() -> Self {
        Self {
            palette: [[0; 3]; COLOR_COUNT],
        }
    }

    pub fn from_rgb(rgb: RGBClass) -> Self {
        let px = [rgb.red(), rgb.green(), rgb.blue()];
        Self {
            palette: [px; COLOR_COUNT],
        }
    }

    pub fn from_binary(binary_palette: &[u8]) -> Self {
        let mut palette = [[0u8; 3]; COLOR_COUNT];
        let needed = COLOR_COUNT * 3;
        let copy_len = binary_palette.len().min(needed);
        let mut index = 0;
        while index + 2 < copy_len {
            let entry = index / 3;
            palette[entry] = [
                binary_palette[index],
                binary_palette[index + 1],
                binary_palette[index + 2],
            ];
            index += 3;
        }
        Self { palette }
    }

    pub fn get_color(&self, index: usize) -> RGBClass {
        let px = self.palette[index % COLOR_COUNT];
        RGBClass::new(px[0], px[1], px[2])
    }

    pub fn get_color_mut(&mut self, index: usize) -> &mut [u8; 3] {
        &mut self.palette[index % COLOR_COUNT]
    }

    pub fn adjust_to_black(&mut self, ratio: i32) {
        for slot in &mut self.palette {
            let mut color = RGBClass::new(slot[0], slot[1], slot[2]);
            color.adjust(ratio, &BLACK_COLOR);
            *slot = [color.red(), color.green(), color.blue()];
        }
    }

    pub fn adjust_to_palette(&mut self, ratio: i32, palette: &PaletteClass) {
        for (index, slot) in self.palette.iter_mut().enumerate() {
            let mut color = RGBClass::new(slot[0], slot[1], slot[2]);
            let other = RGBClass::new(
                palette.palette[index][0],
                palette.palette[index][1],
                palette.palette[index][2],
            );
            color.adjust(ratio, &other);
            *slot = [color.red(), color.green(), color.blue()];
        }
    }

    pub fn partial_adjust_to_black(&mut self, ratio: i32, lut: &[u8]) {
        for (index, slot) in self.palette.iter_mut().enumerate() {
            if lut.get(index).copied().unwrap_or(0) != 0 {
                let mut color = RGBClass::new(slot[0], slot[1], slot[2]);
                color.adjust(ratio, &BLACK_COLOR);
                *slot = [color.red(), color.green(), color.blue()];
            }
        }
    }

    pub fn partial_adjust_to_palette(&mut self, ratio: i32, palette: &PaletteClass, lut: &[u8]) {
        for (index, slot) in self.palette.iter_mut().enumerate() {
            if lut.get(index).copied().unwrap_or(0) != 0 {
                let mut color = RGBClass::new(slot[0], slot[1], slot[2]);
                let other = RGBClass::new(
                    palette.palette[index][0],
                    palette.palette[index][1],
                    palette.palette[index][2],
                );
                color.adjust(ratio, &other);
                *slot = [color.red(), color.green(), color.blue()];
            }
        }
    }

    pub fn closest_color(&self, rgb: &RGBClass) -> usize {
        let mut closest = 0usize;
        let mut value: Option<i32> = None;

        for (index, slot) in self.palette.iter().enumerate() {
            let color = RGBClass::new(slot[0], slot[1], slot[2]);
            let difference = rgb.difference(&color);
            if value.map_or(true, |current| difference < current) {
                value = Some(difference);
                closest = index;
            }
        }

        closest
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.palette.as_flattened()
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.palette.as_flattened_mut()
    }
}

impl Default for PaletteClass {
    fn default() -> Self {
        Self::new()
    }
}
