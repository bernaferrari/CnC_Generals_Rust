//! WOLLoginMenu.cpp callback port.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::game_text::GameText;
use crate::gamespy_overlay::{gs_message_box_ok, raise_gs_message_box};
use crate::gui::gadgets::ComboBoxItem;
use crate::gui::{
    get_shell, with_window_manager, write_input_focus_response, GameWindow, WindowLayout,
    WindowMessage, WindowMsgData, WindowMsgHandled,
};
use crate::shell_hooks::{signal_ui_interaction, SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGIN};
use chrono::Datelike;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::preferences::GameSpyMiscPreferences;
use game_engine::common::system::quoted_printable::{
    ascii_string_to_quoted_printable, quoted_printable_to_ascii_string,
};
use game_network::gamespy::buddy_thread::{
    get_buddy_message_queue, BuddyRequest, BuddyRequestType,
};
use game_network::gamespy::config::GameSpyConfig;
use game_network::gamespy::peer_defs::{
    default_gamespy_colors, get_gamespy_info, GameSpyColor, GameSpyGroupRoom,
};
use game_network::gamespy::peer_thread::{
    get_peer_message_queue, DisconnectReason, PeerRequest, PeerRequestType, PeerResponseType,
};
use game_network::gamespy::persistent_storage_thread::GameSpyPSMessageQueue;
use game_network::gamespy::ping_thread::{get_ping_queue, init_ping_queue, PingRequest};

const LOGIN_TIMEOUT: Duration = Duration::from_millis(10_000);
const PREF_FILENAME: &str = "GameSpyLogin.ini";

#[derive(Default)]
struct WolLoginState {
    parent_id: u32,
    button_back_id: u32,
    button_login_id: u32,
    button_create_account_id: u32,
    button_use_account_id: u32,
    button_dont_use_account_id: u32,
    button_tos_id: u32,
    parent_tos_id: u32,
    button_tos_ok_id: u32,
    listbox_tos_id: u32,
    combo_box_email_id: u32,
    combo_box_login_name_id: u32,
    text_entry_login_name_id: u32,
    text_entry_password_id: u32,
    check_box_remember_password_id: u32,
    text_entry_month_id: u32,
    text_entry_day_id: u32,
    text_entry_year_id: u32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_back: Option<Rc<RefCell<GameWindow>>>,
    button_login: Option<Rc<RefCell<GameWindow>>>,
    button_create_account: Option<Rc<RefCell<GameWindow>>>,
    button_use_account: Option<Rc<RefCell<GameWindow>>>,
    button_dont_use_account: Option<Rc<RefCell<GameWindow>>>,
    button_tos: Option<Rc<RefCell<GameWindow>>>,
    parent_tos: Option<Rc<RefCell<GameWindow>>>,
    button_tos_ok: Option<Rc<RefCell<GameWindow>>>,
    listbox_tos: Option<Rc<RefCell<GameWindow>>>,
    combo_box_email: Option<Rc<RefCell<GameWindow>>>,
    combo_box_login_name: Option<Rc<RefCell<GameWindow>>>,
    text_entry_login_name: Option<Rc<RefCell<GameWindow>>>,
    text_entry_password: Option<Rc<RefCell<GameWindow>>>,
    check_box_remember_password: Option<Rc<RefCell<GameWindow>>>,
    text_entry_month: Option<Rc<RefCell<GameWindow>>>,
    text_entry_day: Option<Rc<RefCell<GameWindow>>>,
    text_entry_year: Option<Rc<RefCell<GameWindow>>>,
    login_pref: Option<GameSpyLoginPreferences>,
    is_shutting_down: bool,
    button_pushed: bool,
    logged_in_ok: bool,
    next_screen: Option<String>,
    login_attempt_time: Option<Instant>,
    web_browser_active: bool,
    use_web_browser_for_tos: bool,
}

static WOL_LOGIN_STATE: OnceLock<Mutex<WolLoginState>> = OnceLock::new();

fn wol_login_state() -> &'static Mutex<WolLoginState> {
    WOL_LOGIN_STATE.get_or_init(|| Mutex::new(WolLoginState::default()))
}

#[derive(Default)]
struct GameSpyLoginPreferences {
    prefs: HashMap<String, String>,
    email_password_map: HashMap<String, String>,
    email_nick_map: HashMap<String, Vec<String>>,
    email_date_map: HashMap<String, String>,
}

