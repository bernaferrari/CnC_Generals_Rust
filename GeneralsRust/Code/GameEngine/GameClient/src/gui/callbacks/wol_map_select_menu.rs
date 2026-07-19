//! WOLMapSelectMenu.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::gamespy_game::{
    push_gamespy_game_options, with_gamespy_game_info, with_gamespy_game_info_mut,
};
use crate::gamespy_overlay::{close_overlay, raise_gs_message_box, GameSpyOverlayType};
use crate::gui::callbacks::wol_game_setup_menu::refresh_map_selection_ui;
use crate::gui::gadgets::ListBoxItemData;
use crate::gui::{
    get_shell, show_shell_map_if_available, try_with_shell_mut, with_window_manager,
    write_input_focus_response, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, WindowStatus,
};
use crate::map_util::{
    find_draw_positions, get_map_cache_manager, get_map_preview_image, populate_map_listbox,
};
use game_engine::common::ini::ini_map_cache::MapMetaData;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::preferences::CustomMatchPreferences;
use game_network::gamespy::peer_defs::get_gamespy_info;
use game_network::{SlotState, MAX_SLOTS};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

#[derive(Default)]
struct WolMapSelectState {
    parent_id: i32,
    button_back_id: i32,
    button_ok_id: i32,
    listbox_map_id: i32,
    map_preview_id: i32,
    radio_system_maps_id: i32,
    radio_user_maps_id: i32,
    start_position_ids: [i32; MAX_SLOTS],
    parent: Option<Rc<RefCell<GameWindow>>>,
    listbox_map: Option<Rc<RefCell<GameWindow>>>,
    map_preview: Option<Rc<RefCell<GameWindow>>>,
    selected_map: Option<String>,
    use_system_maps: bool,
    raise_message_boxes: bool,
}

static WOL_MAP_SELECT_STATE: OnceLock<Mutex<WolMapSelectState>> = OnceLock::new();

fn map_select_state() -> &'static Mutex<WolMapSelectState> {
    WOL_MAP_SELECT_STATE.get_or_init(|| Mutex::new(WolMapSelectState::default()))
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn set_radio_selected(window: &Option<Rc<RefCell<GameWindow>>>, selected: bool) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    if let Some(widget) = guard.widget_mut() {
        if let crate::gui::WindowWidget::RadioButton(radio) = widget {
            if selected {
                radio.select();
            }
        }
    }
}

fn set_window_image(win: &Option<Rc<RefCell<GameWindow>>>, image_name: &str) {
    let Some(win) = win else {
        return;
    };
    if image_name.is_empty() {
        return;
    }

    let mut image = crate::display::image::Image::with_name(image_name);
    if let Some(collection) = crate::display::image::get_mapped_image_collection().try_read() {
        if let Some(found) = collection.find_image_by_name(image_name) {
            image.set_filename(found.get_filename());
        }
    }

    let mut win_guard = win.borrow_mut();
    if win_guard.set_enabled_image(0, image).is_ok() {
        win_guard.set_status(WindowStatus::IMAGE);
    }
}

fn map_start_waypoint_name(index: usize) -> String {
    format!("Player_{}_Start", index + 1)
}

