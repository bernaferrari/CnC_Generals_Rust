//! WOLWelcomeMenu.cpp callback port.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::display::image::Image;
use crate::game_text::GameText;
use crate::gamespy_overlay::{
    gs_message_box_ok, open_overlay, raise_gs_message_box, toggle_overlay, GameSpyOverlayType,
};
use crate::gui::callbacks::wol_buddy_overlay::handle_buddy_responses;
use crate::gui::gadgets::{ListBox, ListBoxItemData};
use crate::gui::{
    get_shell, with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::preferences::GameSpyMiscPreferences;
use game_engine::common::rts::player_template::get_player_template_store;
use game_engine::common::skirmish_battle_honors::{
    BATTLE_HONOR_AIR_WING, BATTLE_HONOR_APOCALYPSE, BATTLE_HONOR_BATTLE_TANK, BATTLE_HONOR_BLITZ10,
    BATTLE_HONOR_BLITZ5, BATTLE_HONOR_GLOBAL_GENERAL, BATTLE_HONOR_OFFICERSCLUB,
    MAX_BATTLE_HONOR_COLUMNS, MAX_BATTLE_HONOR_IMAGE_HEIGHT, MAX_BATTLE_HONOR_IMAGE_WIDTH,
};
use game_engine::common::user_preferences::UserPreferences;
use game_network::gamespy::buddy_thread::{
    get_buddy_message_queue, BuddyRequest, BuddyRequestType,
};
use game_network::gamespy::peer_defs::{
    default_gamespy_colors, get_gamespy_info, make_color, GameSpyColor, GameSpyGroupRoom,
};
use game_network::gamespy::peer_thread::{
    get_peer_message_queue, PeerRequest, PeerRequestType, PeerResponse, PeerResponseType,
};
use game_network::gamespy::persistent_storage_thread::{
    get_ps_message_queue, PSResponseType, LOC_MAX, LOC_MIN,
};
use game_network::rank_point_value::{calculate_rank, get_favorite_side, get_rank_point_values};

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;

#[derive(Default)]
struct WolWelcomeState {
    parent_id: u32,
    button_back_id: u32,
    button_quick_match_id: u32,
    button_lobby_id: u32,
    button_buddies_id: u32,
    button_ladder_id: u32,
    button_my_info_id: u32,
    button_options_id: u32,
    listbox_info_id: u32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_back: Option<Rc<RefCell<GameWindow>>>,
    button_quick_match: Option<Rc<RefCell<GameWindow>>>,
    button_lobby: Option<Rc<RefCell<GameWindow>>>,
    button_buddies: Option<Rc<RefCell<GameWindow>>>,
    button_ladder: Option<Rc<RefCell<GameWindow>>>,
    button_my_info: Option<Rc<RefCell<GameWindow>>>,
    button_options: Option<Rc<RefCell<GameWindow>>>,
    listbox_info: Option<Rc<RefCell<GameWindow>>>,
    static_text_server_name: Option<Rc<RefCell<GameWindow>>>,
    static_text_last_updated: Option<Rc<RefCell<GameWindow>>>,
    static_text_ladder_wins: Option<Rc<RefCell<GameWindow>>>,
    static_text_ladder_losses: Option<Rc<RefCell<GameWindow>>>,
    static_text_ladder_rank: Option<Rc<RefCell<GameWindow>>>,
    static_text_ladder_points: Option<Rc<RefCell<GameWindow>>>,
    static_text_ladder_disconnects: Option<Rc<RefCell<GameWindow>>>,
    static_text_highscore_wins: Option<Rc<RefCell<GameWindow>>>,
    static_text_highscore_losses: Option<Rc<RefCell<GameWindow>>>,
    static_text_highscore_rank: Option<Rc<RefCell<GameWindow>>>,
    static_text_highscore_points: Option<Rc<RefCell<GameWindow>>>,
    is_shutting_down: bool,
    button_pushed: bool,
    raise_message_boxes: bool,
    next_screen: Option<String>,
    last_num_players_online: i32,
    server_name: String,
    win_stats: HashMap<String, f32>,
    total_win_percent: f32,
}

static WOL_WELCOME_STATE: OnceLock<Mutex<WolWelcomeState>> = OnceLock::new();
static LOOK_AT_PLAYER: OnceLock<Mutex<(i32, String)>> = OnceLock::new();

fn wol_state() -> &'static Mutex<WolWelcomeState> {
    WOL_WELCOME_STATE.get_or_init(|| Mutex::new(WolWelcomeState::default()))
}

pub fn set_look_at_player(profile_id: i32, name: &str) {
    let slot = LOOK_AT_PLAYER.get_or_init(|| Mutex::new((0, String::new())));
    let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    *guard = (profile_id, name.to_string());
}

pub fn get_look_at_player() -> (i32, String) {
    LOOK_AT_PLAYER
        .get_or_init(|| Mutex::new((0, String::new())))
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or((0, String::new()))
}

