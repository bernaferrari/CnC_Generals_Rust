//! NetworkDirectConnect.cpp callback port.

use std::cell::RefCell;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::gui::gadgets::ComboBoxItem;
use crate::gui::{
    get_shell, with_window_manager, GameWindow, LanPreferences, WindowLayout, WindowMessage,
    WindowMsgData, WindowMsgHandled,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_network::lan_api::{LanApi, LanConfig};
use gamelogic::helpers::TheGameText;

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;
const LAN_PLAYER_NAME_LENGTH: usize = 12;

#[derive(Default)]
struct NetworkDirectConnectState {
    parent_id: i32,
    button_back_id: i32,
    button_host_id: i32,
    button_join_id: i32,
    edit_player_name_id: i32,
    combobox_remote_ip_id: i32,
    static_local_ip_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_back: Option<Rc<RefCell<GameWindow>>>,
    button_host: Option<Rc<RefCell<GameWindow>>>,
    button_join: Option<Rc<RefCell<GameWindow>>>,
    edit_player_name: Option<Rc<RefCell<GameWindow>>>,
    combobox_remote_ip: Option<Rc<RefCell<GameWindow>>>,
    static_local_ip: Option<Rc<RefCell<GameWindow>>>,
    button_pushed: bool,
    is_shutting_down: bool,
}

static NETWORK_DIRECT_CONNECT_STATE: OnceLock<Mutex<NetworkDirectConnectState>> = OnceLock::new();
static LAN_API: OnceLock<tokio::sync::Mutex<Option<LanApi>>> = OnceLock::new();

fn network_direct_connect_state() -> &'static Mutex<NetworkDirectConnectState> {
    NETWORK_DIRECT_CONNECT_STATE.get_or_init(|| Mutex::new(NetworkDirectConnectState::default()))
}

fn lan_api_cell() -> &'static tokio::sync::Mutex<Option<LanApi>> {
    LAN_API.get_or_init(|| tokio::sync::Mutex::new(None))
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn local_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    let _ = socket.connect("8.8.8.8:80");
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(addr) => Some(addr),
        _ => None,
    }
}

fn parse_ip(entry: &str) -> Option<Ipv4Addr> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return None;
    }
    let ip_part = trimmed
        .split('(')
        .next()
        .unwrap_or(trimmed)
        .split(':')
        .next()
        .unwrap_or(trimmed)
        .trim();
    ip_part.parse().ok()
}

fn normalize_remote_entry(entry: &str) -> String {
    let trimmed = entry.trim();
    if let Some((ip, rest)) = trimmed.split_once('(') {
        let desc = rest.trim_end_matches(')').trim();
        if desc.is_empty() {
            return ip.trim().to_string();
        }
        return format!("{}:{}", ip.trim(), desc);
    }
    trimmed.to_string()
}

fn populate_remote_ip_combo(state: &NetworkDirectConnectState) {
    let Some(combo) = state.combobox_remote_ip.as_ref() else {
        return;
    };
    let mut combo_guard = combo.borrow_mut();
    let Some(combo_box) = combo_guard.combo_box_mut() else {
        return;
    };
    combo_box.clear();

    let prefs = LanPreferences::new();
    let num_remote_ips = prefs.get_num_remote_ips();
    for idx in 0..num_remote_ips {
        let entry = prefs.get_remote_ip_entry(idx);
        if entry.is_empty() {
            continue;
        }
        combo_box.add_item(ComboBoxItem::new(idx as u32, entry));
    }

    if !combo_box.items().is_empty() {
        let _ = combo_box.select_index(0);
    }
    prefs.write();
}

fn update_remote_ip_list(state: &NetworkDirectConnectState) {
    let Some(combo) = state.combobox_remote_ip.as_ref() else {
        return;
    };
    let mut combo_guard = combo.borrow_mut();
    let Some(combo_box) = combo_guard.combo_box_mut() else {
        return;
    };
    let selected_index = combo_box.selected_index();
    let Some(selected_item) = selected_index.and_then(|idx| combo_box.items().get(idx)) else {
        return;
    };
    let selected_text = selected_item.text.clone();
    let selected_ip = match parse_ip(&selected_text) {
        Some(ip) => ip,
        None => return,
    };

    let mut ordered: Vec<String> = Vec::new();
    let mut seen_ips: Vec<Ipv4Addr> = Vec::new();

    let selected_entry = normalize_remote_entry(&selected_text);
    ordered.push(selected_entry);
    seen_ips.push(selected_ip);

    for (idx, item) in combo_box.items().iter().enumerate() {
        if Some(idx) == selected_index {
            continue;
        }
        let Some(ip) = parse_ip(&item.text) else {
            continue;
        };
        if seen_ips.contains(&ip) {
            continue;
        }
        ordered.push(normalize_remote_entry(&item.text));
        seen_ips.push(ip);
    }

    let mut prefs = LanPreferences::new();
    for (idx, entry) in ordered.iter().enumerate() {
        prefs.set_remote_ip_entry(idx as i32, entry.clone());
    }
    prefs.set_num_remote_ips(ordered.len() as i32);
    prefs.write();
}

