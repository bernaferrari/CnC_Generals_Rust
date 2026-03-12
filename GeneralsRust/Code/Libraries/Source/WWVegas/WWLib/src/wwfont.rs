//! WWFontClass implementation (ported from WWLib wwfont.cpp/h).

use crate::convert::ConvertClass;
use crate::font::FontClass;
use crate::point::Point2D;
use crate::surface::{Rect, Surface};
use std::sync::Arc;

const FONTINFOMAXHEIGHT: usize = 4;
const FONTINFOMAXWIDTH: usize = 5;
const FUDGEDIV: i32 = 16;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct FontType {
    font_length: u16,
    font_compress: u8,
    font_data_blocks: u8,
    info_block_offset: u16,
    offset_block_offset: u16,
    width_block_offset: u16,
    data_block_offset: u16,
    height_offset: u16,
}

impl FontType {
    fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 14 {
            return None;
        }
        Some(FontType {
            font_length: u16::from_le_bytes([data[0], data[1]]),
            font_compress: data[2],
            font_data_blocks: data[3],
            info_block_offset: u16::from_le_bytes([data[4], data[5]]),
            offset_block_offset: u16::from_le_bytes([data[6], data[7]]),
            width_block_offset: u16::from_le_bytes([data[8], data[9]]),
            data_block_offset: u16::from_le_bytes([data[10], data[11]]),
            height_offset: u16::from_le_bytes([data[12], data[13]]),
        })
    }
}

pub struct WWFontClass {
    is_outlined_data: bool,
    shadow: i32,
    font_x_spacing: i32,
    font_y_spacing: i32,
    font_data: Vec<u8>,
    font_header: Option<FontType>,
    remap_palette: Option<Vec<u8>>,
    converter: Option<Arc<ConvertClass>>,
}

impl WWFontClass {
    pub fn new(
        fontdata: &[u8],
        isoutlined: bool,
        shadow: i32,
        converter: Option<Arc<ConvertClass>>,
        remap: Option<&[u8]>,
    ) -> Self {
        let mut font = WWFontClass {
            is_outlined_data: isoutlined,
            shadow,
            font_x_spacing: 0,
            font_y_spacing: 0,
            font_data: Vec::new(),
            font_header: None,
            remap_palette: remap.map(|data| data.to_vec()),
            converter,
        };
        font.set_font_data(fontdata);
        font
    }

    pub fn set_font_data(&mut self, fontdata: &[u8]) {
        self.font_data = fontdata.to_vec();
        self.font_header = FontType::from_bytes(&self.font_data);
        if self.font_header.is_some() {
            let current_x = self.font_x_spacing;
            let current_y = self.font_y_spacing;
            self.set_xspacing(current_x);
            self.set_yspacing(current_y);
        }
    }

    pub fn get_font_data(&self) -> &[u8] {
        &self.font_data
    }

    pub fn set_remap_palette(&mut self, palette: Option<&[u8]>) -> Option<Vec<u8>> {
        let old = self.remap_palette.take();
        self.remap_palette = palette.map(|data| data.to_vec());
        old
    }

    pub fn get_remap_palette(&self) -> Option<&[u8]> {
        self.remap_palette.as_deref()
    }

    pub fn set_converter(
        &mut self,
        converter: Option<Arc<ConvertClass>>,
    ) -> Option<Arc<ConvertClass>> {
        let old = self.converter.take();
        self.converter = converter;
        old
    }

    pub fn get_converter(&self) -> Option<Arc<ConvertClass>> {
        self.converter.clone()
    }

    fn raw_width(&self) -> i32 {
        if let Some(header) = self.font_header {
            let offset = header.info_block_offset as usize + FONTINFOMAXWIDTH;
            if offset < self.font_data.len() {
                return self.font_data[offset] as i32;
            }
        }
        0
    }

    fn raw_height(&self) -> i32 {
        if let Some(header) = self.font_header {
            let offset = header.info_block_offset as usize + FONTINFOMAXHEIGHT;
            if offset < self.font_data.len() {
                return self.font_data[offset] as i32;
            }
        }
        0
    }
}

impl FontClass for WWFontClass {
    fn char_pixel_width(&self, c: u8) -> i32 {
        if let Some(header) = self.font_header {
            let offset = header.width_block_offset as usize + c as usize;
            if offset < self.font_data.len() {
                return self.font_data[offset] as i32 + self.font_x_spacing;
            }
        }
        0
    }

