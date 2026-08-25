//! PopupHostGame.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;

use crate::gamespy_overlay::{
    GameSpyHostRequest, GameSpyOverlayType, close_overlay, open_overlay, queue_host_request,
    set_lobby_attempt_host_join,
};
use crate::gui::callbacks::online_callback_support::dispatch_esc_gadget_selected;
use crate::gui::callbacks::popup_ladder_select::populate_custom_ladder_combo_box;
use crate::gui::{
    CustomMatchPreferencesStore, GCM_SELECTED, GameWindow, WindowLayout, WindowMessage,
    WindowMsgData, WindowMsgHandled, with_window_manager, write_input_focus_response,
};
use game_engine::common::ini::ini_game_data::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::version::get_version;
use game_network::gamespy::config::GameSpyConfig;
use game_network::gamespy::ladder_defs::get_ladder_list;
use game_network::gamespy::peer_defs::get_gamespy_info;
use game_network::gamespy::peer_thread::{
    PeerRequest, PeerRequestType, get_peer_message_queue, init_peer_message_queue,
};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

#[derive(Default)]
struct PopupHostState {
    parent_id: i32,
    text_entry_game_name_id: i32,
    text_entry_game_description_id: i32,
    text_entry_game_password_id: i32,
    combo_box_ladder_name_id: i32,
    button_create_game_id: i32,
    button_cancel_id: i32,
    check_box_allow_observers_id: i32,
    check_box_limit_armies_id: i32,
    check_box_use_stats_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    text_entry_game_name: Option<Rc<RefCell<GameWindow>>>,
    text_entry_game_description: Option<Rc<RefCell<GameWindow>>>,
    text_entry_game_password: Option<Rc<RefCell<GameWindow>>>,
    combo_box_ladder_name: Option<Rc<RefCell<GameWindow>>>,
    check_box_allow_observers: Option<Rc<RefCell<GameWindow>>>,
    check_box_limit_armies: Option<Rc<RefCell<GameWindow>>>,
    check_box_use_stats: Option<Rc<RefCell<GameWindow>>>,
}

thread_local! {
    static POPUP_HOST_STATE: Rc<RefCell<PopupHostState>> = Rc::new(RefCell::new(PopupHostState::default()));
}

fn popup_host_state() -> Rc<RefCell<PopupHostState>> {
    POPUP_HOST_STATE.with(Rc::clone)
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

pub fn custom_match_hide_host_popup(hide: bool) {
    let state_slot = popup_host_state();
    let mut state = state_slot.borrow_mut();
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().hide(hide);
        return;
    }
    if state.parent_id == 0 {
        state.parent_id = name_to_id("PopupHostGame.wnd:ParentHostPopUp");
    }
    state.parent = with_window_manager(|manager| manager.get_window_by_id(state.parent_id));
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().hide(hide);
    }
}

fn set_text_entry(window: &Option<Rc<RefCell<GameWindow>>>, value: &str) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    if let Some(entry) = guard.text_entry_mut() {
        entry.set_text(value);
    }
}

fn get_text_entry(window: &Option<Rc<RefCell<GameWindow>>>) -> String {
    let Some(window) = window.as_ref() else {
        return String::new();
    };
    let guard = window.borrow();
    if let Some(entry) = guard.widget().and_then(|widget| match widget {
        crate::gui::WindowWidget::TextEntry(entry) => Some(entry),
        _ => None,
    }) {
        return entry.text().to_string();
    }
    String::new()
}

fn trim_game_name_leading_whitespace(state: &PopupHostState) {
    let text = get_text_entry(&state.text_entry_game_name);
    let trimmed = text.trim_start().to_string();
    if trimmed != text {
        set_text_entry(&state.text_entry_game_name, &trimmed);
    }
}

fn selected_combo_data(window: &GameWindow) -> Option<i32> {
    let combo = match window.widget()? {
        crate::gui::WindowWidget::ComboBox(combo) => combo,
        _ => return None,
    };
    let selected = combo.selected_index()?;
    combo.items().get(selected)?.data.map(|data| data as i32)
}

fn set_checkbox(window: &Option<Rc<RefCell<GameWindow>>>, checked: bool) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let _ = window.borrow_mut().gadget_check_box_set_checked(checked);
}

fn checkbox_checked(window: &Option<Rc<RefCell<GameWindow>>>) -> bool {
    let Some(window) = window.as_ref() else {
        return false;
    };
    let guard = window.borrow();
    guard
        .widget()
        .and_then(|widget| match widget {
            crate::gui::WindowWidget::CheckBox(check) => Some(check),
            _ => None,
        })
        .map(|check| check.is_checked())
        .unwrap_or(false)
}

fn enable_window(window: &Option<Rc<RefCell<GameWindow>>>, enabled: bool) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    let _ = guard.enable(enabled);
}

