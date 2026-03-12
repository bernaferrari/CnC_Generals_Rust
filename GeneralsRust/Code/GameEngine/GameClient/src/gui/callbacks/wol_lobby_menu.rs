//! WOLLobbyMenu.cpp callback port.

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::gamespy_game::with_gamespy_game_info;
use crate::gamespy_overlay::{
    close_all_overlays, close_overlay, gs_message_box_ok, open_overlay, raise_gs_message_box,
    toggle_overlay, GameSpyOverlayType,
};
use crate::gui::callbacks::wol_buddy_overlay::handle_buddy_responses;
use crate::gui::gadgets::{ComboBoxItem, ListBoxItemData, ListBoxRightClick};
use crate::gui::menu_flags::set_dont_show_main_menu;
use crate::gui::{
    get_shell, with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, WindowWidget,
};
use crate::map_util::get_map_cache_manager;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::ini_game_data::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::preferences::CustomMatchPreferences;
use game_network::gamespy::config::GameSpyConfig;
use game_network::gamespy::ladder_defs::get_ladder_list;
use game_network::gamespy::peer_defs::{
    default_gamespy_colors, get_gamespy_info, GameSpyColor, GameSpyStagingRoom, PlayerInfo,
};
use game_network::gamespy::peer_thread::{
    get_peer_message_queue, PeerRequest, PeerRequestType, PeerResponse, PeerResponseType,
};
use game_network::gamespy::persistent_storage_thread::{get_ps_message_queue, PSResponseType};
use game_network::rank_point_value::{calculate_rank, get_favorite_side, get_rank_point_values};
use game_network::{SlotState, MAX_SLOTS};

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;
const COLUMN_PLAYERNAME: usize = 1;
const COLUMN_PING: i32 = 7;
const COLUMN_NUMPLAYERS: i32 = 3;
const COLUMN_PASSWORD: i32 = 4;
const COLUMN_USE_STATS: i32 = 6;
const GAME_LIST_REFRESH_INTERVAL_MS: u128 = 10_000;
const PLAYER_LIST_REFRESH_INTERVAL_MS: u128 = 5_000;
const PEER_FLAG_OP: i32 = 1;
const ROOM_TYPE_GROUP: i32 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum GameSortType {
    AlphaAscending,
    AlphaDescending,
    PingAscending,
    PingDescending,
}

impl Default for GameSortType {
    fn default() -> Self {
        GameSortType::AlphaAscending
    }
}

#[derive(Default)]
struct WolLobbyState {
    parent_id: i32,
    button_back_id: i32,
    button_host_id: i32,
    button_refresh_id: i32,
    button_join_id: i32,
    button_buddy_id: i32,
    button_emote_id: i32,
    text_entry_chat_id: i32,
    listbox_lobby_players_id: i32,
    listbox_lobby_chat_id: i32,
    combo_lobby_group_rooms_id: i32,
    button_sort_alpha_id: i32,
    button_sort_ping_id: i32,
    button_sort_buddies_id: i32,
    window_sort_alpha_id: i32,
    window_sort_ping_id: i32,
    window_sort_buddies_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_back: Option<Rc<RefCell<GameWindow>>>,
    button_host: Option<Rc<RefCell<GameWindow>>>,
    button_refresh: Option<Rc<RefCell<GameWindow>>>,
    button_join: Option<Rc<RefCell<GameWindow>>>,
    button_buddy: Option<Rc<RefCell<GameWindow>>>,
    button_emote: Option<Rc<RefCell<GameWindow>>>,
    text_entry_chat: Option<Rc<RefCell<GameWindow>>>,
    listbox_lobby_players: Option<Rc<RefCell<GameWindow>>>,
    listbox_lobby_chat: Option<Rc<RefCell<GameWindow>>>,
    combo_lobby_group_rooms: Option<Rc<RefCell<GameWindow>>>,
    is_shutting_down: bool,
    button_pushed: bool,
    raise_message_boxes: bool,
    next_screen: Option<String>,
    game_list_refresh_time: u128,
    player_list_refresh_time: u128,
    group_room_to_join: i32,
    initial_gadget_delay: i32,
    just_entered: bool,
    trying_to_host_or_join: bool,
    is_small_game_list: bool,
    sort_type: GameSortType,
    sort_buddies: bool,
    queued_utms: VecDeque<PeerResponse>,
}

static WOL_LOBBY_STATE: OnceLock<Mutex<WolLobbyState>> = OnceLock::new();

fn wol_state() -> &'static Mutex<WolLobbyState> {
    WOL_LOBBY_STATE.get_or_init(|| Mutex::new(WolLobbyState::default()))
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

fn listbox_right_click_info(window: &GameWindow) -> Option<(ListBoxRightClick, i32, i32)> {
    let (win_x, win_y) = window.get_screen_position();
    let WindowWidget::ListBox(listbox) = window.widget()? else {
        return None;
    };
    let info = listbox.last_right_click()?;
    Some((info, win_x + info.mouse_x, win_y + info.mouse_y))
}

fn lookup_small_rank_image(side: i32, rank_points: i32) -> Option<(String, u32, u32)> {
    if rank_points <= 0 {
        return None;
    }

    let rank_values = get_rank_point_values();
    let rank_values = rank_values.read().ok()?;
    let mut rank = 0;
    while rank + 1 < rank_values.ranks.len() && rank_points >= rank_values.ranks[rank + 1] {
        rank += 1;
    }
    let rank_names = [
        "Private",
        "Corporal",
        "Sergeant",
        "Lieutenant",
        "Captain",
        "Major",
        "Colonel",
        "General",
        "Brigadier",
        "Commander",
    ];
    if rank >= rank_names.len() {
        return None;
    }

    let side_str = match side {
        2 | 5 | 6 | 7 => "USA",
        3 | 8 | 9 | 10 => "CHA",
        4 | 11 | 12 | 13 => "GLA",
        _ => "N",
    };
    let image_name = format!("{}-{}", rank_names[rank], side_str);
    let collection = get_mapped_image_collection();
    let image = collection.find_image_by_name(&image_name)?;
    let width = image.get_image_width().max(1) as u32;
    let height = image.get_image_height().max(1) as u32;
    Some((image_name, width, height))
}

fn handle_lobby_slash_commands(message: &str, listbox_id: u32) -> bool {
    if !message.starts_with('/') {
        return false;
    }

    let trimmed = message.trim_start_matches('/');
    let mut parts = trimmed.splitn(2, ' ');
    let token = parts.next().unwrap_or("").to_lowercase();
    let remainder = parts.next().unwrap_or("").trim();

    match token.as_str() {
        "host" => {
            let hosting = get_gamespy_info()
                .and_then(|info| {
                    info.lock()
                        .ok()
                        .map(|guard| if guard.am_i_host() { 1 } else { 0 })
                })
                .unwrap_or(0);
            let msg = format!("Hosting qr2:{} thread:{}", hosting, hosting);
            if let Some(mut info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                info.add_text(
                    msg,
                    default_gamespy_colors()[GameSpyColor::Default as usize],
                    None,
                );
            }
            true
        }
        "me" => {
            if !remainder.is_empty() {
                if let Some(mut info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                    let _ = info.send_chat(remainder.to_string(), true, Some(listbox_id));
                }
                return true;
            }
            false
        }
        "refresh" => {
            refresh_game_list(true);
            refresh_player_list(true);
            true
        }
        _ => false,
    }
}

fn slot_state_from_profile(profile_id: i32, name: &AsciiString) -> Option<SlotState> {
    match profile_id {
        2 => Some(SlotState::EasyAI),
        3 => Some(SlotState::MedAI),
        4 => Some(SlotState::BrutalAI),
        _ => {
            let upper = name.as_str().to_uppercase();
            match upper.as_str() {
                "CE" => Some(SlotState::EasyAI),
                "CM" => Some(SlotState::MedAI),
                "CH" => Some(SlotState::BrutalAI),
                _ => None,
            }
        }
    }
}

fn show_sort_icons(state: &WolLobbyState) {
    if let Some(window) =
        with_window_manager(|manager| manager.get_window_by_id(state.window_sort_alpha_id))
    {
        match state.sort_type {
            GameSortType::AlphaAscending => {
                let _ = window.borrow_mut().hide(false);
                let _ = window.borrow_mut().set_enabled(true);
            }
            GameSortType::AlphaDescending => {
                let _ = window.borrow_mut().hide(false);
                let _ = window.borrow_mut().set_enabled(false);
            }
            _ => {
                let _ = window.borrow_mut().hide(true);
            }
        }
    }
    if let Some(window) =
        with_window_manager(|manager| manager.get_window_by_id(state.window_sort_ping_id))
    {
        match state.sort_type {
            GameSortType::PingAscending => {
                let _ = window.borrow_mut().hide(false);
                let _ = window.borrow_mut().set_enabled(true);
            }
            GameSortType::PingDescending => {
                let _ = window.borrow_mut().hide(false);
                let _ = window.borrow_mut().set_enabled(false);
            }
            _ => {
                let _ = window.borrow_mut().hide(true);
            }
        }
    }
    if let Some(window) =
        with_window_manager(|manager| manager.get_window_by_id(state.window_sort_buddies_id))
    {
        let _ = window.borrow_mut().hide(!state.sort_buddies);
    }
}