    fn string_pixel_width(&self, string: &str) -> i32 {
        if string.is_empty() {
            return 0;
        }
        let mut largest = 0;
        let mut width = 0;
        for ch in string.bytes() {
            if ch == b'\r' || ch == b'\n' {
                if width > largest {
                    largest = width;
                }
                width = 0;
                continue;
            }
            width += self.char_pixel_width(ch);
        }
        if width > largest {
            largest = width;
        }
        largest
    }

    fn get_width(&self) -> i32 {
        let raw = self.raw_width();
        raw + if self.font_x_spacing > 0 {
            self.font_x_spacing
        } else {
            0
        }
    }

    fn get_height(&self) -> i32 {
        let raw = self.raw_height();
        raw + if self.font_y_spacing > 0 {
            self.font_y_spacing
        } else {
            0
        }
    }

    fn print(
        &self,
        string: &str,
        surface: &mut Surface,
        cliprect: Rect,
        drawpoint: Point2D,
        convertref: &ConvertClass,
        remap: Option<&[u8]>,
    ) -> Point2D {
        if string.is_empty() {
            return drawpoint;
        }

        let mut xpos = drawpoint.x + cliprect.x;
        let mut ypos = drawpoint.y + cliprect.y;

        let xspacing = self.font_x_spacing + self.raw_width() / FUDGEDIV;
        let yspacing = self.font_y_spacing + self.raw_width() / FUDGEDIV;

        let fontpalette: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let remap = if let Some(remap_palette) = self.remap_palette.as_deref() {
            remap_palette
        } else if let Some(remap) = remap {
            remap
        } else {
            &fontpalette
        };

        let converter = self.converter.as_deref().unwrap_or(convertref);

        if xpos >= cliprect.x + cliprect.width || ypos >= cliprect.y + cliprect.height {
            return drawpoint;
        }

        let header = match self.font_header {
            Some(header) => header,
            None => return drawpoint,
        };

        let lock = match surface.lock(Point2D::new(0, 0)) {
            Ok(lock) => lock,
            Err(_) => return drawpoint,
        };

        let startx = xpos;
        let fontwidth_base = header.width_block_offset as usize;
        let fontheight_base = header.height_offset as usize;
        let fontoffset_base = header.offset_block_offset as usize;
        let bbp = surface.bytes_per_pixel() as i32;

        for ch in string.bytes() {
            if ch == b'\r' {
                xpos = startx;
                ypos += self.raw_height() + if yspacing > 0 { yspacing } else { 0 };
                continue;
            }
            if ch == b'\n' {
                xpos = cliprect.x;
                ypos += self.raw_height() + if yspacing > 0 { yspacing } else { 0 };
                continue;
            }

            let width = *self
                .font_data
                .get(fontwidth_base + ch as usize)
                .unwrap_or(&0) as i32;
            let fontheight = if fontheight_base + (ch as usize * 2 + 1) < self.font_data.len() {
                let lo = self.font_data[fontheight_base + ch as usize * 2];
                let hi = self.font_data[fontheight_base + ch as usize * 2 + 1];
                u16::from_le_bytes([lo, hi])
            } else {
                0
            };
            let dheight = (fontheight >> 8) as i32;
            let firstrow = (fontheight & 0xFF) as i32;

            let crect = Rect::new(
                xpos,
                ypos,
                width + if xspacing > 0 { xspacing } else { 0 },
                self.raw_height() + if yspacing > 0 { yspacing } else { 0 },
            );

            let crect = match crect.intersect(cliprect) {
                Some(rect) => rect,
                None => {
                    xpos += self.char_pixel_width(ch);
                    continue;
                }
            };

            if remap.get(0).copied().unwrap_or(0) != 0 {
                let color = converter.convert_pixel(remap[0]);
                for y in crect.y..crect.y + crect.height {
                    for x in crect.x..crect.x + crect.width {
                        unsafe {
                            if let Some(ptr) = lock.get_pixel_ptr(Point2D::new(x, y)) {
                                write_pixel(ptr, bbp, color);
                            }
                        }
                    }
                }
            }

            let offset_index = fontoffset_base + (ch as usize * 2);
            let fontoffset = if offset_index + 1 < self.font_data.len() {
                let lo = self.font_data[offset_index];
                let hi = self.font_data[offset_index + 1];
                u16::from_le_bytes([lo, hi]) as usize
            } else {
                0
            };

            if header.font_compress != 2 {
                let mut dataptr = fontoffset;
                let mut draw_y = ypos + firstrow;
                for _ in 0..dheight {
                    if draw_y >= crect.y + crect.height {
                        break;
                    }

                    if draw_y < crect.y {
                        draw_y += 1;
                        dataptr += ((width + 1) / 2) as usize;
                        continue;
                    }

                    let mut dx = xpos;
                    let mut workwidth = width;
                    let mut row_ptr = dataptr;
                    while workwidth > 0 {
                        let packed = *self.font_data.get(row_ptr).unwrap_or(&0);
                        row_ptr += 1;
                        let c1 = remap[(packed & 0x0F) as usize];
                        let c2 = remap[((packed & 0xF0) >> 4) as usize];

                        if dx >= cliprect.x && dx < cliprect.x + cliprect.width && c1 != 0 {
                            unsafe {
                                if let Some(ptr) = lock.get_pixel_ptr(Point2D::new(dx, draw_y)) {
                                    write_pixel(ptr, bbp, converter.convert_pixel(c1));
                                }
                            }
                        }
                        dx += 1;
                        workwidth -= 1;
                        if workwidth == 0 {
                            break;
                        }

                        if dx >= cliprect.x && dx < cliprect.x + cliprect.width && c2 != 0 {
                            unsafe {
                                if let Some(ptr) = lock.get_pixel_ptr(Point2D::new(dx, draw_y)) {
                                    write_pixel(ptr, bbp, converter.convert_pixel(c2));
                                }
                            }
                        }
                        dx += 1;
                        workwidth -= 1;
                    }

                    draw_y += 1;
                    dataptr += ((width + 1) / 2) as usize;
                }
            } else {
                let mut dataptr = header.data_block_offset as usize + fontoffset;
                let mut draw_y = ypos + firstrow;
                for _ in 0..dheight {
                    if draw_y >= crect.y + crect.height {
                        break;
                    }

                    if draw_y < crect.y {
                        draw_y += 1;
                        dataptr += width as usize;
                        continue;
                    }

                    let mut dx = xpos;
                    let mut workwidth = width;
                    let mut row_ptr = dataptr;
                    while workwidth > 0 {
                        let c1 = remap[*self.font_data.get(row_ptr).unwrap_or(&0) as usize];
                        row_ptr += 1;
                        if dx >= cliprect.x && dx < cliprect.x + cliprect.width && c1 != 0 {
                            unsafe {
                                if let Some(ptr) = lock.get_pixel_ptr(Point2D::new(dx, draw_y)) {
                                    write_pixel(ptr, bbp, converter.convert_pixel(c1));
                                }
                            }
                        }
                        dx += 1;
                        workwidth -= 1;
                    }

                    draw_y += 1;
                    dataptr += width as usize;
                }
            }

            xpos += self.char_pixel_width(ch);
        }

        Point2D::new(xpos - cliprect.x, ypos - cliprect.y)
    }

