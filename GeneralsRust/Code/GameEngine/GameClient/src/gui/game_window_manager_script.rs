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

use crate::gui::callbacks as cb;
use crate::gui::callbacks::menu_callbacks::MenuCallbacks;
use crate::gui::game_window::{
    GameWindow, WindowCallbacks as GwCallbacks, WindowInstanceData, WindowMessage, WindowMsgData,
    WindowMsgHandled, WindowWidget,
};

use crate::gui::window_manager::WindowLayout;
use crate::gui::window_script::{WindowLayoutDefinition, WindowScriptError, parse_window_script};
use std::any::Any;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

// WOL/GameSpy callback bodies are intentionally feature-gated while the
// non-network game remains the supported runtime.  Keep the callback names in
// the FunctionLexicon in both configurations, but bind the real C++-parity
// ports whenever `online_ui` is selected.  This prevents the registry from
// silently replacing an enabled online callback with the offline no-op table.
macro_rules! register_online_system {
    ($registry:expr, $name:literal, $callback:path) => {{
        #[cfg(feature = "online_ui")]
        $registry.register_win_system($name, $callback);
        #[cfg(not(feature = "online_ui"))]
        $registry.register_win_system(
            $name,
            |_window: &mut GameWindow,
             _message: WindowMessage,
             _data1: WindowMsgData,
             _data2: WindowMsgData| WindowMsgHandled::Ignored,
        );
    }};
}

macro_rules! register_online_input {
    ($registry:expr, $name:literal, $callback:path) => {{
        #[cfg(feature = "online_ui")]
        $registry.register_win_input($name, $callback);
        #[cfg(not(feature = "online_ui"))]
        $registry.register_win_input(
            $name,
            |_window: &mut GameWindow,
             _message: WindowMessage,
             _data1: WindowMsgData,
             _data2: WindowMsgData| WindowMsgHandled::Ignored,
        );
    }};
}

macro_rules! register_online_layout_init {
    ($registry:expr, $name:literal, $callback:path) => {{
        #[cfg(feature = "online_ui")]
        $registry.register_layout_init($name, |layout, effects| $callback(layout, None));
        #[cfg(not(feature = "online_ui"))]
        $registry.register_layout_init($name, |_layout, effects| {});
    }};
}

macro_rules! register_online_layout_update {
    ($registry:expr, $name:literal, $callback:path) => {{
        #[cfg(feature = "online_ui")]
        $registry.register_layout_update($name, |layout, effects| $callback(layout, None));
        #[cfg(not(feature = "online_ui"))]
        $registry.register_layout_update($name, |_layout, effects| {});
    }};
}

macro_rules! register_online_layout_shutdown {
    ($registry:expr, $name:literal, $callback:path) => {{
        #[cfg(feature = "online_ui")]
        $registry.register_layout_shutdown_without_data($name, |layout, effects| {
            $callback(layout, None)
        });
        #[cfg(not(feature = "online_ui"))]
        $registry.register_layout_shutdown_without_data($name, |_layout, effects| {});
    }};
}

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
    if s == "\"" {
        return "";
    }
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
pub type LayoutInitFn =
    Box<dyn for<'a> Fn(&WindowLayout, &mut crate::gui::game_window::WindowEventEffects<'a>)>;
pub type LayoutUpdateFn =
    Box<dyn for<'a> Fn(&WindowLayout, &mut crate::gui::game_window::WindowEventEffects<'a>)>;
pub type LayoutShutdownFn = Box<
    dyn for<'a> Fn(
        &WindowLayout,
        Option<&dyn Any>,
        &mut crate::gui::game_window::WindowEventEffects<'a>,
    ),
>;

/// Callback types for window-level events.
/// PARITY_NOTE: mirrors C++ `GameWinSystemFunc`, `GameWinInputFunc`,
/// `GameWinTooltipFunc`, `GameWinDrawFunc` resolved from `TheFunctionLexicon`.
///
/// Stored as `Arc` so [`ScriptCallbackRegistry::create_system_callback`] and
/// siblings can clone the handler into a `'static` box. That matches C++
/// function-pointer lexicon semantics: a resolved pointer stays valid if the
/// table entry is later replaced or dropped.
type LegacyWinCallback = Arc<
    dyn Fn(&mut GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled
        + Send
        + Sync,
>;
pub type WinSystemFn = Arc<
    dyn for<'a> Fn(
            &mut GameWindow,
            WindowMessage,
            WindowMsgData,
            WindowMsgData,
            &mut crate::gui::game_window::WindowEventEffects<'a>,
        ) -> WindowMsgHandled
        + Send
        + Sync,
>;
pub type WinInputFn = Arc<
    dyn for<'a> Fn(
            &mut GameWindow,
            WindowMessage,
            WindowMsgData,
            WindowMsgData,
            &mut crate::gui::game_window::WindowEventEffects<'a>,
        ) -> WindowMsgHandled
        + Send
        + Sync,
>;
pub type WinTooltipFn = Arc<dyn Fn(&GameWindow, u32) + Send + Sync>;
pub type WinDrawFn = Arc<
    dyn Fn(
            &GameWindow,
            &WindowInstanceData,
            &mut crate::gui::window_draw_context::WindowDrawContext<'_>,
        ) + Send
        + Sync,
>;

