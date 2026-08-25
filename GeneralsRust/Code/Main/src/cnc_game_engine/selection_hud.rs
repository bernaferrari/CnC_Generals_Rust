//! Selection / HUD click gates that C++ owns in Mouse / SelectionXlat / LookAtXlat.

pub(super) use crate::pick_ray::{
    OS_DOUBLE_CLICK_SLOP_PX, is_os_style_double_click, world_lmb_selection_allowed,
};

/// Win32 `GetDoubleClickTime` default is 500ms; honor the OS when we can.
pub(super) fn os_double_click_time_ms() -> u128 {
    platform_double_click_time_ms().unwrap_or(500)
}

fn platform_double_click_time_ms() -> Option<u128> {
    #[cfg(windows)]
    {
        // SAFETY: `GetDoubleClickTime` is a documented user32 query with no
        // pointer arguments and a millisecond return.
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetDoubleClickTime() -> u32;
        }
        let ms = unsafe { GetDoubleClickTime() };
        return (ms > 0).then_some(u128::from(ms));
    }
    #[cfg(target_os = "macos")]
    {
        return macos_double_click_interval_ms();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn macos_double_click_interval_ms() -> Option<u128> {
    use std::ffi::c_void;
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const i8) -> *mut c_void;
        fn sel_registerName(name: *const i8) -> *mut c_void;
        fn objc_msgSend(obj: *mut c_void, sel: *mut c_void) -> f64;
    }
    // SAFETY: `NSEvent.doubleClickInterval` is a class method returning NSTimeInterval
    // (f64). The selectors are interned C strings with static lifetime.
    unsafe {
        let class = objc_getClass(c"NSEvent".as_ptr());
        let sel = sel_registerName(c"doubleClickInterval".as_ptr());
        if class.is_null() || sel.is_null() {
            return None;
        }
        let secs = objc_msgSend(class, sel);
        if secs.is_finite() && secs > 0.0 {
            Some((secs * 1000.0).round() as u128)
        } else {
            None
        }
    }
}

/// C++ `LookAtXlat.cpp:352-358` + `SelectionXlat.cpp:1253-1258` OPTIONS.
pub(super) fn meta_options_clears_lookat_and_drag() -> (bool, bool) {
    (false, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_click_uses_screen_pixels_not_world_units() {
        // Given: OS 500ms / 4px, a 3px wobble that is 15wu on the terrain.
        // When: the second click is inside the OS rectangle.
        // Then: it is a double-click even though world delta is > 10wu.
        assert!(is_os_style_double_click(
            200,
            3.0,
            0.0,
            500,
            OS_DOUBLE_CLICK_SLOP_PX
        ));
        // Given: 6px wobble (outside SM_CXDOUBLECLK).
        // Then: not a double-click even if world delta is tiny.
        assert!(!is_os_style_double_click(
            200,
            6.0,
            0.0,
            500,
            OS_DOUBLE_CLICK_SLOP_PX
        ));
        // Given: past the OS interval.
        assert!(!is_os_style_double_click(
            600,
            0.0,
            0.0,
            500,
            OS_DOUBLE_CLICK_SLOP_PX
        ));
    }

    #[test]
    fn os_double_click_time_is_positive() {
        assert!(os_double_click_time_ms() > 0);
    }

    #[test]
    fn quit_menu_destroys_world_left_click() {
        assert!(!world_lmb_selection_allowed(true));
        assert!(world_lmb_selection_allowed(false));
    }

    #[test]
    fn options_stops_rmb_scroll_and_cancels_drag() {
        let (scrolling, dragging) = meta_options_clears_lookat_and_drag();
        assert!(!scrolling);
        assert!(!dragging);
    }
}
