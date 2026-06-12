//! PopupSaveLoad.cpp callback port.

use crate::game_text::GameText;
use crate::gui::callbacks::quit_menu::destroy_quit_menu;
use crate::gui::campaign_manager::get_campaign_manager;
use crate::gui::gadgets::ListBoxItemData;
use crate::gui::menu_flags::{
    get_dont_show_main_menu, get_replay_was_pressed, set_replay_was_pressed,
};
use crate::gui::shell::Color as WindowColor;
use crate::gui::{
    get_shell, with_window_manager, write_input_focus_response, GameWindow, KeyModifiers,
    WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, GLM_DOUBLE_CLICKED,
};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::{
    get_game_state, AvailableGameInfo, SaveCode, SaveFileType, SaveLoadLayoutType, SnapshotType,
};
use gamelogic::helpers::TheGameLogic;
use gamelogic::system::game_logic::GAME_SINGLE_PLAYER;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const DIFFICULTY_NORMAL: i32 = 1;

struct SaveLoadMenuState {
    button_back: i32,
    button_save: i32,
    button_load: i32,
    button_delete: i32,
    listbox_games: i32,
    button_overwrite_cancel: i32,
    button_overwrite_confirm: i32,
    button_load_cancel: i32,
    button_load_confirm: i32,
    button_save_desc_cancel: i32,
    button_save_desc_confirm: i32,
    button_delete_confirm: i32,
    button_delete_cancel: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_frame: Option<Rc<RefCell<GameWindow>>>,
    overwrite_confirm: Option<Rc<RefCell<GameWindow>>>,
    load_confirm: Option<Rc<RefCell<GameWindow>>>,
    save_desc: Option<Rc<RefCell<GameWindow>>>,
    listbox_games_window: Option<Rc<RefCell<GameWindow>>>,
    edit_desc: Option<Rc<RefCell<GameWindow>>>,
    delete_confirm: Option<Rc<RefCell<GameWindow>>>,
    current_layout_type: SaveLoadLayoutType,
    is_popup: bool,
    initial_gadget_delay: i32,
    just_entered: bool,
    is_shutting_down: bool,
}

impl Default for SaveLoadMenuState {
    fn default() -> Self {
        Self {
            button_back: 0,
            button_save: 0,
            button_load: 0,
            button_delete: 0,
            listbox_games: 0,
            button_overwrite_cancel: 0,
            button_overwrite_confirm: 0,
            button_load_cancel: 0,
            button_load_confirm: 0,
            button_save_desc_cancel: 0,
            button_save_desc_confirm: 0,
            button_delete_confirm: 0,
            button_delete_cancel: 0,
            parent: None,
            button_frame: None,
            overwrite_confirm: None,
            load_confirm: None,
            save_desc: None,
            listbox_games_window: None,
            edit_desc: None,
            delete_confirm: None,
            current_layout_type: SaveLoadLayoutType::SaveAndLoad,
            is_popup: false,
            initial_gadget_delay: 0,
            just_entered: false,
            is_shutting_down: false,
        }
    }
}

thread_local! {
    static SAVE_LOAD_MENU_STATE: Arc<Mutex<SaveLoadMenuState>> =
        Arc::new(Mutex::new(SaveLoadMenuState::default()));
}

fn save_load_menu_state() -> Arc<Mutex<SaveLoadMenuState>> {
    SAVE_LOAD_MENU_STATE.with(|state| state.clone())
}

