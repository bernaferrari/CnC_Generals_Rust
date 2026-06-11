//! WOLGameSetupMenu.cpp callback port.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::display::image::{get_mapped_image_collection, Image};
use crate::game_text::GameText;
use crate::gamespy_game::{
    push_gamespy_game_options, with_gamespy_game_info, with_gamespy_game_info_mut,
};
use crate::gamespy_overlay::{
    close_all_overlays, close_overlay, gs_message_box_ok, raise_gs_message_box, toggle_overlay,
    GameSpyOverlayType,
};
use crate::gui::challenge_generals::get_challenge_generals;
use crate::gui::gadgets::ComboBoxItem;
use crate::gui::game_window::WindowInstanceData;
use crate::gui::{
    get_shell, with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, WindowStatus,
};
use crate::map_util::{
    find_draw_positions, get_map_cache_manager, get_map_preview_image,
    get_supply_and_tech_image_locations,
};
use game_engine::common::ini::ini_map_cache::MapMetaData;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::preferences::CustomMatchPreferences;
use game_engine::common::rts::player_template::get_player_template_store;
use game_network::gamespy::buddy_thread::get_buddy_message_queue;
use game_network::gamespy::peer_defs::{
    default_gamespy_colors, get_gamespy_info, BuddyMessage, GameSpyColor, PlayerInfo,
};
use game_network::gamespy::peer_thread::{
    get_peer_message_queue, PeerRequest, PeerRequestType, PeerResponseType,
};
use game_network::gamespy::persistent_storage_thread::{
    get_ps_message_queue, PSPlayerStats, PSRequest, PSRequestType, PSResponseType,
};
use game_network::{
    parse_ascii_string_to_game_info, FirewallBehaviorType, GameInfo, Money, SlotState, MAX_SLOTS,
    PLAYERTEMPLATE_MIN, PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM,
};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const SUPPLY_TECH_SIZE: i32 = 15;

fn slot_state_from_int(value: i32) -> SlotState {
    match value {
        0 => SlotState::Open,
        1 => SlotState::Closed,
        2 => SlotState::EasyAI,
        3 => SlotState::MedAI,
        4 => SlotState::BrutalAI,
        5 => SlotState::Player,
        _ => SlotState::Open,
    }
}

#[derive(Default)]
struct WolGameSetupState {
    parent_id: i32,
    button_back_id: i32,
    button_start_id: i32,
    button_emote_id: i32,
    button_select_map_id: i32,
    button_communicator_id: i32,
    text_entry_chat_id: i32,
    text_entry_map_display_id: i32,
    listbox_chat_id: i32,
    map_window_id: i32,
    map_select_preview_id: i32,
    checkbox_use_stats_id: i32,
    checkbox_limit_superweapons_id: i32,
    combo_box_starting_cash_id: i32,
    checkbox_limit_armies_id: i32,
    combo_box_player_ids: [i32; MAX_SLOTS],
    static_text_player_ids: [i32; MAX_SLOTS],
    button_accept_ids: [i32; MAX_SLOTS],
    combo_box_color_ids: [i32; MAX_SLOTS],
    combo_box_template_ids: [i32; MAX_SLOTS],
    combo_box_team_ids: [i32; MAX_SLOTS],
    button_map_start_position_ids: [i32; MAX_SLOTS],
    generic_ping_ids: [i32; MAX_SLOTS],
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_back: Option<Rc<RefCell<GameWindow>>>,
    button_start: Option<Rc<RefCell<GameWindow>>>,
    button_emote: Option<Rc<RefCell<GameWindow>>>,
    button_select_map: Option<Rc<RefCell<GameWindow>>>,
    text_entry_chat: Option<Rc<RefCell<GameWindow>>>,
    text_entry_map_display: Option<Rc<RefCell<GameWindow>>>,
    listbox_chat: Option<Rc<RefCell<GameWindow>>>,
    map_window: Option<Rc<RefCell<GameWindow>>>,
    checkbox_use_stats: Option<Rc<RefCell<GameWindow>>>,
    checkbox_limit_superweapons: Option<Rc<RefCell<GameWindow>>>,
    combo_box_starting_cash: Option<Rc<RefCell<GameWindow>>>,
    checkbox_limit_armies: Option<Rc<RefCell<GameWindow>>>,
    wol_map_select_layout: Option<Rc<RefCell<WindowLayout>>>,
    ping_images: [Option<Image>; 3],
    next_screen: Option<String>,
    is_shutting_down: bool,
    button_pushed: bool,
    raise_message_boxes: bool,
    launch_game_next: bool,
    init_done: bool,
    last_slotlist_time: u128,
    enter_time: u128,
    initial_accept_enable: bool,
    slotlist_updates_enabled: bool,
}

static WOL_GAME_SETUP_STATE: OnceLock<Mutex<WolGameSetupState>> = OnceLock::new();

fn game_setup_state() -> &'static Mutex<WolGameSetupState> {
    WOL_GAME_SETUP_STATE.get_or_init(|| Mutex::new(WolGameSetupState::default()))
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn is_host() -> bool {
    get_gamespy_info()
        .and_then(|info| info.lock().ok().map(|guard| guard.am_i_host()))
        .unwrap_or_else(|| with_gamespy_game_info(|game| game.am_i_host()))
}

fn set_window_image(win: &Option<Rc<RefCell<GameWindow>>>, image_name: &str) {
    let Some(win) = win else {
        return;
    };
    if image_name.is_empty() {
        return;
    }

    let mut image = Image::with_name(image_name);
    if let Some(collection) = get_mapped_image_collection().try_read() {
        if let Some(found) = collection.find_image_by_name(image_name) {
            image.set_filename(found.get_filename());
        }
    }

    let mut guard = win.borrow_mut();
    if guard.set_enabled_image(0, image).is_ok() {
        guard.set_status(WindowStatus::IMAGE);
    }
}

fn set_window_tooltip(window: &GameWindow, tooltip: &str) {
    let id = window.get_id() as i32;
    with_window_manager(|manager| {
        if let Some(win) = manager.get_window_by_id(id) {
            win.borrow_mut().set_tooltip(tooltip);
        }
    });
}

fn combo_box_selected_index(window: &Option<Rc<RefCell<GameWindow>>>) -> Option<usize> {
    let Some(window) = window.as_ref() else {
        return None;
    };
    let guard = window.borrow();
    guard.combo_box().and_then(|combo| combo.selected_index())
}

fn set_combo_box_selected_by_data(window: &Option<Rc<RefCell<GameWindow>>>, data: i32) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    if let Some(combo) = guard.combo_box_mut() {
        if let Some(index) = combo
            .items()
            .iter()
            .position(|item| item.data == Some(data))
        {
            guard.set_combo_box_selected(index, false);
        }
    }
}

fn map_start_waypoint_name(index: usize) -> String {
    format!("Player_{}_Start", index + 1)
}

