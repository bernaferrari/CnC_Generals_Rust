//! WOLQuickMatchMenu.cpp callback port.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::gamespy_game::{with_gamespy_game_info, with_gamespy_game_info_mut};
use crate::gamespy_overlay::{
    close_all_overlays, close_overlay, gs_message_box_ok, open_overlay, raise_gs_message_box,
    toggle_overlay, GameSpyOverlayType,
};
use crate::gui::callbacks::wol_buddy_overlay::handle_buddy_responses;
use crate::gui::callbacks::wol_welcome_menu::populate_player_info_windows;
use crate::gui::challenge_generals::get_challenge_generals;
use crate::gui::gadgets::{ComboBox, ComboBoxItem, ListBox, ListBoxItemData};
use crate::gui::{
    get_shell, with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled,
};
use crate::helpers::TheInGameUI;
use crate::map_util::get_map_cache_manager;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::ini_game_data::get_global_data;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::preferences::{LadderPref, LadderPreferences, QuickmatchPreferences};
use game_engine::common::random_value::get_game_client_random_value;
use game_engine::common::rts::player_template::get_player_template_store;
use game_network::gamespy::config::GameSpyConfig;
use game_network::gamespy::ladder_defs::{get_ladder_list, init_ladder_list, LadderInfo};
use game_network::gamespy::peer_defs::{default_gamespy_colors, get_gamespy_info, GameSpyColor};
use game_network::gamespy::peer_thread::{
    get_peer_message_queue, PeerRequest, PeerRequestType, PeerResponseType, QMStatus,
};
use game_network::gamespy::persistent_storage_thread::{get_ps_message_queue, PSResponseType};
use game_network::rank_point_value::calculate_rank;
use game_network::{FirewallBehaviorType, SlotState, MAX_SLOTS, PLAYERTEMPLATE_RANDOM};

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;

const MAX_DISCONNECTS: [i32; 5] = [0, 5, 10, 25, 50];

#[derive(Default)]
struct WolQuickMatchState {
    parent_id: i32,
    button_back_id: i32,
    button_start_id: i32,
    button_stop_id: i32,
    button_widen_id: i32,
    button_buddies_id: i32,
    listbox_quick_match_id: i32,
    listbox_map_select_id: i32,
    button_select_all_maps_id: i32,
    button_select_no_maps_id: i32,
    text_entry_wait_time_id: i32,
    combo_box_num_players_id: i32,
    combo_box_max_ping_id: i32,
    combo_box_ladder_id: i32,
    combo_box_max_disconnects_id: i32,
    static_text_num_players_id: i32,
    combo_box_side_id: i32,
    combo_box_color_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_back: Option<Rc<RefCell<GameWindow>>>,
    button_start: Option<Rc<RefCell<GameWindow>>>,
    button_stop: Option<Rc<RefCell<GameWindow>>>,
    button_widen: Option<Rc<RefCell<GameWindow>>>,
    quickmatch_text_window: Option<Rc<RefCell<GameWindow>>>,
    listbox_map_select: Option<Rc<RefCell<GameWindow>>>,
    text_entry_wait_time: Option<Rc<RefCell<GameWindow>>>,
    combo_box_num_players: Option<Rc<RefCell<GameWindow>>>,
    combo_box_max_ping: Option<Rc<RefCell<GameWindow>>>,
    combo_box_ladder: Option<Rc<RefCell<GameWindow>>>,
    combo_box_disabled_ladder: Option<Rc<RefCell<GameWindow>>>,
    combo_box_max_disconnects: Option<Rc<RefCell<GameWindow>>>,
    static_text_num_players: Option<Rc<RefCell<GameWindow>>>,
    combo_box_side: Option<Rc<RefCell<GameWindow>>>,
    combo_box_color: Option<Rc<RefCell<GameWindow>>>,
    is_shutting_down: bool,
    button_pushed: bool,
    raise_message_boxes: bool,
    is_in_init: bool,
    is_populating_ladder_box: bool,
    max_ping_entries: i32,
    max_points: i32,
    min_points: i32,
    selected_image_name: String,
    unselected_image_name: String,
    next_screen: Option<String>,
}

static WOL_QUICKMATCH_STATE: OnceLock<Mutex<WolQuickMatchState>> = OnceLock::new();

fn quickmatch_state() -> &'static Mutex<WolQuickMatchState> {
    WOL_QUICKMATCH_STATE.get_or_init(|| Mutex::new(WolQuickMatchState::default()))
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn listbox_mut(window: &Option<Rc<RefCell<GameWindow>>>) -> Option<std::cell::RefMut<'_, ListBox>> {
    let window = window.as_ref()?;
    let mut guard = window.borrow_mut();
    guard.list_box_mut()
}

fn combo_box_mut(
    window: &Option<Rc<RefCell<GameWindow>>>,
) -> Option<std::cell::RefMut<'_, ComboBox>> {
    let window = window.as_ref()?;
    let mut guard = window.borrow_mut();
    guard.combo_box_mut()
}

fn combo_box_selected_data(window: &Option<Rc<RefCell<GameWindow>>>) -> Option<i32> {
    let window = window.as_ref()?;
    let guard = window.borrow();
    match guard.widget() {
        Some(crate::gui::WindowWidget::ComboBox(combo)) => combo.selected_item_data(),
        _ => None,
    }
}

fn combo_box_selected_index(window: &Option<Rc<RefCell<GameWindow>>>) -> Option<usize> {
    let window = window.as_ref()?;
    let guard = window.borrow();
    match guard.widget() {
        Some(crate::gui::WindowWidget::ComboBox(combo)) => combo.selected_index(),
        _ => None,
    }
}

fn set_combo_box_selected(window: &Option<Rc<RefCell<GameWindow>>>, index: usize) {
    if let Some(window) = window.as_ref() {
        window.borrow_mut().set_combo_box_selected(index, false);
    }
}

fn set_window_enabled(window: &Option<Rc<RefCell<GameWindow>>>, enabled: bool) {
    if let Some(window) = window.as_ref() {
        let _ = window.borrow_mut().enable(enabled);
    }
}

fn set_window_hidden(window: &Option<Rc<RefCell<GameWindow>>>, hidden: bool) {
    if let Some(window) = window.as_ref() {
        let _ = window.borrow_mut().hide(hidden);
    }
}

fn is_info_shown(state: &WolQuickMatchState) -> bool {
    let parent_stats_id = name_to_id("WOLQuickMatchMenu.wnd:ParentStats");
    let Some(parent) = state.parent.as_ref() else {
        return false;
    };
    if let Some(win) = parent.borrow().find_child_by_id(parent_stats_id as u32) {
        return !win.borrow().is_hidden();
    }
    false
}

fn hide_info_gadgets(state: &WolQuickMatchState, hide: bool) {
    let parent_stats_id = name_to_id("WOLQuickMatchMenu.wnd:ParentStats");
    if let Some(parent) = state.parent.as_ref() {
        if let Some(win) = parent.borrow().find_child_by_id(parent_stats_id as u32) {
            let _ = win.borrow_mut().hide(hide);
        }
    }
}

fn hide_options_gadgets(state: &WolQuickMatchState, hide: bool) {
    let parent_options_id = name_to_id("WOLQuickMatchMenu.wnd:ParentOptions");
    if let Some(parent) = state.parent.as_ref() {
        if let Some(win) = parent.borrow().find_child_by_id(parent_options_id as u32) {
            let _ = win.borrow_mut().hide(hide);
        }
    }

    set_window_hidden(&state.combo_box_side, hide);
    set_window_hidden(&state.combo_box_color, hide);
    set_window_hidden(&state.combo_box_num_players, hide);
    set_window_hidden(&state.combo_box_ladder, hide);
    set_window_hidden(&state.combo_box_disabled_ladder, hide);
    set_window_hidden(&state.combo_box_max_ping, hide);
    set_window_hidden(&state.combo_box_max_disconnects, hide);
}