fn init_gadget_ids(state: &mut SaveLoadMenuState, prefix: &str) {
    state.button_back = NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonBack")) as i32;
    state.button_save = NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonSave")) as i32;
    state.button_load = NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonLoad")) as i32;
    state.button_delete = NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonDelete")) as i32;
    state.listbox_games = NameKeyGenerator::name_to_key(&format!("{prefix}:ListboxGames")) as i32;
    state.button_overwrite_cancel =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonOverwriteCancel")) as i32;
    state.button_overwrite_confirm =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonOverwriteConfirm")) as i32;
    state.button_load_cancel =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonLoadCancel")) as i32;
    state.button_load_confirm =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonLoadConfirm")) as i32;
    state.button_save_desc_cancel =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonSaveDescCancel")) as i32;
    state.button_save_desc_confirm =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonSaveDescConfirm")) as i32;
    state.button_delete_confirm =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonDeleteConfirm")) as i32;
    state.button_delete_cancel =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonDeleteCancel")) as i32;
}

fn load_controls(state: &mut SaveLoadMenuState, parent_id: i32, prefix: &str) {
    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(parent_id);
    });

    let parent = state.parent.clone();
    state.button_frame = parent.as_ref().and_then(|parent| {
        parent
            .borrow()
            .find_child_by_id(
                NameKeyGenerator::name_to_key(&format!("{prefix}:MenuButtonFrame")) as i32,
            )
    });
    state.overwrite_confirm = parent.as_ref().and_then(|parent| {
        parent
            .borrow()
            .find_child_by_id(NameKeyGenerator::name_to_key(&format!(
                "{prefix}:OverwriteConfirmParent"
            )) as i32)
    });
    state.load_confirm = parent.as_ref().and_then(|parent| {
        parent
            .borrow()
            .find_child_by_id(
                NameKeyGenerator::name_to_key(&format!("{prefix}:LoadConfirmParent")) as i32,
            )
    });
    state.save_desc = parent.as_ref().and_then(|parent| {
        parent
            .borrow()
            .find_child_by_id(
                NameKeyGenerator::name_to_key(&format!("{prefix}:SaveDescParent")) as i32,
            )
    });
    state.delete_confirm = parent.as_ref().and_then(|parent| {
        parent
            .borrow()
            .find_child_by_id(NameKeyGenerator::name_to_key(&format!(
                "{prefix}:DeleteConfirmParent"
            )) as i32)
    });
    state.edit_desc = parent.as_ref().and_then(|parent| {
        parent
            .borrow()
            .find_child_by_id(NameKeyGenerator::name_to_key(&format!("{prefix}:EntryDesc")) as i32)
    });

    with_window_manager(|manager| {
        state.listbox_games_window = manager.get_window_by_id(state.listbox_games);
    });
}

fn normalize_default_save_description_from_map_name(mut default_desc: String) -> String {
    if let Some(pos) = default_desc.rfind('\\') {
        default_desc = default_desc[pos + 1..].to_string();
    }

    let char_len = default_desc.chars().count();
    if char_len >= 4 && default_desc.chars().nth(char_len - 4) == Some('.') {
        for _ in 0..4 {
            let _ = default_desc.pop();
        }
    }

    default_desc
}

fn set_edit_description(edit_control: &Rc<RefCell<GameWindow>>) {
    let mut default_desc = String::new();
    let mut used_campaign = false;
    {
        let manager = get_campaign_manager();
        if let (Some(campaign), Some(mission_number)) = (
            manager.get_current_campaign(),
            manager.get_current_mission_number(),
        ) {
            let campaign_label = GameText::fetch(&campaign.campaign_name_label);
            let label = if campaign_label.is_empty() {
                campaign.campaign_name_label.clone()
            } else {
                campaign_label
            };
            default_desc = format!("{} {}", label, mission_number + 1);
            used_campaign = true;
        }
    }

    if !used_campaign {
        if let Some(data) = get_global_data() {
            let data = data.read();
            default_desc = data.map_name.clone();
        }
    }

    if default_desc.is_empty() {
        return;
    }

    default_desc = normalize_default_save_description_from_map_name(default_desc);

    if let Some(widget) = edit_control.borrow_mut().text_entry_mut() {
        widget.set_text(default_desc);
    }
}

