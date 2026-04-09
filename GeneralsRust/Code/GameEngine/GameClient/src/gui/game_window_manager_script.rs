//! Game Window Manager Script System
//!
//! Central module for the INI-based UI layout and callback script system.
//! Bridges `.wnd` file parsing with callback resolution and script action
//! execution for the C&C Generals menu flow.
//!
//! C++ source: `GameWindowManagerScript.cpp`
//!
//! # Architecture
//!
//! The C++ code uses a `FunctionLexicon` to map callback name strings to
//! raw function pointers at load time. The Rust port stores callback names
//! as strings in the parsed definitions and resolves them at window-creation
//! time through the `ScriptCallbackRegistry`. This preserves behavioral
//! parity while using safe Rust patterns.
//!
//! # Key components
//!
//! - [`ScriptAction`] — enumerated UI script actions (ShowWindow, HideWindow, etc.)
//! - [`ScriptCallbackRegistry`] — maps callback name strings to typed handler closures
//! - [`WindowScriptEngine`] — orchestrates parsing, callback resolution, and action execution
//! - [`WindowDefaults`] — per-file default state (colors, fonts) matching C++ globals

use crate::gui::window_script::{
    parse_window_script, WindowDefinition, WindowLayoutDefinition, WindowScriptError,
};
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Constants matching C++ GameWindowManagerScript.cpp
// ---------------------------------------------------------------------------

pub const WIN_BUFFER_LENGTH: usize = 2048;
pub const WIN_STACK_DEPTH: usize = 10;

/// Window status flag names — same order as `WindowStatus` bit flags in C++.
/// PARITY_NOTE: C++ `WindowStatusNames[]` array, used by `parseBitString()`.
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

/// Window style flag names — same order as `GWS_*` bit flags in C++.
/// PARITY_NOTE: C++ `WindowStyleNames[]` array.
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

// ---------------------------------------------------------------------------
// Bit-flag parsing helpers (matching C++ parseBitFlag / parseBitString)
// ---------------------------------------------------------------------------

/// Parse a single flag string and set the corresponding bit.
/// PARITY_NOTE: mirrors C++ `parseBitFlag()`.
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

/// Parse a `'A+B+C'` style flag string into a bitfield.
/// PARITY_NOTE: mirrors C++ `parseBitString()`.
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

// ---------------------------------------------------------------------------
// Scan helpers (matching C++ scanBool / scanShort / scanInt / scanUnsignedInt)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ScriptAction — UI script actions used in .ini menu flow
// ---------------------------------------------------------------------------

/// UI script actions that can be executed on windows.
///
/// PARITY_NOTE: In C++ these are handled by the `AnimateWindowManager`,
/// `ProcessAnimateWindow`, and individual callback dispatch. This enum
/// consolidates the set of named actions that appear in .ini script blocks
/// for menu flow control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptAction {
    /// Show a named window
    ShowWindow(String),
    /// Hide a named window
    HideWindow(String),
    /// Animate a window using a named animation type
    AnimateWindow {
        window_name: String,
        animation_type: String,
    },
    /// Fade a window over a duration
    FadeWindow {
        window_name: String,
        duration_ms: i32,
    },
    /// Set the text label of a window
    SetWindowText { window_name: String, text: String },
    /// Play a named sound
    PlaySound(String),
    /// Run a `ProcessAnimateWindow` on a named window
    RunProcessAnimateWindow {
        window_name: String,
        process_name: String,
    },
}