fn enable_options_gadgets(state: &WolQuickMatchState, enable: bool) {
    let parent_options_id = name_to_id("WOLQuickMatchMenu.wnd:ParentOptions");
    if let Some(parent) = state.parent.as_ref() {
        if let Some(win) = parent.borrow().find_child_by_id(parent_options_id as u32) {
            let _ = win.borrow_mut().enable(enable);
        }
    }

    let ladder = get_selected_ladder_info(state);
    set_window_enabled(
        &state.combo_box_side,
        enable && ladder.map(|lad| !lad.random_factions).unwrap_or(true),
    );
    set_window_enabled(&state.combo_box_color, enable);
    set_window_enabled(&state.combo_box_num_players, enable);
    set_window_enabled(&state.combo_box_ladder, enable);
    set_window_enabled(&state.combo_box_disabled_ladder, false);
    set_window_enabled(&state.combo_box_max_ping, enable);
    set_window_enabled(&state.combo_box_max_disconnects, enable);
}

fn get_selected_ladder_info(state: &WolQuickMatchState) -> Option<LadderInfo> {
    let ladder_id = combo_box_selected_data(&state.combo_box_ladder)?;
    if ladder_id <= 0 {
        return None;
    }
    let list = get_ladder_list()?;
    let list = list.read().ok()?;
    list.find_ladder_by_index(ladder_id).cloned()
}

fn update_start_button(state: &WolQuickMatchState) {
    let Some(button) = state.button_start.as_ref() else {
        return;
    };
    if let Some(ladder_id) = combo_box_selected_data(&state.combo_box_ladder) {
        if ladder_id > 0 {
            let _ = button.borrow_mut().enable(true);
            return;
        }
    }

    let mut has_selected_map = false;
    if let Some(listbox_window) = state.listbox_map_select.as_ref() {
        let guard = listbox_window.borrow();
        if let Some(crate::gui::WindowWidget::ListBox(listbox)) = guard.widget() {
            for item in listbox.items() {
                if let Some(ListBoxItemData::Integer(val)) = item.data.as_ref() {
                    if *val != 0 {
                        has_selected_map = true;
                        break;
                    }
                }
            }
        }
    }
    let _ = button.borrow_mut().enable(has_selected_map);
}

fn populate_qm_color_combo_box(state: &WolQuickMatchState, pref: &QuickmatchPreferences) {
    let Some(mut combo) = combo_box_mut(&state.combo_box_color) else {
        return;
    };
    combo.clear();
    combo.add_item(ComboBoxItem::new(0, GameText::fetch("GUI:???")).with_data(-1));

    with_multiplayer_settings(|settings| {
        for (index, def) in settings.color_definitions.iter().enumerate() {
            let name_key = def.get_tooltip_name().as_str();
            let label = if name_key.is_empty() {
                def.name.as_str().to_string()
            } else {
                GameText::fetch(name_key)
            };
            combo.add_item(ComboBoxItem::new(index as u32 + 1, label).with_data(index as i32));
        }
    });

    let selected = pref.get_color().max(0) as usize;
    set_combo_box_selected(&state.combo_box_color, selected);
}

fn populate_qm_side_combo_box(
    state: &WolQuickMatchState,
    fav_side: i32,
    ladder: Option<&LadderInfo>,
) {
    let Some(mut combo) = combo_box_mut(&state.combo_box_side) else {
        return;
    };
    combo.clear();
    combo.add_item(
        ComboBoxItem::new(0, GameText::fetch("GUI:Random")).with_data(PLAYERTEMPLATE_RANDOM),
    );

    let mut seen = HashSet::new();
    let mut entry_to_select = 0usize;
    let mut current_id = 1u32;

    let store = get_player_template_store();
    let count = store.get_player_template_count();
    for idx in 0..count {
        let Some(template) = store.get_nth_player_template(idx) else {
            continue;
        };
        if template.starting_building.is_empty() {
            continue;
        }
        let side_key = format!("SIDE:{}", template.side);
        if seen.contains(&side_key) {
            continue;
        }

        if let Some(ladder) = ladder {
            if !ladder
                .valid_factions
                .iter()
                .any(|f| f.as_str() == template.side)
            {
                continue;
            }
        }

        if let Some(generals) = get_challenge_generals() {
            if let Some(general) = generals.general_by_template_name(&template.name) {
                if !general.is_starting_enabled() {
                    continue;
                }
            }
        }

        seen.insert(side_key.clone());
        combo.add_item(
            ComboBoxItem::new(current_id, GameText::fetch(&side_key)).with_data(idx as i32),
        );
        let added_index = combo.items().len().saturating_sub(1);
        if idx as i32 == fav_side {
            entry_to_select = added_index;
        }
        current_id += 1;
    }

    set_combo_box_selected(&state.combo_box_side, entry_to_select);
    let disable = ladder.map(|lad| lad.random_factions).unwrap_or(false);
    set_window_enabled(&state.combo_box_side, !disable);
}

fn is_valid_qm_ladder(info: &LadderInfo) -> bool {
    if info.index <= 0 || !info.valid_qm {
        return false;
    }
    let wins = {
        let Some(queue) = get_ps_message_queue() else {
            return true;
        };
        let queue = queue.lock().ok();
        let Some(queue) = queue else {
            return true;
        };
        let stats = queue.find_player_stats_by_id(
            get_gamespy_info()
                .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
                .unwrap_or(0),
        );
        stats.wins.values().map(|v| *v as i32).sum::<i32>()
    };

    if info.max_wins != 0 && info.max_wins < wins {
        return false;
    }
    if info.min_wins != 0 && info.min_wins > wins {
        return false;
    }

    true
}

