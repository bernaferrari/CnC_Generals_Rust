//! C++ `OSDisplayWarningBox` — blocking native dialog, not auto-OK.

use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum OSDisplayButtonType {
    Ok = 0x0000_0001,
    Cancel = 0x0000_0002,
    Error = 0x8000_0000,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogIcon {
    None,
    Exclamation,
    Information,
    Error,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogModality {
    None,
    SystemModal,
    ApplicationModal,
    TaskModal,
}

#[derive(Debug, Clone)]
pub struct DialogConfig {
    pub show_ok: bool,
    pub show_cancel: bool,
    pub modality: DialogModality,
    pub icon: DialogIcon,
}

impl DialogConfig {
    pub fn from_flags(button_flags: u32, other_flags: u32) -> Self {
        let show_ok = (button_flags & OSDisplayButtonType::Ok as u32) != 0;
        let show_cancel = (button_flags & OSDisplayButtonType::Cancel as u32) != 0;
        let modality = if (other_flags & 0x1) != 0 {
            DialogModality::SystemModal
        } else if (other_flags & 0x2) != 0 {
            DialogModality::ApplicationModal
        } else if (other_flags & 0x4) != 0 {
            DialogModality::TaskModal
        } else {
            DialogModality::None
        };
        let icon = if (other_flags & 0x12) == 0x12 {
            DialogIcon::Stop
        } else if (other_flags & 0x11) == 0x11 {
            DialogIcon::Error
        } else if (other_flags & 0x8) != 0 {
            DialogIcon::Exclamation
        } else if (other_flags & 0x10) != 0 {
            DialogIcon::Information
        } else {
            DialogIcon::None
        };
        Self {
            show_ok,
            show_cancel,
            modality,
            icon,
        }
    }
}

pub trait GameTextProvider: Send + Sync {
    fn fetch(&self, key: &str) -> String;
}

static TEST_DIALOG_HOOK: Mutex<Option<fn(&str, &str, &DialogConfig) -> OSDisplayButtonType>> =
    Mutex::new(None);

pub fn set_os_display_test_hook(
    hook: Option<fn(&str, &str, &DialogConfig) -> OSDisplayButtonType>,
) {
    if let Ok(mut guard) = TEST_DIALOG_HOOK.lock() {
        *guard = hook;
    }
}

pub fn os_display_warning_box(
    game_text: Option<&dyn GameTextProvider>,
    prompt_key: &str,
    message_key: &str,
    button_flags: u32,
    other_flags: u32,
) -> OSDisplayButtonType {
    let Some(gt) = game_text else {
        return OSDisplayButtonType::Error;
    };
    os_display_warning_box_direct(
        &gt.fetch(prompt_key),
        &gt.fetch(message_key),
        button_flags,
        other_flags,
    )
}

pub fn os_display_warning_box_direct(
    prompt: &str,
    message: &str,
    button_flags: u32,
    other_flags: u32,
) -> OSDisplayButtonType {
    let config = DialogConfig::from_flags(button_flags, other_flags);
    if let Ok(guard) = TEST_DIALOG_HOOK.lock() {
        if let Some(hook) = *guard {
            return hook(prompt, message, &config);
        }
    }
    present_native_warning_box(prompt, message, &config)
}

fn present_native_warning_box(
    prompt: &str,
    message: &str,
    config: &DialogConfig,
) -> OSDisplayButtonType {
    #[cfg(target_os = "macos")]
    {
        return macos_alert(prompt, message, config);
    }
    #[cfg(target_os = "windows")]
    {
        return windows_message_box(prompt, message, config);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return unix_dialog(prompt, message, config);
    }
    #[allow(unreachable_code)]
    OSDisplayButtonType::Cancel
}

#[cfg(target_os = "macos")]
fn macos_alert(prompt: &str, message: &str, config: &DialogConfig) -> OSDisplayButtonType {
    let buttons = if config.show_cancel {
        r#"{"OK", "Cancel"}"#
    } else {
        r#"{"OK"}"#
    };
    let script = format!(
        r#"display dialog {message} with title {title} buttons {buttons} default button "OK""#,
        message = escape_as(message),
        title = escape_as(prompt),
        buttons = buttons,
    );
    match std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
    {
        Ok(result) if result.status.success() => {
            if String::from_utf8_lossy(&result.stdout).contains("Cancel") {
                OSDisplayButtonType::Cancel
            } else {
                OSDisplayButtonType::Ok
            }
        }
        _ => OSDisplayButtonType::Cancel,
    }
}

#[cfg(target_os = "macos")]
fn escape_as(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(target_os = "windows")]
fn windows_message_box(prompt: &str, message: &str, config: &DialogConfig) -> OSDisplayButtonType {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut core::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            flags: u32,
        ) -> i32;
    }

    let mut flags = if config.show_cancel { 0x1 } else { 0 };
    flags |= match config.icon {
        DialogIcon::Error | DialogIcon::Stop => 0x10,
        DialogIcon::Exclamation => 0x30,
        DialogIcon::Information => 0x40,
        DialogIcon::None => 0,
    };
    let text: Vec<u16> = OsStr::new(message).encode_wide().chain(Some(0)).collect();
    let caption: Vec<u16> = OsStr::new(prompt).encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        MessageBoxW(
            core::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            flags,
        )
    };
    if result == 1 {
        OSDisplayButtonType::Ok
    } else {
        OSDisplayButtonType::Cancel
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn unix_dialog(prompt: &str, message: &str, config: &DialogConfig) -> OSDisplayButtonType {
    let extra = if config.show_cancel {
        &["--ok-label=OK", "--cancel-label=Cancel"][..]
    } else {
        &["--ok-label=OK"][..]
    };
    if let Ok(status) = std::process::Command::new("zenity")
        .args(["--warning", "--title", prompt, "--text", message])
        .args(extra)
        .status()
    {
        return if status.success() {
            OSDisplayButtonType::Ok
        } else {
            OSDisplayButtonType::Cancel
        };
    }
    OSDisplayButtonType::Cancel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_game_text_is_error() {
        assert_eq!(
            os_display_warning_box(None, "p", "m", 1, 0),
            OSDisplayButtonType::Error
        );
    }

    #[test]
    fn hooked_dialog_returns_user_button() {
        set_os_display_test_hook(Some(|_, _, _| OSDisplayButtonType::Cancel));
        let result = os_display_warning_box_direct("Title", "Message", 3, 0);
        set_os_display_test_hook(None);
        assert_eq!(result, OSDisplayButtonType::Cancel);
    }
}
