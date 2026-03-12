//! WOLLocaleSelectPopup.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::game_text::GameText;
use crate::gamespy_overlay::{check_reopen_player_info, close_overlay, GameSpyOverlayType};
use crate::gui::{
    get_shell, with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::preferences::GameSpyMiscPreferences;
use game_network::gamespy::peer_defs::{default_gamespy_colors, get_gamespy_info, GameSpyColor};
use game_network::gamespy::persistent_storage_thread::{
    get_ps_message_queue, PSRequest, PSRequestType, PSResponse, PSResponseType, LOC_MAX, LOC_MIN,
};

#[derive(Default)]
struct WolLocaleSelectState {
    parent_id: u32,
    button_ok_id: u32,
    listbox_locale_id: u32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_ok: Option<Rc<RefCell<GameWindow>>>,
    listbox_locale: Option<Rc<RefCell<GameWindow>>>,
}

static WOL_LOCALE_SELECT_STATE: OnceLock<Mutex<WolLocaleSelectState>> = OnceLock::new();

fn wol_locale_state() -> &'static Mutex<WolLocaleSelectState> {
    WOL_LOCALE_SELECT_STATE.get_or_init(|| Mutex::new(WolLocaleSelectState::default()))
}

pub fn wol_locale_select_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let mut state = wol_locale_state()
        .lock()
        .expect("WOLLocaleSelect state lock poisoned");
    state.parent_id = NameKeyGenerator::name_to_key("PopupLocaleSelect.wnd:ParentLocaleSelect");
    state.button_ok_id = NameKeyGenerator::name_to_key("PopupLocaleSelect.wnd:ButtonOk");
    state.listbox_locale_id =
        NameKeyGenerator::name_to_key("PopupLocaleSelect.wnd:ListBoxLocaleSelect");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.button_ok = manager.get_window_by_id(state.button_ok_id);
        state.listbox_locale = manager.get_window_by_id(state.listbox_locale_id);
    });

    if let Some(listbox) = state.listbox_locale.as_ref() {
        let mut listbox = listbox.borrow_mut();
        if let Some(widget) = listbox.list_box_mut() {
            widget.clear();
            let colors = default_gamespy_colors();
            let color = colors[GameSpyColor::Default as usize];
            for locale in LOC_MIN..=LOC_MAX {
                let id = format!("WOL:Locale{:02}", locale);
                let text = GameText::fetch(&id);
                widget.add_item_with_color(&text, color);
            }
            if !widget.items().is_empty() {
                widget.set_selected_indices(&[0]);
            }
        }
    }

    layout.hide(false);

    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(parent));
            let _ = manager.set_modal(parent.clone());
        });
    }
}

pub fn wol_locale_select_shutdown(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    layout.hide(true);
    let _ = get_shell().shutdown_complete(Some(layout), false);
}

pub fn wol_locale_select_update(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {}

pub fn wol_locale_select_input(
    _window: &GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char {
        return WindowMsgHandled::Handled;
    }
    WindowMsgHandled::Ignored
}

pub fn wol_locale_select_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            let mut state = wol_locale_state()
                .lock()
                .expect("WOLLocaleSelect state lock poisoned");
            if control_id != state.button_ok_id {
                return WindowMsgHandled::Handled;
            }

            let selected = state
                .listbox_locale
                .as_ref()
                .and_then(|listbox| {
                    let mut listbox = listbox.borrow_mut();
                    listbox
                        .list_box_mut()
                        .and_then(|widget| widget.selected_indices().first().copied())
                })
                .unwrap_or(usize::MAX);

            if selected == usize::MAX {
                return WindowMsgHandled::Handled;
            }

            let locale = LOC_MIN + selected as i32;
            let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) else {
                return WindowMsgHandled::Handled;
            };
            let profile_id = info.get_local_profile_id();
            let email = info.get_local_email().as_str().to_string();
            let nick = info.get_local_base_name().as_str().to_string();
            let password = info.get_local_password().as_str().to_string();
            drop(info);

            let mut request = PSRequest::default();
            request.request_type = PSRequestType::UpdatePlayerLocale;
            request.player.id = profile_id;
            request.player.locale = locale;
            request.email = email;
            request.nick = nick;
            request.password = password;

            if let Some(queue) = get_ps_message_queue() {
                if let Ok(mut queue) = queue.lock() {
                    queue.add_request(request);
                }
            }

            close_overlay(GameSpyOverlayType::LocaleSelect);

            let mut prefs = GameSpyMiscPreferences::new();
            prefs.set_locale(locale);
            prefs.write();

            if let Some(queue) = get_ps_message_queue() {
                if let Ok(mut queue) = queue.lock() {
                    let mut stats = queue.find_player_stats_by_id(profile_id);
                    stats.locale = locale;
                    if stats.id == profile_id {
                        queue.track_player_stats(stats.clone());
                    }

                    if stats.id == 0 {
                        if let Some(info) = get_gamespy_info() {
                            if let Ok(mut info) = info.lock() {
                                let mut cached =
                                    info.get_cached_local_player_stats().unwrap_or_default();
                                cached.locale = locale;
                                info.set_cached_local_player_stats(cached);
                            }
                        }
                    } else {
                        let mut resp = PSResponse::default();
                        resp.response_type = PSResponseType::PlayerStats;
                        resp.player = queue.find_player_stats_by_id(profile_id);
                        queue.add_response(resp);
                    }
                }
            }

            check_reopen_player_info();
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