fn populate_save_game_listbox(state: &mut SaveLoadMenuState) {
    let Some(listbox) = state.listbox_games_window.as_ref() else {
        return;
    };
    let mut listbox_guard = listbox.borrow_mut();
    let Some(list_box) = listbox_guard.list_box_mut() else {
        return;
    };

    list_box.clear();

    if state.current_layout_type != SaveLoadLayoutType::LoadOnly {
        let new_game_text = GameText::fetch("GUI:NewSaveGame");
        let new_game_color = WindowColor::new(200, 200, 255, 255);
        list_box.add_item_with_data_and_color(-1, &new_game_text, None, Some(new_game_color));
    }

    {
        let mut game_state = get_game_state();
        game_state.refresh_available_games();
    }

    let game_state = get_game_state();
    for (index, info) in game_state.available_games().iter().enumerate() {
        let mut display_label = info.save_game_info.description.clone();
        if display_label.is_empty() {
            let localized = GameText::fetch(&info.save_game_info.map_label);
            if localized.is_empty() || localized == info.save_game_info.map_label {
                display_label = info.save_game_info.map_label.clone();
            } else {
                display_label = localized;
            }
        }

        let date = &info.save_game_info.date;
        let display_time = format!("{:02}:{:02}", date.hour, date.minute);
        let display_date = format!("{:04}-{:02}-{:02}", date.year, date.month, date.day);

        let color = if info.save_game_info.save_file_type == SaveFileType::Mission {
            WindowColor::new(200, 255, 200, 255)
        } else if (index & 0x1) != 0 {
            WindowColor::new(255, 255, 255, 255)
        } else {
            WindowColor::new(170, 170, 235, 255)
        };

        let item_index = list_box.add_item_with_data_and_color(
            index as i32,
            &display_label,
            Some(ListBoxItemData::Integer(index as i32)),
            Some(color),
        );
        let _ = list_box.set_item_column_data(item_index, 1, ListBoxItemData::Text(display_time));
        let _ = list_box.set_item_column_color(item_index, 1, Some(color));
        let _ = list_box.set_item_column_data(item_index, 2, ListBoxItemData::Text(display_date));
        let _ = list_box.set_item_column_color(item_index, 2, Some(color));
    }

    if !list_box.items().is_empty() {
        let _ = list_box.select_index(0, KeyModifiers::none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_save_description_uses_cpp_backslash_only_path_strip() {
        assert_eq!(
            normalize_default_save_description_from_map_name(
                "Maps\\USA\\Mission01.map".to_string()
            ),
            "Mission01"
        );
        assert_eq!(
            normalize_default_save_description_from_map_name("Maps/USA/Mission01.map".to_string()),
            "Maps/USA/Mission01"
        );
    }

    #[test]
    fn default_save_description_strips_any_cpp_four_char_extension() {
        assert_eq!(
            normalize_default_save_description_from_map_name("Skirmish.foo".to_string()),
            "Skirmish"
        );
        assert_eq!(
            normalize_default_save_description_from_map_name("Skirmish.long".to_string()),
            "Skirmish.long"
        );
    }

    #[test]
    fn save_load_menu_system_consumes_lifecycle_messages_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            save_load_menu_system(&window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            save_load_menu_system(&window, WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
    }

    #[test]
    fn save_load_menu_system_handles_glm_double_clicked_like_cpp() {
        let listbox_id = 101;
        let listbox_window = Rc::new(RefCell::new(GameWindow::new()));
        let mut list_box = crate::gui::gadgets::ListBox::new(listbox_id as u32, 0, 0, 200, 80);
        list_box.add_item_with_data(0, "Existing save", Some(ListBoxItemData::Integer(0)));
        assert!(list_box.select_index(0, KeyModifiers::none()));
        listbox_window
            .borrow_mut()
            .set_widget(crate::gui::WindowWidget::ListBox(list_box));

        {
            let state_handle = save_load_menu_state();
            let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            *state = SaveLoadMenuState::default();
            state.listbox_games = listbox_id;
            state.listbox_games_window = Some(listbox_window.clone());
        }

        let window = GameWindow::new();
        assert_eq!(
            save_load_menu_system(
                &window,
                WindowMessage::User(GLM_DOUBLE_CLICKED),
                listbox_id as WindowMsgData,
                (-1isize) as WindowMsgData,
            ),
            WindowMsgHandled::Handled
        );

        let selected = listbox_window
            .borrow_mut()
            .list_box_mut()
            .map(|list_box| list_box.selected_indices().to_vec())
            .unwrap_or_default();
        assert!(
            selected.is_empty(),
            "C++ GadgetListBoxSetSelected(-1) clears stale selection"
        );
    }
}

fn selected_game_info(state: &SaveLoadMenuState) -> Option<AvailableGameInfo> {
    let listbox = state.listbox_games_window.as_ref()?;
    let mut listbox_guard = listbox.borrow_mut();
    let list_box = listbox_guard.list_box_mut()?;
    let selected = list_box.selected_indices().first().copied()?;
    let data = list_box.get_item_data(selected)?;
    let index = match data {
        ListBoxItemData::Integer(value) => *value,
        _ => return None,
    };
    let game_state = get_game_state();
    game_state.available_games().get(index as usize).cloned()
}

fn set_listbox_selection_from_cpp_row(list_box: &mut crate::gui::gadgets::ListBox, row: i32) {
    if row < 0 {
        list_box.set_selected_indices(&[]);
    } else {
        let _ = list_box.select_index(row as usize, KeyModifiers::none());
    }
}

fn update_menu_actions(state: &SaveLoadMenuState) {
    let can_save = state.current_layout_type != SaveLoadLayoutType::LoadOnly;
    with_window_manager(|manager| {
        if let Some(save_button) = manager.get_window_by_id(state.button_save) {
            let _ = save_button.borrow_mut().enable(can_save);
        }
    });

    let has_selection = selected_game_info(state).is_some();
    with_window_manager(|manager| {
        if let Some(load_button) = manager.get_window_by_id(state.button_load) {
            let _ = load_button.borrow_mut().enable(has_selection);
        }
        if let Some(delete_button) = manager.get_window_by_id(state.button_delete) {
            let _ = delete_button.borrow_mut().enable(has_selection);
        }
    });
}

fn close_save_menu(window: &GameWindow, is_popup: bool) {
    if is_popup {
        if let Some(layout) = window.get_layout() {
            layout.borrow_mut().hide(true);
        }
    } else {
        let _ = get_shell().hide_shell();
    }
}

fn do_load_game(state: &SaveLoadMenuState) {
    let Some(selected) = selected_game_info(state) else {
        return;
    };

    let shell_active = get_shell().is_shell_active();
    if !shell_active {
        destroy_quit_menu();
    } else {
        with_window_manager(|manager| {
            manager.transition_remove("MainMenuLoadReplayMenu", false);
            manager.transition_remove("MainMenuLoadReplayMenuBack", false);
        });
        TheGameLogic::prepare_new_game(GAME_SINGLE_PLAYER, DIFFICULTY_NORMAL, 0);
    }

    let load_result = {
        let mut game_state = get_game_state();
        game_state.load_game(selected)
    };
    if !matches!(load_result, Ok(SaveCode::Ok)) {
        if TheGameLogic::is_in_game() {
            let _ = TheGameLogic::clear_game_data();
        }
        if let Some(engine) = get_game_engine() {
            let mut engine = engine.lock();
            let _ = pollster::block_on(engine.reset());
        }
        let _ = get_shell().show_shell(true);
    }
}

fn process_load_button_press(state: &mut SaveLoadMenuState, window: &GameWindow) {
    if selected_game_info(state).is_none() {
        return;
    }

    if get_shell().is_shell_active() {
        close_save_menu(window, state.is_popup);
        do_load_game(state);
        return;
    }

    if let Some(listbox) = state.listbox_games_window.as_ref() {
        let _ = listbox.borrow_mut().enable(false);
    }
    if let Some(frame) = state.button_frame.as_ref() {
        let _ = frame.borrow_mut().enable(false);
    }
    if let Some(confirm) = state.load_confirm.as_ref() {
        let _ = confirm.borrow_mut().hide(false);
    }
}

pub fn save_load_menu_init(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.current_layout_type = SaveLoadLayoutType::SaveAndLoad;
    state.is_popup = true;
    if let Some(layout_type) = user_data.and_then(|data| data.downcast_ref::<SaveLoadLayoutType>())
    {
        state.current_layout_type = *layout_type;
    }

    init_gadget_ids(&mut state, "PopupSaveLoad.wnd");
    let parent_id = NameKeyGenerator::name_to_key("PopupSaveLoad.wnd:SaveLoadMenu") as i32;
    load_controls(&mut state, parent_id, "PopupSaveLoad.wnd");

    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(parent));
            let _ = manager.set_modal(parent.clone());
        });
    }

    if let Some(frame) = state.button_frame.as_ref() {
        let _ = frame.borrow_mut().enable(true);
    }
    if let Some(window) = state.overwrite_confirm.as_ref() {
        let _ = window.borrow_mut().hide(true);
    }
    if let Some(window) = state.load_confirm.as_ref() {
        let _ = window.borrow_mut().hide(true);
    }
    if let Some(window) = state.save_desc.as_ref() {
        let _ = window.borrow_mut().hide(true);
    }

    populate_save_game_listbox(&mut state);
    update_menu_actions(&state);

    let _ = layout;
}

