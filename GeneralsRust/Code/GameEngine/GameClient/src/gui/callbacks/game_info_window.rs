//! GameInfoWindow.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::gui::gadgets::ListBoxItemData;
use crate::gui::{
    with_window_manager, Color, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled,
};
use crate::map_util::get_map_cache_manager;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::player_template::get_player_template_store;
use game_network::{GameInfo, SlotState, MAX_SLOTS, PLAYERTEMPLATE_OBSERVER};
use gamelogic::helpers::TheGameText;

#[derive(Default)]
struct GameInfoWindowState {
    parent_id: i32,
    static_text_game_name_id: i32,
    static_text_map_name_id: i32,
    list_box_players_id: i32,
    win_crates_id: i32,
    win_super_weapons_id: i32,
    win_free_for_all_id: i32,
    layout: Option<Rc<RefCell<WindowLayout>>>,
    parent: Option<Rc<RefCell<GameWindow>>>,
    static_text_game_name: Option<Rc<RefCell<GameWindow>>>,
    static_text_map_name: Option<Rc<RefCell<GameWindow>>>,
    list_box_players: Option<Rc<RefCell<GameWindow>>>,
}

thread_local! {
    static GAME_INFO_STATE: Arc<Mutex<GameInfoWindowState>> =
        Arc::new(Mutex::new(GameInfoWindowState::default()));
}