impl ScriptAction {
    /// Parse a script action from an INI-style key-value pair.
    ///
    /// Expected formats (matching C++ script block parsing):
    /// ```text
    /// ShowWindow = "WindowName"
    /// HideWindow = "WindowName"
    /// AnimateWindow = "WindowName", "AnimationType"
    /// FadeWindow = "WindowName", duration_ms
    /// SetWindowText = "WindowName", "text"
    /// PlaySound = "soundName"
    /// RunProcessAnimateWindow = "WindowName", "ProcessName"
    /// ```
    pub fn parse_from_ini(key: &str, value: &str) -> Option<ScriptAction> {
        let key = key.trim();
        let value = value.trim().trim_end_matches(';');
        let parts: Vec<&str> = split_csv(value);

        match key.to_ascii_uppercase().as_str() {
            "SHOWWINDOW" => {
                let name = unquote(parts.first().unwrap_or(&"")).to_string();
                Some(ScriptAction::ShowWindow(name))
            }
            "HIDEWINDOW" => {
                let name = unquote(parts.first().unwrap_or(&"")).to_string();
                Some(ScriptAction::HideWindow(name))
            }
            "ANIMATEWINDOW" => {
                let window_name = unquote(parts.first().unwrap_or(&"")).to_string();
                let animation_type = unquote(parts.get(1).unwrap_or(&"")).to_string();
                Some(ScriptAction::AnimateWindow {
                    window_name,
                    animation_type,
                })
            }
            "FADEWINDOW" => {
                let window_name = unquote(parts.first().unwrap_or(&"")).to_string();
                let duration_ms = parts
                    .get(1)
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .unwrap_or(0);
                Some(ScriptAction::FadeWindow {
                    window_name,
                    duration_ms,
                })
            }
            "SETWINDOWTEXT" => {
                let window_name = unquote(parts.first().unwrap_or(&"")).to_string();
                let text = unquote(parts.get(1).unwrap_or(&"")).to_string();
                Some(ScriptAction::SetWindowText { window_name, text })
            }
            "PLAYSOUND" => {
                let name = unquote(parts.first().unwrap_or(&"")).to_string();
                Some(ScriptAction::PlaySound(name))
            }
            "RUNPROCESSANIMATEWINDOW" => {
                let window_name = unquote(parts.first().unwrap_or(&"")).to_string();
                let process_name = unquote(parts.get(1).unwrap_or(&"")).to_string();
                Some(ScriptAction::RunProcessAnimateWindow {
                    window_name,
                    process_name,
                })
            }
            _ => None,
        }
    }
}