fn name_to_id(name: &str) -> u32 {
    NameKeyGenerator::name_to_key(name) as u32
}

fn enable_controls(state: &mut WolWelcomeState, enabled: bool) {
    if let Some(button) = state.button_quick_match.as_ref() {
        let _ = button.borrow_mut().set_enabled(enabled);
    }
    if let Some(button) = state.button_lobby.as_ref() {
        let _ = button.borrow_mut().set_enabled(enabled);
    }
}

fn update_server_display(state: &mut WolWelcomeState, server_name: &str) {
    if let Some(window) = state.static_text_server_name.as_ref() {
        let _ = window.borrow_mut().set_text(server_name);
    }
    state.server_name = server_name.to_string();
}

pub fn set_wol_server_name(server_name: &str) {
    let mut state = wol_state().lock().unwrap_or_else(|e| e.into_inner());
    update_server_display(&mut state, server_name);
}

fn grab_ubyte(hex: &str) -> u8 {
    let slice = &hex[..2.min(hex.len())];
    u8::from_str_radix(slice, 16).unwrap_or(0)
}

fn replace_first(haystack: &str, needle: &str, replacement: &str) -> String {
    if let Some(pos) = haystack.find(needle) {
        let mut out = String::with_capacity(haystack.len() + replacement.len());
        out.push_str(&haystack[..pos]);
        out.push_str(replacement);
        out.push_str(&haystack[pos + needle.len()..]);
        out
    } else {
        haystack.to_string()
    }
}

fn update_num_players_online(state: &mut WolWelcomeState) {
    let players_online_id = name_to_id("WOLWelcomeMenu.wnd:StaticTextNumPlayersOnline");
    with_window_manager(|manager| {
        if let Some(window) = manager.get_window_by_id(players_online_id) {
            let template = GameText::fetch("GUI:NumPlayersOnline");
            let text = replace_first(&template, "%d", &state.last_num_players_online.to_string());
            let _ = window.borrow_mut().set_text(&text);
        }
    });

    let Some(listbox) = state.listbox_info.as_ref() else {
        return;
    };
    let Some(info) = get_gamespy_info() else {
        return;
    };
    let Ok(info) = info.lock() else {
        return;
    };

    let mut listbox_guard = listbox.borrow_mut();
    let Some(list_box) = listbox_guard.list_box_mut() else {
        return;
    };
    list_box.clear();

    let colors = default_gamespy_colors();
    let mut heading = GameText::fetch("MOTD:NumPlayersHeading");
    if heading.is_empty() {
        heading = " ".to_string();
    }
    for line in heading.split('\n') {
        let trimmed = line.trim_end_matches('\r').trim();
        let text = if trimmed.is_empty() { " " } else { trimmed };
        list_box.add_item_with_data_and_color(
            -1,
            text,
            None,
            Some(colors[GameSpyColor::MotdHeading as usize]),
        );
    }
    list_box.add_item_with_data_and_color(
        -1,
        " ",
        None,
        Some(colors[GameSpyColor::MotdHeading as usize]),
    );

    let motd = info.get_motd().as_str().to_string();
    for raw_line in motd.split('\n') {
        let mut line = raw_line.trim_end_matches('\r').trim().to_string();
        if line.is_empty() {
            line = " ".to_string();
        }
        let mut color = colors[GameSpyColor::Motd as usize];
        if line.starts_with("\\\\") {
            line = line[1..].to_string();
        } else if line.starts_with('\\') && line.len() > 9 {
            let a = grab_ubyte(&line[1..]);
            let r = grab_ubyte(&line[3..]);
            let g = grab_ubyte(&line[5..]);
            let b = grab_ubyte(&line[7..]);
            color = make_color(r, g, b, a);
            line = line[9..].to_string();
        }
        list_box.add_item_with_data_and_color(-1, &line, None, Some(color));
    }
}

pub fn handle_num_players_online(num_players_online: i32) {
    let mut state = wol_state().lock().unwrap_or_else(|e| e.into_inner());
    state.last_num_players_online = num_players_online.max(1);
    update_num_players_online(&mut state);
}

fn find_next_number(mut input: &str) -> &str {
    if let Some(pos) = input.find('\n') {
        input = &input[pos..];
    }
    while let Some(ch) = input.chars().next() {
        if ch.is_ascii_digit() {
            break;
        }
        input = &input[ch.len_utf8()..];
    }
    input
}