fn set_sort_mode(state: &mut WolLobbyState, sort_type: GameSortType) {
    state.sort_type = sort_type;
    show_sort_icons(state);
    refresh_game_list_boxes();
}

fn toggle_sort_buddies(state: &mut WolLobbyState) {
    state.sort_buddies = !state.sort_buddies;
    show_sort_icons(state);
    refresh_game_list_boxes();
}

fn insert_player_in_listbox(
    listbox: &mut crate::gui::gadgets::ListBox,
    info: &PlayerInfo,
    color: crate::gui::Color,
) -> usize {
    let name = info.name.as_str().to_string();
    let index = listbox.add_item_with_color(&name, color);

    if let Some((image_name, width, height)) = lookup_small_rank_image(info.side, info.rank_points)
    {
        let _ = listbox.set_item_column_data(
            index,
            0,
            ListBoxItemData::Image {
                name: image_name,
                width,
                height,
                text: None,
            },
        );
    }
    let _ = listbox.set_item_column_data(index, 1, ListBoxItemData::Text(name));
    index
}

pub fn populate_lobby_player_listbox() {
    let (listbox_id, listbox_window) = {
        let state = wol_state()
            .lock()
            .expect("WOLLobbyMenu state lock poisoned");
        (
            state.listbox_lobby_players_id,
            state.listbox_lobby_players.clone(),
        )
    };

    let Some(listbox_window) = listbox_window else {
        return;
    };
    let mut listbox_guard = listbox_window.borrow_mut();
    let Some(listbox) = listbox_guard.list_box_mut() else {
        return;
    };

    let mut selected_names = HashSet::new();
    let mut selected_indices = Vec::new();
    for idx in listbox.selected_indices() {
        if let Some(item) = listbox.items().get(idx) {
            selected_names.insert(item.text.clone());
        }
        selected_indices.push(idx);
    }
    let previous_top = listbox.get_top_visible_entry();
    listbox.clear();

    let gs_info = get_gamespy_info().and_then(|info| info.lock().ok());
    let Some(gs_info) = gs_info else {
        return;
    };

    let config = GameSpyConfig::new_sync();
    let buddies = gs_info.get_buddy_map();
    let mut indices_to_select = Vec::new();

    let mut players: Vec<PlayerInfo> = gs_info.get_player_info_map().values().cloned().collect();
    players.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));

    for info in players.iter().filter(|p| {
        (p.flags & PEER_FLAG_OP) != 0 || config.is_player_vip(&p.profile_id.to_string())
    }) {
        let ignored = info.profile_id > 0
            && (gs_info.is_saved_ignored(info.profile_id) || gs_info.is_ignored(info.name.clone()));
        let color = if ignored {
            default_gamespy_colors()[GameSpyColor::PlayerIgnored as usize]
        } else {
            default_gamespy_colors()[GameSpyColor::PlayerOwner as usize]
        };
        let index = insert_player_in_listbox(listbox, info, color);
        if selected_names.contains(info.name.as_str()) {
            indices_to_select.push(index);
        }
    }

    for info in players.iter().filter(|p| {
        (p.flags & PEER_FLAG_OP) == 0
            && !config.is_player_vip(&p.profile_id.to_string())
            && buddies.contains_key(&p.profile_id)
    }) {
        let ignored = info.profile_id > 0
            && (gs_info.is_saved_ignored(info.profile_id) || gs_info.is_ignored(info.name.clone()));
        let color = if ignored {
            default_gamespy_colors()[GameSpyColor::PlayerIgnored as usize]
        } else {
            default_gamespy_colors()[GameSpyColor::PlayerBuddy as usize]
        };
        let index = insert_player_in_listbox(listbox, info, color);
        if selected_names.contains(info.name.as_str()) {
            indices_to_select.push(index);
        }
    }

    for info in players.iter().filter(|p| {
        (p.flags & PEER_FLAG_OP) == 0
            && !config.is_player_vip(&p.profile_id.to_string())
            && !buddies.contains_key(&p.profile_id)
    }) {
        let ignored = info.profile_id > 0
            && (gs_info.is_saved_ignored(info.profile_id) || gs_info.is_ignored(info.name.clone()));
        let color = if ignored {
            default_gamespy_colors()[GameSpyColor::PlayerIgnored as usize]
        } else {
            default_gamespy_colors()[GameSpyColor::PlayerNormal as usize]
        };
        let index = insert_player_in_listbox(listbox, info, color);
        if selected_names.contains(info.name.as_str()) {
            indices_to_select.push(index);
        }
    }

    if !indices_to_select.is_empty() {
        listbox.set_selected_indices(&indices_to_select);
    }

    if indices_to_select.len() != selected_indices.len() {
        with_window_manager(|manager| manager.set_lone_window(None));
    }

    listbox.set_top_visible_entry(previous_top);

    let _ = listbox_id;
}

fn populate_group_room_listbox(combo_window: &Rc<RefCell<GameWindow>>) {
    let mut combo_window = combo_window.borrow_mut();
    let Some(combo) = combo_window.combo_box_mut() else {
        return;
    };
    combo.clear();

    let config = GameSpyConfig::new_sync();
    let (_, qm_channel) = config.get_qm_config();
    let current_room = get_gamespy_info()
        .and_then(|info| info.lock().ok().map(|guard| guard.get_current_group_room()))
        .unwrap_or(0);

    let mut index_to_select = None;
    let mut index = 0usize;
    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        for room in info.get_group_room_list().values() {
            if room.group_id == qm_channel {
                continue;
            }
            let text = if !room.translated_name.is_empty() {
                room.translated_name.clone()
            } else {
                room.name.as_str().to_string()
            };
            let item = ComboBoxItem::new(room.group_id as u32, text).with_data(room.group_id);
            combo.add_item(item);
            if room.group_id == current_room {
                index_to_select = Some(index);
            }
            index += 1;
        }
    }

    if let Some(index) = index_to_select {
        let _ = combo.select_index(index);
    }
}

fn refresh_game_list(force_refresh: bool) {
    let mut state = wol_state()
        .lock()
        .expect("WOLLobbyMenu state lock poisoned");
    let now = now_ms();
    if force_refresh
        || state.game_list_refresh_time == 0
        || now.saturating_sub(state.game_list_refresh_time) >= GAME_LIST_REFRESH_INTERVAL_MS
    {
        let changed = get_gamespy_info()
            .and_then(|info| {
                info.lock()
                    .ok()
                    .map(|mut guard| guard.has_staging_room_list_changed())
            })
            .unwrap_or(false);
        if changed {
            drop(state);
            refresh_game_list_boxes();
            let mut state = wol_state()
                .lock()
                .expect("WOLLobbyMenu state lock poisoned");
            state.game_list_refresh_time = now;
        }
    }
}

fn refresh_player_list(force_refresh: bool) {
    let mut state = wol_state()
        .lock()
        .expect("WOLLobbyMenu state lock poisoned");
    let now = now_ms();
    if force_refresh
        || state.player_list_refresh_time == 0
        || now.saturating_sub(state.player_list_refresh_time) >= PLAYER_LIST_REFRESH_INTERVAL_MS
    {
        drop(state);
        populate_lobby_player_listbox();
        let mut state = wol_state()
            .lock()
            .expect("WOLLobbyMenu state lock poisoned");
        state.player_list_refresh_time = now;
    }
}

fn get_game_list_box() -> Option<Rc<RefCell<GameWindow>>> {
    let listbox_large_id = name_to_id("WOLCustomLobby.wnd:ListboxGamesLarge");
    let listbox_small_id = name_to_id("WOLCustomLobby.wnd:ListboxGames");
    with_window_manager(|manager| {
        if let Some(large) = manager.get_window_by_id(listbox_large_id) {
            if !large.borrow().is_hidden() {
                return Some(large);
            }
        }
        manager
            .get_window_by_id(listbox_small_id)
            .or_else(|| manager.get_window_by_id(listbox_large_id))
    })
}

fn get_game_list_box_id() -> i32 {
    let listbox_large_id = name_to_id("WOLCustomLobby.wnd:ListboxGamesLarge");
    let listbox_small_id = name_to_id("WOLCustomLobby.wnd:ListboxGames");
    if let Some(large) = with_window_manager(|manager| manager.get_window_by_id(listbox_large_id)) {
        if !large.borrow().is_hidden() {
            return listbox_large_id;
        }
    }
    if with_window_manager(|manager| manager.get_window_by_id(listbox_small_id)).is_some() {
        listbox_small_id
    } else {
        listbox_large_id
    }
}

