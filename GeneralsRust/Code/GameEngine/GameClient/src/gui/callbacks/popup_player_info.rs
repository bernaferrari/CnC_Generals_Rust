//! PopupPlayerInfo.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::game_text::GameText;
use crate::gamespy_overlay::{
    close_overlay, gs_message_box_yes_no, is_overlay_open, open_overlay, raise_gs_message_box,
    reopen_player_info, GameSpyOverlayType,
};
use crate::gui::callbacks::wol_lobby_menu::refresh_game_list_boxes;
use crate::gui::callbacks::wol_welcome_menu::{get_look_at_player, populate_player_info_windows};
use crate::gui::CustomMatchPreferencesStore;
use crate::gui::{
    with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_network::gamespy::buddy_thread::{
    get_buddy_message_queue, BuddyRequest, BuddyRequestType,
};
use game_network::gamespy::peer_defs::get_gamespy_info;

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;

#[derive(Default)]
struct PopupPlayerInfoState {
    parent_id: u32,
    listbox_info_id: u32,
    button_close_id: u32,
    button_buddies_id: u32,
    button_set_locale_id: u32,
    button_delete_account_id: u32,
    checkbox_asian_id: u32,
    checkbox_non_asian_id: u32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    listbox_info: Option<Rc<RefCell<GameWindow>>>,
    button_close: Option<Rc<RefCell<GameWindow>>>,
    button_buddies: Option<Rc<RefCell<GameWindow>>>,
    button_set_locale: Option<Rc<RefCell<GameWindow>>>,
    button_delete_account: Option<Rc<RefCell<GameWindow>>>,
    checkbox_asian: Option<Rc<RefCell<GameWindow>>>,
    checkbox_non_asian: Option<Rc<RefCell<GameWindow>>>,
    is_overlay_active: bool,
    raise_message_box: bool,
}

static POPUP_STATE: OnceLock<Mutex<PopupPlayerInfoState>> = OnceLock::new();

fn popup_state() -> &'static Mutex<PopupPlayerInfoState> {
    POPUP_STATE.get_or_init(|| Mutex::new(PopupPlayerInfoState::default()))
}

fn name_to_id(name: &str) -> u32 {
    NameKeyGenerator::name_to_key(name) as u32
}

fn checkbox_checked(window: &Rc<RefCell<GameWindow>>) -> bool {
    window
        .borrow()
        .widget()
        .and_then(|widget| match widget {
            crate::gui::WindowWidget::CheckBox(check) => Some(check.is_checked()),
            _ => None,
        })
        .unwrap_or(false)
}

fn set_checkbox_checked(window: &Rc<RefCell<GameWindow>>, checked: bool) {
    if let Some(check) = window.borrow_mut().check_box_mut() {
        check.set_checked(checked);
    }
}

fn message_box_yes() {
    if let Some(queue) = get_buddy_message_queue() {
        if let Ok(mut queue) = queue.lock() {
            let mut request = BuddyRequest::default();
            request.request_type = BuddyRequestType::DeleteAcct;
            queue.add_request(request);
        }
    }
    if let Some(mut info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        info.set_local_profile_id(0);
    }
}

pub fn popup_player_info_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let mut state = popup_state().lock().unwrap_or_else(|e| e.into_inner());
    state.parent_id = name_to_id("PopupPlayerInfo.wnd:PopupParent");
    state.button_close_id = name_to_id("PopupPlayerInfo.wnd:ButtonClose");
    state.button_buddies_id = name_to_id("PopupPlayerInfo.wnd:ButtonCommunicator");
    state.listbox_info_id = name_to_id("PopupPlayerInfo.wnd:ListboxInfo");
    state.button_set_locale_id = name_to_id("PopupPlayerInfo.wnd:ButtonSetLocale");
    state.button_delete_account_id = name_to_id("PopupPlayerInfo.wnd:ButtonDeleteAccount");
    state.checkbox_asian_id = name_to_id("PopupPlayerInfo.wnd:CheckBoxAsianText");
    state.checkbox_non_asian_id = name_to_id("PopupPlayerInfo.wnd:CheckBoxNonAsianText");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.button_close = manager.get_window_by_id(state.button_close_id);
        state.button_buddies = manager.get_window_by_id(state.button_buddies_id);
        state.listbox_info = manager.get_window_by_id(state.listbox_info_id);
        state.button_set_locale = manager.get_window_by_id(state.button_set_locale_id);
        state.button_delete_account = manager.get_window_by_id(state.button_delete_account_id);
        state.checkbox_asian = manager.get_window_by_id(state.checkbox_asian_id);
        state.checkbox_non_asian = manager.get_window_by_id(state.checkbox_non_asian_id);
    });

    layout.hide(false);
    if let Some(parent) = state.parent.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
    }

    state.is_overlay_active = true;
    close_overlay(GameSpyOverlayType::Buddy);
    state.raise_message_box = true;

    let (look_id, look_name) = get_look_at_player();
    if look_id > 0 {
        populate_player_info_windows("PopupPlayerInfo.wnd", look_id, &look_name);
    }

    let local_profile_id = get_gamespy_info()
        .and_then(|info| info.lock().ok().map(|guard| guard.get_local_profile_id()))
        .unwrap_or(0);
    let is_local = look_id == local_profile_id && look_id > 0;

    if let Some(button) = state.button_set_locale.as_ref() {
        let _ = button.borrow_mut().hide(!is_local);
    }
    if let Some(button) = state.button_delete_account.as_ref() {
        let _ = button.borrow_mut().hide(true);
    }
    if let Some(check) = state.checkbox_asian.as_ref() {
        let _ = check.borrow_mut().hide(!is_local);
    }
    if let Some(check) = state.checkbox_non_asian.as_ref() {
        let _ = check.borrow_mut().hide(!is_local);
    }

    let prefs = CustomMatchPreferencesStore::new();
    if let Some(check) = state.checkbox_asian.as_ref() {
        set_checkbox_checked(check, !prefs.prefs().get_disallow_asian_text());
    }
    if let Some(check) = state.checkbox_non_asian.as_ref() {
        set_checkbox_checked(check, !prefs.prefs().get_disallow_non_asian_text());
    }
}