pub fn handle_overall_stats(stats_text: &str) {
    let mut state = wol_state().lock().unwrap_or_else(|e| e.into_inner());
    let Some(today_idx) = stats_text.find("Today") else {
        return;
    };
    let today_text = &stats_text[today_idx..];
    state.win_stats.clear();
    state.total_win_percent = 0.0;

    let store = get_player_template_store();
    for template in store.iter() {
        if !template.is_playable_side() || template.side.eq_ignore_ascii_case("Boss") {
            continue;
        }
        let mut side = template.side.clone();
        if side.eq_ignore_ascii_case("America") {
            side = "USA".to_string();
        }
        let Some(side_pos) = today_text.find(&side) else {
            continue;
        };
        let side_text = &today_text[side_pos..];
        let total_text = find_next_number(side_text);
        let wins_text = find_next_number(total_text);
        let total_val: f32 = total_text
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .parse()
            .unwrap_or(0.0);
        let wins_val: f32 = wins_text
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .parse()
            .unwrap_or(0.0);
        let percent = if total_val.abs() > f32::EPSILON {
            wins_val / total_val
        } else {
            0.0
        };
        state.total_win_percent += percent;
        state.win_stats.insert(side, percent);
    }
    update_overall_stats(&mut state);
}

fn update_overall_stats(state: &mut WolWelcomeState) {
    if state.total_win_percent <= 0.0 {
        state.total_win_percent = 1.0;
    }
    let total = state.total_win_percent;
    for (side, value) in state.win_stats.clone() {
        let percent = (100.0 * (value / total)).round() as i32;
        let template = GameText::fetch("GUI:WinPercent");
        let text = replace_first(&template, "%d", &percent.to_string());
        let window_name = format!("WOLWelcomeMenu.wnd:Percent{}", side);
        let window_id = name_to_id(&window_name);
        if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(window_id)) {
            let _ = window.borrow_mut().set_text(&text);
        }
    }
}

fn get_additional_disconnects_from_user_file(profile_id: i32) -> i32 {
    if profile_id == 0 {
        return 0;
    }
    let mut prefs = UserPreferences::new();
    let filename = format!("GeneralsOnline/MiscPref{profile_id}.ini");
    let _ = prefs.load_from_file(&filename);
    let mut total = 0;
    for key in ["0", "1", "2", "3", "4", "5"] {
        if let Some(val) = prefs.get_int(key) {
            total += val;
        }
    }
    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        let additional = info.get_additional_disconnects();
        if additional > 0 && total == 0 {
            drop(info);
            if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                info.clear_additional_disconnects();
            }
        }
        if additional != -1 {
            return additional;
        }
    }
    total
}

