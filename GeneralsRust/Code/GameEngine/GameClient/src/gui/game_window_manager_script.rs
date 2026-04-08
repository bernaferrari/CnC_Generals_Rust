use crate::gui::window_script::{parse_window_script, WindowLayoutDefinition, WindowScriptError};
use std::path::Path;

pub const WIN_BUFFER_LENGTH: usize = 2048;
pub const WIN_STACK_DEPTH: usize = 10;

pub const WINDOW_STATUS_NAMES: &[&str] = &[
    "ACTIVE",
    "TOGGLE",
    "DRAGABLE",
    "ENABLED",
    "HIDDEN",
    "ABOVE",
    "BELOW",
    "IMAGE",
    "TABSTOP",
    "NOINPUT",
    "NOFOCUS",
    "DESTROYED",
    "BORDER",
    "SMOOTH_TEXT",
    "ONE_LINE",
    "NO_FLUSH",
    "SEE_THRU",
    "RIGHT_CLICK",
    "WRAP_CENTERED",
    "CHECK_LIKE",
    "HOTKEY_TEXT",
    "USE_OVERLAY_STATES",
    "NOT_READY",
    "FLASHING",
    "ALWAYS_COLOR",
    "ON_MOUSE_DOWN",
];

pub const WINDOW_STYLE_NAMES: &[&str] = &[
    "PUSHBUTTON",
    "RADIOBUTTON",
    "CHECKBOX",
    "VERTSLIDER",
    "HORZSLIDER",
    "SCROLLLISTBOX",
    "ENTRYFIELD",
    "STATICTEXT",
    "PROGRESSBAR",
    "USER",
    "MOUSETRACK",
    "ANIMATED",
    "TABSTOP",
    "TABCONTROL",
    "TABPANE",
    "COMBOBOX",
];

pub fn parse_bit_flag(flag: &str, bits: &mut u32, flag_list: &[&str]) -> bool {
    if let Some(index) = flag_list
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(flag))
    {
        *bits |= 1 << index;
        true
    } else {
        false
    }
}

pub fn parse_bit_string(value: &str, bits: &mut u32, flag_list: &[&str]) {
    if value.trim().eq_ignore_ascii_case("NULL") {
        return;
    }
    for token in value
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        let _ = parse_bit_flag(token, bits, flag_list);
    }
}

pub fn scan_bool(source: &str) -> Option<bool> {
    source.trim().parse::<i32>().ok().map(|value| value != 0)
}

pub fn scan_short(source: &str) -> Option<i16> {
    source.trim().parse::<i16>().ok()
}

pub fn scan_int(source: &str) -> Option<i32> {
    source.trim().parse::<i32>().ok()
}

pub fn scan_unsigned_int(source: &str) -> Option<u32> {
    source.trim().parse::<u32>().ok()
}

pub fn load_window_layout(
    path: impl AsRef<Path>,
) -> Result<WindowLayoutDefinition, WindowScriptError> {
    parse_window_script(path.as_ref())
}

// PARITY_NOTE: the full parser lives in `gui::window_script`; this compatibility module preserves
// the legacy token tables and small helper routines that `GameWindowManagerScript.cpp` exposed.
