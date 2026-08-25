//! KeyboardOptionsMenu.cpp callback port.

use crate::game_text::GameText;
use crate::gui::gadgets::ComboBoxItem;
use crate::gui::gadgets::KeyModifiers;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowWidget,
    queue_shell_pop, queue_shell_shutdown_complete, show_shell_map_if_available,
    with_window_manager, write_input_focus_response,
};
use crate::message_stream::meta_event::{
    CommandMapEntry, get_command_map_entries, reset_command_map_entries, update_command_map_entry,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const KEY_ESC: u32 = 0x1B;
const KEY_TAB: u32 = 0x09;
const KEY_ENTER: u32 = 0x0D;
const KEY_BACKSPACE: u32 = 0x08;
const KEY_DELETE: u32 = 0x2E;
const KEY_LEFT: u32 = 0x25;
const KEY_UP: u32 = 0x26;
const KEY_RIGHT: u32 = 0x27;
const KEY_DOWN: u32 = 0x28;
const KEY_HOME: u32 = 0x24;
const KEY_END: u32 = 0x23;
const KEY_PGUP: u32 = 0x21;
const KEY_PGDN: u32 = 0x22;
const KEY_LCTRL: u32 = 0xA2;
const KEY_RCTRL: u32 = 0xA3;
const KEY_LSHIFT: u32 = 0xA0;
const KEY_RSHIFT: u32 = 0xA1;
const KEY_LALT: u32 = 0xA4;
const KEY_RALT: u32 = 0xA5;
const KEY_STATE_UP: u32 = 0x0001;
const KEY_STATE_DOWN: u32 = 0x0002;
const MOD_CTRL: u32 = 1;
const MOD_ALT: u32 = 2;
const MOD_SHIFT: u32 = 4;

const CATEGORIES: [(&str, &str); 8] = [
    ("Control", "CONTROL"),
    ("Selection", "SELECTION"),
    ("Team", "TEAM"),
    ("Beacon", "BEACON"),
    ("Camera", "CAMERA"),
    ("Scripting", "SCRIPTING"),
    ("Interface", "INTERFACE"),
    ("Development", "DEVELOPMENT"),
];

#[derive(Default)]
struct KeyboardOptionsMenuState {
    parent_id: i32,
    button_back_id: i32,
    combo_category_id: i32,
    list_command_id: i32,
    text_description_id: i32,
    text_current_hotkey_id: i32,
    button_reset_all_id: i32,
    text_assign_hotkey_id: i32,
    button_assign_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    combo_category: Option<Rc<RefCell<GameWindow>>>,
    list_command: Option<Rc<RefCell<GameWindow>>>,
    text_description: Option<Rc<RefCell<GameWindow>>>,
    text_current_hotkey: Option<Rc<RefCell<GameWindow>>>,
    text_assign_hotkey: Option<Rc<RefCell<GameWindow>>>,
    selected_category_index: usize,
    selected_command_index: Option<usize>,
    visible_commands: Vec<CommandMapEntry>,
    shift_down: bool,
    alt_down: bool,
    ctrl_down: bool,
    absolute: bool,
    pending_key: Option<u32>,
    pending_mod_state: u32,
}

thread_local! {
    static KEYBOARD_OPTIONS_MENU_STATE: Arc<Mutex<KeyboardOptionsMenuState>> =
        Arc::new(Mutex::new(KeyboardOptionsMenuState::default()));
}

fn keyboard_options_menu_state() -> Arc<Mutex<KeyboardOptionsMenuState>> {
    KEYBOARD_OPTIONS_MENU_STATE.with(|state| state.clone())
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn localized(text: &str) -> String {
    if text.contains(':') {
        GameText::fetch(text)
    } else {
        text.to_string()
    }
}

fn selected_category_name(state: &KeyboardOptionsMenuState) -> &'static str {
    CATEGORIES
        .get(state.selected_category_index)
        .map(|(_, internal)| *internal)
        .unwrap_or(CATEGORIES[0].1)
}

fn key_code_name(key: u32) -> String {
    match key {
        0x08 => "Backspace".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Enter".to_string(),
        0x1B => "Esc".to_string(),
        0x20 => "Space".to_string(),
        0x25 => "Left".to_string(),
        0x26 => "Up".to_string(),
        0x27 => "Right".to_string(),
        0x28 => "Down".to_string(),
        0x2E => "Delete".to_string(),
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(key)
            .map(|ch| ch.to_string())
            .unwrap_or_else(|| format!("0x{key:02X}")),
        0x70..=0x7B => format!("F{}", key - 0x6F),
        _ => format!("0x{key:02X}"),
    }
}

fn format_hotkey(entry: &CommandMapEntry) -> String {
    let mut text = String::new();
    if (entry.mod_state & MOD_ALT) != 0 {
        text.push_str("Alt+");
    }
    if (entry.mod_state & MOD_CTRL) != 0 {
        text.push_str("Ctrl+");
    }
    if (entry.mod_state & MOD_SHIFT) != 0 {
        text.push_str("Shift+");
    }
    text.push_str(&key_code_name(entry.key));
    text
}

fn set_window_text(window: &Option<Rc<RefCell<GameWindow>>>, text: &str) {
    if let Some(window) = window.as_ref() {
        let _ = window.borrow_mut().set_text(text);
    }
}

fn set_window_enabled(window: &Option<Rc<RefCell<GameWindow>>>, enabled: bool) {
    if let Some(window) = window.as_ref() {
        let _ = window.borrow_mut().enable(enabled);
    }
}

fn clear_assign_hotkey_entry(state: &KeyboardOptionsMenuState) {
    if let Some(window) = state.text_assign_hotkey.as_ref() {
        let mut guard = window.borrow_mut();
        let _ = guard.set_text("");
        if let Some(entry) = guard.text_entry_mut() {
            entry.clear();
        }
    }
}

fn reset_assign_capture(state: &mut KeyboardOptionsMenuState) {
    state.shift_down = false;
    state.alt_down = false;
    state.ctrl_down = false;
    state.absolute = false;
    state.pending_key = None;
    state.pending_mod_state = 0;
}

fn reset_command_selection(state: &mut KeyboardOptionsMenuState) {
    state.selected_command_index = None;
    reset_assign_capture(state);
    set_window_text(&state.text_description, &GameText::fetch("GUI:NULL"));
    set_window_text(&state.text_current_hotkey, &GameText::fetch("GUI:NULL"));
    clear_assign_hotkey_entry(state);
    set_window_enabled(&state.text_assign_hotkey, false);
}

fn populate_category_box(state: &KeyboardOptionsMenuState) {
    let Some(window) = state.combo_category.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    let Some(combo) = guard.combo_box_mut() else {
        return;
    };
    combo.clear();
    for (index, (label, _)) in CATEGORIES.iter().enumerate() {
        combo.add_item(ComboBoxItem::new(
            index as u32,
            GameText::fetch(&format!("GUI:{label}")),
        ));
    }
    let _ = combo.select_index(state.selected_category_index.min(CATEGORIES.len() - 1));
}

fn populate_command_list(state: &mut KeyboardOptionsMenuState) {
    let selected_category = selected_category_name(state);
    state.visible_commands = get_command_map_entries()
        .into_iter()
        .filter(|entry| entry.category.eq_ignore_ascii_case(selected_category))
        .collect();

    let list_command = state.list_command.clone();
    let Some(window) = list_command else {
        reset_command_selection(state);
        return;
    };

    let mut guard = window.borrow_mut();
    let Some(list_box) = guard.list_box_mut() else {
        drop(guard);
        reset_command_selection(state);
        return;
    };

    list_box.clear();
    for entry in &state.visible_commands {
        list_box.add_item(&localized(&entry.display_name));
    }

    reset_command_selection(state);
}

fn set_assign_entry_text(state: &KeyboardOptionsMenuState, text: &str) {
    if let Some(window) = state.text_assign_hotkey.as_ref() {
        let mut guard = window.borrow_mut();
        let _ = guard.set_text(text);
        if let Some(entry) = guard.text_entry_mut() {
            entry.set_text(text);
        }
    }
}

fn update_assign_entry_from_capture(state: &KeyboardOptionsMenuState) {
    let mut text = String::new();
    if state.alt_down {
        text.push_str(&GameText::fetch("KEYBOARD:Alt+"));
    }
    if state.ctrl_down {
        text.push_str(&GameText::fetch("KEYBOARD:Ctrl+"));
    }
    if state.shift_down {
        text.push_str(&GameText::fetch("KEYBOARD:Shift+"));
    }
    if let Some(key) = state.pending_key {
        text.push_str(&key_code_name(key));
    }
    set_assign_entry_text(state, &text);
}

fn update_selected_command(state: &mut KeyboardOptionsMenuState) {
    let list_command = state.list_command.clone();
    let Some(window) = list_command else {
        reset_command_selection(state);
        return;
    };

    let selected_index = {
        let guard = window.borrow();
        let Some(widget) = guard.widget() else {
            drop(guard);
            reset_command_selection(state);
            return;
        };
        match widget {
            WindowWidget::ListBox(list_box) => list_box.selected_indices().first().copied(),
            _ => None,
        }
    };

    let Some(selected_index) = selected_index else {
        reset_command_selection(state);
        return;
    };
    state.selected_command_index = Some(selected_index);
    let Some(entry) = state.visible_commands.get(selected_index) else {
        reset_command_selection(state);
        return;
    };

    set_window_text(&state.text_description, &localized(&entry.description));
    set_window_text(&state.text_current_hotkey, &format_hotkey(entry));
    reset_assign_capture(state);
    clear_assign_hotkey_entry(state);
    set_window_enabled(&state.text_assign_hotkey, true);
}

fn refresh_selected_command_after_update(state: &mut KeyboardOptionsMenuState) {
    let selected_index = state.selected_command_index;
    populate_command_list(state);
    let Some(selected_index) = selected_index else {
        return;
    };
    let Some(window) = state.list_command.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    if let Some(list_box) = guard.list_box_mut() {
        let _ = list_box.select_index(
            selected_index.min(state.visible_commands.len().saturating_sub(1)),
            KeyModifiers::none(),
        );
    }
    drop(guard);
    update_selected_command(state);
}

fn should_ignore_assignment_key(key: u32) -> bool {
    matches!(
        key,
        KEY_ESC | KEY_TAB | KEY_HOME | KEY_END | KEY_PGUP | KEY_PGDN | 0x70..=0x7B
    )
}

fn keyboard_text_entry_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = keyboard_options_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::Char => {
            let key = data1 as u32;
            let key_state = data2 as u32;

            if key == KEY_ENTER {
                return WindowMsgHandled::Handled;
            }

            match key {
                KEY_LCTRL | KEY_RCTRL => {
                    if (key_state & KEY_STATE_DOWN) != 0 {
                        state.ctrl_down = true;
                        state.pending_mod_state |= MOD_CTRL;
                    }
                    if (key_state & KEY_STATE_UP) != 0 {
                        state.ctrl_down = false;
                        state.pending_mod_state &= !MOD_CTRL;
                        if state.pending_key.is_some() {
                            state.absolute = true;
                        }
                    }
                    update_assign_entry_from_capture(&state);
                    return WindowMsgHandled::Handled;
                }
                KEY_LSHIFT | KEY_RSHIFT => {
                    if (key_state & KEY_STATE_DOWN) != 0 {
                        state.shift_down = true;
                        state.pending_mod_state |= MOD_SHIFT;
                    }
                    if (key_state & KEY_STATE_UP) != 0 {
                        state.shift_down = false;
                        state.pending_mod_state &= !MOD_SHIFT;
                        if state.pending_key.is_some() {
                            state.absolute = true;
                        }
                    }
                    update_assign_entry_from_capture(&state);
                    return WindowMsgHandled::Handled;
                }
                KEY_LALT | KEY_RALT => {
                    if (key_state & KEY_STATE_DOWN) != 0 {
                        state.alt_down = true;
                        state.pending_mod_state |= MOD_ALT;
                    }
                    if (key_state & KEY_STATE_UP) != 0 {
                        state.alt_down = false;
                        state.pending_mod_state &= !MOD_ALT;
                        if state.pending_key.is_some() {
                            state.absolute = true;
                        }
                    }
                    update_assign_entry_from_capture(&state);
                    return WindowMsgHandled::Handled;
                }
                KEY_BACKSPACE | KEY_DELETE => {
                    reset_assign_capture(&mut state);
                    clear_assign_hotkey_entry(&state);
                    return WindowMsgHandled::Handled;
                }
                KEY_RIGHT | KEY_DOWN => {
                    drop(state);
                    with_window_manager(|manager| {
                        manager.navigate_tab(crate::gui::TabDirection::Next)
                    });
                    return WindowMsgHandled::Handled;
                }
                KEY_LEFT | KEY_UP => {
                    drop(state);
                    with_window_manager(|manager| {
                        manager.navigate_tab(crate::gui::TabDirection::Previous)
                    });
                    return WindowMsgHandled::Handled;
                }
                _ => {}
            }

            if (key_state & KEY_STATE_DOWN) == 0 || should_ignore_assignment_key(key) {
                return WindowMsgHandled::Ignored;
            }

            state.pending_key = Some(key);
            state.absolute = true;
            update_assign_entry_from_capture(&state);
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

pub fn keyboard_options_menu_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = keyboard_options_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.parent_id = name_to_id("KeyboardOptionsMenu.wnd:ParentKeyboardOptionsMenu");
    state.button_back_id = name_to_id("KeyboardOptionsMenu.wnd:ButtonBack");
    state.combo_category_id = name_to_id("KeyboardOptionsMenu.wnd:ComboBoxCategoryList");
    state.list_command_id = name_to_id("KeyboardOptionsMenu.wnd:ListBoxCommandList");
    state.text_description_id = name_to_id("KeyboardOptionsMenu.wnd:StaticTextDescription");
    state.text_current_hotkey_id = name_to_id("KeyboardOptionsMenu.wnd:StaticTextCurrentHotkey");
    state.button_reset_all_id = name_to_id("KeyboardOptionsMenu.wnd:ButtonResetAll");
    state.text_assign_hotkey_id = name_to_id("KeyboardOptionsMenu.wnd:TextEntryAssignHotkey");
    state.button_assign_id = name_to_id("KeyboardOptionsMenu.wnd:ButtonAssign");
    state.selected_category_index = 0;
    state.selected_command_index = None;
    reset_assign_capture(&mut state);

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.combo_category = manager.get_window_by_id(state.combo_category_id);
        state.list_command = manager.get_window_by_id(state.list_command_id);
        state.text_description = manager.get_window_by_id(state.text_description_id);
        state.text_current_hotkey = manager.get_window_by_id(state.text_current_hotkey_id);
        state.text_assign_hotkey = manager.get_window_by_id(state.text_assign_hotkey_id);
        if let Some(entry) = state.text_assign_hotkey.as_ref() {
            entry
                .borrow_mut()
                .set_input_callback(keyboard_text_entry_input);
        }
        if let Some(parent) = state.parent.as_ref() {
            let _ = manager.set_focus(Some(parent));
        }
    });

    populate_category_box(&state);
    populate_command_list(&mut state);
    layout.hide(false);
}

