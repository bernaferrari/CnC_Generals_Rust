//! LanGameOptionsMenu.cpp callback port.
//!
//! Uses thread-local RefCell for window state (idiomatic Rust for single-threaded GUI)
//! and Cell<bool> global flags matching C++ statics (via mod.rs helpers).

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::gui::game_window::Image as WindowImage;
use crate::gui::{
    get_lan_setup, get_shell, with_window_manager, GameWindow, WindowLayout, WindowMessage,
    WindowMsgData, WindowMsgHandled, WindowStatus, GLM_RIGHT_CLICKED,
};
use crate::map_util::{find_draw_positions, get_map_cache_manager, get_map_preview_image};
use game_engine::common::ini::ini_map_cache::MapMetaData;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_network::matchmaking::slots::PlayerColor;
use game_network::{Money, SlotState, PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM};
use gamelogic::helpers::TheGameLogic;
use gamelogic::system::game_logic::GAME_LAN;
use std::cell::RefCell;
use std::rc::Rc;

// Import global state helpers from mod.rs - matches C++ static globals
use super::{
    lan_button_pushed, lan_is_initing, lan_is_shutting_down, lan_slot_updates_enabled,
    set_lan_button_pushed, set_lan_is_initing, set_lan_is_shutting_down,
    set_lan_slot_updates_enabled,
};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const MAX_SLOTS: usize = 8;

/// Window state stored in thread-local RefCell (single-threaded GUI)
/// This replaces Arc<Mutex> with zero-overhead RefCell
#[derive(Default)]
struct LanGameOptionsState {
    parent_id: i32,
    button_start_id: i32,
    button_back_id: i32,
    button_select_map_id: i32,
    button_emote_id: i32,
    button_accept_ids: [i32; MAX_SLOTS],
    text_entry_chat_id: i32,
    text_entry_map_display_id: i32,
    listbox_chat_id: i32,
    map_window_id: i32,
    checkbox_limit_superweapons_id: i32,
    combo_box_starting_cash_id: i32,
    start_position_ids: [i32; MAX_SLOTS],
    combo_box_player_ids: [i32; MAX_SLOTS],
    combo_box_color_ids: [i32; MAX_SLOTS],
    combo_box_team_ids: [i32; MAX_SLOTS],
    combo_box_template_ids: [i32; MAX_SLOTS],
    parent: Option<Rc<RefCell<GameWindow>>>,
    map_window: Option<Rc<RefCell<GameWindow>>>,
    text_entry_map_display: Option<Rc<RefCell<GameWindow>>>,
    listbox_chat: Option<Rc<RefCell<GameWindow>>>,
    selected_map: Option<String>,
}

/// Thread-local state using RefCell - zero-overhead for single-threaded GUI
thread_local! {
    static LAN_GAME_OPTIONS_STATE: RefCell<LanGameOptionsState> =
        RefCell::new(LanGameOptionsState::default());
    static LAN_MAP_SELECT_LAYOUT: RefCell<Option<Rc<RefCell<WindowLayout>>>> =
        const { RefCell::new(None) };
}

/// Access state with closure - panic on borrow conflict (indicates bug)
fn with_state<R>(f: impl FnOnce(&mut LanGameOptionsState) -> R) -> R {
    LAN_GAME_OPTIONS_STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Access state immutably
fn with_state_ref<R>(f: impl FnOnce(&LanGameOptionsState) -> R) -> R {
    LAN_GAME_OPTIONS_STATE.with(|s| f(&s.borrow()))
}

fn with_lan_map_select_layout<R>(f: impl FnOnce(&mut Option<Rc<RefCell<WindowLayout>>>) -> R) -> R {
    LAN_MAP_SELECT_LAYOUT.with(|layout| f(&mut layout.borrow_mut()))
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
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

fn map_start_waypoint_name(index: usize) -> String {
    format!("Player_{}_Start", index + 1)
}

fn format_game_text_number(key: &str, value: i32) -> String {
    let template = GameText::fetch(key);
    if template.contains("%d") {
        template.replace("%d", &value.to_string())
    } else {
        format!("{} {}", template, value)
    }
}

fn position_start_buttons(state: &mut LanGameOptionsState, meta: Option<&MapMetaData>) {
    let Some(map_window) = state.map_window.as_ref() else {
        return;
    };
    let map_guard = map_window.borrow();
    let (map_x, map_y) = map_guard.get_screen_position();
    let (map_w, map_h) = map_guard.get_size();

    let extent = meta.map(|meta| meta.extent).unwrap_or_default();
    let (ul, lr) = find_draw_positions(map_x, map_y, map_w, map_h, extent);
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
            let new_x = draw_x.round() as i32 - btn_w / 2 - map_x;
            let new_y = draw_y.round() as i32 - btn_h / 2 - map_y;
            let _ = button_guard.set_position(new_x, new_y);
            let _ = button_guard.hide(false);
            let _ = button_guard.enable(true);
        } else {
            let _ = button_guard.hide(true);
            let _ = button_guard.enable(false);
        }
    }
}

