//! PopupReplay.cpp callback port.

use crate::game_text::GameText;
use crate::gui::callbacks::score_screen::score_screen_enable_controls;
use crate::gui::{
    GLM_SELECTED, GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
    message_box_ok, message_box_ok_cancel, write_input_focus_response,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::recorder::with_recorder;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::time::{Duration, Instant};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const SAVE_POPUP_DURATION_MS: u64 = 1000;

#[derive(Default)]
pub struct PopupReplayState {
    residual_action: u8,
    residual_slot: Option<i32>,
    residual_name: String,
    button_back: i32,
    button_save: i32,
    listbox_games: i32,
    text_entry_replay_name: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    replay_saved_parent: Option<Rc<RefCell<GameWindow>>>,
    listbox_window: Option<Rc<RefCell<GameWindow>>>,
    text_entry_window: Option<Rc<RefCell<GameWindow>>>,
    save_popup_start: Option<Instant>,
    replay_path: String,
    message_box_window: Option<Rc<RefCell<GameWindow>>>,
}

fn show_replay_saved_popup(
    state: &PopupReplayState,
    show: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    if let Some(parent) = state.replay_saved_parent.as_ref() {
        let _ = parent.borrow_mut().hide(!show, receiving_manager);
    }
}

fn close_save_menu(
    window: &mut GameWindow,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) {
    if let Some(layout) = window.get_layout() {
        layout
            .borrow()
            .hide_with_manager_and_selected(receiving_manager, true, Some(window));
    }
}

fn replay_dir_and_ext() -> (std::path::PathBuf, String) {
    with_recorder(|recorder| {
        (
            recorder.replay_dir(),
            recorder.replay_extension().to_string(),
        )
    })
    .unwrap_or_else(|| (std::path::PathBuf::from("Replays"), ".rep".to_string()))
}

fn last_replay_filename() -> String {
    with_recorder(|recorder| recorder.last_replay_filename().to_string())
        .unwrap_or_else(|| "00000000".to_string())
}

#[allow(dead_code)]
fn replay_display_name_for_popup(entry: &str, last_replay: &str) -> String {
    if entry.eq_ignore_ascii_case(last_replay) {
        GameText::fetch("GUI:LastReplay")
    } else {
        entry.to_string()
    }
}

fn populate_replay_listbox(state: &mut PopupReplayState) {
    let Some(listbox) = state.listbox_window.as_ref() else {
        return;
    };
    let mut listbox_guard = listbox.borrow_mut();
    let Some(list_box) = listbox_guard.list_box_mut() else {
        return;
    };
    crate::gui::shell::populate_replay_file_listbox(list_box);
}

fn get_listbox_text_at_row(state: &PopupReplayState, row: usize) -> Option<String> {
    let listbox = state.listbox_window.as_ref()?;
    let mut listbox_guard = listbox.borrow_mut();
    let list_box = listbox_guard.list_box_mut()?;
    list_box.items().get(row).map(|item| item.text.clone())
}

fn copy_selected_replay_to_text_entry(state: &mut PopupReplayState, row_selected: i32) {
    if row_selected < 0 {
        return;
    }

    if let Some(filename) = get_listbox_text_at_row(state, row_selected as usize) {
        if let Some(entry) = state.text_entry_window.as_ref() {
            if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                widget.set_text(filename);
            }
        }
    }
}

fn save_replay(
    filename: &str,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let translated = if filename == GameText::fetch("GUI:LastReplay") {
        last_replay_filename()
    } else {
        filename.to_string()
    };

    let (replay_dir, ext) = replay_dir_and_ext();
    let full_path = replay_dir.join(format!("{}{}", translated, ext));
    {
        let value = full_path.to_string_lossy().to_string();
        effects.popup_replay_mut().replay_path = value
    };
    {
        let value = None;
        effects.popup_replay_mut().message_box_window = value
    };

    if full_path.exists() {
        let ok: crate::gui::callbacks::message_box::MessageBoxFunc = Box::new(|effects| {
            effects.with_manager(|manager, effects| really_save_replay(manager, effects));
        });
        {
            let value = message_box_ok_cancel(
                &GameText::fetch("GUI:OverwriteReplayTitle"),
                &GameText::fetch("GUI:OverwriteReplay"),
                Some(ok),
                None,
                receiving_manager,
                effects,
            );
            effects.popup_replay_mut().message_box_window = value
        };
    } else {
        really_save_replay(receiving_manager, effects);
    }
}