fn sync_limit_armies_state(state: &PopupHostState) {
    let use_stats = checkbox_checked(&state.check_box_use_stats);
    if use_stats {
        set_checkbox(&state.check_box_limit_armies, false);
        enable_window(&state.check_box_limit_armies, false);
    } else {
        enable_window(&state.check_box_limit_armies, true);
    }
}

fn clear_popup_host_refs(state: &mut PopupHostState) {
    state.parent = None;
    state.text_entry_game_name = None;
    state.text_entry_game_description = None;
    state.text_entry_game_password = None;
    state.check_box_allow_observers = None;
    state.check_box_limit_armies = None;
    state.check_box_use_stats = None;
}

pub fn popup_host_game_init(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_slot = popup_host_state();
    let mut state = state_slot.borrow_mut();

    state.parent_id = name_to_id("PopupHostGame.wnd:ParentHostPopUp");
    state.text_entry_game_name_id = name_to_id("PopupHostGame.wnd:TextEntryGameName");
    state.text_entry_game_description_id = name_to_id("PopupHostGame.wnd:TextEntryGameDescription");
    state.text_entry_game_password_id = name_to_id("PopupHostGame.wnd:TextEntryGamePassword");
    state.combo_box_ladder_name_id = name_to_id("PopupHostGame.wnd:ComboBoxLadderName");
    state.button_create_game_id = name_to_id("PopupHostGame.wnd:ButtonCreateGame");
    state.button_cancel_id = name_to_id("PopupHostGame.wnd:ButtonCancel");
    state.check_box_allow_observers_id = name_to_id("PopupHostGame.wnd:CheckBoxAllowObservers");
    state.check_box_limit_armies_id = name_to_id("PopupHostGame.wnd:CheckBoxLimitArmies");
    state.check_box_use_stats_id = name_to_id("PopupHostGame.wnd:CheckBoxUseStats");

    state.parent = with_window_manager(|manager| manager.get_window_by_id(state.parent_id));

    if let Some(parent) = state.parent.clone() {
        state.text_entry_game_name = parent
            .borrow()
            .find_child_by_id(state.text_entry_game_name_id);
        state.text_entry_game_description = parent
            .borrow()
            .find_child_by_id(state.text_entry_game_description_id);
        state.combo_box_ladder_name = parent
            .borrow()
            .find_child_by_id(state.combo_box_ladder_name_id);
        state.check_box_allow_observers = parent
            .borrow()
            .find_child_by_id(state.check_box_allow_observers_id);
        state.check_box_limit_armies = parent
            .borrow()
            .find_child_by_id(state.check_box_limit_armies_id);
        state.check_box_use_stats = parent
            .borrow()
            .find_child_by_id(state.check_box_use_stats_id);
    }

    let mut prefs = CustomMatchPreferencesStore::new();
    let local_name = get_gamespy_info()
        .and_then(|info| {
            info.lock()
                .ok()
                .map(|guard| guard.get_local_name().to_string())
        })
        .unwrap_or_else(|| "My Game".to_string());
    set_text_entry(&state.text_entry_game_name, &local_name);
    set_text_entry(&state.text_entry_game_description, "");
    set_text_entry(&state.text_entry_game_password, "");
    set_checkbox(
        &state.check_box_allow_observers,
        prefs.prefs().allows_observers(),
    );
    set_checkbox(&state.check_box_use_stats, prefs.prefs().get_use_stats());
    set_checkbox(
        &state.check_box_limit_armies,
        prefs.prefs().get_factions_limited(),
    );

    sync_limit_armies_state(&state);
    drop(state);

    let _ = populate_custom_ladder_combo_box();

    let state_slot = popup_host_state();
    let state = state_slot.borrow_mut();
    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(parent));
            let _ = manager.set_modal(parent.clone());
        });
    }
}

pub fn popup_host_game_update(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_slot = popup_host_state();
    let state = state_slot.borrow_mut();
    sync_limit_armies_state(&state);
}

pub fn popup_host_game_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char {
        return WindowMsgHandled::Ignored;
    }
    if data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if (data2 & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    let (parent, button_id) = {
        let slot = popup_host_state();
        let state = slot.borrow();
        (state.parent.clone(), state.button_cancel_id)
    };
    dispatch_esc_gadget_selected(parent, button_id);

    WindowMsgHandled::Handled
}

