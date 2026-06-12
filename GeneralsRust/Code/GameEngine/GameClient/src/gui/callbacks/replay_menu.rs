//! ReplayMenu.cpp callback port.

use crate::game_text::GameText;
use crate::gui::shell::replay_menu::ReplayMenu as ShellReplayMenu;
use crate::gui::{
    get_shell, message_box_ok, message_box_ok_cancel, message_box_yes_no, with_window_manager,
    write_input_focus_response, Color as WindowColor, GameWindow, KeyModifiers, WindowLayout,
    WindowMessage, WindowMsgData, WindowMsgHandled, GLM_DOUBLE_CLICKED,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::recorder::{init_recorder, with_recorder, with_recorder_mut};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

struct ReplayMenuState {
    parent_id: i32,
    gadget_parent_id: i32,
    button_load_id: i32,
    button_back_id: i32,
    button_delete_id: i32,
    button_copy_id: i32,
    listbox_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    listbox_window: Option<Rc<RefCell<GameWindow>>>,
    menu: ShellReplayMenu,
    is_shutting_down: bool,
    initial_gadget_delay: i32,
    just_entered: bool,
}

impl ReplayMenuState {
    fn new() -> Self {
        let (replay_dir, replay_ext) = replay_dir_and_ext();
        Self {
            parent_id: 0,
            gadget_parent_id: 0,
            button_load_id: 0,
            button_back_id: 0,
            button_delete_id: 0,
            button_copy_id: 0,
            listbox_id: 0,
            parent: None,
            listbox_window: None,
            menu: ShellReplayMenu::new(replay_dir, replay_ext),
            is_shutting_down: false,
            initial_gadget_delay: 2,
            just_entered: false,
        }
    }
}

thread_local! {
    static REPLAY_MENU_STATE: Arc<Mutex<ReplayMenuState>> =
        Arc::new(Mutex::new(ReplayMenuState::new()));
}

fn replay_menu_state() -> Arc<Mutex<ReplayMenuState>> {
    REPLAY_MENU_STATE.with(|state| state.clone())
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn replay_dir_and_ext() -> (PathBuf, String) {
    init_recorder();
    with_recorder(|recorder| {
        (
            recorder.replay_dir(),
            recorder.replay_extension().to_string(),
        )
    })
    .unwrap_or_else(|| (PathBuf::from("Replays"), ".rep".to_string()))
}

fn sync_selected_index(state: &mut ReplayMenuState) {
    let Some(listbox) = state.listbox_window.as_ref() else {
        return;
    };
    let mut guard = listbox.borrow_mut();
    let Some(widget) = guard.list_box_mut() else {
        return;
    };
    let selected = widget
        .selected_indices()
        .first()
        .copied()
        .map(|idx| idx as i32);
    state.menu.set_selected_index(selected.unwrap_or(-1));
}

fn selected_replay_row_for_double_click(state: &mut ReplayMenuState) -> Option<i32> {
    let selected = state
        .listbox_window
        .as_ref()
        .and_then(|listbox| {
            let guard = listbox.borrow();
            guard.widget().and_then(|widget| match widget {
                crate::gui::WindowWidget::ListBox(listbox) => {
                    listbox.selected_indices().first().copied()
                }
                _ => None,
            })
        })
        .map(|idx| idx as i32);
    if let Some(row) = selected {
        state.menu.set_selected_index(row);
    }
    selected
}

fn populate_replay_listbox(state: &mut ReplayMenuState) {
    let Some(listbox) = state.listbox_window.as_ref() else {
        return;
    };
    let mut guard = listbox.borrow_mut();
    let Some(widget) = guard.list_box_mut() else {
        return;
    };

    widget.clear();
    for entry in state.menu.get_replay_list() {
        let color = WindowColor::new(entry.color.r, entry.color.g, entry.color.b, entry.color.a);
        let row = widget.add_item_with_data_and_color(0, &entry.name, None, Some(color));
        let _ = widget.set_item_column_data(
            row,
            1,
            crate::gui::gadgets::ListBoxItemData::Text(entry.date.clone()),
        );
        let _ = widget.set_item_column_color(row, 1, Some(color));
        let _ = widget.set_item_column_data(
            row,
            2,
            crate::gui::gadgets::ListBoxItemData::Text(entry.version.clone()),
        );
        let _ = widget.set_item_column_color(row, 2, Some(color));
        let _ = widget.set_item_column_data(
            row,
            3,
            crate::gui::gadgets::ListBoxItemData::Text(entry.map.clone()),
        );
        let _ = widget.set_item_column_color(row, 3, Some(color));
    }

    let selected = state.menu.get_selected_index();
    if selected >= 0 {
        let _ = widget.select_index(selected as usize, KeyModifiers::none());
    } else if !state.menu.get_replay_list().is_empty() {
        let _ = widget.select_index(0, KeyModifiers::none());
        state.menu.set_selected_index(0);
    }
}

fn hide_parent_menu() {
    let state_handle = replay_menu_state();
    let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().hide(true);
    }
}

fn playback_replay_row_direct(row_selected: i32) {
    if row_selected < 0 {
        return;
    }

    let state_handle = replay_menu_state();
    let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    let filename = state.menu.get_replay_filename_from_listbox(row_selected);
    drop(state);

    init_recorder();
    if let Some(Ok(true)) = with_recorder_mut(|recorder| recorder.playback_file(filename)) {
        hide_parent_menu();
    }
}

fn playback_selected_replay(ignore_version: bool) {
    let state_handle = replay_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    sync_selected_index(&mut state);

    let selected = state.menu.get_selected_index();
    if selected < 0 {
        let _ = message_box_ok(
            &GameText::fetch("GUI:NoFileSelected"),
            &GameText::fetch("GUI:PleaseSelectAFile"),
            None,
        );
        return;
    }

    if ignore_version {
        let filename = state.menu.get_replay_filename_from_listbox(selected);
        drop(state);
        init_recorder();
        if let Some(Ok(true)) =
            with_recorder_mut(|recorder| recorder.playback_file(filename.clone()))
        {
            hide_parent_menu();
        }
        return;
    }

    match state.menu.load_replay() {
        Ok(()) => {
            drop(state);
            hide_parent_menu();
        }
        Err(err) if err == "GUI:OlderReplayVersion" => {
            let ok = Box::new(|| playback_selected_replay(true));
            drop(state);
            let _ = message_box_ok_cancel(
                &GameText::fetch("GUI:OlderReplayVersionTitle"),
                &GameText::fetch("GUI:OlderReplayVersion"),
                Some(ok),
                None,
            );
        }
        Err(err) if err == "GUI:NoFileSelected" || err == "GUI:PleaseSelectAFile" => {
            drop(state);
            let _ = message_box_ok(
                &GameText::fetch("GUI:NoFileSelected"),
                &GameText::fetch("GUI:PleaseSelectAFile"),
                None,
            );
        }
        Err(err) => {
            drop(state);
            let _ = message_box_ok(&GameText::fetch("GUI:Error"), &err, None);
        }
    }
}

fn confirm_delete_replay() {
    let state_handle = replay_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    sync_selected_index(&mut state);
    if state.menu.get_selected_index() < 0 {
        drop(state);
        let _ = message_box_ok(
            &GameText::fetch("GUI:NoFileSelected"),
            &GameText::fetch("GUI:PleaseSelectAFile"),
            None,
        );
        return;
    }
    drop(state);
    let yes = Box::new(|| {
        let state_handle = replay_menu_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        state.menu.delete_replay();
        populate_replay_listbox(&mut state);
    });
    let _ = message_box_yes_no(
        &GameText::fetch("GUI:DeleteFile"),
        &GameText::fetch("GUI:AreYouSureDelete"),
        Some(yes),
        None,
    );
}

fn confirm_copy_replay() {
    let state_handle = replay_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    sync_selected_index(&mut state);
    if state.menu.get_selected_index() < 0 {
        drop(state);
        let _ = message_box_ok(
            &GameText::fetch("GUI:NoFileSelected"),
            &GameText::fetch("GUI:PleaseSelectAFile"),
            None,
        );
        return;
    }
    drop(state);
    let yes = Box::new(|| {
        let state_handle = replay_menu_state();
        let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        state.menu.copy_replay();
        populate_replay_listbox(&mut state);
    });
    let _ = message_box_yes_no(
        &GameText::fetch("GUI:CopyReplay"),
        &GameText::fetch("GUI:AreYouSureCopy"),
        Some(yes),
        None,
    );
}

pub fn replay_menu_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = replay_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.parent_id = name_to_id("ReplayMenu.wnd:ParentReplayMenu");
    state.gadget_parent_id = name_to_id("ReplayMenu.wnd:GadgetParent");
    state.button_load_id = name_to_id("ReplayMenu.wnd:ButtonLoadReplay");
    state.button_back_id = name_to_id("ReplayMenu.wnd:ButtonBack");
    state.button_delete_id = name_to_id("ReplayMenu.wnd:ButtonDeleteReplay");
    state.button_copy_id = name_to_id("ReplayMenu.wnd:ButtonCopyReplay");
    state.listbox_id = name_to_id("ReplayMenu.wnd:ListboxReplayFiles");
    state.menu = ShellReplayMenu::new(replay_dir_and_ext().0, replay_dir_and_ext().1);
    state.menu.init();
    state.is_shutting_down = false;
    state.just_entered = true;
    state.initial_gadget_delay = 2;

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.listbox_window = manager.get_window_by_id(state.listbox_id);
        if let Some(parent) = state.parent.as_ref() {
            let _ = manager.set_focus(Some(parent));
        }
        if let Some(gadget_parent) = manager.get_window_by_id(state.gadget_parent_id) {
            let _ = gadget_parent.borrow_mut().hide(true);
        }
    });

    populate_replay_listbox(&mut state);
    get_shell().show_shell_map(true);
    layout.hide(false);
}