fn really_save_replay(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let filename = effects.popup_replay().replay_path.clone();
    if filename.is_empty() {
        return;
    }

    let (replay_dir, ext) = replay_dir_and_ext();
    let old_filename = replay_dir.join(format!("{}{}", last_replay_filename(), ext));

    if old_filename.to_string_lossy() == filename {
        return;
    }

    let target = std::path::PathBuf::from(&filename);
    if target.exists() {
        if let Err(err) = fs::remove_file(&target) {
            {
                let value = None;
                effects.popup_replay_mut().message_box_window = value
            };
            let _ = message_box_ok(
                &GameText::fetch("GUI:Error"),
                &err.to_string(),
                None,
                receiving_manager,
                effects,
            );
            populate_replay_listbox(effects.popup_replay_mut());
            return;
        }
    }

    if let Err(err) = fs::copy(&old_filename, &target) {
        {
            let value = None;
            effects.popup_replay_mut().message_box_window = value
        };
        let _ = message_box_ok(
            &GameText::fetch("GUI:Error"),
            &err.to_string(),
            None,
            receiving_manager,
            effects,
        );
        return;
    }

    populate_replay_listbox(effects.popup_replay_mut());
    show_replay_saved_popup(effects.popup_replay(), true, receiving_manager);
    {
        let value = Some(Instant::now());
        effects.popup_replay_mut().save_popup_start = value
    };
}

pub fn popup_replay_init(
    _layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    {
        let value = NameKeyGenerator::name_to_key("PopupReplay.wnd:ButtonBack") as i32;
        effects.popup_replay_mut().button_back = value
    };
    {
        let value = NameKeyGenerator::name_to_key("PopupReplay.wnd:ButtonSave") as i32;
        effects.popup_replay_mut().button_save = value
    };
    {
        let value = NameKeyGenerator::name_to_key("PopupReplay.wnd:ListboxGames") as i32;
        effects.popup_replay_mut().listbox_games = value
    };
    {
        let value = NameKeyGenerator::name_to_key("PopupReplay.wnd:TextEntryReplayName") as i32;
        effects.popup_replay_mut().text_entry_replay_name = value
    };

    let parent_id = NameKeyGenerator::name_to_key("PopupReplay.wnd:PopupReplayMenu") as i32;
    let replay_saved_parent_id =
        NameKeyGenerator::name_to_key("PopupReplay.wnd:PopupReplaySaved") as i32;

    (|manager: &mut crate::gui::window_manager::WindowManager| {
        {
            let value = manager.get_window_by_id(parent_id);
            effects.popup_replay_mut().parent = value
        };
        {
            let value = manager.get_window_by_id(replay_saved_parent_id);
            effects.popup_replay_mut().replay_saved_parent = value
        };
        {
            let value = manager.get_window_by_id(effects.popup_replay().listbox_games);
            effects.popup_replay_mut().listbox_window = value
        };
        {
            let value = manager.get_window_by_id(effects.popup_replay().text_entry_replay_name);
            effects.popup_replay_mut().text_entry_window = value
        };
    })(receiving_manager);

    if let Some(parent) = effects.popup_replay().parent.clone().as_ref() {
        (|manager: &mut crate::gui::window_manager::WindowManager| {
            let _ = manager.set_focus(Some(parent), effects);
        })(receiving_manager);
    }

    show_replay_saved_popup(effects.popup_replay(), false, receiving_manager);

    if let Some(parent) = effects.popup_replay().parent.clone().as_ref() {
        if let Some(frame) = parent
            .borrow()
            .find_child_by_id(
                NameKeyGenerator::name_to_key("PopupReplay.wnd:MenuButtonFrame") as i32,
            )
        {
            let _ = frame.borrow_mut().enable(true);
        }
    }

    populate_replay_listbox(effects.popup_replay_mut());

    if let Some(entry) = effects.popup_replay().text_entry_window.clone().as_ref() {
        if let Some(widget) = entry.borrow_mut().text_entry_mut() {
            widget.set_text("");
        }
        (|manager: &mut crate::gui::window_manager::WindowManager| {
            let _ = manager.set_focus(Some(entry), effects);
        })(receiving_manager);
    }

    (|manager: &mut crate::gui::window_manager::WindowManager| {
        if let Some(control) = manager.get_window_by_id(effects.popup_replay().button_save) {
            let _ = control.borrow_mut().enable(false);
        }
    })(receiving_manager);
}