fn update_map_start_spots(state: &mut LanGameOptionsState, meta: Option<&MapMetaData>) {
    for button_id in state.start_position_ids {
        if let Some(button) = with_window_manager(|manager| manager.get_window_by_id(button_id)) {
            let mut guard = button.borrow_mut();
            let _ = guard.set_text("");
            guard.set_tooltip(&GameText::fetch("TOOLTIP:StartPosition"));
        }
    }

    let Some(meta) = meta else {
        return;
    };
    let max_players = meta.num_players.max(0);
    let setup = get_lan_setup();
    let info = setup.game_info();
    for i in 0..MAX_SLOTS {
        let Some(slot) = info.get_slot(i) else {
            continue;
        };
        let pos = slot.get_start_pos();
        if pos >= 0 && pos < max_players && slot.get_player_template() > PLAYERTEMPLATE_OBSERVER {
            let button_id = state.start_position_ids[pos as usize];
            if let Some(button) = with_window_manager(|manager| manager.get_window_by_id(button_id))
            {
                let mut guard = button.borrow_mut();
                let number_key = format!("NUMBER:{}", i + 1);
                let label = GameText::fetch(&number_key);
                let _ = guard.set_text(&label);
                let tooltip = format_game_text_number("TOOLTIP:StartPositionN", (i + 1) as i32);
                guard.set_tooltip(&tooltip);
            }
        }
    }
}

fn map_display_name(map_name: &str, meta: Option<&MapMetaData>) -> String {
    if let Some(meta) = meta {
        if !meta.display_name.is_empty() {
            return meta.display_name.as_str().to_string();
        }
    }
    map_name
        .rsplit('/')
        .next()
        .unwrap_or(map_name)
        .trim_end_matches(".map")
        .to_string()
}

fn update_map_preview(state: &mut LanGameOptionsState) {
    let Some(map_name) = state.selected_map.clone() else {
        return;
    };
    let preview_name = get_map_preview_image(&map_name).unwrap_or_default();
    set_window_image(&state.map_window, &preview_name);
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let meta = cache_guard.find_map(&map_name);
    position_start_buttons(state, meta.as_ref());
    update_map_start_spots(state, meta.as_ref());
    if let Some(text_entry) = state.text_entry_map_display.as_ref() {
        if let Some(widget) = text_entry.borrow_mut().static_text_mut() {
            let label = map_display_name(&map_name, meta.as_ref());
            widget.set_text(label);
        }
    }
}

fn sync_map_metadata(map_name: &str) {
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let mut setup = get_lan_setup();
    let info = setup.game_info_mut();
    if let Some(meta) = cache_guard.find_map(map_name) {
        info.set_map_crc(meta.crc);
        info.set_map_size(meta.filesize);
    } else {
        info.set_map_crc(0);
        info.set_map_size(0);
    }
}

fn sync_map_to_game_info(state: &LanGameOptionsState) {
    if let Some(map) = state.selected_map.as_ref() {
        {
            let mut setup = get_lan_setup();
            let info = setup.game_info_mut();
            info.set_map(map.clone());
            info.reset_start_spots();
            info.adjust_slots_for_map();
        }
        sync_map_metadata(map);
    }
}

pub(crate) fn show_lan_game_options_underlying_gui_elements(show: bool) {
    const LAYOUT: &str = "LanGameOptionsMenu.wnd:";
    let gadgets = [
        "MapWindow",
        "TextEntryMapDisplay",
        "ButtonSelectMap",
        "ButtonStart",
        "ButtonEmote",
        "TextEntryChat",
        "ListboxChatWindowLanGame",
        "CheckboxLimitSuperweapons",
        "ComboBoxStartingCash",
    ];

    with_window_manager(|manager| {
        for name in gadgets {
            let id = name_to_id(&format!("{LAYOUT}{name}"));
            if let Some(win) = manager.get_window_by_id(id) {
                let mut guard = win.borrow_mut();
                let _ = guard.hide(!show);
                let _ = guard.enable(show);
            }
        }

        for i in 0..MAX_SLOTS {
            for base in [
                "ButtonAccept",
                "ButtonMapStartPosition",
                "ComboBoxPlayer",
                "ComboBoxColor",
                "ComboBoxTeam",
                "ComboBoxPlayerTemplate",
            ] {
                let id = name_to_id(&format!("{LAYOUT}{base}{i}"));
                if let Some(win) = manager.get_window_by_id(id) {
                    let mut guard = win.borrow_mut();
                    let _ = guard.hide(!show);
                    let _ = guard.enable(show);
                }
            }
        }

        if let Some(win) = manager.get_window_by_id(name_to_id("LanGameOptionsMenu.wnd:ButtonBack"))
        {
            let _ = win.borrow_mut().enable(show);
        }
    });
}

