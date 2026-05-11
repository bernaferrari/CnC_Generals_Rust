//! PopupReplay.cpp callback port.

use crate::game_text::GameText;
use crate::gui::callbacks::score_screen::score_screen_enable_controls;
use crate::gui::{
    message_box_ok, message_box_ok_cancel, with_window_manager, GameWindow, WindowLayout,
    WindowMessage, WindowMsgData, WindowMsgHandled,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::recorder::with_recorder;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;
const SAVE_POPUP_DURATION_MS: u64 = 1000;

#[derive(Default)]
struct PopupReplayState {
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

thread_local! {
    static POPUP_REPLAY_STATE: Arc<Mutex<PopupReplayState>> =
        Arc::new(Mutex::new(PopupReplayState::default()));
}

fn popup_replay_state() -> Arc<Mutex<PopupReplayState>> {
    POPUP_REPLAY_STATE.with(|state| state.clone())
}

fn show_replay_saved_popup(state: &PopupReplayState, show: bool) {
    if let Some(parent) = state.replay_saved_parent.as_ref() {
        let _ = parent.borrow_mut().hide(!show);
    }
}

fn close_save_menu(window: &GameWindow) {
    if let Some(layout) = window.get_layout() {
        layout.borrow_mut().hide(true);
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

fn populate_replay_listbox(state: &mut PopupReplayState) {
    let Some(listbox) = state.listbox_window.as_ref() else {
        return;
    };
    let mut listbox_guard = listbox.borrow_mut();
    let Some(list_box) = listbox_guard.list_box_mut() else {
        return;
    };
    list_box.clear();

    let (replay_dir, ext) = replay_dir_and_ext();
    let mut entries = Vec::new();
    if let Ok(dir) = fs::read_dir(&replay_dir) {
        for entry in dir.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                if format!(".{}", extension).eq_ignore_ascii_case(&ext) {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        entries.push(stem.to_string());
                    }
                }
            }
        }
    }
    entries.sort();

    let last_replay = last_replay_filename();
    if entries.iter().any(|name| name == &last_replay) {
        list_box.add_item(&GameText::fetch("GUI:LastReplay"));
    }

    for entry in entries {
        list_box.add_item(&entry);
    }
}

fn get_selected_listbox_text(state: &PopupReplayState) -> Option<String> {
    let listbox = state.listbox_window.as_ref()?;
    let mut listbox_guard = listbox.borrow_mut();
    let list_box = listbox_guard.list_box_mut()?;
    let selected = list_box.selected_indices().first().copied()?;
    list_box.items().get(selected).map(|item| item.text.clone())
}

fn get_listbox_text_at_row(state: &PopupReplayState, row: usize) -> Option<String> {
    let listbox = state.listbox_window.as_ref()?;
    let mut listbox_guard = listbox.borrow_mut();
    let list_box = listbox_guard.list_box_mut()?;
    list_box.items().get(row).map(|item| item.text.clone())
}

fn save_replay(filename: &str) {
    let state_handle = popup_replay_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    let translated = if filename == GameText::fetch("GUI:LastReplay") {
        last_replay_filename()
    } else {
        filename.to_string()
    };

    let (replay_dir, ext) = replay_dir_and_ext();
    let full_path = replay_dir.join(format!("{}{}", translated, ext));
    state.replay_path = full_path.to_string_lossy().to_string();
    state.message_box_window = None;

    if full_path.exists() {
        let ok = Box::new(|| {
            really_save_replay();
        });
        state.message_box_window = message_box_ok_cancel(
            &GameText::fetch("GUI:OverwriteReplayTitle"),
            &GameText::fetch("GUI:OverwriteReplay"),
            Some(ok),
            None,
        );
    } else {
        really_save_replay();
    }
}

fn really_save_replay() {
    let state_handle = popup_replay_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    let filename = state.replay_path.clone();
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
            state.message_box_window = None;
            let _ = message_box_ok(&GameText::fetch("GUI:Error"), &err.to_string(), None);
            populate_replay_listbox(&mut state);
            return;
        }
    }

    if let Err(err) = fs::copy(&old_filename, &target) {
        state.message_box_window = None;
        let _ = message_box_ok(&GameText::fetch("GUI:Error"), &err.to_string(), None);
        return;
    }

    populate_replay_listbox(&mut state);
    show_replay_saved_popup(&state, true);
    state.save_popup_start = Some(Instant::now());
}