/// Registry that maps callback name strings to handler closures.
///
/// In C++, `TheFunctionLexicon` stores function pointers indexed by
/// `NameKeyType`. This registry serves the same purpose but uses
/// string keys and `Arc`-owned closures for safe Rust.
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
    win_system_legacy: HashMap<String, LegacyWinCallback>,
    win_input: HashMap<String, WinInputFn>,
    win_input_legacy: HashMap<String, LegacyWinCallback>,
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
            win_system_legacy: HashMap::new(),
            win_input: HashMap::new(),
            win_input_legacy: HashMap::new(),
            win_tooltip: HashMap::new(),
            win_draw: HashMap::new(),
        }
    }

    // -- Layout lifecycle callback registration --

    /// Register a layout init callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->winLayoutInitFunc()`.
    pub fn register_layout_init<
        F: for<'a> Fn(&WindowLayout, &mut crate::gui::game_window::WindowEventEffects<'a>) + 'static,
    >(
        &mut self,
        name: &str,
        callback: F,
    ) {
        self.layout_init.insert(
            name.to_string(),
            Box::new(move |layout, effects| callback(layout, effects)),
        );
    }

    /// Register a layout update callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->winLayoutUpdateFunc()`.
    pub fn register_layout_update<
        F: for<'a> Fn(&WindowLayout, &mut crate::gui::game_window::WindowEventEffects<'a>) + 'static,
    >(
        &mut self,
        name: &str,
        callback: F,
    ) {
        self.layout_update.insert(
            name.to_string(),
            Box::new(move |layout, effects| callback(layout, effects)),
        );
    }

    /// Register a layout shutdown callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->winLayoutShutdownFunc()`.
    pub fn register_layout_shutdown<
        F: for<'a> Fn(
                &WindowLayout,
                Option<&dyn Any>,
                &mut crate::gui::game_window::WindowEventEffects<'a>,
            ) + 'static,
    >(
        &mut self,
        name: &str,
        callback: F,
    ) {
        self.layout_shutdown.insert(
            name.to_string(),
            Box::new(move |layout, data, effects| callback(layout, data, effects)),
        );
    }

    pub fn register_layout_shutdown_without_data<
        F: for<'a> Fn(&WindowLayout, &mut crate::gui::game_window::WindowEventEffects<'a>) + 'static,
    >(
        &mut self,
        name: &str,
        callback: F,
    ) {
        self.register_layout_shutdown(name, move |layout, _data, effects| {
            callback(layout, effects)
        });
    }

    // -- Window-level callback registration --

    /// Register a window system callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->gameWinSystemFunc()`.
    pub fn register_win_system<
        F: Fn(&mut GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled
            + Send
            + Sync
            + 'static,
    >(
        &mut self,
        name: &str,
        callback: F,
    ) {
        let callback: LegacyWinCallback = Arc::new(callback);
        self.win_system_legacy
            .insert(name.to_string(), Arc::clone(&callback));
        self.win_system.insert(
            name.to_string(),
            Arc::new(move |win, msg, d1, d2, _effects| callback(win, msg, d1, d2)),
        );
    }

    /// Register a window input callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->gameWinInputFunc()`.
    pub fn register_win_input<
        F: Fn(&mut GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled
            + Send
            + Sync
            + 'static,
    >(
        &mut self,
        name: &str,
        callback: F,
    ) {
        let callback: LegacyWinCallback = Arc::new(callback);
        self.win_input_legacy
            .insert(name.to_string(), Arc::clone(&callback));
        self.win_input.insert(
            name.to_string(),
            Arc::new(move |win, msg, d1, d2, _effects| callback(win, msg, d1, d2)),
        );
    }

    pub fn register_win_system_with_effects<F>(&mut self, name: &str, callback: F)
    where
        F: for<'a> Fn(
                &mut GameWindow,
                WindowMessage,
                WindowMsgData,
                WindowMsgData,
                &mut crate::gui::game_window::WindowEventEffects<'a>,
            ) -> WindowMsgHandled
            + Send
            + Sync
            + 'static,
    {
        self.win_system_legacy.remove(name);
        self.win_system.insert(name.to_string(), Arc::new(callback));
    }

    pub fn register_win_input_with_effects<F>(&mut self, name: &str, callback: F)
    where
        F: for<'a> Fn(
                &mut GameWindow,
                WindowMessage,
                WindowMsgData,
                WindowMsgData,
                &mut crate::gui::game_window::WindowEventEffects<'a>,
            ) -> WindowMsgHandled
            + Send
            + Sync
            + 'static,
    {
        self.win_input_legacy.remove(name);
        self.win_input.insert(name.to_string(), Arc::new(callback));
    }

    /// Register a window tooltip callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->gameWinTooltipFunc()`.
    pub fn register_win_tooltip<F: Fn(&GameWindow, u32) + Send + Sync + 'static>(
        &mut self,
        name: &str,
        callback: F,
    ) {
        self.win_tooltip
            .insert(name.to_string(), Arc::new(callback));
    }

    /// Register a window draw callback by name.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->gameWinDrawFunc()`.
    pub fn register_win_draw<
        F: Fn(
                &GameWindow,
                &WindowInstanceData,
                &mut crate::gui::window_draw_context::WindowDrawContext<'_>,
            ) + Send
            + Sync
            + 'static,
    >(
        &mut self,
        name: &str,
        callback: F,
    ) {
        self.win_draw.insert(name.to_string(), Arc::new(callback));
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

impl ScriptCallbackRegistry {
    /// Populate the registry with all default GUI callback name-to-function
    /// mappings, matching the C++ `FunctionLexicon::init()` tables exactly.
    ///
    /// PARITY_NOTE: mirrors C++ `FunctionLexicon.cpp` table loading in `init()`:
    ///   - `gameWinDrawTable`      → `register_win_draw`
    ///   - `gameWinSystemTable`    → `register_win_system`
    ///   - `gameWinInputTable`     → `register_win_input`
    ///   - `gameWinTooltipTable`   → `register_win_tooltip`
    ///   - `winLayoutInitTable`    → `register_layout_init`
    ///   - `winLayoutUpdateTable`  → `register_layout_update`
    ///   - `winLayoutShutdownTable`→ `register_layout_shutdown`
    ///
    /// Callback names are case-sensitive and must match the strings used in
    /// `.wnd` files and the C++ `FunctionLexicon` tables exactly.
    ///
    /// The closures registered here use simplified signatures that match the
    /// registry's typed callback types.  Where a concrete Rust handler exists
    /// (e.g. `challenge_menu_system`), the closure forwards to it; otherwise
    /// a stub is registered only for callbacks that are intentionally deferred
    /// with GameNetwork or known legacy no-op/default handlers.
    pub fn populate_defaults(&mut self) {
        self.populate_win_draw_table();
        self.populate_win_system_table();
        self.populate_win_input_table();
        self.populate_win_tooltip_table();
        self.populate_layout_init_table();
        self.populate_layout_update_table();
        self.populate_layout_shutdown_table();
    }

    fn populate_win_draw_table(&mut self) {
        self.register_win_draw("IMECandidateMainDraw", cb::ime_candidate_main_draw);
        self.register_win_draw("IMECandidateTextAreaDraw", cb::ime_candidate_text_area_draw);
    }

    fn populate_win_system_table(&mut self) {
        self.register_win_system(
            "PassSelectedButtonsToParentSystem",
            |_win, _msg, _d1, _d2| WindowMsgHandled::Ignored,
        );
        self.register_win_system("PassMessagesToParentSystem", |_win, _msg, _d1, _d2| {
            WindowMsgHandled::Ignored
        });
        self.register_win_system("GameWinDefaultSystem", |_win, _msg, _d1, _d2| {
            WindowMsgHandled::Ignored
        });

        self.register_win_system_with_effects(
            "GadgetPushButtonSystem",
            |window, msg, d1, d2, effects| gadget_push_button_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "GadgetCheckBoxSystem",
            |window, msg, d1, d2, effects| gadget_check_box_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "GadgetRadioButtonSystem",
            |window, msg, d1, d2, effects| gadget_radio_button_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "GadgetTabControlSystem",
            |window, msg, d1, d2, effects| gadget_tab_control_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "GadgetListBoxSystem",
            |window, msg, d1, d2, effects| gadget_list_box_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "GadgetComboBoxSystem",
            |window, msg, d1, d2, effects| gadget_combo_box_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "GadgetHorizontalSliderSystem",
            |window, msg, d1, d2, effects| {
                gadget_horizontal_slider_system(window, msg, d1, d2, effects)
            },
        );
        self.register_win_system_with_effects(
            "GadgetVerticalSliderSystem",
            |window, msg, d1, d2, effects| {
                gadget_vertical_slider_system(window, msg, d1, d2, effects)
            },
        );
        self.register_win_system_with_effects(
            "GadgetProgressBarSystem",
            |window, msg, d1, d2, effects| gadget_progress_bar_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "GadgetStaticTextSystem",
            |window, msg, d1, d2, effects| gadget_static_text_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "GadgetTextEntrySystem",
            |window, msg, d1, d2, effects| gadget_text_entry_system(window, msg, d1, d2, effects),
        );

        self.register_win_system_with_effects(
            "MessageBoxSystem",
            |window, msg, d1, d2, effects| message_box_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "QuitMessageBoxSystem",
            |window, msg, d1, d2, effects| quit_message_box_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "ExtendedMessageBoxSystem",
            |window, msg, d1, d2, effects| {
                extended_message_box_system(window, msg, d1, d2, effects)
            },
        );

        self.register_win_system_with_effects("MOTDSystem", |window, msg, d1, d2, effects| {
            effects.with_manager(|manager, effects| cb::motd_system(window, msg, d1, d2, manager))
        });

        self.register_win_system_with_effects("MainMenuSystem", |window, msg, d1, d2, effects| {
            main_menu_system(window, msg, d1, d2, effects)
        });
        self.register_win_system_with_effects(
            "OptionsMenuSystem",
            |window, msg, d1, d2, effects| options_menu_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "SinglePlayerMenuSystem",
            |window, msg, d1, d2, effects| single_player_menu_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects("QuitMenuSystem", |window, msg, d1, d2, effects| {
            cb::quit_menu_system(window, msg, d1, d2, effects)
        });
        self.register_win_system_with_effects(
            "MapSelectMenuSystem",
            |window, msg, d1, d2, effects| map_select_menu_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "ReplayMenuSystem",
            |window, msg, d1, d2, effects| cb::replay_menu_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "CreditsMenuSystem",
            |window, msg, d1, d2, effects| credits_menu_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "LanLobbyMenuSystem",
            |window, msg, d1, d2, effects| lan_lobby_menu_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "LanGameOptionsMenuSystem",
            |window, msg, d1, d2, effects| {
                cb::lan_game_options_menu_system(window, msg, d1, d2, effects)
            },
        );
        self.register_win_system_with_effects(
            "LanMapSelectMenuSystem",
            |window, msg, d1, d2, effects| {
                effects.with_manager(|manager, effects| {
                    cb::lan_map_select_menu_system(window, msg, d1, d2, manager, effects)
                })
            },
        );
        self.register_win_system_with_effects(
            "SkirmishGameOptionsMenuSystem",
            |window, msg, d1, d2, effects| {
                cb::skirmish_game_options_menu_system(window, msg, d1, d2, effects)
            },
        );
        self.register_win_system_with_effects(
            "SkirmishMapSelectMenuSystem",
            |window, msg, d1, d2, effects| {
                effects.with_manager(|manager, effects| {
                    cb::skirmish_map_select_menu_system(window, msg, d1, d2, manager, effects)
                })
            },
        );
        self.register_win_system_with_effects(
            "ChallengeMenuSystem",
            |window, msg, d1, d2, effects| cb::challenge_menu_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "SaveLoadMenuSystem",
            |window, msg, d1, d2, effects| cb::save_load_menu_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "PopupCommunicatorSystem",
            |window, msg, d1, d2, effects| {
                effects.with_manager(|manager, effects| {
                    cb::popup_communicator_system(window, msg, d1, d2, manager, effects)
                })
            },
        );
        self.register_win_system("PopupBuddyNotificationSystem", |_win, _msg, _d1, _d2| {
            WindowMsgHandled::Ignored
        });
        self.register_win_system_with_effects(
            "PopupReplaySystem",
            |window, msg, d1, d2, effects| {
                effects.with_manager(|manager, effects| {
                    cb::popup_replay_system(window, msg, d1, d2, manager, effects)
                })
            },
        );
        self.register_win_system_with_effects(
            "KeyboardOptionsMenuSystem",
            |window, msg, d1, d2, effects| {
                cb::keyboard_options_menu_system(window, msg, d1, d2, effects)
            },
        );

        // WOL/GameSpy callback bindings.  Offline builds use the same
        // fail-closed stubs as the original deferred network path; online_ui
        // selects the concrete callback ports.
        register_online_system!(self, "WOLLadderScreenSystem", cb::wol_ladder_screen_system);
        register_online_system!(self, "WOLLoginMenuSystem", cb::wol_login_menu_system);
        register_online_system!(self, "WOLLocaleSelectSystem", cb::wol_locale_select_system);
        register_online_system!(self, "WOLLobbyMenuSystem", cb::wol_lobby_menu_system);
        register_online_system!(
            self,
            "WOLGameSetupMenuSystem",
            cb::wol_game_setup_menu_system
        );
        register_online_system!(
            self,
            "WOLMapSelectMenuSystem",
            cb::wol_map_select_menu_system
        );
        register_online_system!(self, "WOLBuddyOverlaySystem", cb::wol_buddy_overlay_system);
        register_online_system!(
            self,
            "WOLBuddyOverlayRCMenuSystem",
            cb::wol_buddy_overlay_rc_menu_system
        );
        register_online_system!(
            self,
            "RCGameDetailsMenuSystem",
            cb::rc_game_details_menu_system
        );
        register_online_system!(
            self,
            "GameSpyPlayerInfoOverlaySystem",
            cb::popup_player_info_system
        );
        register_online_system!(
            self,
            "WOLMessageWindowSystem",
            cb::wol_message_window_system
        );
        register_online_system!(
            self,
            "WOLQuickMatchMenuSystem",
            cb::wol_quick_match_menu_system
        );
        register_online_system!(self, "WOLWelcomeMenuSystem", cb::wol_welcome_menu_system);
        register_online_system!(self, "WOLStatusMenuSystem", cb::wol_status_menu_system);
        register_online_system!(
            self,
            "WOLQMScoreScreenSystem",
            cb::wol_qm_score_screen_system
        );
        register_online_system!(
            self,
            "WOLCustomScoreScreenSystem",
            cb::wol_custom_score_screen_system
        );
        register_online_system!(
            self,
            "NetworkDirectConnectSystem",
            cb::network_direct_connect_system
        );
        register_online_system!(self, "PopupHostGameSystem", cb::popup_host_game_system);
        register_online_system!(self, "PopupJoinGameSystem", cb::popup_join_game_system);
        register_online_system!(
            self,
            "PopupLadderSelectSystem",
            cb::popup_ladder_select_system
        );

        self.register_win_system_with_effects(
            "InGamePopupMessageSystem",
            |window, msg, d1, d2, effects| {
                cb::in_game_popup_message_system_with_sink(window, msg, d1, d2, effects)
            },
        );

        self.register_win_system_with_effects(
            "ControlBarSystem",
            |window, msg, d1, d2, effects| control_bar_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "ControlBarObserverSystem",
            |window, msg, d1, d2, effects| {
                control_bar_observer_system(window, msg, d1, d2, effects)
            },
        );
        self.register_win_system_with_effects(
            "IMECandidateWindowSystem",
            |window, msg, d1, d2, effects| cb::ime_candidate_window_system(window, msg, d1, d2),
        );
        self.register_win_system_with_effects(
            "ReplayControlSystem",
            |window, msg, d1, d2, effects| cb::replay_control_system(window, msg, d1, d2),
        );

        self.register_win_system_with_effects(
            "InGameChatSystem",
            |window, msg, d1, d2, effects| in_game_chat_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "DisconnectControlSystem",
            |window, msg, d1, d2, effects| disconnect_control_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects("DiplomacySystem", |window, msg, d1, d2, effects| {
            diplomacy_system(window, msg, d1, d2, effects)
        });
        self.register_win_system_with_effects(
            "GeneralsExpPointsSystem",
            |window, msg, d1, d2, effects| cb::generals_exp_points_system(window, msg, d1, d2),
        );
        self.register_win_system_with_effects(
            "DifficultySelectSystem",
            |window, msg, d1, d2, effects| {
                cb::difficulty_select_system(window, msg, d1, d2, effects)
            },
        );
        self.register_win_system_with_effects(
            "IdleWorkerSystem",
            |window, msg, d1, d2, effects| idle_worker_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "EstablishConnectionsControlSystem",
            |window, msg, d1, d2, effects| {
                establish_connections_control_system(window, msg, d1, d2, effects)
            },
        );
        self.register_win_system_with_effects(
            "GameInfoWindowSystem",
            |window, msg, d1, d2, effects| cb::game_info_window_system(window, msg, d1, d2),
        );
        self.register_win_system_with_effects(
            "ScoreScreenSystem",
            |window, msg, d1, d2, effects| cb::score_screen_system(window, msg, d1, d2, effects),
        );
        self.register_win_system_with_effects(
            "DownloadMenuSystem",
            |window, msg, d1, d2, effects| cb::download_menu_system(window, msg, d1, d2, effects),
        );
    }

    fn populate_win_input_table(&mut self) {
        self.register_win_input("GameWinDefaultInput", |_win, _msg, _d1, _d2| {
            WindowMsgHandled::Ignored
        });
        self.register_win_input("GameWinBlockInput", |_win, _msg, _d1, _d2| {
            WindowMsgHandled::Handled
        });

        self.register_win_input_with_effects(
            "GadgetPushButtonInput",
            |window, msg, d1, d2, effects| gadget_push_button_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects(
            "GadgetCheckBoxInput",
            |window, msg, d1, d2, effects| gadget_check_box_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects(
            "GadgetRadioButtonInput",
            |window, msg, d1, d2, effects| gadget_radio_button_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects(
            "GadgetTabControlInput",
            |window, msg, d1, d2, effects| gadget_tab_control_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects(
            "GadgetListBoxInput",
            |window, msg, d1, d2, effects| gadget_list_box_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects(
            "GadgetListBoxMultiInput",
            |window, msg, d1, d2, effects| {
                gadget_list_box_multi_input(window, msg, d1, d2, effects)
            },
        );
        self.register_win_input_with_effects(
            "GadgetComboBoxInput",
            |window, msg, d1, d2, effects| gadget_combo_box_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects(
            "GadgetHorizontalSliderInput",
            |window, msg, d1, d2, effects| {
                gadget_horizontal_slider_input(window, msg, d1, d2, effects)
            },
        );
        self.register_win_input_with_effects(
            "GadgetVerticalSliderInput",
            |window, msg, d1, d2, effects| {
                gadget_vertical_slider_input(window, msg, d1, d2, effects)
            },
        );
        self.register_win_input_with_effects(
            "GadgetStaticTextInput",
            |window, msg, d1, d2, effects| gadget_static_text_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects(
            "GadgetTextEntryInput",
            |window, msg, d1, d2, effects| gadget_text_entry_input(window, msg, d1, d2, effects),
        );

        self.register_win_input_with_effects("MainMenuInput", |window, msg, d1, d2, effects| {
            main_menu_input(window, msg, d1, d2, effects)
        });
        self.register_win_input_with_effects(
            "MapSelectMenuInput",
            |window, msg, d1, d2, effects| map_select_menu_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects("OptionsMenuInput", |window, msg, d1, d2, effects| {
            options_menu_input(window, msg, d1, d2, effects)
        });
        self.register_win_input_with_effects(
            "SinglePlayerMenuInput",
            |window, msg, d1, d2, effects| single_player_menu_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects(
            "LanLobbyMenuInput",
            |window, msg, d1, d2, effects| lan_lobby_menu_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects("ReplayMenuInput", |window, msg, d1, d2, effects| {
            cb::replay_menu_input(window, msg, d1, d2, effects)
        });
        self.register_win_input_with_effects("CreditsMenuInput", |window, msg, d1, d2, effects| {
            credits_menu_input(window, msg, d1, d2, effects)
        });
        self.register_win_input_with_effects(
            "KeyboardOptionsMenuInput",
            |window, msg, d1, d2, effects| {
                cb::keyboard_options_menu_input(window, msg, d1, d2, effects)
            },
        );
        self.register_win_input_with_effects(
            "PopupCommunicatorInput",
            |window, msg, d1, d2, effects| {
                effects.with_manager(|manager, effects| {
                    cb::popup_communicator_input(window, msg, d1, d2, manager, effects)
                })
            },
        );
        self.register_win_input_with_effects(
            "LanGameOptionsMenuInput",
            |window, msg, d1, d2, effects| {
                cb::lan_game_options_menu_input(window, msg, d1, d2, effects)
            },
        );
        self.register_win_input_with_effects(
            "LanMapSelectMenuInput",
            |window, msg, d1, d2, effects| {
                cb::lan_map_select_menu_input(window, msg, d1, d2, effects)
            },
        );
        self.register_win_input_with_effects(
            "SkirmishGameOptionsMenuInput",
            |window, msg, d1, d2, effects| {
                cb::skirmish_game_options_menu_input(window, msg, d1, d2, effects)
            },
        );
        self.register_win_input_with_effects(
            "SkirmishMapSelectMenuInput",
            |window, msg, d1, d2, effects| {
                cb::skirmish_map_select_menu_input(window, msg, d1, d2, effects)
            },
        );
        self.register_win_input_with_effects(
            "ChallengeMenuInput",
            |window, msg, d1, d2, effects| cb::challenge_menu_input(window, msg, d1, d2, effects),
        );

        // WOL/GameSpy callback bindings.  Offline builds use the same
        // fail-closed stubs as the original deferred network path.
        register_online_input!(self, "WOLLadderScreenInput", cb::wol_ladder_screen_input);
        register_online_input!(self, "WOLLoginMenuInput", cb::wol_login_menu_input);
        register_online_input!(self, "WOLLocaleSelectInput", cb::wol_locale_select_input);
        register_online_input!(self, "WOLLobbyMenuInput", cb::wol_lobby_menu_input);
        register_online_input!(self, "WOLGameSetupMenuInput", cb::wol_game_setup_menu_input);
        register_online_input!(self, "WOLMapSelectMenuInput", cb::wol_map_select_menu_input);
        register_online_input!(self, "WOLBuddyOverlayInput", cb::wol_buddy_overlay_input);
        register_online_input!(
            self,
            "GameSpyPlayerInfoOverlayInput",
            cb::popup_player_info_input
        );
        register_online_input!(self, "WOLMessageWindowInput", cb::wol_message_window_input);
        register_online_input!(
            self,
            "WOLQuickMatchMenuInput",
            cb::wol_quick_match_menu_input
        );
        register_online_input!(self, "WOLWelcomeMenuInput", cb::wol_welcome_menu_input);
        register_online_input!(self, "WOLStatusMenuInput", cb::wol_status_menu_input);
        register_online_input!(self, "WOLQMScoreScreenInput", cb::wol_qm_score_screen_input);
        register_online_input!(
            self,
            "WOLCustomScoreScreenInput",
            cb::wol_custom_score_screen_input
        );
        register_online_input!(
            self,
            "NetworkDirectConnectInput",
            cb::network_direct_connect_input
        );
        register_online_input!(self, "PopupHostGameInput", cb::popup_host_game_input);
        register_online_input!(self, "PopupJoinGameInput", cb::popup_join_game_input);
        register_online_input!(
            self,
            "PopupLadderSelectInput",
            cb::popup_ladder_select_input
        );

        self.register_win_input_with_effects(
            "InGamePopupMessageInput",
            |window, msg, d1, d2, effects| {
                cb::in_game_popup_message_input(window, msg, d1, d2, effects)
            },
        );

        self.register_win_input_with_effects("ControlBarInput", |window, msg, d1, d2, effects| {
            control_bar_input(window, msg, d1, d2, effects)
        });
        self.register_win_input_with_effects(
            "ReplayControlInput",
            |window, msg, d1, d2, effects| cb::replay_control_input(window, msg, d1, d2),
        );

        self.register_win_input_with_effects("InGameChatInput", |window, msg, d1, d2, effects| {
            in_game_chat_input(window, msg, d1, d2, effects)
        });
        self.register_win_input_with_effects(
            "DisconnectControlInput",
            |window, msg, d1, d2, effects| default_input_callback(window, msg, d1, d2),
        );
        self.register_win_input_with_effects("DiplomacyInput", |window, msg, d1, d2, effects| {
            diplomacy_input(window, msg, d1, d2, effects)
        });
        self.register_win_input_with_effects(
            "EstablishConnectionsControlInput",
            |window, msg, d1, d2, effects| default_input_callback(window, msg, d1, d2),
        );
        self.register_win_input_with_effects("LeftHUDInput", |window, msg, d1, d2, effects| {
            left_hud_input(window, msg, d1, d2, effects)
        });
        self.register_win_input_with_effects("ScoreScreenInput", |window, msg, d1, d2, effects| {
            cb::score_screen_input(window, msg, d1, d2, effects)
        });
        self.register_win_input_with_effects(
            "SaveLoadMenuInput",
            |window, msg, d1, d2, effects| cb::save_load_menu_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects(
            "BeaconWindowInput",
            |window, msg, d1, d2, effects| cb::beacon_window_input(window, msg, d1, d2),
        );
        self.register_win_input_with_effects(
            "DifficultySelectInput",
            |window, msg, d1, d2, effects| {
                cb::difficulty_select_input(window, msg, d1, d2, effects)
            },
        );
        self.register_win_input_with_effects("PopupReplayInput", |window, msg, d1, d2, effects| {
            cb::popup_replay_input(window, msg, d1, d2, effects)
        });
        self.register_win_input_with_effects(
            "GeneralsExpPointsInput",
            |window, msg, d1, d2, effects| {
                cb::generals_exp_points_input(window, msg, d1, d2, effects)
            },
        );
        self.register_win_input_with_effects(
            "DownloadMenuInput",
            |window, msg, d1, d2, effects| cb::download_menu_input(window, msg, d1, d2, effects),
        );
        self.register_win_input_with_effects(
            "IMECandidateWindowInput",
            |window, msg, d1, d2, effects| cb::ime_candidate_window_input(window, msg, d1, d2),
        );
    }

    fn populate_win_tooltip_table(&mut self) {
        self.register_win_tooltip("GameWinDefaultTooltip", |_win, _time| {});
    }

    fn populate_layout_init_table(&mut self) {
        self.layout_init.insert(
            "MainMenuInit".into(),
            Box::new(|layout, effects| {
                if let Err(err) =
                    effects.with_main_menu(|menu, effects| menu.init(layout, None, effects))
                {
                    log::warn!("MainMenuInit failed: {}", err);
                }
            }),
        );
        self.register_layout_init("OptionsMenuInit", options_menu_init);
        self.register_layout_init("SaveLoadMenuInit", |layout, effects| {
            cb::save_load_menu_init(layout, None, effects)
        });
        self.layout_init.insert(
            "SaveLoadMenuFullScreenInit".into(),
            Box::new(|layout, effects| cb::save_load_menu_full_screen_init(layout, None, effects)),
        );
        self.register_layout_init("PopupCommunicatorInit", |layout, effects| {
            effects.with_manager(|manager, effects| {
                cb::popup_communicator_init(layout, None, manager, effects)
            })
        });
        self.register_layout_init("KeyboardOptionsMenuInit", |layout, effects| {
            cb::keyboard_options_menu_init(layout, None, effects)
        });
        self.layout_init.insert(
            "SinglePlayerMenuInit".into(),
            Box::new(|layout, effects| {
                let _ = effects.with_single_player_menu(|menu, effects| {
                    menu.init_with_effects(layout, None, effects)
                });
            }),
        );
        self.layout_init.insert(
            "MapSelectMenuInit".into(),
            Box::new(|layout, effects| {
                if let Err(error) = effects.with_map_select_menu(|menu, effects| {
                    menu.init_with_effects(layout, None, effects)
                }) {
                    log::warn!("MapSelectMenuInit failed: {error}");
                }
            }),
        );
        self.layout_init.insert(
            "LanLobbyMenuInit".into(),
            Box::new(|layout, effects| {
                if let Err(error) = effects.with_lan_lobby_menu(|menu, effects| {
                    menu.init_with_effects(layout, None, effects)
                }) {
                    log::warn!("LanLobbyMenuInit failed: {error}");
                }
            }),
        );
        self.layout_init.insert(
            "ReplayMenuInit".into(),
            Box::new(|layout, effects| cb::replay_menu_init(layout, None, effects)),
        );
        self.layout_init.insert(
            "CreditsMenuInit".into(),
            Box::new(|layout, effects| {
                if let Err(error) = effects.with_credits_menu(|menu, effects| {
                    menu.init_with_effects(layout, None, effects)
                }) {
                    log::warn!("CreditsMenuInit failed: {error}");
                }
            }),
        );
        self.register_layout_init("LanGameOptionsMenuInit", |layout, effects| {
            cb::lan_game_options_menu_init(layout, None, effects)
        });
        self.register_layout_init("LanMapSelectMenuInit", |layout, effects| {
            cb::lan_map_select_menu_init(layout, None, effects)
        });
        self.layout_init.insert(
            "SkirmishGameOptionsMenuInit".into(),
            Box::new(|layout, effects| cb::skirmish_game_options_menu_init(layout, None, effects)),
        );
        self.register_layout_init("SkirmishMapSelectMenuInit", |layout, effects| {
            cb::skirmish_map_select_menu_init(layout, None, effects)
        });
        self.layout_init.insert(
            "ChallengeMenuInit".into(),
            Box::new(|layout, effects| cb::challenge_menu_init(layout, None, effects)),
        );

        register_online_layout_init!(self, "WOLLadderScreenInit", cb::wol_ladder_screen_init);
        register_online_layout_init!(self, "WOLLoginMenuInit", cb::wol_login_menu_init);
        register_online_layout_init!(self, "WOLLocaleSelectInit", cb::wol_locale_select_init);
        self.register_layout_init("WOLLobbyMenuInit", |layout, effects| {
            cb::wol_lobby_menu_init(layout, None, effects)
        });
        register_online_layout_init!(self, "WOLGameSetupMenuInit", cb::wol_game_setup_menu_init);
        register_online_layout_init!(self, "WOLMapSelectMenuInit", cb::wol_map_select_menu_init);
        register_online_layout_init!(self, "WOLBuddyOverlayInit", cb::wol_buddy_overlay_init);
        register_online_layout_init!(
            self,
            "WOLBuddyOverlayRCMenuInit",
            cb::wol_buddy_overlay_rc_menu_init
        );
        register_online_layout_init!(self, "RCGameDetailsMenuInit", cb::rc_game_details_menu_init);
        register_online_layout_init!(
            self,
            "GameSpyPlayerInfoOverlayInit",
            cb::popup_player_info_init
        );
        register_online_layout_init!(self, "WOLMessageWindowInit", cb::wol_message_window_init);
        register_online_layout_init!(self, "WOLQuickMatchMenuInit", cb::wol_quick_match_menu_init);
        register_online_layout_init!(self, "WOLWelcomeMenuInit", cb::wol_welcome_menu_init);
        register_online_layout_init!(self, "WOLStatusMenuInit", cb::wol_status_menu_init);
        register_online_layout_init!(self, "WOLQMScoreScreenInit", cb::wol_qm_score_screen_init);
        register_online_layout_init!(
            self,
            "WOLCustomScoreScreenInit",
            cb::wol_custom_score_screen_init
        );
        #[cfg(feature = "online_ui")]
        self.layout_init.insert(
            "NetworkDirectConnectInit".into(),
            Box::new(|layout, effects| cb::network_direct_connect_init(layout, None, effects)),
        );
        register_online_layout_init!(self, "PopupHostGameInit", cb::popup_host_game_init);
        register_online_layout_init!(self, "PopupJoinGameInit", cb::popup_join_game_init);
        register_online_layout_init!(self, "PopupLadderSelectInit", cb::popup_ladder_select_init);

        self.register_layout_init("InGamePopupMessageInit", |layout, effects| {
            cb::in_game_popup_message_init(layout, None, effects)
        });
        self.register_layout_init("GameInfoWindowInit", |layout, effects| {
            effects
                .with_manager(|manager, effects| cb::game_info_window_init(layout, None, manager))
        });
        self.layout_init.insert(
            "ScoreScreenInit".into(),
            Box::new(|layout, effects| cb::score_screen_init(layout, None, effects)),
        );
        self.register_layout_init("DownloadMenuInit", |layout, effects| {
            effects.with_manager(|manager, effects| cb::download_menu_init(layout, None, manager))
        });
        self.layout_init.insert(
            "DifficultySelectInit".into(),
            Box::new(|layout, effects| cb::difficulty_select_init(layout, None, effects)),
        );
        self.register_layout_init("PopupReplayInit", |layout, effects| {
            effects.with_manager(|manager, effects| {
                cb::popup_replay_init(layout, None, manager, effects)
            })
        });
    }

    fn populate_layout_update_table(&mut self) {
        self.layout_update.insert(
            "MainMenuUpdate".into(),
            Box::new(|layout, effects| {
                if let Err(err) =
                    effects.with_main_menu(|menu, effects| menu.update(layout, None, effects))
                {
                    log::warn!("MainMenuUpdate failed: {}", err);
                }
            }),
        );
        self.register_layout_update("OptionsMenuUpdate", options_menu_update);
        self.layout_update.insert(
            "SinglePlayerMenuUpdate".into(),
            Box::new(|layout, effects| {
                let _ = effects.with_single_player_menu(|menu, effects| {
                    menu.update_with_effects(layout, None, effects)
                });
            }),
        );
        self.layout_update.insert(
            "MapSelectMenuUpdate".into(),
            Box::new(|layout, effects| {
                let _ = effects.with_map_select_menu(|menu, effects| {
                    menu.update_with_effects(layout, None, effects)
                });
            }),
        );
        self.register_layout_update("LanLobbyMenuUpdate", lan_lobby_menu_update);
        self.register_layout_update("ReplayMenuUpdate", |layout, effects| {
            cb::replay_menu_update(layout, None, effects)
        });
        self.register_layout_update("SaveLoadMenuUpdate", |layout, effects| {
            cb::save_load_menu_update(layout, None, effects)
        });
        self.register_layout_update("CreditsMenuUpdate", credits_menu_update);
        self.register_layout_update("LanGameOptionsMenuUpdate", |layout, effects| {
            cb::lan_game_options_menu_update(layout, None, effects)
        });
        self.register_layout_update("LanMapSelectMenuUpdate", |layout, effects| {
            cb::lan_map_select_menu_update(layout, None)
        });
        self.register_layout_update("SkirmishGameOptionsMenuUpdate", |layout, effects| {
            cb::skirmish_game_options_menu_update(layout, None, effects)
        });
        self.register_layout_update("SkirmishMapSelectMenuUpdate", |layout, effects| {
            cb::skirmish_map_select_menu_update(layout, None)
        });
        self.layout_update.insert(
            "ChallengeMenuUpdate".into(),
            Box::new(|layout, effects| cb::challenge_menu_update(layout, None, effects)),
        );

        register_online_layout_update!(self, "WOLLadderScreenUpdate", cb::wol_ladder_screen_update);
        register_online_layout_update!(self, "WOLLoginMenuUpdate", cb::wol_login_menu_update);
        register_online_layout_update!(self, "WOLLocaleSelectUpdate", cb::wol_locale_select_update);
        register_online_layout_update!(self, "WOLLobbyMenuUpdate", cb::wol_lobby_menu_update);
        register_online_layout_update!(
            self,
            "WOLGameSetupMenuUpdate",
            cb::wol_game_setup_menu_update
        );
        register_online_layout_update!(self, "PopupHostGameUpdate", cb::popup_host_game_update);
        register_online_layout_update!(
            self,
            "WOLMapSelectMenuUpdate",
            cb::wol_map_select_menu_update
        );
        register_online_layout_update!(self, "WOLBuddyOverlayUpdate", cb::wol_buddy_overlay_update);
        register_online_layout_update!(
            self,
            "GameSpyPlayerInfoOverlayUpdate",
            cb::popup_player_info_update
        );
        register_online_layout_update!(
            self,
            "WOLMessageWindowUpdate",
            cb::wol_message_window_update
        );
        register_online_layout_update!(
            self,
            "WOLQuickMatchMenuUpdate",
            cb::wol_quick_match_menu_update
        );
        register_online_layout_update!(self, "WOLWelcomeMenuUpdate", cb::wol_welcome_menu_update);
        register_online_layout_update!(self, "WOLStatusMenuUpdate", cb::wol_status_menu_update);
        register_online_layout_update!(
            self,
            "WOLQMScoreScreenUpdate",
            cb::wol_qm_score_screen_update
        );
        register_online_layout_update!(
            self,
            "WOLCustomScoreScreenUpdate",
            cb::wol_custom_score_screen_update
        );
        register_online_layout_update!(
            self,
            "NetworkDirectConnectUpdate",
            cb::network_direct_connect_update
        );

        self.layout_update.insert(
            "ScoreScreenUpdate".into(),
            Box::new(|layout, effects| cb::score_screen_update(layout, None, effects)),
        );
        self.register_layout_update("DownloadMenuUpdate", |layout, effects| {
            effects.with_manager(|manager, effects| {
                cb::download_menu_update(layout, None, manager, effects)
            })
        });
        self.register_layout_update("PopupReplayUpdate", |layout, effects| {
            effects.with_manager(|manager, effects| {
                cb::popup_replay_update(layout, None, manager, effects)
            })
        });
    }

    fn populate_layout_shutdown_table(&mut self) {
        self.layout_shutdown.insert(
            "MainMenuShutdown".into(),
            Box::new(|layout, data, effects| {
                if let Err(err) =
                    effects.with_main_menu(|menu, effects| menu.shutdown(layout, data, effects))
                {
                    log::warn!("MainMenuShutdown failed: {}", err);
                }
            }),
        );
        self.register_layout_shutdown_without_data("OptionsMenuShutdown", options_menu_shutdown);
        self.register_layout_shutdown_without_data("SaveLoadMenuShutdown", |layout, effects| {
            cb::save_load_menu_shutdown(layout, None, effects)
        });
        self.register_layout_shutdown_without_data(
            "PopupCommunicatorShutdown",
            |layout, effects| cb::popup_communicator_shutdown(layout, None),
        );
        self.register_layout_shutdown_without_data(
            "KeyboardOptionsMenuShutdown",
            |layout, effects| cb::keyboard_options_menu_shutdown(layout, None, effects),
        );
        self.register_layout_shutdown_without_data(
            "SinglePlayerMenuShutdown",
            single_player_menu_shutdown,
        );
        self.register_layout_shutdown_without_data(
            "MapSelectMenuShutdown",
            map_select_menu_shutdown,
        );
        self.register_layout_shutdown_without_data("LanLobbyMenuShutdown", lan_lobby_menu_shutdown);
        self.register_layout_shutdown_without_data("ReplayMenuShutdown", |layout, effects| {
            cb::replay_menu_shutdown(layout, None, effects)
        });
        self.layout_shutdown.insert(
            "CreditsMenuShutdown".into(),
            Box::new(|layout, _, effects| {
                if let Err(error) = effects.with_credits_menu(|menu, effects| {
                    menu.shutdown_with_effects(layout, None, effects)
                }) {
                    log::warn!("CreditsMenuShutdown failed: {error}");
                }
            }),
        );
        self.register_layout_shutdown_without_data(
            "LanGameOptionsMenuShutdown",
            |layout, effects| cb::lan_game_options_menu_shutdown(layout, None, effects),
        );
        self.register_layout_shutdown_without_data(
            "LanMapSelectMenuShutdown",
            |layout, effects| cb::lan_map_select_menu_shutdown(layout, None, effects),
        );
        self.register_layout_shutdown_without_data(
            "SkirmishGameOptionsMenuShutdown",
            |layout, effects| cb::skirmish_game_options_menu_shutdown(layout, None, effects),
        );
        self.register_layout_shutdown_without_data(
            "SkirmishMapSelectMenuShutdown",
            |layout, effects| cb::skirmish_map_select_menu_shutdown(layout, None, effects),
        );
        self.layout_shutdown.insert(
            "ChallengeMenuShutdown".into(),
            Box::new(|layout, data, effects| cb::challenge_menu_shutdown(layout, data, effects)),
        );

        register_online_layout_shutdown!(
            self,
            "WOLLadderScreenShutdown",
            cb::wol_ladder_screen_shutdown
        );
        register_online_layout_shutdown!(self, "WOLLoginMenuShutdown", cb::wol_login_menu_shutdown);
        register_online_layout_shutdown!(
            self,
            "WOLLocaleSelectShutdown",
            cb::wol_locale_select_shutdown
        );
        self.register_layout_shutdown_without_data("WOLLobbyMenuShutdown", |layout, effects| {
            cb::wol_lobby_menu_shutdown(layout, None, effects)
        });
        register_online_layout_shutdown!(
            self,
            "WOLGameSetupMenuShutdown",
            cb::wol_game_setup_menu_shutdown
        );
        register_online_layout_shutdown!(
            self,
            "WOLMapSelectMenuShutdown",
            cb::wol_map_select_menu_shutdown
        );
        register_online_layout_shutdown!(
            self,
            "WOLBuddyOverlayShutdown",
            cb::wol_buddy_overlay_shutdown
        );
        register_online_layout_shutdown!(
            self,
            "GameSpyPlayerInfoOverlayShutdown",
            cb::popup_player_info_shutdown
        );
        register_online_layout_shutdown!(
            self,
            "WOLMessageWindowShutdown",
            cb::wol_message_window_shutdown
        );
        register_online_layout_shutdown!(
            self,
            "WOLQuickMatchMenuShutdown",
            cb::wol_quick_match_menu_shutdown
        );
        register_online_layout_shutdown!(
            self,
            "WOLWelcomeMenuShutdown",
            cb::wol_welcome_menu_shutdown
        );
        register_online_layout_shutdown!(
            self,
            "WOLStatusMenuShutdown",
            cb::wol_status_menu_shutdown
        );
        register_online_layout_shutdown!(
            self,
            "WOLQMScoreScreenShutdown",
            cb::wol_qm_score_screen_shutdown
        );
        register_online_layout_shutdown!(
            self,
            "WOLCustomScoreScreenShutdown",
            cb::wol_custom_score_screen_shutdown
        );
        register_online_layout_shutdown!(
            self,
            "NetworkDirectConnectShutdown",
            cb::network_direct_connect_shutdown
        );

        self.register_layout_shutdown_without_data("ScoreScreenShutdown", |layout, effects| {
            cb::score_screen_shutdown(layout, None, effects)
        });
        self.register_layout_shutdown_without_data("DownloadMenuShutdown", |layout, effects| {
            cb::download_menu_shutdown(layout, None)
        });
        self.register_layout_shutdown_without_data("PopupReplayShutdown", |layout, effects| {
            cb::popup_replay_shutdown(layout, None, effects)
        });
    }
}

impl Default for ScriptCallbackRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Gadget system dispatch functions
// PARITY_NOTE: Each mirrors the C++ GadgetXxx::System() / GadgetXxx::Input()
// pattern — verify widget type, allow built-in widget dispatch to process.
// Returning Ignored allows GameWindow::handle_widget_system/input to run.
// ---------------------------------------------------------------------------

fn gadget_push_button_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::PushButton(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    match msg {
        WindowMessage::GadgetSelected
        | WindowMessage::GadgetMouseEntering
        | WindowMessage::GadgetMouseLeaving
        | WindowMessage::GadgetRightClick
        | WindowMessage::InputFocus => {}
        _ => {}
    }
    WindowMsgHandled::Ignored
}

fn gadget_check_box_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::CheckBox(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let _ = msg;
    WindowMsgHandled::Ignored
}

fn gadget_radio_button_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::RadioButton(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let _ = msg;
    WindowMsgHandled::Ignored
}

fn gadget_tab_control_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::TabControl(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let _ = msg;
    WindowMsgHandled::Ignored
}

fn gadget_list_box_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::ListBox(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let _ = msg;
    WindowMsgHandled::Ignored
}

fn gadget_combo_box_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::ComboBox(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let _ = msg;
    WindowMsgHandled::Ignored
}

fn gadget_horizontal_slider_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::HorizontalSlider(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let _ = msg;
    WindowMsgHandled::Ignored
}

fn gadget_vertical_slider_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::VerticalSlider(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let _ = msg;
    WindowMsgHandled::Ignored
}

fn gadget_progress_bar_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::ProgressBar(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let _ = msg;
    WindowMsgHandled::Ignored
}

fn gadget_static_text_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::StaticText(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let _ = msg;
    WindowMsgHandled::Ignored
}

fn gadget_text_entry_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::TextEntry(_)) => {}
        _ => return WindowMsgHandled::Ignored,
    }
    let _ = msg;
    WindowMsgHandled::Ignored
}

fn legacy_main_menu_message(msg: WindowMessage) -> u32 {
    match msg {
        WindowMessage::None => 0,
        WindowMessage::Create => 1,
        WindowMessage::Destroy => 2,
        WindowMessage::Activate => 3,
        WindowMessage::Enable => 4,
        WindowMessage::LeftDown => 5,
        WindowMessage::LeftUp => 6,
        WindowMessage::LeftDoubleClick => 7,
        WindowMessage::LeftDrag => 8,
        WindowMessage::MiddleDown => 9,
        WindowMessage::MiddleUp => 10,
        WindowMessage::MiddleDoubleClick => 11,
        WindowMessage::MiddleDrag => 12,
        WindowMessage::RightDown => 13,
        WindowMessage::RightUp => 14,
        WindowMessage::RightDoubleClick => 15,
        WindowMessage::RightDrag => 16,
        WindowMessage::MouseEntering => 17,
        WindowMessage::MouseLeaving => 18,
        WindowMessage::WheelUp => 19,
        WindowMessage::WheelDown => 20,
        WindowMessage::Char => 21,
        WindowMessage::ScriptCreate => 22,
        WindowMessage::InputFocus => 23,
        WindowMessage::MousePos => 24,
        WindowMessage::ImeChar => 25,
        WindowMessage::ImeString => 26,
        WindowMessage::GadgetSelected => 16392,
        WindowMessage::GadgetMouseEntering => 16390,
        WindowMessage::GadgetMouseLeaving => 16391,
        WindowMessage::GadgetEditDone => 0x0080,
        WindowMessage::GadgetValueChanged => 0x0081,
        WindowMessage::GadgetRightClick => 0x0082,
        WindowMessage::User(value) => value,
    }
}

fn main_menu_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if effects.with_main_menu_for_window(window, |menu, effects, window| {
        menu.system_for_window(window, legacy_main_menu_message(msg), data1, data2, effects)
    }) {
        WindowMsgHandled::Handled
    } else {
        WindowMsgHandled::Ignored
    }
}

fn main_menu_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if effects.with_main_menu_for_window(window, |menu, effects, window| {
        menu.input_for_window(window, legacy_main_menu_message(msg), data1, data2, effects)
    }) {
        WindowMsgHandled::Handled
    } else {
        WindowMsgHandled::Ignored
    }
}

fn with_arc_write<T, R>(lock: &Arc<RwLock<T>>, f: impl FnOnce(&mut T) -> R) -> R {
    let mut guard = lock.write().unwrap_or_else(|e| e.into_inner());
    f(&mut *guard)
}

fn control_bar_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    cb::receiving_control_bar_system(window, msg, data1, data2, effects)
}

fn control_bar_observer_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    crate::gui::control_bar::control_bar_observer::control_bar_observer_system(
        window, msg, data1, data2, effects,
    )
}

fn control_bar_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    cb::receiving_control_bar_system(window, msg, data1, data2, effects)
}

fn left_hud_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    cb::left_hud_input(
        window,
        msg,
        data1,
        data2,
        effects.emitted_messages,
        effects.pending_control_bar_clicks.as_deref_mut(),
        effects.ui.as_deref_mut().expect("receiving UI"),
    )
}

fn diplomacy_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    let system = cb::get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let callbacks = system.get_callbacks();
    with_arc_write(&callbacks, |callbacks| {
        callbacks.system(window, msg, data1, data2)
    })
}

fn diplomacy_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    let system = cb::get_diplomacy_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let callbacks = system.get_callbacks();
    with_arc_write(&callbacks, |callbacks| {
        effects.with_manager(|manager, effects| {
            callbacks.input(window, msg, data1, data2, manager, effects)
        })
    })
}

fn in_game_chat_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    cb::in_game_chat_system_receiving(window, msg, data1, data2, effects)
}

fn in_game_chat_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    cb::in_game_chat_input_receiving(window, msg, data1, data2, effects)
}

fn idle_worker_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    cb::idle_worker_system_receiving(window, msg, data1, data2, effects)
}