fn position_start_buttons(state: &mut WolMapSelectState, meta: Option<&MapMetaData>) {
    let Some(preview) = state.map_preview.as_ref() else {
        return;
    };
    let preview_guard = preview.borrow();
    let (preview_x, preview_y) = preview_guard.get_screen_position();
    let (preview_w, preview_h) = preview_guard.get_size();

    let extent = meta.map(|meta| meta.extent).unwrap_or_default();
    let (ul, lr) = find_draw_positions(preview_x, preview_y, preview_w, preview_h, extent);
    let extent_width = (extent.hi.x - extent.lo.x).max(1.0);
    let extent_height = (extent.hi.y - extent.lo.y).max(1.0);

    for i in 0..MAX_SLOTS {
        let Some(button) =
            with_window_manager(|manager| manager.get_window_by_id(state.start_position_ids[i]))
        else {
            continue;
        };
        let mut button_guard = button.borrow_mut();
        let waypoint = meta.and_then(|meta| meta.get_waypoint(&map_start_waypoint_name(i)));
        if let Some(coord) = waypoint {
            let ratio_x = (coord.x - extent.lo.x) / extent_width;
            let ratio_y = (extent.hi.y - coord.y) / extent_height;
            let draw_x = ul.x as f32 + (lr.x - ul.x) as f32 * ratio_x;
            let draw_y = ul.y as f32 + (lr.y - ul.y) as f32 * ratio_y;
            let (btn_w, btn_h) = button_guard.get_size();
            let new_x = draw_x.round() as i32 - btn_w / 2 - preview_x;
            let new_y = draw_y.round() as i32 - btn_h / 2 - preview_y;
            let _ = button_guard.set_position(new_x, new_y);
            let _ = button_guard.hide(false);
            let _ = button_guard.enable(true);
        } else {
            let _ = button_guard.hide(true);
            let _ = button_guard.enable(false);
        }
    }
}

fn update_selected_map(state: &mut WolMapSelectState) {
    let Some(listbox) = state.listbox_map.as_ref() else {
        return;
    };
    let listbox_guard = listbox.borrow();
    let Some(widget) = listbox_guard.widget().and_then(|widget| match widget {
        crate::gui::WindowWidget::ListBox(listbox) => Some(listbox),
        _ => None,
    }) else {
        return;
    };
    state.selected_map = widget
        .selected_item()
        .and_then(|item| match item.data.as_ref() {
            Some(ListBoxItemData::Text(path)) => Some(path.clone()),
            _ => None,
        });
}

fn update_preview(state: &mut WolMapSelectState) {
    let Some(map_name) = state.selected_map.clone() else {
        set_window_image(&state.map_preview, "");
        if let Some(preview) = state.map_preview.as_ref() {
            preview
                .borrow_mut()
                .set_user_data::<Option<MapMetaData>>(None);
        }
        position_start_buttons(state, None);
        return;
    };
    let preview_name = get_map_preview_image(&map_name).unwrap_or_default();
    set_window_image(&state.map_preview, &preview_name);
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let meta = cache_guard.find_map(&map_name);
    if let Some(preview) = state.map_preview.as_ref() {
        preview.borrow_mut().set_user_data(meta.clone());
    }
    position_start_buttons(state, meta.as_ref());
}

fn populate_map_list(state: &mut WolMapSelectState) {
    let Some(listbox) = state.listbox_map.as_ref() else {
        return;
    };
    let mut listbox_guard = listbox.borrow_mut();
    let Some(widget) = listbox_guard.list_box_mut() else {
        return;
    };
    let map_to_select = state.selected_map.as_deref();
    populate_map_listbox(widget, state.use_system_maps, true, map_to_select);
    state.selected_map = widget
        .selected_item()
        .and_then(|item| match item.data.as_ref() {
            Some(ListBoxItemData::Text(path)) => Some(path.clone()),
            _ => None,
        });
}

fn show_underlying_game_options(show: bool) {
    const LAYOUT: &str = "GameSpyGameOptionsMenu.wnd:";
    let gadgets = [
        "MapWindow",
        "StaticTextTeam",
        "StaticTextFaction",
        "StaticTextColor",
        "TextEntryMapDisplay",
        "ButtonSelectMap",
        "ButtonStart",
        "StaticTextMapPreview",
    ];
    with_window_manager(|manager| {
        for name in &gadgets {
            let id = name_to_id(&format!("{}{}", LAYOUT, name));
            if let Some(win) = manager.get_window_by_id(id) {
                let mut guard = win.borrow_mut();
                let _ = guard.hide(!show);
                let _ = guard.enable(show);
            }
        }
        for i in 0..MAX_SLOTS {
            for base in ["ComboBoxTeam", "ComboBoxColor", "ComboBoxPlayerTemplate"] {
                let id = name_to_id(&format!("{}{}{}", LAYOUT, base, i));
                if let Some(win) = manager.get_window_by_id(id) {
                    let mut guard = win.borrow_mut();
                    let _ = guard.hide(!show);
                    let _ = guard.enable(show);
                }
            }
        }
        let back_id = name_to_id(&format!("{}ButtonBack", LAYOUT));
        if let Some(back) = manager.get_window_by_id(back_id) {
            let _ = back.borrow_mut().enable(show);
        }
    });
}

