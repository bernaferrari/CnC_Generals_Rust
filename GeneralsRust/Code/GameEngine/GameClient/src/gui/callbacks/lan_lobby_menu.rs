//! LanLobbyMenu.cpp — discover, join, host, and chat on TheLAN.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::game_text::GameText;
use crate::gui::callbacks::{
    lan_button_pushed, lan_is_shutting_down, set_lan_button_pushed, set_lan_is_shutting_down,
};
use crate::gui::gadgets::ListBoxItemData;
use crate::gui::{
    GLM_DOUBLE_CLICKED, GameWindow, LanPreferences, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, get_shell, show_shell_map_if_available, try_with_shell_mut,
    with_window_manager, write_input_focus_response,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_network::lan_api::{
    ChatType, LanEvent, LanGameInfo, LanResult, ensure_the_lan, reset_the_lan, the_lan,
};
use log::warn;

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const LAN_PLAYER_NAME_LENGTH: usize = 12;

#[derive(Default)]
struct LanLobbyState {
    parent_id: i32,
    button_back_id: i32,
    button_clear_id: i32,
    button_host_id: i32,
    button_join_id: i32,
    button_direct_connect_id: i32,
    button_emote_id: i32,
    text_entry_player_name_id: i32,
    text_entry_chat_id: i32,
    listbox_players_id: i32,
    listbox_chat_id: i32,
    listbox_games_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_back: Option<Rc<RefCell<GameWindow>>>,
    button_clear: Option<Rc<RefCell<GameWindow>>>,
    button_host: Option<Rc<RefCell<GameWindow>>>,
    button_join: Option<Rc<RefCell<GameWindow>>>,
    button_direct_connect: Option<Rc<RefCell<GameWindow>>>,
    button_emote: Option<Rc<RefCell<GameWindow>>>,
    text_entry_player_name: Option<Rc<RefCell<GameWindow>>>,
    text_entry_chat: Option<Rc<RefCell<GameWindow>>>,
    listbox_players: Option<Rc<RefCell<GameWindow>>>,
    listbox_chat: Option<Rc<RefCell<GameWindow>>>,
    listbox_games: Option<Rc<RefCell<GameWindow>>>,
    default_name: String,
    just_entered: bool,
    initial_gadget_delay: i32,
    next_screen: Option<String>,
    socket_error: bool,
}

thread_local! {
    static STATE: Rc<RefCell<LanLobbyState>> = Rc::new(RefCell::new(LanLobbyState::default()));
}

static LAN_EVENTS: OnceLock<Mutex<VecDeque<LanEvent>>> = OnceLock::new();

fn events() -> &'static Mutex<VecDeque<LanEvent>> {
    LAN_EVENTS.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn state() -> Rc<RefCell<LanLobbyState>> {
    STATE.with(Rc::clone)
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn local_ipv4() -> Ipv4Addr {
    let socket = UdpSocket::bind("0.0.0.0:0").ok();
    if let Some(socket) = socket {
        let _ = socket.connect("8.8.8.8:80");
        if let Ok(addr) = socket.local_addr() {
            if let IpAddr::V4(v4) = addr.ip() {
                return v4;
            }
        }
    }
    Ipv4Addr::LOCALHOST
}

fn set_text(window: &Option<Rc<RefCell<GameWindow>>>, value: &str) {
    let Some(window) = window else {
        return;
    };
    if let Some(entry) = window.borrow_mut().text_entry_mut() {
        entry.set_text(value);
    }
}

fn get_text(window: &Option<Rc<RefCell<GameWindow>>>) -> String {
    let Some(window) = window else {
        return String::new();
    };
    window
        .borrow_mut()
        .text_entry_mut()
        .map(|e| e.text().to_string())
        .unwrap_or_default()
}

fn add_chat_line(state: &LanLobbyState, text: &str) {
    let Some(list) = state.listbox_chat.as_ref() else {
        return;
    };
    if let Some(mut box_) = list.borrow_mut().list_box_mut() {
        box_.add_item(text);
    }
}

fn reset_list(window: &Option<Rc<RefCell<GameWindow>>>) {
    let Some(window) = window else {
        return;
    };
    if let Some(mut box_) = window.borrow_mut().list_box_mut() {
        box_.clear();
    }
}

fn spawn_lan<F>(fut: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tokio::spawn(fut);
}

fn pump_lan_events() {
    spawn_lan(async move {
        let mut guard = the_lan().lock().await;
        if let Some(api) = guard.as_mut() {
            match api.update().await {
                Ok(Some(event)) => {
                    if let Ok(mut q) = events().lock() {
                        q.push_back(event);
                    }
                }
                Ok(None) => {}
                Err(err) => warn!("TheLAN update failed: {err}"),
            }
        }
    });
}

fn handle_lan_event(state: &mut LanLobbyState, event: LanEvent) {
    match event {
        LanEvent::GameList(games) => {
            reset_list(&state.listbox_games);
            let Some(list) = state.listbox_games.as_ref() else {
                return;
            };
            if let Some(mut box_) = list.borrow_mut().list_box_mut() {
                for (idx, game) in games.iter().enumerate() {
                    let host = game
                        .get_host()
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| game.name.clone());
                    let label =
                        if matches!(game.state, game_network::lan_api::LanGameState::InProgress) {
                            format!("[{host}]")
                        } else {
                            host
                        };
                    box_.add_item_with_data(
                        idx as i32,
                        &label,
                        Some(ListBoxItemData::Integer(idx as i32)),
                    );
                }
            }
        }
        LanEvent::PlayerList(players) => {
            reset_list(&state.listbox_players);
            let Some(list) = state.listbox_players.as_ref() else {
                return;
            };
            if let Some(mut box_) = list.borrow_mut().list_box_mut() {
                for player in players {
                    box_.add_item(&player.name);
                }
            }
        }
        LanEvent::Chat(player, _ip, message, chat_type) => {
            let line = match chat_type {
                ChatType::System => message,
                ChatType::Emote => format!("{player} {message}"),
                ChatType::Normal => format!("[{player}] {message}"),
            };
            add_chat_line(state, &line);
        }
        LanEvent::GameJoin(LanResult::Ok, _) => {
            set_lan_button_pushed(true);
            state.next_screen = Some("Menus/LanGameOptionsMenu.wnd".to_string());
            let _ = get_shell().pop();
        }
        LanEvent::GameJoin(result, _) if result != LanResult::Busy => {
            add_chat_line(state, &GameText::fetch("LAN:JoinFailed"));
        }
        LanEvent::GameCreate(LanResult::Ok) => {
            set_lan_button_pushed(true);
            spawn_lan(async move {
                let mut guard = the_lan().lock().await;
                if let Some(api) = guard.as_mut() {
                    let _ = api.request_lobby_leave(false).await;
                }
            });
            state.next_screen = Some("Menus/LanGameOptionsMenu.wnd".to_string());
            let _ = get_shell().pop();
        }
        LanEvent::GameCreate(LanResult::GameExists) => {
            add_chat_line(state, &GameText::fetch("LAN:ErrorGameExists"));
        }
        LanEvent::GameCreate(LanResult::Busy) => {
            add_chat_line(state, &GameText::fetch("LAN:ErrorBusy"));
        }
        LanEvent::GameCreate(_) => {
            add_chat_line(state, &GameText::fetch("LAN:ErrorUnknown"));
        }
        LanEvent::HostLeave => add_chat_line(state, &GameText::fetch("LAN:HostLeft")),
        LanEvent::PlayerLeave(name) => add_chat_line(state, &format!("{name} left")),
        LanEvent::GameStartTimer(seconds) => {
            let key = if seconds == 1 {
                "LAN:GameStartTimerSingular"
            } else {
                "LAN:GameStartTimerPlural"
            };
            add_chat_line(
                state,
                &GameText::fetch(key).replace("%d", &seconds.to_string()),
            );
        }
        LanEvent::GameStart => {
            set_lan_button_pushed(true);
        }
        LanEvent::NetworkError(err) => {
            warn!("LAN socket error: {err}");
            state.socket_error = true;
        }
        _ => {}
    }
}

fn selected_game_offset(state: &LanLobbyState) -> Option<usize> {
    let list = state.listbox_games.as_ref()?;
    let mut box_ = list.borrow_mut();
    let listbox = box_.list_box_mut()?;
    listbox.selected_indices().first().copied()
}

fn request_join_selected(state: &LanLobbyState) {
    let Some(offset) = selected_game_offset(state) else {
        add_chat_line(state, &GameText::fetch("LAN:ErrorNoGameSelected"));
        return;
    };
    spawn_lan(async move {
        let mut guard = the_lan().lock().await;
        if let Some(api) = guard.as_mut() {
            if let Some(game) = api.lookup_game_by_offset(offset).await {
                if let Err(err) = api.request_game_join(&game, None).await {
                    warn!("LAN join failed: {err}");
                }
            }
        }
    });
}

pub fn lan_lobby_init(layout: &WindowLayout) {
    set_lan_button_pushed(false);
    set_lan_is_shutting_down(false);
    if let Ok(mut q) = events().lock() {
        q.clear();
    }

    let slot = state();
    let mut state = slot.borrow_mut();
    *state = LanLobbyState::default();
    state.just_entered = true;
    state.initial_gadget_delay = 2;

    state.parent_id = name_to_id("LanLobbyMenu.wnd:LanLobbyMenuParent");
    state.button_back_id = name_to_id("LanLobbyMenu.wnd:ButtonBack");
    state.button_clear_id = name_to_id("LanLobbyMenu.wnd:ButtonClear");
    state.button_host_id = name_to_id("LanLobbyMenu.wnd:ButtonHost");
    state.button_join_id = name_to_id("LanLobbyMenu.wnd:ButtonJoin");
    state.button_direct_connect_id = name_to_id("LanLobbyMenu.wnd:ButtonDirectConnect");
    state.button_emote_id = name_to_id("LanLobbyMenu.wnd:ButtonEmote");
    state.text_entry_player_name_id = name_to_id("LanLobbyMenu.wnd:TextEntryPlayerName");
    state.text_entry_chat_id = name_to_id("LanLobbyMenu.wnd:TextEntryChat");
    state.listbox_players_id = name_to_id("LanLobbyMenu.wnd:ListboxPlayers");
    state.listbox_chat_id = name_to_id("LanLobbyMenu.wnd:ListboxChatWindowLanLobby");
    state.listbox_games_id = name_to_id("LanLobbyMenu.wnd:ListboxGames");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.button_back = manager.get_window_by_id(state.button_back_id);
        state.button_clear = manager.get_window_by_id(state.button_clear_id);
        state.button_host = manager.get_window_by_id(state.button_host_id);
        state.button_join = manager.get_window_by_id(state.button_join_id);
        state.button_direct_connect = manager.get_window_by_id(state.button_direct_connect_id);
        state.button_emote = manager.get_window_by_id(state.button_emote_id);
        state.text_entry_player_name = manager.get_window_by_id(state.text_entry_player_name_id);
        state.text_entry_chat = manager.get_window_by_id(state.text_entry_chat_id);
        state.listbox_players = manager.get_window_by_id(state.listbox_players_id);
        state.listbox_chat = manager.get_window_by_id(state.listbox_chat_id);
        state.listbox_games = manager.get_window_by_id(state.listbox_games_id);
    });

    let prefs = LanPreferences::new();
    let mut name = prefs.get_user_name();
    while name.chars().count() > LAN_PLAYER_NAME_LENGTH {
        name.pop();
    }
    if name.is_empty() {
        name = "Player".to_string();
    }
    state.default_name = name.clone();
    set_text(&state.text_entry_player_name, &name);
    set_text(&state.text_entry_chat, "");
    reset_list(&state.listbox_players);
    reset_list(&state.listbox_games);

    let ip = local_ipv4();
    spawn_lan(async move {
        if let Err(err) = ensure_the_lan(&name).await {
            warn!("Failed to init TheLAN: {err}");
            if let Ok(mut q) = events().lock() {
                q.push_back(LanEvent::NetworkError(err.to_string()));
            }
            return;
        }
        let mut guard = the_lan().lock().await;
        if let Some(api) = guard.as_mut() {
            if api.set_local_ip(IpAddr::V4(ip)).await.is_err() {
                if let Ok(mut q) = events().lock() {
                    q.push_back(LanEvent::NetworkError("socket".into()));
                }
            }
            let _ = api.request_set_name(name.clone()).await;
            let _ = api.request_locations().await;
        }
    });

    if let Some(chat) = state.text_entry_chat.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(chat)));
    }
    show_shell_map_if_available(true);
    layout.hide(false);
    if let Some(gadget) = with_window_manager(|manager| {
        manager.get_window_by_id(name_to_id("LanLobbyMenu.wnd:GadgetParent"))
    }) {
        let _ = gadget.borrow_mut().hide(true);
    }
}

