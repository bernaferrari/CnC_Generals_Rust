//! PopupLadderSelect.cpp callback port.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::game_text::GameText;
use crate::gamespy_overlay::{close_overlay, GameSpyOverlayType};
use crate::gui::callbacks::popup_host_game::custom_match_hide_host_popup;
use crate::gui::gadgets::{ComboBoxItem, ListBox, ListBoxItemData};
use crate::gui::{
    with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
};
use crate::map_util::get_map_cache_manager;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::preferences::{
    CustomMatchPreferences, LadderPreferences, QuickmatchPreferences,
};
use game_engine::common::system::encrypt::encrypt_string;
use game_network::gamespy::ladder_defs::{get_ladder_list, LadderInfo};
use game_network::gamespy::peer_defs::{
    default_gamespy_colors, get_gamespy_info, make_color, GameSpyColor,
};
use game_network::gamespy::persistent_storage_thread::get_ps_message_queue;

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasswordMode {
    None,
    Entry,
    Error,
}

#[derive(Default)]
struct PopupLadderSelectState {
    parent_id: u32,
    listbox_ladder_select_id: u32,
    listbox_ladder_details_id: u32,
    static_text_ladder_name_id: u32,
    button_ok_id: u32,
    button_cancel_id: u32,
    password_parent_id: u32,
    button_password_ok_id: u32,
    button_password_cancel_id: u32,
    text_entry_password_id: u32,
    bad_password_parent_id: u32,
    button_bad_password_ok_id: u32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    listbox_ladder_select: Option<Rc<RefCell<GameWindow>>>,
    listbox_ladder_details: Option<Rc<RefCell<GameWindow>>>,
    static_text_ladder_name: Option<Rc<RefCell<GameWindow>>>,
    button_ok: Option<Rc<RefCell<GameWindow>>>,
    button_cancel: Option<Rc<RefCell<GameWindow>>>,
    password_parent: Option<Rc<RefCell<GameWindow>>>,
    text_entry_password: Option<Rc<RefCell<GameWindow>>>,
    bad_password_parent: Option<Rc<RefCell<GameWindow>>>,
    password_mode: PasswordMode,
    ladder_index: i32,
}

static POPUP_STATE: OnceLock<Mutex<PopupLadderSelectState>> = OnceLock::new();

fn popup_state() -> &'static Mutex<PopupLadderSelectState> {
    POPUP_STATE.get_or_init(|| Mutex::new(PopupLadderSelectState::default()))
}

fn name_to_id(name: &str) -> u32 {
    NameKeyGenerator::name_to_key(name) as u32
}

fn set_window_enabled(window: &Option<Rc<RefCell<GameWindow>>>, enabled: bool) {
    if let Some(window) = window {
        let _ = window.borrow_mut().enable(enabled);
    }
}

fn set_window_hidden(window: &Option<Rc<RefCell<GameWindow>>>, hidden: bool) {
    if let Some(window) = window {
        let _ = window.borrow_mut().hide(hidden);
    }
}

fn set_text_entry(window: &Option<Rc<RefCell<GameWindow>>>, value: &str) {
    let Some(window) = window else {
        return;
    };
    if let Some(entry) = window.borrow_mut().text_entry_mut() {
        entry.set_text(value);
    }
}

fn text_entry_text(window: &Option<Rc<RefCell<GameWindow>>>) -> String {
    window
        .as_ref()
        .and_then(|window| {
            window.borrow().widget().and_then(|widget| match widget {
                crate::gui::WindowWidget::TextEntry(entry) => Some(entry.text().to_string()),
                _ => None,
            })
        })
        .unwrap_or_default()
}

fn listbox_mut(window: &Option<Rc<RefCell<GameWindow>>>) -> Option<std::cell::RefMut<'_, ListBox>> {
    let window = window.as_ref()?;
    let guard = window.borrow_mut();
    if guard.list_box_mut().is_some() {
        Some(std::cell::RefMut::map(guard, |w| w.list_box_mut().unwrap()))
    } else {
        None
    }
}

fn listbox_selected_ladder_id(window: &Option<Rc<RefCell<GameWindow>>>) -> Option<i32> {
    let window = window.as_ref()?;
    let guard = window.borrow();
    let widget = guard.widget()?;
    if let crate::gui::WindowWidget::ListBox(listbox) = widget {
        let item = listbox.selected_item()?;
        match item.data.as_ref()? {
            ListBoxItemData::Integer(val) => Some(*val),
            _ => None,
        }
    } else {
        None
    }
}