fn position_start_buttons(state: &mut WolGameSetupState, meta: Option<&MapMetaData>) {
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
        let Some(button) = with_window_manager(|manager| {
            manager.get_window_by_id(state.button_map_start_position_ids[i])
        }) else {
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

fn update_map_start_spots(state: &mut WolGameSetupState, meta: Option<&MapMetaData>) {
    for button_id in state.button_map_start_position_ids {
        if let Some(button) = with_window_manager(|manager| manager.get_window_by_id(button_id)) {
            let mut guard = button.borrow_mut();
            let _ = guard.set_text("");
            guard.set_tooltip(&GameText::fetch("TOOLTIP:StartPosition"));
        }
    }

    let Some(meta) = meta else {
        return;
    };
    let max_players = meta.num_players.max(0) as i32;
    with_gamespy_game_info(|info| {
        for i in 0..MAX_SLOTS {
            if let Some(slot) = info.get_slot(i) {
                let pos = slot.get_start_pos();
                if pos >= 0
                    && pos < max_players
                    && slot.get_player_template() > PLAYERTEMPLATE_OBSERVER
                {
                    let button_id = state.button_map_start_position_ids[pos as usize];
                    if let Some(button) =
                        with_window_manager(|manager| manager.get_window_by_id(button_id))
                    {
                        let mut guard = button.borrow_mut();
                        let number_key = format!("NUMBER:{}", i + 1);
                        let label = GameText::fetch(&number_key);
                        let _ = guard.set_text(&label);
                        let tooltip = GameText::fetch("TOOLTIP:StartPositionN")
                            .replace("%d", &(i + 1).to_string());
                        guard.set_tooltip(&tooltip);
                    }
                }
            }
        }
    });
}

fn map_display_name(map_name: &str, meta: Option<&MapMetaData>) -> String {
    if let Some(meta) = meta {
        if !meta.display_name.is_empty() {
            return meta.display_name.as_str().to_string();
        }
    }
    map_name
        .rsplit('\\')
        .next()
        .unwrap_or(map_name)
        .rsplit('/')
        .next()
        .unwrap_or(map_name)
        .trim_end_matches(".map")
        .to_string()
}

fn update_map_preview(state: &mut WolGameSetupState) {
    let map_name = with_gamespy_game_info(|info| info.get_map().to_string());
    if map_name.is_empty() {
        return;
    }
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

fn get_additional_disconnects_from_user_file(profile_id: i32) -> i32 {
    let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) else {
        return 0;
    };
    if info.get_local_profile_id() != profile_id {
        return 0;
    }
    let additional = info.get_additional_disconnects();
    if additional > 0 {
        drop(info);
        if let Some(mut info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
            info.clear_additional_disconnects();
        }
    }
    additional
}

fn player_tooltip(window: &GameWindow, _inst: &WindowInstanceData, _mouse: u32) {
    let state = game_setup_state().lock().unwrap_or_else(|e| e.into_inner());
    let window_id = window.get_id() as i32;
    let mut slot_idx = None;
    for i in 0..MAX_SLOTS {
        if window_id == state.combo_box_player_ids[i]
            || window_id == state.static_text_player_ids[i]
        {
            slot_idx = Some(i);
            break;
        }
    }
    let Some(slot_idx) = slot_idx else {
        return;
    };

    let info = get_gamespy_info().and_then(|info| info.lock().ok());
    let Some(info) = info else {
        return;
    };
    let game = with_gamespy_game_info(|game| game.clone());
    let Some(slot) = game.get_slot(slot_idx) else {
        return;
    };
    if !slot.is_human() {
        return;
    }

    let name = slot.get_name().to_string();
    let lower = name.to_lowercase();
    let Some(player_info) = info.get_player_info_map().get(&lower) else {
        return;
    };
    let profile_id = player_info.profile_id;

    let stats = get_ps_message_queue()
        .and_then(|queue| {
            queue
                .lock()
                .ok()
                .map(|queue| queue.find_player_stats_by_id(profile_id))
        })
        .unwrap_or_default();

    if stats.id == 0 {
        set_window_tooltip(window, &name);
        return;
    }

    let is_local = game.get_local_slot_num() == slot_idx as i32;
    let locale_key = format!("WOL:Locale{:02}", stats.locale);
    let locale = GameText::fetch(&locale_key);

    let total_wins: i32 = stats.wins.values().map(|v| *v as i32).sum();
    let total_losses: i32 = stats.losses.values().map(|v| *v as i32).sum();
    let mut total_discons: i32 = stats.discons.values().map(|v| *v as i32).sum();
    total_discons += stats.desyncs.values().map(|v| *v as i32).sum::<i32>();
    total_discons += get_additional_disconnects_from_user_file(profile_id);

    let mut favorite_side = GameText::fetch("GUI:None");
    let mut num_games = 0;
    let mut favorite = 0;
    for (key, value) in stats.games.iter() {
        if *value as i32 >= num_games {
            num_games = *value as i32;
            favorite = *key;
        }
    }
    if num_games == 0 {
        favorite_side = GameText::fetch("GUI:None");
    } else if stats.games_as_random >= num_games {
        favorite_side = GameText::fetch("GUI:Random");
    } else {
        let store = get_player_template_store();
        if let Some(template) = store.get_nth_player_template(favorite) {
            let side_key = format!("SIDE:{}", template.side.as_str());
            favorite_side = GameText::fetch(&side_key);
        }
    }

    let base = GameText::fetch("TOOLTIP:StagingPlayerInfo");
    let player_info = base
        .replace("%1", &locale)
        .replace("%2", "0")
        .replace("%3", &total_wins.to_string())
        .replace("%4", &total_losses.to_string())
        .replace("%5", &total_discons.to_string())
        .replace("%6", &favorite_side);

    let mut tooltip = if is_local {
        GameText::fetch("TOOLTIP:LocalPlayer").replace("%s", &name)
    } else if info.get_buddy_map().contains_key(&profile_id) {
        GameText::fetch("TOOLTIP:BuddyPlayer").replace("%s", &name)
    } else if profile_id != 0 {
        GameText::fetch("TOOLTIP:ProfiledPlayer").replace("%s", &name)
    } else {
        GameText::fetch("TOOLTIP:GenericPlayer").replace("%s", &name)
    };

    tooltip.push_str(&player_info);
    set_window_tooltip(window, &tooltip);
}

fn game_accept_tooltip(window: &GameWindow, _inst: &WindowInstanceData, mouse: u32) {
    let x = (mouse & 0xFFFF) as i16 as i32;
    let y = (mouse >> 16) as i16 as i32;
    let (win_x, win_y) = window.get_screen_position();
    let (win_w, win_h) = window.get_size();
    if x > win_x && x < win_x + win_w && y > win_y && y < win_y + win_h {
        set_window_tooltip(window, &GameText::fetch("TOOLTIP:GameAcceptance"));
    }
}

fn ping_tooltip(window: &GameWindow, _inst: &WindowInstanceData, mouse: u32) {
    let x = (mouse & 0xFFFF) as i16 as i32;
    let y = (mouse >> 16) as i16 as i32;
    let (win_x, win_y) = window.get_screen_position();
    let (win_w, win_h) = window.get_size();
    if x > win_x && x < win_x + win_w && y > win_y && y < win_y + win_h {
        set_window_tooltip(window, &GameText::fetch("TOOLTIP:ConnectionSpeed"));
    }
}

fn map_selector_tooltip(window: &GameWindow, _inst: &WindowInstanceData, mouse: u32) {
    let x = (mouse & 0xFFFF) as i16 as i32;
    let y = (mouse >> 16) as i16 as i32;
    let (pixel_x, pixel_y) = window.get_screen_position();

    let supply_and_tech = get_supply_and_tech_image_locations();
    let guard = supply_and_tech.lock().unwrap_or_else(|e| e.into_inner());
    let tech_positions = &guard.tech_positions;
    let supply_positions = &guard.supply_positions;

    for pos in tech_positions {
        if x > pixel_x + pos.x
            && x < pixel_x + pos.x + SUPPLY_TECH_SIZE
            && y > pixel_y + pos.y
            && y < pixel_y + pos.y + SUPPLY_TECH_SIZE
        {
            set_window_tooltip(window, &GameText::fetch("TOOLTIP:TechBuilding"));
            return;
        }
    }

    for pos in supply_positions {
        if x > pixel_x + pos.x
            && x < pixel_x + pos.x + SUPPLY_TECH_SIZE
            && y > pixel_y + pos.y
            && y < pixel_y + pos.y + SUPPLY_TECH_SIZE
        {
            set_window_tooltip(window, &GameText::fetch("TOOLTIP:SupplyDock"));
            return;
        }
    }
}

fn populate_color_combo(index: usize, state: &mut WolGameSetupState, is_observer: bool) {
    let Some(window) =
        with_window_manager(|manager| manager.get_window_by_id(state.combo_box_color_ids[index]))
    else {
        return;
    };
    let mut guard = window.borrow_mut();
    let Some(combo) = guard.combo_box_mut() else {
        return;
    };

    let num_colors = with_multiplayer_settings(|settings| settings.get_num_colors());
    let mut available = vec![true; num_colors.max(0) as usize];
    with_gamespy_game_info(|game| {
        for i in 0..MAX_SLOTS {
            if i == index {
                continue;
            }
            if let Some(slot) = game.get_slot(i) {
                let color = slot.get_color();
                if color >= 0 && (color as usize) < available.len() {
                    available[color as usize] = false;
                }
            }
        }
    });

    let was_observer = combo.items().len() == 1;
    combo.clear();

    let random_label = if is_observer {
        GameText::fetch("GUI:None")
    } else {
        GameText::fetch("GUI:???")
    };
    combo.add_item(ComboBoxItem::new(u32::MAX, random_label).with_data(-1));

    if is_observer {
        let _ = combo.select_index(0);
        return;
    }

    with_multiplayer_settings(|settings| {
        for (idx, def) in settings.color_definitions.iter().enumerate() {
            if idx < available.len() && !available[idx] {
                continue;
            }
            let tooltip = def.get_tooltip_name().as_str();
            let label = GameText::fetch(tooltip);
            let text = if label.starts_with("GUI:") {
                def.name.as_str().to_string()
            } else {
                label
            };
            combo.add_item(ComboBoxItem::new(idx as u32, text).with_data(idx as i32));
        }
    });

    if was_observer {
        let _ = combo.select_index(0);
    }
}

fn populate_template_combo(index: usize, state: &mut WolGameSetupState, allow_observers: bool) {
    let Some(window) = with_window_manager(|manager| {
        manager.get_window_by_id(state.combo_box_template_ids[index])
    }) else {
        return;
    };
    let mut guard = window.borrow_mut();
    let Some(combo) = guard.combo_box_mut() else {
        return;
    };
    combo.clear();

    combo.add_item(
        ComboBoxItem::new(PLAYERTEMPLATE_RANDOM as u32, GameText::fetch("GUI:Random"))
            .with_data(PLAYERTEMPLATE_RANDOM),
    );

    let store = get_player_template_store();
    let mut seen_sides = HashSet::new();
    let old_factions_only = with_gamespy_game_info(|game| game.old_factions_only());

    for idx in 0..store.get_player_template_count() {
        let Some(template) = store.get_nth_player_template(idx) else {
            continue;
        };
        if template.starting_building.is_empty() {
            continue;
        }
        if old_factions_only && !template.old_faction {
            continue;
        }
        if let Some(generals) = get_challenge_generals() {
            if let Some(persona) = generals.general_by_template_name(template.name.as_str()) {
                if !persona.is_starting_enabled() {
                    continue;
                }
            }
        }
        let side_key = format!("SIDE:{}", template.side.as_str());
        if seen_sides.contains(&side_key) {
            continue;
        }
        seen_sides.insert(side_key.clone());
        let label = GameText::fetch(&side_key);
        combo.add_item(ComboBoxItem::new(idx as u32, label).with_data(idx as i32));
    }

    if allow_observers {
        combo.add_item(
            ComboBoxItem::new(
                PLAYERTEMPLATE_OBSERVER as u32,
                GameText::fetch("GUI:Observer"),
            )
            .with_data(PLAYERTEMPLATE_OBSERVER),
        );
    }

    let _ = combo.select_index(0);
}

fn populate_team_combo(index: usize, state: &mut WolGameSetupState, is_observer: bool) {
    let Some(window) =
        with_window_manager(|manager| manager.get_window_by_id(state.combo_box_team_ids[index]))
    else {
        return;
    };
    let mut guard = window.borrow_mut();
    let Some(combo) = guard.combo_box_mut() else {
        return;
    };
    combo.clear();

    combo.add_item(ComboBoxItem::new(0, GameText::fetch("Team:0")).with_data(-1));
    if is_observer {
        let _ = combo.select_index(0);
        return;
    }
    for i in 0..(MAX_SLOTS / 2) {
        let team_key = format!("Team:{}", i + 1);
        let label = GameText::fetch(&team_key);
        combo.add_item(ComboBoxItem::new((i + 1) as u32, label).with_data(i as i32));
    }
    let _ = combo.select_index(0);
}

fn populate_starting_cash_combo(state: &mut WolGameSetupState) {
    let Some(combo_window) = state.combo_box_starting_cash.as_ref() else {
        return;
    };
    let mut guard = combo_window.borrow_mut();
    let Some(combo) = guard.combo_box_mut() else {
        return;
    };
    combo.clear();

    let current_cash = with_gamespy_game_info(|info| info.get_starting_cash().count_money());
    let mut selected_index = None;

    with_multiplayer_settings(|settings| {
        for (idx, entry) in settings.starting_money_choices.iter().enumerate() {
            let label = GameText::fetch("GUI:StartingMoneyFormat")
                .replace("%d", &entry.money.count_money().to_string());
            combo.add_item(
                ComboBoxItem::new(idx as u32, label).with_data(entry.money.count_money() as i32),
            );
            if entry.money.count_money() == current_cash {
                selected_index = Some(idx);
            }
        }
    });

    if let Some(index) = selected_index {
        let _ = combo.select_index(index);
    }
}

fn enable_accept_controls(state: &mut WolGameSetupState, enabled: bool, slot_num: Option<usize>) {
    let slot_num = slot_num.unwrap_or_else(|| {
        with_gamespy_game_info(|info| info.get_local_slot_num().max(0) as usize)
    });
    let info = with_gamespy_game_info(|info| info.clone());
    let Some(slot) = info.get_slot(slot_num) else {
        return;
    };
    let is_observer = slot.get_player_template() == PLAYERTEMPLATE_OBSERVER;

    if !is_host() {
        if let Some(button_start) = state.button_start.as_ref() {
            let _ = button_start.borrow_mut().enable(enabled);
        }
    }

    for id in [
        state.combo_box_color_ids[slot_num],
        state.combo_box_team_ids[slot_num],
    ] {
        if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(id)) {
            let mut guard = window.borrow_mut();
            let _ = guard.enable(enabled && !is_observer);
        }
    }

    if let Some(window) = with_window_manager(|manager| {
        manager.get_window_by_id(state.combo_box_template_ids[slot_num])
    }) {
        let _ = window.borrow_mut().enable(enabled);
    }

    let mut can_choose_start = !is_observer;
    if is_host() {
        for i in 0..MAX_SLOTS {
            if info.get_slot(i).map(|slot| slot.is_ai()).unwrap_or(false) {
                can_choose_start = true;
                break;
            }
        }
    }

    if slot_num as i32 == info.get_local_slot_num() {
        let can_enable = if slot.has_map() { enabled } else { enabled };
        for id in state.button_map_start_position_ids {
            if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(id)) {
                let _ = window.borrow_mut().enable(can_enable && can_choose_start);
            }
        }
    }
}