fn message_box_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    let system = cb::get_message_box_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let standard = system.get_standard();
    with_arc_write(&standard, |standard| {
        standard.system(window, msg, data1, data2, effects)
    })
}

fn extended_message_box_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    let system = cb::get_message_box_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let extended = system.get_extended();
    with_arc_write(&extended, |extended| {
        extended.system(window, msg, data1, data2, effects)
    })
}

fn quit_message_box_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    let system = cb::get_message_box_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let quit = system.get_quit();
    with_arc_write(&quit, |quit| {
        quit.system(window, msg, data1, data2, effects)
    })
}

fn disconnect_control_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if !matches!(
        msg,
        WindowMessage::GadgetSelected | WindowMessage::GadgetEditDone
    ) {
        return WindowMsgHandled::Ignored;
    }
    effects.with_disconnect_menu(|menu, effects| menu.system(window, msg, data1, data2, effects))
}

fn establish_connections_control_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if !matches!(msg, WindowMessage::GadgetSelected) {
        return WindowMsgHandled::Ignored;
    }
    effects.with_establish_connections_menu(|menu, effects| {
        menu.system(window, msg, data1, data2, effects)
    })
}

fn default_input_callback(
    _window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    WindowMsgHandled::Ignored
}

fn single_player_menu_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_single_player_menu(|menu, effects| {
        menu.system_with_effects(window, msg, data1, data2, effects)
    })
}