/// Split a comma-separated value string, respecting quoted strings.
fn split_csv(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    for (i, ch) in value.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                parts.push(value[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < value.len() {
        parts.push(value[start..].trim());
    }
    parts
}

/// Strip surrounding quotes from a value.
fn unquote(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// ScriptCallbackRegistry — maps callback names to typed handler closures
// ---------------------------------------------------------------------------

/// Callback types for layout-level lifecycle events.
/// PARITY_NOTE: mirrors C++ `WindowLayoutInfo` function pointers resolved
/// from `TheFunctionLexicon->winLayoutInitFunc()` etc.
pub type LayoutInitFn = Box<dyn Fn(&str)>;
pub type LayoutUpdateFn = Box<dyn Fn(&str)>;
pub type LayoutShutdownFn = Box<dyn Fn(&str)>;

/// Callback types for window-level events.
/// PARITY_NOTE: mirrors C++ `GameWinSystemFunc`, `GameWinInputFunc`,
/// `GameWinTooltipFunc`, `GameWinDrawFunc` resolved from `TheFunctionLexicon`.
pub type WinSystemFn = Box<dyn Fn(u32, u32) -> bool>;
pub type WinInputFn = Box<dyn Fn(u32, u32) -> bool>;
pub type WinTooltipFn = Box<dyn Fn(u32) -> bool>;
pub type WinDrawFn = Box<dyn Fn()>;

/// Registry that maps callback name strings to handler closures.
///
/// In C++, `TheFunctionLexicon` stores function pointers indexed by
/// `NameKeyType`. This registry serves the same purpose but uses
/// string keys and boxed closures for safe Rust.
///
/// PARITY_NOTE: maps to C++ `FunctionLexicon` lookups:
///   `gameWinSystemFunc(key)`, `gameWinInputFunc(key)`,
///   `gameWinTooltipFunc(key)`, `gameWinDrawFunc(key)`,
///   `winLayoutInitFunc(key)`, `winLayoutUpdateFunc(key)`,
///   `winLayoutShutdownFunc(key)`
pub struct ScriptCallbackRegistry {
    layout_init: HashMap<String, LayoutInitFn>,
    layout_update: HashMap<String, LayoutUpdateFn>,
    layout_shutdown: HashMap<String, LayoutShutdownFn>,
    win_system: HashMap<String, WinSystemFn>,
    win_input: HashMap<String, WinInputFn>,
    win_tooltip: HashMap<String, WinTooltipFn>,
    win_draw: HashMap<String, WinDrawFn>,
}

impl ScriptCallbackRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            layout_init: HashMap::new(),
            layout_update: HashMap::new(),
            layout_shutdown: HashMap::new(),
            win_system: HashMap::new(),
            win_input: HashMap::new(),
            win_tooltip: HashMap::new(),
            win_draw: HashMap::new(),
        }
    }

    // -- Layout lifecycle callback registration --

    /// Register a layout init callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->winLayoutInitFunc()`.
    pub fn register_layout_init<F: Fn(&str) + 'static>(&mut self, name: &str, callback: F) {
        self.layout_init
            .insert(name.to_string(), Box::new(callback));
    }

    /// Register a layout update callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->winLayoutUpdateFunc()`.
    pub fn register_layout_update<F: Fn(&str) + 'static>(&mut self, name: &str, callback: F) {
        self.layout_update
            .insert(name.to_string(), Box::new(callback));
    }

    /// Register a layout shutdown callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->winLayoutShutdownFunc()`.
    pub fn register_layout_shutdown<F: Fn(&str) + 'static>(&mut self, name: &str, callback: F) {
        self.layout_shutdown
            .insert(name.to_string(), Box::new(callback));
    }

    // -- Window-level callback registration --

    /// Register a window system callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->gameWinSystemFunc()`.
    pub fn register_win_system<F: Fn(u32, u32) -> bool + 'static>(
        &mut self,
        name: &str,
        callback: F,
    ) {
        self.win_system.insert(name.to_string(), Box::new(callback));
    }

    /// Register a window input callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->gameWinInputFunc()`.
    pub fn register_win_input<F: Fn(u32, u32) -> bool + 'static>(
        &mut self,
        name: &str,
        callback: F,
    ) {
        self.win_input.insert(name.to_string(), Box::new(callback));
    }

    /// Register a window tooltip callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->gameWinTooltipFunc()`.
    pub fn register_win_tooltip<F: Fn(u32) -> bool + 'static>(&mut self, name: &str, callback: F) {
        self.win_tooltip
            .insert(name.to_string(), Box::new(callback));
    }

    /// Register a window draw callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->gameWinDrawFunc()`.
    pub fn register_win_draw<F: Fn() + 'static>(&mut self, name: &str, callback: F) {
        self.win_draw.insert(name.to_string(), Box::new(callback));
    }

    // -- Lookup helpers --

    /// Look up a layout init callback by name.
    /// PARITY_NOTE: mirrors C++ `info->init = TheFunctionLexicon->winLayoutInitFunc(key)`.
    pub fn get_layout_init(&self, name: &str) -> Option<&LayoutInitFn> {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        self.layout_init.get(&normalized)
    }

    /// Look up a layout update callback by name.
    pub fn get_layout_update(&self, name: &str) -> Option<&LayoutUpdateFn> {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        self.layout_update.get(&normalized)
    }

    /// Look up a layout shutdown callback by name.
    pub fn get_layout_shutdown(&self, name: &str) -> Option<&LayoutShutdownFn> {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        self.layout_shutdown.get(&normalized)
    }

    /// Look up a window system callback by name.
    /// PARITY_NOTE: mirrors C++ `systemFunc = TheFunctionLexicon->gameWinSystemFunc(key)`.
    pub fn get_win_system(&self, name: &str) -> Option<&WinSystemFn> {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        self.win_system.get(&normalized)
    }

    /// Look up a window input callback by name.
    pub fn get_win_input(&self, name: &str) -> Option<&WinInputFn> {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        self.win_input.get(&normalized)
    }

    /// Look up a window tooltip callback by name.
    pub fn get_win_tooltip(&self, name: &str) -> Option<&WinTooltipFn> {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        self.win_tooltip.get(&normalized)
    }

    /// Look up a window draw callback by name.
    pub fn get_win_draw(&self, name: &str) -> Option<&WinDrawFn> {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        self.win_draw.get(&normalized)
    }
}