impl GameSpyLoginPreferences {
    fn load(&mut self, filename: &str) -> bool {
        let data = match fs::read_to_string(filename) {
            Ok(data) => data,
            Err(_) => return false,
        };
        self.prefs.clear();
        self.email_password_map.clear();
        self.email_nick_map.clear();
        self.email_date_map.clear();

        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let mut parts = line.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();
            if key.is_empty() {
                continue;
            }

            if let Some(email) = key.strip_prefix("pass_") {
                let decoded = quoted_printable_to_ascii_string(value);
                let pass = obfuscate(&decoded);
                self.email_password_map.insert(email.to_string(), pass);
            } else if let Some(email) = key.strip_prefix("date_") {
                let decoded = quoted_printable_to_ascii_string(value);
                self.email_date_map.insert(email.to_string(), decoded);
            } else if let Some(email) = key.strip_prefix("nick_") {
                let nicks = value
                    .split(',')
                    .filter(|nick| !nick.trim().is_empty())
                    .map(|nick| nick.trim().to_string())
                    .collect::<Vec<_>>();
                self.email_nick_map.insert(email.to_string(), nicks);
            } else {
                self.prefs.insert(key.to_string(), value.to_string());
            }
        }
        true
    }

    fn write(&self, filename: &str) -> bool {
        let mut out = String::new();
        let last_email = self.prefs.get("lastEmail").cloned().unwrap_or_default();
        let last_name = self.prefs.get("lastName").cloned().unwrap_or_default();
        let use_profiles = self.prefs.get("useProfiles").cloned().unwrap_or_default();
        out.push_str(&format!("lastEmail = {}\n", last_email));
        out.push_str(&format!("lastName = {}\n", last_name));
        out.push_str(&format!("useProfiles = {}\n", use_profiles));

        for (email, pass) in &self.email_password_map {
            let encoded = ascii_string_to_quoted_printable(&obfuscate(pass));
            out.push_str(&format!("pass_{} = {}\n", email, encoded));
        }
        for (email, date) in &self.email_date_map {
            let encoded = ascii_string_to_quoted_printable(date);
            out.push_str(&format!("date_{} = {}\n", email, encoded));
        }
        for (email, nicks) in &self.email_nick_map {
            let mut list = String::new();
            for nick in nicks {
                list.push_str(nick);
                list.push(',');
            }
            out.push_str(&format!("nick_{} = {}\n", email, list));
        }

        fs::write(filename, out).is_ok()
    }

    fn get_password_for_email(&self, email: &str) -> String {
        self.email_password_map
            .get(email)
            .cloned()
            .unwrap_or_default()
    }

    fn get_date_for_email(&self, email: &str) -> Option<(String, String, String)> {
        let full = self.email_date_map.get(email)?;
        if full.len() != 8 {
            return None;
        }
        let month = full.get(0..2)?.to_string();
        let day = full.get(2..4)?.to_string();
        let year = full.get(4..8)?.to_string();
        Some((month, day, year))
    }

    fn get_nicks_for_email(&self, email: &str) -> Vec<String> {
        self.email_nick_map.get(email).cloned().unwrap_or_default()
    }

    fn add_login(&mut self, email: &str, nick: &str, password: &str, date: &str) {
        let entry = self.email_nick_map.entry(email.to_string()).or_default();
        if !entry.iter().any(|existing| existing == nick) {
            entry.push(nick.to_string());
        }
        self.email_password_map
            .insert(email.to_string(), password.to_string());
        self.email_date_map
            .insert(email.to_string(), date.to_string());
    }

    fn forget_login(&mut self, email: &str) {
        self.email_nick_map.remove(email);
        self.email_password_map.remove(email);
        self.email_date_map.remove(email);
    }

    fn get_emails(&self) -> Vec<String> {
        self.email_nick_map.keys().cloned().collect()
    }

    fn set_pref(&mut self, key: &str, value: String) {
        self.prefs.insert(key.to_string(), value);
    }

    fn get_pref(&self, key: &str) -> Option<String> {
        self.prefs.get(key).cloned()
    }
}