fn get_game_info_list_box() -> Option<Rc<RefCell<GameWindow>>> {
    let id = name_to_id("WOLCustomLobby.wnd:ListboxGameInfo");
    with_window_manager(|manager| manager.get_window_by_id(id))
}

fn toggle_game_list_type(state: &mut WolLobbyState) {
    state.is_small_game_list = !state.is_small_game_list;
    let parent_large_id = name_to_id("WOLCustomLobby.wnd:ParentGameListLarge");
    if let Some(parent_large) =
        with_window_manager(|manager| manager.get_window_by_id(parent_large_id))
    {
        let _ = parent_large.borrow_mut().hide(state.is_small_game_list);
    }
    refresh_game_list_boxes();
}

pub fn refresh_game_list_boxes() {
    let Some(listbox_window) = get_game_list_box() else {
        return;
    };
    let mut listbox_guard = listbox_window.borrow_mut();
    let Some(listbox) = listbox_guard.list_box_mut() else {
        return;
    };

    let previous_top = listbox.get_top_visible_entry();
    let selected = listbox.selected_indices().first().copied();
    let selected_id = selected
        .and_then(|idx| listbox.get_item_data(idx))
        .and_then(|data| {
            if let ListBoxItemData::Integer(id) = data {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap_or(0);
    listbox.clear();
    let show_map = get_game_info_list_box().is_none();

    let info = get_gamespy_info().and_then(|info| info.lock().ok());
    let Some(info) = info else {
        return;
    };

    let mut rooms: Vec<GameSpyStagingRoom> =
        info.get_staging_room_list().values().cloned().collect();
    let (exe_crc, ini_crc) = get_global_data()
        .and_then(|data| data.read().ok().map(|guard| (guard.exe_crc, guard.ini_crc)))
        .unwrap_or((0, 0));
    let buddies_in_staging: std::collections::HashSet<String> = info
        .get_buddy_map()
        .values()
        .filter(|buddy| {
            buddy.status == game_network::gamespy::peer_defs::GameSpyBuddyStatus::Staging
        })
        .map(|buddy| buddy.location_string.to_lowercase())
        .collect();
    let sort_type = wol_state()
        .lock()
        .ok()
        .map(|state| state.sort_type)
        .unwrap_or(GameSortType::AlphaAscending);
    let sort_buddies = wol_state()
        .lock()
        .ok()
        .map(|state| state.sort_buddies)
        .unwrap_or(true);
    let ladder_list = get_ladder_list().and_then(|list| list.read().ok());
    rooms.sort_by(|a, b| {
        let a_crc_bad = exe_crc != 0 && (a.exe_crc != exe_crc || a.ini_crc != ini_crc);
        let b_crc_bad = exe_crc != 0 && (b.exe_crc != exe_crc || b.ini_crc != ini_crc);
        if a_crc_bad != b_crc_bad {
            return if a_crc_bad {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            };
        }
        let a_unknown_ladder = a.ladder_port != 0
            && ladder_list
                .as_ref()
                .and_then(|list| list.find_ladder(&a.ladder_ip, a.ladder_port))
                .is_none();
        let b_unknown_ladder = b.ladder_port != 0
            && ladder_list
                .as_ref()
                .and_then(|list| list.find_ladder(&b.ladder_ip, b.ladder_port))
                .is_none();
        if a_unknown_ladder != b_unknown_ladder {
            return if a_unknown_ladder {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            };
        }
        let a_non_obs = a.num_players.saturating_sub(a.num_observers);
        let b_non_obs = b.num_players.saturating_sub(b.num_observers);
        let a_full = a_non_obs == a.max_players || a.num_players == MAX_SLOTS as i32;
        let b_full = b_non_obs == b.max_players || b.num_players == MAX_SLOTS as i32;
        if a_full != b_full {
            return if a_full {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            };
        }
        if sort_buddies {
            let a_host = a.player_names[0].as_str().to_lowercase();
            let b_host = b.player_names[0].as_str().to_lowercase();
            let a_has_buddy = !a_host.is_empty() && buddies_in_staging.contains(&a_host);
            let b_has_buddy = !b_host.is_empty() && buddies_in_staging.contains(&b_host);
            if a_has_buddy != b_has_buddy {
                return if a_has_buddy {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }
        }
        match sort_type {
            GameSortType::AlphaAscending => a.name.cmp(&b.name),
            GameSortType::AlphaDescending => b.name.cmp(&a.name),
            GameSortType::PingAscending => info
                .get_ping_value(&a.host_ping)
                .cmp(&info.get_ping_value(&b.host_ping)),
            GameSortType::PingDescending => info
                .get_ping_value(&b.host_ping)
                .cmp(&info.get_ping_value(&a.host_ping)),
        }
    });
    let config = GameSpyConfig::new_sync();
    let (_, _, cutoff_good, cutoff_bad) = config.get_ping_config();

    for room in rooms.iter() {
        let name = room.name.clone();
        let mut map_name = room.map_name.as_str().to_string();
        if let Ok(mut cache) = get_map_cache_manager().lock() {
            cache.update_cache();
            if let Some(meta) = cache.find_map(room.map_name.as_str()) {
                map_name = meta.display_name;
            }
        }
        if map_name.is_empty() {
            let raw = room.map_name.as_str();
            let trimmed = raw.rsplit(['\\', '/']).next().unwrap_or(raw);
            map_name = trimmed.to_string();
        }
        let players = format!("{}/{}", room.num_players.max(0), room.max_players.max(0));
        let ping = info.get_ping_value(&room.host_ping);

        let mut color = default_gamespy_colors()[GameSpyColor::Game as usize];
        let non_obs = room.num_players.saturating_sub(room.num_observers);
        if exe_crc != 0 && (room.exe_crc != exe_crc || room.ini_crc != ini_crc) {
            color = default_gamespy_colors()[GameSpyColor::GameCrcMismatch as usize];
        } else if non_obs == room.max_players || room.num_players == MAX_SLOTS as i32 {
            color = default_gamespy_colors()[GameSpyColor::GameFull as usize];
        }
        let index = listbox.add_item_with_color(&name, color);
        let _ = listbox.set_item_data(index, Some(ListBoxItemData::Integer(room.id)));

        if show_map {
            let _ = listbox.set_item_column_data(index, 1, ListBoxItemData::Text(map_name.clone()));
            if room.ladder_port != 0 {
                if let Some(list) = ladder_list.as_ref() {
                    if let Some(ladder) = list.find_ladder(&room.ladder_ip, room.ladder_port) {
                        let _ = listbox.set_item_column_data(
                            index,
                            2,
                            ListBoxItemData::Text(ladder.name.clone()),
                        );
                    } else {
                        let _ = listbox.set_item_column_data(
                            index,
                            2,
                            ListBoxItemData::Text(GameText::fetch("GUI:UnknownLadder")),
                        );
                    }
                } else {
                    let _ = listbox.set_item_column_data(
                        index,
                        2,
                        ListBoxItemData::Text(GameText::fetch("GUI:UnknownLadder")),
                    );
                }
            } else {
                let _ = listbox.set_item_column_data(
                    index,
                    2,
                    ListBoxItemData::Text(GameText::fetch("GUI:NoLadder")),
                );
            }
        } else {
            let _ = listbox.set_item_column_data(index, 1, ListBoxItemData::Text(" ".to_string()));
            let _ = listbox.set_item_column_data(index, 2, ListBoxItemData::Text(" ".to_string()));
        }

        let _ = listbox.set_item_column_data(index, 3, ListBoxItemData::Text(players));

        if room.has_password {
            let (width, height) = get_mapped_image_collection()
                .find_image_by_name("Password")
                .map(|img| (img.get_image_width() as u32, img.get_image_height() as u32))
                .unwrap_or((10, 10));
            let _ = listbox.set_item_column_data(
                index,
                4,
                ListBoxItemData::Image {
                    name: "Password".to_string(),
                    width,
                    height,
                    text: None,
                },
            );
        } else {
            let _ = listbox.set_item_column_data(index, 4, ListBoxItemData::Text(" ".to_string()));
        }

        if room.allow_observers {
            let _ = listbox.set_item_column_data(
                index,
                5,
                ListBoxItemData::Image {
                    name: "Observer".to_string(),
                    width: 10,
                    height: 10,
                    text: None,
                },
            );
        } else {
            let _ = listbox.set_item_column_data(index, 5, ListBoxItemData::Text(" ".to_string()));
        }

        if room.use_stats {
            let (width, height) = get_mapped_image_collection()
                .find_image_by_name("GoodStatsIcon")
                .map(|img| (img.get_image_width() as u32, img.get_image_height() as u32))
                .unwrap_or((10, 10));
            let _ = listbox.set_item_column_data(
                index,
                6,
                ListBoxItemData::Image {
                    name: "GoodStatsIcon".to_string(),
                    width,
                    height,
                    text: None,
                },
            );
        }

        let (width, height) =
            if let Some(img) = get_mapped_image_collection().find_image_by_name("Ping03") {
                (img.get_image_width() as u32, img.get_image_height() as u32)
            } else {
                (10, 10)
            };
        let ping_icon = if ping <= cutoff_good {
            "Ping03"
        } else if ping <= cutoff_bad {
            "Ping02"
        } else {
            "Ping01"
        };
        let _ = listbox.set_item_column_data(
            index,
            7,
            ListBoxItemData::Image {
                name: ping_icon.to_string(),
                width,
                height,
                text: Some(ping.to_string()),
            },
        );

        if selected_id != 0 && room.id == selected_id {
            listbox.set_selected_indices(&[index]);
        }
    }

    listbox.set_top_visible_entry(previous_top);

    if let Some(info_box) = get_game_info_list_box() {
        let listbox_ref = listbox_window.borrow();
        refresh_game_info_list_box(&*listbox_ref, &info_box);
    }
}

fn refresh_game_info_list_box(game_list: &GameWindow, info_window: &Rc<RefCell<GameWindow>>) {
    let Some(WindowWidget::ListBox(game_list)) = game_list.widget() else {
        return;
    };
    let mut info_window = info_window.borrow_mut();
    let Some(info_list) = info_window.list_box_mut() else {
        return;
    };

    let selected = game_list.selected_indices().first().copied();
    let Some(selected) = selected else {
        return;
    };
    let Some(id_data) = game_list.get_item_data(selected) else {
        return;
    };
    let game_id = match id_data {
        ListBoxItemData::Integer(id) => *id,
        _ => return,
    };

    let info = get_gamespy_info().and_then(|info| info.lock().ok());
    let Some(info) = info else {
        return;
    };
    let Some(room) = info.find_staging_room_by_id(game_id) else {
        return;
    };

    info_list.clear();
    info_list.add_item_with_color(
        &format!("{}", room.name),
        default_gamespy_colors()[GameSpyColor::Default as usize],
    );
    info_list.add_item_with_color(
        &format!("Map: {}", room.map_name.as_str()),
        default_gamespy_colors()[GameSpyColor::Default as usize],
    );
    info_list.add_item_with_color(
        &format!("Players: {}/{}", room.num_players, room.max_players),
        default_gamespy_colors()[GameSpyColor::Default as usize],
    );
}

fn set_unignore_text(layout: &WindowLayout, nick: &AsciiString, profile_id: i32) {
    let control_name = format!(
        "{}:ButtonIgnore",
        layout.get_filename().trim_start_matches("Menus/")
    );
    let id = name_to_id(&control_name);
    let window = with_window_manager(|manager| manager.get_window_by_id(id));
    let Some(window) = window else {
        return;
    };

    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        if info.is_saved_ignored(profile_id) || info.is_ignored(nick.clone()) {
            let _ = window
                .borrow_mut()
                .set_text(&GameText::fetch("GUI:Unignore"));
        }
    }
}

fn close_right_click_menu(window: &GameWindow) {
    if let Some(layout) = window.get_layout() {
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
}

pub fn wol_lobby_menu_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let mut state = wol_state()
        .lock()
        .expect("WOLLobbyMenu state lock poisoned");
    state.next_screen = None;
    state.button_pushed = false;
    state.is_shutting_down = false;
    state.raise_message_boxes = false;
    state.trying_to_host_or_join = false;
    state.game_list_refresh_time = 0;
    state.player_list_refresh_time = 0;
    state.is_small_game_list = true;
    state.sort_type = GameSortType::AlphaAscending;
    state.sort_buddies = true;

    state.parent_id = name_to_id("WOLCustomLobby.wnd:WOLLobbyMenuParent");
    state.button_back_id = name_to_id("WOLCustomLobby.wnd:ButtonBack");
    state.button_host_id = name_to_id("WOLCustomLobby.wnd:ButtonHost");
    state.button_refresh_id = name_to_id("WOLCustomLobby.wnd:ButtonRefresh");
    state.button_join_id = name_to_id("WOLCustomLobby.wnd:ButtonJoin");
    state.button_buddy_id = name_to_id("WOLCustomLobby.wnd:ButtonBuddy");
    state.button_emote_id = name_to_id("WOLCustomLobby.wnd:ButtonEmote");
    state.button_sort_alpha_id = name_to_id("WOLCustomLobby.wnd:ButtonSortAlpha");
    state.button_sort_ping_id = name_to_id("WOLCustomLobby.wnd:ButtonSortPing");
    state.button_sort_buddies_id = name_to_id("WOLCustomLobby.wnd:ButtonSortBuddies");
    state.window_sort_alpha_id = name_to_id("WOLCustomLobby.wnd:WindowSortAlpha");
    state.window_sort_ping_id = name_to_id("WOLCustomLobby.wnd:WindowSortPing");
    state.window_sort_buddies_id = name_to_id("WOLCustomLobby.wnd:WindowSortBuddies");
    state.text_entry_chat_id = name_to_id("WOLCustomLobby.wnd:TextEntryChat");
    state.listbox_lobby_players_id = name_to_id("WOLCustomLobby.wnd:ListboxPlayers");
    state.listbox_lobby_chat_id = name_to_id("WOLCustomLobby.wnd:ListboxChat");
    state.combo_lobby_group_rooms_id = name_to_id("WOLCustomLobby.wnd:ComboBoxGroupRooms");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.button_back = manager.get_window_by_id(state.button_back_id);
        state.button_host = manager.get_window_by_id(state.button_host_id);
        state.button_refresh = manager.get_window_by_id(state.button_refresh_id);
        state.button_join = manager.get_window_by_id(state.button_join_id);
        state.button_buddy = manager.get_window_by_id(state.button_buddy_id);
        state.button_emote = manager.get_window_by_id(state.button_emote_id);
        state.text_entry_chat = manager.get_window_by_id(state.text_entry_chat_id);
        state.listbox_lobby_players = manager.get_window_by_id(state.listbox_lobby_players_id);
        state.listbox_lobby_chat = manager.get_window_by_id(state.listbox_lobby_chat_id);
        state.combo_lobby_group_rooms = manager.get_window_by_id(state.combo_lobby_group_rooms_id);
    });

    if let Some(button_join) = state.button_join.as_ref() {
        let _ = button_join.borrow_mut().set_enabled(false);
    }

    if let Some(listbox) = state.listbox_lobby_players.as_ref() {
        listbox
            .borrow_mut()
            .set_tooltip_callback(|win, _inst, mouse| {
                player_tooltip(win, mouse);
            });
    }
    let listbox_large_id = name_to_id("WOLCustomLobby.wnd:ListboxGamesLarge");
    let listbox_small_id = name_to_id("WOLCustomLobby.wnd:ListboxGames");
    for id in [listbox_large_id, listbox_small_id] {
        if let Some(listbox) = with_window_manager(|manager| manager.get_window_by_id(id)) {
            listbox
                .borrow_mut()
                .set_tooltip_callback(|win, _inst, mouse| {
                    game_list_tooltip(win, mouse);
                });
        }
    }

    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        info.register_text_window(state.listbox_lobby_chat_id as u32);
    }

    if let Some(entry) = state.text_entry_chat.as_ref() {
        if let Some(widget) = entry.borrow_mut().text_entry_mut() {
            widget.set_text("");
        }
    }

    if let Some(combo) = state.combo_lobby_group_rooms.as_ref() {
        populate_group_room_listbox(combo);
    }

    if CustomMatchPreferences::new().uses_long_game_list() {
        toggle_game_list_type(&mut state);
    }

    show_sort_icons(&state);

    layout.hide(false);

    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        if info.get_current_group_room() == 0 {
            if state.group_room_to_join != 0 {
                info.join_group_room(state.group_room_to_join);
                state.group_room_to_join = 0;
            } else {
                info.join_best_group_room();
            }
        }
    }

    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        info.clear_staging_room_list();
    }
    let mut req = PeerRequest::default();
    req.request_type = PeerRequestType::StartGameList;
    req.restrict_game_list = GameSpyConfig::new_sync().restrict_games_to_lobby();
    if let Some(queue) = get_peer_message_queue() {
        if let Ok(mut queue) = queue.lock() {
            queue.add_request(req);
        }
    }

    get_shell().show_shell_map(true);
    crate::gamespy_game::with_gamespy_game_info_mut(|game| game.reset());

    if let Some(entry) = state.text_entry_chat.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(entry)));
    }

    state.raise_message_boxes = true;
    state.just_entered = true;
    state.initial_gadget_delay = 2;

    let gadget_parent_id = name_to_id("WOLCustomLobby.wnd:GadgetParent");
    if let Some(gadget_parent) =
        with_window_manager(|manager| manager.get_window_by_id(gadget_parent_id))
    {
        let _ = gadget_parent.borrow_mut().hide(true);
    }

    set_dont_show_main_menu(true);
}