fn update_slot_list(state: &mut WolGameSetupState) {
    if !state.slotlist_updates_enabled {
        return;
    }
    let game = with_gamespy_game_info(|info| info.clone());
    if !game.is_in_game() {
        return;
    }

    for i in 0..MAX_SLOTS {
        let slot = match game.get_slot(i) {
            Some(slot) => slot.clone(),
            None => continue,
        };

        if is_host() && slot.is_ai() {
            enable_accept_controls(state, true, Some(i));
        } else if i as i32 == game.get_local_slot_num() {
            if slot.is_accepted() && !is_host() {
                enable_accept_controls(state, false, None);
            } else {
                enable_accept_controls(state, true, None);
            }
        } else if is_host() {
            enable_accept_controls(state, false, Some(i));
        }

        if let Some(combo) =
            with_window_manager(|manager| manager.get_window_by_id(state.combo_box_player_ids[i]))
        {
            let mut guard = combo.borrow_mut();
            if let Some(widget) = guard.combo_box_mut() {
                if slot.is_human() {
                    widget.set_text(slot.get_name());
                } else {
                    let _ = widget.select_item(slot.get_state() as u32);
                }
            }
        }

        if let Some(button) =
            with_window_manager(|manager| manager.get_window_by_id(state.button_accept_ids[i]))
        {
            let mut guard = button.borrow_mut();
            if slot.is_human() {
                if i != 0 {
                    let _ = guard.hide(false);
                    let _ = guard.enable(slot.is_accepted());
                }
            } else {
                let _ = guard.hide(true);
            }
        }

        if !is_host() {
            if let Some(combo) = with_window_manager(|manager| {
                manager.get_window_by_id(state.combo_box_player_ids[i])
            }) {
                let _ = combo.borrow_mut().enable(false);
            }
        }

        populate_color_combo(
            i,
            state,
            slot.get_player_template() == PLAYERTEMPLATE_OBSERVER,
        );
        if let Some(window) =
            with_window_manager(|manager| manager.get_window_by_id(state.combo_box_color_ids[i]))
        {
            set_combo_box_selected_by_data(&Some(window), slot.get_color());
        }
        if let Some(window) =
            with_window_manager(|manager| manager.get_window_by_id(state.combo_box_team_ids[i]))
        {
            set_combo_box_selected_by_data(&Some(window), slot.get_team_number());
        }
        if let Some(window) =
            with_window_manager(|manager| manager.get_window_by_id(state.combo_box_template_ids[i]))
        {
            set_combo_box_selected_by_data(&Some(window), slot.get_player_template());
        }
    }

    update_map_preview(state);
    display_game_options(state);

    for i in 0..MAX_SLOTS {
        let Some(slot) = game.get_slot(i) else {
            continue;
        };
        if let Some(ping_window) =
            with_window_manager(|manager| manager.get_window_by_id(state.generic_ping_ids[i]))
        {
            let mut guard = ping_window.borrow_mut();
            if slot.is_human() {
                if let Some(image) = state.ping_images[0].clone() {
                    let _ = guard.set_enabled_image(0, image);
                }
                let _ = guard.hide(false);
            } else {
                let _ = guard.hide(true);
            }
        }
    }
}

fn display_game_options(state: &mut WolGameSetupState) {
    let game = with_gamespy_game_info(|game| game.clone());

    if let Some(window) = state.checkbox_use_stats.as_ref() {
        let is_using_stats = game.get_use_stats() != 0;
        let mut guard = window.borrow_mut();
        if let Some(check) = guard.check_box_mut() {
            if check.is_checked() != is_using_stats {
                check.set_checked(is_using_stats);
            }
        }
        let tooltip = if is_using_stats {
            "TOOLTIP:UseStatsOn"
        } else {
            "TOOLTIP:UseStatsOff"
        };
        guard.set_tooltip(&GameText::fetch(tooltip));
    }

    if let Some(window) = state.checkbox_limit_armies.as_ref() {
        let old_factions_only = game.old_factions_only();
        let mut guard = window.borrow_mut();
        if let Some(check) = guard.check_box_mut() {
            if check.is_checked() != old_factions_only {
                check.set_checked(old_factions_only);
                for i in 0..MAX_SLOTS {
                    populate_template_combo(i, state, true);
                    handle_template_selection(state, i);
                }
            }
        }
    }

    if let Some(window) = state.checkbox_limit_superweapons.as_ref() {
        let limit = game.get_superweapon_restriction() != 0;
        let mut guard = window.borrow_mut();
        if let Some(check) = guard.check_box_mut() {
            if check.is_checked() != limit {
                check.set_checked(limit);
            }
        }
    }

    if let Some(window) = state.combo_box_starting_cash.as_ref() {
        let mut guard = window.borrow_mut();
        if let Some(combo) = guard.combo_box_mut() {
            let current = game.get_starting_cash().count_money() as i32;
            if let Some(index) = combo
                .items()
                .iter()
                .position(|item| item.data == Some(current))
            {
                if combo.selected_index() != Some(index) {
                    let _ = combo.select_index(index);
                }
            }
        }
    }
}

fn handle_color_selection(state: &mut WolGameSetupState, index: usize) {
    let combo =
        with_window_manager(|manager| manager.get_window_by_id(state.combo_box_color_ids[index]));
    let Some(combo) = combo else {
        return;
    };
    let guard = combo.borrow();
    let Some(combo) = guard.combo_box() else {
        return;
    };
    let color = combo.selected_item_data().unwrap_or(-1);

    with_gamespy_game_info_mut(|game| {
        if let Some(slot) = game.get_slot_mut(index) {
            if color == slot.get_color() {
                return;
            }
            if color >= -1 {
                if color != -1 && game.is_color_taken(color, index as i32) {
                    return;
                }
                slot.set_color(color);
            }
        }
    });

    if is_host() {
        push_gamespy_game_options();
        update_slot_list(state);
    } else {
        let host_name = with_gamespy_game_info(|game| {
            game.get_slot(0)
                .map(|slot| slot.get_name().to_string())
                .unwrap_or_default()
        });
        let local_name = get_gamespy_info()
            .and_then(|info| {
                info.lock()
                    .ok()
                    .map(|guard| guard.get_local_name().as_str().to_string())
            })
            .unwrap_or_default();
        if with_gamespy_game_info(|game| {
            game.get_slot(index)
                .map(|slot| slot.get_name() == local_name)
                .unwrap_or(false)
        }) {
            let mut req = PeerRequest::default();
            req.request_type = PeerRequestType::UtmPlayer;
            req.id = "REQ/".to_string();
            req.nick = host_name;
            req.options = format!("Color={}", color);
            if let Some(queue) = get_peer_message_queue() {
                if let Ok(mut queue) = queue.lock() {
                    queue.add_request(req);
                }
            }
        }
    }
}

fn handle_template_selection(state: &mut WolGameSetupState, index: usize) {
    let combo = with_window_manager(|manager| {
        manager.get_window_by_id(state.combo_box_template_ids[index])
    });
    let Some(combo) = combo else {
        return;
    };
    let guard = combo.borrow();
    let Some(combo) = guard.combo_box() else {
        return;
    };
    let template = combo.selected_item_data().unwrap_or(PLAYERTEMPLATE_RANDOM);

    let old_template = with_gamespy_game_info(|game| {
        game.get_slot(index)
            .map(|slot| slot.get_player_template())
            .unwrap_or(PLAYERTEMPLATE_RANDOM)
    });
    if template == old_template {
        return;
    }

    with_gamespy_game_info_mut(|game| {
        if let Some(slot) = game.get_slot_mut(index) {
            slot.set_player_template(template);
            if old_template == PLAYERTEMPLATE_OBSERVER || template == PLAYERTEMPLATE_OBSERVER {
                slot.set_start_pos(-1);
                slot.set_team_number(-1);
            }
        }
        game.reset_accepted();
    });

    if is_host() {
        push_gamespy_game_options();
        update_slot_list(state);
    } else {
        let host_name = with_gamespy_game_info(|game| {
            game.get_slot(0)
                .map(|slot| slot.get_name().to_string())
                .unwrap_or_default()
        });
        let mut req = PeerRequest::default();
        req.request_type = PeerRequestType::UtmPlayer;
        req.id = "REQ/".to_string();
        req.nick = host_name;
        req.options = format!("PlayerTemplate={}", template);
        if let Some(queue) = get_peer_message_queue() {
            if let Ok(mut queue) = queue.lock() {
                queue.add_request(req);
            }
        }
    }
}

