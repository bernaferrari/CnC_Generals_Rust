//! Win32OSDisplay - Platform-specific display warning dialogs
//!
//! Corresponds to C++ file: GameEngineDevice/Source/Win32Device/Common/Win32OSDisplay.cpp
//! Original author: John McDonald, December 2002
//!
//! This module provides the platform-specific implementation of OSDisplayWarningBox,
//! which shows a warning/error dialog to the user. In C++ this uses the Win32 MessageBox API.
//! In Rust, we provide a cross-platform abstraction that matches the C++ behavior.

// ---- Enums matching C++ OSDisplay.h ----

/// Button types for OS display dialogs
/// C++ Ref: OSDisplay.h enum OSDisplayButtonType
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum OSDisplayButtonType {
    /// OK button was pressed
    Ok = 0x00000001,
    /// Cancel button was pressed
    Cancel = 0x00000002,
    /// Error occurred (could not display dialog)
    Error = 0x80000000,
}

/// Additional flags for OS display dialogs
/// C++ Ref: OSDisplay.h enum OSDisplayOtherFlags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum OSDisplayOtherFlags {
    /// System modal dialog (blocks all windows)
    SystemModal = 0x00000001,
    /// Application modal dialog
    ApplicationModal = 0x00000002,
    /// Task modal dialog
    TaskModal = 0x00000004,
    /// Show exclamation icon
    ExclamationIcon = 0x00000008,
    /// Show information icon
    InformationIcon = 0x00000010,
    /// Show error icon
    ErrorIcon = 0x00000011,
    /// Show stop icon
    StopIcon = 0x00000012,
    /// Error flag
    OddError = 0x80000000,
}

/// Dialog icon type for cross-platform rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogIcon {
    None,
    Exclamation,
    Information,
    Error,
    Stop,
}

/// Dialog modality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogModality {
    /// Modeless (default)
    None,
    /// Blocks all windows in the system
    SystemModal,
    /// Blocks windows in the application
    ApplicationModal,
    /// Blocks windows in the current task
    TaskModal,
}

/// Parsed dialog configuration from button/other flags
#[derive(Debug, Clone)]
pub struct DialogConfig {
    pub show_ok: bool,
    pub show_cancel: bool,
    pub modality: DialogModality,
    pub icon: DialogIcon,
}