fn get_player_name(state: &NetworkDirectConnectState) -> String {
    let Some(entry) = state.edit_player_name.as_ref() else {
        return String::new();
    };
    let guard = entry.borrow();
    let Some(text_entry) = guard.widget().and_then(|widget| match widget {
        crate::gui::WindowWidget::TextEntry(entry) => Some(entry),
        _ => None,
    }) else {
        return String::new();
    };
    text_entry.text().to_string()
}

fn set_player_name_text(state: &NetworkDirectConnectState, name: &str) {
    let Some(entry) = state.edit_player_name.as_ref() else {
        return;
    };
    let mut guard = entry.borrow_mut();
    if let Some(text_entry) = guard.text_entry_mut() {
        text_entry.set_text(name);
    }
}

fn trim_player_name(name: &mut String) {
    while name.chars().count() > LAN_PLAYER_NAME_LENGTH {
        name.pop();
    }
}

fn save_player_name(name: &str) {
    let mut prefs = LanPreferences::new();
    prefs.set_user_name(name.to_string());
    prefs.write();
}

fn update_local_ip_text(state: &NetworkDirectConnectState, local_ip: Ipv4Addr) {
    let Some(label) = state.static_local_ip.as_ref() else {
        return;
    };
    let mut guard = label.borrow_mut();
    if let Some(static_text) = guard.static_text_mut() {
        let text = local_ip.to_string();
        static_text.set_text(text.clone());
        let _ = guard.set_text(&text);
    }
}

async fn ensure_lan_api(player_name: &str) -> Option<tokio::sync::MutexGuard<'_, Option<LanApi>>> {
    let mut guard = lan_api_cell().lock().await;
    if guard.is_none() {
        let mut config = LanConfig::default();
        config.player_name = player_name.to_string();
        config.login_name = player_name.to_string();
        config.host_name = player_name.to_string();
        match LanApi::new(config).await {
            Ok(mut api) => {
                if api.init().await.is_ok() {
                    *guard = Some(api);
                }
            }
            Err(err) => {
                log::warn!("Failed to initialize LAN API: {}", err);
            }
        }
    }
    Some(guard)
}

fn request_set_name(name: String) {
    tokio::spawn(async move {
        let Some(mut guard) = ensure_lan_api(&name).await else {
            return;
        };
        if let Some(api) = guard.as_mut() {
            if let Err(err) = api.request_set_name(name).await {
                log::warn!("Failed to set LAN name: {}", err);
            }
        }
    });
}

fn request_leave_lobby() {
    tokio::spawn(async move {
        let Some(mut guard) = ensure_lan_api("Player").await else {
            return;
        };
        if let Some(api) = guard.as_mut() {
            let _ = api.request_lobby_leave(true).await;
        }
    });
}

fn set_local_ip_for_api(local_ip: Ipv4Addr, name: String) {
    tokio::spawn(async move {
        let Some(mut guard) = ensure_lan_api(&name).await else {
            return;
        };
        if let Some(api) = guard.as_mut() {
            let _ = api.set_local_ip(IpAddr::V4(local_ip)).await;
        }
    });
}

fn host_direct_connect(state: &NetworkDirectConnectState) {
    let mut name = get_player_name(state);
    save_player_name(&name);
    trim_player_name(&mut name);
    request_set_name(name.clone());

    let local_ip = local_ipv4().unwrap_or(Ipv4Addr::LOCALHOST);
    set_local_ip_for_api(local_ip, name.clone());

    tokio::spawn(async move {
        let Some(mut guard) = ensure_lan_api(&name).await else {
            return;
        };
        if let Some(api) = guard.as_mut() {
            let ip_label = local_ip.to_string();
            if let Err(err) = api.request_game_create(ip_label, true).await {
                log::warn!("Failed to host direct connect game: {}", err);
            }
        }
    });
}