fn shutdown_complete(layout: &WindowLayout) {
    let mut state = wol_state()
        .lock()
        .expect("WOLLobbyMenu state lock poisoned");
    state.is_shutting_down = false;
    layout.hide(true);
    let next = state.next_screen.take();
    get_shell().shutdown_complete(layout, next.is_some());
    if let Some(next) = next {
        let _ = get_shell().push(&next, false);
    }
}

pub fn wol_lobby_menu_shutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);

    let mut prefs = CustomMatchPreferences::new();
    let uses_long = wol_state()
        .lock()
        .ok()
        .map(|state| !state.is_small_game_list)
        .unwrap_or(false);
    prefs.set_uses_long_game_list(uses_long);
    prefs.write();

    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        info.unregister_text_window(name_to_id("WOLCustomLobby.wnd:ListboxChat") as u32);
    }

    let mut req = PeerRequest::default();
    req.request_type = PeerRequestType::StopGameList;
    if let Some(queue) = get_peer_message_queue() {
        if let Ok(mut queue) = queue.lock() {
            queue.add_request(req);
        }
    }

    let mut state = wol_state()
        .lock()
        .expect("WOLLobbyMenu state lock poisoned");
    state.listbox_lobby_chat = None;
    state.listbox_lobby_players = None;
    state.is_shutting_down = true;
    set_dont_show_main_menu(false);

    if pop_immediate {
        shutdown_complete(layout);
        return;
    }

    get_shell().reverse_animate_window();
    with_window_manager(|manager| manager.transition_reverse("WOLCustomLobbyFade"));
    raise_gs_message_box();
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
                        let favorite = get_favorite_side(&resp.player);
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