pub fn popup_host_game_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetValueChanged => {
            let state_slot = popup_host_state();
            let state = state_slot.borrow_mut();
            if data1 as i32 == state.text_entry_game_name_id {
                trim_game_name_leading_whitespace(&state);
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::User(code) if code == GCM_SELECTED => {
            let state_slot = popup_host_state();
            let state = state_slot.borrow_mut();
            if data1 as i32 == state.combo_box_ladder_name_id
                && selected_combo_data(window).is_some_and(|ladder_id| ladder_id < 0)
            {
                drop(state);
                let _ = populate_custom_ladder_combo_box();
                open_overlay(GameSpyOverlayType::LadderSelect);
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetSelected => {
            let state_slot = popup_host_state();
            let mut state = state_slot.borrow_mut();
            let control_id = data1 as i32;
            if control_id == state.button_cancel_id {
                // Clear modal before closing - matches C++ GWM_DESTROY handling
                if let Some(parent) = state.parent.as_ref() {
                    with_window_manager(|manager| {
                        let _ = manager.unset_modal(parent);
                    });
                }
                close_overlay(GameSpyOverlayType::GameOptions);
                set_lobby_attempt_host_join(false);
                clear_popup_host_refs(&mut state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_create_game_id {
                let mut prefs = CustomMatchPreferencesStore::new();
                let mut game_name = get_text_entry(&state.text_entry_game_name);
                game_name = game_name.trim().to_string();
                if game_name.is_empty() {
                    game_name = get_gamespy_info()
                        .and_then(|info| {
                            info.lock()
                                .ok()
                                .map(|guard| guard.get_local_name().to_string())
                        })
                        .unwrap_or_else(|| "My Game".to_string());
                    set_text_entry(&state.text_entry_game_name, &game_name);
                }

                let description = get_text_entry(&state.text_entry_game_description);
                let password = get_text_entry(&state.text_entry_game_password);
                let allow_observers = checkbox_checked(&state.check_box_allow_observers);
                let use_stats = checkbox_checked(&state.check_box_use_stats);
                let limit_armies = checkbox_checked(&state.check_box_limit_armies);

                prefs.prefs_mut().set_allows_observers(allow_observers);
                prefs.prefs_mut().set_use_stats(use_stats);
                prefs.prefs_mut().set_factions_limited(limit_armies);
                prefs.write();

                let (exe_crc, ini_crc) = get_global_data()
                    .map(|data| {
                        let g = data.read();
                        (g.exe_crc, g.ini_crc)
                    })
                    .unwrap_or((0, 0));
                let game_version = get_version().get_version_number();
                let restrict_game_list = GameSpyConfig::new_sync().restrict_games_to_lobby();
                let host_ping_str = get_gamespy_info()
                    .and_then(|info| info.lock().ok().map(|g| g.get_ping_string().to_string()))
                    .unwrap_or_default();
                let ladder_id = state
                    .combo_box_ladder_name
                    .as_ref()
                    .and_then(|w| selected_combo_data(&w.borrow()))
                    .unwrap_or(-1);
                let (ladder_ip, ladder_port) = get_ladder_list()
                    .and_then(|list| {
                        list.read().ok().and_then(|l| {
                            l.find_ladder_by_index(ladder_id)
                                .map(|info| (info.address.to_string(), info.port))
                        })
                    })
                    .unwrap_or_default();
                let request = GameSpyHostRequest {
                    game_name: game_name.clone(),
                    game_description: description.clone(),
                    game_password: password.clone(),
                    allow_observers,
                    use_stats,
                    limit_armies,
                    exe_crc,
                    ini_crc,
                    game_version,
                    restrict_game_list,
                    ladder_ip: ladder_ip.clone(),
                    ladder_port,
                    host_ping_str: host_ping_str.clone(),
                };
                queue_host_request(request);

                let mut req = PeerRequest::default();
                req.request_type = PeerRequestType::CreateStagingRoom;
                req.text = game_name;
                req.password = password;
                req.options = description;
                req.allow_observers = allow_observers;
                req.use_stats = use_stats;
                req.exe_crc = exe_crc;
                req.ini_crc = ini_crc;
                req.game_version = game_version;
                req.restrict_game_list = restrict_game_list;
                req.ladder_ip = ladder_ip;
                req.lad_port = ladder_port;
                req.host_ping_str = host_ping_str;
                let queue = get_peer_message_queue().unwrap_or_else(init_peer_message_queue);
                if let Ok(mut queue) = queue.lock() {
                    queue.add_request(req);
                }
                // Clear modal before closing
                if let Some(parent) = state.parent.as_ref() {
                    with_window_manager(|manager| {
                        let _ = manager.unset_modal(parent);
                    });
                }
                close_overlay(GameSpyOverlayType::GameOptions);
                set_lobby_attempt_host_join(false);
                clear_popup_host_refs(&mut state);
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => {
            let state_slot = popup_host_state();
            let state = state_slot.borrow_mut();
            if data1 as i32 == state.text_entry_game_name_id {
                trim_game_name_leading_whitespace(&state);
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::Destroy => {
            let state_slot = popup_host_state();
            let mut state = state_slot.borrow_mut();
            state.parent = None;
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