pub(crate) fn populate_player_info_windows(
    parent_window_name: &str,
    lookup_id: i32,
    look_at_name: &str,
) {
    let Some(queue) = get_ps_message_queue() else {
        return;
    };
    let stats = {
        let queue = queue.lock().ok();
        queue
            .as_ref()
            .map(|queue| queue.find_player_stats_by_id(lookup_id))
            .unwrap_or_default()
    };
    let mut have_stats = stats.id != 0;

    let stats = if !have_stats {
        if let Some(info) = get_gamespy_info() {
            if let Ok(info) = info.lock() {
                let cached = info.get_cached_local_player_stats();
                have_stats = true;
                cached
            } else {
                stats
            }
        } else {
            stats
        }
    } else {
        stats
    };

    let rank_points = calculate_rank(&stats);
    let rank_values = get_rank_point_values();
    let rank_values = rank_values.read().ok();
    let current_rank = rank_values
        .as_ref()
        .map(|values| {
            let mut rank = 0;
            while rank + 1 < values.ranks.len() && rank_points >= values.ranks[rank + 1] {
                rank += 1;
            }
            rank as i32
        })
        .unwrap_or(0);

    let wins: i32 = stats.wins.values().map(|v| *v as i32).sum();
    let losses: i32 = stats.losses.values().map(|v| *v as i32).sum();
    let mut discons: i32 = stats.discons.values().map(|v| *v as i32).sum();
    discons += stats.desyncs.values().map(|v| *v as i32).sum::<i32>();
    discons += get_additional_disconnects_from_user_file(lookup_id);
    let games_played = wins + losses + discons;

    let locale_key = if (LOC_MIN..=LOC_MAX).contains(&stats.locale) {
        format!("WOL:Locale{:02}", stats.locale)
    } else {
        "WOL:Locale00".to_string()
    };
    let header_template = GameText::fetch("GUI:PlayerStatistics");
    let header = replace_first(&header_template, "%s", look_at_name);
    let header = replace_first(&header, "%s", &GameText::fetch(&locale_key));

    let set_text = |name: &str, value: String| {
        let full = format!("{parent_window_name}:{name}");
        let id = name_to_id(&full);
        if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(id)) {
            let _ = window.borrow_mut().set_text(&value);
        }
    };

    set_text("StaticTextPlayerStatisticsLabel", header);
    set_text("StaticTextGamesPlayedValue", games_played.to_string());
    set_text("StaticTextWinsValue", wins.to_string());
    set_text("StaticTextLossesValue", losses.to_string());
    set_text("StaticTextDisconnectsValue", discons.to_string());
    set_text(
        "StaticTextBestStreakValue",
        stats.max_wins_in_a_row.to_string(),
    );

    let streak_label = if stats.losses_in_a_row > 0 {
        GameText::fetch("GUI:CurrentLossStreak")
    } else {
        GameText::fetch("GUI:CurrentWinStreak")
    };
    set_text("StaticTextStreak", streak_label);
    let streak = stats.losses_in_a_row.max(stats.wins_in_a_row);
    set_text("StaticTextStreakValue", streak.to_string());

    let units_killed: i32 = stats.units_killed.values().map(|v| *v as i32).sum();
    let units_lost: i32 = stats.units_lost.values().map(|v| *v as i32).sum();
    let units_built: i32 = stats.units_built.values().map(|v| *v as i32).sum();
    let buildings_killed: i32 = stats.buildings_killed.values().map(|v| *v as i32).sum();
    let buildings_lost: i32 = stats.buildings_lost.values().map(|v| *v as i32).sum();
    let buildings_built: i32 = stats.buildings_built.values().map(|v| *v as i32).sum();

    set_text("StaticTextTotalKillsValue", units_killed.to_string());
    set_text("StaticTextTotalDeathsValue", units_lost.to_string());
    set_text("StaticTextTotalBuiltValue", units_built.to_string());
    set_text(
        "StaticTextBuildingsKilledValue",
        buildings_killed.to_string(),
    );
    set_text("StaticTextBuildingsLostValue", buildings_lost.to_string());
    set_text("StaticTextBuildingsBuiltValue", buildings_built.to_string());

    let win_percent = if games_played > 0 {
        (wins as f32 / games_played as f32 * 100.0).round() as i32
    } else {
        0
    };
    let win_template = GameText::fetch("GUI:WinPercent");
    let win_percent_text = replace_first(&win_template, "%d", &win_percent.to_string());
    set_text("StaticTextWinPercentValue", win_percent_text);

    if let Some(rank_values) = rank_values.as_ref() {
        let progress_name = format!("{parent_window_name}:ProgressBarRank");
        let id = name_to_id(&progress_name);
        if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(id)) {
            if current_rank as usize >= rank_values.ranks.len().saturating_sub(1) {
                let _ = window.borrow_mut().hide(true);
            } else if let Some(progress) = window.borrow_mut().progress_bar_mut() {
                let next_rank = rank_values.ranks[current_rank as usize + 1];
                let current_val = rank_values.ranks[current_rank as usize];
                let denom = (next_rank - current_val).max(1);
                let percent = ((rank_points - current_val) as f32 / denom as f32) * 100.0;
                progress.set_progress(percent.max(0.0).min(100.0));
            }
        }
    }

    let rank_name = format!("GUI:GSRank{}", current_rank);
    set_text("StaticTextRank", GameText::fetch(&rank_name));

    let set_window_image = |name: &str, image_name: &str| {
        if image_name.is_empty() {
            return;
        }
        let full = format!("{parent_window_name}:{name}");
        let id = name_to_id(&full);
        if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(id)) {
            let mut image = Image::with_name(image_name);
            if let Some(collection) =
                crate::display::image::get_mapped_image_collection().try_read()
            {
                if let Some(found) = collection.find_image_by_name(image_name) {
                    image.set_filename(found.get_filename());
                }
            }
            let mut win_guard = window.borrow_mut();
            if win_guard.set_enabled_image(0, image).is_ok() {
                win_guard.set_status(crate::gui::game_window::WindowStatus::IMAGE);
            }
        }
    };

    let favorite_side = get_favorite_side(&stats);
    let favorite_template = if favorite_side >= 0 {
        get_player_template_store()
            .get_nth_player_template(favorite_side as usize)
            .cloned()
    } else {
        None
    };
    let rank_image_name = if rank_points == 0 || favorite_template.is_none() {
        "NewPlayer".to_string()
    } else {
        let mut side_name = favorite_template
            .as_ref()
            .map(|tpl| tpl.base_side.clone())
            .unwrap_or_default();
        if side_name.eq_ignore_ascii_case("USA") {
            side_name = "_USA".to_string();
        } else if side_name.eq_ignore_ascii_case("China") {
            side_name = "_China".to_string();
        } else if side_name.eq_ignore_ascii_case("GLA") {
            side_name = "_GLA".to_string();
        } else if side_name.eq_ignore_ascii_case("Random") {
            side_name = "Elite".to_string();
        }
        let rank_label = match current_rank {
            0 => "Private",
            1 => "Corporal",
            2 => "Sergeant",
            3 => "Lieutenant",
            4 => "Captain",
            5 => "Major",
            6 => "Colonel",
            7 => "Brigadier",
            8 => "General",
            _ => "Commander",
        };
        let mut name = format!("Rank_{}{}", rank_label, side_name);
        if name == "Rank_PrivateElite" {
            name = "Rank".to_string();
        }
        name
    };
    set_window_image("WinRank", &rank_image_name);

    if let Some(template) = favorite_template {
        set_window_image("FactionImage", &template.general_image);
    }

    let in_progress_name = format!("{parent_window_name}:StaticTextInProgress");
    if let Some(window) =
        with_window_manager(|manager| manager.get_window_by_id(name_to_id(&in_progress_name)))
    {
        if have_stats {
            let _ = window.borrow_mut().hide(true);
        } else {
            let _ = window.borrow_mut().hide(false);
            let _ = window
                .borrow_mut()
                .set_text(&GameText::fetch("GUI:FetchingPlayerInfo"));
        }
    }

    populate_battle_honors_list(parent_window_name, &stats);
}

