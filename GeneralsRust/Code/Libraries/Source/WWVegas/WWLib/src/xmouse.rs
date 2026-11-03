//! Mouse cursor interface (ported from WWLib xmouse.h).

use crate::trect::TRect;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, Default)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub buttons: u32,
}

pub type Rect = TRect;

pub trait Mouse: Send + Sync {
    fn set_cursor(&self, x_hotspot: i32, y_hotspot: i32, cursor: *const (), shape: i32);
    fn hide_mouse(&self);
    fn show_mouse(&self);
    fn release_mouse(&self);
    fn capture_mouse(&self);
    fn is_captured(&self) -> bool;
    fn conditional_hide_mouse(&self, region: Rect);
    fn conditional_show_mouse(&self);
    fn get_mouse_state(&self) -> i32;
    fn get_mouse_x(&self) -> i32;
    fn get_mouse_y(&self) -> i32;
    fn set_mouse_xy(&self, xpos: i32, ypos: i32);
    fn draw_mouse(&self, _surface: *mut (), _is_sidebar_surface: bool);
    fn erase_mouse(&self, _surface: *mut (), _is_sidebar_surface: bool);
    fn convert_coordinate(&self, x: &mut i32, y: &mut i32);
}

pub fn set_mouse_cursor(runtime: &crate::process_state::WwLibRuntime, cursor: Option<Arc<dyn Mouse>>) {
    *runtime.mouse.lock().unwrap() = cursor;
}

pub fn with_mouse_cursor<F, R>(runtime: &crate::process_state::WwLibRuntime, f: F) -> Option<R>
where
    F: FnOnce(&dyn Mouse) -> R,
{
    let guard = runtime.mouse.lock().unwrap();
    guard.as_ref().map(|cursor| f(cursor.as_ref()))
}