pub fn wol_map_select_menu_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let mut state = map_select_state().lock().unwrap_or_else(|e| e.into_inner());

    state.parent_id = name_to_id("WOLMapSelectMenu.wnd:WOLMapSelectMenuParent");
    state.button_back_id = name_to_id("WOLMapSelectMenu.wnd:ButtonBack");
    state.button_ok_id = name_to_id("WOLMapSelectMenu.wnd:ButtonOK");
    state.listbox_map_id = name_to_id("WOLMapSelectMenu.wnd:ListboxMap");
    state.radio_system_maps_id = name_to_id("WOLMapSelectMenu.wnd:RadioButtonSystemMaps");
    state.radio_user_maps_id = name_to_id("WOLMapSelectMenu.wnd:RadioButtonUserMaps");
    state.map_preview_id = name_to_id("WOLMapSelectMenu.wnd:WinMapPreview");
    for i in 0..MAX_SLOTS {
        state.start_position_ids[i] =
            name_to_id(&format!("WOLMapSelectMenu.wnd:ButtonMapStartPosition{}", i));
    }

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.listbox_map = manager.get_window_by_id(state.listbox_map_id);
        state.map_preview = manager.get_window_by_id(state.map_preview_id);
    });

    let mut pref = CustomMatchPreferences::new();
    state.use_system_maps = pref.uses_system_map_dir();

    let current_map = with_gamespy_game_info(|info| info.get_map().to_string());
    if !current_map.is_empty() && current_map != "NOMAP" {
        state.selected_map = Some(current_map);
    }

    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        if let Some(room) = info.get_current_staging_room() {
            if !room.map_name.as_str().is_empty() {
                state.selected_map = Some(room.map_name.as_str().to_string());
            }
            if room.use_stats {
                state.use_system_maps = true;
            }
        }
    }

    let radio_system =
        with_window_manager(|manager| manager.get_window_by_id(state.radio_system_maps_id));
    let radio_user =
        with_window_manager(|manager| manager.get_window_by_id(state.radio_user_maps_id));

    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        if let Some(room) = info.get_current_staging_room() {
            if room.use_stats {
                set_radio_selected(&radio_system, true);
                if let Some(user) = radio_user.as_ref() {
                    let _ = user.borrow_mut().enable(false);
                }
            } else if state.use_system_maps {
                set_radio_selected(&radio_system, true);
            } else {
                set_radio_selected(&radio_user, true);
            }
        }
    }

    if let Some(map_cache) = get_map_cache_manager().lock().ok() {
        map_cache.update_cache();
    }

    populate_map_list(&mut state);
    update_selected_map(&mut state);
    update_preview(&mut state);

    state.raise_message_boxes = true;
    show_underlying_game_options(false);

    if let Some(parent) = state.parent.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
    }

    layout.hide(false);
}

pub fn wol_map_select_menu_shutdown(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    {
        let mut state = map_select_state().lock().unwrap_or_else(|e| e.into_inner());
        state.parent = None;
        state.listbox_map = None;
        state.map_preview = None;
        state.selected_map = None;
    }
    layout.hide(true);
    let _ = try_with_shell_mut(|shell| shell.shutdown_complete(layout));
}

pub fn wol_map_select_menu_update(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let mut state = map_select_state().lock().unwrap_or_else(|e| e.into_inner());
    if state.raise_message_boxes {
        raise_gs_message_box();
        state.raise_message_boxes = false;
    }
}

pub fn wol_map_select_menu_input(
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

    let state = map_select_state().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            state.button_back_id as WindowMsgData,
            state.button_back_id as WindowMsgData,
        );
    }

    WindowMsgHandled::Handled
}

