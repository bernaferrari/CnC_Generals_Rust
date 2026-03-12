//! RGB color representation and utilities (ported from WWLib rgb.cpp/h).

use crate::hsv::HSVClass;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RGBClass {
    red: u8,
    green: u8,
    blue: u8,
}

impl RGBClass {
    pub const MAX_VALUE: u8 = 255;

    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn adjust(&mut self, ratio: i32, rgb: &RGBClass) {
        // Ratio is clamped to 0..=255 via masking in the original.
        let ratio = (ratio & 0x00FF) as i32;

        let value = rgb.red as i32 - self.red as i32;
        self.red = (self.red as i32 + (value * ratio) / 256) as u8;

        let value = rgb.green as i32 - self.green as i32;
        self.green = (self.green as i32 + (value * ratio) / 256) as u8;

        let value = rgb.blue as i32 - self.blue as i32;
        self.blue = (self.blue as i32 + (value * ratio) / 256) as u8;
    }

    pub fn difference(&self, rgb: &RGBClass) -> i32 {
        let mut r = self.red as i32 - rgb.red as i32;
        if r < 0 {
            r = -r;
        }

        let mut g = self.green as i32 - rgb.green as i32;
        if g < 0 {
            g = -g;
        }

        let mut b = self.blue as i32 - rgb.blue as i32;
        if b < 0 {
            b = -b;
        }

        4 * g + 3 * b + 2 * r
    }

    pub fn red(&self) -> u8 {
        self.red
    }

    pub fn green(&self) -> u8 {
        self.green
    }

    pub fn blue(&self) -> u8 {
        self.blue
    }

    pub fn set_red(&mut self, value: u8) {
        self.red = value;
    }

    pub fn set_green(&mut self, value: u8) {
        self.green = value;
    }

    pub fn set_blue(&mut self, value: u8) {
        self.blue = value;
    }

    pub fn to_hsv(self) -> HSVClass {
        self.into()
    }
}

impl From<RGBClass> for HSVClass {
    fn from(rgb: RGBClass) -> HSVClass {
        let red = rgb.red as i32;
        let green = rgb.green as i32;
        let blue = rgb.blue as i32;

        let mut hue = 0;

        let mut value = if red > green { red } else { green };
        if blue > value {
            value = blue;
        }

        let mut white = if red < green { red } else { green };
        if blue < white {
            white = blue;
        }

        let mut saturation = 0;
        if value != 0 {
            saturation = ((value - white) * 255) / value;
        }

        if saturation != 0 {
            let tmp = (value - white) as i32;
            let r1 = ((value - red) * 255) / tmp;
            let g1 = ((value - green) * 255) / tmp;
            let b1 = ((value - blue) * 255) / tmp;

            let mut t = if value == red {
                if white == green {
                    5 * 256 + b1
                } else {
                    1 * 256 - g1
                }
            } else if value == green {
                if white == blue {
                    1 * 256 + r1
                } else {
                    3 * 256 - b1
                }
            } else if white == red {
                3 * 256 + g1
            } else {
                5 * 256 - r1
            };

            hue = t / 6;
        }

        HSVClass::new(hue as u8, saturation as u8, value as u8)
    }
}

pub const BLACK_COLOR: RGBClass = RGBClass::new(0, 0, 0);
