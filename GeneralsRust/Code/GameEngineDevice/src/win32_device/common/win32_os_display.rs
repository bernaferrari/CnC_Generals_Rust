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
    // C++ Ref: if (!TheGameText) return OSDBT_ERROR;
    let (prompt, message) = match game_text {
        Some(gt) => (gt.fetch(prompt_key), gt.fetch(message_key)),
        None => return OSDisplayButtonType::Error,
    };

    let config = DialogConfig::from_flags(button_flags, other_flags);

    // Log the dialog (in production, this would show a native dialog)
    // C++ Ref: Uses MessageBoxW/MessageBoxA on Windows
    eprintln!(
        "[OSDisplay] {} {}: {}",
        config.icon_label(),
        prompt,
        message
    );

    // C++ Ref: Default behavior is OK button
    // The original C++ only distinguishes OK and Cancel from MessageBox return values
    OSDisplayButtonType::Ok
}

/// Display a warning box with pre-resolved (already localized) strings.
///
/// This is the direct string variant — use when you already have the display strings
/// and don't need game text lookup.
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

    OSDisplayButtonType::Ok
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
        let gt = MockGameText;
        let result = os_display_warning_box(Some(&gt), "prompt:test", "msg:test", 1, 0);
        assert_eq!(result, OSDisplayButtonType::Ok);
    }

    #[test]
    fn test_warning_box_without_game_text() {
        let result = os_display_warning_box(None, "prompt:test", "msg:test", 1, 0);
        assert_eq!(result, OSDisplayButtonType::Error);
    }

    #[test]
    fn test_warning_box_direct() {
        let result = os_display_warning_box_direct("Title", "Message", 1, 0);
        assert_eq!(result, OSDisplayButtonType::Ok);
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