fn obfuscate(input: &str) -> String {
    let mut bytes = input.as_bytes().to_vec();
    let xor = b"1337Munkee";
    let mut idx = 0usize;
    for byte in &mut bytes {
        if idx >= xor.len() {
            idx = 0;
        }
        if *byte != xor[idx] {
            *byte ^= xor[idx];
        }
        idx += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
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
        let data = match fs::read_to_string(filename) {
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

    fn write(&self, filename: &str) -> bool {
        let mut out = String::new();
        for (key, value) in &self.prefs {
            out.push_str(&format!("{key} = {value}\n"));
        }
        fs::write(filename, out).is_ok()
    }

    fn get_bool(&self, key: &str, default: bool) -> bool {
        self.prefs
            .get(key)
            .map(|val| {
                val.eq_ignore_ascii_case("yes") || val == "1" || val.eq_ignore_ascii_case("true")
            })
            .unwrap_or(default)
    }

    fn set_yes(&mut self, key: &str) {
        self.prefs.insert(key.to_string(), "yes".to_string());
    }
}

fn enable_login_controls(state: &WolLoginState, enabled: bool) {
    for window in [
        state.button_login.as_ref(),
        state.button_create_account.as_ref(),
        state.button_use_account.as_ref(),
        state.button_dont_use_account.as_ref(),
        state.combo_box_email.as_ref(),
        state.combo_box_login_name.as_ref(),
        state.text_entry_login_name.as_ref(),
        state.text_entry_password.as_ref(),
        state.check_box_remember_password.as_ref(),
        state.button_tos.as_ref(),
        state.text_entry_month.as_ref(),
        state.text_entry_day.as_ref(),
        state.text_entry_year.as_ref(),
    ] {
        if let Some(window) = window {
            let _ = window.borrow_mut().enable(enabled);
        }
    }
}

fn set_text_entry(window: &Option<Rc<RefCell<GameWindow>>>, value: &str) {
    if let Some(window) = window {
        if let Some(widget) = window.borrow_mut().text_entry_mut() {
            widget.set_text(value);
        }
    }
}

fn combo_box_text(window: &Option<Rc<RefCell<GameWindow>>>) -> String {
    window
        .as_ref()
        .and_then(|combo| {
            combo
                .borrow_mut()
                .combo_box_mut()
                .map(|widget| widget.text().to_string())
        })
        .unwrap_or_default()
}

fn set_combo_box_text(window: &Option<Rc<RefCell<GameWindow>>>, value: &str) {
    if let Some(window) = window {
        if let Some(widget) = window.borrow_mut().combo_box_mut() {
            widget.set_text(value);
        }
    }
}

fn set_combo_box_items(
    window: &Option<Rc<RefCell<GameWindow>>>,
    items: &[String],
    selected: Option<usize>,
) {
    if let Some(window) = window {
        if let Some(widget) = window.borrow_mut().combo_box_mut() {
            widget.clear();
            for (idx, entry) in items.iter().enumerate() {
                widget.add_item(ComboBoxItem::new(idx as u32, entry));
            }
            if let Some(index) = selected {
                let _ = widget.select_index(index);
            }
        }
    }
}

fn set_check_box(window: &Option<Rc<RefCell<GameWindow>>>, checked: bool) {
    if let Some(window) = window {
        if let Some(widget) = window.borrow_mut().check_box_mut() {
            widget.set_checked(checked);
        }
    }
}

fn is_check_box_checked(window: &Option<Rc<RefCell<GameWindow>>>) -> bool {
    window
        .as_ref()
        .and_then(|window| {
            window.borrow().widget().and_then(|widget| match widget {
                crate::gui::WindowWidget::CheckBox(checkbox) => Some(checkbox.is_checked()),
                _ => None,
            })
        })
        .unwrap_or(false)
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

fn show_tos(state: &mut WolLoginState) {
    if let Some(parent_tos) = state.parent_tos.as_ref() {
        let _ = parent_tos.borrow_mut().hide(false);
    }
    // TODO: C++ has useWebBrowserForTOS flag that uses embedded browser for TOS display
    // TODO: C++ shutdown calls TheWebBrowser->closeBrowserWindow(listboxTOS)
    state.use_web_browser_for_tos = false;
    state.web_browser_active = false;

    if let Some(listbox) = state.listbox_tos.as_ref() {
        let mut listbox = listbox.borrow_mut();
        if let Some(widget) = listbox.list_box_mut() {
            widget.clear();
            let mut file_name = String::from("Data/TOS.txt");
            let data = fs::read_to_string(&file_name).or_else(|_| {
                // TODO: C++ uses GetRegistryLanguage() for language-specific TOS path (e.g. Data/German/TOS.txt)
                let lang = GameSpyMiscPreferences::new().get_locale();
                let lang_name = match lang {
                    1 => "German",
                    2 => "French",
                    3 => "Spanish",
                    4 => "Italian",
                    5 => "Japanese",
                    6 => "Korean",
                    7 => "Chinese",
                    8 => "Russian",
                    _ => "English",
                };
                file_name = format!("Data/{}/TOS.txt", lang_name);
                fs::read_to_string(&file_name)
            });
            if let Ok(data) = data {
                let mut content = data.as_str();
                if content.starts_with('\u{feff}') {
                    content = &content[1..];
                }
                let colors = default_gamespy_colors();
                let color = colors[GameSpyColor::Default as usize];
                for line in content.lines() {
                    let trimmed = line.trim_end();
                    if !trimmed.is_empty() {
                        widget.add_item_with_color(trimmed, color);
                    }
                }
            }
        }
    }

    enable_login_controls(state, false);
    if let Some(button_back) = state.button_back.as_ref() {
        let _ = button_back.borrow_mut().enable(false);
    }
}

fn start_pings() {
    let config = GameSpyConfig::new_sync();
    let servers = config.get_ping_servers().to_vec();
    let (reps, timeout_ms, _, _) = config.get_ping_config();
    let queue = init_ping_queue();
    if let Ok(mut queue) = queue.lock() {
        for host in servers {
            queue.add_request(PingRequest {
                hostname: host,
                repetitions: reps,
                timeout_ms,
            });
        }
    }
}

fn disconnect_reason_id(reason: DisconnectReason) -> i32 {
    match reason {
        DisconnectReason::NickTaken => 1,
        DisconnectReason::BadNick => 2,
        DisconnectReason::LostConnection => 3,
        DisconnectReason::CouldNotConnect => 4,
        DisconnectReason::GpLoginTimeout => 5,
        DisconnectReason::GpLoginBadNick => 6,
        DisconnectReason::GpLoginBadEmail => 7,
        DisconnectReason::GpLoginBadPassword => 8,
        DisconnectReason::GpLoginBadProfile => 9,
        DisconnectReason::GpLoginProfileDeleted => 10,
        DisconnectReason::GpLoginConnectionFailed => 11,
        DisconnectReason::GpLoginServerAuthFailed => 12,
        DisconnectReason::SerialInvalid => 13,
        DisconnectReason::SerialNotPresent => 14,
        DisconnectReason::SerialBanned => 15,
        DisconnectReason::GpNewUserBadNick => 16,
        DisconnectReason::GpNewUserBadPassword => 17,
        DisconnectReason::GpNewProfileBadNick => 18,
        DisconnectReason::GpNewProfileBadOldNick => 19,
    }
}

fn reset_gamespy() {
    if let Some(info) = get_gamespy_info() {
        if let Ok(mut info) = info.lock() {
            let motd = info.get_motd();
            let config = info.get_config();
            info.reset();
            info.set_motd(motd);
            info.set_config(config);
        }
    }
    game_network::gamespy::buddy_thread::teardown_buddy_message_queue();
    game_network::gamespy::peer_thread::teardown_peer_message_queue();
    game_network::gamespy::persistent_storage_thread::teardown_ps_message_queue();
    game_network::gamespy::ping_thread::teardown_ping_queue();
    game_network::gamespy::buddy_thread::init_buddy_message_queue();
    game_network::gamespy::peer_thread::init_peer_message_queue();
    game_network::gamespy::persistent_storage_thread::init_ps_message_queue();
    game_network::gamespy::ping_thread::init_ping_queue();
}

fn check_login(state: &mut WolLoginState) {
    let Some(queue) = get_ping_queue() else {
        return;
    };
    let queue = queue.lock().ok();
    let Some(queue) = queue.as_ref() else {
        return;
    };

    if state.logged_in_ok && !queue.are_pings_in_progress() {
        let ping_str = queue.get_ping_string(1000);
        if let Some(info) = get_gamespy_info() {
            if let Ok(mut info) = info.lock() {
                info.set_ping_string(ping_str.into());
                info.clear_group_room_list();
            }
        }

        state.button_pushed = true;
        state.logged_in_ok = false;
        state.login_attempt_time = None;

        signal_ui_interaction(SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGIN);
        state.next_screen = Some("Menus/WOLWelcomeMenu.wnd".to_string());
        let mut shell = get_shell();
        let _ = shell.pop();

        let misc = GameSpyMiscPreferences::new();
        let mut stats = GameSpyPSMessageQueue::parse_player_kv_pairs(&misc.get_cached_stats());
        if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
            stats.id = info.get_local_profile_id();
        }
        if let Some(info) = get_gamespy_info() {
            if let Ok(mut info) = info.lock() {
                info.set_cached_local_player_stats(stats);
            }
        }
    }
}

fn shutdown_complete(
    layout: &WindowLayout,
    state: &mut WolLoginState,
    next_screen: Option<String>,
) {
    state.is_shutting_down = false;
    layout.hide(true);
    let mut shell = get_shell();
    let _ = shell.shutdown_complete(Some(layout), next_screen.is_some());
    if let Some(screen) = next_screen {
        if let Some(pref) = state.login_pref.take() {
            pref.write(PREF_FILENAME);
        }
        let _ = shell.push(&screen, false);
    } else if let Some(pref) = state.login_pref.take() {
        pref.write(PREF_FILENAME);
    }
    state.next_screen = None;
}

pub fn wol_login_menu_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let mut state = wol_login_state().lock().unwrap_or_else(|e| e.into_inner());
    state.next_screen = None;
    state.button_pushed = false;
    state.is_shutting_down = false;
    state.logged_in_ok = false;
    state.login_attempt_time = None;
    state.web_browser_active = false;
    state.use_web_browser_for_tos = false;

    if state.login_pref.is_none() {
        let mut pref = GameSpyLoginPreferences::default();
        let _ = pref.load(PREF_FILENAME);
        state.login_pref = Some(pref);
    }

    let esrb_title_id = NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:StaticTextESRBTop");
    let esrb_parent_id = NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ParentESRB");
    let esrb_title = with_window_manager(|manager| manager.get_window_by_id(esrb_title_id));
    let esrb_parent = with_window_manager(|manager| manager.get_window_by_id(esrb_parent_id));
    if let (Some(title), Some(parent)) = (esrb_title.as_ref(), esrb_parent.as_ref()) {
        let len = title
            .borrow()
            .widget()
            .and_then(|widget| match widget {
                crate::gui::WindowWidget::StaticText(text) => Some(text.text().len()),
                _ => None,
            })
            .unwrap_or(0);
        if len < 2 {
            let _ = parent.borrow_mut().hide(true);
        }
    }

    state.parent_id = NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:WOLLoginMenuParent");
    state.button_back_id = NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ButtonBack");
    state.button_login_id = NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ButtonLogin");
    state.button_create_account_id =
        NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ButtonCreateAccount");
    state.button_use_account_id =
        NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ButtonUseAccount");
    state.button_dont_use_account_id =
        NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ButtonDontUseAccount");
    state.button_tos_id = NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ButtonTOS");
    state.parent_tos_id = NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ParentTOS");
    state.button_tos_ok_id = NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ButtonTOSOK");
    state.listbox_tos_id = NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ListboxTOS");
    state.combo_box_email_id =
        NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ComboBoxEmail");
    state.combo_box_login_name_id =
        NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:ComboBoxLoginName");
    state.text_entry_login_name_id =
        NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:TextEntryLoginName");
    state.text_entry_password_id =
        NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:TextEntryPassword");
    state.check_box_remember_password_id =
        NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:CheckBoxRememberInfo");
    state.text_entry_month_id =
        NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:TextEntryMonth");
    state.text_entry_day_id = NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:TextEntryDay");
    state.text_entry_year_id =
        NameKeyGenerator::name_to_key("GameSpyLoginProfile.wnd:TextEntryYear");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.button_back = manager.get_window_by_id(state.button_back_id);
        state.button_login = manager.get_window_by_id(state.button_login_id);
        state.button_create_account = manager.get_window_by_id(state.button_create_account_id);
        state.button_use_account = manager.get_window_by_id(state.button_use_account_id);
        state.button_dont_use_account = manager.get_window_by_id(state.button_dont_use_account_id);
        state.button_tos = manager.get_window_by_id(state.button_tos_id);
        state.parent_tos = manager.get_window_by_id(state.parent_tos_id);
        state.button_tos_ok = manager.get_window_by_id(state.button_tos_ok_id);
        state.listbox_tos = manager.get_window_by_id(state.listbox_tos_id);
        state.combo_box_email = manager.get_window_by_id(state.combo_box_email_id);
        state.combo_box_login_name = manager.get_window_by_id(state.combo_box_login_name_id);
        state.text_entry_login_name = manager.get_window_by_id(state.text_entry_login_name_id);
        state.text_entry_password = manager.get_window_by_id(state.text_entry_password_id);
        state.check_box_remember_password =
            manager.get_window_by_id(state.check_box_remember_password_id);
        state.text_entry_month = manager.get_window_by_id(state.text_entry_month_id);
        state.text_entry_day = manager.get_window_by_id(state.text_entry_day_id);
        state.text_entry_year = manager.get_window_by_id(state.text_entry_year_id);
    });

    set_text_entry(&state.text_entry_month, "");
    set_text_entry(&state.text_entry_day, "");
    set_text_entry(&state.text_entry_year, "");

    let mut tab_list = Vec::new();
    for window in [
        state.combo_box_email.as_ref(),
        state.combo_box_login_name.as_ref(),
        state.text_entry_password.as_ref(),
        state.text_entry_month.as_ref(),
        state.text_entry_day.as_ref(),
        state.text_entry_year.as_ref(),
        state.check_box_remember_password.as_ref(),
        state.button_login.as_ref(),
        state.button_create_account.as_ref(),
        state.button_tos.as_ref(),
        state.button_back.as_ref(),
    ] {
        if let Some(window) = window {
            tab_list.push(window.clone());
        }
    }
    with_window_manager(|manager| {
        manager.clear_tab_list();
        if !tab_list.is_empty() {
            manager.register_tab_list(tab_list);
        }
    });

    if let Some(combo) = state.combo_box_email.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(combo)));
    }

    if let Some(pref) = state.login_pref.as_ref() {
        let last_name = pref.get_pref("lastName").unwrap_or_default();
        let last_email = pref.get_pref("lastEmail").unwrap_or_default();
        let emails = pref.get_emails();
        let mut selected_email = None;
        for (idx, email) in emails.iter().enumerate() {
            if email == &last_email {
                selected_email = Some(idx);
            }
        }
        set_combo_box_items(&state.combo_box_email, &emails, selected_email);
        set_text_entry(&state.text_entry_password, "");

        if let Some(index) = selected_email {
            if let Some(email) = emails.get(index) {
                let pass = pref.get_password_for_email(email);
                set_text_entry(&state.text_entry_password, &pass);
                if let Some((month, day, year)) = pref.get_date_for_email(email) {
                    set_text_entry(&state.text_entry_month, &month);
                    set_text_entry(&state.text_entry_day, &day);
                    set_text_entry(&state.text_entry_year, &year);
                }
                let nicks = pref.get_nicks_for_email(email);
                let mut selected_nick = None;
                for (idx, nick) in nicks.iter().enumerate() {
                    if nick == &last_name {
                        selected_nick = Some(idx);
                    }
                }
                set_combo_box_items(&state.combo_box_login_name, &nicks, selected_nick);
                set_check_box(&state.check_box_remember_password, true);
            }
        } else {
            set_check_box(&state.check_box_remember_password, false);
        }
    }

    enable_login_controls(&state, true);
    layout.hide(false);
    raise_gs_message_box();

    let mut option_pref = OptionPreferences::new();
    if !option_pref.get_bool("SawTOS", true) {
        show_tos(&mut state);
    }

    with_window_manager(|manager| manager.transition_set_group("GameSpyLoginProfileFade", false));
}

