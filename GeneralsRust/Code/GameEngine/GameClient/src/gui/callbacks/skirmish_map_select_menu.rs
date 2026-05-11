//! SkirmishMapSelectMenu.cpp callback port.

use super::skirmish_game_options_menu::{
    destroy_skirmish_map_select_overlay, refresh_skirmish_game_options_from_setup,
    show_skirmish_game_options_underlying_gui_elements,
};
use crate::display::image::get_mapped_image_collection;
use crate::gui::gadgets::{KeyModifiers, ListBoxItemData};
use crate::gui::game_window::Image as WindowImage;
use crate::gui::{
    get_shell, get_skirmish_setup, with_window_manager, with_window_manager_ref, GameWindow,
    WindowInstanceData, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
    WIN_COLOR_UNDEFINED,
};
use crate::map_util::{
    find_draw_positions, get_map_cache_manager, get_map_preview_image,
    get_supply_and_tech_image_locations, populate_map_listbox, populate_map_listbox_no_reset,
};
use game_engine::common::ini::ini_map_cache::MapMetaData;
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;
const MAX_SLOTS: usize = 8;
const MAP_SELECT_PARENT_NAME: &str = "SkirmishMapSelectMenu.wnd:SkrimishMapSelectMenuParent";
const UNKNOWN_MAP_IMAGE: &str = "UnknownMap";
const SUPPLY_TECH_SIZE: i32 = 15;

#[derive(Default)]
struct SkirmishMapSelectState {
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
    static SKIRMISH_MAP_SELECT_STATE: Arc<Mutex<SkirmishMapSelectState>> =
        Arc::new(Mutex::new(SkirmishMapSelectState::default()));
}

fn map_select_state() -> Arc<Mutex<SkirmishMapSelectState>> {
    SKIRMISH_MAP_SELECT_STATE.with(|state| state.clone())
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
    let image_name = if image_name.trim().is_empty() {
        UNKNOWN_MAP_IMAGE
    } else {
        image_name
    };

    let mut resolved_name = image_name;
    let mut resolved_size = None;
    if let Some(collection) = get_mapped_image_collection().try_read() {
        if let Some(found) = collection.find_image_by_name(image_name) {
            let size = found.get_image_size();
            resolved_size = Some((size.x, size.y));
        } else if image_name != UNKNOWN_MAP_IMAGE {
            if let Some(found) = collection.find_image_by_name(UNKNOWN_MAP_IMAGE) {
                let size = found.get_image_size();
                resolved_name = UNKNOWN_MAP_IMAGE;
                resolved_size = Some((size.x, size.y));
            }
        }
    }

    let Some((width, height)) = resolved_size else {
        let mut win_guard = win.borrow_mut();
        let _ = win_guard.clear_status(WindowStatus::IMAGE);
        return;
    };

    let image = WindowImage {
        name: resolved_name.to_string(),
        width,
        height,
    };

    let mut win_guard = win.borrow_mut();
    if win_guard.set_enabled_image(0, image).is_ok() {
        win_guard.set_status(WindowStatus::IMAGE);
    }
}

fn resolve_preview_image_name(preview_name: Option<String>) -> String {
    preview_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| UNKNOWN_MAP_IMAGE.to_string())
}

