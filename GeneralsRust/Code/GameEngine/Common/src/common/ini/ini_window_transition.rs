//! INI parser for WindowTransition
//!
//! Reference: GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowTransitions.cpp
//! Reference: GeneralsMD/Code/GameEngine/Include/GameClient/GameWindowTransitions.h
//! Parses [WindowTransition] blocks from INI files for GUI window animations.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use super::ini::{INIError, INIResult, INI};

/// Transition style types - matches C++ TransitionStyleNames lookup table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum TransitionStyle {
    #[default]
    Flash = 0,
    ButtonFlash = 1,
    WinFade = 2,
    WinScaleUp = 3,
    MainMenuScaleUp = 4,
    TextType = 5,
    ScreenFade = 6,
    CountUp = 7,
    FullFade = 8,
    TextOnFrame = 9,
    MainMenuMediumScaleUp = 10,
    MainMenuSmallScaleDown = 11,
    ControlBarArrow = 12,
    ScoreScaleUp = 13,
    ReverseSound = 14,
    // Keep this last
    MaxTransitionWindowStyles = 15,
}

impl TransitionStyle {
    /// Parse transition style from string
    /// Matches C++ TransitionStyleNames lookup table
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "flash" => Some(TransitionStyle::Flash),
            "buttonflash" => Some(TransitionStyle::ButtonFlash),
            "winfade" => Some(TransitionStyle::WinFade),
            "winscaleup" => Some(TransitionStyle::WinScaleUp),
            "mainmenuscaleup" => Some(TransitionStyle::MainMenuScaleUp),
            "typetext" => Some(TransitionStyle::TextType),
            "screenfade" => Some(TransitionStyle::ScreenFade),
            "countup" => Some(TransitionStyle::CountUp),
            "fullfade" => Some(TransitionStyle::FullFade),
            "textonframe" => Some(TransitionStyle::TextOnFrame),
            "mainmenumediumscaleup" => Some(TransitionStyle::MainMenuMediumScaleUp),
            "mainmenussmallscaledown" => Some(TransitionStyle::MainMenuSmallScaleDown),
            "controlbararrow" => Some(TransitionStyle::ControlBarArrow),
            "scorescaleup" => Some(TransitionStyle::ScoreScaleUp),
            "reversesound" => Some(TransitionStyle::ReverseSound),
            _ => None,
        }
    }

    /// Convert to string representation
    pub fn to_str(&self) -> &'static str {
        match self {
            TransitionStyle::Flash => "FLASH",
            TransitionStyle::ButtonFlash => "BUTTONFLASH",
            TransitionStyle::WinFade => "WINFADE",
            TransitionStyle::WinScaleUp => "WINSCALEUP",
            TransitionStyle::MainMenuScaleUp => "MAINMENUSCALEUP",
            TransitionStyle::TextType => "TYPETEXT",
            TransitionStyle::ScreenFade => "SCREENFADE",
            TransitionStyle::CountUp => "COUNTUP",
            TransitionStyle::FullFade => "FULLFADE",
            TransitionStyle::TextOnFrame => "TEXTONFRAME",
            TransitionStyle::MainMenuMediumScaleUp => "MAINMENUMEDIUMSCALEUP",
            TransitionStyle::MainMenuSmallScaleDown => "MAINMENUSMALLSCALEDOWN",
            TransitionStyle::ControlBarArrow => "CONTROLBARARROW",
            TransitionStyle::ScoreScaleUp => "SCORESCALEUP",
            TransitionStyle::ReverseSound => "REVERSESOUND",
            TransitionStyle::MaxTransitionWindowStyles => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for TransitionStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

/// Transition window definition
/// Matches C++ TransitionWindow class from GameWindowTransitions.h
#[derive(Debug, Clone)]
pub struct TransitionWindow {
    /// Window name (INI parsed) - m_winName in C++
    pub win_name: String,
    /// Frame delay before transition starts (INI parsed) - m_frameDelay in C++
    pub frame_delay: i32,
    /// Transition style (INI parsed) - m_style in C++
    pub style: TransitionStyle,
}

impl Default for TransitionWindow {
    fn default() -> Self {
        Self {
            win_name: String::new(),
            frame_delay: 0,
            style: TransitionStyle::default(),
        }
    }
}

impl TransitionWindow {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Transition group definition
/// Matches C++ TransitionGroup class from GameWindowTransitions.h
#[derive(Debug, Clone)]
pub struct TransitionGroup {
    /// Group name
    pub name: String,
    /// Fire once flag - m_fireOnce in C++
    pub fire_once: bool,
    /// List of transition windows
    pub windows: Vec<TransitionWindow>,
}

impl TransitionGroup {
    pub fn new(name: String) -> Self {
        Self {
            name,
            fire_once: false,
            windows: Vec::new(),
        }
    }

    /// Add a transition window to this group
    pub fn add_window(&mut self, window: TransitionWindow) {
        self.windows.push(window);
    }
}

/// Window transition store / manager
/// Matches C++ GameWindowTransitionsHandler functionality
#[derive(Debug, Default)]
pub struct WindowTransitionStore {
    groups: HashMap<String, TransitionGroup>,
}

impl WindowTransitionStore {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    /// Create or get a new transition group by name
    /// If the group already exists, it's replaced (matches C++ getNewGroup behavior)
    pub fn new_group(&mut self, name: String) -> &mut TransitionGroup {
        self.groups.remove(&name.to_lowercase());
        let group = TransitionGroup::new(name.clone());
        self.groups.insert(name.to_lowercase(), group);
        self.groups.get_mut(&name.to_lowercase()).unwrap()
    }

    /// Find a transition group by name
    pub fn find_group(&self, name: &str) -> Option<&TransitionGroup> {
        self.groups.get(&name.to_lowercase())
    }

    /// Get a mutable reference to a transition group
    pub fn get_group_mut(&mut self, name: &str) -> Option<&mut TransitionGroup> {
        self.groups.get_mut(&name.to_lowercase())
    }

    /// Clear all groups
    pub fn clear(&mut self) {
        self.groups.clear();
    }

    /// Get number of groups
    pub fn len(&self) -> usize {
        self.groups.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

/// Global window transition store singleton
static WINDOW_TRANSITION_STORE: OnceLock<RwLock<WindowTransitionStore>> = OnceLock::new();

/// Get the window transition store (read guard)
pub fn get_window_transition_store() -> std::sync::RwLockReadGuard<'static, WindowTransitionStore> {
    WINDOW_TRANSITION_STORE
        .get_or_init(|| RwLock::new(WindowTransitionStore::new()))
        .read()
        .unwrap()
}

/// Get the window transition store (write guard)
pub fn get_window_transition_store_mut(
) -> std::sync::RwLockWriteGuard<'static, WindowTransitionStore> {
    WINDOW_TRANSITION_STORE
        .get_or_init(|| RwLock::new(WindowTransitionStore::new()))
        .write()
        .unwrap()
}

/// Initialize the window transition store
pub fn init_window_transition_store() {
    let _ = WINDOW_TRANSITION_STORE.get_or_init(|| RwLock::new(WindowTransitionStore::new()));
}

/// Parse bool value from string
fn parse_bool(value: &str) -> INIResult<bool> {
    match value.to_ascii_lowercase().as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(INIError::InvalidData),
    }
}

/// Parse Window sub-block for TransitionGroup
/// Matches C++ GameWindowTransitionsHandler::parseWindow
fn parse_window_field(ini: &mut INI, group: &mut TransitionGroup) -> INIResult<()> {
    let mut window = TransitionWindow::new();

    loop {
        ini.read_line()?;

        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let tokens = ini.get_line_tokens();
        let Some(first) = tokens.first() else {
            continue;
        };

        // Check for End of Window block
        if first.eq_ignore_ascii_case("End") {
            break;
        }

        let field_name = first;
        let value_idx = if tokens.len() > 2 && tokens[1] == "=" {
            2
        } else {
            1
        };

        if value_idx >= tokens.len() {
            continue;
        }

        match field_name.to_lowercase().as_str() {
            "winname" => {
                // WinName = <window_name>
                window.win_name = tokens[value_idx].to_string();
            }
            "style" => {
                // Style = <style_name>
                window.style = TransitionStyle::from_str(tokens[value_idx]).ok_or_else(|| {
                    log::warn!("Unknown transition style: {}", tokens[value_idx]);
                    INIError::InvalidData
                })?;
            }
            "framedelay" => {
                // FrameDelay = <value>
                window.frame_delay = tokens[value_idx]
                    .parse::<i32>()
                    .map_err(|_| INIError::InvalidData)?;
            }
            _ => {
                // Unknown field - skip
            }
        }
    }

    group.add_window(window);
    Ok(())
}

/// Parse a [WindowTransition] block from an INI file
///
/// Matches the C++ INI::parseWindowTransitions function
///
/// Example INI format:
/// ```ini
/// WindowTransition MyTransition
///     FireOnce = Yes
///     Window
///         WinName = MainMenu.wnd:ButtonSinglePlayer
///         Style = FLASH
///         FrameDelay = 0
///     End
///     Window
///         WinName = MainMenu.wnd:ButtonMultiPlayer
///         Style = FLASH
///         FrameDelay = 5
///     End
/// End
/// ```
pub fn parse_window_transition_definition(ini: &mut INI) -> INIResult<()> {
    // Read the group name
    let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

    if name.trim().is_empty() {
        return Err(INIError::InvalidData);
    }

    // Create a new group in the store
    let mut group = TransitionGroup::new(name.clone());

    // Parse the group contents
    loop {
        ini.read_line()?;

        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let tokens = ini.get_line_tokens();
        let Some(first) = tokens.first() else {
            continue;
        };

        // Check for End of WindowTransition block
        if first.eq_ignore_ascii_case("End") {
            break;
        }

        // Check for FireOnce field
        if first.eq_ignore_ascii_case("FireOnce") {
            let value = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
            group.fire_once = parse_bool(&value)?;
        }
        // Check for Window sub-block
        else if first.eq_ignore_ascii_case("Window") {
            parse_window_field(ini, &mut group)?;
        }
    }

    // Store the parsed group
    {
        let mut store = get_window_transition_store_mut();
        store.new_group(name.clone());
        if let Some(stored_group) = store.get_group_mut(&name) {
            stored_group.fire_once = group.fire_once;
            stored_group.windows = group.windows;
        }
    }

    Ok(())
}

/// Block parser function for registration with INI system
pub fn parse_window_transition_block(ini: &mut INI) -> INIResult<()> {
    parse_window_transition_definition(ini)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_style_from_str() {
        assert_eq!(
            TransitionStyle::from_str("FLASH"),
            Some(TransitionStyle::Flash)
        );
        assert_eq!(
            TransitionStyle::from_str("flash"),
            Some(TransitionStyle::Flash)
        );
        assert_eq!(
            TransitionStyle::from_str("BUTTONFLASH"),
            Some(TransitionStyle::ButtonFlash)
        );
        assert_eq!(
            TransitionStyle::from_str("WINFADE"),
            Some(TransitionStyle::WinFade)
        );
        assert_eq!(TransitionStyle::from_str("UNKNOWN"), None);
    }

    #[test]
    fn test_transition_style_to_str() {
        assert_eq!(TransitionStyle::Flash.to_str(), "FLASH");
        assert_eq!(TransitionStyle::ButtonFlash.to_str(), "BUTTONFLASH");
    }

    #[test]
    fn test_transition_window_default() {
        let window = TransitionWindow::default();
        assert!(window.win_name.is_empty());
        assert_eq!(window.frame_delay, 0);
        assert_eq!(window.style, TransitionStyle::Flash);
    }

    #[test]
    fn test_transition_group() {
        let mut group = TransitionGroup::new("TestGroup".to_string());
        assert_eq!(group.name, "TestGroup");
        assert!(!group.fire_once);
        assert!(group.windows.is_empty());

        group.fire_once = true;
        group.add_window(TransitionWindow::new());
        assert!(group.fire_once);
        assert_eq!(group.windows.len(), 1);
    }

    #[test]
    fn test_window_transition_store() {
        let mut store = WindowTransitionStore::new();
        assert!(store.is_empty());

        store.new_group("Group1".to_string());
        assert_eq!(store.len(), 1);

        assert!(store.find_group("Group1").is_some());
        assert!(store.find_group("Group2").is_none());

        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("yes"), Ok(true));
        assert_eq!(parse_bool("YES"), Ok(true));
        assert_eq!(parse_bool("no"), Ok(false));
        assert_eq!(parse_bool("NO"), Ok(false));

        assert_eq!(parse_bool("true"), Err(INIError::InvalidData));
        assert_eq!(parse_bool("1"), Err(INIError::InvalidData));
        assert_eq!(parse_bool("invalid"), Err(INIError::InvalidData));
    }

    #[test]
    fn test_window_transition_block_parses_cpp_fields() {
        get_window_transition_store_mut().clear();

        let mut ini = INI::new();
        ini.with_inline_source(
            "\
WindowTransition TestTransition
    FireOnce = Yes
    Window
        WinName = MainMenu.wnd:ButtonSinglePlayer
        Style = FLASH
        FrameDelay = 7
    End
End
",
            |ini| ini.parse_current_file(),
        )
        .expect("valid C++ WindowTransition block");

        let store = get_window_transition_store();
        let group = store
            .find_group("TestTransition")
            .expect("transition group registered");

        assert!(group.fire_once);
        assert_eq!(group.windows.len(), 1);
        assert_eq!(group.windows[0].win_name, "MainMenu.wnd:ButtonSinglePlayer");
        assert_eq!(group.windows[0].style, TransitionStyle::Flash);
        assert_eq!(group.windows[0].frame_delay, 7);
    }

    #[test]
    fn test_window_transition_rejects_invalid_cpp_bool_and_int() {
        let mut ini = INI::new();
        let invalid_bool = "\
WindowTransition BadBool
    FireOnce = true
End
";
        assert!(ini
            .with_inline_source(invalid_bool, |ini| ini.parse_current_file())
            .is_err());

        let mut ini = INI::new();
        let invalid_frame_delay = "\
WindowTransition BadDelay
    Window
        WinName = MainMenu.wnd:ButtonSinglePlayer
        Style = FLASH
        FrameDelay = soon
    End
End
";
        assert!(ini
            .with_inline_source(invalid_frame_delay, |ini| ini.parse_current_file())
            .is_err());
    }
}