fn set_password_mode(state: &mut PopupLadderSelectState, mode: PasswordMode) {
    state.password_mode = mode;
    match mode {
        PasswordMode::None => {
            set_window_hidden(&state.password_parent, true);
            set_window_hidden(&state.bad_password_parent, true);
            set_window_enabled(&state.button_ok, true);
            set_window_enabled(&state.button_cancel, true);
            set_window_enabled(&state.text_entry_password, false);
            set_window_enabled(&state.listbox_ladder_select, true);
            if let Some(listbox) = state.listbox_ladder_select.as_ref() {
                let _ = with_window_manager(|manager| manager.set_focus(Some(listbox)));
            }
        }
        PasswordMode::Entry => {
            set_window_hidden(&state.password_parent, false);
            set_window_hidden(&state.bad_password_parent, true);
            set_window_enabled(&state.button_ok, false);
            set_window_enabled(&state.button_cancel, false);
            set_window_enabled(&state.text_entry_password, true);
            set_text_entry(&state.text_entry_password, "");
            set_window_enabled(&state.listbox_ladder_select, false);
            if let Some(entry) = state.text_entry_password.as_ref() {
                let _ = with_window_manager(|manager| manager.set_focus(Some(entry)));
            }
        }
        PasswordMode::Error => {
            set_window_hidden(&state.password_parent, true);
            set_window_hidden(&state.bad_password_parent, false);
            set_window_enabled(&state.button_ok, false);
            set_window_enabled(&state.button_cancel, false);
            set_window_enabled(&state.text_entry_password, false);
            set_window_enabled(&state.listbox_ladder_select, false);
            if let Some(parent) = state.parent.as_ref() {
                let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
            }
        }
    }
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

fn to_ascii_string(value: &str) -> AsciiString {
    AsciiString::from(value)
}

fn format_ladder_name_and_size(template: &str, name: &str, size: i32) -> String {
    let mut out = replace_first(template, "%s", name);
    out = replace_first(&out, "%d", &size.to_string());
    out = replace_first(&out, "%d", &size.to_string());
    out
}

fn format_single(template: &str, value: &str) -> String {
    replace_first(template, "%s", value)
}

fn update_ladder_details(
    ladder_id: i32,
    static_text_ladder_name: &Option<Rc<RefCell<GameWindow>>>,
    listbox_ladder_details: &Option<Rc<RefCell<GameWindow>>>,
) {
    let Some(static_text) = static_text_ladder_name.as_ref() else {
        return;
    };
    let Some(listbox_window) = listbox_ladder_details.as_ref() else {
        return;
    };

    let mut listbox_guard = listbox_window.borrow_mut();
    let Some(listbox) = listbox_guard.list_box_mut() else {
        return;
    };
    listbox.clear();
    let _ = static_text.borrow_mut().set_text("");

    let Some(ladder_list) = get_ladder_list() else {
        return;
    };
    let Ok(ladder_list) = ladder_list.read() else {
        return;
    };
    let Some(info) = ladder_list.find_ladder_by_index(ladder_id) else {
        return;
    };

    let color = make_color(255, 255, 255, 255);
    let caption_color = make_color(0, 255, 255, 255);

    let template = GameText::fetch("GUI:LadderNameAndSize");
    let name_line = format_ladder_name_and_size(&template, &info.name, info.players_per_team);
    let _ = static_text.borrow_mut().set_text(&name_line);

    if !info.location.is_empty() {
        listbox.add_item_with_data_and_color(-1, &info.location, None, Some(caption_color));
    }

    // C++ always adds the homepage URL line, even if empty
    let url_template = GameText::fetch("GUI:LadderURL");
    let url_line = format_single(&url_template, info.homepage_url.as_str());
    listbox.add_item_with_data_and_color(-1, &url_line, None, Some(caption_color));

    if !info.description.is_empty() {
        listbox.add_item_with_data_and_color(-1, &info.description, None, Some(color));
    }

    if !info.crypted_password.is_empty() {
        let line = GameText::fetch("GUI:LadderHasPassword");
        listbox.add_item_with_data_and_color(-1, &line, None, Some(caption_color));
    }

    if info.min_wins > 0 {
        let template = GameText::fetch("GUI:LadderMinWins");
        let line = replace_first(&template, "%d", &info.min_wins.to_string());
        listbox.add_item_with_data_and_color(-1, &line, None, Some(caption_color));
    }
    if info.max_wins > 0 {
        let template = GameText::fetch("GUI:LadderMaxWins");
        let line = replace_first(&template, "%d", &info.max_wins.to_string());
        listbox.add_item_with_data_and_color(-1, &line, None, Some(caption_color));
    }

    if info.random_factions {
        let line = GameText::fetch("GUI:LadderRandomFactions");
        listbox.add_item_with_data_and_color(-1, &line, None, Some(caption_color));
    } else {
        let line = GameText::fetch("GUI:LadderFactions");
        listbox.add_item_with_data_and_color(-1, &line, None, Some(caption_color));
    }

    for faction in &info.valid_factions {
        let key = format!("INI:Faction{}", faction.as_str());
        let line = GameText::fetch(&key);
        listbox.add_item_with_data_and_color(-1, &line, None, Some(color));
    }

    if info.random_maps {
        let line = GameText::fetch("GUI:LadderRandomMaps");
        listbox.add_item_with_data_and_color(-1, &line, None, Some(caption_color));
    } else {
        let line = GameText::fetch("GUI:LadderMaps");
        listbox.add_item_with_data_and_color(-1, &line, None, Some(caption_color));
    }

    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    for map_name in &info.valid_maps {
        if let Some(meta) = cache_guard.find_map(map_name.as_str()) {
            let display_name = meta.display_name.to_string();
            listbox.add_item_with_data_and_color(-1, &display_name, None, Some(color));
        }
    }
}

fn ladder_stats_wins() -> i32 {
    let Some(queue) = get_ps_message_queue() else {
        return 0;
    };
    let Some(profile) = get_gamespy_info()
        .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
    else {
        return 0;
    };
    let Ok(queue) = queue.lock() else {
        return 0;
    };
    let stats = queue.find_player_stats_by_id(profile);
    stats.wins.values().sum::<i32>()
}

fn is_valid_qm_ladder(info: &LadderInfo) -> bool {
    if info.index <= 0 || !info.valid_qm {
        return false;
    }
    let wins = ladder_stats_wins();
    if info.max_wins > 0 && info.max_wins < wins {
        return false;
    }
    if info.min_wins > 0 && info.min_wins > wins {
        return false;
    }
    true
}

fn populate_qm_ladder_listbox(listbox: &mut ListBox) -> bool {
    let combo_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxLadder");
    let combo_exists = with_window_manager(|manager| manager.get_window_by_id(combo_id)).is_some();
    if !combo_exists {
        return false;
    }

    let Some(ladder_list) = get_ladder_list() else {
        return false;
    };
    let Ok(ladder_list) = ladder_list.read() else {
        return false;
    };

    listbox.clear();
    let colors = default_gamespy_colors();
    let special_color = colors[GameSpyColor::MapSelected as usize];
    let normal_color = colors[GameSpyColor::MapUnselected as usize];
    let favorite_color = colors[GameSpyColor::MapUnselected as usize];

    let mut used = HashSet::new();
    let mut selected_index = 0usize;

    let no_ladder = GameText::fetch("GUI:NoLadder");
    listbox.add_item_with_data_and_color(
        0,
        &no_ladder,
        Some(ListBoxItemData::Integer(0)),
        Some(normal_color),
    );

    let mut pref = QuickmatchPreferences::new();
    let last_addr = pref.get_last_ladder_addr();
    let last_port = pref.get_last_ladder_port();
    let last_addr_ascii = to_ascii_string(&last_addr);
    if let Some(info) = ladder_list.find_ladder(&last_addr_ascii, last_port) {
        if is_valid_qm_ladder(info) {
            used.insert(info.index);
            let index = listbox.add_item_with_data_and_color(
                info.index,
                &info.name,
                Some(ListBoxItemData::Integer(info.index)),
                Some(favorite_color),
            );
            selected_index = index;
        }
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
        let addr = to_ascii_string(&pref.address);
        if let Some(info) = ladder_list.find_ladder(&addr, pref.port) {
            if is_valid_qm_ladder(info) && !used.contains(&info.index) {
                used.insert(info.index);
                listbox.add_item_with_data_and_color(
                    info.index,
                    &info.name,
                    Some(ListBoxItemData::Integer(info.index)),
                    Some(favorite_color),
                );
            }
        }
    }

    for info in ladder_list.get_special_ladders() {
        if is_valid_qm_ladder(info) && !used.contains(&info.index) {
            used.insert(info.index);
            listbox.add_item_with_data_and_color(
                info.index,
                &info.name,
                Some(ListBoxItemData::Integer(info.index)),
                Some(special_color),
            );
        }
    }

    for info in ladder_list.get_standard_ladders() {
        if is_valid_qm_ladder(info) && !used.contains(&info.index) {
            used.insert(info.index);
            listbox.add_item_with_data_and_color(
                info.index,
                &info.name,
                Some(ListBoxItemData::Integer(info.index)),
                Some(normal_color),
            );
        }
    }

    if selected_index < listbox.items().len() {
        listbox.set_selected_indices(&[selected_index]);
    }
    true
}

fn populate_custom_ladder_listbox(listbox: &mut ListBox) -> bool {
    let parent_id = name_to_id("PopupHostGame.wnd:ParentHostPopUp");
    let parent_exists =
        with_window_manager(|manager| manager.get_window_by_id(parent_id)).is_some();
    if !parent_exists {
        return false;
    }

    let Some(ladder_list) = get_ladder_list() else {
        return false;
    };
    let Ok(ladder_list) = ladder_list.read() else {
        return false;
    };

    listbox.clear();
    let colors = default_gamespy_colors();
    let special_color = colors[GameSpyColor::MapSelected as usize];
    let normal_color = colors[GameSpyColor::MapUnselected as usize];
    let favorite_color = colors[GameSpyColor::MapUnselected as usize];
    let local_color = colors[GameSpyColor::MapUnselected as usize];

    let mut used = HashSet::new();
    let mut selected_index = 0usize;

    let no_ladder = GameText::fetch("GUI:NoLadder");
    listbox.add_item_with_data_and_color(
        0,
        &no_ladder,
        Some(ListBoxItemData::Integer(0)),
        Some(normal_color),
    );

    let pref = CustomMatchPreferences::new();
    let last_addr = pref.get_last_ladder_addr();
    let last_port = pref.get_last_ladder_port();
    let last_addr_ascii = to_ascii_string(&last_addr);
    if let Some(info) = ladder_list.find_ladder(&last_addr_ascii, last_port) {
        if info.index > 0 && info.valid_custom {
            used.insert(info.index);
            let index = listbox.add_item_with_data_and_color(
                info.index,
                &info.name,
                Some(ListBoxItemData::Integer(info.index)),
                Some(favorite_color),
            );
            selected_index = index;
        }
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
        let addr = to_ascii_string(&pref.address);
        if let Some(info) = ladder_list.find_ladder(&addr, pref.port) {
            if info.index > 0 && info.valid_custom && !used.contains(&info.index) {
                used.insert(info.index);
                listbox.add_item_with_data_and_color(
                    info.index,
                    &info.name,
                    Some(ListBoxItemData::Integer(info.index)),
                    Some(favorite_color),
                );
            }
        }
    }

    for info in ladder_list.get_local_ladders() {
        if info.index < 0 && info.valid_custom && !used.contains(&info.index) {
            used.insert(info.index);
            listbox.add_item_with_data_and_color(
                info.index,
                &info.name,
                Some(ListBoxItemData::Integer(info.index)),
                Some(local_color),
            );
        }
    }

    for info in ladder_list.get_special_ladders() {
        if info.index > 0 && info.valid_custom && !used.contains(&info.index) {
            used.insert(info.index);
            listbox.add_item_with_data_and_color(
                info.index,
                &info.name,
                Some(ListBoxItemData::Integer(info.index)),
                Some(special_color),
            );
        }
    }

    for info in ladder_list.get_standard_ladders() {
        if info.index > 0 && info.valid_custom && !used.contains(&info.index) {
            used.insert(info.index);
            listbox.add_item_with_data_and_color(
                info.index,
                &info.name,
                Some(ListBoxItemData::Integer(info.index)),
                Some(normal_color),
            );
        }
    }

    if selected_index < listbox.items().len() {
        listbox.set_selected_indices(&[selected_index]);
    }
    true
}

fn populate_ladder_listbox(state: &mut PopupLadderSelectState) {
    {
        let Some(mut listbox) = listbox_mut(&state.listbox_ladder_select) else {
            return;
        };
        if populate_qm_ladder_listbox(&mut listbox) {
            return;
        }
        let _ = populate_custom_ladder_listbox(&mut listbox);
    }

    let ladder_id = listbox_selected_ladder_id(&state.listbox_ladder_select);
    if let Some(ladder_id) = ladder_id {
        update_ladder_details(
            ladder_id,
            &state.static_text_ladder_name,
            &state.listbox_ladder_details,
        );
    }
}

fn populate_qm_ladder_combo_box() -> bool {
    let combo_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxLadder");
    let combo = with_window_manager(|manager| manager.get_window_by_id(combo_id));
    let Some(combo) = combo else {
        return false;
    };

    let mut combo_guard = combo.borrow_mut();
    let Some(combo_box) = combo_guard.combo_box_mut() else {
        return false;
    };

    let Some(ladder_list) = get_ladder_list() else {
        return false;
    };
    let Ok(ladder_list) = ladder_list.read() else {
        return false;
    };

    combo_box.clear();
    let no_ladder = GameText::fetch("GUI:NoLadder");
    combo_box.add_item(ComboBoxItem::new(0, &no_ladder).with_data(0));

    let mut used = HashSet::new();
    let mut selected_index = 0usize;

    let pref = QuickmatchPreferences::new();
    let last_addr = pref.get_last_ladder_addr();
    let last_port = pref.get_last_ladder_port();
    let last_addr_ascii = to_ascii_string(&last_addr);
    let mut ladder_selected = false;
    if let Some(info) = ladder_list.find_ladder(&last_addr_ascii, last_port) {
        if is_valid_qm_ladder(info) {
            used.insert(info.index);
            combo_box
                .add_item(ComboBoxItem::new(info.index as u32, &info.name).with_data(info.index));
            selected_index = combo_box.items().len().saturating_sub(1);
            ladder_selected = true;

            let num_players_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxNumPlayers");
            if let Some(num_players) =
                with_window_manager(|manager| manager.get_window_by_id(num_players_id))
            {
                if let Some(num_box) = num_players.borrow_mut().combo_box_mut() {
                    let target = info.players_per_team.saturating_sub(1).max(0) as usize;
                    let _ = num_box.select_index(target);
                }
                let _ = num_players.borrow_mut().enable(false);
            }
        }
    }
    if !ladder_selected {
        let num_players_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxNumPlayers");
        if let Some(num_players) =
            with_window_manager(|manager| manager.get_window_by_id(num_players_id))
        {
            let _ = num_players.borrow_mut().enable(true);
        }
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
        let addr = to_ascii_string(&pref.address);
        if let Some(info) = ladder_list.find_ladder(&addr, pref.port) {
            if is_valid_qm_ladder(info) && !used.contains(&info.index) {
                used.insert(info.index);
                combo_box.add_item(
                    ComboBoxItem::new(info.index as u32, &info.name).with_data(info.index),
                );
            }
        }
    }

    for info in ladder_list.get_special_ladders() {
        if is_valid_qm_ladder(info) && !used.contains(&info.index) {
            used.insert(info.index);
            combo_box
                .add_item(ComboBoxItem::new(info.index as u32, &info.name).with_data(info.index));
        }
    }

    for info in ladder_list.get_standard_ladders() {
        if is_valid_qm_ladder(info) && !used.contains(&info.index) {
            used.insert(info.index);
            combo_box
                .add_item(ComboBoxItem::new(info.index as u32, &info.name).with_data(info.index));
        }
    }

    let choose = GameText::fetch("GUI:ChooseLadder");
    combo_box.add_item(ComboBoxItem::new(0xFFFF_FFFF, &choose).with_data(-1));
    let _ = combo_box.select_index(selected_index);

    true
}

fn populate_custom_ladder_combo_box() -> bool {
    let combo_id = name_to_id("PopupHostGame.wnd:ComboBoxLadderName");
    let combo = with_window_manager(|manager| manager.get_window_by_id(combo_id));
    let Some(combo) = combo else {
        return false;
    };

    let mut combo_guard = combo.borrow_mut();
    let Some(combo_box) = combo_guard.combo_box_mut() else {
        return false;
    };

    let Some(ladder_list) = get_ladder_list() else {
        return false;
    };
    let Ok(ladder_list) = ladder_list.read() else {
        return false;
    };

    combo_box.clear();
    let no_ladder = GameText::fetch("GUI:NoLadder");
    combo_box.add_item(ComboBoxItem::new(0, &no_ladder).with_data(0));

    let mut used = HashSet::new();
    let mut selected_index = 0usize;

    let pref = CustomMatchPreferences::new();
    let last_addr = pref.get_last_ladder_addr();
    let last_port = pref.get_last_ladder_port();
    let last_addr_ascii = to_ascii_string(&last_addr);
    if let Some(info) = ladder_list.find_ladder(&last_addr_ascii, last_port) {
        if info.valid_custom {
            used.insert(info.index);
            combo_box
                .add_item(ComboBoxItem::new(info.index as u32, &info.name).with_data(info.index));
            selected_index = combo_box.items().len().saturating_sub(1);
        }
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
        let addr = to_ascii_string(&pref.address);
        if let Some(info) = ladder_list.find_ladder(&addr, pref.port) {
            if info.valid_custom && !used.contains(&info.index) {
                used.insert(info.index);
                combo_box.add_item(
                    ComboBoxItem::new(info.index as u32, &info.name).with_data(info.index),
                );
            }
        }
    }

    let choose = GameText::fetch("GUI:ChooseLadder");
    combo_box.add_item(ComboBoxItem::new(0xFFFF_FFFF, &choose).with_data(-1));
    let _ = combo_box.select_index(selected_index);

    true
}

fn populate_ladder_combo_box() {
    if populate_qm_ladder_combo_box() {
        return;
    }
    let _ = populate_custom_ladder_combo_box();
}

fn handle_qm_ladder_selection(ladder_id: i32) -> bool {
    let combo_id = name_to_id("WOLQuickMatchMenu.wnd:ComboBoxLadder");
    let combo_exists = with_window_manager(|manager| manager.get_window_by_id(combo_id)).is_some();
    if !combo_exists {
        return false;
    }

    let mut pref = QuickmatchPreferences::new();
    if ladder_id < 1 {
        pref.set_last_ladder("", 0);
        pref.write();
        return true;
    }

    let Some(ladder_list) = get_ladder_list() else {
        return true;
    };
    let Ok(ladder_list) = ladder_list.read() else {
        return true;
    };
    if let Some(info) = ladder_list.find_ladder_by_index(ladder_id) {
        pref.set_last_ladder(info.address.as_str(), info.port);
    } else {
        pref.set_last_ladder("", 0);
    }
    pref.write();
    true
}

fn handle_custom_ladder_selection(ladder_id: i32) -> bool {
    let parent_id = name_to_id("PopupHostGame.wnd:ParentHostPopUp");
    let parent_exists =
        with_window_manager(|manager| manager.get_window_by_id(parent_id)).is_some();
    if !parent_exists {
        return false;
    }

    let mut pref = CustomMatchPreferences::new();
    if ladder_id == 0 {
        pref.set_last_ladder("", 0);
        pref.write();
        return true;
    }

    let Some(ladder_list) = get_ladder_list() else {
        return true;
    };
    let Ok(ladder_list) = ladder_list.read() else {
        return true;
    };
    if let Some(info) = ladder_list.find_ladder_by_index(ladder_id) {
        pref.set_last_ladder(info.address.as_str(), info.port);
    } else {
        pref.set_last_ladder("", 0);
    }
    pref.write();
    true
}

fn handle_ladder_selection(ladder_id: i32) {
    if handle_qm_ladder_selection(ladder_id) {
        return;
    }
    let _ = handle_custom_ladder_selection(ladder_id);
}

pub fn popup_ladder_select_init(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let mut state = popup_state().lock().unwrap_or_else(|e| e.into_inner());

    state.parent_id = name_to_id("PopupLadderSelect.wnd:Parent");
    state.listbox_ladder_select_id = name_to_id("PopupLadderSelect.wnd:ListBoxLadderSelect");
    state.listbox_ladder_details_id = name_to_id("PopupLadderSelect.wnd:ListBoxLadderDetails");
    state.static_text_ladder_name_id = name_to_id("PopupLadderSelect.wnd:StaticTextLadderName");
    state.button_ok_id = name_to_id("PopupLadderSelect.wnd:ButtonOk");
    state.button_cancel_id = name_to_id("PopupLadderSelect.wnd:ButtonCancel");

    state.password_parent_id = name_to_id("PopupLadderSelect.wnd:PasswordParent");
    state.button_password_ok_id = name_to_id("PopupLadderSelect.wnd:ButtonPasswordOk");
    state.button_password_cancel_id = name_to_id("PopupLadderSelect.wnd:ButtonPasswordCancel");
    state.text_entry_password_id = name_to_id("PopupLadderSelect.wnd:PasswordEntry");

    state.bad_password_parent_id = name_to_id("PopupLadderSelect.wnd:BadPasswordParent");
    state.button_bad_password_ok_id = name_to_id("PopupLadderSelect.wnd:ButtonBadPasswordOk");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.listbox_ladder_select = manager.get_window_by_id(state.listbox_ladder_select_id);
        state.listbox_ladder_details = manager.get_window_by_id(state.listbox_ladder_details_id);
        state.static_text_ladder_name = manager.get_window_by_id(state.static_text_ladder_name_id);
        state.button_ok = manager.get_window_by_id(state.button_ok_id);
        state.button_cancel = manager.get_window_by_id(state.button_cancel_id);
        state.password_parent = manager.get_window_by_id(state.password_parent_id);
        state.text_entry_password = manager.get_window_by_id(state.text_entry_password_id);
        state.bad_password_parent = manager.get_window_by_id(state.bad_password_parent_id);
    });

    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(parent));
            let _ = manager.set_modal(parent.clone());
        });
    }

    set_password_mode(&mut state, PasswordMode::None);
    custom_match_hide_host_popup(true);
    populate_ladder_listbox(&mut state);
}