fn options_menu_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_options_menu(|menu, effects| {
        menu.system_with_effects(window, msg, data1, data2, effects)
    })
}

fn map_select_menu_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_map_select_menu(|menu, effects| {
        menu.system_with_effects(window, msg, data1, data2, effects)
    })
}

fn credits_menu_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_credits_menu(|menu, effects| {
        menu.system_with_effects(window, msg, data1, data2, effects)
    })
}

fn lan_lobby_menu_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_lan_lobby_menu(|menu, effects| {
        menu.system_with_effects(window, msg, data1, data2, effects)
    })
}

fn single_player_menu_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_single_player_menu(|menu, effects| {
        menu.input_with_effects(window, msg, data1, data2, effects)
    })
}

fn options_menu_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_options_menu(|menu, effects| {
        menu.input_with_effects(window, msg, data1, data2, effects)
    })
}

fn map_select_menu_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_map_select_menu(|menu, effects| {
        menu.input_with_effects(window, msg, data1, data2, effects)
    })
}

fn credits_menu_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_credits_menu(|menu, effects| {
        menu.input_with_effects(window, msg, data1, data2, effects)
    })
}

fn lan_lobby_menu_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    effects.with_lan_lobby_menu(|menu, effects| {
        menu.input_with_effects(window, msg, data1, data2, effects)
    })
}