pub(crate) fn destroy_lan_map_select_overlay() {
    with_lan_map_select_layout(|layout| {
        let Some(layout) = layout.take() else {
            return;
        };
        with_window_manager(|manager| manager.destroy_layout(&layout));
    });
}

pub(crate) fn refresh_lan_game_options_from_setup() {
    with_state(|state| {
        let selected_map = {
            let setup = get_lan_setup();
            setup.selected_map().to_string()
        };

        if !selected_map.is_empty() {
            state.selected_map = Some(selected_map);
        }
        update_map_preview(state);
        sync_map_to_game_info(state);
        lan_update_slot_list(state);
    });
}

fn ensure_default_slots() {
    let mut setup = get_lan_setup();
    let info = setup.game_info_mut();
    let has_player = info
        .get_slot(0)
        .map(|slot| slot.is_occupied())
        .unwrap_or(false);
    if !has_player {
        if let Some(slot) = info.get_slot_mut(0) {
            slot.set_state(SlotState::Player, GameText::fetch("GUI:Player"), 0);
            slot.set_color(0);
        }
        for index in 1..MAX_SLOTS {
            if let Some(slot) = info.get_slot_mut(index) {
                slot.set_state(SlotState::Open, String::new(), 0);
            }
        }
    }
}

fn combo_selected_id(window_id: i32) -> Option<u32> {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).and_then(|window| {
        let guard = window.borrow();
        guard.widget().and_then(|widget| match widget {
            crate::gui::WindowWidget::ComboBox(combo) => combo.selected_id(),
            _ => None,
        })
    })
}

fn checkbox_is_checked(window_id: i32) -> Option<bool> {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).and_then(|window| {
        let guard = window.borrow();
        guard.widget().and_then(|widget| match widget {
            crate::gui::WindowWidget::CheckBox(check) => Some(check.is_checked()),
            _ => None,
        })
    })
}

fn populate_player_combo(window_id: i32, is_local: bool) {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).map(|window| {
        let mut guard = window.borrow_mut();
        let Some(combo) = guard.combo_box_mut() else {
            return;
        };
        combo.clear();
        if is_local {
            combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
                SlotState::Player as u32,
                GameText::fetch("GUI:Player"),
            ));
        } else {
            for state in [
                SlotState::Open,
                SlotState::Closed,
                SlotState::EasyAI,
                SlotState::MedAI,
                SlotState::BrutalAI,
            ] {
                let label = match state {
                    SlotState::Open => GameText::fetch("GUI:Open"),
                    SlotState::Closed => GameText::fetch("GUI:Closed"),
                    SlotState::EasyAI => GameText::fetch("GUI:EasyAI"),
                    SlotState::MedAI => GameText::fetch("GUI:MediumAI"),
                    SlotState::BrutalAI => GameText::fetch("GUI:HardAI"),
                    _ => String::new(),
                };
                combo.add_item(crate::gui::gadgets::ComboBoxItem::new(state as u32, label));
            }
        }
    });
}

fn populate_color_combo(window_id: i32) {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).map(|window| {
        let mut guard = window.borrow_mut();
        let Some(combo) = guard.combo_box_mut() else {
            return;
        };
        combo.clear();
        combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
            u32::MAX,
            GameText::fetch("GUI:Random"),
        ));
        for color in PlayerColor::all() {
            let label = GameText::fetch(&format!("GUI:{}", color.name()));
            let display = if label.starts_with("GUI:") {
                color.name().to_string()
            } else {
                label
            };
            combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
                *color as u32,
                display,
            ));
        }
    });
}

fn populate_template_combo(window_id: i32) {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).map(|window| {
        let mut guard = window.borrow_mut();
        let Some(combo) = guard.combo_box_mut() else {
            return;
        };
        combo.clear();
        combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
            PLAYERTEMPLATE_RANDOM as u32,
            GameText::fetch("GUI:Random"),
        ));
        combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
            PLAYERTEMPLATE_OBSERVER as u32,
            GameText::fetch("GUI:Observer"),
        ));
        let store = game_engine::common::rts::player_template::get_player_template_store();
        for index in 0..store.len() {
            if let Some(template) = store.get_nth_player_template(index) {
                if !template.playable {
                    continue;
                }
                combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
                    index as u32,
                    template.get_display_name().to_string(),
                ));
            }
        }
    });
}