pub fn save_load_menu_full_screen_init(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
) {
    let mut shell = get_shell();
    shell.show_shell_map(true);

    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.is_popup = false;
    state.current_layout_type = SaveLoadLayoutType::LoadOnly;
    if let Some(layout_type) = user_data.and_then(|data| data.downcast_ref::<SaveLoadLayoutType>())
    {
        state.current_layout_type = *layout_type;
    }

    init_gadget_ids(&mut state, "SaveLoad.wnd");
    let parent_id = NameKeyGenerator::name_to_key("SaveLoad.wnd:SaveLoadMenu") as i32;
    load_controls(&mut state, parent_id, "SaveLoad.wnd");

    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(parent));
        });
    }

    if let Some(frame) = state.button_frame.as_ref() {
        let _ = frame.borrow_mut().enable(true);
    }
    if let Some(window) = state.overwrite_confirm.as_ref() {
        let _ = window.borrow_mut().hide(true);
    }
    if let Some(window) = state.load_confirm.as_ref() {
        let _ = window.borrow_mut().hide(true);
    }
    if let Some(window) = state.save_desc.as_ref() {
        let _ = window.borrow_mut().hide(true);
    }

    populate_save_game_listbox(&mut state);
    update_menu_actions(&state);

    layout.hide(false);
    state.just_entered = true;
    state.initial_gadget_delay = 2;
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().hide(true);
    }
    state.is_shutting_down = false;
}