fn ensure_listbox_row(listbox: &mut ListBox, row: usize) {
    while listbox.items().len() <= row {
        listbox.add_item("");
    }
}

fn set_listbox_image(listbox: &mut ListBox, row: usize, column: usize, image_name: &str) {
    ensure_listbox_row(listbox, row);
    let _ = listbox.set_item_column_data(
        row,
        column,
        ListBoxItemData::Image {
            name: image_name.to_string(),
            width: MAX_BATTLE_HONOR_IMAGE_WIDTH,
            height: MAX_BATTLE_HONOR_IMAGE_HEIGHT,
            text: None,
        },
    );
}

fn insert_battle_honor(
    listbox: &mut ListBox,
    image_name: &str,
    enabled: bool,
    row: &mut usize,
    column: &mut usize,
) {
    let image = if enabled { image_name } else { image_name };
    set_listbox_image(listbox, *row, *column, image);
    *column += 1;
    if *column >= MAX_BATTLE_HONOR_COLUMNS as usize {
        *column = 0;
        *row += 1;
    }
}

fn populate_battle_honors_list(
    parent_window_name: &str,
    stats: &game_network::gamespy::persistent_storage_thread::PSPlayerStats,
) {
    if parent_window_name != "PopupPlayerInfo.wnd" {
        return;
    }
    let listbox_id = name_to_id("PopupPlayerInfo.wnd:ListboxInfo");
    let Some(listbox_window) = with_window_manager(|manager| manager.get_window_by_id(listbox_id))
    else {
        return;
    };
    let mut guard = listbox_window.borrow_mut();
    let Some(listbox) = guard.list_box_mut() else {
        return;
    };

    listbox.clear();

    let mut num_games = 0;
    let mut num_discons = 0;
    for val in stats.games.values() {
        num_games += *val as i32;
    }
    for val in stats.discons.values() {
        num_discons += *val as i32;
    }
    for val in stats.desyncs.values() {
        num_discons += *val as i32;
    }
    let is_fair_player = num_games >= 10 && num_discons * 10 < num_games;

    ensure_listbox_row(listbox, 0);
    let mut row = 1usize;
    let mut column = 0usize;
    let honors = stats.battle_honors as u32;

    insert_battle_honor(listbox, "FairPlay", is_fair_player, &mut row, &mut column);
    insert_battle_honor(
        listbox,
        "HonorAirWing",
        (honors & BATTLE_HONOR_AIR_WING) != 0,
        &mut row,
        &mut column,
    );
    insert_battle_honor(
        listbox,
        "HonorBattleTank",
        (honors & BATTLE_HONOR_BATTLE_TANK) != 0,
        &mut row,
        &mut column,
    );
    insert_battle_honor(
        listbox,
        "Apocalypse",
        (honors & BATTLE_HONOR_APOCALYPSE) != 0,
        &mut row,
        &mut column,
    );

    ensure_listbox_row(listbox, 2);
    row = 3;
    column = 0;

    if (honors & BATTLE_HONOR_BLITZ5) != 0 {
        insert_battle_honor(listbox, "HonorBlitz5", true, &mut row, &mut column);
    } else if (honors & BATTLE_HONOR_BLITZ10) != 0 {
        insert_battle_honor(listbox, "HonorBlitz10", true, &mut row, &mut column);
    } else {
        insert_battle_honor(listbox, "HonorBlitz10", false, &mut row, &mut column);
    }

    let streak = stats.wins_in_a_row;
    let streak_image = if streak >= 1000 {
        "HonorStreak_1000"
    } else if streak >= 500 {
        "HonorStreak_500"
    } else if streak >= 100 {
        "HonorStreak_100"
    } else if streak >= 25 {
        "HonorStreak_G"
    } else if streak >= 10 {
        "HonorStreak_S"
    } else {
        "HonorStreak_B"
    };
    insert_battle_honor(listbox, streak_image, streak >= 3, &mut row, &mut column);

    let mut total_wins = 0;
    for val in stats.wins.values() {
        total_wins += *val as i32;
    }
    let domination_image = if total_wins >= 10000 {
        "Domination_10000"
    } else if total_wins >= 1000 {
        "Domination_1000"
    } else if total_wins >= 500 {
        "Domination_500"
    } else {
        "Domination_100"
    };
    insert_battle_honor(
        listbox,
        domination_image,
        total_wins >= 100,
        &mut row,
        &mut column,
    );

    insert_battle_honor(
        listbox,
        "GlobalGen",
        (honors & BATTLE_HONOR_GLOBAL_GENERAL) != 0,
        &mut row,
        &mut column,
    );

    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        if info.did_player_preorder(stats.id) {
            insert_battle_honor(
                listbox,
                "OfficersClub",
                (honors & BATTLE_HONOR_OFFICERSCLUB) != 0,
                &mut row,
                &mut column,
            );
        }
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
    populate_player_info_windows("WOLWelcomeMenu.wnd", lookup_id, &name);
}