fn update_selected_map(state: &mut SkirmishMapSelectState) {
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

fn populate_map_list(state: &mut SkirmishMapSelectState) {
    let Some(listbox) = state.listbox_map.as_ref() else {
        return;
    };
    let mut listbox_guard = listbox.borrow_mut();
    let Some(widget) = listbox_guard.list_box_mut() else {
        return;
    };
    let map_to_select = state.selected_map.as_deref();
    if state.use_system_maps {
        populate_map_listbox(widget, true, true, map_to_select);
    } else {
        // C++ parity: user-map mode first appends single-player user maps, then multiplayer user maps.
        populate_map_listbox(widget, false, false, map_to_select);
        populate_map_listbox_no_reset(widget, false, true, map_to_select);
    }
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

fn position_start_buttons(state: &mut SkirmishMapSelectState, meta: Option<&MapMetaData>) {
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

fn update_preview(state: &mut SkirmishMapSelectState) {
    let Some(map_name) = state.selected_map.clone() else {
        set_window_image(&state.map_preview, UNKNOWN_MAP_IMAGE);
        if let Some(preview) = state.map_preview.as_ref() {
            preview
                .borrow_mut()
                .set_user_data::<Option<MapMetaData>>(None);
        }
        position_start_buttons(state, None);
        return;
    };
    let preview_name = resolve_preview_image_name(get_map_preview_image(&map_name));
    set_window_image(&state.map_preview, &preview_name);
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let meta = cache_guard.find_map(&map_name);
    if let Some(preview) = state.map_preview.as_ref() {
        preview.borrow_mut().set_user_data(meta.clone());
    }
    position_start_buttons(state, meta.as_ref());
}

pub fn skirmish_map_select_menu_init(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let state_handle = map_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.parent_id = name_to_id(MAP_SELECT_PARENT_NAME);
    state.listbox_map_id = name_to_id("SkirmishMapSelectMenu.wnd:ListboxMap");
    state.button_ok_id = name_to_id("SkirmishMapSelectMenu.wnd:ButtonOK");
    state.button_back_id = name_to_id("SkirmishMapSelectMenu.wnd:ButtonBack");
    state.radio_system_maps_id = name_to_id("SkirmishMapSelectMenu.wnd:RadioButtonSystemMaps");
    state.radio_user_maps_id = name_to_id("SkirmishMapSelectMenu.wnd:RadioButtonUserMaps");
    state.map_preview_id = name_to_id("SkirmishMapSelectMenu.wnd:WinMapPreview");
    for i in 0..MAX_SLOTS {
        state.start_position_ids[i] = name_to_id(&format!(
            "SkirmishMapSelectMenu.wnd:ButtonMapStartPosition{}",
            i
        ));
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
        let setup = get_skirmish_setup();
        if !setup.selected_map().is_empty() {
            state.selected_map = Some(setup.selected_map().to_string());
        } else if !setup.game_info().game_info().get_map().is_empty() {
            state.selected_map = Some(setup.game_info().game_info().get_map().to_string());
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
        let setup = get_skirmish_setup();
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
    show_skirmish_game_options_underlying_gui_elements(false);
    layout.hide(false);
}

pub fn skirmish_map_select_menu_update(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

pub fn skirmish_map_select_menu_shutdown(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    layout.hide(true);
    let state_handle = map_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.parent = None;
    state.listbox_map = None;
    state.map_preview = None;
}

pub fn skirmish_map_select_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = map_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::InputFocus => {
            // C++ parity: acknowledge focus handoff explicitly so the menu can take keyboard focus.
            return WindowMsgHandled::Handled;
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == state.button_ok_id {
                let Some(map_name) = state.selected_map.clone() else {
                    // C++ parity: with no selected row, ignore OK and keep overlay open.
                    return WindowMsgHandled::Handled;
                };

                let cache = get_map_cache_manager();
                let mut setup = get_skirmish_setup();
                setup.set_selected_map(map_name.clone());
                setup.set_use_system_maps(state.use_system_maps);
                let info = setup.game_info_mut().game_info_mut();
                info.set_map(map_name.clone());
                if let Ok(cache_guard) = cache.lock() {
                    if let Some(meta) = cache_guard.find_map(&map_name) {
                        info.set_map_crc(meta.crc);
                        info.set_map_size(meta.filesize);
                    } else {
                        info.set_map_crc(0);
                        info.set_map_size(0);
                    }
                }
                info.reset_start_spots();

                show_skirmish_game_options_underlying_gui_elements(true);
                refresh_skirmish_game_options_from_setup();
                destroy_skirmish_map_select_overlay();
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_back_id {
                show_skirmish_game_options_underlying_gui_elements(true);
                refresh_skirmish_game_options_from_setup();
                destroy_skirmish_map_select_overlay();
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
                        state.button_ok_id as u32,
                        state.button_ok_id as u32,
                    );
                }
                return WindowMsgHandled::Handled;
            }
        }
        _ => {}
    }

    WindowMsgHandled::Ignored
}

pub fn skirmish_map_select_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char {
        let key = data1 as u32;
        let state = data2 as u32;
        if key == KEY_ESC && (state & KEY_STATE_UP) != 0 {
            let state_handle = map_select_state();
            let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(parent) = state.parent.as_ref() {
                let _ = parent.borrow_mut().send_system_message(
                    WindowMessage::GadgetSelected,
                    state.button_back_id as u32,
                    state.button_back_id as u32,
                );
            }
            return WindowMsgHandled::Handled;
        }
    }
    WindowMsgHandled::Ignored
}

fn draw_skinny_border(pixel_x: i32, pixel_y: i32, width: i32, height: i32) {
    const BORDER_LINE_SIZE: i32 = 5;
    const SIZE: i32 = 5;
    const HALF_SIZE: i32 = SIZE / 2;
    const OFFSET: i32 = 2;
    const OFFSET_LOWER: i32 = 5;

    let max_x = pixel_x + width;
    let max_y = pixel_y + height;

    with_window_manager_ref(|manager| {
        let top = manager.win_find_image("FrameT");
        let bottom = manager.win_find_image("FrameB");
        if let (Some(top), Some(bottom)) = (top, bottom) {
            let top_y = pixel_y - OFFSET;
            let bottom_y = max_y - OFFSET_LOWER;
            let mut x = pixel_x + 3;
            let x_limit = max_x - (OFFSET_LOWER + SIZE);
            while x <= x_limit {
                manager.win_draw_image(&top, x, top_y, x + SIZE, top_y + SIZE, WIN_COLOR_UNDEFINED);
                manager.win_draw_image(
                    &bottom,
                    x,
                    bottom_y,
                    x + SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                x += SIZE;
            }
            let border_end = max_x - SIZE;
            if (border_end - x) >= (BORDER_LINE_SIZE / 2) {
                manager.win_draw_image(
                    &top,
                    x,
                    top_y,
                    x + HALF_SIZE,
                    top_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &bottom,
                    x,
                    bottom_y,
                    x + HALF_SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                x += BORDER_LINE_SIZE / 2;
            }
            if x < border_end {
                let adjust = (BORDER_LINE_SIZE / 2) - (((border_end - x) + 1) & !1);
                x -= adjust;
                manager.win_draw_image(
                    &top,
                    x,
                    top_y,
                    x + HALF_SIZE,
                    top_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &bottom,
                    x,
                    bottom_y,
                    x + HALF_SIZE,
                    bottom_y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }

        let left = manager.win_find_image("FrameL");
        let right = manager.win_find_image("FrameR");
        if let (Some(left), Some(right)) = (left, right) {
            let left_x = pixel_x - OFFSET;
            let right_x = max_x - OFFSET_LOWER;
            let mut y = pixel_y + 3;
            let y_limit = max_y - (OFFSET_LOWER + SIZE);
            while y <= y_limit {
                manager.win_draw_image(
                    &left,
                    left_x,
                    y,
                    left_x + SIZE,
                    y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &right,
                    right_x,
                    y,
                    right_x + SIZE,
                    y + SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                y += SIZE;
            }
            let border_end = max_y - OFFSET_LOWER;
            if (border_end - y) >= (BORDER_LINE_SIZE / 2) {
                manager.win_draw_image(
                    &left,
                    left_x,
                    y,
                    left_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &right,
                    right_x,
                    y,
                    right_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                y += BORDER_LINE_SIZE / 2;
            }
            if y < border_end {
                let adjust = (BORDER_LINE_SIZE / 2) - (((border_end - y) + 1) & !1);
                y -= adjust;
                manager.win_draw_image(
                    &left,
                    left_x,
                    y,
                    left_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
                manager.win_draw_image(
                    &right,
                    right_x,
                    y,
                    right_x + SIZE,
                    y + HALF_SIZE,
                    WIN_COLOR_UNDEFINED,
                );
            }
        }

        for (name, x, y) in [
            ("FrameCornerUL", pixel_x - 2, pixel_y - 2),
            ("FrameCornerUR", max_x - 5, pixel_y - 2),
            ("FrameCornerLL", pixel_x - 2, max_y - 5),
            ("FrameCornerLR", max_x - 5, max_y - 5),
        ] {
            if let Some(image) = manager.win_find_image(name) {
                manager.win_draw_image(&image, x, y, x + SIZE, y + SIZE, WIN_COLOR_UNDEFINED);
            }
        }
    });
}

pub fn draw_map_preview(window: &GameWindow, _inst: &WindowInstanceData) {
    let (x, y) = window.get_screen_position();
    let (w, h) = window.get_size();
    if w <= 0 || h <= 0 {
        return;
    }

    let meta = window
        .get_user_data::<Option<MapMetaData>>()
        .and_then(|meta| meta.as_ref())
        .cloned();
    let Some(meta) = meta else {
        super::super::game_window::default_draw_callback(window, _inst);
        draw_skinny_border(x - 1, y - 1, w + 2, h + 2);
        return;
    };

    let (ul, lr) = find_draw_positions(x, y, w, h, meta.extent);
    let fill_color = 0xFF000000;
    let line_color = 0xFF323232;

    with_window_manager_ref(|manager| {
        let map_ratio = (meta.extent.hi.x - meta.extent.lo.x) / (w as f32).max(1.0);
        let window_ratio = (meta.extent.hi.y - meta.extent.lo.y) / (h as f32).max(1.0);
        if map_ratio >= window_ratio {
            manager.win_fill_rect(fill_color, 1.0, x, y, x + w, ul.y - 1);
            manager.win_fill_rect(fill_color, 1.0, x, lr.y + 1, x + w, y + h);
            manager.win_draw_line(line_color, 1.0, x, ul.y, x + w, ul.y);
            manager.win_draw_line(line_color, 1.0, x, lr.y + 1, x + w, lr.y + 1);
        } else {
            manager.win_fill_rect(fill_color, 1.0, x, y, ul.x - 1, y + h);
            manager.win_fill_rect(fill_color, 1.0, lr.x + 1, y, x + w, y + h);
            manager.win_draw_line(line_color, 1.0, ul.x, y, ul.x, y + h);
            manager.win_draw_line(line_color, 1.0, lr.x + 1, y, lr.x + 1, y + h);
        }
    });

    if let Some(draw) = window.get_enabled_draw_data(0) {
        if window.get_status().contains(WindowStatus::IMAGE) {
            if let Some(image) = draw.image {
                with_window_manager_ref(|manager| {
                    manager.win_draw_image(&image, ul.x, ul.y, lr.x, lr.y, draw.color);
                });
            } else {
                with_window_manager_ref(|manager| {
                    manager.win_fill_rect(line_color, 1.0, ul.x, ul.y, lr.x, lr.y);
                });
            }
        } else {
            with_window_manager_ref(|manager| {
                manager.win_fill_rect(line_color, 1.0, ul.x, ul.y, lr.x, lr.y);
            });
        }
    }

    let supply_and_tech = get_supply_and_tech_image_locations();
    let overlay = supply_and_tech.lock().unwrap_or_else(|e| e.into_inner());
    with_window_manager_ref(|manager| {
        if let Some(image) = manager.win_find_image("TecBuilding") {
            for pos in &overlay.tech_positions {
                manager.win_draw_image(
                    &image,
                    x + pos.x,
                    y + pos.y,
                    x + pos.x + SUPPLY_TECH_SIZE,
                    y + pos.y + SUPPLY_TECH_SIZE,
                    0xFFFFFFFF,
                );
            }
        }
        if let Some(image) = manager.win_find_image("Cash") {
            for pos in &overlay.supply_positions {
                manager.win_draw_image(
                    &image,
                    x + pos.x,
                    y + pos.y,
                    x + pos.x + SUPPLY_TECH_SIZE,
                    y + pos.y + SUPPLY_TECH_SIZE,
                    0xFFFFFFFF,
                );
            }
        }
    });

    draw_skinny_border(x - 1, y - 1, w + 2, h + 2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_image_name_falls_back_to_unknown_map() {
        assert_eq!(resolve_preview_image_name(None), UNKNOWN_MAP_IMAGE);
        assert_eq!(
            resolve_preview_image_name(Some(String::new())),
            UNKNOWN_MAP_IMAGE
        );
        assert_eq!(
            resolve_preview_image_name(Some("SkirmishPreview".to_string())),
            "SkirmishPreview"
        );
    }

    #[test]
    fn map_select_parent_name_preserves_legacy_typo() {
        assert_eq!(
            MAP_SELECT_PARENT_NAME,
            "SkirmishMapSelectMenu.wnd:SkrimishMapSelectMenuParent"
        );
    }
}