fn single_player_menu_init(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) = effects
        .with_single_player_menu(|menu, effects| menu.init_with_effects(layout, None, effects))
    {
        log::warn!("single_player_menu_init failed: {}", error);
    }
}

fn options_menu_init(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) =
        effects.with_options_menu(|menu, effects| menu.init_with_effects(layout, None, effects))
    {
        log::warn!("options_menu_init failed: {}", error);
    }
}

fn map_select_menu_init(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) =
        effects.with_map_select_menu(|menu, effects| menu.init_with_effects(layout, None, effects))
    {
        log::warn!("map_select_menu_init failed: {}", error);
    }
}

fn credits_menu_init(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) =
        effects.with_credits_menu(|menu, effects| menu.init_with_effects(layout, None, effects))
    {
        log::warn!("credits_menu_init failed: {}", error);
    }
}

fn lan_lobby_menu_init(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) =
        effects.with_lan_lobby_menu(|menu, effects| menu.init_with_effects(layout, None, effects))
    {
        log::warn!("lan_lobby_menu_init failed: {}", error);
    }
}

fn single_player_menu_update(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) = effects
        .with_single_player_menu(|menu, effects| menu.update_with_effects(layout, None, effects))
    {
        log::warn!("single_player_menu_update failed: {}", error);
    }
}