impl DialogConfig {
    /// Parse dialog configuration from C++ style button and other flags
    /// C++ Ref: Win32OSDisplay.cpp RTSFlagsToOSFlags()
    pub fn from_flags(button_flags: u32, other_flags: u32) -> Self {
        let show_ok = (button_flags & OSDisplayButtonType::Ok as u32) != 0;
        let show_cancel = (button_flags & OSDisplayButtonType::Cancel as u32) != 0;

        let modality = if (other_flags & OSDisplayOtherFlags::SystemModal as u32) != 0 {
            DialogModality::SystemModal
        } else if (other_flags & OSDisplayOtherFlags::ApplicationModal as u32) != 0 {
            DialogModality::ApplicationModal
        } else if (other_flags & OSDisplayOtherFlags::TaskModal as u32) != 0 {
            DialogModality::TaskModal
        } else {
            DialogModality::None
        };

        // C++ Ref: OSDOF_ERRORICON = 0x11, OSDOF_STOPICON = 0x12 — these overlap with combinations
        let icon = if (other_flags & 0x12) == 0x12 {
            DialogIcon::Stop
        } else if (other_flags & 0x11) == 0x11 {
            DialogIcon::Error
        } else if (other_flags & OSDisplayOtherFlags::ExclamationIcon as u32) != 0 {
            DialogIcon::Exclamation
        } else if (other_flags & OSDisplayOtherFlags::InformationIcon as u32) != 0 {
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

    /// Returns a human-readable icon label
    pub fn icon_label(&self) -> &str {
        match self.icon {
            DialogIcon::Exclamation => "⚠",
            DialogIcon::Information => "ℹ",
            DialogIcon::Error => "✕",
            DialogIcon::Stop => "🛑",
            DialogIcon::None => "",
        }
    }
}

// ---- Game text interface ----

/// Trait for looking up localized game text strings
/// C++ Ref: TheGameText->fetch(p) in Win32OSDisplay.cpp
pub trait GameTextProvider: Send + Sync {
    /// Fetch a localized string by key
    fn fetch(&self, key: &str) -> String;
}

// ---- Warning box display ----

/// Result of displaying a warning box
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarningBoxResult {
    pub button: OSDisplayButtonType,
}

/// Display a warning box to the user with the specified localized prompt and message.
///
/// C++ Ref: Win32OSDisplay.cpp OSDisplayWarningBox()
///
/// In C++, this function:
/// 1. Checks if TheGameText exists (returns OSDBT_ERROR if not)
/// 2. Fetches localized prompt and message strings
/// 3. Translates game flags to Win32 MessageBox flags
/// 4. Shows MessageBoxW (Unicode) or MessageBoxA (ASCII fallback)
/// 5. Returns OSDBT_OK or OSDBT_CANCEL based on user response
///
/// In Rust, this delegates to a platform-appropriate dialog mechanism.
/// On CI/headless environments, it logs and returns Ok.
pub fn os_display_warning_box(
    game_text: Option<&dyn GameTextProvider>,
    prompt_key: &str,
    message_key: &str,
    button_flags: u32,
    other_flags: u32,
) -> OSDisplayButtonType {
    let (prompt, message) = match game_text {
        Some(gt) => (gt.fetch(prompt_key), gt.fetch(message_key)),
        None => return OSDisplayButtonType::Error,
    };
    os_display_warning_box_direct(&prompt, &message, button_flags, other_flags)
}

pub fn os_display_warning_box_direct(
    prompt: &str,
    message: &str,
    button_flags: u32,
    other_flags: u32,
) -> OSDisplayButtonType {
    let config = DialogConfig::from_flags(button_flags, other_flags);
    eprintln!(
        "[OSDisplay] {} {}: {}",
        config.icon_label(),
        prompt,
        message
    );
    if let Some(hook) = test_dialog_hook() {
        return hook(prompt, message, &config);
    }
    present_native_warning_box(prompt, message, &config)
}

fn test_dialog_hook() -> Option<fn(&str, &str, &DialogConfig) -> OSDisplayButtonType> {
    TEST_DIALOG_HOOK.lock().ok().and_then(|guard| *guard)
}

/// Tests install a hook so unit tests do not block on a real MessageBox.
pub fn set_os_display_test_hook(
    hook: Option<fn(&str, &str, &DialogConfig) -> OSDisplayButtonType>,
) {
    if let Ok(mut guard) = TEST_DIALOG_HOOK.lock() {
        *guard = hook;
    }
}

static TEST_DIALOG_HOOK: std::sync::Mutex<
    Option<fn(&str, &str, &DialogConfig) -> OSDisplayButtonType>,
> = std::sync::Mutex::new(None);

fn present_native_warning_box(
    prompt: &str,
    message: &str,
    config: &DialogConfig,
) -> OSDisplayButtonType {
    #[cfg(target_os = "windows")]
    {
        return windows_message_box(prompt, message, config);
    }
    #[cfg(target_os = "macos")]
    {
        return macos_alert(prompt, message, config);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return unix_zenity_or_kdialog(prompt, message, config);
    }
    #[allow(unreachable_code)]
    OSDisplayButtonType::Cancel
}

#[cfg(target_os = "windows")]
fn windows_message_box(
    prompt: &str,
    message: &str,
    config: &DialogConfig,
) -> OSDisplayButtonType {
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::OsStr;

    const MB_OK: u32 = 0x0000_0000;
    const MB_OKCANCEL: u32 = 0x0000_0001;
    const MB_SYSTEMMODAL: u32 = 0x0000_1000;
    const MB_APPLMODAL: u32 = 0x0000_0000;
    const MB_TASKMODAL: u32 = 0x0000_2000;
    const MB_ICONEXCLAMATION: u32 = 0x0000_0030;
    const MB_ICONINFORMATION: u32 = 0x0000_0040;
    const MB_ICONERROR: u32 = 0x0000_0010;
    const IDOK: i32 = 1;

    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut core::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            flags: u32,
        ) -> i32;
    }

    let mut flags = if config.show_cancel { MB_OKCANCEL } else { MB_OK };
    flags |= match config.modality {
        DialogModality::SystemModal => MB_SYSTEMMODAL,
        DialogModality::TaskModal => MB_TASKMODAL,
        _ => MB_APPLMODAL,
    };
    flags |= match config.icon {
        DialogIcon::Exclamation => MB_ICONEXCLAMATION,
        DialogIcon::Information => MB_ICONINFORMATION,
        DialogIcon::Error | DialogIcon::Stop => MB_ICONERROR,
        DialogIcon::None => 0,
    };
    let text: Vec<u16> = OsStr::new(message).encode_wide().chain(Some(0)).collect();
    let caption: Vec<u16> = OsStr::new(prompt).encode_wide().chain(Some(0)).collect();
    // SAFETY: text and caption are NUL-terminated wide buffers that outlive the
    // SAFETY: call; the null hwnd means no owner window. MessageBoxW only reads
    // SAFETY: the two strings and returns the pressed-button code.
    let result = unsafe { MessageBoxW(core::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), flags) };
    if result == IDOK {
        OSDisplayButtonType::Ok
    } else {
        OSDisplayButtonType::Cancel
    }
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
        message = apple_script_string(message),
        title = apple_script_string(prompt),
        buttons = buttons,
    );
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output();
    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            if stdout.contains("Cancel") {
                OSDisplayButtonType::Cancel
            } else {
                OSDisplayButtonType::Ok
            }
        }
        _ => OSDisplayButtonType::Cancel,
    }
}