fn handle_persistent_storage_responses() {
    let Some(queue) = get_ps_message_queue() else {
        return;
    };
    let resp = {
        let mut queue = match queue.lock() {
            Ok(queue) => queue,
            Err(_) => return,
        };
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
            crate::gamespy_overlay::close_overlay(GameSpyOverlayType::PlayerInfo);
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
                        let mut prefs = GameSpyMiscPreferences::new();
                        req.stats_locale = prefs.get_locale();
                        let wins: i32 = resp.player.wins.values().map(|v| *v as i32).sum();
                        let losses: i32 = resp.player.losses.values().map(|v| *v as i32).sum();
                        req.stats_wins = wins;
                        req.stats_losses = losses;
                        req.stats_rank_points = calculate_rank(&resp.player);
                        let favorite = get_favorite_side(&resp.player);
                        req.stats_side = if favorite < 0 { 0 } else { favorite };
                        req.stats_preorder = info.did_player_preorder(info.get_local_profile_id());
                        if let Some(peer_queue) = get_peer_message_queue() {
                            if let Ok(mut peer_queue) = peer_queue.lock() {
                                peer_queue.add_request(req);
                            }
                        }
                        info.set_cached_local_player_stats(resp.player.clone());
                    }
                }
            }
            if let Some(queue) = get_ps_message_queue() {
                if let Ok(mut queue) = queue.lock() {
                    queue.track_player_stats(resp.player.clone());
                }
            }
            if let Some(info) = get_gamespy_info() {
                if let Ok(info) = info.lock() {
                    if resp.player.id == info.get_local_profile_id() {
                        update_local_player_stats();
                    }
                }
            }
            if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                let wins: i32 = resp.player.wins.values().map(|v| *v as i32).sum();
                let losses: i32 = resp.player.losses.values().map(|v| *v as i32).sum();
                let rank_points = calculate_rank(&resp.player);
                let favorite = get_favorite_side(&resp.player);
                let side = if favorite < 0 { 0 } else { favorite };
                if let Some(updated) =
                    info.update_player_stats(resp.player.id, wins, losses, rank_points, side)
                {
                    let mut response = PeerResponse::default();
                    response.response_type = PeerResponseType::PlayerInfo;
                    response.nick = updated.name.as_str().to_string();
                    response.player_profile_id = updated.profile_id;
                    response.player_flags = updated.flags;
                    response.player_wins = updated.wins;
                    response.player_losses = updated.losses;
                    response.player_rank_points = updated.rank_points;
                    response.player_side = updated.side;
                    response.player_preorder = updated.preorder;
                    response.locale = updated.locale.as_str().to_string();
                    if let Some(peer_queue) = get_peer_message_queue() {
                        if let Ok(mut peer_queue) = peer_queue.lock() {
                            peer_queue.add_response(response);
                        }
                    }
                }
            }
            let (look_id, look_name) = get_look_at_player();
            if look_id > 0 {
                populate_player_info_windows("PopupPlayerInfo.wnd", look_id, &look_name);
            }
        }
    }
}

fn shutdown_complete(layout: &WindowLayout, next_screen: Option<String>) {
    let mut state = wol_state().lock().unwrap_or_else(|e| e.into_inner());
    state.is_shutting_down = false;
    layout.hide(true);
    let mut shell = get_shell();
    let _ = shell.shutdown_complete(Some(layout), next_screen.is_some());
    if let Some(screen) = next_screen {
        let _ = shell.push(&screen, false);
    }
    state.next_screen = None;
}