fn options_menu_update(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) =
        effects.with_options_menu(|menu, effects| menu.update_with_effects(layout, None, effects))
    {
        log::warn!("options_menu_update failed: {}", error);
    }
}

fn map_select_menu_update(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) = effects
        .with_map_select_menu(|menu, effects| menu.update_with_effects(layout, None, effects))
    {
        log::warn!("map_select_menu_update failed: {}", error);
    }
}

fn credits_menu_update(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) =
        effects.with_credits_menu(|menu, effects| menu.update_with_effects(layout, None, effects))
    {
        log::warn!("credits_menu_update failed: {}", error);
    }
}

fn lan_lobby_menu_update(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) =
        effects.with_lan_lobby_menu(|menu, effects| menu.update_with_effects(layout, None, effects))
    {
        log::warn!("lan_lobby_menu_update failed: {}", error);
    }
}

fn single_player_menu_shutdown(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) = effects
        .with_single_player_menu(|menu, effects| menu.shutdown_with_effects(layout, None, effects))
    {
        log::warn!("single_player_menu_shutdown failed: {}", error);
    }
}

fn options_menu_shutdown(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) =
        effects.with_options_menu(|menu, effects| menu.shutdown_with_effects(layout, None, effects))
    {
        log::warn!("options_menu_shutdown failed: {}", error);
    }
}

fn map_select_menu_shutdown(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) = effects
        .with_map_select_menu(|menu, effects| menu.shutdown_with_effects(layout, None, effects))
    {
        log::warn!("map_select_menu_shutdown failed: {}", error);
    }
}

fn credits_menu_shutdown(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) =
        effects.with_credits_menu(|menu, effects| menu.shutdown_with_effects(layout, None, effects))
    {
        log::warn!("credits_menu_shutdown failed: {}", error);
    }
}

fn lan_lobby_menu_shutdown(
    layout: &WindowLayout,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Err(error) = effects
        .with_lan_lobby_menu(|menu, effects| menu.shutdown_with_effects(layout, None, effects))
    {
        log::warn!("lan_lobby_menu_shutdown failed: {}", error);
    }
}

// ---------------------------------------------------------------------------
// Gadget input dispatch functions
// PARITY_NOTE: Each mirrors the C++ GadgetXxx::Input() pattern. Returns
// Ignored so GameWindow::handle_widget_input converts the message to an
// InputEvent and dispatches to the widget's handle_input method.
// ---------------------------------------------------------------------------

fn gadget_push_button_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::PushButton(_)) => WindowMsgHandled::Ignored,
        _ => WindowMsgHandled::Ignored,
    }
}

fn gadget_check_box_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::CheckBox(_)) => WindowMsgHandled::Ignored,
        _ => WindowMsgHandled::Ignored,
    }
}

fn gadget_radio_button_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::RadioButton(_)) => WindowMsgHandled::Ignored,
        _ => WindowMsgHandled::Ignored,
    }
}

fn gadget_tab_control_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::TabControl(_)) => WindowMsgHandled::Ignored,
        _ => WindowMsgHandled::Ignored,
    }
}