fn populate_team_combo(window_id: i32) {
    with_window_manager(|manager| manager.get_window_by_id(window_id)).map(|window| {
        let mut guard = window.borrow_mut();
        let Some(combo) = guard.combo_box_mut() else {
            return;
        };
        combo.clear();
        combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
            0,
            "None".to_string(),
        ));
        let base_label = GameText::fetch("GUI:Team");
        for i in 1..=MAX_SLOTS {
            let label = if base_label.starts_with("GUI:") {
                format!("Team {}", i)
            } else {
                format!("{} {}", base_label, i)
            };
            combo.add_item(crate::gui::gadgets::ComboBoxItem::new(i as u32, label));
        }
    });
}

fn populate_slot_controls(state: &mut LanGameOptionsState) {
    for i in 0..MAX_SLOTS {
        populate_player_combo(state.combo_box_player_ids[i], i == 0);
        populate_color_combo(state.combo_box_color_ids[i]);
        populate_template_combo(state.combo_box_template_ids[i]);
        populate_team_combo(state.combo_box_team_ids[i]);
        update_slot_selection(state, i);
    }
}

fn update_slot_selection(state: &mut LanGameOptionsState, index: usize) {
    let setup = get_lan_setup();
    let Some(slot) = setup.game_info().get_slot(index) else {
        return;
    };
    let player_id = slot.get_state() as u32;
    let color_id = if slot.get_color() < 0 {
        u32::MAX
    } else {
        slot.get_color() as u32
    };
    let template_id = slot.get_player_template();
    let template_id = if template_id < 0 {
        template_id as u32
    } else {
        template_id as u32
    };
    let team_id = if slot.get_team_number() < 0 {
        0
    } else {
        (slot.get_team_number() + 1) as u32
    };

    for (id, value) in [
        (state.combo_box_player_ids[index], player_id),
        (state.combo_box_color_ids[index], color_id),
        (state.combo_box_template_ids[index], template_id),
        (state.combo_box_team_ids[index], team_id),
    ] {
        with_window_manager(|manager| manager.get_window_by_id(id)).map(|window| {
            let mut guard = window.borrow_mut();
            if let Some(combo) = guard.combo_box_mut() {
                let _ = combo.select_item(value);
            }
        });
    }

    let is_observer = slot.get_player_template() == PLAYERTEMPLATE_OBSERVER;
    let enable = slot.get_state() != SlotState::Closed && !is_observer;
    for id in [
        state.combo_box_color_ids[index],
        state.combo_box_team_ids[index],
    ] {
        if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(id)) {
            let _ = window.borrow_mut().enable(enable);
        }
    }

    if let Some(window) =
        with_window_manager(|manager| manager.get_window_by_id(state.button_accept_ids[index]))
    {
        let mut guard = window.borrow_mut();
        let label = if slot.is_accepted() {
            GameText::fetch("GUI:Cancel")
        } else {
            GameText::fetch("GUI:Accept")
        };
        let _ = guard.set_text(&label);
    }
}

fn populate_global_controls() {
    if let Some(window) = with_window_manager(|manager| {
        manager.get_window_by_id(name_to_id("LanGameOptionsMenu.wnd:ComboBoxStartingCash"))
    }) {
        let mut guard = window.borrow_mut();
        if let Some(combo) = guard.combo_box_mut() {
            combo.clear();
            for amount in [10000u32, 20000, 30000, 40000, 50000] {
                combo.add_item(crate::gui::gadgets::ComboBoxItem::new(
                    amount,
                    format!("${}", amount),
                ));
            }
            let setup = get_lan_setup();
            let selected = setup.game_info().get_starting_cash().count_money();
            let _ = combo.select_item(selected);
        }
    }

    if let Some(window) = with_window_manager(|manager| {
        manager.get_window_by_id(name_to_id(
            "LanGameOptionsMenu.wnd:CheckboxLimitSuperweapons",
        ))
    }) {
        let mut guard = window.borrow_mut();
        let setup = get_lan_setup();
        let enabled = setup.game_info().get_superweapon_restriction() != 0;
        let _ = guard.gadget_check_box_set_checked(enabled);
    }
}

/// Update slot list - matches C++ lanUpdateSlotList with guard checks
fn lan_update_slot_list(state: &mut LanGameOptionsState) {
    // Guard checks matching C++: if(!AreSlotListUpdatesEnabled() || s_isIniting) return;
    if !lan_slot_updates_enabled() || lan_is_initing() {
        return;
    }

    for i in 0..MAX_SLOTS {
        update_slot_selection(state, i);
    }
    let map_name = {
        let setup = get_lan_setup();
        setup.game_info().get_map().to_string()
    };
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let meta = cache_guard.find_map(&map_name);
    update_map_start_spots(state, meta.as_ref());
}