pub fn lan_lobby_shutdown(layout: &WindowLayout, immediate: bool) {
    let name = {
        let slot = state();
        let state = slot.borrow();
        get_text(&state.text_entry_player_name)
    };
    let mut prefs = LanPreferences::new();
    prefs.set_user_name(name);
    prefs.write();

    spawn_lan(async move {
        let mut guard = the_lan().lock().await;
        if let Some(api) = guard.as_mut() {
            let _ = api.request_lobby_leave(true).await;
        }
    });

    set_lan_is_shutting_down(true);
    if immediate {
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
        return;
    }
    get_shell().reverse_animate_window();
    with_window_manager(|manager| manager.transition_reverse("LanLobbyFade"));
}

pub fn lan_lobby_update(layout: &WindowLayout) {
    let slot = state();
    let mut state = slot.borrow_mut();
    if state.just_entered {
        if state.initial_gadget_delay == 1 {
            with_window_manager(|manager| manager.transition_set_group("LanLobbyFade", false));
            state.initial_gadget_delay = 2;
            state.just_entered = false;
        } else {
            state.initial_gadget_delay -= 1;
        }
    }

    if lan_is_shutting_down()
        && get_shell().is_anim_finished()
        && with_window_manager(|manager| manager.transitions_finished())
    {
        layout.hide(true);
        let next = state.next_screen.clone();
        let _ = get_shell().shutdown_complete(None, next.is_some());
        if let Some(screen) = next {
            let _ = get_shell().push(&screen, false);
        }
        set_lan_is_shutting_down(false);
        return;
    }

    if !lan_button_pushed() {
        pump_lan_events();
        let drained: Vec<LanEvent> = events()
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default();
        for event in drained {
            handle_lan_event(&mut state, event);
        }
    }

    if state.socket_error {
        state.socket_error = false;
        let parent_id = state.parent_id;
        let id = state.button_back_id;
        drop(state);
        let _ = with_window_manager(|manager| {
            if let Some(parent) = manager.get_window_by_id(parent_id) {
                let _ = parent.borrow_mut().send_system_message(
                    WindowMessage::GadgetSelected,
                    id as WindowMsgData,
                    id as WindowMsgData,
                );
            }
        });
    }
}