pub fn wol_map_select_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => {
            let mut state = map_select_state().lock().unwrap_or_else(|e| e.into_inner());
            state.parent = None;
            state.listbox_map = None;
            state.map_preview = None;
            state.selected_map = None;
            WindowMsgHandled::Handled
        }
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let mut state = map_select_state().lock().unwrap_or_else(|e| e.into_inner());
            let control_id = data1 as i32;
            if control_id == state.button_back_id {
                show_underlying_game_options(true);
                close_overlay(GameSpyOverlayType::MapSelect);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.radio_system_maps_id {
                state.use_system_maps = true;
                populate_map_list(&mut state);
                update_preview(&mut state);
                let mut pref = CustomMatchPreferences::new();
                pref.set_uses_system_map_dir(true);
                pref.write();
                return WindowMsgHandled::Handled;
            }
            if control_id == state.radio_user_maps_id {
                state.use_system_maps = false;
                populate_map_list(&mut state);
                update_preview(&mut state);
                let mut pref = CustomMatchPreferences::new();
                pref.set_uses_system_map_dir(false);
                pref.write();
                return WindowMsgHandled::Handled;
            }
            if control_id == state.listbox_map_id {
                update_selected_map(&mut state);
                update_preview(&mut state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_ok_id {
                if let Some(map_name) = state.selected_map.clone() {
                    let map_cache = get_map_cache_manager();
                    let mut cache_guard = map_cache.lock().unwrap_or_else(|e| e.into_inner());
                    cache_guard.update_cache();
                    let meta = cache_guard.find_map(&map_name).cloned();

                    with_gamespy_game_info_mut(|info| {
                        info.set_map(map_name.clone());
                        if let Some(meta) = meta.as_ref() {
                            info.set_map_crc(meta.crc);
                            info.set_map_size(meta.filesize);
                        }
                        if let Some(slot) = info.get_slot_mut(0) {
                            slot.set_map_availability(true);
                            if slot.get_state() == SlotState::Closed {
                                slot.set_state(SlotState::Open, String::new(), 0);
                            }
                        }
                        info.adjust_slots_for_map();
                        info.reset_accepted();
                        info.reset_start_spots();
                    });

                    let _ = push_gamespy_game_options();
                    refresh_map_selection_ui();
                }

                show_underlying_game_options(true);
                close_overlay(GameSpyOverlayType::MapSelect);
                return WindowMsgHandled::Handled;
            }
        }
        WindowMessage::GadgetValueChanged => {
            let mut state = map_select_state().lock().unwrap_or_else(|e| e.into_inner());
            if data1 as i32 == state.listbox_map_id {
                update_selected_map(&mut state);
                update_preview(&mut state);
                return WindowMsgHandled::Handled;
            }
        }
        WindowMessage::User(0x8000) => {
            let mut state = map_select_state().lock().unwrap_or_else(|e| e.into_inner());
            if data1 as i32 == state.listbox_map_id {
                update_selected_map(&mut state);
                update_preview(&mut state);
                if let Some(map_name) = state.selected_map.clone() {
                    let map_cache = get_map_cache_manager();
                    let mut cache_guard = map_cache.lock().unwrap_or_else(|e| e.into_inner());
                    cache_guard.update_cache();
                    let meta = cache_guard.find_map(&map_name).cloned();

                    with_gamespy_game_info_mut(|info| {
                        info.set_map(map_name.clone());
                        if let Some(meta) = meta.as_ref() {
                            info.set_map_crc(meta.crc);
                            info.set_map_size(meta.filesize);
                        }
                        if let Some(slot) = info.get_slot_mut(0) {
                            slot.set_map_availability(true);
                            if slot.get_state() == SlotState::Closed {
                                slot.set_state(SlotState::Open, String::new(), 0);
                            }
                        }
                        info.adjust_slots_for_map();
                        info.reset_accepted();
                        info.reset_start_spots();
                    });

                    let _ = push_gamespy_game_options();
                }

                show_underlying_game_options(true);
                close_overlay(GameSpyOverlayType::MapSelect);
                return WindowMsgHandled::Handled;
            }
        }
        _ => {}
    }

    WindowMsgHandled::Ignored
}