impl Default for ScriptCallbackRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalize a callback name: strip brackets/quotes, convert "None"/empty to "".
/// PARITY_NOTE: C++ treats "[None]" and empty as no callback.
fn normalize_callback_name(name: &str) -> String {
    let n = name.trim().trim_start_matches('[').trim_end_matches(']');
    if n.eq_ignore_ascii_case("none") || n.is_empty() {
        String::new()
    } else {
        n.to_string()
    }
}

// ---------------------------------------------------------------------------
// WindowDefaults — per-file default state (matching C++ static globals)
// ---------------------------------------------------------------------------

/// Per-file default state for color and font settings.
///
/// PARITY_NOTE: In C++ these are static globals (`defEnabledColor`,
/// `defDisabledColor`, `defBackgroundColor`, `defHiliteColor`,
/// `defSelectedColor`, `defTextColor`, `defFont`) that get reset at
/// the start of each `winCreateFromScript()` call via `resetWindowDefaults()`.
#[derive(Debug, Clone)]
pub struct WindowDefaults {
    pub enabled_color: u32,
    pub disabled_color: u32,
    pub background_color: u32,
    pub hilite_color: u32,
    pub selected_color: u32,
    pub text_color: u32,
    pub font: Option<super::game_window::GameFont>,
}

impl Default for WindowDefaults {
    fn default() -> Self {
        Self {
            enabled_color: 0,
            disabled_color: 0,
            background_color: 0,
            hilite_color: 0,
            selected_color: 0,
            text_color: 0,
            font: None,
        }
    }
}

impl WindowDefaults {
    /// Create a fresh defaults state.
    /// PARITY_NOTE: mirrors C++ `resetWindowDefaults()`.
    pub fn reset() -> Self {
        Self::default()
    }