fn choose_default_map(state: &mut LanGameOptionsState) {
    if state.selected_map.is_some() {
        return;
    }
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let mut candidates: Vec<_> = cache_guard
        .iter_maps()
        .into_iter()
        .filter(|(_, meta)| meta.is_multiplayer)
        .collect();
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    if let Some((name, _)) = candidates.first() {
        state.selected_map = Some(name.clone());
        let mut setup = get_lan_setup();
        setup.set_selected_map(name.clone());
    }
}

fn next_selectable_player(info: &game_network::GameInfo, start: usize) -> Option<usize> {
    if !info.am_i_host() {
        return None;
    }
    for index in start..MAX_SLOTS {
        if let Some(slot) = info.get_slot(index) {
            if slot.get_start_pos() == -1
                && (index as i32 == info.get_local_slot_num()
                    || (slot.is_ai() && slot.get_player_template() != PLAYERTEMPLATE_OBSERVER))
            {
                return Some(index);
            }
        }
    }
    None
}

fn set_start_position(info: &mut game_network::GameInfo, index: usize, position: i32) {
    let Some(current_pos) = info.get_slot(index).map(|slot| slot.get_start_pos()) else {
        return;
    };
    if position == current_pos {
        return;
    }
    if position >= 0 && info.is_start_position_taken(position, index as i32) {
        return;
    }
    if let Some(slot) = info.get_slot_mut(index) {
        slot.set_start_pos(if position < 0 { -1 } else { position });
    }
}

fn handle_start_position_click(state: &mut LanGameOptionsState, control_id: i32) -> bool {
    for (index, button_id) in state.start_position_ids.iter().enumerate() {
        if control_id == *button_id {
            let mut setup = get_lan_setup();
            let info = setup.game_info_mut();
            let mut player_in_pos: Option<usize> = None;
            for slot_index in 0..MAX_SLOTS {
                if let Some(slot) = info.get_slot(slot_index) {
                    if slot.get_start_pos() == index as i32 {
                        player_in_pos = Some(slot_index);
                        break;
                    }
                }
            }
            if let Some(player_idx) = player_in_pos {
                let is_local = player_idx as i32 == info.get_local_slot_num();
                let can_move = is_local
                    || (info.am_i_host()
                        && info
                            .get_slot(player_idx)
                            .map(|slot| slot.is_ai())
                            .unwrap_or(false));
                if can_move {
                    let next_player = next_selectable_player(info, player_idx + 1);
                    set_start_position(info, player_idx, -1);
                    if let Some(next) = next_player {
                        set_start_position(info, next, index as i32);
                    }
                }
            } else {
                let mut next_player = next_selectable_player(info, 0);
                if next_player.is_none() {
                    let local = info.get_local_slot_num();
                    if local >= 0 {
                        next_player = Some(local as usize);
                    }
                }
                if let Some(next) = next_player {
                    set_start_position(info, next, index as i32);
                }
            }
            return true;
        }
    }
    false
}

fn handle_start_position_right_click(state: &mut LanGameOptionsState, control_id: i32) -> bool {
    for (index, button_id) in state.start_position_ids.iter().enumerate() {
        if control_id == *button_id {
            let mut setup = get_lan_setup();
            let info = setup.game_info_mut();
            let mut player_in_pos: Option<usize> = None;
            for slot_index in 0..MAX_SLOTS {
                if let Some(slot) = info.get_slot(slot_index) {
                    if slot.get_start_pos() == index as i32 {
                        player_in_pos = Some(slot_index);
                        break;
                    }
                }
            }
            if let Some(player_idx) = player_in_pos {
                let is_local = player_idx as i32 == info.get_local_slot_num();
                let can_move = is_local
                    || (info.am_i_host()
                        && info
                            .get_slot(player_idx)
                            .map(|slot| slot.is_ai())
                            .unwrap_or(false));
                if can_move {
                    set_start_position(info, player_idx, -1);
                }
            }
            return true;
        }
    }
    false
}