pub fn wol_welcome_menu_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let mut state = wol_state().lock().unwrap_or_else(|e| e.into_inner());
    state.next_screen = None;
    state.button_pushed = false;
    state.is_shutting_down = false;
    state.raise_message_boxes = true;
    state.parent_id = name_to_id("WOLWelcomeMenu.wnd:WOLWelcomeMenuParent");
    state.button_back_id = name_to_id("WOLWelcomeMenu.wnd:ButtonBack");
    state.button_options_id = name_to_id("WOLWelcomeMenu.wnd:ButtonOptions");
    state.listbox_info_id = name_to_id("WOLWelcomeMenu.wnd:InfoListbox");
    state.button_quick_match_id = name_to_id("WOLWelcomeMenu.wnd:ButtonQuickMatch");
    state.button_lobby_id = name_to_id("WOLWelcomeMenu.wnd:ButtonCustomMatch");
    state.button_buddies_id = name_to_id("WOLWelcomeMenu.wnd:ButtonBuddies");
    state.button_my_info_id = name_to_id("WOLWelcomeMenu.wnd:ButtonMyInfo");
    state.button_ladder_id = name_to_id("WOLWelcomeMenu.wnd:ButtonLadder");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.button_back = manager.get_window_by_id(state.button_back_id);
        state.button_options = manager.get_window_by_id(state.button_options_id);
        state.listbox_info = manager.get_window_by_id(state.listbox_info_id);
    });

    if let Some(parent) = state.parent.as_ref() {
        state.static_text_server_name = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextServerName"));
        state.static_text_last_updated = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextLastUpdated"));
        state.static_text_ladder_wins = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextLadderWins"));
        state.static_text_ladder_losses = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextLadderLosses"));
        state.static_text_ladder_points = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextLadderPoints"));
        state.static_text_ladder_rank = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextLadderRank"));
        state.static_text_ladder_disconnects = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextDisconnects"));
        state.static_text_highscore_wins = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextHighscoreWins"));
        state.static_text_highscore_losses = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextHighscoreLosses"));
        state.static_text_highscore_points = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextHighscorePoints"));
        state.static_text_highscore_rank = parent
            .borrow()
            .find_child_by_id(name_to_id("WOLWelcomeMenu.wnd:StaticTextHighscoreRank"));
        state.button_quick_match = parent
            .borrow()
            .find_child_by_id(state.button_quick_match_id);
        state.button_lobby = parent.borrow().find_child_by_id(state.button_lobby_id);
        state.button_buddies = parent.borrow().find_child_by_id(state.button_buddies_id);
        state.button_my_info = parent.borrow().find_child_by_id(state.button_my_info_id);
        state.button_ladder = parent.borrow().find_child_by_id(state.button_ladder_id);
    }

    if !state.server_name.is_empty() {
        update_server_display(&mut state, &state.server_name);
    }

    if let Some(parent) = state.parent.as_ref() {
        let title_id = name_to_id("WOLWelcomeMenu.wnd:StaticTextTitle");
        if let Some(title) = parent.borrow().find_child_by_id(title_id) {
            if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                let text = GameText::fetch("GUI:WOLWelcome")
                    .replace("%s", info.get_local_base_name().as_str());
                let _ = title.borrow_mut().set_text(&text);
            }
        }
    }

    if let Some(parent) = state.parent.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
    }

    let got_group_room_list = get_gamespy_info()
        .and_then(|info| info.lock().ok().map(|guard| guard.got_group_room_list()))
        .unwrap_or(false);
    enable_controls(&mut state, got_group_room_list);
    get_shell().show_shell_map(true);

    update_num_players_online(&mut state);
    update_overall_stats(&mut state);
    update_local_player_stats();

    let mut prefs = GameSpyMiscPreferences::new();
    if prefs.get_locale() < LOC_MIN || prefs.get_locale() > LOC_MAX {
        open_overlay(GameSpyOverlayType::LocaleSelect);
    }

    with_window_manager(|manager| manager.transition_set_group("WOLWelcomeMenuFade", false));
}

pub fn wol_welcome_menu_shutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);
    {
        let mut state = wol_state().lock().unwrap_or_else(|e| e.into_inner());
        state.listbox_info = None;
        state.is_shutting_down = true;
    }

    if pop_immediate {
        shutdown_complete(layout, None);
        return;
    }

    get_shell().reverse_animate_window();
    with_window_manager(|manager| manager.transition_reverse("WOLWelcomeMenuFade"));
    raise_gs_message_box();
}

