//! Compatibility layer for _xmouse.h.

use crate::xmouse::{with_mouse_cursor, Mouse, Rect};
use std::sync::Arc;

pub fn set_mouse_cursor(cursor: Option<Arc<dyn Mouse>>) {
    crate::xmouse::set_mouse_cursor(cursor);
}

pub fn hide_mouse() {
    let _ = with_mouse_cursor(|cursor| cursor.hide_mouse());
}

pub fn show_mouse() {
    let _ = with_mouse_cursor(|cursor| cursor.show_mouse());
}

pub fn conditional_hide_mouse(rect: Rect) {
    let _ = with_mouse_cursor(|cursor| cursor.conditional_hide_mouse(rect));
}

pub fn conditional_show_mouse() {
    let _ = with_mouse_cursor(|cursor| cursor.conditional_show_mouse());
}

pub fn get_mouse_state() -> i32 {
    with_mouse_cursor(|cursor| cursor.get_mouse_state()).unwrap_or(0)
}

pub fn set_mouse_cursor_shape(hotx: i32, hoty: i32, cursor: *const (), shape: i32) {
    let _ = with_mouse_cursor(|cursor_ref| cursor_ref.set_cursor(hotx, hoty, cursor, shape));
}

pub fn get_mouse_x() -> i32 {
    with_mouse_cursor(|cursor| cursor.get_mouse_x()).unwrap_or(0)
}

pub fn get_mouse_y() -> i32 {
    with_mouse_cursor(|cursor| cursor.get_mouse_y()).unwrap_or(0)
}