fn handle_team_selection(state: &mut WolGameSetupState, index: usize) {
    let combo =
        with_window_manager(|manager| manager.get_window_by_id(state.combo_box_team_ids[index]));
    let Some(combo) = combo else {
        return;
    };
    let guard = combo.borrow();
    let Some(combo) = guard.combo_box() else {
        return;
    };
    let team = combo.selected_item_data().unwrap_or(-1);

    with_gamespy_game_info_mut(|game| {
        if let Some(slot) = game.get_slot_mut(index) {
            if team != slot.get_team_number() {
                slot.set_team_number(team);
                game.reset_accepted();
            }
        }
    });

    if is_host() {
        push_gamespy_game_options();
        update_slot_list(state);
    } else {
        let host_name = with_gamespy_game_info(|game| {
            game.get_slot(0)
                .map(|slot| slot.get_name().to_string())
                .unwrap_or_default()
        });
        let mut req = PeerRequest::default();
        req.request_type = PeerRequestType::UtmPlayer;
        req.id = "REQ/".to_string();
        req.nick = host_name;
        req.options = format!("Team={}", team);
        if let Some(queue) = get_peer_message_queue() {
            if let Ok(mut queue) = queue.lock() {
                queue.add_request(req);
            }
        }
    }
}

fn handle_start_position_selection(state: &mut WolGameSetupState, player: usize, start_pos: i32) {
    with_gamespy_game_info_mut(|game| {
        if let Some(slot) = game.get_slot_mut(player) {
            if start_pos == slot.get_start_pos() {
                return;
            }
            if start_pos >= 0 && game.is_start_position_taken(start_pos, player as i32) {
                return;
            }
            slot.set_start_pos(start_pos);
        }
        game.reset_accepted();
    });

    if is_host() {
        push_gamespy_game_options();
        update_slot_list(state);
    } else {
        if state.slotlist_updates_enabled {
            let host_name = with_gamespy_game_info(|game| {
                game.get_slot(0)
                    .map(|slot| slot.get_name().to_string())
                    .unwrap_or_default()
            });
            let mut req = PeerRequest::default();
            req.request_type = PeerRequestType::UtmPlayer;
            req.id = "REQ/".to_string();
            req.nick = host_name;
            req.options = format!("StartPos={}", start_pos);
            if let Some(queue) = get_peer_message_queue() {
                if let Ok(mut queue) = queue.lock() {
                    queue.add_request(req);
                }
            }
        }
    }
}

fn handle_starting_cash_selection(state: &mut WolGameSetupState) {
    let Some(combo) = state.combo_box_starting_cash.as_ref() else {
        return;
    };
    let guard = combo.borrow();
    let Some(combo) = guard.combo_box() else {
        return;
    };
    let selected = combo.selected_item_data().unwrap_or(0).max(0) as u32;

    with_gamespy_game_info_mut(|game| {
        game.set_starting_cash(Money::new(selected));
        game.reset_accepted();
    });

    if is_host() {
        push_gamespy_game_options();
        update_slot_list(state);
    }
}

fn handle_limit_superweapons_click(state: &mut WolGameSetupState) {
    let Some(window) = state.checkbox_limit_superweapons.as_ref() else {
        return;
    };
    let is_checked = window
        .borrow()
        .check_box()
        .map(|check| check.is_checked())
        .unwrap_or(false);

    with_gamespy_game_info_mut(|game| {
        game.set_superweapon_restriction(if is_checked { 1 } else { 0 });
        game.reset_accepted();
    });

    if is_host() {
        push_gamespy_game_options();
        update_slot_list(state);
    }
}

fn send_stats_to_other_players(game: &GameInfo) {
    let Some(queue) = get_peer_message_queue() else {
        return;
    };
    let Some(ps_queue) = get_ps_message_queue() else {
        return;
    };

    let profile_id = get_gamespy_info()
        .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
        .unwrap_or(0);

    let stats = ps_queue
        .lock()
        .ok()
        .map(|queue| queue.find_player_stats_by_id(profile_id))
        .unwrap_or_default();

    let mut sub_stats = PSPlayerStats::default();
    sub_stats.id = stats.id;
    sub_stats.wins = stats.wins.clone();
    sub_stats.losses = stats.losses.clone();
    sub_stats.discons = stats.discons.clone();
    sub_stats.desyncs = stats.desyncs.clone();
    sub_stats.games = stats.games.clone();
    sub_stats.locale = stats.locale;
    sub_stats.games_as_random = stats.games_as_random;
    sub_stats.last_ladder_port = stats.last_ladder_port;
    sub_stats.last_ladder_host = stats.last_ladder_host.clone();

    let stats_string = game_network::gamespy::persistent_storage_thread::GameSpyPSMessageQueue::format_player_kv_pairs(&sub_stats);
    let options = format!("{} {}", profile_id, stats_string);

    let local_index = game.get_local_slot_num();
    if let Ok(mut queue) = queue.lock() {
        for i in 0..MAX_SLOTS {
            if i as i32 == local_index {
                continue;
            }
            if let Some(slot) = game.get_slot(i) {
                if slot.is_human() {
                    let mut req = PeerRequest::default();
                    req.request_type = PeerRequestType::UtmPlayer;
                    req.id = "STATS/".to_string();
                    req.nick = slot.get_name().to_string();
                    req.options = options.clone();
                    queue.add_request(req);
                }
            }
        }
    }
}

fn start_pressed(state: &mut WolGameSetupState) {
    let game = with_gamespy_game_info(|info| info.clone());
    if !game.is_in_game() {
        return;
    }

    let map_name = game.get_map().to_string();
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let map_data = cache_guard.find_map(&map_name);
    let map_display_name = map_data
        .as_ref()
        .map(|meta| meta.display_name.as_str().to_string())
        .unwrap_or_else(|| map_display_name(&map_name, map_data.as_ref()));

    let mut is_ready = true;
    let mut all_have_map = true;
    let mut player_count = 0;
    let mut human_count = 0;

    for i in 0..MAX_SLOTS {
        if let Some(slot) = game.get_slot(i) {
            if slot.is_human() && !slot.is_accepted() {
                is_ready = false;
                if !slot.has_map() {
                    let msg = GameText::fetch("GUI:PlayerNoMap")
                        .replace("%s", slot.get_name())
                        .replace("%2", &map_display_name);
                    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                        info.add_text(
                            msg,
                            default_gamespy_colors()[GameSpyColor::Default as usize],
                            Some(state.listbox_chat_id as u32),
                        );
                    }
                    all_have_map = false;
                }
            }
            if slot.is_occupied() && slot.get_player_template() != PLAYERTEMPLATE_OBSERVER {
                player_count += 1;
                if slot.is_human() {
                    human_count += 1;
                }
            }
        }
    }

    if let Some(meta) = map_data.as_ref() {
        if meta.num_players < player_count {
            if is_host() {
                let msg = GameText::fetch("LAN:TooManyPlayers")
                    .replace("%d", &meta.num_players.to_string());
                if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                    info.add_text(
                        msg,
                        default_gamespy_colors()[GameSpyColor::Default as usize],
                        Some(state.listbox_chat_id as u32),
                    );
                }
            }
            return;
        }
    }

    let min_players = game_engine::common::ini::get_global_data()
        .and_then(|data| data.read().ok().map(|data| data.net_min_players))
        .unwrap_or(2);
    if min_players > 0 && human_count == 0 {
        if is_host() {
            let msg = GameText::fetch("GUI:NeedHumanPlayers");
            if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                info.add_text(
                    msg,
                    default_gamespy_colors()[GameSpyColor::Default as usize],
                    Some(state.listbox_chat_id as u32),
                );
            }
        }
        return;
    }

    if player_count < min_players {
        if is_host() {
            let msg =
                GameText::fetch("LAN:NeedMorePlayers").replace("%d", &player_count.to_string());
            if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                info.add_text(
                    msg,
                    default_gamespy_colors()[GameSpyColor::Default as usize],
                    Some(state.listbox_chat_id as u32),
                );
            }
        }
        return;
    }

    let mut num_random = 0;
    let mut teams = HashSet::new();
    for i in 0..MAX_SLOTS {
        if let Some(slot) = game.get_slot(i) {
            if slot.is_occupied() && slot.get_player_template() != PLAYERTEMPLATE_OBSERVER {
                let team = slot.get_team_number();
                if team >= 0 {
                    teams.insert(team);
                } else {
                    num_random += 1;
                }
            }
        }
    }

    if num_random + teams.len() < min_players as usize {
        if is_host() {
            let msg = GameText::fetch("LAN:NeedMoreTeams");
            if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                info.add_text(
                    msg,
                    default_gamespy_colors()[GameSpyColor::Default as usize],
                    Some(state.listbox_chat_id as u32),
                );
            }
        }
        return;
    }

    if num_random + teams.len() < 2 {
        let msg = GameText::fetch("GUI:SandboxMode");
        if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
            info.add_text(
                msg,
                default_gamespy_colors()[GameSpyColor::Default as usize],
                Some(state.listbox_chat_id as u32),
            );
        }
    }

    if is_ready {
        if let Some(queue) = get_peer_message_queue() {
            if let Ok(mut queue) = queue.lock() {
                let mut req = PeerRequest::default();
                req.request_type = PeerRequestType::StartGame;
                queue.add_request(req);
            }
        }

        send_stats_to_other_players(&game);

        if let Some(back) = state.button_back.as_ref() {
            let _ = back.borrow_mut().enable(false);
        }
        close_overlay(GameSpyOverlayType::Buddy);
        with_gamespy_game_info_mut(|info| info.start_game(0));
    } else if all_have_map {
        if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
            info.add_text(
                GameText::fetch("GUI:NotifiedStartIntent"),
                default_gamespy_colors()[GameSpyColor::Default as usize],
                Some(state.listbox_chat_id as u32),
            );
        }
        if let Some(queue) = get_peer_message_queue() {
            if let Ok(mut queue) = queue.lock() {
                let mut req = PeerRequest::default();
                req.request_type = PeerRequestType::UtmRoom;
                req.id = "HWS/".to_string();
                req.options = "true".to_string();
                queue.add_request(req);
            }
        }
    }
}