pub fn popup_replay_shutdown(
    _layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    {
        let value = None;
        effects.popup_replay_mut().parent = value
    };
    {
        let value = None;
        effects.popup_replay_mut().replay_saved_parent = value
    };
    {
        let value = None;
        effects.popup_replay_mut().listbox_window = value
    };
    {
        let value = None;
        effects.popup_replay_mut().text_entry_window = value
    };
    {
        let value = None;
        effects.popup_replay_mut().save_popup_start = value
    };
    {
        let value = None;
        effects.popup_replay_mut().message_box_window = value
    };
}

pub fn popup_replay_update(
    _layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let Some(start) = effects.popup_replay().save_popup_start {
        if start.elapsed() >= Duration::from_millis(SAVE_POPUP_DURATION_MS) {
            show_replay_saved_popup(effects.popup_replay(), false, receiving_manager);
            if let Some(parent) = effects.popup_replay().parent.clone().as_ref() {
                let layout = parent.borrow().get_layout();
                if let Some(layout) = layout {
                    layout.borrow_mut().hide(receiving_manager, true);
                }
            }
            score_screen_enable_controls(true, effects);
            {
                let value = None;
                effects.popup_replay_mut().save_popup_start = value
            };
        }
    }

    if let Some(entry) = effects.popup_replay().text_entry_window.clone().as_ref() {
        let mut enabled = false;
        if let Some(widget) = entry.borrow_mut().text_entry_mut() {
            enabled = !widget.text().is_empty();
        }
        (|manager: &mut crate::gui::window_manager::WindowManager| {
            if let Some(control) = manager.get_window_by_id(effects.popup_replay().button_save) {
                let _ = control.borrow_mut().enable(enabled);
            }
        })(receiving_manager);
    }
}

pub fn popup_replay_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if (data2 & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    let button_back = effects.popup_replay().button_back;
    window.send_system_message(
        WindowMessage::GadgetSelected,
        button_back as WindowMsgData,
        0,
        effects,
    );
    WindowMsgHandled::Handled
}

fn popup_replay_filename(effects: &crate::gui::game_window::WindowEventEffects<'_>) -> String {
    let Some(entry) = effects.popup_replay().text_entry_window.as_ref() else {
        return String::new();
    };
    let entry = entry.borrow();
    match entry.widget() {
        Some(crate::gui::WindowWidget::TextEntry(widget)) => widget.text().to_string(),
        _ => String::new(),
    }
}