pub fn wol_login_menu_shutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);

    let mut state = wol_login_state().lock().unwrap_or_else(|e| e.into_inner());
    state.is_shutting_down = true;
    state.logged_in_ok = false;
    with_window_manager(|manager| manager.clear_tab_list());

    if state.web_browser_active {
        state.web_browser_active = false;
    }

    if pop_immediate {
        let next = state.next_screen.clone();
        shutdown_complete(layout, &mut state, next);
        return;
    }

    get_shell().reverse_animate_window();
    with_window_manager(|manager| manager.transition_reverse("GameSpyLoginProfileFade"));
}

pub fn wol_login_menu_update(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let mut state = wol_login_state().lock().unwrap_or_else(|e| e.into_inner());

    let shell_finished = get_shell().is_anim_finished();
    let transitions_finished = with_window_manager(|manager| manager.transitions_finished());
    if state.is_shutting_down && shell_finished && transitions_finished {
        let next = state.next_screen.clone();
        shutdown_complete(layout, &mut state, next);
        return;
    }

    if shell_finished && !state.button_pushed {
        if let Some(queue) = get_ping_queue() {
            if let Ok(mut queue) = queue.lock() {
                if queue.get_response().is_some() {
                    check_login(&mut state);
                }
            }
        }

        if let Some(peer_queue) = get_peer_message_queue() {
            if let Ok(mut peer_queue) = peer_queue.lock() {
                if !state.logged_in_ok {
                    if let Some(resp) = peer_queue.get_response() {
                        match resp.response_type {
                            PeerResponseType::GroupRoom => {
                                let room = GameSpyGroupRoom {
                                    name: resp.group_room_name.clone().into(),
                                    translated_name: resp.group_room_name.clone(),
                                    group_id: resp.group_room_id,
                                    num_waiting: resp.group_room_num_waiting,
                                    max_waiting: resp.group_room_max_waiting,
                                    num_games: resp.group_room_num_games,
                                    num_playing: resp.group_room_num_playing,
                                };
                                if let Some(info) = get_gamespy_info() {
                                    if let Ok(mut info) = info.lock() {
                                        info.add_group_room(room);
                                    }
                                }
                            }
                            PeerResponseType::Login => {
                                state.logged_in_ok = true;
                                if let Some(info) = get_gamespy_info() {
                                    if let Ok(mut info) = info.lock() {
                                        info.set_local_name(resp.nick.clone().into());
                                        info.set_local_base_name(resp.nick.clone().into());
                                        info.set_local_profile_id(resp.player_profile_id);
                                        info.load_saved_ignore_list();
                                        info.set_local_ips(
                                            resp.player_internal_ip,
                                            resp.player_external_ip,
                                        );
                                        info.read_additional_disconnects();
                                        let misc = GameSpyMiscPreferences::new();
                                        info.set_max_messages_per_update(
                                            misc.get_max_messages_per_update(),
                                        );
                                    }
                                }
                            }
                            PeerResponseType::Disconnect => {
                                state.login_attempt_time = None;
                                let reason = disconnect_reason_id(resp.discon_reason);
                                let title = GameText::fetch("GUI:GSErrorTitle");
                                let body =
                                    GameText::fetch(&format!("GUI:GSDisconReason{}", reason));
                                gs_message_box_ok(&title, &body, None);
                                enable_login_controls(&state, true);
                                reset_gamespy();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        check_login(&mut state);
    }

    if !state.button_pushed {
        if let Some(started) = state.login_attempt_time {
            if started.elapsed() > LOGIN_TIMEOUT {
                state.login_attempt_time = None;
                let title = GameText::fetch("GUI:GSErrorTitle");
                let body = GameText::fetch("GUI:GSDisconReason4");
                gs_message_box_ok(&title, &body, None);
                enable_login_controls(&state, true);
                reset_gamespy();
            }
        }
    }
}

pub fn wol_login_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char {
        return WindowMsgHandled::Ignored;
    }
    let key = data1 as u32;
    let state_up = data2 & 0x0001;
    if key == 0x1B && state_up != 0 {
        let mut state = wol_login_state().lock().unwrap_or_else(|e| e.into_inner());
        if state.button_pushed {
            return WindowMsgHandled::Handled;
        }
        state.button_pushed = true;
        reset_gamespy();
        let _ = get_shell().pop();
        return WindowMsgHandled::Handled;
    }
    WindowMsgHandled::Ignored
}

fn sanitize_combo_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let trimmed = trimmed.trim_end_matches(['\\', '/']);
    if trimmed != text {
        return Some(trimmed.to_string());
    }
    None
}

fn refresh_for_email_change(state: &mut WolLoginState, email: &str) {
    if let Some(pref) = state.login_pref.as_ref() {
        let pass = pref.get_password_for_email(email);
        set_text_entry(&state.text_entry_password, &pass);

        let nicks = pref.get_nicks_for_email(email);
        set_combo_box_items(
            &state.combo_box_login_name,
            &nicks,
            if nicks.is_empty() { None } else { Some(0) },
        );
        if !nicks.is_empty() {
            set_check_box(&state.check_box_remember_password, true);
            if let Some((month, day, year)) = pref.get_date_for_email(email) {
                set_text_entry(&state.text_entry_month, &month);
                set_text_entry(&state.text_entry_day, &day);
                set_text_entry(&state.text_entry_year, &year);
            }
        } else {
            set_check_box(&state.check_box_remember_password, false);
            set_text_entry(&state.text_entry_month, "");
            set_text_entry(&state.text_entry_day, "");
            set_text_entry(&state.text_entry_year, "");
        }
    }
}

fn is_age_okay(month: &mut String, day: &mut String, year: &str) -> bool {
    if month.is_empty() || day.is_empty() || year.is_empty() || year.len() != 4 {
        return false;
    }
    let month_val: i32 = month.parse().unwrap_or(0);
    let day_val: i32 = day.parse().unwrap_or(0);
    if month_val <= 0 || month_val > 12 || day_val <= 0 || day_val > 31 {
        return false;
    }
    *month = format!("{:02}", month_val);
    *day = format!("{:02}", day_val);

    let year_val: i32 = year.parse().unwrap_or(0);
    let now = chrono::Local::now();
    let current_year = now.year();
    let current_month = now.month() as i32;
    let current_day = now.day() as i32;

    let diff = current_year - year_val;
    if diff >= 14 {
        return true;
    }
    if diff <= 12 {
        return false;
    }

    let user_month = month_val;
    let user_day = day_val;
    if current_month - user_month > 0 {
        return true;
    }
    if current_month - user_month < 0 {
        return false;
    }
    if current_day - user_day < 0 {
        return false;
    }
    true
}

pub fn wol_login_menu_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetValueChanged => {
            let control_id = data1 as u32;
            let mut state = wol_login_state().lock().unwrap_or_else(|e| e.into_inner());
            if state.button_pushed {
                return WindowMsgHandled::Handled;
            }
            if control_id == state.combo_box_email_id {
                let text = combo_box_text(&state.combo_box_email);
                if let Some(sanitized) = sanitize_combo_text(&text) {
                    set_combo_box_text(&state.combo_box_email, &sanitized);
                    return WindowMsgHandled::Handled;
                }
                refresh_for_email_change(&mut state, &text);
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            let mut state = wol_login_state().lock().unwrap_or_else(|e| e.into_inner());
            if state.button_pushed {
                return WindowMsgHandled::Handled;
            }

            if control_id == state.combo_box_email_id {
                let text = combo_box_text(&state.combo_box_email);
                if let Some(sanitized) = sanitize_combo_text(&text) {
                    set_combo_box_text(&state.combo_box_email, &sanitized);
                    return WindowMsgHandled::Handled;
                }
                refresh_for_email_change(&mut state, &text);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_back_id {
                state.button_pushed = true;
                reset_gamespy();
                let _ = get_shell().pop();
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_create_account_id || control_id == state.button_login_id {
                // TODO: ALLOW_NON_PROFILED_LOGIN — C++ has a quick-login path that skips profile creation when this flag is set
                let mut month = text_entry_text(&state.text_entry_month);
                let mut day = text_entry_text(&state.text_entry_day);
                let year = text_entry_text(&state.text_entry_year);

                if !is_age_okay(&mut month, &mut day, &year) {
                    gs_message_box_ok(
                        &GameText::fetch("GUI:AgeFailedTitle"),
                        &GameText::fetch("GUI:AgeFailed"),
                        None,
                    );
                    return WindowMsgHandled::Handled;
                }

                let email = combo_box_text(&state.combo_box_email);
                let login = combo_box_text(&state.combo_box_login_name);
                let password = text_entry_text(&state.text_entry_password);

                if email.is_empty() || login.is_empty() || password.is_empty() {
                    let title = if control_id == state.button_login_id {
                        "GUI:GSErrorTitle"
                    } else {
                        "GUI:Error"
                    };
                    let body = if email.is_empty() && login.is_empty() && password.is_empty() {
                        "GUI:GSNoLoginInfoAll"
                    } else if email.is_empty() && login.is_empty() {
                        "GUI:GSNoLoginInfoEmailNickname"
                    } else if email.is_empty() && password.is_empty() {
                        "GUI:GSNoLoginInfoEmailPassword"
                    } else if login.is_empty() && password.is_empty() {
                        "GUI:GSNoLoginInfoNicknamePassword"
                    } else if email.is_empty() {
                        "GUI:GSNoLoginInfoEmail"
                    } else if password.is_empty() {
                        "GUI:GSNoLoginInfoPassword"
                    } else {
                        "GUI:GSNoLoginInfoNickname"
                    };
                    gs_message_box_ok(&GameText::fetch(title), &GameText::fetch(body), None);
                    return WindowMsgHandled::Handled;
                }

                state.login_attempt_time = Some(Instant::now());
                let mut req = BuddyRequest::default();
                req.request_type = if control_id == state.button_create_account_id {
                    BuddyRequestType::LoginNew
                } else {
                    BuddyRequestType::Login
                };
                req.nick = login.clone();
                req.email = email.clone();
                req.password = password.clone();
                req.has_firewall = true;
                if let Some(queue) = get_buddy_message_queue() {
                    if let Ok(mut queue) = queue.lock() {
                        queue.add_request(req);
                    }
                }

                if let Some(info) = get_gamespy_info() {
                    if let Ok(mut info) = info.lock() {
                        info.set_local_base_name(login.clone().into());
                        info.set_local_email(email.clone().into());
                        info.set_local_password(password.clone().into());
                    }
                }

                if let Some(pref) = state.login_pref.as_mut() {
                    if is_check_box_checked(&state.check_box_remember_password) {
                        pref.set_pref("lastName", login.clone());
                        pref.set_pref("lastEmail", email.clone());
                        pref.set_pref("useProfiles", "yes".to_string());
                        let date = format!("{}{}{}", month, day, year);
                        pref.add_login(&email, &login, &password, &date);
                    } else if control_id == state.button_login_id {
                        pref.forget_login(&email);
                    }
                }

                enable_login_controls(&state, false);
                start_pings();
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_tos_id {
                show_tos(&mut state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_tos_ok_id {
                enable_login_controls(&state, true);
                if let Some(parent_tos) = state.parent_tos.as_ref() {
                    let _ = parent_tos.borrow_mut().hide(true);
                }
                let mut option_pref = OptionPreferences::new();
                option_pref.set_yes("SawTOS");
                let _ = option_pref.write("Options.ini");
                state.web_browser_active = false;
                if let Some(button_back) = state.button_back.as_ref() {
                    let _ = button_back.borrow_mut().enable(true);
                }
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