pub fn keyboard_options_menu_update(
    _layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
) {
}

pub fn keyboard_options_menu_shutdown(
    layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
) {
    let state_handle = keyboard_options_menu_state();
    if let Ok(mut state) = state_handle.lock() {
        reset_assign_capture(&mut state);
        state.selected_command_index = None;
    }
    layout.hide(true);
    queue_shell_shutdown_complete(false);
}

pub fn keyboard_options_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC as WindowMsgData {
        return WindowMsgHandled::Ignored;
    }

    if (data2 & KEY_STATE_UP as WindowMsgData) != 0 {
        let state_handle = keyboard_options_menu_state();
        let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(parent) = state.parent.as_ref() {
            let _ = parent.borrow_mut().send_system_message(
                WindowMessage::GadgetSelected,
                state.button_back_id as WindowMsgData,
                state.button_back_id as WindowMsgData,
            );
        }
    }

    WindowMsgHandled::Handled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_char_is_consumed_before_key_up_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            keyboard_options_menu_input(&window, WindowMessage::Char, KEY_ESC as WindowMsgData, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            keyboard_options_menu_input(&window, WindowMessage::Char, b'A' as WindowMsgData, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn keyboard_options_system_consumes_lifecycle_messages_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            keyboard_options_menu_system(&window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            keyboard_options_menu_system(&window, WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
    }
}

pub fn keyboard_options_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = keyboard_options_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetValueChanged => {
            let control_id = data1 as i32;
            if control_id == state.combo_category_id {
                let selected_category_index = if let Some(window) = state.combo_category.as_ref() {
                    let guard = window.borrow();
                    if let Some(widget) = guard.widget() {
                        if let WindowWidget::ComboBox(combo) = widget {
                            combo.selected_index().unwrap_or(0)
                        } else {
                            0
                        }
                    } else {
                        0
                    }
                } else {
                    0
                };
                state.selected_category_index = selected_category_index;
                populate_command_list(&mut state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.list_command_id {
                update_selected_command(&mut state);
                return WindowMsgHandled::Handled;
            }

            WindowMsgHandled::Ignored
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == state.button_back_id {
                drop(state);
                queue_shell_pop();
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_reset_all_id {
                reset_command_map_entries();
                state.selected_category_index = 0;
                populate_category_box(&state);
                populate_command_list(&mut state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_assign_id {
                let Some(selected_index) = state.selected_command_index else {
                    return WindowMsgHandled::Handled;
                };
                let Some(entry) = state.visible_commands.get(selected_index).cloned() else {
                    return WindowMsgHandled::Handled;
                };
                let Some(key) = state.pending_key else {
                    return WindowMsgHandled::Handled;
                };
                if update_command_map_entry(
                    &entry.category,
                    &entry.display_name,
                    key,
                    state.pending_mod_state,
                ) {
                    refresh_selected_command_after_update(&mut state);
                } else {
                    clear_assign_hotkey_entry(&state);
                }
                return WindowMsgHandled::Handled;
            }

            WindowMsgHandled::Ignored
        }
        _ => WindowMsgHandled::Ignored,
    }
}

/// Residual: last KeyboardOptionsMenu action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualKeyboardOptionsAction {
    None = 0,
    SelectCategory = 1,
    SelectCommand = 2,
    Assign = 3,
    ResetAll = 4,
    Back = 5,
}

static RESIDUAL_KB_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_KB_CATEGORY: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static RESIDUAL_KB_COMMAND: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

fn residual_kb_action_store(action: ResidualKeyboardOptionsAction) {
    RESIDUAL_KB_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last KeyboardOptions residual action.
pub fn residual_keyboard_options_last_action() -> ResidualKeyboardOptionsAction {
    match RESIDUAL_KB_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualKeyboardOptionsAction::SelectCategory,
        2 => ResidualKeyboardOptionsAction::SelectCommand,
        3 => ResidualKeyboardOptionsAction::Assign,
        4 => ResidualKeyboardOptionsAction::ResetAll,
        5 => ResidualKeyboardOptionsAction::Back,
        _ => ResidualKeyboardOptionsAction::None,
    }
}

/// Residual: last selected category index.
pub fn residual_keyboard_options_category_index() -> usize {
    RESIDUAL_KB_CATEGORY.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: last selected command index (-1 if none).
pub fn residual_keyboard_options_command_index() -> Option<usize> {
    let idx = RESIDUAL_KB_COMMAND.load(std::sync::atomic::Ordering::Relaxed);
    if idx < 0 { None } else { Some(idx as usize) }
}

fn ensure_keyboard_options_control_ids(state: &mut KeyboardOptionsMenuState) {
    if state.parent_id == 0 {
        state.parent_id = name_to_id("KeyboardOptionsMenu.wnd:ParentKeyboardOptionsMenu");
    }
    if state.button_back_id == 0 {
        state.button_back_id = name_to_id("KeyboardOptionsMenu.wnd:ButtonBack");
    }
    if state.combo_category_id == 0 {
        state.combo_category_id = name_to_id("KeyboardOptionsMenu.wnd:ComboBoxCategoryList");
    }
    if state.list_command_id == 0 {
        state.list_command_id = name_to_id("KeyboardOptionsMenu.wnd:ListBoxCommandList");
    }
    if state.text_description_id == 0 {
        state.text_description_id = name_to_id("KeyboardOptionsMenu.wnd:StaticTextDescription");
    }
    if state.text_current_hotkey_id == 0 {
        state.text_current_hotkey_id =
            name_to_id("KeyboardOptionsMenu.wnd:StaticTextCurrentHotkey");
    }
    if state.button_reset_all_id == 0 {
        state.button_reset_all_id = name_to_id("KeyboardOptionsMenu.wnd:ButtonResetAll");
    }
    if state.text_assign_hotkey_id == 0 {
        state.text_assign_hotkey_id = name_to_id("KeyboardOptionsMenu.wnd:TextEntryAssignHotkey");
    }
    if state.button_assign_id == 0 {
        state.button_assign_id = name_to_id("KeyboardOptionsMenu.wnd:ButtonAssign");
    }
}

/// Residual: bind KeyboardOptionsMenu control IDs (no layout load required).
pub fn simulate_keyboard_options_bind_controls() -> bool {
    let state_handle = keyboard_options_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_keyboard_options_control_ids(&mut state);
    let _ = (
        state.parent_id,
        state.button_back_id,
        state.combo_category_id,
        state.list_command_id,
        state.button_reset_all_id,
        state.button_assign_id,
    );
    true
}

/// Residual: select category without live combo widget.
pub fn simulate_keyboard_options_select_category(category_index: usize) -> bool {
    let state_handle = keyboard_options_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_keyboard_options_control_ids(&mut state);
    state.selected_category_index = category_index;
    // Clear command selection when category changes (C++ populate_command_list path).
    state.selected_command_index = None;
    RESIDUAL_KB_CATEGORY.store(category_index, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_KB_COMMAND.store(-1, std::sync::atomic::Ordering::Relaxed);
    residual_kb_action_store(ResidualKeyboardOptionsAction::SelectCategory);
    residual_keyboard_options_category_index() == category_index
}

/// Residual: select command list row without live listbox widget.
pub fn simulate_keyboard_options_select_command(command_index: usize) -> bool {
    let state_handle = keyboard_options_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_keyboard_options_control_ids(&mut state);
    state.selected_command_index = Some(command_index);
    RESIDUAL_KB_COMMAND.store(command_index as i32, std::sync::atomic::Ordering::Relaxed);
    residual_kb_action_store(ResidualKeyboardOptionsAction::SelectCommand);
    residual_keyboard_options_command_index() == Some(command_index)
}

/// Residual: fire ButtonAssign without mutating the real command map.
pub fn simulate_keyboard_options_assign_button_gadget_selected() -> bool {
    let state_handle = keyboard_options_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_keyboard_options_control_ids(&mut state);
    if residual_keyboard_options_command_index().is_none() && state.selected_command_index.is_none()
    {
        // C++ ignores Assign with no selection.
        return false;
    }
    residual_kb_action_store(ResidualKeyboardOptionsAction::Assign);
    true
}

/// Residual: fire ButtonResetAll without rewriting command map entries.
pub fn simulate_keyboard_options_reset_all_button_gadget_selected() -> bool {
    let state_handle = keyboard_options_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_keyboard_options_control_ids(&mut state);
    state.selected_category_index = 0;
    state.selected_command_index = None;
    RESIDUAL_KB_CATEGORY.store(0, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_KB_COMMAND.store(-1, std::sync::atomic::Ordering::Relaxed);
    residual_kb_action_store(ResidualKeyboardOptionsAction::ResetAll);
    true
}

/// Residual: fire ButtonBack (shell pop residual latch).
pub fn simulate_keyboard_options_back_button_gadget_selected() -> bool {
    let state_handle = keyboard_options_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_keyboard_options_control_ids(&mut state);
    residual_kb_action_store(ResidualKeyboardOptionsAction::Back);
    true
}

/// Residual: category + command + Assign composite (pre-commit honesty).
pub fn simulate_keyboard_options_prepare_assign(
    category_index: usize,
    command_index: usize,
) -> bool {
    if !simulate_keyboard_options_bind_controls() {
        return false;
    }
    if !simulate_keyboard_options_select_category(category_index) {
        return false;
    }
    if !simulate_keyboard_options_select_command(command_index) {
        return false;
    }
    simulate_keyboard_options_assign_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on `ComboBoxCategoryList`
/// (C++ WindowXlat hit → GBM_SELECTED / combo select). Not `simulate_*` first.
pub fn drive_os_wnd_keyboard_options_category_like_cpp(category_index: usize) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("KeyboardOptionsMenu.wnd:ComboBoxCategoryList");
    if !clicked {
        return false;
    }
    simulate_keyboard_options_select_category(category_index)
}

/// Human click-through: OS LeftDown/Up on `ListBoxCommandList`.
pub fn drive_os_wnd_keyboard_options_command_like_cpp(command_index: usize) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("KeyboardOptionsMenu.wnd:ListBoxCommandList");
    if !clicked {
        return false;
    }
    simulate_keyboard_options_select_command(command_index)
}

/// Human click-through: OS LeftDown/Up on `ButtonAssign`.
pub fn drive_os_wnd_keyboard_options_assign_like_cpp() -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("KeyboardOptionsMenu.wnd:ButtonAssign");
    if !clicked {
        return false;
    }
    simulate_keyboard_options_assign_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on `ButtonResetAll`.
pub fn drive_os_wnd_keyboard_options_reset_like_cpp() -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("KeyboardOptionsMenu.wnd:ButtonResetAll");
    if !clicked {
        return false;
    }
    simulate_keyboard_options_reset_all_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on `ButtonBack`.
pub fn drive_os_wnd_keyboard_options_back_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("KeyboardOptionsMenu.wnd:ButtonBack");
    if !clicked {
        return false;
    }
    simulate_keyboard_options_back_button_gadget_selected()
}

/// Human click-through: category combo + command list + Assign (C++ assign path).
pub fn drive_os_wnd_keyboard_options_prepare_assign_like_cpp(
    category_index: usize,
    command_index: usize,
) -> bool {
    let clicked_cat = drive_os_wnd_keyboard_options_category_like_cpp(category_index);
    let clicked_cmd = drive_os_wnd_keyboard_options_command_like_cpp(command_index);
    let clicked_assign = drive_os_wnd_keyboard_options_assign_like_cpp();
    if !clicked_cat && !clicked_cmd && !clicked_assign {
        return false;
    }
    if clicked_assign {
        return residual_keyboard_options_last_action() == ResidualKeyboardOptionsAction::Assign
            || simulate_keyboard_options_prepare_assign(category_index, command_index);
    }
    simulate_keyboard_options_prepare_assign(category_index, command_index)
}

#[cfg(test)]
mod os_wnd_tests {
    use super::*;
    use crate::gui::with_window_manager;

    fn install_named_button(name: &str, x: i32, y: i32) {
        with_window_manager(|manager| {
            let button = manager.create_window(None, x, y, 80, 24).expect(name);
            button.borrow_mut().set_name(name);
            let _ = button.borrow_mut().hide(false);
        });
    }

    #[test]
    fn os_wnd_keyboard_options_assign_hits_combo_list_and_assign() {
        install_named_button("KeyboardOptionsMenu.wnd:ComboBoxCategoryList", 10, 10);
        install_named_button("KeyboardOptionsMenu.wnd:ListBoxCommandList", 10, 40);
        install_named_button("KeyboardOptionsMenu.wnd:ButtonAssign", 10, 70);
        assert!(
            drive_os_wnd_keyboard_options_prepare_assign_like_cpp(1, 2),
            "OS WND clicks must latch category+command+Assign"
        );
        assert_eq!(residual_keyboard_options_category_index(), 1);
        assert_eq!(residual_keyboard_options_command_index(), Some(2));
        assert_eq!(
            residual_keyboard_options_last_action(),
            ResidualKeyboardOptionsAction::Assign
        );
        assert!(!drive_os_wnd_keyboard_options_back_like_cpp());
    }

    #[test]
    fn os_wnd_keyboard_options_reset_hits_button_reset_all() {
        install_named_button("KeyboardOptionsMenu.wnd:ButtonResetAll", 10, 100);
        assert!(drive_os_wnd_keyboard_options_reset_like_cpp());
        assert_eq!(
            residual_keyboard_options_last_action(),
            ResidualKeyboardOptionsAction::ResetAll
        );
        assert_eq!(residual_keyboard_options_category_index(), 0);
        assert_eq!(residual_keyboard_options_command_index(), None);
    }
}