fn game_info_state() -> Arc<Mutex<GameInfoWindowState>> {
    GAME_INFO_STATE.with(|state| state.clone())
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn color_from_rgb(rgb: u32) -> Color {
    Color::new(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
        255,
    )
}

fn map_display_name(map_name: &str) -> String {
    if map_name.is_empty() {
        return String::new();
    }
    let cache = get_map_cache_manager();
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.update_cache();

    let lookup = map_name.to_lowercase();
    if let Some(meta) = cache_guard.find_map(&lookup) {
        let display = meta.display_name.as_str().to_string();
        if !display.is_empty() {
            return display;
        }
    }

    let leaf = map_name.rsplit(['\\', '/']).next().unwrap_or(map_name);
    leaf.to_string()
}

pub fn create_lan_game_info_window(size_and_pos_window: &GameWindow) {
    let needs_layout = {
        let state_handle = game_info_state();
        let state = state_handle
            .lock()
            .expect("GameInfoWindow state lock poisoned");
        state.layout.is_none()
    };

    if needs_layout {
        if let Some(layout) = with_window_manager(|manager| {
            manager
                .create_layout_with_windows("Menus/GameInfoWindow.wnd")
                .ok()
                .map(|(layout, _)| layout)
        }) {
            layout.borrow().run_init(None);
            layout.borrow_mut().bring_forward();
            layout.borrow_mut().hide(true);
            let state_handle = game_info_state();
            let mut state = state_handle
                .lock()
                .expect("GameInfoWindow state lock poisoned");
            state.layout = Some(layout);
        }
    }

    let parent = {
        let state_handle = game_info_state();
        let mut state = state_handle
            .lock()
            .expect("GameInfoWindow state lock poisoned");
        if state.parent.is_none() {
            with_window_manager(|manager| {
                state.parent = manager.get_window_by_id(state.parent_id);
            });
        }
        state.parent.clone()
    };

    let Some(parent) = parent.as_ref() else {
        return;
    };
    let (x, y) = size_and_pos_window.get_screen_position();
    let (width, height) = size_and_pos_window.get_size();
    let mut parent_guard = parent.borrow_mut();
    let _ = parent_guard.set_position(x, y);
    let _ = parent_guard.set_size(width, height);
}

pub fn destroy_game_info_window() {
    let state_handle = game_info_state();
    let mut state = state_handle
        .lock()
        .expect("GameInfoWindow state lock poisoned");
    if let Some(layout) = state.layout.take() {
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
    state.parent = None;
    state.static_text_game_name = None;
    state.static_text_map_name = None;
    state.list_box_players = None;
}

pub fn refresh_game_info_window(game_info: &GameInfo) {
    let state_handle = game_info_state();
    let mut state = state_handle
        .lock()
        .expect("GameInfoWindow state lock poisoned");
    if state.layout.is_none() || state.parent.is_none() {
        return;
    }

    if let Some(parent) = state.parent.as_ref() {
        let mut parent_guard = parent.borrow_mut();
        let _ = parent_guard.hide(false);
        let _ = parent_guard.bring_to_front();
    }

    if let Some(static_text) = state.static_text_game_name.as_ref() {
        if let Some(host_slot) = game_info.get_slot(0) {
            let text = host_slot.get_name();
            if let Some(widget) = static_text.borrow_mut().static_text_mut() {
                widget.set_text(text.to_string());
            }
        }
    }

    if let Some(static_text) = state.static_text_map_name.as_ref() {
        let map_name = map_display_name(game_info.get_map());
        if let Some(widget) = static_text.borrow_mut().static_text_mut() {
            widget.set_text(map_name);
        }
    }

    let Some(listbox_window) = state.list_box_players.as_ref() else {
        return;
    };
    let mut listbox_guard = listbox_window.borrow_mut();
    let Some(listbox) = listbox_guard.list_box_mut() else {
        return;
    };
    listbox.clear();

    for slot_index in 0..MAX_SLOTS {
        let Some(slot) = game_info.get_slot(slot_index) else {
            continue;
        };
        if !slot.is_occupied() {
            continue;
        }
        let text = match slot.get_state() {
            SlotState::Player => slot.get_name().to_string(),
            SlotState::EasyAI => TheGameText::fetch("GUI:EasyAI"),
            SlotState::MedAI => TheGameText::fetch("GUI:MediumAI"),
            SlotState::BrutalAI => TheGameText::fetch("GUI:HardAI"),
            _ => slot.get_name().to_string(),
        };

        let mut text_color = None;
        if let Some(rgb) =
            with_multiplayer_settings(|settings| settings.get_color_value(slot.get_color()))
        {
            text_color = Some(color_from_rgb(rgb));
        }

        let row = if let Some(color) = text_color {
            listbox.add_item_with_color(&text, color)
        } else {
            listbox.add_item(&text);
            listbox.items().len().saturating_sub(1)
        };

        let player_template = slot.get_player_template();
        if player_template == PLAYERTEMPLATE_OBSERVER {
            let _ = listbox.set_item_column_data(
                row,
                0,
                ListBoxItemData::Image {
                    name: "GameinfoOBSRVR".to_string(),
                    width: 22,
                    height: 25,
                    text: None,
                },
            );
        } else if player_template < 0 {
            let _ = listbox.set_item_column_data(
                row,
                0,
                ListBoxItemData::Image {
                    name: "GameinfoRANDOM".to_string(),
                    width: 22,
                    height: 25,
                    text: None,
                },
            );
        } else if let Some(template) =
            get_player_template_store().get_nth_player_template(player_template as usize)
        {
            let icon = template.get_side_icon_image();
            if !icon.is_empty() {
                let _ = listbox.set_item_column_data(
                    row,
                    0,
                    ListBoxItemData::Image {
                        name: icon.to_string(),
                        width: 22,
                        height: 25,
                        text: None,
                    },
                );
            }
        }
    }
}

pub fn hide_game_info_window(hide: bool) {
    let state_handle = game_info_state();
    let state = state_handle
        .lock()
        .expect("GameInfoWindow state lock poisoned");
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().hide(hide);
    }
}

pub fn game_info_window_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let state_handle = game_info_state();
    let mut state = state_handle
        .lock()
        .expect("GameInfoWindow state lock poisoned");

    state.parent_id = name_to_id("GameInfoWindow.wnd:ParentGameInfo");
    state.static_text_game_name_id = name_to_id("GameInfoWindow.wnd:StaticTextGameName");
    state.static_text_map_name_id = name_to_id("GameInfoWindow.wnd:StaticTextMapName");
    state.list_box_players_id = name_to_id("GameInfoWindow.wnd:ListBoxPlayers");
    state.win_crates_id = name_to_id("GameInfoWindow.wnd:WinCrates");
    state.win_super_weapons_id = name_to_id("GameInfoWindow.wnd:WinSuperWeapons");
    state.win_free_for_all_id = name_to_id("GameInfoWindow.wnd:WinFreeForAll");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.static_text_game_name = manager.get_window_by_id(state.static_text_game_name_id);
        state.static_text_map_name = manager.get_window_by_id(state.static_text_map_name_id);
        state.list_box_players = manager.get_window_by_id(state.list_box_players_id);
    });

    if let Some(static_text) = state.static_text_game_name.as_ref() {
        if let Some(widget) = static_text.borrow_mut().static_text_mut() {
            widget.set_text(String::new());
        }
    }
    if let Some(static_text) = state.static_text_map_name.as_ref() {
        if let Some(widget) = static_text.borrow_mut().static_text_mut() {
            widget.set_text(String::new());
        }
    }
    if let Some(listbox) = state.list_box_players.as_ref() {
        if let Some(widget) = listbox.borrow_mut().list_box_mut() {
            widget.clear();
        }
    }

    let _ = layout;
}

pub fn game_info_window_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::Create => WindowMsgHandled::Handled,
        _ => WindowMsgHandled::Ignored,
    }
}