pub fn popup_player_info_shutdown(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    layout.hide(true);
    let mut state = popup_state().lock().unwrap_or_else(|e| e.into_inner());
    state.parent = None;
    state.is_overlay_active = false;
}

pub fn popup_player_info_update(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let mut state = popup_state().lock().unwrap_or_else(|e| e.into_inner());
    if state.raise_message_box {
        raise_gs_message_box();
        state.raise_message_box = false;
    }
}

pub fn popup_player_info_input(
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
    let state = popup_state().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            state.button_close_id,
            state.button_close_id,
        );
    }
    WindowMsgHandled::Handled
}

pub fn popup_player_info_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    let mut state = popup_state().lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => {
            // TODO: C++ writes back focus state via mData2 pointer; Rust uses values, needs write-back parity
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            if control_id == state.button_close_id {
                refresh_game_list_boxes();
                close_overlay(GameSpyOverlayType::PlayerInfo);
            } else if control_id == state.button_buddies_id {
                refresh_game_list_boxes();
                open_overlay(GameSpyOverlayType::Buddy);
            } else if control_id == state.button_set_locale_id {
                refresh_game_list_boxes();
                close_overlay(GameSpyOverlayType::PlayerInfo);
                if !is_overlay_open(GameSpyOverlayType::LocaleSelect) {
                    open_overlay(GameSpyOverlayType::LocaleSelect);
                }
                reopen_player_info();
            } else if control_id == state.button_delete_account_id {
                refresh_game_list_boxes();
                close_overlay(GameSpyOverlayType::PlayerInfo);
                gs_message_box_yes_no(
                    &GameText::fetch("GUI:DeleteAccount"),
                    &GameText::fetch("GUI:AreYouSureDeleteAccount"),
                    Some(Box::new(message_box_yes)),
                    None,
                );
            } else if control_id == state.checkbox_asian_id {
                if let Some(check) = state.checkbox_asian.as_ref() {
                    let allow_asian = checkbox_checked(check);
                    let disallow_asian = !allow_asian;
                    let mut prefs = CustomMatchPreferencesStore::new();
                    prefs.prefs_mut().set_disallow_asian_text(disallow_asian);
                    prefs.write();
                    if let Some(mut info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                        info.set_disallow_asian_text(disallow_asian);
                    }
                    if disallow_asian {
                        if let Some(other) = state.checkbox_non_asian.as_ref() {
                            if !checkbox_checked(other) {
                                set_checkbox_checked(other, true);
                                let mut prefs = CustomMatchPreferencesStore::new();
                                prefs.prefs_mut().set_disallow_non_asian_text(false);
                                prefs.write();
                                if let Some(mut info) =
                                    get_gamespy_info().and_then(|info| info.lock().ok())
                                {
                                    info.set_disallow_non_asian_text(false);
                                }
                            }
                        }
                    }
                }
            } else if control_id == state.checkbox_non_asian_id {
                if let Some(check) = state.checkbox_non_asian.as_ref() {
                    let allow_non_asian = checkbox_checked(check);
                    let disallow_non_asian = !allow_non_asian;
                    let mut prefs = CustomMatchPreferencesStore::new();
                    prefs
                        .prefs_mut()
                        .set_disallow_non_asian_text(disallow_non_asian);
                    prefs.write();
                    if let Some(mut info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
                        info.set_disallow_non_asian_text(disallow_non_asian);
                    }
                    if disallow_non_asian {
                        if let Some(other) = state.checkbox_asian.as_ref() {
                            if !checkbox_checked(other) {
                                set_checkbox_checked(other, true);
                                let mut prefs = CustomMatchPreferencesStore::new();
                                prefs.prefs_mut().set_disallow_asian_text(false);
                                prefs.write();
                                if let Some(mut info) =
                                    get_gamespy_info().and_then(|info| info.lock().ok())
                                {
                                    info.set_disallow_asian_text(false);
                                }
                            }
                        }
                    }
                }
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