pub fn wol_lobby_menu_update(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let mut state = wol_state()
        .lock()
        .expect("WOLLobbyMenu state lock poisoned");

    if state.just_entered {
        if state.initial_gadget_delay == 1 {
            with_window_manager(|manager| {
                manager.transition_remove("MainMenuDefaultMenuLogoFade", false)
            });
            with_window_manager(|manager| {
                manager.transition_set_group("WOLCustomLobbyFade", false)
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
        drop(state);
        shutdown_complete(layout);
        return;
    }

    if state.raise_message_boxes {
        raise_gs_message_box();
        state.raise_message_boxes = false;
    }

    if get_shell().is_anim_finished()
        && with_window_manager(|manager| manager.transitions_finished())
        && !state.button_pushed
    {
        drop(state);
        handle_buddy_responses();
        handle_persistent_storage_responses();

        let mut saw_important = false;
        let allowed = get_gamespy_info()
            .and_then(|info| {
                info.lock()
                    .ok()
                    .map(|guard| guard.get_max_messages_per_update())
            })
            .unwrap_or(10);
        let mut allowed = allowed;

        if let Some(queue) = get_peer_message_queue() {
            if let Ok(mut queue) = queue.lock() {
                while allowed > 0 && !saw_important {
                    allowed -= 1;
                    let Some(resp) = queue.get_response() else {
                        break;
                    };
                    match resp.response_type {
                        PeerResponseType::JoinGroupRoom => {
                            saw_important = true;
                            if resp.join_group_ok {
                                if let Some(info) =
                                    get_gamespy_info().and_then(|info| info.lock().ok())
                                {
                                    info.set_current_group_room(resp.group_room_id);
                                    info.clear_player_info();
                                    if let Some(room) =
                                        info.get_group_room_list().get(&resp.group_room_id)
                                    {
                                        let msg = GameText::fetch("GUI:LobbyJoined")
                                            .replace("%s", room.translated_name.as_str());
                                        info.add_text(
                                            msg,
                                            default_gamespy_colors()
                                                [GameSpyColor::Default as usize],
                                            None,
                                        );
                                    }
                                }
                            } else if let Some(info) =
                                get_gamespy_info().and_then(|info| info.lock().ok())
                            {
                                info.join_best_group_room();
                            }
                            if let Some(combo) = wol_state()
                                .lock()
                                .ok()
                                .and_then(|state| state.combo_lobby_group_rooms.clone())
                            {
                                populate_group_room_listbox(&combo);
                            }
                            refresh_player_list(true);
                        }
                        PeerResponseType::PlayerChangedFlags
                        | PeerResponseType::PlayerChangedNick
                        | PeerResponseType::PlayerInfo => {
                            if let Some(mut info) =
                                get_gamespy_info().and_then(|info| info.lock().ok())
                            {
                                let player = fill_player_info(&resp);
                                let old =
                                    if resp.response_type == PeerResponseType::PlayerChangedNick {
                                        Some(AsciiString::from(resp.old_nick.clone()))
                                    } else {
                                        None
                                    };
                                info.update_player_info(player, old);
                            }
                        }
                        PeerResponseType::PlayerJoin => {
                            if resp.player_room_type == ROOM_TYPE_GROUP {
                                if let Some(mut info) =
                                    get_gamespy_info().and_then(|info| info.lock().ok())
                                {
                                    let player = fill_player_info(&resp);
                                    info.update_player_info(player, None);
                                }
                            }
                        }
                        PeerResponseType::PlayerLeft => {
                            if let Some(mut info) =
                                get_gamespy_info().and_then(|info| info.lock().ok())
                            {
                                info.player_left_group_room(AsciiString::from(resp.nick.clone()));
                            }
                        }
                        PeerResponseType::Message => {
                            if let Some(mut info) =
                                get_gamespy_info().and_then(|info| info.lock().ok())
                            {
                                info.add_chat(
                                    AsciiString::from(resp.nick.clone()),
                                    resp.message_profile_id,
                                    resp.text.clone(),
                                    !resp.message_is_private,
                                    resp.message_is_action,
                                    Some(name_to_id("WOLCustomLobby.wnd:ListboxChat") as u32),
                                );
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
                            if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok())
                            {
                                info.reset();
                            }
                            let _ = get_shell().pop();
                        }
                        PeerResponseType::CreateStagingRoom => {
                            saw_important = true;
                            let mut state = wol_state()
                                .lock()
                                .expect("WOLLobbyMenu state lock poisoned");
                            state.trying_to_host_or_join = false;
                            if resp.create_staging_result == 0 {
                                state.button_pushed = true;
                                state.next_screen =
                                    Some("Menus/GameSpyGameOptionsMenu.wnd".to_string());
                                let _ = get_shell().pop();
                                if let Some(info) =
                                    get_gamespy_info().and_then(|info| info.lock().ok())
                                {
                                    info.mark_as_staging_room_host();
                                }
                            }
                        }
                        PeerResponseType::JoinStagingRoom => {
                            saw_important = true;
                            let mut state = wol_state()
                                .lock()
                                .expect("WOLLobbyMenu state lock poisoned");
                            state.trying_to_host_or_join = false;
                            if resp.join_staging_ok {
                                state.button_pushed = true;
                                state.next_screen =
                                    Some("Menus/GameSpyGameOptionsMenu.wnd".to_string());
                                let _ = get_shell().pop();
                            } else {
                                let msg = GameText::fetch("GUI:JoinFailedDefault");
                                gs_message_box_ok(
                                    &GameText::fetch("GUI:JoinFailedDefault"),
                                    &msg,
                                    None,
                                );
                            }
                        }
                        PeerResponseType::StagingRoomListComplete => {
                            if let Some(mut info) =
                                get_gamespy_info().and_then(|info| info.lock().ok())
                            {
                                info.saw_full_game_list();
                            }
                        }
                        PeerResponseType::StagingRoom => {
                            if let Some(mut info) =
                                get_gamespy_info().and_then(|info| info.lock().ok())
                            {
                                let mut room = GameSpyStagingRoom::default();
                                room.id = resp.staging_id;
                                room.name = resp.staging_server_name.clone();
                                room.map_name =
                                    AsciiString::from(resp.staging_room_map_name.clone());
                                room.has_password = resp.staging_requires_password;
                                room.allow_observers = resp.staging_allow_observers;
                                room.use_stats = resp.staging_use_stats;
                                room.exe_crc = resp.staging_exe_crc;
                                room.ini_crc = resp.staging_ini_crc;
                                room.version = resp.staging_version;
                                room.ladder_ip =
                                    AsciiString::from(resp.staging_server_ladder_ip.clone());
                                room.ladder_port = resp.staging_ladder_port;
                                room.num_players = resp.staging_num_players;
                                room.max_players = resp.staging_max_players;
                                room.num_observers = resp.staging_num_observers;
                                room.host_ping =
                                    AsciiString::from(resp.staging_server_ping_string.clone());
                                room.ping_string =
                                    AsciiString::from(resp.staging_server_ping_string.clone());
                                for i in 0..MAX_SLOTS {
                                    room.player_names[i] = AsciiString::from(
                                        resp.staging_room_player_names[i].clone(),
                                    );
                                    room.slot_profiles[i] = resp.staging_profiles[i];
                                    room.slot_wins[i] = resp.staging_wins[i];
                                    room.slot_losses[i] = resp.staging_losses[i];
                                    room.slot_faction[i] = resp.staging_faction[i];
                                    room.slot_color[i] = resp.staging_color[i];
                                }

                                if resp.staging_action == 0 {
                                    info.clear_staging_room_list();
                                } else {
                                    if resp.staging_action == 2 {
                                        info.update_staging_room(room);
                                    } else if resp.staging_action == 3 {
                                        info.remove_staging_room(&room);
                                    } else {
                                        info.add_staging_room(room);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        refresh_player_list(false);
        refresh_game_list(false);
    }

    flush_gamespy_chat_entries();
}

fn fill_player_info(resp: &PeerResponse) -> PlayerInfo {
    let mut info = PlayerInfo::default();
    info.name = AsciiString::from(resp.nick.clone());
    info.profile_id = resp.player_profile_id;
    info.flags = resp.player_flags;
    info.wins = resp.player_wins;
    info.losses = resp.player_losses;
    info.locale = AsciiString::from(resp.locale.clone());
    info.rank_points = resp.player_rank_points;
    info.side = resp.player_side;
    info.preorder = resp.player_preorder;
    info
}

fn flush_gamespy_chat_entries() {
    let Some(listbox_window) = wol_state()
        .lock()
        .ok()
        .and_then(|state| state.listbox_lobby_chat.clone())
    else {
        return;
    };
    let mut entries = None;
    if let Some(mut info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        entries = Some(info.drain_chat_entries());
    }
    let Some(mut entries) = entries else {
        return;
    };
    if entries.is_empty() {
        return;
    }

    let mut listbox_guard = listbox_window.borrow_mut();
    let Some(listbox) = listbox_guard.list_box_mut() else {
        return;
    };

    while let Some(entry) = entries.pop_front() {
        if entry.window_id.is_some()
            && entry.window_id != Some(name_to_id("WOLCustomLobby.wnd:ListboxChat") as u32)
        {
            continue;
        }
        listbox.add_item_with_color(&entry.text, entry.color);
    }
}

fn game_list_tooltip(window: &GameWindow, mouse: u32) {
    let x = (mouse & 0xFFFF) as i16 as i32;
    let y = ((mouse >> 16) & 0xFFFF) as i16 as i32;
    let Some(WindowWidget::ListBox(listbox)) = window.widget() else {
        return;
    };
    let (row, col) = listbox.entry_from_xy(x, y);
    if row < 0 || col < 0 {
        set_window_tooltip(window, "");
        return;
    }
    let row = row as usize;
    let game_id = listbox
        .get_item_data(row)
        .and_then(|data| {
            if let ListBoxItemData::Integer(id) = data {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap_or(0);
    let info = get_gamespy_info().and_then(|info| info.lock().ok());
    let Some(info) = info else {
        set_window_tooltip(window, &GameText::fetch("TOOLTIP:UnknownGame"));
        return;
    };
    let Some(room) = info.find_staging_room_by_id(game_id) else {
        set_window_tooltip(window, &GameText::fetch("TOOLTIP:UnknownGame"));
        return;
    };

    if col == COLUMN_PING {
        set_window_tooltip(window, &GameText::fetch("TOOLTIP:PingInfo"));
        return;
    }
    if col == COLUMN_NUMPLAYERS {
        set_window_tooltip(window, &GameText::fetch("TOOLTIP:NumberOfPlayers"));
        return;
    }
    if col == COLUMN_PASSWORD {
        if room.has_password {
            let mut check = GameText::fetch("TOOTIP:Password");
            if check == "Password required to joing game" {
                check = "Password required to join game".to_string();
            }
            set_window_tooltip(window, &check);
        } else {
            set_window_tooltip(window, "");
        }
        return;
    }
    if col == COLUMN_USE_STATS {
        if room.use_stats {
            set_window_tooltip(window, &GameText::fetch("TOOLTIP:UseStatsOn"));
        } else {
            set_window_tooltip(window, &GameText::fetch("TOOLTIP:UseStatsOff"));
        }
        return;
    }

    let mut map_name = room.map_name.as_str().to_string();
    if let Ok(mut cache) = get_map_cache_manager().lock() {
        cache.update_cache();
        if let Some(meta) = cache.find_map(room.map_name.as_str()) {
            map_name = meta.display_name;
        }
    }
    if map_name.is_empty() {
        let raw = room.map_name.as_str();
        let trimmed = raw.rsplit(['\\', '/']).next().unwrap_or(raw);
        map_name = trimmed.to_string();
    }

    let mut tooltip = GameText::fetch("TOOLTIP:GameInfoGameName").replace("%s", &room.name);
    if room.ladder_port != 0 {
        if let Some(ladder) = get_ladder_list()
            .and_then(|list| list.read().ok())
            .and_then(|list| list.find_ladder(&room.ladder_ip, room.ladder_port))
        {
            let line = GameText::fetch("TOOLTIP:GameInfoLadderName").replace("%s", &ladder.name);
            tooltip.push_str(&line);
        }
    }
    if let Some((exe_crc, ini_crc)) = get_global_data()
        .and_then(|data| data.read().ok().map(|guard| (guard.exe_crc, guard.ini_crc)))
    {
        if room.exe_crc != exe_crc || room.ini_crc != ini_crc {
            let line = GameText::fetch("TOOLTIP:InvalidGameVersion").replace("%s", &map_name);
            tooltip.push_str(&line);
        }
    }
    let line = GameText::fetch("TOOLTIP:GameInfoMap").replace("%s", &map_name);
    tooltip.push_str(&line);

    let mut num_players = 0;
    for i in 0..MAX_SLOTS {
        let name = &room.player_names[i];
        let profile_id = room.slot_profiles[i];
        if name.is_empty() && profile_id == 0 {
            continue;
        }
        if let Some(state) = slot_state_from_profile(profile_id, name) {
            tooltip.push('\n');
            let text = match state {
                SlotState::EasyAI => GameText::fetch("GUI:EasyAI"),
                SlotState::MedAI => GameText::fetch("GUI:MediumAI"),
                SlotState::BrutalAI => GameText::fetch("GUI:HardAI"),
                _ => String::new(),
            };
            tooltip.push_str(&text);
            num_players += 1;
            continue;
        }
        if !name.is_empty() && profile_id > 0 {
            let wins = room.slot_wins[i];
            let losses = room.slot_losses[i];
            let mut line = GameText::fetch("TOOLTIP:GameInfoPlayer");
            line = line.replace("%s", name.as_str());
            line = line.replace("%d", &wins.to_string());
            line = line.replace("%2", &losses.to_string());
            tooltip.push_str(&line);
            num_players += 1;
        }
    }

    if num_players == 0 {
        set_window_tooltip(window, "");
        return;
    }

    set_window_tooltip(window, &tooltip);
}

fn player_tooltip(window: &GameWindow, mouse: u32) {
    let x = (mouse & 0xFFFF) as i16 as i32;
    let y = ((mouse >> 16) & 0xFFFF) as i16 as i32;
    let Some(WindowWidget::ListBox(listbox)) = window.widget() else {
        return;
    };
    let (row, col) = listbox.entry_from_xy(x, y);
    if row < 0 || col < 0 {
        return;
    }
    let row = row as usize;
    let name = match listbox.items().get(row) {
        Some(item) => item.text.clone(),
        None => return,
    };
    let a_name = AsciiString::from(name.clone());
    let info = get_gamespy_info().and_then(|info| info.lock().ok());
    let Some(info) = info else {
        return;
    };
    let player = info
        .get_player_info_map()
        .get(&a_name.as_str().to_lowercase());
    let Some(player) = player else {
        return;
    };

    if col == 0 {
        if player.preorder != 0 {
            set_window_tooltip(window, &GameText::fetch("TOOLTIP:LobbyOfficersClub"));
        } else {
            set_window_tooltip(window, "");
        }
        return;
    }

    let locale_val = player.locale.as_str().parse::<i32>().unwrap_or(0);
    let locale_key = format!("WOL:Locale{:02}", locale_val);
    let player_info = GameText::fetch("TOOLTIP:PlayerInfo")
        .replace("%s", GameText::fetch(&locale_key).as_str())
        .replace("%d", &player.wins.to_string())
        .replace("%2", &player.losses.to_string());
    let is_local = get_gamespy_info()
        .and_then(|info| {
            info.lock().ok().map(|guard| {
                guard
                    .get_local_name()
                    .compare_no_case_str(player.name.as_str())
                    == std::cmp::Ordering::Equal
            })
        })
        .unwrap_or(false);
    let mut tooltip = if is_local {
        GameText::fetch("TOOLTIP:LocalPlayer").replace("%s", &name)
    } else if get_gamespy_info()
        .and_then(|info| {
            info.lock()
                .ok()
                .map(|guard| guard.is_buddy(player.profile_id))
        })
        .unwrap_or(false)
    {
        GameText::fetch("TOOLTIP:BuddyPlayer").replace("%s", &name)
    } else {
        GameText::fetch("TOOLTIP:ProfiledPlayer").replace("%s", &name)
    };
    tooltip.push_str(&player_info);
    set_window_tooltip(window, &tooltip);
}

fn set_window_tooltip(window: &GameWindow, tooltip: &str) {
    let id = window.get_id();
    with_window_manager(|manager| {
        if let Some(win) = manager.get_window_by_id(id) {
            win.borrow_mut().set_tooltip(tooltip);
        }
    });
}

pub fn wol_lobby_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char {
        let key = data1 as u32;
        let state = data2 as u32;
        if key == KEY_ESC && (state & KEY_STATE_UP) != 0 {
            let state = wol_state()
                .lock()
                .expect("WOLLobbyMenu state lock poisoned");
            if let Some(button_back) = state.button_back.as_ref() {
                let _ = button_back.borrow_mut().send_system_message(
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

pub fn wol_lobby_menu_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => {
            return WindowMsgHandled::Handled;
        }
        WindowMessage::GadgetValueChanged => {
            let control_id = data1 as i32;
            if control_id == get_game_list_box_id() {
                let mut state = wol_state()
                    .lock()
                    .expect("WOLLobbyMenu state lock poisoned");
                let selected = get_game_list_box().and_then(|win| {
                    win.borrow().widget().and_then(|widget| match widget {
                        WindowWidget::ListBox(lb) => lb.selected_indices().first().copied(),
                        _ => None,
                    })
                });
                if selected.is_some() {
                    if let Some(button_join) = state.button_join.as_ref() {
                        let _ = button_join.borrow_mut().set_enabled(true);
                    }
                } else if let Some(button_join) = state.button_join.as_ref() {
                    let _ = button_join.borrow_mut().set_enabled(false);
                }
                if let (Some(game_list), Some(info_list)) =
                    (get_game_list_box(), get_game_info_list_box())
                {
                    let game_list_ref = game_list.borrow();
                    refresh_game_info_list_box(&*game_list_ref, &info_list);
                }
            } else if control_id == name_to_id("WOLCustomLobby.wnd:ComboBoxGroupRooms") {
                if let Some(combo) = wol_state()
                    .lock()
                    .ok()
                    .and_then(|state| state.combo_lobby_group_rooms.clone())
                {
                    let selected = combo.borrow().widget().and_then(|widget| match widget {
                        WindowWidget::ComboBox(cb) => cb.selected_item_data(),
                        _ => None,
                    });
                    if let Some(group_id) = selected {
                        if let Some(mut info) = get_gamespy_info().and_then(|info| info.lock().ok())
                        {
                            if group_id != info.get_current_group_room() {
                                info.leave_group_room();
                                info.join_group_room(group_id);
                                if GameSpyConfig::new_sync().restrict_games_to_lobby() {
                                    info.clear_staging_room_list();
                                    refresh_game_list_boxes();
                                    let mut req = PeerRequest::default();
                                    req.request_type = PeerRequestType::StartGameList;
                                    req.restrict_game_list = true;
                                    if let Some(queue) = get_peer_message_queue() {
                                        if let Ok(mut queue) = queue.lock() {
                                            queue.add_request(req);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            let (sort_alpha_id, sort_ping_id, sort_buddies_id) = wol_state()
                .lock()
                .ok()
                .map(|state| {
                    (
                        state.button_sort_alpha_id,
                        state.button_sort_ping_id,
                        state.button_sort_buddies_id,
                    )
                })
                .unwrap_or((0, 0, 0));
            if control_id == name_to_id("WOLCustomLobby.wnd:ButtonBack") {
                let mut state = wol_state()
                    .lock()
                    .expect("WOLLobbyMenu state lock poisoned");
                if state.trying_to_host_or_join {
                    return WindowMsgHandled::Handled;
                }
                if let Some(mut info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                    info.leave_group_room();
                }
                state.trying_to_host_or_join = true;
                state.button_pushed = true;
                state.next_screen = Some("Menus/WOLWelcomeMenu.wnd".to_string());
                let _ = get_shell().pop();
            } else if control_id == name_to_id("WOLCustomLobby.wnd:ButtonRefresh") {
                refresh_game_list(true);
                refresh_player_list(true);
            } else if control_id == name_to_id("WOLCustomLobby.wnd:ButtonHost") {
                let mut state = wol_state()
                    .lock()
                    .expect("WOLLobbyMenu state lock poisoned");
                if state.trying_to_host_or_join {
                    return WindowMsgHandled::Handled;
                }
                state.trying_to_host_or_join = true;
                state.queued_utms.clear();
                state.group_room_to_join = get_gamespy_info()
                    .and_then(|info| info.lock().ok().map(|guard| guard.get_current_group_room()))
                    .unwrap_or(0);
                open_overlay(GameSpyOverlayType::GameOptions);
            } else if control_id == name_to_id("WOLCustomLobby.wnd:ButtonJoin") {
                let mut state = wol_state()
                    .lock()
                    .expect("WOLLobbyMenu state lock poisoned");
                if state.trying_to_host_or_join {
                    return WindowMsgHandled::Handled;
                }
                state.queued_utms.clear();
                state.group_room_to_join = get_gamespy_info()
                    .and_then(|info| info.lock().ok().map(|guard| guard.get_current_group_room()))
                    .unwrap_or(0);

                let Some(listbox) = get_game_list_box() else {
                    gs_message_box_ok(
                        &GameText::fetch("GUI:Error"),
                        &GameText::fetch("GUI:NoGameSelected"),
                        None,
                    );
                    return WindowMsgHandled::Handled;
                };
                let selected = listbox.borrow().widget().and_then(|widget| match widget {
                    WindowWidget::ListBox(lb) => lb.selected_indices().first().copied(),
                    _ => None,
                });
                let Some(selected) = selected else {
                    gs_message_box_ok(
                        &GameText::fetch("GUI:Error"),
                        &GameText::fetch("GUI:NoGameSelected"),
                        None,
                    );
                    return WindowMsgHandled::Handled;
                };
                let id = listbox
                    .borrow()
                    .widget()
                    .and_then(|widget| match widget {
                        WindowWidget::ListBox(lb) => lb.get_item_data(selected),
                        _ => None,
                    })
                    .and_then(|data| {
                        if let ListBoxItemData::Integer(id) = data {
                            Some(*id)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                if id <= 0 {
                    gs_message_box_ok(
                        &GameText::fetch("GUI:Error"),
                        &GameText::fetch("GUI:NoGameInfo"),
                        None,
                    );
                    return WindowMsgHandled::Handled;
                }
                if let Some(mut info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                    if let Some(room) = info.find_staging_room_by_id(id) {
                        if let Some((exe_crc, ini_crc)) = get_global_data().and_then(|data| {
                            data.read().ok().map(|guard| (guard.exe_crc, guard.ini_crc))
                        }) {
                            if room.exe_crc != exe_crc || room.ini_crc != ini_crc {
                                gs_message_box_ok(
                                    &GameText::fetch("GUI:JoinFailedDefault"),
                                    &GameText::fetch("GUI:JoinFailedCRCMismatch"),
                                    None,
                                );
                                return WindowMsgHandled::Handled;
                            }
                        }
                        info.mark_as_staging_room_joiner(id);
                        with_gamespy_game_info(|game| {
                            game.set_game_name(room.name.clone());
                            game.set_ladder_ip(room.ladder_ip.clone());
                            game.set_ladder_port(room.ladder_port);
                        });
                        state.trying_to_host_or_join = true;
                        if room.has_password {
                            open_overlay(GameSpyOverlayType::GamePassword);
                        } else {
                            let mut req = PeerRequest::default();
                            req.request_type = PeerRequestType::JoinStagingRoom;
                            req.text = room.name.clone();
                            req.staging_room_id = id;
                            req.password = String::new();
                            if let Some(queue) = get_peer_message_queue() {
                                if let Ok(mut queue) = queue.lock() {
                                    queue.add_request(req);
                                }
                            }
                        }
                    }
                }
            } else if control_id == name_to_id("WOLCustomLobby.wnd:ButtonBuddy") {
                toggle_overlay(GameSpyOverlayType::Buddy);
            } else if control_id == name_to_id("WOLCustomLobby.wnd:ButtonGameListToggle") {
                let mut state = wol_state()
                    .lock()
                    .expect("WOLLobbyMenu state lock poisoned");
                toggle_game_list_type(&mut state);
            } else if control_id == sort_alpha_id {
                let mut state = wol_state()
                    .lock()
                    .expect("WOLLobbyMenu state lock poisoned");
                if state.sort_type == GameSortType::AlphaAscending {
                    set_sort_mode(&mut state, GameSortType::AlphaDescending);
                } else {
                    set_sort_mode(&mut state, GameSortType::AlphaAscending);
                }
            } else if control_id == sort_ping_id {
                let mut state = wol_state()
                    .lock()
                    .expect("WOLLobbyMenu state lock poisoned");
                if state.sort_type == GameSortType::PingAscending {
                    set_sort_mode(&mut state, GameSortType::PingDescending);
                } else {
                    set_sort_mode(&mut state, GameSortType::PingAscending);
                }
            } else if control_id == sort_buddies_id {
                let mut state = wol_state()
                    .lock()
                    .expect("WOLLobbyMenu state lock poisoned");
                toggle_sort_buddies(&mut state);
            } else if control_id == name_to_id("WOLCustomLobby.wnd:ButtonEmote") {
                if let Some(entry) = wol_state()
                    .lock()
                    .ok()
                    .and_then(|state| state.text_entry_chat.clone())
                {
                    let listbox_id = wol_state()
                        .lock()
                        .ok()
                        .map(|state| state.listbox_lobby_players_id as u32)
                        .unwrap_or(0);
                    if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                        let text = widget.text().trim().to_string();
                        widget.set_text("");
                        if !text.is_empty() {
                            if let Some(mut info) =
                                get_gamespy_info().and_then(|info| info.lock().ok())
                            {
                                let _ = info.send_chat(text, false, Some(listbox_id));
                            }
                        }
                    }
                }
            }
        }
        WindowMessage::GadgetEditDone => {
            let control_id = data1 as i32;
            let state = wol_state()
                .lock()
                .expect("WOLLobbyMenu state lock poisoned");
            if control_id == state.text_entry_chat_id {
                if let Some(entry) = state.text_entry_chat.as_ref() {
                    if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                        let text = widget.text().trim().to_string();
                        widget.set_text("");
                        if !text.is_empty() {
                            let listbox_id = state.listbox_lobby_players_id as u32;
                            if !handle_lobby_slash_commands(&text, listbox_id) {
                                if let Some(mut info) =
                                    get_gamespy_info().and_then(|info| info.lock().ok())
                                {
                                    let _ = info.send_chat(text, false, Some(listbox_id));
                                }
                            }
                        }
                    }
                }
            }
        }
        WindowMessage::GadgetRightClick => {
            let control_id = data1 as i32;
            let state = wol_state()
                .lock()
                .expect("WOLLobbyMenu state lock poisoned");
            if control_id == state.listbox_lobby_players_id {
                let Some(listbox_window) = state.listbox_lobby_players.as_ref() else {
                    return WindowMsgHandled::Handled;
                };
                let (rc, mouse_x, mouse_y) =
                    match listbox_right_click_info(&listbox_window.borrow()) {
                        Some(info) => info,
                        None => return WindowMsgHandled::Handled,
                    };
                if rc.index < 0 {
                    return WindowMsgHandled::Handled;
                }
                let index = rc.index as usize;
                let nick = listbox_window
                    .borrow_mut()
                    .list_box_mut()
                    .and_then(|lb| lb.items().get(index))
                    .map(|item| item.text.clone())
                    .unwrap_or_default();
                let nick_ascii = AsciiString::from(nick.clone());
                let profile_id = get_gamespy_info()
                    .and_then(|info| info.lock().ok())
                    .and_then(|info| {
                        info.get_player_info_map()
                            .get(&nick.to_lowercase())
                            .map(|p| p.profile_id)
                    })
                    .unwrap_or(0);

                let is_buddy = get_gamespy_info()
                    .and_then(|info| info.lock().ok().map(|guard| guard.is_buddy(profile_id)))
                    .unwrap_or(false);
                let local_profile = get_gamespy_info()
                    .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
                    .unwrap_or(0);
                let layout_name = if profile_id == 0 {
                    "Menus/RCNoProfileMenu.wnd"
                } else if profile_id == local_profile {
                    "Menus/RCLocalPlayerMenu.wnd"
                } else if is_buddy {
                    "Menus/RCBuddiesMenu.wnd"
                } else {
                    "Menus/RCNonBuddiesMenu.wnd"
                };

                let layout = with_window_manager(|manager| {
                    manager.create_layout_with_windows(layout_name).ok()
                });
                if let Some((layout, _)) = layout {
                    layout.borrow().run_init(None);
                    if let Some(rc_menu) = layout.borrow().get_first_window() {
                        rc_menu.borrow_mut().hide(false);
                        rc_menu.borrow_mut().bring_to_front();
                        let (win_w, win_h) = rc_menu.borrow().get_size();
                        let (screen_w, screen_h) =
                            with_window_manager(|manager| manager.screen_size());
                        let mut pos_x = mouse_x;
                        let mut pos_y = mouse_y;
                        if pos_x + win_w > screen_w {
                            pos_x = screen_w - win_w;
                        }
                        if pos_y + win_h > screen_h {
                            pos_y = screen_h - win_h;
                        }
                        let _ = rc_menu.borrow_mut().set_position(pos_x, pos_y);

                        set_unignore_text(&layout.borrow(), &nick_ascii, profile_id);
                        let item_type = if is_buddy {
                            crate::gui::callbacks::wol_buddy_overlay::RcItemType::Buddy
                        } else {
                            crate::gui::callbacks::wol_buddy_overlay::RcItemType::NonBuddy
                        };
                        rc_menu.borrow_mut().set_user_data(
                            crate::gui::callbacks::wol_buddy_overlay::GameSpyRcMenuData {
                                id: profile_id,
                                nick: nick_ascii,
                                item_type,
                            },
                        );
                        with_window_manager(|manager| manager.set_lone_window(Some(&rc_menu)));
                    }
                }
            } else if control_id == get_game_list_box_id() {
                let Some(listbox_window) = get_game_list_box() else {
                    return WindowMsgHandled::Handled;
                };
                let (rc, mouse_x, mouse_y) =
                    match listbox_right_click_info(&listbox_window.borrow()) {
                        Some(info) => info,
                        None => return WindowMsgHandled::Handled,
                    };
                if rc.index < 0 {
                    if let Some(mut listbox) = listbox_window.borrow_mut().list_box_mut() {
                        listbox.set_selected_indices(&[]);
                    }
                    return WindowMsgHandled::Handled;
                }

                let index = rc.index as usize;
                let selected_id = listbox_window
                    .borrow()
                    .widget()
                    .and_then(|widget| match widget {
                        WindowWidget::ListBox(lb) => lb.get_item_data(index),
                        _ => None,
                    })
                    .and_then(|data| {
                        if let ListBoxItemData::Integer(id) = data {
                            Some(*id)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);

                if let Some(mut listbox) = listbox_window.borrow_mut().list_box_mut() {
                    listbox.set_selected_indices(&[index]);
                }

                if selected_id <= 0 {
                    return WindowMsgHandled::Handled;
                }

                let ladder_ok = get_gamespy_info()
                    .and_then(|info| info.lock().ok())
                    .and_then(|info| info.find_staging_room_by_id(selected_id))
                    .and_then(|room| {
                        get_ladder_list()
                            .and_then(|list| list.read().ok())
                            .and_then(|list| list.find_ladder(&room.ladder_ip, room.ladder_port))
                    })
                    .is_some();
                if !ladder_ok {
                    return WindowMsgHandled::Handled;
                }

                let layout = with_window_manager(|manager| {
                    manager
                        .create_layout_with_windows("Menus/RCGameDetailsMenu.wnd")
                        .ok()
                });
                if let Some((layout, _)) = layout {
                    layout.borrow().run_init(None);
                    if let Some(rc_menu) = layout.borrow().get_first_window() {
                        rc_menu.borrow_mut().hide(false);
                        rc_menu.borrow_mut().bring_to_front();
                        let (win_w, win_h) = rc_menu.borrow().get_size();
                        let (screen_w, screen_h) =
                            with_window_manager(|manager| manager.screen_size());
                        let mut pos_x = mouse_x;
                        let mut pos_y = mouse_y;
                        if pos_x + win_w > screen_w {
                            pos_x = screen_w - win_w;
                        }
                        if pos_y + win_h > screen_h {
                            pos_y = screen_h - win_h;
                        }
                        let _ = rc_menu.borrow_mut().set_position(pos_x, pos_y);
                        rc_menu.borrow_mut().set_user_data(selected_id);
                        with_window_manager(|manager| manager.set_lone_window(Some(&rc_menu)));
                    }
                }
            }
        }
        WindowMessage::Destroy => {
            close_right_click_menu(window);
        }
        _ => return WindowMsgHandled::Ignored,
    }

    WindowMsgHandled::Handled
}
