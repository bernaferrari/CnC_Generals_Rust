//! CPP ScriptActions2637: configure the real Drawable; only its update pulses.
use crate::common::Color;
use crate::object::drawable::Drawable;

pub(crate) fn apply_script_drawable_flash(drawable: &mut Drawable, seconds: i32, color: Color) {
    if seconds <= 0 {
        return;
    }
    let count = seconds.wrapping_mul(30) / 15;
    drawable.set_flash_color(color);
    drawable.set_flash_count(count);
}

pub(crate) fn script_indicator_color(raw: u32) -> Color {
    Color::new(
        (raw >> 16) as u8,
        (raw >> 8) as u8,
        raw as u8,
        (raw >> 24) as u8,
    )
}