#[cfg(target_os = "macos")]
fn apple_script_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn unix_zenity_or_kdialog(
    prompt: &str,
    message: &str,
    config: &DialogConfig,
) -> OSDisplayButtonType {
    let extra = if config.show_cancel {
        &["--ok-label=OK", "--cancel-label=Cancel"][..]
    } else {
        &["--ok-label=OK"][..]
    };
    let zenity = std::process::Command::new("zenity")
        .args(["--warning", "--title", prompt, "--text", message])
        .args(extra)
        .status();
    if let Ok(status) = zenity {
        return if status.success() {
            OSDisplayButtonType::Ok
        } else {
            OSDisplayButtonType::Cancel
        };
    }
    let kdialog = std::process::Command::new("kdialog")
        .args(["--title", prompt, "--sorry", message])
        .status();
    match kdialog {
        Ok(status) if status.success() => OSDisplayButtonType::Ok,
        _ => OSDisplayButtonType::Cancel,
    }
}

/// Check whether the system supports Unicode display.
/// C++ Ref: TheSystemIsUnicode in Win32OSDisplay.cpp
pub fn is_system_unicode() -> bool {
    // Rust strings are always UTF-8, so effectively always "Unicode"
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockGameText;
    impl GameTextProvider for MockGameText {
        fn fetch(&self, key: &str) -> String {
            match key {
                "prompt:test" => "Test Prompt".to_string(),
                "msg:test" => "Test Message".to_string(),
                _ => format!("<{}>", key),
            }
        }
    }

    #[test]
    fn test_dialog_config_ok_only() {
        let config = DialogConfig::from_flags(OSDisplayButtonType::Ok as u32, 0);
        assert!(config.show_ok);
        assert!(!config.show_cancel);
        assert_eq!(config.modality, DialogModality::None);
        assert_eq!(config.icon, DialogIcon::None);
    }

    #[test]
    fn test_dialog_config_ok_cancel() {
        let config = DialogConfig::from_flags(
            OSDisplayButtonType::Ok as u32 | OSDisplayButtonType::Cancel as u32,
            0,
        );
        assert!(config.show_ok);
        assert!(config.show_cancel);
    }

    #[test]
    fn test_dialog_config_error_icon() {
        let config = DialogConfig::from_flags(
            OSDisplayButtonType::Ok as u32,
            OSDisplayOtherFlags::ErrorIcon as u32,
        );
        assert_eq!(config.icon, DialogIcon::Error);
    }

    #[test]
    fn test_dialog_config_exclamation_icon() {
        let config = DialogConfig::from_flags(
            OSDisplayButtonType::Ok as u32,
            OSDisplayOtherFlags::ExclamationIcon as u32,
        );
        assert_eq!(config.icon, DialogIcon::Exclamation);
    }

    #[test]
    fn test_dialog_config_system_modal() {
        let config = DialogConfig::from_flags(
            OSDisplayButtonType::Ok as u32,
            OSDisplayOtherFlags::SystemModal as u32,
        );
        assert_eq!(config.modality, DialogModality::SystemModal);
    }

    #[test]
    fn test_warning_box_with_game_text() {
        set_os_display_test_hook(Some(|_, _, _| OSDisplayButtonType::Ok));
        let gt = MockGameText;
        let result = os_display_warning_box(Some(&gt), "prompt:test", "msg:test", 1, 0);
        set_os_display_test_hook(None);
        assert_eq!(result, OSDisplayButtonType::Ok);
    }

    #[test]
    fn test_warning_box_without_game_text() {
        let result = os_display_warning_box(None, "prompt:test", "msg:test", 1, 0);
        assert_eq!(result, OSDisplayButtonType::Error);
    }

    #[test]
    fn test_warning_box_direct() {
        set_os_display_test_hook(Some(|_, _, _| OSDisplayButtonType::Cancel));
        let result = os_display_warning_box_direct("Title", "Message", 1 | 2, 0);
        set_os_display_test_hook(None);
        assert_eq!(result, OSDisplayButtonType::Cancel);
    }

    #[test]
    fn test_is_system_unicode() {
        assert!(is_system_unicode());
    }

    #[test]
    fn test_dialog_icon_label() {
        let config = DialogConfig::from_flags(1, OSDisplayOtherFlags::ErrorIcon as u32);
        assert_eq!(config.icon_label(), "✕");
    }
}