pub fn popup_replay_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::User(code) if code == GLM_SELECTED => {
            let control_id = data1 as i32;
            if control_id == effects.popup_replay().listbox_games {
                copy_selected_replay_to_text_entry(effects.popup_replay_mut(), data2 as i32);
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == effects.popup_replay().button_save {
                let filename = popup_replay_filename(effects);
                if !filename.is_empty() {
                    save_replay(&filename, receiving_manager, effects);
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == effects.popup_replay().button_back {
                close_save_menu(window, receiving_manager);
                score_screen_enable_controls(true, effects);
                return WindowMsgHandled::Handled;
            }

            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => {
            let control_id = data1 as i32;
            if control_id == effects.popup_replay().text_entry_replay_name {
                let filename = popup_replay_filename(effects);
                if !filename.is_empty() {
                    save_replay(&filename, receiving_manager, effects);
                }
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::WindowWidget;
    use crate::gui::gadgets::{ListBox, TextEntry};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn popup_replay_esc_char_is_consumed_before_key_up_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            popup_replay_input(&window, WindowMessage::Char, KEY_ESC as WindowMsgData, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            popup_replay_input(&window, WindowMessage::Char, b'A' as WindowMsgData, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn popup_replay_system_consumes_lifecycle_messages_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            popup_replay_system(&window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            popup_replay_system(&window, WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
    }

    #[test]
    fn popup_replay_last_replay_replaces_row_label_without_duplicate() {
        let rows = ["00000000", "Custom"]
            .into_iter()
            .map(|entry| replay_display_name_for_popup(entry, "00000000"))
            .collect::<Vec<_>>();

        assert_eq!(
            rows,
            vec![GameText::fetch("GUI:LastReplay"), "Custom".to_string()]
        );
        assert!(!rows.iter().any(|row| row == "00000000"));
    }

    #[test]
    fn popup_replay_system_handles_glm_selected_listbox_like_cpp() {
        let listbox_id = 101;
        let text_entry_id = 102;

        let listbox_window = Rc::new(RefCell::new(GameWindow::new()));
        let text_entry_window = Rc::new(RefCell::new(GameWindow::new()));
        let mut listbox = ListBox::new(listbox_id as u32, 0, 0, 100, 60);
        listbox.add_item("first-replay");
        listbox.add_item("selected-replay");
        listbox.set_selected_indices(&[1]);
        listbox_window
            .borrow_mut()
            .set_widget(WindowWidget::ListBox(listbox));
        text_entry_window
            .borrow_mut()
            .set_widget(WindowWidget::TextEntry(TextEntry::new(
                text_entry_id as u32,
                0,
                0,
                100,
                20,
            )));
        text_entry_window
            .borrow_mut()
            .text_entry_mut()
            .unwrap()
            .set_text("unchanged");

        {
            let state_handle = popup_replay_state();
            let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            *state = PopupReplayState::default();
            state.listbox_games = listbox_id;
            state.text_entry_replay_name = text_entry_id;
            state.listbox_window = Some(listbox_window);
            state.text_entry_window = Some(text_entry_window.clone());
        }

        let window = GameWindow::new();
        assert_eq!(
            popup_replay_system(
                &window,
                WindowMessage::User(GLM_SELECTED),
                listbox_id as WindowMsgData,
                1,
            ),
            WindowMsgHandled::Handled
        );

        assert_eq!(
            text_entry_window
                .borrow_mut()
                .text_entry_mut()
                .unwrap()
                .text(),
            "selected-replay"
        );
    }

    #[test]
    fn popup_replay_negative_listbox_row_keeps_text_entry_like_cpp() {
        let listbox_id = 101;
        let text_entry_id = 102;

        let listbox_window = Rc::new(RefCell::new(GameWindow::new()));
        let text_entry_window = Rc::new(RefCell::new(GameWindow::new()));
        let mut listbox = ListBox::new(listbox_id as u32, 0, 0, 100, 60);
        listbox.add_item("selected-replay");
        listbox.set_selected_indices(&[0]);
        listbox_window
            .borrow_mut()
            .set_widget(WindowWidget::ListBox(listbox));
        text_entry_window
            .borrow_mut()
            .set_widget(WindowWidget::TextEntry(TextEntry::new(
                text_entry_id as u32,
                0,
                0,
                100,
                20,
            )));
        text_entry_window
            .borrow_mut()
            .text_entry_mut()
            .unwrap()
            .set_text("unchanged");

        {
            let state_handle = popup_replay_state();
            let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            *state = PopupReplayState::default();
            state.listbox_games = listbox_id;
            state.text_entry_replay_name = text_entry_id;
            state.listbox_window = Some(listbox_window);
            state.text_entry_window = Some(text_entry_window.clone());
        }

        let window = GameWindow::new();
        assert_eq!(
            popup_replay_system(
                &window,
                WindowMessage::User(GLM_SELECTED),
                listbox_id as WindowMsgData,
                (-1isize) as WindowMsgData,
            ),
            WindowMsgHandled::Handled
        );

        assert_eq!(
            text_entry_window
                .borrow_mut()
                .text_entry_mut()
                .unwrap()
                .text(),
            "unchanged"
        );
    }
}

/// Residual: last PopupReplay action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualPopupReplayAction {
    None = 0,
    SelectSlot = 1,
    Save = 2,
    Back = 3,
    SetName = 4,
}

pub fn residual_popup_replay_last_action(
    effects: &crate::gui::game_window::WindowEventEffects<'_>,
) -> ResidualPopupReplayAction {
    match effects.popup_replay().residual_action {
        1 => ResidualPopupReplayAction::SelectSlot,
        2 => ResidualPopupReplayAction::Save,
        3 => ResidualPopupReplayAction::Back,
        4 => ResidualPopupReplayAction::SetName,
        _ => ResidualPopupReplayAction::None,
    }
}
pub fn residual_popup_replay_selected_slot(
    effects: &crate::gui::game_window::WindowEventEffects<'_>,
) -> Option<i32> {
    effects.popup_replay().residual_slot
}
pub fn residual_popup_replay_name(
    effects: &crate::gui::game_window::WindowEventEffects<'_>,
) -> String {
    effects.popup_replay().residual_name.clone()
}

fn ensure_popup_replay_control_ids(state: &mut PopupReplayState) {
    if state.button_back == 0 {
        state.button_back = NameKeyGenerator::name_to_key("PopupReplay.wnd:ButtonBack") as i32;
    }
    if state.button_save == 0 {
        state.button_save = NameKeyGenerator::name_to_key("PopupReplay.wnd:ButtonSave") as i32;
    }
    if state.listbox_games == 0 {
        state.listbox_games = NameKeyGenerator::name_to_key("PopupReplay.wnd:ListboxGames") as i32;
    }
    if state.text_entry_replay_name == 0 {
        state.text_entry_replay_name =
            NameKeyGenerator::name_to_key("PopupReplay.wnd:TextEntryReplayName") as i32;
    }
}

/// Bind authored control identities in the receiving popup only.
pub fn simulate_popup_replay_bind_controls(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    ensure_popup_replay_control_ids(effects.popup_replay_mut());
    true
}
pub fn simulate_popup_replay_select_slot(
    slot_index: i32,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if slot_index < 0 {
        return false;
    }
    let state = effects.popup_replay_mut();
    ensure_popup_replay_control_ids(state);
    state.residual_slot = Some(slot_index);
    state.residual_action = ResidualPopupReplayAction::SelectSlot as u8;
    true
}
pub fn simulate_popup_replay_set_name(
    name: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if name.is_empty() {
        return false;
    }
    let state = effects.popup_replay_mut();
    ensure_popup_replay_control_ids(state);
    state.residual_name = name.to_owned();
    state.residual_action = ResidualPopupReplayAction::SetName as u8;
    true
}
pub fn simulate_popup_replay_save_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let state = effects.popup_replay_mut();
    ensure_popup_replay_control_ids(state);
    if state.residual_name.is_empty() {
        return false;
    }
    state.residual_action = ResidualPopupReplayAction::Save as u8;
    true
}
pub fn simulate_popup_replay_back_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let state = effects.popup_replay_mut();
    ensure_popup_replay_control_ids(state);
    state.residual_action = ResidualPopupReplayAction::Back as u8;
    state.residual_slot = None;
    state.residual_name.clear();
    true
}
pub fn simulate_popup_replay_prepare_save(
    name: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_popup_replay_bind_controls(effects)
        && simulate_popup_replay_set_name(name, effects)
        && simulate_popup_replay_save_button_gadget_selected(effects)
}
pub fn simulate_popup_replay_prepare_save_from_slot(
    slot_index: i32,
    name: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_popup_replay_bind_controls(effects)
        && simulate_popup_replay_select_slot(slot_index, effects)
        && simulate_popup_replay_set_name(name, effects)
        && simulate_popup_replay_save_button_gadget_selected(effects)
}
