//! HSV color representation and utilities (ported from WWLib hsv.cpp/h).

use crate::rgb::RGBClass;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HSVClass {
    hue: u8,
    saturation: u8,
    value: u8,
}

impl HSVClass {
    pub const MAX_VALUE: u8 = 255;

    pub const fn new(hue: u8, saturation: u8, value: u8) -> Self {
        Self {
            hue,
            saturation,
            value,
        }
    }

    pub fn adjust(&mut self, ratio: i32, hsv: &HSVClass) {
        let ratio = (ratio & 0x00FF) as i32;

        let value = hsv.value as i32 - self.value as i32;
        self.value = (self.value as i32 + (value * ratio) / 256) as u8;

        let saturation = hsv.saturation as i32 - self.saturation as i32;
        self.saturation = (self.saturation as i32 + (saturation * ratio) / 256) as u8;

        let hue = hsv.hue as i32 - self.hue as i32;
        self.hue = (self.hue as i32 + (hue * ratio) / 256) as u8;
    }

    pub fn difference(&self, hsv: &HSVClass) -> i32 {
        let mut hue = self.hue as i32 - hsv.hue as i32;
        if hue < 0 {
            hue = -hue;
        }

        let mut saturation = self.saturation as i32 - hsv.saturation as i32;
        if saturation < 0 {
            saturation = -saturation;
        }

        let mut value = self.value as i32 - hsv.value as i32;
        if value < 0 {
            value = -value;
        }

        hue * hue + saturation * saturation + value * value
    }

    pub fn hue(&self) -> u8 {
        self.hue
    }

    pub fn saturation(&self) -> u8 {
        self.saturation
    }

    pub fn value(&self) -> u8 {
        self.value
    }

    pub fn set_hue(&mut self, value: u8) {
        self.hue = value;
    }

    pub fn set_saturation(&mut self, value: u8) {
        self.saturation = value;
    }

    pub fn set_value(&mut self, value: u8) {
        self.value = value;
    }

    pub fn to_rgb(self) -> RGBClass {
        self.into()
    }
}

impl From<HSVClass> for RGBClass {
    fn from(hsv: HSVClass) -> RGBClass {
        let hue = hsv.hue as i32;
        let saturation = hsv.saturation as i32;
        let value = hsv.value as i32;

        let hue = hue * 6;
        let f = hue % 255;

        let mut values = [0i32; 7];
        values[1] = value;
        values[2] = value;

        let tmp = (saturation * f) / 255;
        values[3] = (value * (255 - tmp)) / 255;

        values[4] = (value * (255 - saturation)) / 255;
        values[5] = values[4];

        let tmp = 255 - (saturation * (255 - f)) / 255;
        values[6] = (value * tmp) / 255;

        let mut i = hue / 255;
        i += if i > 4 { -4 } else { 2 };
        let red = values[i as usize];

        i += if i > 4 { -4 } else { 2 };
        let blue = values[i as usize];

        i += if i > 4 { -4 } else { 2 };
        let green = values[i as usize];

        RGBClass::new(red as u8, green as u8, blue as u8)
    }
}

const BLACK_COLOR: HSVClass = HSVClass::new(0, 0, 0);