fn populate_qm_ladder_combo_box(state: &mut WolQuickMatchState) {
    let Some(mut combo) = combo_box_mut(&state.combo_box_ladder) else {
        return;
    };
    state.is_populating_ladder_box = true;
    combo.clear();
    combo.add_item(ComboBoxItem::new(0, GameText::fetch("GUI:NoLadder")).with_data(0));

    let mut used = HashSet::new();
    let mut selected_pos = 0usize;

    let pref = QuickmatchPreferences::new();
    let last_addr = pref.get_last_ladder_addr();
    let last_port = pref.get_last_ladder_port();

    if let Some(list) = get_ladder_list() {
        if let Ok(list) = list.read() {
            let last_addr_ascii = AsciiString::from(last_addr.as_str());
            let mut ladder_selected = false;
            if let Some(info) = list.find_ladder(&last_addr_ascii, last_port) {
                if is_valid_qm_ladder(info) {
                    used.insert(info.index);
                    combo.add_item(ComboBoxItem::new(1, info.name.clone()).with_data(info.index));
                    selected_pos = 1;
                    if let Some(mut num_players) = combo_box_mut(&state.combo_box_num_players) {
                        let _ =
                            num_players.select_index((info.players_per_team - 1).max(0) as usize);
                    }
                    set_window_enabled(&state.combo_box_num_players, false);
                    ladder_selected = true;
                }
            }
            if !ladder_selected {
                set_window_enabled(&state.combo_box_num_players, true);
            }

            let profile_id = get_gamespy_info()
                .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
                .unwrap_or(0);
            let mut ladder_prefs = LadderPreferences::new();
            let _ = ladder_prefs.load_profile(profile_id);
            for pref in ladder_prefs.get_recent_ladders().values() {
                if pref.address == last_addr && pref.port == last_port {
                    continue;
                }
                let addr_ascii = AsciiString::from(pref.address.as_str());
                if let Some(info) = list.find_ladder(&addr_ascii, pref.port) {
                    if is_valid_qm_ladder(info) && !used.contains(&info.index) {
                        used.insert(info.index);
                        combo.add_item(
                            ComboBoxItem::new(0, info.name.clone()).with_data(info.index),
                        );
                    }
                }
            }

            for info in list.get_special_ladders() {
                if is_valid_qm_ladder(info) && !used.contains(&info.index) {
                    used.insert(info.index);
                    combo.add_item(ComboBoxItem::new(0, info.name.clone()).with_data(info.index));
                }
            }
            for info in list.get_standard_ladders() {
                if is_valid_qm_ladder(info) && !used.contains(&info.index) {
                    used.insert(info.index);
                    combo.add_item(ComboBoxItem::new(0, info.name.clone()).with_data(info.index));
                }
            }
        }
    }

    combo.add_item(ComboBoxItem::new(0, GameText::fetch("GUI:ChooseLadder")).with_data(-1));
    set_combo_box_selected(&state.combo_box_ladder, selected_pos);
    state.is_populating_ladder_box = false;

    let ladder_info = get_selected_ladder_info(state);
    populate_qm_side_combo_box(state, pref.get_side(), ladder_info.as_ref());
}

fn add_map_row(
    listbox: &mut ListBox,
    display_name: &str,
    map_name: &str,
    selected: bool,
    selected_image: &str,
    unselected_image: &str,
) {
    let row = listbox.add_item(display_name);
    let color = default_gamespy_colors()[if selected {
        GameSpyColor::MapSelected
    } else {
        GameSpyColor::MapUnselected
    } as usize];
    let _ = listbox.set_item_color(row, color);
    let _ = listbox.set_item_data(
        row,
        Some(ListBoxItemData::Integer(if selected { 1 } else { 0 })),
    );
    let _ = listbox.set_item_column_data(row, 1, ListBoxItemData::Text(map_name.to_string()));

    let mut width = 10i32;
    let mut height = 10i32;
    if let Some(collection) = get_mapped_image_collection().try_read() {
        let image_name = if selected {
            selected_image
        } else {
            unselected_image
        };
        if let Some(image) = collection.find_image_by_name(image_name) {
            width = image.get_image_width();
            height = image.get_image_height();
        }
    }

    let _ = listbox.set_item_column_data(
        row,
        0,
        ListBoxItemData::Image {
            name: if selected {
                selected_image.to_string()
            } else {
                unselected_image.to_string()
            },
            width: width.max(1) as u32,
            height: height.max(1) as u32,
            text: None,
        },
    );
}

fn populate_quickmatch_map_select_listbox(
    state: &WolQuickMatchState,
    pref: &QuickmatchPreferences,
) {
    let Some(mut listbox) = listbox_mut(&state.listbox_map_select) else {
        return;
    };
    listbox.clear();

    let ladder_id = combo_box_selected_data(&state.combo_box_ladder).unwrap_or(0);
    let ladder = if ladder_id > 0 {
        get_ladder_list()
            .and_then(|list| list.read().ok())
            .and_then(|list| list.find_ladder_by_index(ladder_id).cloned())
    } else {
        None
    };

    let (num_players, maps): (i32, Vec<String>) = if let Some(ladder) = ladder.as_ref() {
        (
            ladder.players_per_team * 2,
            ladder
                .valid_maps
                .iter()
                .map(|m| m.as_str().to_string())
                .collect(),
        )
    } else {
        let selected = combo_box_selected_index(&state.combo_box_num_players).unwrap_or(0) as i32;
        let num_players = (selected + 1) * 2;
        let config = GameSpyConfig::new_sync();
        (num_players, config.get_qm_maps().iter().cloned().collect())
    };

    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap();
    for map in maps {
        if let Some(md) = cache_guard.find_map(&map) {
            if md.num_players < num_players {
                continue;
            }
            let display = md.display_name.to_string();
            let mut is_selected = pref.is_map_selected(&map);
            if ladder.as_ref().map(|lad| lad.random_maps).unwrap_or(false) {
                is_selected = true;
            }
            add_map_row(
                &mut listbox,
                &display,
                &md.file_name,
                is_selected,
                &state.selected_image_name,
                &state.unselected_image_name,
            );
        }
    }
}

fn save_quickmatch_options(state: &WolQuickMatchState) {
    if state.is_in_init {
        return;
    }

    let mut pref = QuickmatchPreferences::new();
    let ladder_id = combo_box_selected_data(&state.combo_box_ladder).unwrap_or(0);
    let ladder = if ladder_id > 0 {
        get_ladder_list()
            .and_then(|list| list.read().ok())
            .and_then(|list| list.find_ladder_by_index(ladder_id).cloned())
    } else {
        None
    };

    if let Some(ladder) = ladder.as_ref() {
        pref.set_last_ladder(ladder.address.as_str(), ladder.port);
    } else {
        pref.set_last_ladder("", 0);
    }

    if ladder.as_ref().map(|lad| !lad.random_maps).unwrap_or(true) {
        if let Some(listbox_window) = state.listbox_map_select.as_ref() {
            let guard = listbox_window.borrow();
            if let Some(crate::gui::WindowWidget::ListBox(listbox)) = guard.widget() {
                for item in listbox.items().iter() {
                    let selected =
                        matches!(item.data, Some(ListBoxItemData::Integer(val)) if val != 0);
                    let map_name = item
                        .column_data
                        .get(1)
                        .and_then(|data| match data {
                            ListBoxItemData::Text(name) => Some(name.clone()),
                            _ => None,
                        })
                        .unwrap_or_else(|| item.text.clone());
                    pref.set_map_selected(&map_name, selected);
                }
            }
        }
    }

    let num_players_index =
        combo_box_selected_index(&state.combo_box_num_players).unwrap_or(0) as i32;
    pref.set_num_players(num_players_index);
    let max_ping = combo_box_selected_index(&state.combo_box_max_ping).unwrap_or(0) as i32;
    pref.set_max_ping(max_ping);

    let side = combo_box_selected_data(&state.combo_box_side)
        .unwrap_or(0)
        .max(0);
    pref.set_side(side);

    let color = combo_box_selected_index(&state.combo_box_color).unwrap_or(0) as i32;
    pref.set_color(color.max(0));

    let max_discons =
        combo_box_selected_index(&state.combo_box_max_disconnects).unwrap_or(0) as i32;
    pref.set_max_disconnects(max_discons);

    pref.write();
}