pub fn replay_menu_shutdown(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);

    if pop_immediate {
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
        return;
    }

    let state_handle = replay_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.menu.shutdown(false);
    state.is_shutting_down = true;
}

pub fn replay_menu_update(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = replay_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if state.just_entered {
        if state.initial_gadget_delay == 1 {
            with_window_manager(|manager| {
                manager.transition_remove("MainMenuDefaultMenuLogoFade", false);
                manager.transition_set_group("ReplayMenuFade", false);
            });
            state.initial_gadget_delay = 2;
            state.just_entered = false;
        } else {
            state.initial_gadget_delay -= 1;
        }
    }

    if state.is_shutting_down
        && get_shell().is_anim_finished()
        && with_window_manager(|manager| manager.transitions_finished())
    {
        state.is_shutting_down = false;
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
    }
}

pub fn replay_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = replay_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::InputFocus => write_input_focus_response(data1, _data2, true),
        WindowMessage::GadgetSelected | WindowMessage::GadgetValueChanged => {
            let control_id = data1 as i32;
            if control_id == state.listbox_id {
                sync_selected_index(&mut state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_load_id {
                drop(state);
                playback_selected_replay(false);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_back_id {
                drop(state);
                let _ = get_shell().pop();
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_delete_id {
                drop(state);
                confirm_delete_replay();
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_copy_id {
                drop(state);
                confirm_copy_replay();
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::User(code) if code == GLM_DOUBLE_CLICKED => {
            if data1 as i32 == state.listbox_id {
                if let Some(row_selected) = selected_replay_row_for_double_click(&mut state) {
                    drop(state);
                    playback_replay_row_direct(row_selected);
                }
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Ignored
        }
        _ => WindowMsgHandled::Ignored,
    }
}

pub fn replay_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }

    if (data2 & KEY_STATE_UP) != 0 {
        let state_handle = replay_menu_state();
        let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(parent) = state.parent.as_ref() {
            let _ = parent.borrow_mut().send_system_message(
                WindowMessage::GadgetSelected,
                state.button_back_id as WindowMsgData,
                0,
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
            replay_menu_input(&window, WindowMessage::Char, KEY_ESC as WindowMsgData, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            replay_menu_input(&window, WindowMessage::Char, b'A' as WindowMsgData, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn replay_double_click_uses_current_listbox_selection_like_cpp() {
        let mut state = ReplayMenuState::new();
        state.listbox_id = 42;

        let mut listbox = crate::gui::gadgets::ListBox::new(42, 0, 0, 200, 80);
        listbox.add_item("first");
        listbox.add_item("second");
        assert!(listbox.select_index(1, KeyModifiers::none()));

        let listbox_window = Rc::new(RefCell::new(GameWindow::new()));
        listbox_window
            .borrow_mut()
            .set_widget(crate::gui::WindowWidget::ListBox(listbox));
        state.listbox_window = Some(listbox_window.clone());

        assert_eq!(selected_replay_row_for_double_click(&mut state), Some(1));
        let selected = listbox_window
            .borrow()
            .widget()
            .and_then(|widget| match widget {
                crate::gui::WindowWidget::ListBox(listbox) => {
                    listbox.selected_indices().first().copied()
                }
                _ => None,
            });
        assert_eq!(selected, Some(1));
    }

    #[test]
    fn replay_menu_system_handles_glm_double_clicked_like_cpp() {
        let listbox_id = 42;
        let listbox_window = Rc::new(RefCell::new(GameWindow::new()));
        listbox_window
            .borrow_mut()
            .set_widget(crate::gui::WindowWidget::ListBox(
                crate::gui::gadgets::ListBox::new(listbox_id as u32, 0, 0, 200, 80),
            ));

        {
            let state_handle = replay_menu_state();
            let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            *state = ReplayMenuState::new();
            state.listbox_id = listbox_id;
            state.listbox_window = Some(listbox_window);
        }

        let window = GameWindow::new();
        assert_eq!(
            replay_menu_system(
                &window,
                WindowMessage::User(GLM_DOUBLE_CLICKED),
                listbox_id as WindowMsgData,
                (-1isize) as WindowMsgData,
            ),
            WindowMsgHandled::Handled
        );
    }
}