fn gadget_list_box_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::ListBox(_)) => WindowMsgHandled::Ignored,
        _ => WindowMsgHandled::Ignored,
    }
}

fn gadget_list_box_multi_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::ListBox(_)) => WindowMsgHandled::Ignored,
        _ => WindowMsgHandled::Ignored,
    }
}

fn gadget_combo_box_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::ComboBox(_)) => WindowMsgHandled::Ignored,
        _ => WindowMsgHandled::Ignored,
    }
}

fn gadget_horizontal_slider_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::HorizontalSlider(_)) => WindowMsgHandled::Ignored,
        _ => WindowMsgHandled::Ignored,
    }
}

fn gadget_vertical_slider_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::VerticalSlider(_)) => WindowMsgHandled::Ignored,
        _ => WindowMsgHandled::Ignored,
    }
}

fn gadget_static_text_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    WindowMsgHandled::Ignored
}

fn gadget_text_entry_input(
    window: &mut GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match window.widget() {
        Some(WindowWidget::TextEntry(_)) => WindowMsgHandled::Ignored,
        _ => WindowMsgHandled::Ignored,
    }
}

// ---------------------------------------------------------------------------
// Bridge: create WindowCallbacks from registry lookups
// PARITY_NOTE: mirrors C++ parseSystemCallback/parseInputCallback which
// resolves a callback name from TheFunctionLexicon and assigns it to the
// GameWindow's callback slot.
// ---------------------------------------------------------------------------