pub fn popup_ladder_select_update(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

pub fn popup_ladder_select_shutdown(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let mut state = popup_state().lock().unwrap_or_else(|e| e.into_inner());
    state.parent = None;
    state.listbox_ladder_select = None;
    state.listbox_ladder_details = None;
    state.static_text_ladder_name = None;
    state.button_ok = None;
    state.button_cancel = None;
    state.password_parent = None;
    state.text_entry_password = None;
    state.bad_password_parent = None;
}

pub fn popup_ladder_select_input(
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

    let mut state = popup_state().lock().unwrap_or_else(|e| e.into_inner());
    match state.password_mode {
        PasswordMode::None => {
            populate_ladder_combo_box();
            close_overlay(GameSpyOverlayType::LadderSelect);
        }
        PasswordMode::Entry | PasswordMode::Error => {
            set_password_mode(&mut state, PasswordMode::None);
        }
    }
    WindowMsgHandled::Handled
}

fn ladder_selected_callback(state: &mut PopupLadderSelectState) {
    handle_ladder_selection(state.ladder_index);
    populate_ladder_combo_box();
    close_overlay(GameSpyOverlayType::LadderSelect);
}

pub fn popup_ladder_select_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let mut state = popup_state().lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => {
            custom_match_hide_host_popup(false);
            state.parent = None;
            state.listbox_ladder_select = None;
            state.listbox_ladder_details = None;
            WindowMsgHandled::Handled
        }
        WindowMessage::InputFocus => {
            // TODO: C++ writes back to mData2 (data2) to indicate focus state;
            // Rust uses values not pointers for WindowMsgData so write-back is not
            // possible without API changes. Preserve this as a parity note.
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            if control_id == state.button_ok_id {
                if let Some(ladder_id) = listbox_selected_ladder_id(&state.listbox_ladder_select) {
                    state.ladder_index = ladder_id;
                    let ladder_list = get_ladder_list();
                    let has_password = ladder_list
                        .and_then(|list| list.read().ok())
                        .and_then(|list| list.find_ladder_by_index(ladder_id))
                        .map(|info| !info.crypted_password.is_empty())
                        .unwrap_or(false);
                    if has_password {
                        set_password_mode(&mut state, PasswordMode::Entry);
                    } else {
                        ladder_selected_callback(&mut state);
                    }
                }
            } else if control_id == state.button_cancel_id {
                populate_ladder_combo_box();
                close_overlay(GameSpyOverlayType::LadderSelect);
            } else if control_id == state.button_password_ok_id {
                let ladder_list = get_ladder_list();
                let info = ladder_list
                    .and_then(|list| list.read().ok())
                    .and_then(|list| list.find_ladder_by_index(state.ladder_index));
                let Some(info) = info else {
                    set_password_mode(&mut state, PasswordMode::Error);
                    return WindowMsgHandled::Handled;
                };
                if info.crypted_password.is_empty() {
                    set_password_mode(&mut state, PasswordMode::Error);
                    return WindowMsgHandled::Handled;
                }
                let pass = text_entry_text(&state.text_entry_password);
                if pass.is_empty() {
                    set_password_mode(&mut state, PasswordMode::Error);
                } else {
                    let crypted = encrypt_string(&pass);
                    if crypted == info.crypted_password.as_str() {
                        ladder_selected_callback(&mut state);
                    } else {
                        set_password_mode(&mut state, PasswordMode::Error);
                    }
                }
            } else if control_id == state.button_password_cancel_id {
                set_password_mode(&mut state, PasswordMode::None);
            } else if control_id == state.button_bad_password_ok_id {
                set_password_mode(&mut state, PasswordMode::None);
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetValueChanged => {
            let ladder_id = listbox_selected_ladder_id(&state.listbox_ladder_select);
            if let Some(ladder_id) = ladder_id {
                update_ladder_details(
                    ladder_id,
                    &state.static_text_ladder_name,
                    &state.listbox_ladder_details,
                );
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::User(0x8000) => {
            if data1 as u32 == state.listbox_ladder_select_id {
                if let Some(parent) = state.parent.as_ref() {
                    let _ = parent.borrow_mut().send_system_message(
                        WindowMessage::GadgetSelected,
                        state.button_ok_id,
                        state.button_ok_id,
                    );
                }
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => {
            if data1 as u32 == state.text_entry_password_id {
                if let Some(parent) = state.parent.as_ref() {
                    let _ = parent.borrow_mut().send_system_message(
                        WindowMessage::GadgetSelected,
                        state.button_password_ok_id,
                        state.button_password_ok_id,
                    );
                }
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

pub fn rc_game_details_menu_init(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

pub fn rc_game_details_menu_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        // TODO: C++ handles GWM_CLOSE (WindowMessage::Close) in the RC game details menu
        // system callback. The Rust WindowMessage enum does not yet have a Close variant.
        // Add handling when Close is added to the enum.
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            let ladder_button = name_to_id("RCGameDetailsMenu.wnd:ButtonLadderDetails");
            if control_id != ladder_button {
                return WindowMsgHandled::Handled;
            }

            let selected_id = window.get_user_data::<i32>().copied().unwrap_or(0);
            if selected_id == 0 {
                return WindowMsgHandled::Handled;
            }

            if let Some(layout) = window.get_layout() {
                with_window_manager(|manager| manager.destroy_layout(&layout));
            }

            let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) else {
                return WindowMsgHandled::Handled;
            };
            let Some(room) = info.find_staging_room_by_id(selected_id) else {
                return WindowMsgHandled::Handled;
            };

            let Some(ladder_list) = get_ladder_list() else {
                return WindowMsgHandled::Handled;
            };
            let Ok(ladder_list) = ladder_list.read() else {
                return WindowMsgHandled::Handled;
            };
            let Some(info) = ladder_list.find_ladder(&room.ladder_ip, room.ladder_port) else {
                return WindowMsgHandled::Handled;
            };
            if info.address.is_empty() || info.port == 0 {
                return WindowMsgHandled::Handled;
            }

            let layout = with_window_manager(|manager| {
                manager
                    .create_layout_with_windows("Menus/PopupLadderDetails.wnd")
                    .ok()
                    .map(|(layout, _)| layout)
            });
            let Some(layout) = layout else {
                return WindowMsgHandled::Handled;
            };
            layout.borrow().run_init(None);
            let first_window = layout.borrow().get_first_window();
            if let Some(first) = first_window {
                first.borrow_mut().set_user_data(selected_id);
                // TODO: C++ calls TheWindowManager->setLoneWindow(first) here to make
                // this the exclusive lone window. Add call when set_lone_window is available
                // in the window manager API.
                let _ = first.borrow_mut().hide(false);
                first.borrow_mut().bring_to_top();
            }

            let static_text_id = name_to_id("PopupLadderDetails.wnd:StaticTextLadderName");
            let listbox_id = name_to_id("PopupLadderDetails.wnd:ListBoxLadderDetails");
            let static_text =
                with_window_manager(|manager| manager.get_window_by_id(static_text_id));
            let listbox = with_window_manager(|manager| manager.get_window_by_id(listbox_id));
            update_ladder_details(info.index, &static_text, &listbox);

            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