fn handle_buddy_responses() {
    let Some(queue) = get_buddy_message_queue() else {
        return;
    };
    let resp = {
        let mut queue = queue.lock().ok()?;
        queue.get_response()
    };
    let Some(resp) = resp else {
        return;
    };

    if let Some(info) = get_gamespy_info() {
        if let Ok(mut info) = info.lock() {
            match resp.response_type {
                game_network::gamespy::buddy_thread::BuddyResponseType::Login => {}
                game_network::gamespy::buddy_thread::BuddyResponseType::Disconnect => {}
                game_network::gamespy::buddy_thread::BuddyResponseType::Message => {
                    let message = resp.message_text.clone();
                    if !message.is_empty() {
                        let sender_nick = resp.message_nick.clone();
                        let buddy_msg = BuddyMessage::new(
                            resp.profile,
                            sender_nick.into(),
                            info.get_local_profile_id(),
                            info.get_local_base_name(),
                            message,
                        );
                        info.push_buddy_message(buddy_msg);
                    }
                }
                game_network::gamespy::buddy_thread::BuddyResponseType::Request => {
                    info.add_buddy_request(
                        resp.profile,
                        resp.request_nick.clone(),
                        resp.request_email.clone(),
                        resp.request_country_code.clone(),
                    );
                }
                game_network::gamespy::buddy_thread::BuddyResponseType::Status => {
                    info.update_buddy_status(
                        resp.profile,
                        resp.status_nick.clone(),
                        resp.status_email.clone(),
                        resp.status_country_code.clone(),
                        resp.status_location.clone(),
                        resp.status_value,
                        resp.status_string.clone(),
                    );
                }
            }
        }
    }
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
                        req.stats_rank_points =
                            game_network::rank_point_value::calculate_rank(&resp.player);
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

fn handle_slash_command(text: &str) -> bool {
    if !text.starts_with('/') {
        return false;
    }
    let mut parts = text[1..].split_whitespace();
    let cmd = parts.next().unwrap_or("").to_lowercase();

    if cmd == "host" {
        let msg = format!("Hosting qr2:{} thread:{}", 0, 0);
        if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
            info.add_text(
                msg,
                default_gamespy_colors()[GameSpyColor::Default as usize],
                None,
            );
        }
        return true;
    }
    if cmd == "me" {
        let action = text.strip_prefix("/me ").unwrap_or("");
        if !action.is_empty() {
            if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                info.send_chat(action.to_string(), true, None);
            }
            return true;
        }
    }
    false
}

fn next_selectable_player(game: &GameInfo, start: usize) -> Option<usize> {
    if !is_host() {
        return None;
    }
    for i in start..MAX_SLOTS {
        if let Some(slot) = game.get_slot(i) {
            if slot.get_start_pos() == -1
                && ((i as i32 == game.get_local_slot_num()
                    && slot.get_player_template() != PLAYERTEMPLATE_OBSERVER)
                    || slot.is_ai())
            {
                return Some(i);
            }
        }
    }
    None
}

fn first_selectable_player(game: &GameInfo) -> usize {
    if !is_host() {
        return game.get_local_slot_num().max(0) as usize;
    }
    let local_slot = game.get_local_slot_num().max(0) as usize;
    if let Some(slot) = game.get_slot(local_slot) {
        if slot.get_player_template() != PLAYERTEMPLATE_OBSERVER {
            return local_slot;
        }
    }
    for i in 0..MAX_SLOTS {
        if game.get_slot(i).map(|slot| slot.is_ai()).unwrap_or(false) {
            return i;
        }
    }
    local_slot
}