impl ScriptCallbackRegistry {
    /// Resolve a system callback name and create a `GwCallbacks` struct
    /// with the system callback wired. Returns `None` if the name resolves
    /// to nothing or is "[None]".
    pub fn create_system_callback(
        &self,
        name: &str,
    ) -> Option<
        Rc<
            dyn for<'a> Fn(
                &mut GameWindow,
                WindowMessage,
                WindowMsgData,
                WindowMsgData,
                &mut crate::gui::game_window::WindowEventEffects<'a>,
            ) -> WindowMsgHandled,
        >,
    > {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        let cb = Arc::clone(self.win_system.get(&normalized)?);
        Some(Rc::new(move |win, msg, d1, d2, effects| {
            cb(win, msg, d1, d2, effects)
        }))
    }

    pub fn create_input_callback(
        &self,
        name: &str,
    ) -> Option<
        Rc<
            dyn for<'a> Fn(
                &mut GameWindow,
                WindowMessage,
                WindowMsgData,
                WindowMsgData,
                &mut crate::gui::game_window::WindowEventEffects<'a>,
            ) -> WindowMsgHandled,
        >,
    > {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        let cb = Arc::clone(self.win_input.get(&normalized)?);
        Some(Rc::new(move |win, msg, d1, d2, effects| {
            cb(win, msg, d1, d2, effects)
        }))
    }

    pub fn create_tooltip_callback(&self, name: &str) -> Option<Box<dyn Fn(&GameWindow, u32)>> {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        let cb = Arc::clone(self.win_tooltip.get(&normalized)?);
        Some(Box::new(move |win, time| cb(win, time)))
    }

    pub fn create_draw_callback(
        &self,
        name: &str,
    ) -> Option<
        Box<
            dyn Fn(
                &GameWindow,
                &WindowInstanceData,
                &mut crate::gui::window_draw_context::WindowDrawContext<'_>,
            ),
        >,
    > {
        let normalized = normalize_callback_name(name);
        if normalized.is_empty() {
            return None;
        }
        let cb = Arc::clone(self.win_draw.get(&normalized)?);
        Some(Box::new(move |win, inst, draw_context| {
            cb(win, inst, draw_context)
        }))
    }

    /// Create a complete `GwCallbacks` instance by resolving all four
    /// callback names (system, input, tooltip, draw). Each field is set
    /// only if the name resolves to a registered callback.
    pub fn create_window_callbacks(
        &self,
        system_name: &str,
        input_name: &str,
        tooltip_name: &str,
        draw_name: &str,
    ) -> GwCallbacks {
        let mut callbacks = GwCallbacks::default();
        if let Some(callback) = self
            .win_system_legacy
            .get(&normalize_callback_name(system_name))
        {
            let callback = Arc::clone(callback);
            callbacks.system = Some(Rc::new(move |win, msg, d1, d2| callback(win, msg, d1, d2)));
        } else if let Some(cb) = self.create_system_callback(system_name) {
            callbacks.system_with_message_sink = Some(cb);
        }
        if let Some(callback) = self
            .win_input_legacy
            .get(&normalize_callback_name(input_name))
        {
            let callback = Arc::clone(callback);
            callbacks.input = Some(Rc::new(move |win, msg, d1, d2| callback(win, msg, d1, d2)));
        } else if let Some(cb) = self.create_input_callback(input_name) {
            callbacks.input_with_message_sink = Some(cb);
        }
        if let Some(cb) = self.create_tooltip_callback(tooltip_name) {
            // Adapt from Fn(&GameWindow, u32) to Fn(&GameWindow, &WindowInstanceData, u32)
            callbacks.tooltip = Some(Box::new(
                move |win: &GameWindow, _inst: &WindowInstanceData, time: u32| {
                    cb(win, time);
                },
            ));
        }
        if let Some(cb) = self.create_draw_callback(draw_name) {
            callbacks.draw =
                Some(Box::new(
                    move |win: &GameWindow,
                          inst: &WindowInstanceData,
                          draw_context: &mut crate::gui::window_draw_context::WindowDrawContext<
                        '_,
                    >| {
                        cb(win, inst, draw_context);
                    },
                ));
        }
        callbacks
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
#[derive(Debug, Clone, Default)]
pub struct WindowDefaults {
    pub enabled_color: u32,
    pub disabled_color: u32,
    pub background_color: u32,
    pub hilite_color: u32,
    pub selected_color: u32,
    pub text_color: u32,
    pub font: Option<super::game_window::GameFont>,
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
    /// Create a new engine with a fully populated callback registry.
    /// PARITY_NOTE: mirrors C++ `TheFunctionLexicon->init()` which loads all
    /// callback tables at startup.
    pub fn new() -> Self {
        let mut registry = ScriptCallbackRegistry::new();
        registry.populate_defaults();
        Self {
            registry,
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
        registry.register_layout_init("TestInit", move |_layout, effects| {
            called_clone.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        let cb = registry.get_layout_init("TestInit").unwrap();
        let layout = WindowLayout::new("test".to_string());
        cb(&layout);
        assert!(called.load(std::sync::atomic::Ordering::Relaxed));

        assert!(registry.get_layout_init("None").is_none());
        assert!(registry.get_layout_init("[None]").is_none());
    }

    #[test]
    fn test_callback_registry_win_system() {
        use crate::gui::game_window::WindowMsgHandled;
        let mut registry = ScriptCallbackRegistry::new();
        registry.register_win_system("TestSystem", |_win, _msg, _d1, _d2| {
            WindowMsgHandled::Ignored
        });
        let cb = registry.get_win_system("TestSystem").unwrap();
        let _ = cb;
        assert!(registry.get_win_system("Nonexistent").is_none());
    }

    #[cfg(not(feature = "online_ui"))]
    #[test]
    fn deferred_online_callbacks_remain_registered_and_fail_closed() {
        let mut registry = ScriptCallbackRegistry::new();
        registry.populate_defaults();

        // The .wnd lexicon must still resolve these names in the supported
        // offline build, even though their transport-dependent bodies are
        // deliberately no-ops until the network milestone lands.
        for name in [
            "WOLLoginMenuSystem",
            "WOLGameSetupMenuSystem",
            "WOLBuddyOverlaySystem",
            "GameSpyPlayerInfoOverlaySystem",
            "PopupHostGameSystem",
        ] {
            assert!(registry.get_win_system(name).is_some(), "missing {name}");
        }
        for name in [
            "WOLLoginMenuInit",
            "WOLGameSetupMenuInit",
            "WOLBuddyOverlayInit",
            "GameSpyPlayerInfoOverlayInit",
            "PopupHostGameInit",
        ] {
            assert!(registry.get_layout_init(name).is_some(), "missing {name}");
        }
    }

    /// C++ FunctionLexicon.cpp gameWinSystemTable WOL block order
    /// (lines 91-106) plus the immediately following popup names.
    #[cfg(feature = "online_ui")]
    const ONLINE_SYSTEM_NAMES: &[&str] = &[
        "WOLLadderScreenSystem",
        "WOLLoginMenuSystem",
        "WOLLocaleSelectSystem",
        "WOLLobbyMenuSystem",
        "WOLGameSetupMenuSystem",
        "WOLMapSelectMenuSystem",
        "WOLBuddyOverlaySystem",
        "WOLBuddyOverlayRCMenuSystem",
        "RCGameDetailsMenuSystem",
        "GameSpyPlayerInfoOverlaySystem",
        "WOLMessageWindowSystem",
        "WOLQuickMatchMenuSystem",
        "WOLWelcomeMenuSystem",
        "WOLStatusMenuSystem",
        "WOLQMScoreScreenSystem",
        "WOLCustomScoreScreenSystem",
        "NetworkDirectConnectSystem",
        "PopupHostGameSystem",
        "PopupJoinGameSystem",
        "PopupLadderSelectSystem",
    ];

    #[cfg(feature = "online_ui")]
    #[test]
    fn online_ui_binds_concrete_wol_gamespy_callbacks_in_lexicon_order() {
        let mut registry = ScriptCallbackRegistry::new();
        registry.populate_defaults();

        for name in ONLINE_SYSTEM_NAMES {
            assert!(
                registry.get_win_system(name).is_some(),
                "missing online system {name}"
            );
        }
        for name in [
            "WOLLoginMenuInit",
            "WOLWelcomeMenuInit",
            "WOLLobbyMenuInit",
            "WOLGameSetupMenuInit",
            "WOLBuddyOverlayInit",
            "GameSpyPlayerInfoOverlayInit",
            "PopupHostGameInit",
        ] {
            assert!(registry.get_layout_init(name).is_some(), "missing {name}");
        }

        let win = GameWindow::new();
        let login = registry.get_win_system("WOLLoginMenuSystem").unwrap();
        assert_eq!(
            login(&win, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            login(&win, WindowMessage::User(1), 0, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn create_system_callback_survives_registry_replace() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut registry = ScriptCallbackRegistry::new();
        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let hits_for_cb = hits.clone();
        registry.register_win_system("OwnedSystem", move |_win, _msg, _d1, _d2| {
            hits_for_cb.fetch_add(1, Ordering::Relaxed);
            WindowMsgHandled::Handled
        });

        let created = registry.create_system_callback("OwnedSystem").unwrap();

        // Replacing the lexicon entry must not invalidate already-resolved
        // callbacks (C++ function pointers stay valid).
        registry.register_win_system("OwnedSystem", |_win, _msg, _d1, _d2| {
            WindowMsgHandled::Ignored
        });

        let win = GameWindow::new();
        assert_eq!(
            created(&win, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(hits.load(Ordering::Relaxed), 1);

        let replacement = registry.create_system_callback("OwnedSystem").unwrap();
        assert_eq!(
            replacement(&win, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Ignored
        );
        assert_eq!(hits.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn create_input_callback_survives_registry_drop() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let created = {
            let mut registry = ScriptCallbackRegistry::new();
            let hits_for_cb = hits.clone();
            registry.register_win_input("OwnedInput", move |_win, _msg, _d1, _d2| {
                hits_for_cb.fetch_add(1, Ordering::Relaxed);
                WindowMsgHandled::Handled
            });
            registry.create_input_callback("OwnedInput").unwrap()
            // registry dropped here
        };

        let win = GameWindow::new();
        assert_eq!(
            created(&win, WindowMessage::LeftDown, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(hits.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn create_tooltip_and_draw_callbacks_survive_registry_replace() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut registry = ScriptCallbackRegistry::new();
        let tooltip_hits = std::sync::Arc::new(AtomicUsize::new(0));
        let draw_hits = std::sync::Arc::new(AtomicUsize::new(0));
        let tooltip_for_cb = tooltip_hits.clone();
        let draw_for_cb = draw_hits.clone();

        registry.register_win_tooltip("OwnedTooltip", move |_win, time| {
            tooltip_for_cb.fetch_add(time as usize, Ordering::Relaxed);
        });
        registry.register_win_draw("OwnedDraw", move |_win, _inst, _draw_context| {
            draw_for_cb.fetch_add(1, Ordering::Relaxed);
        });

        let tooltip = registry.create_tooltip_callback("OwnedTooltip").unwrap();
        let draw = registry.create_draw_callback("OwnedDraw").unwrap();

        registry.register_win_tooltip("OwnedTooltip", |_win, _time| {});
        registry.register_win_draw("OwnedDraw", |_win, _inst, _draw_context| {});

        let win = GameWindow::new();
        let inst = WindowInstanceData::default();
        tooltip(&win, 7);
        draw(&win, &inst);
        assert_eq!(tooltip_hits.load(Ordering::Relaxed), 7);
        assert_eq!(draw_hits.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn main_menu_callbacks_are_wired_to_registry() {
        let mut registry = ScriptCallbackRegistry::new();
        registry.populate_defaults();

        assert!(registry.get_win_system("MainMenuSystem").is_some());
        assert!(registry.get_win_input("MainMenuInput").is_some());
        assert!(registry.get_layout_init("MainMenuInit").is_some());
        assert!(registry.get_layout_update("MainMenuUpdate").is_some());
        assert!(registry.get_layout_shutdown("MainMenuShutdown").is_some());
    }

    #[test]
    fn ime_draw_callbacks_are_wired_with_instance_data() {
        let mut registry = ScriptCallbackRegistry::new();
        registry.populate_defaults();

        assert!(registry.get_win_draw("IMECandidateMainDraw").is_some());
        assert!(registry.get_win_draw("IMECandidateTextAreaDraw").is_some());
        assert!(
            registry
                .create_draw_callback("IMECandidateMainDraw")
                .is_some()
        );
        assert!(
            registry
                .create_draw_callback("IMECandidateTextAreaDraw")
                .is_some()
        );
    }

    #[test]
    fn menu_callback_trait_adapters_are_wired_to_registry() {
        let mut registry = ScriptCallbackRegistry::new();
        registry.populate_defaults();

        for name in [
            "SinglePlayerMenu",
            "OptionsMenu",
            "MapSelectMenu",
            "CreditsMenu",
            "LanLobbyMenu",
        ] {
            assert!(registry.get_win_system(&format!("{name}System")).is_some());
            assert!(registry.get_win_input(&format!("{name}Input")).is_some());
            assert!(registry.get_layout_init(&format!("{name}Init")).is_some());
            assert!(
                registry
                    .get_layout_update(&format!("{name}Update"))
                    .is_some()
            );
            assert!(
                registry
                    .get_layout_shutdown(&format!("{name}Shutdown"))
                    .is_some()
            );
        }
    }

    #[test]
    fn singleton_callback_adapters_are_wired_to_registry() {
        let mut registry = ScriptCallbackRegistry::new();
        registry.populate_defaults();

        for name in [
            "MessageBoxSystem",
            "QuitMessageBoxSystem",
            "ExtendedMessageBoxSystem",
            "ControlBarSystem",
            "ControlBarObserverSystem",
            "InGameChatSystem",
            "DisconnectControlSystem",
            "EstablishConnectionsControlSystem",
            "DiplomacySystem",
            "IdleWorkerSystem",
        ] {
            assert!(registry.get_win_system(name).is_some());
        }

        for name in [
            "ControlBarInput",
            "InGameChatInput",
            "DisconnectControlInput",
            "EstablishConnectionsControlInput",
            "DiplomacyInput",
            "LeftHUDInput",
        ] {
            assert!(registry.get_win_input(name).is_some());
        }
    }

    #[test]
    fn main_menu_adapter_uses_legacy_gadget_message_ids() {
        assert_eq!(legacy_main_menu_message(WindowMessage::Create), 1);
        assert_eq!(legacy_main_menu_message(WindowMessage::InputFocus), 23);
        assert_eq!(
            legacy_main_menu_message(WindowMessage::GadgetMouseEntering),
            16390
        );
        assert_eq!(
            legacy_main_menu_message(WindowMessage::GadgetMouseLeaving),
            16391
        );
        assert_eq!(
            legacy_main_menu_message(WindowMessage::GadgetSelected),
            16392
        );
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
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("DefaultText.wnd");
        std::fs::write(
            &path,
            concat!(
                "TEXTCOLOR=187 204 221 170;\n",
                "FONT=NAME:\"Arial\", SIZE:12, BOLD:0;\n",
                "WINDOW\nWINDOWTYPE=USER;\nNAME=\"DefaultText\";\n",
                "SCREENRECT=0 0 100 100;\nEND\n",
            ),
        )
        .unwrap();
        let engine = WindowScriptEngine::new();
        let parsed = engine.load_window_layout(&path).unwrap();
        assert_eq!(parsed.windows.len(), 1);
        let def = &parsed.windows[0];
        assert_eq!(def.enabled_text.color, 0xAABBCCDD);
        assert_eq!(def.disabled_text.color, 0xAABBCCDD);
        assert_eq!(def.hilite_text.color, 0xAABBCCDD);
        assert_eq!(def.enabled_text.border_color, 0xAABBCCDD);
        assert_eq!(def.disabled_text.border_color, 0xAABBCCDD);
        assert_eq!(def.hilite_text.border_color, 0xAABBCCDD);

        let mut manager = super::super::window_manager::WindowManager::new();
        let info = manager
            .create_windows_from_script(path.to_str().unwrap())
            .unwrap();
        let window = info.windows[0].borrow();
        let data = window.instance_data();
        assert_eq!(data.enabled_text.color, 0xAABBCCDD);
        assert_eq!(data.disabled_text.color, 0xAABBCCDD);
        assert_eq!(data.hilite_text.color, 0xAABBCCDD);
        assert!(data.font.is_some());
        let font = data.font.as_ref().unwrap();
        assert_eq!(font.name, "Arial");
        assert_eq!(font.size, 12);
        assert!(!font.bold);
    }

    #[test]
    fn test_script_engine_apply_defaults_respects_explicit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ExplicitText.wnd");
        std::fs::write(
            &path,
            concat!(
                "TEXTCOLOR=17 17 17 17;\n",
                "FONT=NAME:\"Arial\", SIZE:12, BOLD:0;\n",
                "WINDOW\nWINDOWTYPE=USER;\nNAME=\"ExplicitText\";\n",
                "SCREENRECT=0 0 100 100;\n",
                "TEXTCOLOR=ENABLED:0 0 0 255, ENABLEDBORDER:255 255 255 0, ",
                "DISABLED:17 17 17 17, DISABLEDBORDER:0 0 0 0, ",
                "HILITE:0 0 0 0, HILITEBORDER:255 255 255 0;\n",
                "FONT=NAME:\"Courier\", SIZE:18, BOLD:1;\nEND\n",
            ),
        )
        .unwrap();
        let engine = WindowScriptEngine::new();
        let parsed = engine.load_window_layout(&path).unwrap();
        let def = &parsed.windows[0];
        // Explicit packed zero and transparent white are real authored colors.
        assert_eq!(def.enabled_text.color, 0xFF000000);
        assert_eq!(def.disabled_text.color, 0x11111111);
        assert_eq!(def.hilite_text.color, 0);
        assert_eq!(
            def.enabled_text.border_color,
            super::super::game_window::WIN_COLOR_UNDEFINED
        );
        assert_eq!(def.disabled_text.border_color, 0);
        assert_eq!(
            def.hilite_text.border_color,
            super::super::game_window::WIN_COLOR_UNDEFINED
        );

        let mut manager = super::super::window_manager::WindowManager::new();
        let info = manager
            .create_windows_from_script(path.to_str().unwrap())
            .unwrap();
        let window = info.windows[0].borrow();
        let data = window.instance_data();
        assert_eq!(data.enabled_text.color, 0xFF000000);
        assert_eq!(data.disabled_text.color, 0x11111111);
        assert_eq!(data.hilite_text.color, 0);
        assert_eq!(
            data.enabled_text.border_color,
            super::super::game_window::WIN_COLOR_UNDEFINED
        );
        assert_eq!(data.disabled_text.border_color, 0);
        assert_eq!(
            data.hilite_text.border_color,
            super::super::game_window::WIN_COLOR_UNDEFINED
        );
        let font = data.font.as_ref().unwrap();
        assert_eq!(font.name, "Courier");
        assert_eq!(font.size, 18);
        assert!(font.bold);
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