pub fn popup_replay_init(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = popup_replay_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.button_back = NameKeyGenerator::name_to_key("PopupReplay.wnd:ButtonBack") as i32;
    state.button_save = NameKeyGenerator::name_to_key("PopupReplay.wnd:ButtonSave") as i32;
    state.listbox_games = NameKeyGenerator::name_to_key("PopupReplay.wnd:ListboxGames") as i32;
    state.text_entry_replay_name =
        NameKeyGenerator::name_to_key("PopupReplay.wnd:TextEntryReplayName") as i32;

    let parent_id = NameKeyGenerator::name_to_key("PopupReplay.wnd:PopupReplayMenu") as i32;
    let replay_saved_parent_id =
        NameKeyGenerator::name_to_key("PopupReplay.wnd:PopupReplaySaved") as i32;

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(parent_id);
        state.replay_saved_parent = manager.get_window_by_id(replay_saved_parent_id);
        state.listbox_window = manager.get_window_by_id(state.listbox_games);
        state.text_entry_window = manager.get_window_by_id(state.text_entry_replay_name);
    });

    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(parent));
        });
    }

    show_replay_saved_popup(&state, false);

    if let Some(parent) = state.parent.as_ref() {
        if let Some(frame) = parent
            .borrow()
            .find_child_by_id(
                NameKeyGenerator::name_to_key("PopupReplay.wnd:MenuButtonFrame") as i32,
            )
        {
            let _ = frame.borrow_mut().enable(true);
        }
    }

    populate_replay_listbox(&mut state);

    if let Some(entry) = state.text_entry_window.as_ref() {
        if let Some(widget) = entry.borrow_mut().text_entry_mut() {
            widget.set_text("");
        }
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(entry));
        });
    }

    with_window_manager(|manager| {
        if let Some(control) = manager.get_window_by_id(state.button_save) {
            let _ = control.borrow_mut().enable(false);
        }
    });
}

pub fn popup_replay_shutdown(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = popup_replay_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.parent = None;
    state.replay_saved_parent = None;
    state.listbox_window = None;
    state.text_entry_window = None;
    state.save_popup_start = None;
    state.message_box_window = None;
}

pub fn popup_replay_update(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = popup_replay_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(start) = state.save_popup_start {
        if start.elapsed() >= Duration::from_millis(SAVE_POPUP_DURATION_MS) {
            show_replay_saved_popup(&state, false);
            if let Some(parent) = state.parent.as_ref() {
                let parent_ref = parent.borrow();
                close_save_menu(&parent_ref);
            }
            score_screen_enable_controls(true);
            state.save_popup_start = None;
        }
    }

    if let Some(entry) = state.text_entry_window.as_ref() {
        let mut enabled = false;
        if let Some(widget) = entry.borrow_mut().text_entry_mut() {
            enabled = !widget.text().is_empty();
        }
        with_window_manager(|manager| {
            if let Some(control) = manager.get_window_by_id(state.button_save) {
                let _ = control.borrow_mut().enable(enabled);
            }
        });
    }
}

pub fn popup_replay_input(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if (data2 & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    let state_handle = popup_replay_state();
    let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            state.button_back as u32,
            state.button_back as u32,
        );
    }
    WindowMsgHandled::Handled
}

pub fn popup_replay_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = popup_replay_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == state.listbox_games {
                let row_selected = data2 as i32;
                if row_selected >= 0 {
                    if let Some(filename) = get_listbox_text_at_row(&state, row_selected as usize) {
                        if let Some(entry) = state.text_entry_window.as_ref() {
                            if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                                widget.set_text(filename);
                            }
                        }
                    }
                } else if let Some(filename) = get_selected_listbox_text(&state) {
                    if let Some(entry) = state.text_entry_window.as_ref() {
                        if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                            widget.set_text(filename);
                        }
                    }
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_save {
                if let Some(entry) = state.text_entry_window.as_ref() {
                    if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                        let filename = widget.text().to_string();
                        if !filename.is_empty() {
                            save_replay(&filename);
                        }
                    }
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_back {
                close_save_menu(window);
                score_screen_enable_controls(true);
                return WindowMsgHandled::Handled;
            }

            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => {
            let control_id = data1 as i32;
            if control_id == state.text_entry_replay_name {
                if let Some(entry) = state.text_entry_window.as_ref() {
                    if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                        let filename = widget.text().to_string();
                        if !filename.is_empty() {
                            save_replay(&filename);
                        }
                    }
                }
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