pub fn wol_game_setup_menu_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let mut state = game_setup_state().lock().unwrap_or_else(|e| e.into_inner());

    if with_gamespy_game_info(|info| info.is_game_in_progress()) {
        with_gamespy_game_info_mut(|info| info.set_game_in_progress(false));
        if get_gamespy_info()
            .and_then(|info| {
                info.lock()
                    .ok()
                    .and_then(|guard| guard.is_disconnected_after_game_start())
            })
            .is_some()
        {
            close_all_overlays();
            gs_message_box_ok(
                &GameText::fetch("GUI:GSErrorTitle"),
                &GameText::fetch("GUI:GSDisconnected"),
                None,
            );
            if let Some(info) = get_gamespy_info() {
                if let Ok(mut info) = info.lock() {
                    info.reset();
                }
            }
            let _ = get_shell().pop_immediate();
            return;
        }
        let _ = get_shell().pop_immediate();
        if get_peer_message_queue()
            .and_then(|queue| queue.lock().ok().map(|q| q.is_connected()))
            .unwrap_or(false)
        {
            let _ = get_shell().push("Menus/WOLCustomLobby.wnd", true);
        }
        return;
    }

    if let Some(info) = get_gamespy_info() {
        if let Ok(mut info) = info.lock() {
            info.set_current_group_room(0);
        }
    }

    state.next_screen = None;
    state.button_pushed = false;
    state.is_shutting_down = false;
    state.launch_game_next = false;
    state.initial_accept_enable = false;
    state.slotlist_updates_enabled = false;

    state.parent_id = name_to_id("GameSpyGameOptionsMenu.wnd:GameSpyGameOptionsMenuParent");
    state.button_back_id = name_to_id("GameSpyGameOptionsMenu.wnd:ButtonBack");
    state.button_start_id = name_to_id("GameSpyGameOptionsMenu.wnd:ButtonStart");
    state.text_entry_chat_id = name_to_id("GameSpyGameOptionsMenu.wnd:TextEntryChat");
    state.text_entry_map_display_id = name_to_id("GameSpyGameOptionsMenu.wnd:TextEntryMapDisplay");
    state.listbox_chat_id =
        name_to_id("GameSpyGameOptionsMenu.wnd:ListboxChatWindowGameSpyGameSetup");
    state.button_emote_id = name_to_id("GameSpyGameOptionsMenu.wnd:ButtonEmote");
    state.button_select_map_id = name_to_id("GameSpyGameOptionsMenu.wnd:ButtonSelectMap");
    state.checkbox_use_stats_id = name_to_id("GameSpyGameOptionsMenu.wnd:CheckBoxUseStats");
    state.map_window_id = name_to_id("GameSpyGameOptionsMenu.wnd:MapWindow");
    state.checkbox_limit_superweapons_id =
        name_to_id("GameSpyGameOptionsMenu.wnd:CheckboxLimitSuperweapons");
    state.combo_box_starting_cash_id =
        name_to_id("GameSpyGameOptionsMenu.wnd:ComboBoxStartingCash");
    state.checkbox_limit_armies_id = name_to_id("GameSpyGameOptionsMenu.wnd:CheckBoxLimitArmies");
    state.map_select_preview_id = name_to_id("WOLMapSelectMenu.wnd:WinMapPreview");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.button_back = manager.get_window_by_id(state.button_back_id);
        state.button_start = manager.get_window_by_id(state.button_start_id);
        state.button_emote = manager.get_window_by_id(state.button_emote_id);
        state.button_select_map = manager.get_window_by_id(state.button_select_map_id);
        state.text_entry_chat = manager.get_window_by_id(state.text_entry_chat_id);
        state.text_entry_map_display = manager.get_window_by_id(state.text_entry_map_display_id);
        state.listbox_chat = manager.get_window_by_id(state.listbox_chat_id);
        state.map_window = manager.get_window_by_id(state.map_window_id);
        state.checkbox_use_stats = manager.get_window_by_id(state.checkbox_use_stats_id);
        state.checkbox_limit_superweapons =
            manager.get_window_by_id(state.checkbox_limit_superweapons_id);
        state.combo_box_starting_cash = manager.get_window_by_id(state.combo_box_starting_cash_id);
        state.checkbox_limit_armies = manager.get_window_by_id(state.checkbox_limit_armies_id);
    });

    if let Some(window) = state.map_window.as_ref() {
        window
            .borrow_mut()
            .set_tooltip_callback(|window, inst, mouse| map_selector_tooltip(window, inst, mouse));
    }

    state.ping_images = [
        Some(Image::with_name("Ping03")),
        Some(Image::with_name("Ping02")),
        Some(Image::with_name("Ping01")),
    ];

    for i in 0..MAX_SLOTS {
        state.combo_box_player_ids[i] =
            name_to_id(&format!("GameSpyGameOptionsMenu.wnd:ComboBoxPlayer{}", i));
        state.static_text_player_ids[i] =
            name_to_id(&format!("GameSpyGameOptionsMenu.wnd:StaticTextPlayer{}", i));
        state.combo_box_color_ids[i] =
            name_to_id(&format!("GameSpyGameOptionsMenu.wnd:ComboBoxColor{}", i));
        state.combo_box_template_ids[i] = name_to_id(&format!(
            "GameSpyGameOptionsMenu.wnd:ComboBoxPlayerTemplate{}",
            i
        ));
        state.combo_box_team_ids[i] =
            name_to_id(&format!("GameSpyGameOptionsMenu.wnd:ComboBoxTeam{}", i));
        state.button_accept_ids[i] =
            name_to_id(&format!("GameSpyGameOptionsMenu.wnd:ButtonAccept{}", i));
        state.button_map_start_position_ids[i] = name_to_id(&format!(
            "GameSpyGameOptionsMenu.wnd:ButtonMapStartPosition{}",
            i
        ));
        state.generic_ping_ids[i] =
            name_to_id(&format!("GameSpyGameOptionsMenu.wnd:GenericPing{}", i));

        if let Some(window) =
            with_window_manager(|manager| manager.get_window_by_id(state.combo_box_player_ids[i]))
        {
            let mut guard = window.borrow_mut();
            if let Some(combo) = guard.combo_box_mut() {
                combo.clear();
                if i == 0 && is_host() {
                    let local_name = get_gamespy_info()
                        .and_then(|info| {
                            info.lock()
                                .ok()
                                .map(|guard| guard.get_local_name().as_str().to_string())
                        })
                        .unwrap_or_else(|| GameText::fetch("GUI:Player"));
                    combo.add_item(ComboBoxItem::new(0, local_name));
                } else {
                    for state_value in [
                        SlotState::Open,
                        SlotState::Closed,
                        SlotState::EasyAI,
                        SlotState::MedAI,
                        SlotState::BrutalAI,
                    ] {
                        let label = match state_value {
                            SlotState::Open => GameText::fetch("GUI:Open"),
                            SlotState::Closed => GameText::fetch("GUI:Closed"),
                            SlotState::EasyAI => GameText::fetch("GUI:EasyAI"),
                            SlotState::MedAI => GameText::fetch("GUI:MediumAI"),
                            SlotState::BrutalAI => GameText::fetch("GUI:HardAI"),
                            _ => String::new(),
                        };
                        combo.add_item(ComboBoxItem::new(state_value as u32, label));
                    }
                }
            }
            guard.set_tooltip_callback(|window, inst, mouse| player_tooltip(window, inst, mouse));
        }

        if let Some(window) =
            with_window_manager(|manager| manager.get_window_by_id(state.static_text_player_ids[i]))
        {
            window
                .borrow_mut()
                .set_tooltip_callback(|window, inst, mouse| player_tooltip(window, inst, mouse));
        }

        if let Some(window) =
            with_window_manager(|manager| manager.get_window_by_id(state.button_accept_ids[i]))
        {
            window
                .borrow_mut()
                .set_tooltip_callback(|window, inst, mouse| {
                    game_accept_tooltip(window, inst, mouse)
                });
        }

        if let Some(window) =
            with_window_manager(|manager| manager.get_window_by_id(state.generic_ping_ids[i]))
        {
            window
                .borrow_mut()
                .set_tooltip_callback(|window, inst, mouse| ping_tooltip(window, inst, mouse));
        }
    }

    with_gamespy_game_info_mut(|game| {
        game.init();
        game.enter_game();
    });

    for i in 0..MAX_SLOTS {
        let is_observer = with_gamespy_game_info(|game| {
            game.get_slot(i)
                .map(|slot| slot.get_player_template() == PLAYERTEMPLATE_OBSERVER)
                .unwrap_or(false)
        });
        populate_color_combo(i, &mut state, is_observer);
        populate_template_combo(i, &mut state, true);
        populate_team_combo(i, &mut state, is_observer);
    }

    let map_cache = get_map_cache_manager();
    if let Ok(mut cache) = map_cache.lock() {
        cache.update_cache();
    }

    let is_host = is_host();

    if let Some(check) = state.checkbox_limit_armies.as_ref() {
        let _ = check.borrow_mut().enable(false);
    }
    if let Some(check) = state.checkbox_use_stats.as_ref() {
        let _ = check.borrow_mut().enable(false);
    }

    if is_host {
        let mut pref = CustomMatchPreferences::new();
        with_gamespy_game_info_mut(|game| {
            if let Some(slot) = game.get_slot_mut(0) {
                slot.set_color(pref.get_preferred_color());
                slot.set_player_template(pref.get_preferred_faction());
            }
            game.set_map(pref.get_preferred_map());
            game.set_starting_cash(pref.get_starting_cash());
            game.set_superweapon_restriction(if pref.get_superweapon_restricted() {
                1
            } else {
                0
            });
        });
        update_slot_list(&mut state);
    } else {
        let mut pref = CustomMatchPreferences::new();
        let host_name = with_gamespy_game_info(|game| {
            game.get_slot(0)
                .map(|slot| slot.get_name().to_string())
                .unwrap_or_default()
        });
        if let Some(queue) = get_peer_message_queue() {
            if let Ok(mut queue) = queue.lock() {
                let mut req = PeerRequest::default();
                req.request_type = PeerRequestType::UtmPlayer;
                req.id = "REQ/".to_string();
                req.nick = host_name.clone();
                req.options = format!("PlayerTemplate={}", pref.get_preferred_faction());
                queue.add_request(req.clone());
                req.options = format!("Color={}", pref.get_preferred_color());
                queue.add_request(req.clone());
            }
        }
        for i in 0..MAX_SLOTS {
            if let Some(window) = with_window_manager(|manager| {
                manager.get_window_by_id(state.combo_box_player_ids[i])
            }) {
                let _ = window.borrow_mut().enable(false);
            }
            if let Some(window) = with_window_manager(|manager| {
                manager.get_window_by_id(state.combo_box_color_ids[i])
            }) {
                let _ = window.borrow_mut().enable(false);
            }
            if let Some(window) = with_window_manager(|manager| {
                manager.get_window_by_id(state.combo_box_template_ids[i])
            }) {
                let _ = window.borrow_mut().enable(false);
            }
            if let Some(window) =
                with_window_manager(|manager| manager.get_window_by_id(state.combo_box_team_ids[i]))
            {
                let _ = window.borrow_mut().enable(false);
            }
            if let Some(window) = with_window_manager(|manager| {
                manager.get_window_by_id(state.button_map_start_position_ids[i])
            }) {
                let _ = window.borrow_mut().enable(false);
            }
        }
        if let Some(button) = state.button_start.as_ref() {
            let _ = button.borrow_mut().set_text(&GameText::fetch("GUI:Accept"));
            let _ = button.borrow_mut().enable(false);
        }
        if let Some(button) = state.button_select_map.as_ref() {
            let _ = button.borrow_mut().enable(false);
        }
        if let Some(check) = state.checkbox_limit_superweapons.as_ref() {
            let _ = check.borrow_mut().enable(false);
        }
        if let Some(combo) = state.combo_box_starting_cash.as_ref() {
            let _ = combo.borrow_mut().enable(false);
        }
    }

    let use_stats = with_gamespy_game_info(|game| game.get_use_stats() != 0);
    if use_stats {
        if let Some(check) = state.checkbox_limit_superweapons.as_ref() {
            let _ = check.borrow_mut().enable(false);
        }
        if let Some(combo) = state.combo_box_starting_cash.as_ref() {
            let _ = combo.borrow_mut().enable(false);
        }
        if let Some(check) = state.checkbox_limit_armies.as_ref() {
            let _ = check.borrow_mut().enable(false);
        }
    }

    if let Some(info) = get_gamespy_info() {
        if let Ok(mut info) = info.lock() {
            info.register_text_window(state.listbox_chat_id as u32);
        }
    }

    populate_starting_cash_combo(&mut state);
    update_map_preview(&mut state);

    if let Some(text_entry) = state.text_entry_chat.as_ref() {
        if let Some(widget) = text_entry.borrow_mut().text_entry_mut() {
            widget.set_text("");
        }
    }
    if let Some(listbox) = state.listbox_chat.as_ref() {
        if let Some(widget) = listbox.borrow_mut().list_box_mut() {
            widget.clear();
        }
    }

    state.init_done = true;
    state.raise_message_boxes = true;
    state.slotlist_updates_enabled = true;
    state.last_slotlist_time = 0;
    state.enter_time = now_ms();

    layout.hide(false);
}