pub fn save_load_menu_shutdown(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);

    if pop_immediate {
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
        return;
    }

    with_window_manager(|manager| {
        manager.transition_reverse("SaveLoadMenuFade");
    });
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.is_shutting_down = true;
}

pub fn save_load_menu_update(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    if get_dont_show_main_menu() && state.just_entered {
        state.just_entered = false;
    }
    if get_replay_was_pressed() && state.just_entered {
        state.just_entered = false;
        set_replay_was_pressed(false);
    }

    if state.just_entered {
        if state.initial_gadget_delay == 1 {
            with_window_manager(|manager| {
                manager.transition_remove("MainMenuDefaultMenuLogoFade", false);
                manager.transition_set_group("SaveLoadMenuFade", false);
            });
            state.initial_gadget_delay = 2;
            state.just_entered = false;
        } else {
            state.initial_gadget_delay -= 1;
        }
    }

    if state.is_shutting_down {
        let shell_finished = get_shell().is_anim_finished();
        let transitions_finished = with_window_manager(|manager| manager.transitions_finished());
        if shell_finished && transitions_finished {
            layout.hide(true);
            let _ = get_shell().shutdown_complete(None, false);
        }
    }
}

pub fn save_load_menu_input(
    _window: &GameWindow,
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

    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(confirm) = state.delete_confirm.as_ref() {
        let _ = confirm.borrow_mut().hide(true);
    }
    if let Some(listbox) = state.listbox_games_window.as_ref() {
        let _ = listbox.borrow_mut().enable(true);
    }
    if let Some(frame) = state.button_frame.as_ref() {
        let _ = frame.borrow_mut().enable(true);
    }

    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            state.button_back as WindowMsgData,
            0,
        );
    }

    WindowMsgHandled::Handled
}