fn handle_combo_selection(state: &mut LanGameOptionsState, control_id: i32) -> bool {
    for index in 0..MAX_SLOTS {
        if control_id == state.combo_box_player_ids[index] {
            if index == 0 {
                return true;
            }
            if let Some(selected) = combo_selected_id(control_id) {
                let mut setup = get_lan_setup();
                if let Some(slot) = setup.game_info_mut().get_slot_mut(index) {
                    let state_value = match selected {
                        x if x == SlotState::Open as u32 => SlotState::Open,
                        x if x == SlotState::Closed as u32 => SlotState::Closed,
                        x if x == SlotState::EasyAI as u32 => SlotState::EasyAI,
                        x if x == SlotState::MedAI as u32 => SlotState::MedAI,
                        x if x == SlotState::BrutalAI as u32 => SlotState::BrutalAI,
                        _ => SlotState::Open,
                    };
                    slot.set_state(state_value, String::new(), 0);
                    setup.game_info_mut().reset_accepted();
                }
                update_slot_selection(state, index);
            }
            return true;
        }
        if control_id == state.combo_box_color_ids[index] {
            if let Some(selected) = combo_selected_id(control_id) {
                let mut setup = get_lan_setup();
                let color = if selected == u32::MAX {
                    -1
                } else {
                    selected as i32
                };
                let color_taken =
                    color >= 0 && setup.game_info().is_color_taken(color, index as i32);
                if let Some(slot) = setup.game_info_mut().get_slot_mut(index) {
                    if color_taken {
                        update_slot_selection(state, index);
                    } else {
                        slot.set_color(color);
                        setup.game_info_mut().reset_accepted();
                    }
                }
            }
            return true;
        }
        if control_id == state.combo_box_template_ids[index] {
            if let Some(selected) = combo_selected_id(control_id) {
                let mut setup = get_lan_setup();
                if let Some(slot) = setup.game_info_mut().get_slot_mut(index) {
                    let template = selected as i32;
                    if template != slot.get_player_template() {
                        slot.set_player_template(template);
                        if template == PLAYERTEMPLATE_OBSERVER {
                            slot.set_start_pos(-1);
                            slot.set_color(-1);
                            slot.set_team_number(-1);
                        }
                        setup.game_info_mut().reset_accepted();
                    }
                }
            }
            return true;
        }
        if control_id == state.combo_box_team_ids[index] {
            if let Some(selected) = combo_selected_id(control_id) {
                let mut setup = get_lan_setup();
                if let Some(slot) = setup.game_info_mut().get_slot_mut(index) {
                    let team = if selected == 0 {
                        -1
                    } else {
                        selected as i32 - 1
                    };
                    slot.set_team_number(team);
                    setup.game_info_mut().reset_accepted();
                }
            }
            return true;
        }
    }

    if control_id == state.combo_box_starting_cash_id {
        if let Some(selected) = combo_selected_id(control_id) {
            let mut setup = get_lan_setup();
            setup
                .game_info_mut()
                .set_starting_cash(Money::new(selected));
            setup.game_info_mut().reset_accepted();
        }
        return true;
    }
    if control_id == state.checkbox_limit_superweapons_id {
        if let Some(checked) = checkbox_is_checked(control_id) {
            let mut setup = get_lan_setup();
            setup
                .game_info_mut()
                .set_superweapon_restriction(if checked { 1 } else { 0 });
            setup.game_info_mut().reset_accepted();
        }
        return true;
    }

    false
}

fn handle_accept_click(state: &mut LanGameOptionsState, control_id: i32) -> bool {
    for index in 0..MAX_SLOTS {
        if control_id == state.button_accept_ids[index] {
            let mut setup = get_lan_setup();
            if let Some(slot) = setup.game_info_mut().get_slot_mut(index) {
                if slot.is_accepted() {
                    slot.un_accept();
                } else {
                    slot.set_accept();
                }
            }
            return true;
        }
    }
    false
}

fn start_lan_game(state: &mut LanGameOptionsState) {
    let Some(map_name) = state.selected_map.clone() else {
        return;
    };
    sync_map_to_game_info(state);
    {
        let setup = get_lan_setup();
        let info = setup.game_info();
        let mut num_users = 0;
        let mut num_humans = 0;
        for i in 0..MAX_SLOTS {
            if let Some(slot) = info.get_slot(i) {
                if slot.is_occupied() && slot.get_player_template() != PLAYERTEMPLATE_OBSERVER {
                    num_users += 1;
                    if slot.is_human() {
                        num_humans += 1;
                    }
                }
            }
        }
        let min_players = game_engine::common::ini::get_global_data()
            .map(|data| data.read().net_min_players)
            .unwrap_or(2)
            .max(1);
        let cache = get_map_cache_manager();
        let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(meta) = cache_guard.find_map(&map_name) {
            if meta.num_players > 0 && num_users > meta.num_players as usize {
                return;
            }
        }
        if min_players > 0 && num_humans == 0 {
            return;
        }
        if num_users < min_players as usize {
            return;
        }
        let mut teams = std::collections::HashSet::new();
        let mut random_teams = 0;
        for i in 0..MAX_SLOTS {
            if let Some(slot) = info.get_slot(i) {
                if slot.is_occupied() && slot.get_player_template() != PLAYERTEMPLATE_OBSERVER {
                    if slot.get_team_number() >= 0 {
                        teams.insert(slot.get_team_number());
                    } else {
                        random_teams += 1;
                    }
                }
            }
        }
        if (random_teams + teams.len()) < min_players as usize {
            return;
        }
    }
    {
        let mut setup = get_lan_setup();
        setup.game_info_mut().start_game(0);
    }
    if let Some(data) = game_engine::common::ini::get_global_data() {
        let mut data = data.write();
        data.pending_file = map_name;
    }
    let mut shell = get_shell();
    let _ = shell.pop();
    let _ = shell.hide_shell();
    TheGameLogic::prepare_new_game(GAME_LAN, 1, 0);
}