pub fn wol_game_setup_menu_update(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let mut state = game_setup_state().lock().unwrap_or_else(|e| e.into_inner());

    if state.is_shutting_down
        && get_shell().is_anim_finished()
        && with_window_manager(|manager| manager.transitions_finished())
    {
        state.is_shutting_down = false;
        layout.hide(true);
        get_shell().shutdown_complete(layout, state.next_screen.is_some());
        if let Some(next) = state.next_screen.take() {
            let _ = get_shell().push(&next, false);
        }
        return;
    }

    if state.raise_message_boxes {
        raise_gs_message_box();
        state.raise_message_boxes = false;
    }

    if get_shell().is_anim_finished() {
        handle_buddy_responses();
        handle_persistent_storage_responses();

        if with_gamespy_game_info(|game| game.is_game_in_progress()) {
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
        }

        let mut allowed_messages = get_gamespy_info()
            .and_then(|info| {
                info.lock()
                    .ok()
                    .map(|guard| guard.get_max_messages_per_update())
            })
            .unwrap_or(10);

        while allowed_messages > 0 {
            allowed_messages -= 1;
            let resp = get_peer_message_queue()
                .and_then(|queue| queue.lock().ok().and_then(|mut queue| queue.get_response()));
            let Some(resp) = resp else {
                break;
            };

            match resp.response_type {
                PeerResponseType::FailedToHost => {
                    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                        info.add_text(
                            GameText::fetch("GUI:GSFailedToHost"),
                            default_gamespy_colors()[GameSpyColor::Default as usize],
                            None,
                        );
                    }
                }
                PeerResponseType::GameStart => {
                    if with_gamespy_game_info(|game| game.is_in_game()) {
                        send_stats_to_other_players(&with_gamespy_game_info(|game| game.clone()));
                        if let Some(back) = state.button_back.as_ref() {
                            let _ = back.borrow_mut().enable(false);
                        }
                        close_overlay(GameSpyOverlayType::Buddy);
                        with_gamespy_game_info_mut(|game| game.start_game(0));
                    }
                }
                PeerResponseType::PlayerChangedFlags | PeerResponseType::PlayerInfo => {
                    let mut p = PlayerInfo::default();
                    p.name = resp.nick.clone().into();
                    p.profile_id = resp.player_profile_id;
                    p.flags = resp.player_flags;
                    p.wins = resp.player_wins;
                    p.losses = resp.player_losses;
                    p.locale = resp.locale.clone().into();
                    p.rank_points = resp.player_rank_points;
                    p.side = resp.player_side;
                    p.preorder = resp.player_preorder;
                    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                        info.update_player_info(p, None);
                    }
                    update_slot_list(&mut state);
                    if resp.response_type == PeerResponseType::PlayerInfo {
                        push_gamespy_game_options();
                    }
                }
                PeerResponseType::PlayerJoin => {
                    let mut p = PlayerInfo::default();
                    p.name = resp.nick.clone().into();
                    p.profile_id = resp.player_profile_id;
                    p.flags = resp.player_flags;
                    p.wins = resp.player_wins;
                    p.losses = resp.player_losses;
                    p.locale = resp.locale.clone().into();
                    p.rank_points = resp.player_rank_points;
                    p.side = resp.player_side;
                    p.preorder = resp.player_preorder;

                    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                        info.update_player_info(p.clone(), None);
                    }

                    if p.profile_id != 0 {
                        if let Some(queue) = get_ps_message_queue() {
                            if let Ok(mut queue) = queue.lock() {
                                if queue.find_player_stats_by_id(p.profile_id).id == 0 {
                                    let mut req = PSRequest::default();
                                    req.request_type = PSRequestType::ReadPlayerStats;
                                    req.player.id = p.profile_id;
                                    queue.add_request(req);
                                }
                            }
                        }
                    }

                    if is_host() {
                        with_gamespy_game_info_mut(|game| {
                            let open_slot = (0..MAX_SLOTS)
                                .find(|&i| game.get_slot(i).map(|s| s.is_open()).unwrap_or(false));
                            if let Some(open_slot) = open_slot {
                                let mut new_slot = game_network::GameSlot::new();
                                new_slot.set_state(
                                    SlotState::Player,
                                    p.name.as_str().to_string(),
                                    resp.player_ip,
                                );
                                game.set_slot(open_slot, new_slot);
                                game.reset_accepted();
                            } else {
                                if let Some(queue) = get_peer_message_queue() {
                                    if let Ok(mut queue) = queue.lock() {
                                        let mut req = PeerRequest::default();
                                        req.request_type = PeerRequestType::UtmPlayer;
                                        req.id = "KICK/".to_string();
                                        req.nick = p.name.as_str().to_string();
                                        req.options = "GameFull".to_string();
                                        queue.add_request(req);
                                    }
                                }
                            }
                        });
                        push_gamespy_game_options();
                    }
                    update_slot_list(&mut state);
                }
                PeerResponseType::PlayerLeft => {
                    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                        info.player_left_group_room(resp.nick.clone().into());
                    }
                    if !with_gamespy_game_info(|game| game.is_game_in_progress()) {
                        with_gamespy_game_info_mut(|game| {
                            if is_host() {
                                let idx = game.get_slot_num_by_name(&resp.nick);
                                if idx >= 0 {
                                    if let Some(slot) = game.get_slot_mut(idx as usize) {
                                        slot.set_state(SlotState::Open, String::new(), 0);
                                        game.reset_accepted();
                                    }
                                }
                            }
                        });
                        push_gamespy_game_options();
                        update_slot_list(&mut state);
                    }
                }
                PeerResponseType::Message => {
                    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                        info.add_chat(
                            resp.nick.clone().into(),
                            resp.message_profile_id,
                            resp.text.clone(),
                            !resp.message_is_private,
                            resp.message_is_action,
                            Some(state.listbox_chat_id as u32),
                        );
                    }
                }
                PeerResponseType::Disconnect => {
                    let title = GameText::fetch("GUI:GSErrorTitle");
                    let reason_key = format!("GUI:GSDisconReason{}", resp.discon_reason as i32);
                    let body = GameText::fetch(&reason_key);
                    close_all_overlays();
                    gs_message_box_ok(&title, &body, None);
                    if let Some(info) = get_gamespy_info() {
                        if let Ok(mut info) = info.lock() {
                            info.reset();
                        }
                    }
                    let _ = get_shell().pop();
                }
                PeerResponseType::RoomUtm => {
                    let cmd = resp.command.clone();
                    if cmd == "SL" {
                        let mut game = with_gamespy_game_info(|game| game.clone());
                        let options = resp.command_options.trim().to_string();
                        let old_map_crc = game.get_map_crc();
                        let old_in_game = game.get_local_slot_num() >= 0;
                        let ok = parse_ascii_string_to_game_info(&mut game, &options);
                        if ok {
                            with_gamespy_game_info_mut(|info| {
                                *info = game.clone();
                            });
                            update_slot_list(&mut state);
                            let new_map_crc = game.get_map_crc();
                            if game.get_local_slot_num() >= 0 {
                                state.last_slotlist_time = now_ms();
                                if old_map_crc != new_map_crc || !old_in_game {
                                    let host_name = game
                                        .get_slot(0)
                                        .map(|slot| slot.get_name().to_string())
                                        .unwrap_or_default();
                                    let mut req = PeerRequest::default();
                                    req.request_type = PeerRequestType::UtmPlayer;
                                    req.id = "MAP".to_string();
                                    req.nick = host_name;
                                    req.options = if game
                                        .get_slot(game.get_local_slot_num() as usize)
                                        .map(|s| s.has_map())
                                        .unwrap_or(false)
                                    {
                                        "1".to_string()
                                    } else {
                                        "0".to_string()
                                    };
                                    if let Some(queue) = get_peer_message_queue() {
                                        if let Ok(mut queue) = queue.lock() {
                                            queue.add_request(req);
                                        }
                                    }
                                }
                                if !state.initial_accept_enable {
                                    if let Some(start) = state.button_start.as_ref() {
                                        let _ = start.borrow_mut().enable(true);
                                    }
                                    state.initial_accept_enable = true;
                                }
                            } else if state.last_slotlist_time != 0 {
                                state.button_pushed = true;
                                with_gamespy_game_info_mut(|info| info.reset());
                                if let Some(info) = get_gamespy_info() {
                                    if let Ok(mut info) = info.lock() {
                                        info.leave_staging_room();
                                    }
                                }
                                gs_message_box_ok(
                                    &GameText::fetch("GUI:GSErrorTitle"),
                                    &GameText::fetch("GUI:GSKicked"),
                                    None,
                                );
                                state.next_screen = Some("Menus/WOLCustomLobby.wnd".to_string());
                                let _ = get_shell().pop();
                            }
                        }
                    } else if cmd == "HWS" {
                        let game = with_gamespy_game_info(|game| game.clone());
                        let slot_num = game.get_local_slot_num();
                        if slot_num >= 0 {
                            if let Some(slot) = game.get_slot(slot_num as usize) {
                                if !slot.is_accepted() {
                                    if let Some(info) =
                                        get_gamespy_info().and_then(|info| info.lock().ok())
                                    {
                                        info.add_text(
                                            GameText::fetch("GUI:HostWantsToStart"),
                                            default_gamespy_colors()
                                                [GameSpyColor::Default as usize],
                                            Some(state.listbox_chat_id as u32),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                PeerResponseType::PlayerUtm => {
                    if resp.command == "STATS" {
                        if let Some(queue) = get_ps_message_queue() {
                            if let Ok(mut queue) = queue.lock() {
                                let stats = game_network::gamespy::persistent_storage_thread::GameSpyPSMessageQueue::parse_player_kv_pairs(&resp.command_options);
                                if stats.id != 0 && queue.find_player_stats_by_id(stats.id).id == 0
                                {
                                    queue.track_player_stats(stats);
                                }
                            }
                        }
                        continue;
                    }

                    let slot_num =
                        with_gamespy_game_info(|game| game.get_slot_num_by_name(&resp.nick));
                    if slot_num == 0 && !is_host() {
                        if resp.command == "KICK" {
                            state.button_pushed = true;
                            with_gamespy_game_info_mut(|info| info.reset());
                            if let Some(info) = get_gamespy_info() {
                                if let Ok(mut info) = info.lock() {
                                    info.leave_staging_room();
                                }
                            }
                            let mut message = GameText::fetch("GUI:GSKicked");
                            if resp.command_options.trim() == "GameStarted" {
                                message = GameText::fetch("GUI:GSKickedGameStarted");
                            } else if resp.command_options.trim() == "GameFull" {
                                message = GameText::fetch("GUI:GSKickedGameFull");
                            }
                            gs_message_box_ok(&GameText::fetch("GUI:GSErrorTitle"), &message, None);
                            state.next_screen = Some("Menus/WOLCustomLobby.wnd".to_string());
                            let _ = get_shell().pop();
                        }
                    } else if slot_num > 0 && is_host() {
                        if resp.command == "accept" {
                            with_gamespy_game_info_mut(|game| {
                                if let Some(slot) = game.get_slot_mut(slot_num as usize) {
                                    slot.set_accept();
                                }
                            });
                            push_gamespy_game_options();
                            update_slot_list(&mut state);
                        } else if resp.command == "MAP" {
                            let has_map =
                                resp.command_options.trim().parse::<i32>().unwrap_or(0) != 0;
                            with_gamespy_game_info_mut(|game| {
                                if let Some(slot) = game.get_slot_mut(slot_num as usize) {
                                    slot.set_map_availability(has_map);
                                }
                            });
                            update_slot_list(&mut state);
                        } else if resp.command == "REQ" {
                            let options = resp.command_options.trim();
                            let mut parts = options.split('=');
                            let key = parts.next().unwrap_or("");
                            let val_str = parts.next().unwrap_or("0");
                            let val = val_str.parse::<i32>().unwrap_or(0);
                            let mut should_unaccept = false;

                            with_gamespy_game_info_mut(|game| {
                                if let Some(slot) = game.get_slot_mut(slot_num as usize) {
                                    match key {
                                        "Color" => {
                                            if val >= -1
                                                && !game.is_color_taken(val, slot_num)
                                                && slot.get_player_template()
                                                    != PLAYERTEMPLATE_OBSERVER
                                            {
                                                slot.set_color(val);
                                            }
                                        }
                                        "PlayerTemplate" => {
                                            if val >= PLAYERTEMPLATE_MIN
                                                && val
                                                    < get_player_template_store()
                                                        .get_player_template_count()
                                            {
                                                let mut template = val;
                                                if game.old_factions_only() {
                                                    if let Some(template_info) =
                                                        get_player_template_store()
                                                            .get_nth_player_template(val)
                                                    {
                                                        if !template_info.old_faction {
                                                            template = PLAYERTEMPLATE_RANDOM;
                                                        }
                                                    }
                                                }
                                                slot.set_player_template(template);
                                                if template == PLAYERTEMPLATE_OBSERVER {
                                                    slot.set_color(-1);
                                                    slot.set_start_pos(-1);
                                                    slot.set_team_number(-1);
                                                }
                                                should_unaccept = true;
                                            }
                                        }
                                        "StartPos" => {
                                            if val >= -1
                                                && !game.is_start_position_taken(val, slot_num)
                                                && slot.get_player_template()
                                                    != PLAYERTEMPLATE_OBSERVER
                                            {
                                                slot.set_start_pos(val);
                                                should_unaccept = true;
                                            }
                                        }
                                        "Team" => {
                                            if val >= -1
                                                && val < (MAX_SLOTS / 2) as i32
                                                && slot.get_player_template()
                                                    != PLAYERTEMPLATE_OBSERVER
                                            {
                                                slot.set_team_number(val);
                                                should_unaccept = true;
                                            }
                                        }
                                        "IP" => {
                                            if let Ok(ip) = val_str.parse::<u32>() {
                                                slot.set_ip(ip);
                                                should_unaccept = true;
                                            }
                                        }
                                        "NAT" => {
                                            slot.set_nat_behavior(firewall_behavior_from_int(val));
                                        }
                                        "Ping" => {
                                            // Ping strings are not yet stored per-slot in Rust.
                                        }
                                        _ => {}
                                    }
                                }
                                if should_unaccept {
                                    game.reset_accepted();
                                }
                            });

                            push_gamespy_game_options();
                            update_slot_list(&mut state);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

pub fn wol_game_setup_menu_shutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    let mut state = game_setup_state().lock().unwrap_or_else(|e| e.into_inner());

    if let Some(info) = get_gamespy_info() {
        if let Ok(mut info) = info.lock() {
            info.unregister_text_window(state.listbox_chat_id as u32);
        }
    }

    if let Some(layout) = state.wol_map_select_layout.take() {
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }

    state.parent = None;
    state.button_emote = None;
    state.button_select_map = None;
    state.button_start = None;
    state.button_back = None;
    state.listbox_chat = None;
    state.text_entry_chat = None;
    state.text_entry_map_display = None;
    state.map_window = None;
    state.checkbox_use_stats = None;
    state.checkbox_limit_superweapons = None;
    state.combo_box_starting_cash = None;
    state.checkbox_limit_armies = None;
    state.init_done = false;
    state.slotlist_updates_enabled = false;

    state.is_shutting_down = true;

    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>().copied())
        .unwrap_or(false);
    if pop_immediate {
        layout.hide(true);
        get_shell().shutdown_complete(layout, state.next_screen.is_some());
        if let Some(next) = state.next_screen.take() {
            let _ = get_shell().push(&next, false);
        }
        state.is_shutting_down = false;
        return;
    }

    get_shell().reverse_animate_window();
    raise_gs_message_box();
    with_window_manager(|manager| manager.transition_reverse("GameSpyGameOptionsMenuFade"));
}

pub fn wol_game_setup_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    let mut state = game_setup_state().lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::Create => {
            state.button_communicator_id =
                name_to_id("GameSpyGameOptionsMenu.wnd:ButtonCommunicator");
        }
        WindowMessage::InputFocus => {
            return WindowMsgHandled::Handled;
        }
        WindowMessage::GadgetValueChanged => {
            if !state.init_done || state.button_pushed {
                return WindowMsgHandled::Handled;
            }
            let control_id = data1 as i32;

            if control_id == state.combo_box_starting_cash_id {
                handle_starting_cash_selection(&mut state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.checkbox_limit_superweapons_id {
                handle_limit_superweapons_click(&mut state);
                return WindowMsgHandled::Handled;
            }

            for i in 0..MAX_SLOTS {
                if control_id == state.combo_box_color_ids[i] {
                    handle_color_selection(&mut state, i);
                    return WindowMsgHandled::Handled;
                }
                if control_id == state.combo_box_template_ids[i] {
                    handle_template_selection(&mut state, i);
                    return WindowMsgHandled::Handled;
                }
                if control_id == state.combo_box_team_ids[i] {
                    handle_team_selection(&mut state, i);
                    return WindowMsgHandled::Handled;
                }
                if control_id == state.combo_box_player_ids[i] {
                    if !is_host() {
                        continue;
                    }
                    let pos = combo_box_selected_index(&with_window_manager(|manager| {
                        manager.get_window_by_id(state.combo_box_player_ids[i])
                    }))
                    .unwrap_or(0) as i32;
                    if pos >= 0 && pos != SlotState::Player as i32 {
                        with_gamespy_game_info_mut(|game| {
                            if let Some(slot) = game.get_slot_mut(i) {
                                slot.set_state(
                                    slot_state_from_int(pos),
                                    slot.get_name().to_string(),
                                    slot.get_ip(),
                                );
                                game.reset_accepted();
                            }
                        });
                        push_gamespy_game_options();
                        update_slot_list(&mut state);
                    }
                    return WindowMsgHandled::Handled;
                }
            }
        }
        WindowMessage::GadgetSelected => {
            if state.button_pushed {
                return WindowMsgHandled::Handled;
            }
            let control_id = data1 as i32;
            if control_id == state.button_back_id {
                state.button_pushed = true;
                if let Some(info) = get_gamespy_info() {
                    if let Ok(mut info) = info.lock() {
                        info.leave_staging_room();
                    }
                }
                state.next_screen = Some("Menus/WOLCustomLobby.wnd".to_string());
                let _ = get_shell().pop();
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_communicator_id {
                toggle_overlay(GameSpyOverlayType::Buddy);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_emote_id {
                if let Some(entry) = state.text_entry_chat.as_ref() {
                    if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                        let text = widget.text().trim().to_string();
                        widget.set_text("");
                        if !text.is_empty() {
                            if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok())
                            {
                                info.send_chat(text, false, None);
                            }
                        }
                    }
                }
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_select_map_id {
                let layout = with_window_manager(|manager| {
                    manager
                        .create_layout_with_windows("Menus/WOLMapSelectMenu.wnd")
                        .ok()
                });
                if let Some((layout, _)) = layout {
                    layout.borrow().run_init(None);
                    layout.borrow_mut().hide(false);
                    layout.borrow_mut().bring_forward();
                    state.wol_map_select_layout = Some(layout);
                }
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_start_id {
                if is_host() {
                    start_pressed(&mut state);
                } else {
                    with_gamespy_game_info_mut(|game| {
                        if let Some(slot) =
                            game.get_slot_mut(game.get_local_slot_num().max(0) as usize)
                        {
                            slot.set_accept();
                        }
                    });
                    let host_name = with_gamespy_game_info(|game| {
                        game.get_slot(0)
                            .map(|slot| slot.get_name().to_string())
                            .unwrap_or_default()
                    });
                    if let Some(queue) = get_peer_message_queue() {
                        if let Ok(mut queue) = queue.lock() {
                            let mut req = PeerRequest::default();
                            req.request_type = PeerRequestType::UtmPlayer;
                            req.id = "accept".to_string();
                            req.nick = host_name;
                            req.options = "true".to_string();
                            queue.add_request(req);
                        }
                    }
                    update_slot_list(&mut state);
                }
                return WindowMsgHandled::Handled;
            }
            for i in 0..MAX_SLOTS {
                if control_id == state.button_map_start_position_ids[i] {
                    let game = with_gamespy_game_info(|game| game.clone());
                    let mut player_idx = None;
                    for j in 0..MAX_SLOTS {
                        if let Some(slot) = game.get_slot(j) {
                            if slot.get_start_pos() == i as i32 {
                                player_idx = Some(j);
                                break;
                            }
                        }
                    }
                    if let Some(player_idx) = player_idx {
                        let slot = game.get_slot(player_idx);
                        let can_move = player_idx as i32 == game.get_local_slot_num()
                            || (is_host() && slot.map(|s| s.is_ai()).unwrap_or(false));
                        if can_move {
                            if let Some(next) = next_selectable_player(&game, player_idx + 1) {
                                handle_start_position_selection(&mut state, player_idx, -1);
                                handle_start_position_selection(&mut state, next, i as i32);
                            }
                        }
                    } else {
                        let next_player = next_selectable_player(&game, 0)
                            .unwrap_or_else(|| first_selectable_player(&game));
                        handle_start_position_selection(&mut state, next_player, i as i32);
                    }
                    return WindowMsgHandled::Handled;
                }
            }
        }
        WindowMessage::GadgetRightClick => {
            if state.button_pushed {
                return WindowMsgHandled::Handled;
            }
            let control_id = data1 as i32;
            for i in 0..MAX_SLOTS {
                if control_id == state.button_map_start_position_ids[i] {
                    let game = with_gamespy_game_info(|game| game.clone());
                    let mut player_idx = None;
                    for j in 0..MAX_SLOTS {
                        if let Some(slot) = game.get_slot(j) {
                            if slot.get_start_pos() == i as i32 {
                                player_idx = Some(j);
                                break;
                            }
                        }
                    }
                    if let Some(player_idx) = player_idx {
                        let slot = game.get_slot(player_idx);
                        let can_move = player_idx as i32 == game.get_local_slot_num()
                            || (is_host() && slot.map(|s| s.is_ai()).unwrap_or(false));
                        if can_move {
                            handle_start_position_selection(&mut state, player_idx, -1);
                        }
                    }
                    return WindowMsgHandled::Handled;
                }
            }
        }
        WindowMessage::GadgetEditDone => {
            let control_id = data1 as i32;
            if control_id == state.text_entry_chat_id {
                if let Some(entry) = state.text_entry_chat.as_ref() {
                    if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                        let text = widget.text().trim().to_string();
                        widget.set_text("");
                        if !text.is_empty() {
                            if !handle_slash_command(&text) {
                                if let Some(info) =
                                    get_gamespy_info().and_then(|info| info.lock().ok())
                                {
                                    info.send_chat(text, false, None);
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }

    WindowMsgHandled::Handled
}

pub fn wol_game_setup_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char {
        let key = data1 as u32;
        let state = data2 as u32;
        if key == KEY_ESC && (state & KEY_STATE_UP) != 0 {
            let _ = get_shell().pop();
            return WindowMsgHandled::Handled;
        }
    }
    WindowMsgHandled::Ignored
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
