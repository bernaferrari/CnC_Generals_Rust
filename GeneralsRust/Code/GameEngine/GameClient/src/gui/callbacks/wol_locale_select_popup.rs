//! WOLLocaleSelectPopup.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;

use crate::game_text::GameText;
use crate::gamespy_overlay::{GameSpyOverlayType, check_reopen_player_info, close_overlay};
use crate::gui::callbacks::online_callback_support::packed_ui_color;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, get_shell,
    with_window_manager, write_input_focus_response,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::preferences::GameSpyMiscPreferences;
use game_network::gamespy::peer_defs::{GameSpyColor, default_gamespy_colors, get_gamespy_info};
use game_network::gamespy::persistent_storage_thread::{
    LOC_MAX, LOC_MIN, PSRequest, PSRequestType, PSResponse, PSResponseType, get_ps_message_queue,
};

#[derive(Default)]
struct WolLocaleSelectState {
    parent_id: i32,
    button_ok_id: i32,
    listbox_locale_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_ok: Option<Rc<RefCell<GameWindow>>>,
    listbox_locale: Option<Rc<RefCell<GameWindow>>>,
}

thread_local! {
    static WOL_LOCALE_SELECT_STATE: Rc<RefCell<WolLocaleSelectState>> = Rc::new(RefCell::new(WolLocaleSelectState::default()));
}

fn wol_locale_state() -> Rc<RefCell<WolLocaleSelectState>> {
    WOL_LOCALE_SELECT_STATE.with(Rc::clone)
}

pub fn wol_locale_select_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_slot = wol_locale_state();
    let mut state = state_slot.borrow_mut();
    state.parent_id =
        NameKeyGenerator::name_to_key("PopupLocaleSelect.wnd:ParentLocaleSelect") as i32;
    state.button_ok_id = NameKeyGenerator::name_to_key("PopupLocaleSelect.wnd:ButtonOk") as i32;
    state.listbox_locale_id =
        NameKeyGenerator::name_to_key("PopupLocaleSelect.wnd:ListBoxLocaleSelect") as i32;

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
                widget.add_item_with_color(&text, packed_ui_color(color));
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
    let _ = get_shell().shutdown_complete(None, false);
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
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            let state_slot = wol_locale_state();
            let mut state = state_slot.borrow_mut();
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
            let Some(slot) = get_gamespy_info() else {
                return WindowMsgHandled::Handled;
            };
            let Ok(info) = slot.lock() else {
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
        WindowMessage::GadgetEditDone => WindowMsgHandled::Handled,
        _ => WindowMsgHandled::Ignored,
    }
}