pub fn lan_game_options_menu_init(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    // Set initing flag - matches C++ s_isIniting = TRUE
    set_lan_is_initing(true);
    set_lan_button_pushed(false);
    set_lan_is_shutting_down(false);

    with_state(|state| {
        state.parent_id = name_to_id("LanGameOptionsMenu.wnd:LanGameOptionsMenuParent");
        state.button_start_id = name_to_id("LanGameOptionsMenu.wnd:ButtonStart");
        state.button_back_id = name_to_id("LanGameOptionsMenu.wnd:ButtonBack");
        state.button_select_map_id = name_to_id("LanGameOptionsMenu.wnd:ButtonSelectMap");
        state.button_emote_id = name_to_id("LanGameOptionsMenu.wnd:ButtonEmote");
        state.text_entry_chat_id = name_to_id("LanGameOptionsMenu.wnd:TextEntryChat");
        state.text_entry_map_display_id = name_to_id("LanGameOptionsMenu.wnd:TextEntryMapDisplay");
        state.listbox_chat_id = name_to_id("LanGameOptionsMenu.wnd:ListboxChatWindowLanGame");
        state.map_window_id = name_to_id("LanGameOptionsMenu.wnd:MapWindow");
        state.checkbox_limit_superweapons_id =
            name_to_id("LanGameOptionsMenu.wnd:CheckboxLimitSuperweapons");
        state.combo_box_starting_cash_id =
            name_to_id("LanGameOptionsMenu.wnd:ComboBoxStartingCash");
        for i in 0..MAX_SLOTS {
            state.start_position_ids[i] = name_to_id(&format!(
                "LanGameOptionsMenu.wnd:ButtonMapStartPosition{}",
                i
            ));
            state.button_accept_ids[i] =
                name_to_id(&format!("LanGameOptionsMenu.wnd:ButtonAccept{}", i));
            state.combo_box_player_ids[i] =
                name_to_id(&format!("LanGameOptionsMenu.wnd:ComboBoxPlayer{}", i));
            state.combo_box_color_ids[i] =
                name_to_id(&format!("LanGameOptionsMenu.wnd:ComboBoxColor{}", i));
            state.combo_box_template_ids[i] = name_to_id(&format!(
                "LanGameOptionsMenu.wnd:ComboBoxPlayerTemplate{}",
                i
            ));
            state.combo_box_team_ids[i] =
                name_to_id(&format!("LanGameOptionsMenu.wnd:ComboBoxTeam{}", i));
        }

        with_window_manager(|manager| {
            state.parent = manager.get_window_by_id(state.parent_id);
            state.map_window = manager.get_window_by_id(state.map_window_id);
            state.text_entry_map_display =
                manager.get_window_by_id(state.text_entry_map_display_id);
            state.listbox_chat = manager.get_window_by_id(state.listbox_chat_id);
        });

        {
            let setup = get_lan_setup();
            if !setup.selected_map().is_empty() {
                state.selected_map = Some(setup.selected_map().to_string());
            }
        }

        // Disable slot list updates during init - matches C++ EnableSlotListUpdates(FALSE)
        set_lan_slot_updates_enabled(false);
        ensure_default_slots();
        populate_slot_controls(state);
        populate_global_controls();
        set_lan_slot_updates_enabled(true);

        choose_default_map(state);
        sync_map_to_game_info(state);
        update_map_preview(state);
        lan_update_slot_list(state);
    });

    layout.hide(false);

    // Clear initing flag - matches C++ s_isIniting = FALSE
    set_lan_is_initing(false);
}