    fn set_xspacing(&mut self, x: i32) -> i32 {
        let old = self.font_x_spacing;
        self.font_x_spacing = x;
        if self.is_outlined_data {
            match self.shadow {
                0 => self.font_x_spacing += -2,
                1 => self.font_x_spacing += -1,
                2 => self.font_x_spacing += -1,
                _ => {}
            }
        }
        self.font_x_spacing += self.get_width() / FUDGEDIV;
        old
    }

    fn set_yspacing(&mut self, y: i32) -> i32 {
        let old = self.font_y_spacing;
        self.font_y_spacing = y;
        if self.is_outlined_data {
            match self.shadow {
                0 => self.font_y_spacing += -2,
                1 => self.font_y_spacing += -1,
                2 => self.font_y_spacing += -1,
                _ => {}
            }
        }
        self.font_y_spacing += self.get_height() / FUDGEDIV;
        old
    }
}

unsafe fn write_pixel(ptr: *mut u8, bbp: i32, color: u32) {
    match bbp {
        1 => {
            *ptr = color as u8;
        }
        2 => {
            let bytes = (color as u16).to_le_bytes();
            *ptr = bytes[0];
            *ptr.add(1) = bytes[1];
        }
        3 => {
            let b = (color & 0xFF) as u8;
            let g = ((color >> 8) & 0xFF) as u8;
            let r = ((color >> 16) & 0xFF) as u8;
            *ptr = b;
            *ptr.add(1) = g;
            *ptr.add(2) = r;
        }
        4 => {
            let b = (color & 0xFF) as u8;
            let g = ((color >> 8) & 0xFF) as u8;
            let r = ((color >> 16) & 0xFF) as u8;
            let a = ((color >> 24) & 0xFF) as u8;
            *ptr = b;
            *ptr.add(1) = g;
            *ptr.add(2) = r;
            *ptr.add(3) = a;
        }
        _ => {}
    }
}