fn handle_persistent_storage_responses() {
    let Some(queue) = get_ps_message_queue() else {
        return;
    };
    let resp = {
        let mut queue = queue.lock().ok()?;
        queue.get_response()
    };
    let Some(resp) = resp else {
        return;
    };

    match resp.response_type {
        PSResponseType::CouldNotConnect => {
            gs_message_box_ok(
                &GameText::fetch("GUI:Error"),
                &GameText::fetch("GUI:PSCannotConnect"),
                None,
            );
            close_overlay(GameSpyOverlayType::PlayerInfo);
        }
        PSResponseType::Preorder => {
            if resp.preorder {
                if let Some(info) = get_gamespy_info() {
                    if let Ok(mut info) = info.lock() {
                        info.mark_player_as_preorder(info.get_local_profile_id());
                    }
                }
                if let Some(queue) = get_ps_message_queue() {
                    if let Ok(mut queue) = queue.lock() {
                        let stats = queue.find_player_stats_by_id(
                            get_gamespy_info()
                                .and_then(|info| {
                                    info.lock().ok().map(|guard| guard.get_local_profile_id())
                                })
                                .unwrap_or(0),
                        );
                        let mut new_resp = resp.clone();
                        new_resp.response_type = PSResponseType::PlayerStats;
                        new_resp.player = stats;
                        queue.add_response(new_resp);
                    }
                }
            }
        }
        PSResponseType::PlayerStats => {
            if let Some(info) = get_gamespy_info() {
                if let Ok(mut info) = info.lock() {
                    if resp.player.id == info.get_local_profile_id() {
                        let mut req = PeerRequest::default();
                        req.request_type = PeerRequestType::PushStats;
                        let wins: i32 = resp.player.wins.values().map(|v| *v as i32).sum();
                        let losses: i32 = resp.player.losses.values().map(|v| *v as i32).sum();
                        req.stats_wins = wins;
                        req.stats_losses = losses;
                        req.stats_rank_points = calculate_rank(&resp.player);
                        let favorite = game_network::get_favorite_side(&resp.player);
                        req.stats_side = if favorite < 0 { 0 } else { favorite };
                        req.stats_preorder = info.did_player_preorder(resp.player.id);
                        if let Some(peer_queue) = get_peer_message_queue() {
                            if let Ok(mut peer_queue) = peer_queue.lock() {
                                peer_queue.add_request(req);
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn update_local_player_stats() {
    let lookup_id = get_gamespy_info()
        .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
        .unwrap_or(0);
    let name = get_gamespy_info()
        .and_then(|info| {
            info.lock()
                .ok()
                .map(|guard| guard.get_local_name().as_str().to_string())
        })
        .unwrap_or_default();
    populate_player_info_windows("WOLQuickMatchMenu.wnd", lookup_id, &name);
}

fn ladder_list_is_empty() -> bool {
    let config = GameSpyConfig::new_sync();
    let list = init_ladder_list(&config);
    let Ok(list) = list.read() else {
        return true;
    };
    list.get_standard_ladders().is_empty()
        && list.get_special_ladders().is_empty()
        && list.get_local_ladders().is_empty()
}

fn ladder_choice_selected(state: &mut WolQuickMatchState, selected: i32) {
    if selected == 0 {
        let pref = QuickmatchPreferences::new();
        set_combo_box_selected(
            &state.combo_box_num_players,
            pref.get_num_players().max(0) as usize,
        );
        set_window_enabled(&state.combo_box_num_players, true);
        populate_qm_side_combo_box(state, pref.get_side(), None);
    } else if selected > 0 {
        if let Some(list) = get_ladder_list().and_then(|list| list.read().ok()) {
            if let Some(info) = list.find_ladder_by_index(selected) {
                let index = (info.players_per_team - 1).max(0) as usize;
                set_combo_box_selected(&state.combo_box_num_players, index);
            } else {
                set_combo_box_selected(&state.combo_box_num_players, 0);
            }
            set_window_enabled(&state.combo_box_num_players, false);
            let pref = QuickmatchPreferences::new();
            populate_qm_side_combo_box(state, pref.get_side(), list.find_ladder_by_index(selected));
        }
    } else {
        populate_qm_ladder_combo_box(state);
        open_overlay(GameSpyOverlayType::LadderSelect);
    }
}

fn set_listbox_selection(
    listbox: &mut ListBox,
    row: usize,
    selected: bool,
    selected_image: &str,
    unselected_image: &str,
) {
    let _ = listbox.set_item_data(
        row,
        Some(ListBoxItemData::Integer(if selected { 1 } else { 0 })),
    );
    let color = default_gamespy_colors()[if selected {
        GameSpyColor::MapSelected
    } else {
        GameSpyColor::MapUnselected
    } as usize];
    let _ = listbox.set_item_color(row, color);
    let mut width = 10i32;
    let mut height = 10i32;
    if let Some(collection) = get_mapped_image_collection().try_read() {
        let image_name = if selected {
            selected_image
        } else {
            unselected_image
        };
        if let Some(image) = collection.find_image_by_name(image_name) {
            width = image.get_image_width();
            height = image.get_image_height();
        }
    }
    let _ = listbox.set_item_column_data(
        row,
        0,
        ListBoxItemData::Image {
            name: if selected {
                selected_image.to_string()
            } else {
                unselected_image.to_string()
            },
            width: width.max(1) as u32,
            height: height.max(1) as u32,
            text: None,
        },
    );
}

pub fn wol_quick_match_menu_init(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let mut state = quickmatch_state()
        .lock()
        .expect("WOLQuickMatchMenu state lock poisoned");
    state.is_in_init = true;

    if with_gamespy_game_info(|info| info.is_game_in_progress()) {
        with_gamespy_game_info_mut(|info| info.set_game_in_progress(false));
        if let Some(reason) = get_gamespy_info().and_then(|info| {
            info.lock()
                .ok()
                .and_then(|guard| guard.is_disconnected_after_game_start())
        }) {
            let title = GameText::fetch("GUI:GSErrorTitle");
            let body = GameText::fetch(&format!("GUI:GSDisconReason{}", reason));
            close_all_overlays();
            gs_message_box_ok(&title, &body, None);
            if let Some(info) = get_gamespy_info() {
                if let Ok(mut info) = info.lock() {
                    info.reset();
                }
            }
            let _ = get_shell().pop_immediate();
            state.is_in_init = false;
            return;
        }
    }

    state.next_screen = None;
    state.button_pushed = false;
    state.is_shutting_down = false;
    state.raise_message_boxes = true;

    state.parent_id = name_to_id("WOLQuickMatchMenu.wnd:WOLQuickMatchMenuParent");
    state.button_back_id = name_to_id("WOLQuickMatchMenu.wnd:ButtonBack");
    state.button_buddies_id = name_to_id("WOLQuickMatchMenu.wnd:ButtonBuddies");
    state.button_start_id = name_to_id("WOLQuickMatchMenu.wnd:ButtonStart");
    state.button_stop_id = name_to_id("WOLQuickMatchMenu.wnd:ButtonStop");
    state.button_widen_id = name_to_id("WOLQuickMatchMenu.wnd:ButtonWiden");
    state.listbox_quick_match_id = name_to_id("WOLQuickMatchMenu.wnd:ListboxQuickMatch");
    state.listbox_map_select_id = name_to_id("WOLQuickMatchMenu.wnd:ListBoxMapSelect");
    state.button_select_all_maps_id = name_to_id("WOLQuickMatchMenu.wnd:ButtonSelectAllMaps");
    state.button_select_no_maps_id = name_to_id("WOLQuickMatchMenu.wnd:ButtonSelectNoMaps");
    state.text_entry_wait_time_id = name_to_id("WOLQuickMatchMenu.wnd:TextEntryWaitTime");
    state.combo_box_max_ping_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxMaxPing");
    state.combo_box_num_players_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxNumPlayers");
    state.combo_box_ladder_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxLadder");
    state.combo_box_max_disconnects_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxMaxDisconnects");
    state.static_text_num_players_id = name_to_id("WOLQuickMatchMenu.wnd:StaticTextNumPlayers");
    state.combo_box_side_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxSide");
    state.combo_box_color_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxColor");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
    });

    if let Some(parent) = state.parent.as_ref() {
        state.button_back = parent
            .borrow()
            .find_child_by_id(state.button_back_id as u32);
        state.button_start = parent
            .borrow()
            .find_child_by_id(state.button_start_id as u32);
        state.button_stop = parent
            .borrow()
            .find_child_by_id(state.button_stop_id as u32);
        state.button_widen = parent
            .borrow()
            .find_child_by_id(state.button_widen_id as u32);
        state.quickmatch_text_window = parent
            .borrow()
            .find_child_by_id(state.listbox_quick_match_id as u32);
        state.listbox_map_select = parent
            .borrow()
            .find_child_by_id(state.listbox_map_select_id as u32);
        state.text_entry_wait_time = parent
            .borrow()
            .find_child_by_id(state.text_entry_wait_time_id as u32);
        state.combo_box_max_ping = parent
            .borrow()
            .find_child_by_id(state.combo_box_max_ping_id as u32);
        state.combo_box_num_players = parent
            .borrow()
            .find_child_by_id(state.combo_box_num_players_id as u32);
        state.combo_box_ladder = parent
            .borrow()
            .find_child_by_id(state.combo_box_ladder_id as u32);
        state.combo_box_max_disconnects = parent
            .borrow()
            .find_child_by_id(state.combo_box_max_disconnects_id as u32);
        state.static_text_num_players = parent
            .borrow()
            .find_child_by_id(state.static_text_num_players_id as u32);
        state.combo_box_side = parent
            .borrow()
            .find_child_by_id(state.combo_box_side_id as u32);
        state.combo_box_color = parent
            .borrow()
            .find_child_by_id(state.combo_box_color_id as u32);
    }

    if let Some(info) = get_gamespy_info() {
        if let Ok(mut info) = info.lock() {
            info.register_text_window(state.listbox_quick_match_id as u32);
        }
    }

    if ladder_list_is_empty() {
        state.combo_box_disabled_ladder = state.combo_box_ladder.take();
        state.is_populating_ladder_box = true;
        if let Some(mut combo) = combo_box_mut(&state.combo_box_disabled_ladder) {
            combo.clear();
            combo.add_item(ComboBoxItem::new(0, GameText::fetch("GUI:NoLadder")).with_data(0));
            let _ = combo.select_index(0);
        }
        state.is_populating_ladder_box = false;
    }

    if let Some(parent) = state.parent.as_ref() {
        if let Some(static_text) = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLQuickMatchMenu.wnd:StaticTextTitle") as u32)
        {
            let name = get_gamespy_info()
                .and_then(|info| {
                    info.lock()
                        .ok()
                        .map(|guard| guard.get_local_name().as_str().to_string())
                })
                .unwrap_or_default();
            let title = GameText::fetch("GUI:QuickMatchTitle").replace("%s", &name);
            let _ = static_text.borrow_mut().set_text(&title);
        }
    }

    set_window_enabled(&state.button_widen, false);
    set_window_hidden(&state.button_stop, true);
    set_window_hidden(&state.button_start, false);

    if let Some(mut listbox) = listbox_mut(&state.quickmatch_text_window) {
        listbox.clear();
    }

    enable_options_gadgets(&state, true);
    layout.hide(false);
    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(parent.clone()));
        });
    }

    state.selected_image_name = "CustomMatch_selected".to_string();
    state.unselected_image_name = "CustomMatch_deselected".to_string();

    let pref = QuickmatchPreferences::new();
    state.max_points = pref.get_max_points();
    state.min_points = pref.get_min_points();

    if let Some(mut combo) = combo_box_mut(&state.combo_box_num_players) {
        combo.clear();
        for i in 1..5 {
            let label = GameText::fetch("GUI:PlayersVersusPlayers")
                .replace("%d", &i.to_string())
                .replace("%d", &i.to_string());
            combo.add_item(ComboBoxItem::new(i as u32, label));
        }
    }
    set_combo_box_selected(
        &state.combo_box_num_players,
        pref.get_num_players().max(0) as usize,
    );

    if let Some(mut combo) = combo_box_mut(&state.combo_box_max_disconnects) {
        combo.clear();
        combo.add_item(ComboBoxItem::new(0, GameText::fetch("GUI:Any")));
        for value in MAX_DISCONNECTS.iter().skip(1) {
            combo.add_item(ComboBoxItem::new(*value as u32, value.to_string()));
        }
    }
    set_combo_box_selected(
        &state.combo_box_max_disconnects,
        pref.get_max_disconnects().max(0) as usize,
    );

    let config = GameSpyConfig::new_sync();
    let (_, ping_timeout_ms, _, _) = config.get_ping_config();
    state.max_ping_entries = (ping_timeout_ms - 1) / 100 + 1;
    if let Some(mut combo) = combo_box_mut(&state.combo_box_max_ping) {
        combo.clear();
        for i in 1..state.max_ping_entries {
            let label =
                GameText::fetch("GUI:TimeInMilliseconds").replace("%d", &(i * 100).to_string());
            combo.add_item(ComboBoxItem::new(i as u32, label));
        }
        combo.add_item(ComboBoxItem::new(
            state.max_ping_entries as u32,
            GameText::fetch("GUI:ANY"),
        ));
    }
    let mut ping_index = pref.get_max_ping();
    if ping_index < 0 {
        ping_index = 0;
    }
    if ping_index >= state.max_ping_entries {
        ping_index = state.max_ping_entries - 1;
    }
    set_combo_box_selected(&state.combo_box_max_ping, ping_index as usize);

    populate_qm_color_combo_box(&state, &pref);
    populate_qm_side_combo_box(
        &state,
        pref.get_side(),
        get_selected_ladder_info(&state).as_ref(),
    );
    if state.combo_box_ladder.is_some() {
        populate_qm_ladder_combo_box(&mut state);
    }

    get_shell().show_shell_map(true);
    with_gamespy_game_info_mut(|info| info.reset());

    populate_quickmatch_map_select_listbox(&state, &pref);
    update_local_player_stats();
    update_start_button(&state);

    with_window_manager(|manager| manager.transition_set_group("WOLQuickMatchMenuFade", false));
    state.is_in_init = false;
}

fn shutdown_complete(layout: &WindowLayout) {
    let mut state = quickmatch_state()
        .lock()
        .expect("WOLQuickMatchMenu state lock poisoned");
    state.is_shutting_down = false;
    layout.hide(true);
    let next = state.next_screen.clone();
    let mut shell = get_shell();
    let _ = shell.shutdown_complete(Some(layout), next.is_some());
    if let Some(screen) = next {
        let _ = shell.push(&screen, false);
    }
    state.next_screen = None;
}

pub fn wol_quick_match_menu_shutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    if let Some(info) = get_gamespy_info() {
        if let Ok(mut info) = info.lock() {
            info.unregister_text_window(
                name_to_id("WOLQuickMatchMenu.wnd:ListboxQuickMatch") as u32
            );
        }
    }

    let mut state = quickmatch_state()
        .lock()
        .expect("WOLQuickMatchMenu state lock poisoned");

    let quitting = get_game_engine()
        .and_then(|engine| engine.lock().ok().map(|guard| guard.get_quitting()))
        .unwrap_or(false);
    if !quitting {
        save_quickmatch_options(&state);
    }

    state.parent = None;
    state.button_back = None;
    state.quickmatch_text_window = None;
    state.selected_image_name.clear();
    state.unselected_image_name.clear();
    state.is_shutting_down = true;

    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);
    if pop_immediate {
        shutdown_complete(layout);
        return;
    }

    get_shell().reverse_animate_window();
    with_window_manager(|manager| manager.transition_reverse("WOLQuickMatchMenuFade"));
    raise_gs_message_box();
}