pub fn save_load_menu_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::User(code) if code == GLM_DOUBLE_CLICKED => {
            if data1 as i32 == state.listbox_games {
                let row_selected = data2 as i32;
                if let Some(listbox) = state.listbox_games_window.as_ref() {
                    if let Some(widget) = listbox.borrow_mut().list_box_mut() {
                        set_listbox_selection_from_cpp_row(widget, row_selected);
                    }
                }
                process_load_button_press(&mut state, window);
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Ignored
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == state.listbox_games {
                update_menu_actions(&state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_load {
                process_load_button_press(&mut state, window);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_save {
                let selected = selected_game_info(&state);
                if selected.is_none() {
                    if let Some(save_desc) = state.save_desc.as_ref() {
                        let _ = save_desc.borrow_mut().hide(false);
                    }
                    if let Some(edit_desc) = state.edit_desc.as_ref() {
                        set_edit_description(edit_desc);
                        with_window_manager(|manager| {
                            let _ = manager.set_focus(Some(edit_desc));
                        });
                    }
                    if let Some(listbox) = state.listbox_games_window.as_ref() {
                        let _ = listbox.borrow_mut().enable(false);
                    }
                } else {
                    if let Some(listbox) = state.listbox_games_window.as_ref() {
                        let _ = listbox.borrow_mut().enable(false);
                    }
                    if let Some(frame) = state.button_frame.as_ref() {
                        let _ = frame.borrow_mut().enable(false);
                    }
                    if let Some(confirm) = state.overwrite_confirm.as_ref() {
                        let _ = confirm.borrow_mut().hide(false);
                    }
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_delete {
                if selected_game_info(&state).is_some() {
                    if let Some(listbox) = state.listbox_games_window.as_ref() {
                        let _ = listbox.borrow_mut().enable(false);
                    }
                    if let Some(frame) = state.button_frame.as_ref() {
                        let _ = frame.borrow_mut().enable(false);
                    }
                    if let Some(confirm) = state.delete_confirm.as_ref() {
                        let _ = confirm.borrow_mut().hide(false);
                    }
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_back {
                if state.is_popup {
                    close_save_menu(window, true);
                } else {
                    let _ = get_shell().pop();
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_delete_confirm || control_id == state.button_delete_cancel
            {
                if control_id == state.button_delete_confirm {
                    if let Some(selected) = selected_game_info(&state) {
                        let game_state = get_game_state();
                        let filepath =
                            game_state.get_file_path_in_save_directory(&selected.filename);
                        let _ = fs::remove_file(filepath);
                    }
                    populate_save_game_listbox(&mut state);
                }

                if let Some(confirm) = state.delete_confirm.as_ref() {
                    let _ = confirm.borrow_mut().hide(true);
                }
                if let Some(listbox) = state.listbox_games_window.as_ref() {
                    let _ = listbox.borrow_mut().enable(true);
                }
                if let Some(frame) = state.button_frame.as_ref() {
                    let _ = frame.borrow_mut().enable(true);
                }
                update_menu_actions(&state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_overwrite_cancel
                || control_id == state.button_overwrite_confirm
            {
                if let Some(confirm) = state.overwrite_confirm.as_ref() {
                    let _ = confirm.borrow_mut().hide(true);
                }

                if control_id == state.button_overwrite_confirm {
                    if let Some(listbox) = state.listbox_games_window.as_ref() {
                        let _ = listbox.borrow_mut().enable(true);
                    }
                    if let Some(frame) = state.button_frame.as_ref() {
                        let _ = frame.borrow_mut().enable(true);
                    }
                    update_menu_actions(&state);
                    close_save_menu(window, state.is_popup);

                    let file_type = if state.current_layout_type == SaveLoadLayoutType::SaveAndLoad
                    {
                        SaveFileType::Normal
                    } else {
                        SaveFileType::Mission
                    };
                    let selected = selected_game_info(&state);
                    let filename = selected
                        .as_ref()
                        .map(|info| info.filename.clone())
                        .unwrap_or_default();
                    let desc = selected
                        .as_ref()
                        .map(|info| info.save_game_info.description.clone())
                        .unwrap_or_default();
                    let mut game_state = get_game_state();
                    let _ = game_state.save_game(filename, desc, file_type, SnapshotType::SaveLoad);
                } else {
                    if let Some(frame) = state.button_frame.as_ref() {
                        let _ = frame.borrow_mut().enable(true);
                    }
                    update_menu_actions(&state);
                    if let Some(listbox) = state.listbox_games_window.as_ref() {
                        let _ = listbox.borrow_mut().enable(true);
                    }
                }

                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_save_desc_confirm {
                let desc = state
                    .edit_desc
                    .as_ref()
                    .and_then(|edit| {
                        let mut edit = edit.borrow_mut();
                        edit.text_entry_mut().map(|entry| entry.text().to_string())
                    })
                    .unwrap_or_default();

                if let Some(save_desc) = state.save_desc.as_ref() {
                    let _ = save_desc.borrow_mut().hide(true);
                }
                if let Some(listbox) = state.listbox_games_window.as_ref() {
                    let _ = listbox.borrow_mut().enable(true);
                }
                if let Some(frame) = state.button_frame.as_ref() {
                    let _ = frame.borrow_mut().enable(true);
                }
                update_menu_actions(&state);
                close_save_menu(window, state.is_popup);

                let selected = selected_game_info(&state);
                let file_type = if state.current_layout_type == SaveLoadLayoutType::SaveAndLoad {
                    SaveFileType::Normal
                } else {
                    SaveFileType::Mission
                };
                let filename = selected
                    .as_ref()
                    .map(|info| info.filename.clone())
                    .unwrap_or_default();
                let mut game_state = get_game_state();
                let _ = game_state.save_game(filename, desc, file_type, SnapshotType::SaveLoad);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_save_desc_cancel {
                if let Some(save_desc) = state.save_desc.as_ref() {
                    let _ = save_desc.borrow_mut().hide(true);
                }
                if let Some(listbox) = state.listbox_games_window.as_ref() {
                    let _ = listbox.borrow_mut().enable(true);
                }
                if let Some(frame) = state.button_frame.as_ref() {
                    let _ = frame.borrow_mut().enable(true);
                }
                update_menu_actions(&state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_load_confirm || control_id == state.button_load_cancel {
                if let Some(confirm) = state.load_confirm.as_ref() {
                    let _ = confirm.borrow_mut().hide(true);
                }
                if let Some(listbox) = state.listbox_games_window.as_ref() {
                    let _ = listbox.borrow_mut().enable(true);
                }
                if let Some(frame) = state.button_frame.as_ref() {
                    let _ = frame.borrow_mut().enable(true);
                }
                update_menu_actions(&state);

                if control_id == state.button_load_confirm {
                    close_save_menu(window, state.is_popup);
                    do_load_game(&state);
                }
                return WindowMsgHandled::Handled;
            }

            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