    /// Apply a named color field from a parsed value.
    /// PARITY_NOTE: mirrors C++ `parseDefaultColor()` and the
    /// `ENABLEDCOLOR` / `DISABLEDCOLOR` / etc. keywords.
    pub fn set_color(&mut self, field: &str, color: u32) {
        match field.to_ascii_uppercase().as_str() {
            "ENABLEDCOLOR" => self.enabled_color = color,
            "DISABLEDCOLOR" => self.disabled_color = color,
            "BACKGROUNDCOLOR" => self.background_color = color,
            "HILITECOLOR" => self.hilite_color = color,
            "SELECTEDCOLOR" => self.selected_color = color,
            "TEXTCOLOR" => self.text_color = color,
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// ScriptBlock — parsed INI script block
// ---------------------------------------------------------------------------

/// A parsed script block from an INI file.
///
/// PARITY_NOTE: C++ parses these via `parseWindowManagerScript(INI* ini)`
/// from `[ScriptBlock]` sections in .ini files. Each block has a name and
/// a list of actions to execute.
#[derive(Debug, Clone)]
pub struct ScriptBlock {
    /// Name of the script block (used for lookup by the UI flow).
    pub name: String,
    /// Ordered list of actions to execute.
    pub actions: Vec<ScriptAction>,
}

impl ScriptBlock {
    /// Parse a script block from INI key-value pairs.
    ///
    /// The expected INI format is:
    /// ```ini
    /// [ScriptBlock]
    ///   ScriptName = "MyScript"
    ///   ShowWindow = "SomeWindow"
    ///   AnimateWindow = "SomeWindow", "FadeIn"
    /// ```
    pub fn parse_from_ini(entries: &[(String, String)]) -> Option<ScriptBlock> {
        let mut name = String::new();
        let mut actions = Vec::new();

        for (key, value) in entries {
            let key_trimmed = key.trim();
            if key_trimmed.eq_ignore_ascii_case("ScriptName") {
                name = value.trim().trim_matches('"').to_string();
            } else if let Some(action) = ScriptAction::parse_from_ini(key_trimmed, value) {
                actions.push(action);
            }
        }

        if name.is_empty() {
            return None;
        }

        Some(ScriptBlock { name, actions })
    }
}

// ---------------------------------------------------------------------------
// WindowScriptEngine — central orchestrator
// ---------------------------------------------------------------------------

/// Central engine for the window script system.
///
/// Ties together `.wnd` file parsing, callback resolution, and
/// script action execution. This is the primary interface between
/// the parsed window definitions and the window manager.
///
/// PARITY_NOTE: This struct encapsulates the behavior that C++ spreads
/// across `GameWindowManager::winCreateFromScript()`, the static globals
/// in `GameWindowManagerScript.cpp`, and the `FunctionLexicon` lookups.
pub struct WindowScriptEngine {
    /// Callback registry for layout and window callbacks.
    registry: ScriptCallbackRegistry,
    /// Per-file default state, reset for each layout load.
    defaults: WindowDefaults,
    /// Cached script blocks parsed from INI.
    script_blocks: HashMap<String, ScriptBlock>,
}

impl WindowScriptEngine {
    /// Create a new engine with an empty callback registry.
    pub fn new() -> Self {
        Self {
            registry: ScriptCallbackRegistry::new(),
            defaults: WindowDefaults::reset(),
            script_blocks: HashMap::new(),
        }
    }

    /// Get a reference to the callback registry for registration.
    pub fn registry(&self) -> &ScriptCallbackRegistry {
        &self.registry
    }

    /// Get a mutable reference to the callback registry for registration.
    pub fn registry_mut(&mut self) -> &mut ScriptCallbackRegistry {
        &mut self.registry
    }

    /// Get the current per-file defaults.
    pub fn defaults(&self) -> &WindowDefaults {
        &self.defaults
    }

    /// Reset per-file defaults for a new layout load.
    /// PARITY_NOTE: mirrors C++ `resetWindowDefaults()` called at the
    /// start of `winCreateFromScript()`.
    pub fn reset_defaults(&mut self) {
        self.defaults = WindowDefaults::reset();
    }

    // -- Parsing --

    /// Load and parse a `.wnd` file into a layout definition.
    /// PARITY_NOTE: mirrors C++ `GameWindowManager::winCreateFromScript()`
    /// file loading and parsing phase.
    pub fn load_window_layout(
        &self,
        path: &Path,
    ) -> Result<WindowLayoutDefinition, WindowScriptError> {
        parse_window_script(path)
    }

    // -- Callback resolution --

    /// Resolve layout init callback name to a callable.
    /// PARITY_NOTE: mirrors C++ `parseInit()` which does:
    /// `info->init = TheFunctionLexicon->winLayoutInitFunc(key)`.
    pub fn resolve_layout_init(&self, callback_name: &str) -> Option<&LayoutInitFn> {
        self.registry.get_layout_init(callback_name)
    }

    /// Resolve layout update callback name to a callable.
    pub fn resolve_layout_update(&self, callback_name: &str) -> Option<&LayoutUpdateFn> {
        self.registry.get_layout_update(callback_name)
    }

    /// Resolve layout shutdown callback name to a callable.
    pub fn resolve_layout_shutdown(&self, callback_name: &str) -> Option<&LayoutShutdownFn> {
        self.registry.get_layout_shutdown(callback_name)
    }

    /// Resolve a window system callback name.
    /// PARITY_NOTE: mirrors C++ `parseSystemCallback()` which does:
    /// `systemFunc = TheFunctionLexicon->gameWinSystemFunc(key)`.
    pub fn resolve_win_system(&self, callback_name: &str) -> Option<&WinSystemFn> {
        self.registry.get_win_system(callback_name)
    }

    /// Resolve a window input callback name.
    /// PARITY_NOTE: mirrors C++ `parseInputCallback()`.
    pub fn resolve_win_input(&self, callback_name: &str) -> Option<&WinInputFn> {
        self.registry.get_win_input(callback_name)
    }

    /// Resolve a window tooltip callback name.
    /// PARITY_NOTE: mirrors C++ `parseTooltipCallback()`.
    pub fn resolve_win_tooltip(&self, callback_name: &str) -> Option<&WinTooltipFn> {
        self.registry.get_win_tooltip(callback_name)
    }

    /// Resolve a window draw callback name.
    /// PARITY_NOTE: mirrors C++ `parseDrawCallback()`.
    pub fn resolve_win_draw(&self, callback_name: &str) -> Option<&WinDrawFn> {
        self.registry.get_win_draw(callback_name)
    }

    // -- Script block management --

    /// Register a script block parsed from INI.
    pub fn register_script_block(&mut self, block: ScriptBlock) {
        self.script_blocks.insert(block.name.clone(), block);
    }

    /// Look up a script block by name.
    pub fn get_script_block(&self, name: &str) -> Option<&ScriptBlock> {
        self.script_blocks.get(name)
    }

    /// Execute all actions in a named script block.
    ///
    /// PARITY_NOTE: mirrors C++ `executeScript()` which dispatches
    /// script actions. Returns the list of actions for the caller
    /// to execute against the window manager (since actual window
    /// manipulation requires `WindowManager` access).
    pub fn execute_script_block(&self, name: &str) -> &[ScriptAction] {
        match self.script_blocks.get(name) {
            Some(block) => &block.actions,
            None => &[],
        }
    }

    // -- Default application --

    /// Apply per-file defaults to a window definition that lacks explicit values.
    ///
    /// PARITY_NOTE: In C++, defaults are applied via global variables during
    /// `parseWindow()` — `instData.m_enabledText.color = defTextColor` etc.
    /// This method replicates that behavior for the parsed definition layer.
    pub fn apply_defaults_to_definition(&self, window_def: &mut WindowDefinition) {
        // Apply default text color when no explicit text colors were parsed.
        if window_def.enabled_text.color == 0
            && window_def.disabled_text.color == 0
            && window_def.hilite_text.color == 0
        {
            let default = self.defaults.text_color;
            window_def.enabled_text.color = default;
            window_def.enabled_text.border_color = default;
            window_def.disabled_text.color = default;
            window_def.disabled_text.border_color = default;
            window_def.hilite_text.color = default;
            window_def.hilite_text.border_color = default;
        }

        // Apply default font when none specified.
        if window_def.font.is_none() && self.defaults.font.is_some() {
            window_def.font = self.defaults.font.clone();
        }
    }

    /// Apply per-file color default for a named color field.
    /// PARITY_NOTE: mirrors C++ parseDefaultColor called during
    /// the top-level parsing loop of `winCreateFromScript()`.
    pub fn apply_default_color(&mut self, field: &str, value: &str) {
        let cleaned = value.trim().trim_end_matches(';');
        if cleaned.eq_ignore_ascii_case("TRANSPARENT") {
            self.defaults
                .set_color(field, super::game_window::WIN_COLOR_UNDEFINED);
            return;
        }
        let mut components = Vec::new();
        for part in cleaned.split(|c: char| c.is_whitespace() || c == ',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Ok(v) = part.parse::<u8>() {
                components.push(v);
            }
        }
        match components.as_slice() {
            [r, g, b, a] => {
                self.defaults.set_color(field, pack_color([*r, *g, *b, *a]));
            }
            [r, g, b] => {
                self.defaults
                    .set_color(field, pack_color([*r, *g, *b, 255]));
            }
            _ => {}
        }
    }
}

impl Default for WindowScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Pack RGBA components into a 32-bit color value.
/// PARITY_NOTE: mirrors C++ `GameMakeColor(r, g, b, a)`.
fn pack_color(rgba: [u8; 4]) -> u32 {
    ((rgba[3] as u32) << 24) | ((rgba[0] as u32) << 16) | ((rgba[1] as u32) << 8) | (rgba[2] as u32)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bit_string() {
        let mut bits: u32 = 0;
        parse_bit_string("ENABLED+HIDDEN", &mut bits, WINDOW_STATUS_NAMES);
        assert_ne!(bits & (1 << 3), 0); // ENABLED
        assert_ne!(bits & (1 << 4), 0); // HIDDEN
    }

    #[test]
    fn test_parse_bit_string_null() {
        let mut bits: u32 = 0xFFFF;
        parse_bit_string("NULL", &mut bits, WINDOW_STATUS_NAMES);
        assert_eq!(bits, 0xFFFF); // NULL should not change bits
    }

    #[test]
    fn test_scan_helpers() {
        assert_eq!(scan_bool("1"), Some(true));
        assert_eq!(scan_bool("0"), Some(false));
        assert_eq!(scan_int("42"), Some(42));
        assert_eq!(scan_unsigned_int("100"), Some(100u32));
        assert_eq!(scan_short("-10"), Some(-10i16));
    }

    #[test]
    fn test_script_action_parse_show_window() {
        let action = ScriptAction::parse_from_ini("ShowWindow", r#""MyWindow""#).unwrap();
        assert_eq!(action, ScriptAction::ShowWindow("MyWindow".to_string()));
    }

    #[test]
    fn test_script_action_parse_hide_window() {
        let action = ScriptAction::parse_from_ini("HideWindow", r#""MyWindow""#).unwrap();
        assert_eq!(action, ScriptAction::HideWindow("MyWindow".to_string()));
    }

    #[test]
    fn test_script_action_parse_animate_window() {
        let action =
            ScriptAction::parse_from_ini("AnimateWindow", r#""MyWindow", "FadeIn""#).unwrap();
        assert_eq!(
            action,
            ScriptAction::AnimateWindow {
                window_name: "MyWindow".to_string(),
                animation_type: "FadeIn".to_string(),
            }
        );
    }

    #[test]
    fn test_script_action_parse_fade_window() {
        let action = ScriptAction::parse_from_ini("FadeWindow", r#""MyWindow", 500"#).unwrap();
        assert_eq!(
            action,
            ScriptAction::FadeWindow {
                window_name: "MyWindow".to_string(),
                duration_ms: 500,
            }
        );
    }

    #[test]
    fn test_script_action_parse_set_window_text() {
        let action =
            ScriptAction::parse_from_ini("SetWindowText", r#""MyWindow", "Hello""#).unwrap();
        assert_eq!(
            action,
            ScriptAction::SetWindowText {
                window_name: "MyWindow".to_string(),
                text: "Hello".to_string(),
            }
        );
    }

    #[test]
    fn test_script_action_parse_play_sound() {
        let action = ScriptAction::parse_from_ini("PlaySound", r#""ButtonSound""#).unwrap();
        assert_eq!(action, ScriptAction::PlaySound("ButtonSound".to_string()));
    }

    #[test]
    fn test_script_action_parse_run_process_animate_window() {
        let action =
            ScriptAction::parse_from_ini("RunProcessAnimateWindow", r#""MyWindow", "SlideIn""#)
                .unwrap();
        assert_eq!(
            action,
            ScriptAction::RunProcessAnimateWindow {
                window_name: "MyWindow".to_string(),
                process_name: "SlideIn".to_string(),
            }
        );
    }

    #[test]
    fn test_script_action_parse_unknown() {
        assert!(ScriptAction::parse_from_ini("UnknownAction", "value").is_none());
    }

    #[test]
    fn test_callback_registry() {
        let mut registry = ScriptCallbackRegistry::new();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();
        registry.register_layout_init("TestInit", move |_name| {
            called_clone.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        let cb = registry.get_layout_init("TestInit").unwrap();
        cb("test");
        assert!(called.load(std::sync::atomic::Ordering::Relaxed));

        // "None" returns None
        assert!(registry.get_layout_init("None").is_none());
        assert!(registry.get_layout_init("[None]").is_none());
    }

    #[test]
    fn test_callback_registry_win_system() {
        let mut registry = ScriptCallbackRegistry::new();
        registry.register_win_system("MainMenuSystem", |msg, data| msg == 1 && data == 0);
        let cb = registry.get_win_system("MainMenuSystem").unwrap();
        assert!(cb(1, 0));
        assert!(!cb(0, 0));
    }

    #[test]
    fn test_window_defaults_reset() {
        let mut defaults = WindowDefaults::default();
        defaults.text_color = 0xFF0000;
        let reset = WindowDefaults::reset();
        assert_eq!(reset.text_color, 0);
    }

    #[test]
    fn test_window_defaults_set_color() {
        let mut defaults = WindowDefaults::default();
        defaults.set_color("ENABLEDCOLOR", 0x11223344);
        assert_eq!(defaults.enabled_color, 0x11223344);
        defaults.set_color("TEXTCOLOR", 0xAABBCCDD);
        assert_eq!(defaults.text_color, 0xAABBCCDD);
    }

    #[test]
    fn test_script_block_parse() {
        let entries = vec![
            ("ScriptName".to_string(), "\"TestBlock\"".to_string()),
            ("ShowWindow".to_string(), "\"Win1\"".to_string()),
            ("HideWindow".to_string(), "\"Win2\"".to_string()),
        ];
        let block = ScriptBlock::parse_from_ini(&entries).unwrap();
        assert_eq!(block.name, "TestBlock");
        assert_eq!(block.actions.len(), 2);
        assert_eq!(
            block.actions[0],
            ScriptAction::ShowWindow("Win1".to_string())
        );
        assert_eq!(
            block.actions[1],
            ScriptAction::HideWindow("Win2".to_string())
        );
    }

    #[test]
    fn test_script_block_empty_name_returns_none() {
        let entries: Vec<(String, String)> =
            vec![("ShowWindow".to_string(), "\"Win1\"".to_string())];
        assert!(ScriptBlock::parse_from_ini(&entries).is_none());
    }

    #[test]
    fn test_script_engine_execute_script_block() {
        let mut engine = WindowScriptEngine::new();
        let block = ScriptBlock {
            name: "TestScript".to_string(),
            actions: vec![
                ScriptAction::ShowWindow("Win1".to_string()),
                ScriptAction::PlaySound("Click".to_string()),
            ],
        };
        engine.register_script_block(block);

        let actions = engine.execute_script_block("TestScript");
        assert_eq!(actions.len(), 2);

        let missing = engine.execute_script_block("NoSuchScript");
        assert!(missing.is_empty());
    }

    #[test]
    fn test_script_engine_apply_defaults() {
        let mut engine = WindowScriptEngine::new();
        engine.defaults.text_color = 0xAABBCCDD;
        engine.defaults.font = Some(super::super::game_window::GameFont {
            name: "Arial".to_string(),
            size: 12,
            bold: false,
        });

        let mut def = WindowDefinition::default();
        engine.apply_defaults_to_definition(&mut def);

        assert_eq!(def.enabled_text.color, 0xAABBCCDD);
        assert_eq!(def.disabled_text.color, 0xAABBCCDD);
        assert_eq!(def.hilite_text.color, 0xAABBCCDD);
        assert!(def.font.is_some());
        assert_eq!(def.font.as_ref().unwrap().name, "Arial");
    }

    #[test]
    fn test_script_engine_apply_defaults_respects_explicit() {
        let mut engine = WindowScriptEngine::new();
        engine.defaults.text_color = 0x11111111;

        let mut def = WindowDefinition::default();
        def.enabled_text.color = 0xFF000000; // explicit
        engine.apply_defaults_to_definition(&mut def);

        // Should not overwrite explicit color
        assert_eq!(def.enabled_text.color, 0xFF000000);
        // But disabled/hilite were 0 so they get defaults
        assert_eq!(def.disabled_text.color, 0x11111111);
    }

    #[test]
    fn test_engine_apply_default_color() {
        let mut engine = WindowScriptEngine::new();
        engine.apply_default_color("TEXTCOLOR", "255 128 64 255");
        assert_ne!(engine.defaults.text_color, 0);
        // Reconstruct: R=255 G=128 B=64 A=255
        let expected = pack_color([255, 128, 64, 255]);
        assert_eq!(engine.defaults.text_color, expected);
    }

    #[test]
    fn test_engine_apply_default_color_transparent() {
        let mut engine = WindowScriptEngine::new();
        engine.apply_default_color("TEXTCOLOR", "TRANSPARENT");
        assert_eq!(
            engine.defaults.text_color,
            super::super::game_window::WIN_COLOR_UNDEFINED
        );
    }

    #[test]
    fn test_split_csv() {
        let parts = split_csv(r#""Win1", "AnimType""#);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], r#""Win1""#);
        assert_eq!(parts[1], r#""AnimType""#);
    }

    #[test]
    fn test_unquote() {
        assert_eq!(unquote(r#""hello""#), "hello");
        assert_eq!(unquote("hello"), "hello");
        assert_eq!(unquote(r#"""#), "");
    }
}