pub fn wol_quick_match_menu_update(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let mut state = quickmatch_state()
        .lock()
        .expect("WOLQuickMatchMenu state lock poisoned");
    let shell_finished = get_shell().is_anim_finished();
    let transitions_finished = with_window_manager(|manager| manager.transitions_finished());
    if state.is_shutting_down && shell_finished && transitions_finished {
        shutdown_complete(layout);
        return;
    }

    if state.raise_message_boxes {
        raise_gs_message_box();
        state.raise_message_boxes = false;
    }

    if shell_finished && !state.button_pushed {
        handle_buddy_responses();
        handle_persistent_storage_responses();

        if with_gamespy_game_info(|info| info.is_game_in_progress()) {
            if get_gamespy_info()
                .and_then(|info| {
                    info.lock()
                        .ok()
                        .and_then(|guard| guard.is_disconnected_after_game_start())
                })
                .is_some()
            {
                return;
            }

            let allowed = game_engine::common::preferences::GameSpyMiscPreferences::new()
                .get_max_messages_per_update();
            if let Some(peer_queue) = get_peer_message_queue() {
                if let Ok(mut peer_queue) = peer_queue.lock() {
                    let mut allowed = allowed;
                    let mut saw_important = false;
                    while allowed > 0 && !saw_important {
                        allowed -= 1;
                        let Some(resp) = peer_queue.get_response() else {
                            break;
                        };
                        if resp.response_type == PeerResponseType::Disconnect {
                            saw_important = true;
                            let reason_key =
                                format!("GUI:GSDisconReason{}", resp.discon_reason as i32);
                            if let Some(listbox_window) = with_window_manager(|manager| {
                                manager.get_window_by_id(name_to_id(
                                    "ScoreScreen.wnd:ListboxChatWindowScoreScreen",
                                ))
                            }) {
                                if let Some(mut listbox) =
                                    listbox_window.borrow_mut().list_box_mut()
                                {
                                    listbox.add_item_with_color(
                                        &GameText::fetch(&reason_key),
                                        default_gamespy_colors()[GameSpyColor::Default as usize],
                                    );
                                }
                            } else {
                                TheInGameUI::message(&reason_key);
                            }
                            if let Some(info) = get_gamespy_info() {
                                if let Ok(mut info) = info.lock() {
                                    info.mark_as_disconnected_after_game_start(
                                        resp.discon_reason as i32,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            return;
        }

        let allowed = game_engine::common::preferences::GameSpyMiscPreferences::new()
            .get_max_messages_per_update();
        if let Some(peer_queue) = get_peer_message_queue() {
            if let Ok(mut peer_queue) = peer_queue.lock() {
                let mut allowed = allowed;
                let mut saw_important = false;
                while allowed > 0 && !saw_important {
                    allowed -= 1;
                    let Some(resp) = peer_queue.get_response() else {
                        break;
                    };
                    match resp.response_type {
                        PeerResponseType::PlayerUtm => {
                            if resp.command.eq_ignore_ascii_case("STATS") {
                                let mut parts = resp.command_options.splitn(2, ' ');
                                let id_str = parts.next().unwrap_or("0");
                                let id = id_str.parse::<i32>().unwrap_or(0);
                                let kv = parts.next().unwrap_or("");
                                if let Some(queue) = get_ps_message_queue() {
                                    if let Ok(mut queue) = queue.lock() {
                                        let mut stats = queue.parse_player_kv_pairs(kv);
                                        let old = queue.find_player_stats_by_id(id);
                                        stats.id = id;
                                        if stats.id != 0 && old.id == 0 {
                                            queue.track_player_stats(stats);
                                        }
                                    }
                                }
                            }
                        }
                        PeerResponseType::Disconnect => {
                            saw_important = true;
                            let reason_key =
                                format!("GUI:GSDisconReason{}", resp.discon_reason as i32);
                            gs_message_box_ok(
                                &GameText::fetch("GUI:GSErrorTitle"),
                                &GameText::fetch(&reason_key),
                                None,
                            );
                            close_all_overlays();
                            if let Some(info) = get_gamespy_info() {
                                if let Ok(mut info) = info.lock() {
                                    info.reset();
                                }
                            }
                            let _ = get_shell().pop();
                        }
                        PeerResponseType::QuickMatchStatus => {
                            saw_important = true;
                            let window_id = state.listbox_quick_match_id as u32;
                            let mut add_qm_text = |text: String| {
                                if let Some(info) = get_gamespy_info() {
                                    if let Ok(mut info) = info.lock() {
                                        info.add_text(
                                            text,
                                            default_gamespy_colors()
                                                [GameSpyColor::Default as usize],
                                            Some(window_id),
                                        );
                                    }
                                }
                            };
                            match resp.qm_status {
                                QMStatus::Idle => {}
                                QMStatus::JoiningQmChannel => {
                                    add_qm_text(GameText::fetch("QM:JOININGQMCHANNEL"))
                                }
                                QMStatus::LookingForBot => {
                                    add_qm_text(GameText::fetch("QM:LOOKINGFORBOT"))
                                }
                                QMStatus::SentInfo => add_qm_text(GameText::fetch("QM:SENTINFO")),
                                QMStatus::Working => {
                                    add_qm_text(
                                        GameText::fetch("QM:WORKING")
                                            .replace("%d", &resp.qm_pool_size.to_string()),
                                    );
                                    set_window_enabled(&state.button_widen, true);
                                }
                                QMStatus::PoolSize => {
                                    add_qm_text(
                                        GameText::fetch("QM:POOLSIZE")
                                            .replace("%d", &resp.qm_pool_size.to_string()),
                                    );
                                }
                                QMStatus::WideningSearch => {
                                    add_qm_text(GameText::fetch("QM:WIDENINGSEARCH"));
                                    set_window_enabled(&state.button_widen, false);
                                }
                                QMStatus::Matched => {
                                    add_qm_text(GameText::fetch("QM:MATCHED"));
                                    set_window_enabled(&state.button_widen, false);

                                    with_gamespy_game_info_mut(|info| {
                                        info.enter_game();
                                        info.set_seed(resp.qm_seed);
                                    });

                                    let num_players = resp
                                        .staging_room_player_names
                                        .iter()
                                        .filter(|n| !n.is_empty())
                                        .count();
                                    let mut selected_map = None::<String>;
                                    let config = GameSpyConfig::new_sync();
                                    let mut map_idx = resp.qm_map_idx;
                                    for map in config.get_qm_maps() {
                                        let map_lower = map.to_lowercase();
                                        let cache = get_map_cache_manager();
                                        let cache_guard = cache.lock().unwrap();
                                        if let Some(md) = cache_guard.find_map(&map_lower) {
                                            if md.num_players < num_players as i32 {
                                                continue;
                                            }
                                            if map_idx == 0 {
                                                selected_map = Some(map.clone());
                                                break;
                                            }
                                            map_idx -= 1;
                                        }
                                    }

                                    let num_players_per_team = (num_players / 2).max(1) as i32;
                                    with_gamespy_game_info_mut(|info| {
                                        if let Some(map_name) = selected_map.clone() {
                                            info.set_map(map_name);
                                            let cache = get_map_cache_manager();
                                            let cache_guard = cache.lock().unwrap();
                                            if let Some(md) = cache_guard.find_map(info.get_map()) {
                                                info.set_map_crc(md.crc);
                                                info.set_map_size(md.filesize);
                                            }
                                        }

                                        for i in 0..MAX_SLOTS {
                                            let name = &resp.staging_room_player_names[i];
                                            let slot = info.get_slot_mut(i).unwrap();
                                            if name.is_empty() {
                                                slot.set_state(SlotState::Closed, String::new(), 0);
                                            } else {
                                                slot.set_state(
                                                    SlotState::Player,
                                                    name.clone(),
                                                    resp.qm_ip[i],
                                                );
                                                slot.set_color(resp.qm_color[i]);
                                                slot.set_player_template(resp.qm_side[i]);
                                                slot.set_nat_behavior(firewall_behavior_from_int(
                                                    resp.qm_nat[i],
                                                ));
                                                slot.set_team_number(
                                                    (i as i32) / num_players_per_team,
                                                );
                                            }
                                        }
                                    });

                                    with_gamespy_game_info_mut(|info| info.start_game(0));

                                    set_window_enabled(&state.button_buddies, false);
                                    close_overlay(GameSpyOverlayType::Buddy);
                                }
                                QMStatus::InChannel => add_qm_text(GameText::fetch("QM:INCHANNEL")),
                                QMStatus::NegotiatingFirewalls => {
                                    add_qm_text(GameText::fetch("QM:NEGOTIATINGFIREWALLS"))
                                }
                                QMStatus::StartingGame => {
                                    add_qm_text(GameText::fetch("QM:STARTINGGAME"))
                                }
                                QMStatus::CouldNotFindBot => {
                                    add_qm_text(GameText::fetch("QM:COULDNOTFINDBOT"));
                                    set_window_enabled(&state.button_widen, false);
                                    set_window_hidden(&state.button_start, false);
                                    set_window_hidden(&state.button_stop, true);
                                    enable_options_gadgets(&state, true);
                                }
                                QMStatus::CouldNotFindChannel => {
                                    add_qm_text(GameText::fetch("QM:COULDNOTFINDCHANNEL"));
                                    set_window_enabled(&state.button_widen, false);
                                    set_window_hidden(&state.button_start, false);
                                    set_window_hidden(&state.button_stop, true);
                                    enable_options_gadgets(&state, true);
                                }
                                QMStatus::CouldNotNegotiateFirewalls => {
                                    add_qm_text(GameText::fetch("QM:COULDNOTNEGOTIATEFIREWALLS"));
                                    set_window_enabled(&state.button_widen, false);
                                    set_window_hidden(&state.button_start, false);
                                    set_window_hidden(&state.button_stop, true);
                                    enable_options_gadgets(&state, true);
                                }
                                QMStatus::Stopped => {
                                    add_qm_text(GameText::fetch("QM:STOPPED"));
                                    set_window_enabled(&state.button_widen, false);
                                    set_window_hidden(&state.button_start, false);
                                    set_window_hidden(&state.button_stop, true);
                                    enable_options_gadgets(&state, true);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

pub fn wol_quick_match_menu_input(
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

    let state = quickmatch_state()
        .lock()
        .expect("WOLQuickMatchMenu state lock poisoned");
    if state.button_pushed {
        return WindowMsgHandled::Handled;
    }
    if let Some(button) = state.button_back.as_ref() {
        let _ = button.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            state.button_back_id as u32,
            state.button_back_id as u32,
        );
    }

    let _ = window;
    WindowMsgHandled::Handled
}

pub fn wol_quick_match_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            let mut state = quickmatch_state()
                .lock()
                .expect("WOLQuickMatchMenu state lock poisoned");
            let control_id = data1 as i32;
            if state.button_pushed {
                return WindowMsgHandled::Handled;
            }

            if control_id == state.combo_box_ladder_id && !state.is_populating_ladder_box {
                if let Some(selected) = combo_box_selected_data(&state.combo_box_ladder) {
                    save_quickmatch_options(&state);
                    ladder_choice_selected(&mut state, selected);
                    let pref = QuickmatchPreferences::new();
                    populate_quickmatch_map_select_listbox(&state, &pref);
                    update_start_button(&state);
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_stop_id {
                if let Some(queue) = get_peer_message_queue() {
                    if let Ok(mut queue) = queue.lock() {
                        let mut req = PeerRequest::default();
                        req.request_type = PeerRequestType::StopQuickMatch;
                        queue.add_request(req);
                    }
                }
                set_window_enabled(&state.button_widen, false);
                set_window_hidden(&state.button_start, false);
                set_window_hidden(&state.button_stop, true);
                enable_options_gadgets(&state, true);
                if let Some(info) = get_gamespy_info() {
                    if let Ok(mut info) = info.lock() {
                        info.add_text(
                            GameText::fetch("GUI:QMAborted"),
                            default_gamespy_colors()[GameSpyColor::Default as usize],
                            Some(state.listbox_quick_match_id as u32),
                        );
                    }
                }
                return WindowMsgHandled::Handled;
            }

            let button_options_id = name_to_id("WOLQuickMatchMenu.wnd:ButtonOptions");
            if control_id == button_options_id {
                if let Some(win) =
                    with_window_manager(|manager| manager.get_window_by_id(button_options_id))
                {
                    if is_info_shown(&state) {
                        hide_info_gadgets(&state, true);
                        hide_options_gadgets(&state, false);
                        let _ = win
                            .borrow_mut()
                            .set_text(&GameText::fetch("GUI:PlayerInfo"));
                    } else {
                        hide_info_gadgets(&state, false);
                        hide_options_gadgets(&state, true);
                        let _ = win.borrow_mut().set_text(&GameText::fetch("GUI:Setup"));
                    }
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_widen_id {
                if let Some(queue) = get_peer_message_queue() {
                    if let Ok(mut queue) = queue.lock() {
                        let mut req = PeerRequest::default();
                        req.request_type = PeerRequestType::WidenQuickMatchSearch;
                        queue.add_request(req);
                    }
                }
                set_window_enabled(&state.button_widen, false);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_start_id {
                start_quickmatch(&mut state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_buddies_id {
                toggle_overlay(GameSpyOverlayType::Buddy);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_back_id {
                state.button_pushed = true;
                if let Some(info) = get_gamespy_info() {
                    if let Ok(mut info) = info.lock() {
                        info.leave_group_room();
                    }
                }
                state.next_screen = Some("Menus/WOLWelcomeMenu.wnd".to_string());
                let _ = get_shell().pop();
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_select_all_maps_id {
                if let Some(mut listbox) = listbox_mut(&state.listbox_map_select) {
                    let rows = listbox.items().len();
                    for row in 0..rows {
                        set_listbox_selection(
                            &mut listbox,
                            row,
                            true,
                            &state.selected_image_name,
                            &state.unselected_image_name,
                        );
                    }
                }
                update_start_button(&state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_select_no_maps_id {
                if let Some(mut listbox) = listbox_mut(&state.listbox_map_select) {
                    let rows = listbox.items().len();
                    for row in 0..rows {
                        set_listbox_selection(
                            &mut listbox,
                            row,
                            false,
                            &state.selected_image_name,
                            &state.unselected_image_name,
                        );
                    }
                }
                update_start_button(&state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.listbox_map_select_id {
                if let Some(mut listbox) = listbox_mut(&state.listbox_map_select) {
                    if let Some(selected) = listbox.selected_indices().first().copied() {
                        let ladder = get_selected_ladder_info(&state);
                        if ladder.as_ref().map(|lad| !lad.random_maps).unwrap_or(true) {
                            let was_selected = matches!(listbox.get_item_data(selected), Some(ListBoxItemData::Integer(val)) if *val != 0);
                            set_listbox_selection(
                                &mut listbox,
                                selected,
                                !was_selected,
                                &state.selected_image_name,
                                &state.unselected_image_name,
                            );
                        }
                        listbox.set_selected_indices(&[]);
                    }
                }
                update_start_button(&state);
                return WindowMsgHandled::Handled;
            }

            save_quickmatch_options(&state);
            let pref = QuickmatchPreferences::new();
            populate_quickmatch_map_select_listbox(&state, &pref);
            update_start_button(&state);
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

fn firewall_behavior_from_int(value: i32) -> FirewallBehaviorType {
    match value {
        1 => FirewallBehaviorType::Simple,
        2 => FirewallBehaviorType::DumbMangling,
        4 => FirewallBehaviorType::SmartMangling,
        8 => FirewallBehaviorType::NetgearBug,
        16 => FirewallBehaviorType::SimplePortAllocation,
        32 => FirewallBehaviorType::RelativePortAllocation,
        64 => FirewallBehaviorType::DestinationPortDelta,
        _ => FirewallBehaviorType::Unknown,
    }
}

fn start_quickmatch(state: &mut WolQuickMatchState) {
    let mut req = PeerRequest::default();
    req.request_type = PeerRequestType::StartQuickMatch;

    if let Some(listbox_window) = state.listbox_map_select.as_ref() {
        let guard = listbox_window.borrow();
        if let Some(crate::gui::WindowWidget::ListBox(listbox)) = guard.widget() {
            req.qm_maps = listbox
                .items()
                .iter()
                .map(|item| matches!(item.data, Some(ListBoxItemData::Integer(val)) if val != 0))
                .collect();
        }
    }

    req.qm_max_point_percentage = state.max_points.max(100);
    req.qm_min_point_percentage = state.min_points.min(100);
    req.qm_widen_time = 0;

    let max_discons = combo_box_selected_index(&state.combo_box_max_disconnects).unwrap_or(0);
    req.qm_max_discons = MAX_DISCONNECTS.get(max_discons).copied().unwrap_or(0);

    let config = GameSpyConfig::new_sync();
    let (_, ping_timeout_ms, _, _) = config.get_ping_config();
    let mut ping_index = combo_box_selected_index(&state.combo_box_max_ping).unwrap_or(0) as i32;
    if ping_index >= state.max_ping_entries - 1 {
        req.qm_max_ping = ping_timeout_ms;
    } else {
        req.qm_max_ping = (ping_index + 1) * 100;
    }

    let stats = get_ps_message_queue()
        .and_then(|queue| {
            queue.lock().ok().map(|queue| {
                let profile_id = get_gamespy_info()
                    .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
                    .unwrap_or(0);
                queue.find_player_stats_by_id(profile_id)
            })
        })
        .unwrap_or_default();
    req.qm_points = calculate_rank(&stats);

    let ladder_id = combo_box_selected_data(&state.combo_box_ladder)
        .unwrap_or(0)
        .max(0);
    req.qm_ladder_id = ladder_id;
    req.qm_ladder_pass_crc = 0;

    let ladder_info = if ladder_id > 0 {
        get_ladder_list()
            .and_then(|list| list.read().ok())
            .and_then(|list| list.find_ladder_by_index(ladder_id))
            .cloned()
    } else {
        None
    };

    let mut side = combo_box_selected_data(&state.combo_box_side).unwrap_or(-1);
    if let Some(ladder) = ladder_info.as_ref() {
        if ladder.random_factions && !ladder.valid_factions.is_empty() {
            let rand_index =
                get_game_client_random_value(0, ladder.valid_factions.len() as i32 - 1) as usize;
            if let Some(side_name) = ladder.valid_factions.get(rand_index) {
                let store = get_player_template_store();
                for idx in 0..store.get_player_template_count() {
                    if let Some(template) = store.get_nth_player_template(idx) {
                        if template.side == side_name.as_str() {
                            side = idx as i32;
                            break;
                        }
                    }
                }
            }
        }
    } else if side == PLAYERTEMPLATE_RANDOM {
        let mut tries = 0;
        while tries < 10 && side == PLAYERTEMPLATE_RANDOM {
            if let Some(combo) = combo_box_mut(&state.combo_box_side) {
                let count = combo.items().len().max(1);
                let rand_index = get_game_client_random_value(0, count as i32 - 1) as usize;
                if let Some(item) = combo.items().get(rand_index) {
                    if let Some(data) = item.data {
                        side = data;
                    }
                }
            }
            tries += 1;
        }
    }
    req.qm_side = side;

    let color = combo_box_selected_data(&state.combo_box_color).unwrap_or(-1);
    req.qm_color = color;

    let firewall_behavior = OptionPreferences::new().get_firewall_behavior();
    req.qm_nat = firewall_behavior;

    if let Some(ladder) = ladder_info.as_ref() {
        req.qm_num_players = ladder.players_per_team * 2;
    } else {
        let num_players_index =
            combo_box_selected_index(&state.combo_box_num_players).unwrap_or(0) as i32;
        req.qm_num_players = (num_players_index + 1) * 2;
    }

    let mut num_discons = 0;
    for val in stats.discons.values() {
        num_discons += *val as i32;
    }
    for val in stats.desyncs.values() {
        num_discons += *val as i32;
    }
    req.qm_discons = num_discons;

    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        let ping = info.get_ping_string().as_str();
        for (dst, src) in req.qm_pings.iter_mut().zip(ping.as_bytes().iter()) {
            *dst = *src;
        }
    }

    let (bot_id, room_id) = config.get_qm_config();
    req.qm_bot_id = bot_id;
    req.qm_room_id = room_id;

    if let Some(global) = get_global_data().and_then(|data| data.read().ok()) {
        req.exe_crc = global.exe_crc;
        req.ini_crc = global.ini_crc;
    }

    if let Some(queue) = get_peer_message_queue() {
        if let Ok(mut queue) = queue.lock() {
            queue.add_request(req);
        }
    }

    set_window_enabled(&state.button_widen, false);
    set_window_hidden(&state.button_start, true);
    set_window_hidden(&state.button_stop, false);
    enable_options_gadgets(state, false);

    if let Some(ladder) = ladder_info.as_ref() {
        let profile_id = get_gamespy_info()
            .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
            .unwrap_or(0);
        let mut ladder_prefs = LadderPreferences::new();
        let _ = ladder_prefs.load_profile(profile_id);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_secs() as i64)
            .unwrap_or(0);
        let pref = LadderPref {
            last_play_date: timestamp,
            address: ladder.address.as_str().to_string(),
            port: ladder.port,
            name: ladder.name.clone(),
        };
        ladder_prefs.add_recent_ladder(pref);
        ladder_prefs.write();
    }
}

#[derive(Default)]
struct OptionPreferences {
    prefs: HashMap<String, String>,
}

impl OptionPreferences {
    fn new() -> Self {
        let mut prefs = OptionPreferences::default();
        let _ = prefs.load("Options.ini");
        prefs
    }

    fn load(&mut self, filename: &str) -> bool {
        let data = match std::fs::read_to_string(filename) {
            Ok(data) => data,
            Err(_) => return false,
        };
        self.prefs.clear();
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let mut parts = line.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();
            if !key.is_empty() {
                self.prefs.insert(key.to_string(), value.to_string());
            }
        }
        true
    }

    fn get_firewall_behavior(&self) -> i32 {
        self.prefs
            .get("FirewallBehavior")
            .and_then(|value| value.parse::<i32>().ok())
            .map(|value| value.max(0))
            .unwrap_or(0)
    }
}