pub fn lan_game_options_menu_update(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    // Check shutdown - matches C++ pattern
    if lan_is_shutting_down()
        && get_shell().is_anim_finished()
        && with_window_manager(|manager| manager.transitions_finished())
    {
        // Clear window refs before shutdown complete
        with_state(|state| {
            state.parent = None;
            state.map_window = None;
            state.text_entry_map_display = None;
            state.listbox_chat = None;
        });
        set_lan_is_shutting_down(false);
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
        return;
    }

    // Check for map changes
    let setup_map = {
        let setup = get_lan_setup();
        setup.selected_map().to_string()
    };

    with_state(|state| {
        if !setup_map.is_empty() && state.selected_map.as_deref() != Some(setup_map.as_str()) {
            state.selected_map = Some(setup_map);
            update_map_preview(state);
            sync_map_to_game_info(state);
            lan_update_slot_list(state);
        }
    });
}

pub fn lan_game_options_menu_shutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);

    if pop_immediate {
        // Clear window refs
        with_state(|state| {
            state.parent = None;
            state.map_window = None;
            state.text_entry_map_display = None;
            state.listbox_chat = None;
        });
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
        return;
    }

    set_lan_is_shutting_down(true);
    get_shell().reverse_animate_window();
    with_window_manager(|manager| manager.transition_reverse("LanGameOptionsFade"));
}

pub fn lan_game_options_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    let mut handled = false;
    with_state(|state| match msg {
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == state.button_start_id {
                start_lan_game(state);
                handled = true;
                return;
            }
            if control_id == state.button_back_id {
                let _ = get_shell().pop();
                handled = true;
                return;
            }
            if control_id == state.button_select_map_id {
                destroy_lan_map_select_overlay();
                if let Some((layout, _)) = with_window_manager(|manager| {
                    manager
                        .create_layout_with_windows("Menus/LanMapSelectMenu.wnd")
                        .ok()
                }) {
                    with_lan_map_select_layout(|current| *current = Some(layout.clone()));
                    layout.borrow().run_init(None);
                    let mut layout_mut = layout.borrow_mut();
                    layout_mut.hide(false);
                    layout_mut.bring_forward();
                }
                handled = true;
                return;
            }
            if handle_accept_click(state, control_id) {
                lan_update_slot_list(state);
                handled = true;
                return;
            }
            if handle_combo_selection(state, control_id) {
                lan_update_slot_list(state);
                handled = true;
                return;
            }
            if handle_start_position_click(state, control_id) {
                lan_update_slot_list(state);
                handled = true;
            }
        }
        WindowMessage::GadgetValueChanged => {
            let control_id = data1 as i32;
            if handle_combo_selection(state, control_id) {
                lan_update_slot_list(state);
                handled = true;
            }
        }
        WindowMessage::GadgetRightClick | WindowMessage::User(GLM_RIGHT_CLICKED) => {
            let control_id = data1 as i32;
            if handle_start_position_right_click(state, control_id) {
                lan_update_slot_list(state);
                handled = true;
            }
        }
        _ => {}
    });

    if handled {
        WindowMsgHandled::Handled
    } else {
        WindowMsgHandled::Ignored
    }
}

pub fn lan_game_options_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }

    if lan_button_pushed() {
        return WindowMsgHandled::Ignored;
    }

    if (data2 & KEY_STATE_UP) != 0 {
        with_state_ref(|state| {
            if let Some(parent) = state.parent.as_ref() {
                let _ = parent.borrow_mut().send_system_message(
                    WindowMessage::GadgetSelected,
                    state.button_back_id as WindowMsgData,
                    state.button_back_id as WindowMsgData,
                );
            }
        });
    }

    WindowMsgHandled::Handled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_char_is_consumed_before_key_up_like_cpp() {
        let window = GameWindow::new();
        set_lan_button_pushed(false);

        assert_eq!(
            lan_game_options_menu_input(&window, WindowMessage::Char, KEY_ESC as WindowMsgData, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            lan_game_options_menu_input(&window, WindowMessage::Char, b'A' as WindowMsgData, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn esc_char_is_ignored_after_button_pushed_like_cpp() {
        let window = GameWindow::new();
        set_lan_button_pushed(true);

        assert_eq!(
            lan_game_options_menu_input(
                &window,
                WindowMessage::Char,
                KEY_ESC as WindowMsgData,
                KEY_STATE_UP,
            ),
            WindowMsgHandled::Ignored
        );

        set_lan_button_pushed(false);
    }

    #[test]
    fn glm_right_clicked_routes_start_position_like_cpp() {
        let window = GameWindow::new();
        with_state(|state| {
            *state = LanGameOptionsState::default();
            state.start_position_ids[0] = 77;
        });

        assert_eq!(
            lan_game_options_menu_system(&window, WindowMessage::User(GLM_RIGHT_CLICKED), 77, 0),
            WindowMsgHandled::Handled
        );

        with_state(|state| *state = LanGameOptionsState::default());
    }
}
