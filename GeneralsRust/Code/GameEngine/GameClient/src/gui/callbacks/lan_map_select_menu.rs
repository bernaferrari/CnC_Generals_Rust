//! LanMapSelectMenu.cpp callback port.

use super::lan_game_options_menu::{
    destroy_lan_map_select_overlay, refresh_lan_game_options_from_setup,
    show_lan_game_options_underlying_gui_elements,
};
use crate::display::image::get_mapped_image_collection;
use crate::gui::gadgets::{KeyModifiers, ListBoxItemData};
use crate::gui::game_window::Image as WindowImage;
use crate::gui::{
    get_lan_setup, get_shell, with_window_manager, write_input_focus_response, GameWindow,
    WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
};
use crate::map_util::{
    find_draw_positions, get_map_cache_manager, get_map_preview_image, populate_map_listbox,
};
use game_engine::common::ini::ini_map_cache::MapMetaData;
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const MAX_SLOTS: usize = 8;

#[derive(Default)]
struct LanMapSelectState {
    parent_id: i32,
    listbox_map_id: i32,
    button_ok_id: i32,
    button_back_id: i32,
    radio_system_maps_id: i32,
    radio_user_maps_id: i32,
    map_preview_id: i32,
    start_position_ids: [i32; MAX_SLOTS],
    parent: Option<Rc<RefCell<GameWindow>>>,
    listbox_map: Option<Rc<RefCell<GameWindow>>>,
    map_preview: Option<Rc<RefCell<GameWindow>>>,
    selected_map: Option<String>,
    use_system_maps: bool,
}

thread_local! {
    static LAN_MAP_SELECT_STATE: Arc<Mutex<LanMapSelectState>> =
        Arc::new(Mutex::new(LanMapSelectState::default()));
}

fn map_select_state() -> Arc<Mutex<LanMapSelectState>> {
    LAN_MAP_SELECT_STATE.with(|state| state.clone())
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

    let (width, height) = if let Some(collection) = get_mapped_image_collection().try_read() {
        if let Some(found) = collection.find_image_by_name(image_name) {
            let size = found.get_image_size();
            (size.x, size.y)
        } else {
            (0, 0)
        }
    } else {
        (0, 0)
    };
    let image = WindowImage {
        name: image_name.to_string(),
        width,
        height,
    };

    let mut win_guard = win.borrow_mut();
    if win_guard.set_enabled_image(0, image).is_ok() {
        win_guard.set_status(WindowStatus::IMAGE);
    }
}