pub fn lan_lobby_input(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if lan_button_pushed() || (data2 & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }
    let slot = state();
    let state = slot.borrow();
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            state.button_back_id as WindowMsgData,
            state.button_back_id as WindowMsgData,
        );
    }
    WindowMsgHandled::Handled
}

pub fn lan_lobby_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => {
            spawn_lan(async move {
                reset_the_lan().await;
            });
            WindowMsgHandled::Handled
        }
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::User(code) if code == GLM_DOUBLE_CLICKED => {
            if lan_button_pushed() {
                return WindowMsgHandled::Handled;
            }
            let slot = state();
            let st = slot.borrow();
            if data1 as i32 == st.listbox_games_id {
                request_join_selected(&st);
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetSelected => {
            if lan_button_pushed() {
                return WindowMsgHandled::Handled;
            }
            let slot = state();
            let st = slot.borrow();
            let id = data1 as i32;
            if id == st.button_back_id {
                set_lan_button_pushed(true);
                drop(st);
                spawn_lan(async move {
                    reset_the_lan().await;
                });
                let _ = get_shell().pop();
            } else if id == st.button_host_id {
                spawn_lan(async move {
                    let mut guard = the_lan().lock().await;
                    if let Some(api) = guard.as_mut() {
                        let _ = api.request_game_create(String::new(), false).await;
                    }
                });
            } else if id == st.button_clear_id {
                set_text(&st.text_entry_player_name, "");
            } else if id == st.button_join_id {
                request_join_selected(&st);
            } else if id == st.button_emote_id {
                let mut text = get_text(&st.text_entry_chat);
                set_text(&st.text_entry_chat, "");
                text = text.trim().to_string();
                if !text.is_empty() {
                    spawn_lan(async move {
                        let mut guard = the_lan().lock().await;
                        if let Some(api) = guard.as_mut() {
                            let _ = api.request_chat(text, ChatType::Normal).await;
                        }
                    });
                }
            } else if id == st.button_direct_connect_id {
                spawn_lan(async move {
                    let mut guard = the_lan().lock().await;
                    if let Some(api) = guard.as_mut() {
                        let _ = api.request_lobby_leave(false).await;
                    }
                });
                let _ =
                    try_with_shell_mut(|shell| shell.push("Menus/NetworkDirectConnect.wnd", false));
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone | WindowMessage::GadgetValueChanged => {
            if lan_button_pushed() {
                return WindowMsgHandled::Handled;
            }
            let slot = state();
            let st = slot.borrow();
            if data1 as i32 == st.text_entry_player_name_id {
                let mut name = get_text(&st.text_entry_player_name);
                name = name.trim().to_string();
                while name.chars().count() > LAN_PLAYER_NAME_LENGTH {
                    name.pop();
                }
                for ch in [',', ':', ';'] {
                    if name.ends_with(ch) {
                        name.pop();
                    }
                }
                if name.is_empty() {
                    name = st.default_name.clone();
                }
                set_text(&st.text_entry_player_name, &name);
                spawn_lan(async move {
                    let mut guard = the_lan().lock().await;
                    if let Some(api) = guard.as_mut() {
                        let _ = api.request_set_name(name).await;
                    }
                });
            } else if data1 as i32 == st.text_entry_chat_id
                && matches!(msg, WindowMessage::GadgetEditDone)
            {
                let mut text = get_text(&st.text_entry_chat);
                set_text(&st.text_entry_chat, "");
                text = text.trim().to_string();
                if !text.is_empty() {
                    spawn_lan(async move {
                        let mut guard = the_lan().lock().await;
                        if let Some(api) = guard.as_mut() {
                            let _ = api.request_chat(text, ChatType::Normal).await;
                        }
                    });
                }
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
