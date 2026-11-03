//! Compatibility layer for _xmouse.h.

use crate::process_state::WwLibRuntime;
use crate::xmouse::{Mouse, Rect, with_mouse_cursor};
use std::sync::Arc;

pub fn set_mouse_cursor(runtime: &WwLibRuntime, cursor: Option<Arc<dyn Mouse>>) {
    crate::xmouse::set_mouse_cursor(runtime, cursor);
}

pub fn hide_mouse(runtime: &WwLibRuntime) {
    let _ = with_mouse_cursor(runtime, |cursor| cursor.hide_mouse());
}

pub fn show_mouse(runtime: &WwLibRuntime) {
    let _ = with_mouse_cursor(runtime, |cursor| cursor.show_mouse());
}

pub fn conditional_hide_mouse(runtime: &WwLibRuntime, rect: Rect) {
    let _ = with_mouse_cursor(runtime, |cursor| cursor.conditional_hide_mouse(rect));
}

pub fn conditional_show_mouse(runtime: &WwLibRuntime) {
    let _ = with_mouse_cursor(runtime, |cursor| cursor.conditional_show_mouse());
}

pub fn get_mouse_state(runtime: &WwLibRuntime) -> i32 {
    with_mouse_cursor(runtime, |cursor| cursor.get_mouse_state()).unwrap_or(0)
}

pub fn set_mouse_cursor_shape(
    runtime: &WwLibRuntime,
    hotx: i32,
    hoty: i32,
    cursor: *const (),
    shape: i32,
) {
    let _ = with_mouse_cursor(runtime, |cursor_ref| {
        cursor_ref.set_cursor(hotx, hoty, cursor, shape)
    });
}

pub fn get_mouse_x(runtime: &WwLibRuntime) -> i32 {
    with_mouse_cursor(runtime, |cursor| cursor.get_mouse_x()).unwrap_or(0)
}

pub fn get_mouse_y(runtime: &WwLibRuntime) -> i32 {
    with_mouse_cursor(runtime, |cursor| cursor.get_mouse_y()).unwrap_or(0)
}