pub fn wol_welcome_menu_update(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let mut state = wol_state().lock().unwrap_or_else(|e| e.into_inner());
    let shell_finished = get_shell().is_anim_finished();
    let transitions_finished = with_window_manager(|manager| manager.transitions_finished());
    if state.is_shutting_down && shell_finished && transitions_finished {
        let next = state.next_screen.clone();
        shutdown_complete(layout, next);
        return;
    }

    if state.raise_message_boxes {
        raise_gs_message_box();
        state.raise_message_boxes = false;
    }

    if shell_finished && !state.button_pushed {
        handle_buddy_responses();
        handle_persistent_storage_responses();

        let allowed = GameSpyMiscPreferences::new().get_max_messages_per_update();
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
                        PeerResponseType::GroupRoom => {
                            if let Some(info) = get_gamespy_info() {
                                if let Ok(mut info) = info.lock() {
                                    let room = GameSpyGroupRoom {
                                        name: resp.group_room_name.clone().into(),
                                        translated_name: resp.group_room_name.clone(),
                                        group_id: resp.group_room_id,
                                        num_waiting: resp.group_room_num_waiting,
                                        max_waiting: resp.group_room_max_waiting,
                                        num_games: resp.group_room_num_games,
                                        num_playing: resp.group_room_num_playing,
                                    };
                                    info.add_group_room(room);
                                    if resp.group_room_id == 0 {
                                        enable_controls(&mut state, true);
                                    }
                                }
                            }
                        }
                        PeerResponseType::JoinGroupRoom => {
                            saw_important = true;
                            enable_controls(&mut state, true);
                            if resp.join_group_ok {
                                if let Some(info) = get_gamespy_info() {
                                    if let Ok(mut info) = info.lock() {
                                        info.set_current_group_room(resp.group_room_id);
                                    }
                                }
                                state.button_pushed = true;
                                state.next_screen = Some("Menus/WOLCustomLobby.wnd".to_string());
                                let _ = get_shell().pop();
                            } else {
                                gs_message_box_ok(
                                    &GameText::fetch("GUI:GSErrorTitle"),
                                    &GameText::fetch("GUI:GSGroupRoomJoinFail"),
                                    None,
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
                            crate::gamespy_overlay::close_all_overlays();
                            let _ = get_shell().pop();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

pub fn wol_welcome_menu_input(
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
    let mut state = wol_state().lock().unwrap_or_else(|e| e.into_inner());
    if state.button_pushed {
        return WindowMsgHandled::Handled;
    }
    state.button_pushed = true;
    logout_gamespy();
    let _ = get_shell().pop();
    WindowMsgHandled::Handled
}

fn logout_gamespy() {
    if let Some(queue) = get_peer_message_queue() {
        if let Ok(mut queue) = queue.lock() {
            let mut req = PeerRequest::default();
            req.request_type = PeerRequestType::Logout;
            queue.add_request(req);
        }
    }
    if let Some(queue) = get_buddy_message_queue() {
        if let Ok(mut queue) = queue.lock() {
            let mut req = BuddyRequest::default();
            req.request_type = BuddyRequestType::Logout;
            queue.add_request(req);
        }
    }
    game_network::gamespy::peer_defs::tear_down_gamespy();
}

fn join_best_group_room(state: &mut WolWelcomeState) {
    let info = get_gamespy_info();
    let Some(info) = info else {
        gs_message_box_ok(
            &GameText::fetch("GUI:Error"),
            &GameText::fetch("GUI:GSGroupRoomJoinFail"),
            None,
        );
        return;
    };
    let mut info = match info.lock() {
        Ok(info) => info,
        Err(_) => return,
    };
    if info.get_current_group_room() != 0 {
        info.set_current_group_room(0);
        return;
    }

    let config = game_network::gamespy::config::GameSpyConfig::new_sync();
    let (_, qm_channel) = config.get_qm_config();
    let mut min_id = -1;
    let mut min_players = 1000;
    for room in info.get_group_room_list().values() {
        if room.group_id != qm_channel && min_players > 25 && room.num_waiting < min_players {
            min_id = room.group_id;
            min_players = room.num_waiting;
        }
    }

    if min_id > 0 {
        if let Some(queue) = get_peer_message_queue() {
            if let Ok(mut queue) = queue.lock() {
                let mut req = PeerRequest::default();
                req.request_type = PeerRequestType::JoinGroupRoom;
                req.group_id = min_id;
                queue.add_request(req);
            }
        }
        info.clear_player_info();
        enable_controls(state, false);
    } else {
        gs_message_box_ok(
            &GameText::fetch("GUI:Error"),
            &GameText::fetch("GUI:GSGroupRoomJoinFail"),
            None,
        );
    }
}

pub fn wol_welcome_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            let control_id = data1;
            let mut state = wol_state().lock().unwrap_or_else(|e| e.into_inner());
            if state.button_pushed {
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_back_id {
                state.button_pushed = true;
                logout_gamespy();
                let _ = get_shell().pop();
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_options_id {
                open_overlay(GameSpyOverlayType::Options);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_quick_match_id {
                let prefs = GameSpyMiscPreferences::new();
                let mut user_prefs = UserPreferences::new();
                let width = user_prefs.get_int("graphics_resolution_x").unwrap_or(800);
                let height = user_prefs.get_int("graphics_resolution_y").unwrap_or(600);
                if (width != 800 || height != 600) && prefs.get_quick_match_res_locked() {
                    gs_message_box_ok(
                        &GameText::fetch("GUI:GSErrorTitle"),
                        &GameText::fetch("GUI:QuickMatch800x600"),
                        None,
                    );
                } else {
                    state.button_pushed = true;
                    state.next_screen = Some("Menus/WOLQuickMatchMenu.wnd".to_string());
                    let _ = get_shell().pop();
                }
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_my_info_id {
                if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                    set_look_at_player(info.get_local_profile_id(), info.get_local_name().as_str());
                }
                toggle_overlay(GameSpyOverlayType::PlayerInfo);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_lobby_id {
                join_best_group_room(&mut state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_buddies_id {
                toggle_overlay(GameSpyOverlayType::Buddy);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_ladder_id {
                let _ = get_shell().push("Menus/WOLLadderScreen.wnd", false);
                return WindowMsgHandled::Handled;
            }
        }
        _ => {}
    }
    WindowMsgHandled::Ignored
}