fn update_selected_map(state: &mut LanMapSelectState) {
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

fn populate_map_list(state: &mut LanMapSelectState) {
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

fn map_start_waypoint_name(index: usize) -> String {
    format!("Player_{}_Start", index + 1)
}

fn position_start_buttons(state: &mut LanMapSelectState, meta: Option<&MapMetaData>) {
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

fn update_preview(state: &mut LanMapSelectState) {
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

pub fn lan_map_select_menu_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let state_handle = map_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.parent_id = name_to_id("LanMapSelectMenu.wnd:LanMapSelectMenuParent");
    state.listbox_map_id = name_to_id("LanMapSelectMenu.wnd:ListboxMap");
    state.button_ok_id = name_to_id("LanMapSelectMenu.wnd:ButtonOK");
    state.button_back_id = name_to_id("LanMapSelectMenu.wnd:ButtonBack");
    state.radio_system_maps_id = name_to_id("LanMapSelectMenu.wnd:RadioButtonSystemMaps");
    state.radio_user_maps_id = name_to_id("LanMapSelectMenu.wnd:RadioButtonUserMaps");
    state.map_preview_id = name_to_id("LanMapSelectMenu.wnd:WinMapPreview");
    for i in 0..MAX_SLOTS {
        state.start_position_ids[i] =
            name_to_id(&format!("LanMapSelectMenu.wnd:ButtonMapStartPosition{}", i));
    }

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.listbox_map = manager.get_window_by_id(state.listbox_map_id);
        state.map_preview = manager.get_window_by_id(state.map_preview_id);
        if let Some(parent) = state.parent.as_ref() {
            let _ = manager.set_focus(Some(parent));
        }
    });

    {
        let setup = get_lan_setup();
        if !setup.selected_map().is_empty() {
            state.selected_map = Some(setup.selected_map().to_string());
        }
    }

    if let Some(map_name) = state.selected_map.as_ref() {
        let cache = get_map_cache_manager();
        if let Ok(cache_guard) = cache.lock() {
            if let Some(meta) = cache_guard.find_map(map_name) {
                state.use_system_maps = meta.is_official;
            }
        };
    } else {
        let setup = get_lan_setup();
        state.use_system_maps = setup.use_system_maps();
    }

    for button_id in state.start_position_ids {
        if let Some(button) = with_window_manager(|manager| manager.get_window_by_id(button_id)) {
            let _ = button.borrow_mut().hide(true);
            let _ = button.borrow_mut().enable(false);
        }
    }

    if let Ok(mut cache) = get_map_cache_manager().lock() {
        cache.update_cache();
    }

    set_radio_selected(
        &with_window_manager(|manager| manager.get_window_by_id(state.radio_system_maps_id)),
        state.use_system_maps,
    );
    set_radio_selected(
        &with_window_manager(|manager| manager.get_window_by_id(state.radio_user_maps_id)),
        !state.use_system_maps,
    );

    populate_map_list(&mut state);
    update_selected_map(&mut state);
    update_preview(&mut state);
    show_lan_game_options_underlying_gui_elements(false);
    layout.hide(false);
}

pub fn lan_map_select_menu_update(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

pub fn lan_map_select_menu_shutdown(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    layout.hide(true);
    nullify_controls();
}

fn nullify_controls() {
    let state_handle = map_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.parent = None;
    state.listbox_map = None;
    state.map_preview = None;
}

pub fn lan_map_select_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = map_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::Create => return WindowMsgHandled::Handled,
        WindowMessage::Destroy => {
            drop(state);
            nullify_controls();
            return WindowMsgHandled::Handled;
        }
        WindowMessage::InputFocus => return write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == state.button_ok_id {
                if let Some(map_name) = state.selected_map.clone() {
                    let mut setup = get_lan_setup();
                    setup.set_selected_map(map_name);
                    setup.set_use_system_maps(state.use_system_maps);
                    setup.game_info_mut().reset_start_spots();
                }
                show_lan_game_options_underlying_gui_elements(true);
                refresh_lan_game_options_from_setup();
                destroy_lan_map_select_overlay();
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_back_id {
                show_lan_game_options_underlying_gui_elements(true);
                refresh_lan_game_options_from_setup();
                destroy_lan_map_select_overlay();
                return WindowMsgHandled::Handled;
            }
            if control_id == state.radio_system_maps_id {
                state.use_system_maps = true;
                populate_map_list(&mut state);
                update_preview(&mut state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.radio_user_maps_id {
                state.use_system_maps = false;
                populate_map_list(&mut state);
                update_preview(&mut state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.listbox_map_id {
                update_selected_map(&mut state);
                update_preview(&mut state);
                return WindowMsgHandled::Handled;
            }
        }
        WindowMessage::GadgetValueChanged => {
            if data1 as i32 == state.listbox_map_id {
                update_selected_map(&mut state);
                update_preview(&mut state);
                return WindowMsgHandled::Handled;
            }
        }
        WindowMessage::User(0x8000) => {
            if data1 as i32 == state.listbox_map_id {
                if (data2 as i32) >= 0 {
                    if let Some(listbox) = state.listbox_map.as_ref() {
                        if let Some(widget) = listbox.borrow_mut().list_box_mut() {
                            let _ = widget.select_index(data2 as usize, KeyModifiers::none());
                        }
                    }
                }
                update_selected_map(&mut state);
                update_preview(&mut state);
                if let Some(parent) = state.parent.as_ref() {
                    let _ = parent.borrow_mut().send_system_message(
                        WindowMessage::GadgetSelected,
                        state.button_ok_id as WindowMsgData,
                        state.button_ok_id as WindowMsgData,
                    );
                }
                return WindowMsgHandled::Handled;
            }
        }
        _ => {}
    }

    WindowMsgHandled::Ignored
}

pub fn lan_map_select_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }

    if (data2 & KEY_STATE_UP) != 0 {
        let state_handle = map_select_state();
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
            lan_map_select_menu_input(&window, WindowMessage::Char, KEY_ESC as WindowMsgData, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            lan_map_select_menu_input(&window, WindowMessage::Char, b'A' as WindowMsgData, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn lan_map_select_system_consumes_lifecycle_messages_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            lan_map_select_menu_system(&window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            lan_map_select_menu_system(&window, WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
    }
}