fn join_direct_connect(state: &NetworkDirectConnectState) {
    let Some(combo) = state.combobox_remote_ip.as_ref() else {
        return;
    };
    let mut combo_guard = combo.borrow_mut();
    let Some(combo_box) = combo_guard.combo_box_mut() else {
        return;
    };
    let selected_text = combo_box
        .selected_item()
        .map(|item| item.text.clone())
        .unwrap_or_default();
    let Some(ip) = parse_ip(&selected_text) else {
        return;
    };

    let mut name = get_player_name(state);
    save_player_name(&name);
    update_remote_ip_list(state);
    populate_remote_ip_combo(state);
    trim_player_name(&mut name);
    request_set_name(name.clone());

    tokio::spawn(async move {
        let Some(mut guard) = ensure_lan_api(&name).await else {
            return;
        };
        if let Some(api) = guard.as_mut() {
            if let Err(err) = api.request_game_join_direct_connect(IpAddr::V4(ip)).await {
                log::warn!("Failed to join direct connect game: {}", err);
            }
        }
    });
}

fn handle_back(state: &mut NetworkDirectConnectState) {
    let mut name = get_player_name(state);
    save_player_name(&name);
    trim_player_name(&mut name);
    request_set_name(name);
    state.button_pushed = true;
    let _ = get_shell().pop();
}

pub fn network_direct_connect_init(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let mut state = network_direct_connect_state()
        .lock()
        .expect("NetworkDirectConnect state lock poisoned");

    state.button_pushed = false;
    state.is_shutting_down = false;

    state.parent_id = name_to_id("NetworkDirectConnect.wnd:NetworkDirectConnectParent");
    state.button_back_id = name_to_id("NetworkDirectConnect.wnd:ButtonBack");
    state.button_host_id = name_to_id("NetworkDirectConnect.wnd:ButtonHost");
    state.button_join_id = name_to_id("NetworkDirectConnect.wnd:ButtonJoin");
    state.edit_player_name_id = name_to_id("NetworkDirectConnect.wnd:EditPlayerName");
    state.combobox_remote_ip_id = name_to_id("NetworkDirectConnect.wnd:ComboboxRemoteIP");
    state.static_local_ip_id = name_to_id("NetworkDirectConnect.wnd:StaticLocalIP");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.button_back = manager.get_window_by_id(state.button_back_id);
        state.button_host = manager.get_window_by_id(state.button_host_id);
        state.button_join = manager.get_window_by_id(state.button_join_id);
        state.edit_player_name = manager.get_window_by_id(state.edit_player_name_id);
        state.combobox_remote_ip = manager.get_window_by_id(state.combobox_remote_ip_id);
        state.static_local_ip = manager.get_window_by_id(state.static_local_ip_id);
        if let Some(parent) = state.parent.as_ref() {
            let _ = manager.set_focus(Some(parent));
        }
        manager.transition_set_group("NetworkDirectConnectFade", false);
    });

    let prefs = LanPreferences::new();
    let mut name = prefs.get_user_name();
    if name.trim().is_empty() {
        name = TheGameText::fetch("GUI:Player");
    }
    set_player_name_text(&state, &name);

    populate_remote_ip_combo(&state);

    let local_ip = local_ipv4().unwrap_or(Ipv4Addr::LOCALHOST);
    update_local_ip_text(&state, local_ip);
    set_local_ip_for_api(local_ip, name.clone());
    request_leave_lobby();

    get_shell().show_shell_map(true);
    layout.hide(false);
    layout.bring_forward();
}

pub fn network_direct_connect_update(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let mut state = network_direct_connect_state()
        .lock()
        .expect("NetworkDirectConnect state lock poisoned");
    if state.is_shutting_down
        && get_shell().is_anim_finished()
        && with_window_manager(|manager| manager.transitions_finished())
    {
        state.is_shutting_down = false;
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
    }
}

pub fn network_direct_connect_shutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);

    if pop_immediate {
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
        return;
    }

    get_shell().reverse_animate_window();
    with_window_manager(|manager| manager.transition_reverse("NetworkDirectConnectFade"));

    let mut state = network_direct_connect_state()
        .lock()
        .expect("NetworkDirectConnect state lock poisoned");
    state.is_shutting_down = true;
}

pub fn network_direct_connect_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    let mut state = network_direct_connect_state()
        .lock()
        .expect("NetworkDirectConnect state lock poisoned");

    match msg {
        WindowMessage::InputFocus => return WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            if state.button_pushed {
                return WindowMsgHandled::Handled;
            }
            let control_id = data1 as i32;
            if control_id == state.button_back_id {
                handle_back(&mut state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_host_id {
                host_direct_connect(&state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_join_id {
                join_direct_connect(&state);
                return WindowMsgHandled::Handled;
            }
        }
        _ => {}
    }

    WindowMsgHandled::Ignored
}

pub fn network_direct_connect_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char {
        let key = data1 as u32;
        let state = data2 as u32;
        if key == KEY_ESC && (state & KEY_STATE_UP) != 0 {
            let state = network_direct_connect_state()
                .lock()
                .expect("NetworkDirectConnect state lock poisoned");
            if state.button_pushed {
                return WindowMsgHandled::Handled;
            }
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
