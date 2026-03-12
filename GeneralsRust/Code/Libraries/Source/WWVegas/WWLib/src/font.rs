//! Font interface (ported from WWLib font.h).

use crate::point::Point2D;
use crate::surface::{Rect, Surface};

use crate::convert::ConvertClass;

/// Abstract font interface.
pub trait FontClass {
    fn char_pixel_width(&self, c: u8) -> i32;
    fn string_pixel_width(&self, string: &str) -> i32;
    fn get_width(&self) -> i32;
    fn get_height(&self) -> i32;
    fn print(
        &self,
        string: &str,
        surface: &mut Surface,
        cliprect: Rect,
        point: Point2D,
        converter: &ConvertClass,
        remap: Option<&[u8]>,
    ) -> Point2D;

    fn set_xspacing(&mut self, x: i32) -> i32;
    fn set_yspacing(&mut self, y: i32) -> i32;
}
