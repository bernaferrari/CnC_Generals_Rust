//! KeyboardOptionsMenu.cpp callback port.

use crate::game_text::GameText;
use crate::gui::gadgets::ComboBoxItem;
use crate::gui::gadgets::KeyModifiers;
use crate::gui::{
    get_shell, with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, WindowWidget,
};
use crate::message_stream::meta_event::{
    get_command_map_entries, reset_command_map_entries, update_command_map_entry, CommandMapEntry,
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
    let mut state = state_handle
        .lock()
        .expect("keyboard options menu state lock poisoned");

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
    let mut state = state_handle
        .lock()
        .expect("keyboard options menu state lock poisoned");

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
    let _ = get_shell().shutdown_complete(None, false);
}

pub fn keyboard_options_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char && data1 == KEY_ESC && (data2 & KEY_STATE_UP) != 0 {
        let _ = get_shell().pop();
        return WindowMsgHandled::Handled;
    }

    WindowMsgHandled::Ignored
}

pub fn keyboard_options_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = keyboard_options_menu_state();
    let mut state = state_handle
        .lock()
        .expect("keyboard options menu state lock poisoned");

    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
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
                let _ = get_shell().pop();
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
