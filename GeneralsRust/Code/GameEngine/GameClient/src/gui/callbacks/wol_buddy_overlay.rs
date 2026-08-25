//! WOLBuddyOverlay.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::game_text::GameText;
use crate::gamespy_game::with_gamespy_game_info;
use crate::gamespy_overlay::{GameSpyOverlayType, close_overlay, open_overlay};
use crate::gui::callbacks::online_callback_support::packed_ui_color;
use crate::gui::callbacks::wol_lobby_menu::{
    populate_lobby_player_listbox, refresh_game_list_boxes,
};
use crate::gui::gadgets::{ListBoxItemData, ListBoxRightClick};
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowWidget,
    with_window_manager, write_input_focus_response,
};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_network::gamespy::buddy_thread::{
    BuddyRequest, BuddyRequestType, BuddyResponseType, MAX_BUDDY_CHAT_LEN, get_buddy_message_queue,
};
use game_network::gamespy::peer_defs::{
    BuddyMessage, GPProfile, GameSpyBuddyStatus, GameSpyColor, default_gamespy_colors,
    get_gamespy_info, make_color,
};
use game_network::gamespy::persistent_storage_thread::{
    PSRequest, PSRequestType, get_ps_message_queue,
};
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const NOTIFICATION_EXPIRES_MS: u128 = 3000;
const GGM_LEFT_DRAG: u32 = 16384;
const GGM_CLOSE: u32 = GGM_LEFT_DRAG + 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RcItemType {
    Buddy,
    Request,
    NonBuddy,
}

#[derive(Debug, Clone)]
pub struct GameSpyRcMenuData {
    pub id: GPProfile,
    pub nick: AsciiString,
    pub item_type: RcItemType,
}

#[derive(Default)]
struct WolBuddyOverlayState {
    parent_id: i32,
    button_hide_id: i32,
    button_add_buddy_id: i32,
    button_delete_buddy_id: i32,
    button_accept_buddy_id: i32,
    button_deny_buddy_id: i32,
    radio_button_buddies_id: i32,
    radio_button_ignore_id: i32,
    parent_buddies_id: i32,
    parent_ignore_id: i32,
    listbox_ignore_id: i32,
    button_notification_id: i32,
    rc_button_add_id: i32,
    rc_button_delete_id: i32,
    rc_button_play_id: i32,
    rc_button_ignore_id: i32,
    rc_button_stats_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_hide: Option<Rc<RefCell<GameWindow>>>,
    button_add_buddy: Option<Rc<RefCell<GameWindow>>>,
    button_delete_buddy: Option<Rc<RefCell<GameWindow>>>,
    button_accept_buddy: Option<Rc<RefCell<GameWindow>>>,
    button_deny_buddy: Option<Rc<RefCell<GameWindow>>>,
    radio_button_buddies: Option<Rc<RefCell<GameWindow>>>,
    radio_button_ignore: Option<Rc<RefCell<GameWindow>>>,
    parent_buddies: Option<Rc<RefCell<GameWindow>>>,
    parent_ignore: Option<Rc<RefCell<GameWindow>>>,
    listbox_ignore: Option<Rc<RefCell<GameWindow>>>,
    rc_menu: Option<Rc<RefCell<GameWindow>>>,
    rc_layout: Option<Rc<RefCell<WindowLayout>>>,
    notice_layout: Option<Rc<RefCell<WindowLayout>>>,
    notice_expires: u128,
    last_notification_was_status: bool,
    num_online_in_notification: i32,
    is_overlay_active: bool,
}

thread_local! {
    static WOL_BUDDY_OVERLAY_STATE: Rc<RefCell<WolBuddyOverlayState>> = Rc::new(RefCell::new(WolBuddyOverlayState::default()));
}

fn wol_buddy_state() -> Rc<RefCell<WolBuddyOverlayState>> {
    WOL_BUDDY_OVERLAY_STATE.with(Rc::clone)
}

#[derive(Default)]
struct BuddyControls {
    listbox_chat_id: i32,
    listbox_buddies_id: i32,
    text_entry_edit_id: i32,
    listbox_chat: Option<Rc<RefCell<GameWindow>>>,
    listbox_buddies: Option<Rc<RefCell<GameWindow>>>,
    text_entry_edit: Option<Rc<RefCell<GameWindow>>>,
    is_init: bool,
}

thread_local! {
    static BUDDY_CONTROLS: Rc<RefCell<BuddyControls>> = Rc::new(RefCell::new(BuddyControls::default()));
}

fn buddy_controls() -> Rc<RefCell<BuddyControls>> {
    BUDDY_CONTROLS.with(Rc::clone)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuddyWindowType {
    ResetAll = -1,
    Buddies = 0,
    Diplomacy = 1,
    WelcomeScreen = 2,
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

fn clear_buddy_controls() {
    let controls_slot = buddy_controls();
    let mut controls = controls_slot.borrow_mut();
    controls.listbox_chat_id = 0;
    controls.listbox_buddies_id = 0;
    controls.text_entry_edit_id = 0;
    controls.listbox_chat = None;
    controls.listbox_buddies = None;
    controls.text_entry_edit = None;
    controls.is_init = false;
}

fn init_buddy_controls(kind: BuddyWindowType) {
    if get_gamespy_info().is_none() {
        clear_buddy_controls();
        return;
    }

    match kind {
        BuddyWindowType::ResetAll => {
            clear_buddy_controls();
        }
        BuddyWindowType::Buddies => {
            let controls_slot = buddy_controls();
            let mut controls = controls_slot.borrow_mut();
            controls.text_entry_edit_id = name_to_id("WOLBuddyOverlay.wnd:TextEntryChat");
            controls.listbox_buddies_id = name_to_id("WOLBuddyOverlay.wnd:ListboxBuddies");
            controls.listbox_chat_id = name_to_id("WOLBuddyOverlay.wnd:ListboxBuddyChat");
            with_window_manager(|manager| {
                controls.text_entry_edit = manager.get_window_by_id(controls.text_entry_edit_id);
                controls.listbox_buddies = manager.get_window_by_id(controls.listbox_buddies_id);
                controls.listbox_chat = manager.get_window_by_id(controls.listbox_chat_id);
            });
            if let Some(entry) = controls.text_entry_edit.as_ref() {
                if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                    widget.set_text("");
                }
            }
            controls.is_init = true;
        }
        BuddyWindowType::Diplomacy => {
            let controls_slot = buddy_controls();
            let mut controls = controls_slot.borrow_mut();
            controls.text_entry_edit_id = name_to_id("Diplomacy.wnd:TextEntryChat");
            controls.listbox_buddies_id = name_to_id("Diplomacy.wnd:ListboxBuddies");
            controls.listbox_chat_id = name_to_id("Diplomacy.wnd:ListboxBuddyChat");
            with_window_manager(|manager| {
                controls.text_entry_edit = manager.get_window_by_id(controls.text_entry_edit_id);
                controls.listbox_buddies = manager.get_window_by_id(controls.listbox_buddies_id);
                controls.listbox_chat = manager.get_window_by_id(controls.listbox_chat_id);
            });
            if let Some(entry) = controls.text_entry_edit.as_ref() {
                if let Some(widget) = entry.borrow_mut().text_entry_mut() {
                    widget.set_text("");
                }
            }
            controls.is_init = true;
        }
        BuddyWindowType::WelcomeScreen => {
            clear_buddy_controls();
        }
    }
}

fn listbox_right_click_info(window: &GameWindow) -> Option<(ListBoxRightClick, i32, i32)> {
    let (win_x, win_y) = window.get_screen_position();
    let WindowWidget::ListBox(listbox) = window.widget()? else {
        return None;
    };
    let info = listbox.last_right_click()?;
    Some((info, win_x + info.mouse_x, win_y + info.mouse_y))
}

fn listbox_selected_profile(listbox: &GameWindow) -> Option<(usize, GPProfile, RcItemType)> {
    let WindowWidget::ListBox(widget) = listbox.widget()? else {
        return None;
    };
    let selected = widget.selected_indices().first().copied()?;
    let profile = match widget.get_item_column_user_data(selected, 0)? {
        ListBoxItemData::Integer(id) => *id,
        _ => return None,
    };
    let item_type = match widget.get_item_column_user_data(selected, 1)? {
        ListBoxItemData::Integer(0) => RcItemType::Buddy,
        ListBoxItemData::Integer(1) => RcItemType::Request,
        ListBoxItemData::Integer(2) => RcItemType::NonBuddy,
        _ => RcItemType::NonBuddy,
    };
    Some((selected, profile, item_type))
}

fn listbox_item_text(listbox: &GameWindow, index: usize) -> Option<String> {
    let WindowWidget::ListBox(widget) = listbox.widget()? else {
        return None;
    };
    widget.items().get(index).map(|item| item.text.clone())
}

fn insert_chat_with_local(
    message: BuddyMessage,
    local_profile: GPProfile,
    local_name: AsciiString,
) {
    let controls_slot = buddy_controls();
    let controls = controls_slot.borrow_mut();
    let Some(listbox_window) = controls.listbox_chat.as_ref() else {
        return;
    };
    let mut listbox_guard = listbox_window.borrow_mut();
    let Some(listbox) = listbox_guard.list_box_mut() else {
        return;
    };

    let local_sender = message.sender_id == local_profile;
    let mut line = String::new();
    let mut color = default_gamespy_colors()[GameSpyColor::Default as usize];

    if local_sender {
        line = format!(
            "[{} -> {}] {}",
            local_name.as_str(),
            message.recipient_nick.as_str(),
            message.message
        );
        color = default_gamespy_colors()[GameSpyColor::PlayerSelf as usize];
    } else if message.sender_id == 0 {
        line = message.message.clone();
        color = default_gamespy_colors()[GameSpyColor::Default as usize];
    } else {
        line = format!("[{}] {}", message.sender_nick.as_str(), message.message);
        color = default_gamespy_colors()[GameSpyColor::PlayerBuddy as usize];
    }

    let index = listbox.add_item_with_color(&line, packed_ui_color(color));
    let _ = listbox.set_item_column_data(index, 1, ListBoxItemData::Text(String::new()));
}

fn insert_chat(message: BuddyMessage) {
    let (local_profile, local_name) = get_gamespy_info()
        .and_then(|info| {
            info.lock()
                .ok()
                .map(|guard| (guard.get_local_profile_id(), guard.get_local_base_name()))
        })
        .unwrap_or((0, AsciiString::new()));
    insert_chat_with_local(message, local_profile, local_name);
}

fn update_buddy_info() {
    let controls_slot = buddy_controls();
    let mut controls = controls_slot.borrow_mut();
    let Some(listbox_window) = controls.listbox_buddies.as_ref() else {
        return;
    };

    let mut listbox_guard = listbox_window.borrow_mut();
    let Some(listbox) = listbox_guard.list_box_mut() else {
        return;
    };

    let queue_connected = get_buddy_message_queue()
        .and_then(|queue| queue.lock().ok().map(|queue| queue.is_connected()))
        .unwrap_or(false);

    if !queue_connected {
        listbox.clear();
        return;
    }

    if !controls.is_init {
        return;
    }

    let visible_pos = listbox.get_top_visible_entry();
    let selected_profile = listbox
        .selected_indices()
        .first()
        .and_then(|idx| listbox.get_item_column_user_data(*idx, 0))
        .and_then(|data| match data {
            ListBoxItemData::Integer(id) => Some(*id),
            _ => None,
        })
        .unwrap_or(0);

    listbox.clear();

    let Some((buddies, requests, group_rooms)) =
        crate::gui::callbacks::online_callback_support::with_gamespy_info(|info| {
            let buddies = info
                .get_buddy_map()
                .iter()
                .map(|(id, buddy)| (*id, buddy.clone(), info.is_saved_ignored(*id)))
                .collect::<Vec<_>>();
            let requests = info
                .get_buddy_request_map()
                .iter()
                .map(|(id, buddy)| (*id, buddy.clone()))
                .collect::<Vec<_>>();
            let group_rooms = info.get_group_room_list().clone();
            (buddies, requests, group_rooms)
        })
    else {
        return;
    };

    for (profile_id, buddy_info, ignored) in buddies.iter() {
        let name = buddy_info.name.as_str().to_string();
        let name_color = if *ignored {
            default_gamespy_colors()[GameSpyColor::PlayerIgnored as usize]
        } else {
            default_gamespy_colors()[GameSpyColor::PlayerBuddy as usize]
        };
        let index = listbox.add_item_with_color(&name, packed_ui_color(name_color));

        let status_key = buddy_info.status_string.to_lowercase();
        let status_text = match status_key.as_str() {
            "offline" | "online" | "matching" => {
                let marker = format!("Buddy:{}", buddy_info.status_string);
                GameText::fetch(&marker)
            }
            "staging" | "loading" | "playing" => {
                let marker = format!("Buddy:{}", buddy_info.status_string);
                let mut text = GameText::fetch(&marker);
                if !buddy_info.location_string.is_empty() {
                    text = text.replace("%s", buddy_info.location_string.as_str());
                }
                text
            }
            "chatting" => {
                let mut room_name = String::new();
                if let Ok(room_id) = buddy_info.location_string.parse::<i32>() {
                    if let Some(room) = group_rooms.get(&room_id) {
                        let key = format!("GUI:{}", room.name.as_str());
                        room_name = GameText::fetch(&key);
                    }
                }
                let marker = format!("Buddy:{}", buddy_info.status_string);
                let mut text = GameText::fetch(&marker);
                if !room_name.is_empty() {
                    text = text.replace("%s", &room_name);
                }
                text
            }
            _ => buddy_info.status_string.clone(),
        };

        let _ = listbox.set_item_column_user_data(
            index,
            0,
            Some(ListBoxItemData::Integer(*profile_id)),
        );
        let _ = listbox.set_item_column_user_data(
            index,
            1,
            Some(ListBoxItemData::Integer(RcItemType::Buddy as i32)),
        );
        let _ = listbox.set_item_column_data(index, 2, ListBoxItemData::Text(status_text));

        if *profile_id == selected_profile {
            listbox.set_selected_indices(&[index]);
        }
    }

    for (profile_id, buddy_info) in requests.iter() {
        let name = buddy_info.name.as_str().to_string();
        let index = listbox.add_item_with_color(
            &name,
            packed_ui_color(default_gamespy_colors()[GameSpyColor::Default as usize]),
        );
        let status = GameText::fetch("GUI:BuddyAddReq");
        let _ = listbox.set_item_column_user_data(
            index,
            0,
            Some(ListBoxItemData::Integer(*profile_id)),
        );
        let _ = listbox.set_item_column_user_data(
            index,
            1,
            Some(ListBoxItemData::Integer(RcItemType::Request as i32)),
        );
        let _ = listbox.set_item_column_data(index, 2, ListBoxItemData::Text(status));
        if *profile_id == selected_profile {
            listbox.set_selected_indices(&[index]);
        }
    }

    listbox.set_top_visible_entry(visible_pos);
}

pub fn handle_buddy_responses() {
    let Some(queue) = get_buddy_message_queue() else {
        return;
    };
    let resp = queue.lock().ok().and_then(|mut queue| queue.get_response());
    let Some(resp) = resp else {
        return;
    };

    if let Some(info) = get_gamespy_info() {
        if let Ok(mut info) = info.lock() {
            match resp.response_type {
                BuddyResponseType::Login => {
                    delete_notification_box();
                }
                BuddyResponseType::Disconnect => {
                    let state_slot = wol_buddy_state();
                    let mut state = state_slot.borrow_mut();
                    state.last_notification_was_status = false;
                    state.num_online_in_notification = 0;
                    drop(state);
                    show_notification_box(
                        AsciiString::new(),
                        &GameText::fetch("Buddy:MessageDisconnected"),
                    );
                }
                BuddyResponseType::Message => {
                    if resp.message_text == "I have authorized your request to add me to your list"
                    {
                        return;
                    }

                    if info.is_saved_ignored(resp.profile) {
                        return;
                    }

                    let sender_nick = if let Some(buddy) = info.get_buddy_map().get(&resp.profile) {
                        buddy.name.clone()
                    } else {
                        AsciiString::from(resp.message_nick.as_str())
                    };

                    let buddy_msg = BuddyMessage::new(
                        resp.profile,
                        sender_nick.clone(),
                        info.get_local_profile_id(),
                        info.get_local_base_name(),
                        resp.message_text.clone(),
                    );
                    info.push_buddy_message(buddy_msg.clone());
                    let local_profile = info.get_local_profile_id();
                    let local_name = info.get_local_base_name();
                    drop(info);
                    insert_chat_with_local(buddy_msg.clone(), local_profile, local_name);

                    if let Some(audio) = TheAudio::get() {
                        let event = AudioEventRts::new("GUIMessageReceived".to_string());
                        let _ = audio.add_audio_event(&event);
                    }

                    let mut snippet = buddy_msg.message.clone();
                    if snippet.len() > 11 {
                        snippet.truncate(11);
                    }
                    let notification = GameText::fetch("Buddy:MessageNotification")
                        .replace("%s", sender_nick.as_str())
                        .replace("%2", &snippet);
                    let state_slot = wol_buddy_state();
                    let mut state = state_slot.borrow_mut();
                    state.last_notification_was_status = false;
                    state.num_online_in_notification = 0;
                    drop(state);
                    show_notification_box(AsciiString::new(), &notification);
                }
                BuddyResponseType::Request => {
                    info.add_buddy_request(
                        resp.profile,
                        resp.request_nick.clone(),
                        resp.request_email.clone(),
                        resp.request_country_code.clone(),
                    );
                    update_buddy_info();

                    let state_slot = wol_buddy_state();
                    let mut state = state_slot.borrow_mut();
                    state.last_notification_was_status = false;
                    state.num_online_in_notification = 0;
                    drop(state);
                    show_notification_box(
                        AsciiString::from(resp.request_nick.as_str()),
                        &GameText::fetch("Buddy:AddNotification"),
                    );
                }
                BuddyResponseType::Status => {
                    let seen_previously = info.get_buddy_map().contains_key(&resp.profile);
                    let old_status = info
                        .get_buddy_map()
                        .get(&resp.profile)
                        .map(|info| info.status)
                        .unwrap_or(GameSpyBuddyStatus::Offline);

                    info.update_buddy_status(
                        resp.profile,
                        resp.status_nick.clone(),
                        resp.status_email.clone(),
                        resp.status_country_code.clone(),
                        resp.status_location.clone(),
                        resp.status_value,
                        resp.status_string.clone(),
                    );
                    update_buddy_info();
                    populate_lobby_player_listbox();
                    refresh_game_list_boxes();

                    let new_status = info
                        .get_buddy_map()
                        .get(&resp.profile)
                        .map(|info| info.status)
                        .unwrap_or(GameSpyBuddyStatus::Offline);

                    if (new_status == GameSpyBuddyStatus::Offline && seen_previously)
                        || (new_status == GameSpyBuddyStatus::Online
                            && (old_status == GameSpyBuddyStatus::Offline || !seen_previously))
                    {
                        let marker = format!("Buddy:{}Notification", resp.status_string);
                        let state_slot = wol_buddy_state();
                        let mut state = state_slot.borrow_mut();
                        state.last_notification_was_status = true;
                        if new_status != GameSpyBuddyStatus::Offline {
                            state.num_online_in_notification += 1;
                        }
                        drop(state);
                        show_notification_box(
                            AsciiString::from(resp.status_nick.as_str()),
                            &GameText::fetch(&marker),
                        );
                    } else if new_status == GameSpyBuddyStatus::Lobby && !seen_previously {
                        let state_slot = wol_buddy_state();
                        let mut state = state_slot.borrow_mut();
                        state.last_notification_was_status = true;
                        if new_status != GameSpyBuddyStatus::Offline {
                            state.num_online_in_notification += 1;
                        }
                        drop(state);
                        show_notification_box(
                            AsciiString::from(resp.status_nick.as_str()),
                            &GameText::fetch("Buddy:OnlineNotification"),
                        );
                    }
                }
            }
        }
    }

    let expired = {
        let slot = wol_buddy_state();
        let state = slot.borrow();
        state.notice_layout.is_some() && now_ms() > state.notice_expires
    };
    if expired {
        delete_notification_box();
    }
}

fn show_notification_box(nick: AsciiString, message: &str) {
    let state_slot = wol_buddy_state();
    let mut state = state_slot.borrow_mut();

    if state.notice_layout.is_none() {
        let layout = with_window_manager(|manager| {
            manager
                .create_layout_with_windows("Menus/PopupBuddyListNotification.wnd")
                .ok()
        });
        if let Some((layout, _)) = layout {
            layout.borrow().run_init(None);
            state.notice_layout = Some(layout);
        }
    }

    let layout = state.notice_layout.clone();
    let Some(layout) = layout else {
        return;
    };
    layout.borrow_mut().hide(false);

    if state.button_notification_id == 0 {
        state.button_notification_id =
            name_to_id("PopupBuddyListNotification.wnd:ButtonNotification");
    }

    let notification_window =
        with_window_manager(|manager| manager.get_window_by_id(state.button_notification_id));

    let Some(window) = notification_window else {
        drop(state);
        delete_notification_box();
        return;
    };

    let mut final_message = message.to_string();
    if state.last_notification_was_status && state.num_online_in_notification > 1 {
        final_message = GameText::fetch("Buddy:MultipleOnlineNotification");
    }

    if !nick.as_str().is_empty() {
        final_message = final_message.replace("%s", nick.as_str());
    }

    let _ = window.borrow_mut().set_text(&final_message);
    state.notice_expires = now_ms() + NOTIFICATION_EXPIRES_MS;
    drop(state);
    layout.borrow_mut().bring_forward();

    if let Some(audio) = TheAudio::get() {
        let event = AudioEventRts::new("GUICommunicatorIncoming".to_string());
        let _ = audio.add_audio_event(&event);
    }
}

fn delete_notification_box() {
    let state_slot = wol_buddy_state();
    let mut state = state_slot.borrow_mut();
    state.last_notification_was_status = false;
    state.num_online_in_notification = 0;
    if let Some(layout) = state.notice_layout.take() {
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
}

fn populate_old_buddy_messages() {
    let messages = get_gamespy_info()
        .and_then(|info| {
            info.lock()
                .ok()
                .map(|guard| guard.get_buddy_messages().clone())
        })
        .unwrap_or_default();

    for msg in messages {
        insert_chat(msg);
    }
}

pub fn wol_buddy_overlay_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_slot = wol_buddy_state();
    let mut state = state_slot.borrow_mut();

    state.parent_id = name_to_id("WOLBuddyOverlay.wnd:BuddyMenuParent");
    state.button_hide_id = name_to_id("WOLBuddyOverlay.wnd:ButtonHide");
    state.button_add_buddy_id = name_to_id("WOLBuddyOverlay.wnd:ButtonAdd");
    state.button_delete_buddy_id = name_to_id("WOLBuddyOverlay.wnd:ButtonDelete");
    state.button_accept_buddy_id = name_to_id("WOLBuddyOverlay.wnd:ButtonYes");
    state.button_deny_buddy_id = name_to_id("WOLBuddyOverlay.wnd:ButtonNo");
    state.radio_button_buddies_id = name_to_id("WOLBuddyOverlay.wnd:RadioButtonBuddies");
    state.radio_button_ignore_id = name_to_id("WOLBuddyOverlay.wnd:RadioButtonIgnore");
    state.parent_buddies_id = name_to_id("WOLBuddyOverlay.wnd:BuddiesParent");
    state.parent_ignore_id = name_to_id("WOLBuddyOverlay.wnd:IgnoreParent");
    state.listbox_ignore_id = name_to_id("WOLBuddyOverlay.wnd:ListboxIgnore");

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        state.button_hide = manager.get_window_by_id(state.button_hide_id);
        state.button_add_buddy = manager.get_window_by_id(state.button_add_buddy_id);
        state.button_delete_buddy = manager.get_window_by_id(state.button_delete_buddy_id);
        state.button_accept_buddy = manager.get_window_by_id(state.button_accept_buddy_id);
        state.button_deny_buddy = manager.get_window_by_id(state.button_deny_buddy_id);
        state.radio_button_buddies = manager.get_window_by_id(state.radio_button_buddies_id);
        state.radio_button_ignore = manager.get_window_by_id(state.radio_button_ignore_id);
        state.parent_buddies = manager.get_window_by_id(state.parent_buddies_id);
        state.parent_ignore = manager.get_window_by_id(state.parent_ignore_id);
        state.listbox_ignore = manager.get_window_by_id(state.listbox_ignore_id);
    });

    init_buddy_controls(BuddyWindowType::Buddies);

    if let Some(radio) = state.radio_button_buddies.as_ref() {
        if let Some(widget) = radio.borrow_mut().widget_mut() {
            if let WindowWidget::RadioButton(rb) = widget {
                rb.select();
            }
        }
    }

    if let Some(parent) = state.parent_buddies.as_ref() {
        let _ = parent.borrow_mut().hide(false);
    }
    if let Some(parent) = state.parent_ignore.as_ref() {
        let _ = parent.borrow_mut().hide(true);
    }

    populate_old_buddy_messages();

    layout.hide(false);

    if let Some(parent) = state.parent.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
    }

    state.is_overlay_active = true;
    drop(state);
    update_buddy_info();
}

pub fn wol_buddy_overlay_shutdown(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_slot = wol_buddy_state();
    let mut state = state_slot.borrow_mut();
    state.listbox_ignore = None;
    layout.hide(true);
    state.is_overlay_active = false;
    init_buddy_controls(BuddyWindowType::ResetAll);
}

pub fn wol_buddy_overlay_update(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let connected = get_buddy_message_queue()
        .and_then(|queue| queue.lock().ok().map(|queue| queue.is_connected()))
        .unwrap_or(false);
    if !connected {
        close_overlay(GameSpyOverlayType::Buddy);
    }
}

pub fn wol_buddy_overlay_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    // C++ WOLBuddyOverlay.cpp:796-811 — KEY_ESC synthesizes Hide GBM_SELECTED.
    if (data2 & KEY_STATE_UP) != 0 {
        let (parent, hide_id) = {
            let slot = wol_buddy_state();
            let state = slot.borrow();
            (state.parent.clone(), state.button_hide_id)
        };
        crate::gui::callbacks::online_callback_support::dispatch_esc_gadget_selected(
            parent, hide_id,
        );
    }
    WindowMsgHandled::Handled
}

fn refresh_ignore_list() {
    let state_slot = wol_buddy_state();
    let mut state = state_slot.borrow_mut();
    let Some(listbox_window) = state.listbox_ignore.as_ref() else {
        return;
    };
    let mut listbox_guard = listbox_window.borrow_mut();
    let Some(listbox) = listbox_guard.list_box_mut() else {
        return;
    };

    listbox.clear();
    let Some((saved_ignore, ignore_list)) =
        crate::gui::callbacks::online_callback_support::with_gamespy_info(|info| {
            (info.return_saved_ignore_list(), info.return_ignore_list())
        })
    else {
        return;
    };

    for (profile_id, name) in saved_ignore {
        let index = listbox.add_item_with_color(
            name.as_str(),
            packed_ui_color(make_color(255, 100, 100, 255)),
        );
        let _ = listbox.set_item_data(index, Some(ListBoxItemData::Integer(profile_id)));
    }

    for name in ignore_list {
        let index =
            listbox.add_item_with_color(&name, packed_ui_color(make_color(255, 100, 100, 255)));
        let _ = listbox.set_item_data(index, Some(ListBoxItemData::Integer(0)));
    }
}

fn set_unignore_text(layout: &WindowLayout, nick: &AsciiString, profile_id: GPProfile) {
    let control_name = format!(
        "{}:ButtonIgnore",
        layout.get_filename().trim_start_matches("Menus/")
    );
    let id = name_to_id(&control_name);
    let window = with_window_manager(|manager| manager.get_window_by_id(id));
    let Some(window) = window else {
        return;
    };

    if let Some(slot) = get_gamespy_info() {
        if let Ok(info) = slot.lock() {
            if info.is_saved_ignored(profile_id) || info.is_ignored(nick.clone()) {
                let _ = window
                    .borrow_mut()
                    .set_text(&GameText::fetch("GUI:Unignore"));
            }
        }
    }
}

fn close_right_click_menu(window: &GameWindow) {
    let layout = window.get_layout();
    if let Some(layout) = layout {
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
    let slot = wol_buddy_state();
    let mut state = slot.borrow_mut();
    state.rc_menu = None;
    state.rc_layout = None;
}

fn request_buddy_add(profile_id: GPProfile, nick: &AsciiString) {
    let mut req = BuddyRequest::default();
    req.request_type = BuddyRequestType::AddBuddy;
    req.id = profile_id;
    req.message = GameText::fetch("GUI:BuddyAddReq");
    if let Some(queue) = get_buddy_message_queue() {
        if let Ok(mut queue) = queue.lock() {
            queue.add_request(req);
        }
    }

    let mut exists = true;
    let mut invite = GameText::fetch("Buddy:InviteSent");
    if invite.is_empty() {
        exists = false;
    }
    if !exists {
        return;
    }

    if let Some(slot) = get_gamespy_info() {
        if let Ok(mut info) = slot.lock() {
            let local_profile = info.get_local_profile_id();
            let local_name = info.get_local_base_name();
            let message = BuddyMessage::new(
                0,
                AsciiString::new(),
                local_profile,
                local_name.clone(),
                GameText::fetch("Buddy:InviteSentToPlayer").replace("%s", nick.as_str()),
            );
            info.push_buddy_message(message.clone());
            drop(info);
            insert_chat_with_local(message.clone(), local_profile, local_name);
        }
    }

    if let Some(audio) = TheAudio::get() {
        let event = AudioEventRts::new("GUIMessageReceived".to_string());
        let _ = audio.add_audio_event(&event);
    }

    let state_slot = wol_buddy_state();
    let mut state = state_slot.borrow_mut();
    state.last_notification_was_status = false;
    state.num_online_in_notification = 0;
    drop(state);
    show_notification_box(AsciiString::new(), &invite);
}

pub fn wol_buddy_overlay_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if buddy_control_system(window, msg, data1).is_handled() {
        return WindowMsgHandled::Handled;
    }

    match msg {
        WindowMessage::Create | WindowMessage::Destroy => return WindowMsgHandled::Handled,
        WindowMessage::InputFocus => return write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetRightClick => {
            let (listbox_ignore_id, listbox_ignore) = {
                let state_slot = wol_buddy_state();
                let state = state_slot.borrow_mut();
                (state.listbox_ignore_id, state.listbox_ignore.clone())
            };
            let control_id = data1 as i32;
            if control_id != listbox_ignore_id {
                return WindowMsgHandled::Ignored;
            }
            let Some(listbox_window) = listbox_ignore.as_ref() else {
                return WindowMsgHandled::Handled;
            };
            let (rc, mouse_x, mouse_y) = match listbox_right_click_info(&listbox_window.borrow()) {
                Some(info) => info,
                None => return WindowMsgHandled::Handled,
            };
            if rc.index < 0 {
                return WindowMsgHandled::Handled;
            }

            let index = rc.index as usize;
            let profile_id = listbox_window
                .borrow()
                .widget()
                .and_then(|widget| match widget {
                    WindowWidget::ListBox(listbox) => listbox.get_item_data(index),
                    _ => None,
                })
                .and_then(|data| match data {
                    ListBoxItemData::Integer(id) => Some(*id),
                    _ => None,
                })
                .unwrap_or(0);

            let nick = listbox_item_text(&listbox_window.borrow(), index)
                .map(|s| AsciiString::from(s.as_str()))
                .unwrap_or_default();

            let mut is_buddy = false;
            let mut is_request = false;
            if let Some(slot) = get_gamespy_info() {
                if let Ok(info) = slot.lock() {
                    if info.get_buddy_map().contains_key(&profile_id) {
                        is_buddy = true;
                    } else if info.get_buddy_request_map().contains_key(&profile_id) {
                        is_request = true;
                    }
                }
            }

            if let Some(widget) = listbox_window.borrow_mut().list_box_mut() {
                widget.set_selected_indices(&[index]);
            }

            let layout_name = if is_buddy {
                "Menus/RCBuddiesMenu.wnd"
            } else if is_request {
                "Menus/RCBuddyRequestMenu.wnd"
            } else {
                "Menus/RCNonBuddiesMenu.wnd"
            };

            let layout =
                with_window_manager(|manager| manager.create_layout_with_windows(layout_name).ok());
            if let Some((layout, _)) = layout {
                layout.borrow().run_init(None);
                let rc_menu = layout.borrow().get_first_window();
                if let Some(rc_menu) = rc_menu {
                    rc_menu.borrow_mut().hide(false);
                    rc_menu.borrow_mut().bring_to_front();
                    let (win_w, win_h) = rc_menu.borrow().get_size();
                    let (screen_w, screen_h) = with_window_manager(|manager| manager.screen_size());
                    let mut pos_x = mouse_x;
                    let mut pos_y = mouse_y;
                    if pos_x + win_w > screen_w {
                        pos_x = screen_w - win_w;
                    }
                    if pos_y + win_h > screen_h {
                        pos_y = screen_h - win_h;
                    }
                    let _ = rc_menu.borrow_mut().set_position(pos_x, pos_y);

                    let rc_data = GameSpyRcMenuData {
                        id: profile_id,
                        nick: nick.clone(),
                        item_type: if is_buddy {
                            RcItemType::Buddy
                        } else if is_request {
                            RcItemType::Request
                        } else {
                            RcItemType::NonBuddy
                        },
                    };
                    set_unignore_text(&layout.borrow(), &rc_data.nick, rc_data.id);
                    rc_menu.borrow_mut().set_user_data(rc_data);
                    with_window_manager(|manager| manager.set_lone_window(Some(&rc_menu)));
                    let state_slot = wol_buddy_state();
                    let mut state = state_slot.borrow_mut();
                    state.rc_menu = Some(rc_menu);
                    state.rc_layout = Some(layout);
                }
            }
            return WindowMsgHandled::Handled;
        }
        WindowMessage::GadgetSelected => {
            let (button_hide_id, radio_buddies_id, radio_ignore_id, parent_buddies, parent_ignore) = {
                let state_slot = wol_buddy_state();
                let state = state_slot.borrow_mut();
                (
                    state.button_hide_id,
                    state.radio_button_buddies_id,
                    state.radio_button_ignore_id,
                    state.parent_buddies.clone(),
                    state.parent_ignore.clone(),
                )
            };
            let control_id = data1 as i32;
            if control_id == button_hide_id {
                close_overlay(GameSpyOverlayType::Buddy);
            } else if control_id == radio_buddies_id {
                if let Some(parent) = parent_buddies.as_ref() {
                    let _ = parent.borrow_mut().hide(false);
                }
                if let Some(parent) = parent_ignore.as_ref() {
                    let _ = parent.borrow_mut().hide(true);
                }
            } else if control_id == radio_ignore_id {
                if let Some(parent) = parent_buddies.as_ref() {
                    let _ = parent.borrow_mut().hide(true);
                }
                if let Some(parent) = parent_ignore.as_ref() {
                    let _ = parent.borrow_mut().hide(false);
                }
                refresh_ignore_list();
            }
            return WindowMsgHandled::Handled;
        }
        WindowMessage::GadgetEditDone => return WindowMsgHandled::Handled,
        _ => return WindowMsgHandled::Ignored,
    }
}

pub fn popup_buddy_notification_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::GadgetSelected {
        return WindowMsgHandled::Ignored;
    }
    let control_id = data1 as i32;
    let state_slot = wol_buddy_state();
    let state = state_slot.borrow_mut();
    if control_id == state.button_notification_id {
        open_overlay(GameSpyOverlayType::Buddy);
        return WindowMsgHandled::Handled;
    }
    WindowMsgHandled::Ignored
}

pub fn wol_buddy_overlay_rc_menu_init(
    layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
) {
    let base = layout.get_filename().trim_start_matches("Menus/");
    let add_name = format!("{base}:ButtonAdd");
    let delete_name = format!("{base}:ButtonDelete");
    let play_name = format!("{base}:ButtonPlay");
    let ignore_name = format!("{base}:ButtonIgnore");
    let stats_name = format!("{base}:ButtonStats");

    let state_slot = wol_buddy_state();
    let mut state = state_slot.borrow_mut();
    state.rc_button_add_id = name_to_id(&add_name);
    state.rc_button_delete_id = name_to_id(&delete_name);
    state.rc_button_play_id = name_to_id(&play_name);
    state.rc_button_ignore_id = name_to_id(&ignore_name);
    state.rc_button_stats_id = name_to_id(&stats_name);
}

pub fn wol_buddy_overlay_rc_menu_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::User(code) if code == GGM_CLOSE => {
            close_right_click_menu(window);
            return WindowMsgHandled::Handled;
        }
        WindowMessage::Create => return WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            let rc_data = window.get_user_data::<GameSpyRcMenuData>().cloned();
            let Some(rc_data) = rc_data else {
                return WindowMsgHandled::Handled;
            };

            let profile_id = rc_data.id;
            let nick = rc_data.nick.clone();
            let is_buddy = rc_data.item_type == RcItemType::Buddy;
            let is_request = rc_data.item_type == RcItemType::Request;
            let is_gamespy_user = profile_id > 0;

            let (rc_add, rc_delete, rc_ignore, rc_stats) = {
                let state_slot = wol_buddy_state();
                let state = state_slot.borrow_mut();
                (
                    state.rc_button_add_id,
                    state.rc_button_delete_id,
                    state.rc_button_ignore_id,
                    state.rc_button_stats_id,
                )
            };

            if control_id == rc_add {
                if !is_gamespy_user {
                    close_right_click_menu(window);
                    return WindowMsgHandled::Handled;
                }
                if is_request {
                    let mut req = BuddyRequest::default();
                    req.request_type = BuddyRequestType::OkAdd;
                    req.id = profile_id;
                    if let Some(queue) = get_buddy_message_queue() {
                        if let Ok(mut queue) = queue.lock() {
                            queue.add_request(req);
                        }
                    }
                    if let Some(slot) = get_gamespy_info() {
                        if let Ok(mut info) = slot.lock() {
                            info.remove_buddy_request(profile_id);
                            if !info.is_buddy(profile_id) {
                                request_buddy_add(profile_id, &nick);
                            }
                        }
                    }
                    update_buddy_info();
                    populate_lobby_player_listbox();
                    refresh_game_list_boxes();
                } else if !is_buddy {
                    request_buddy_add(profile_id, &nick);
                }
            } else if control_id == rc_delete {
                if !is_gamespy_user {
                    close_right_click_menu(window);
                    return WindowMsgHandled::Handled;
                }
                if is_buddy {
                    let mut req = BuddyRequest::default();
                    req.request_type = BuddyRequestType::DelBuddy;
                    req.id = profile_id;
                    if let Some(queue) = get_buddy_message_queue() {
                        if let Ok(mut queue) = queue.lock() {
                            queue.add_request(req);
                        }
                    }
                } else {
                    let mut req = BuddyRequest::default();
                    req.request_type = BuddyRequestType::DenyAdd;
                    req.id = profile_id;
                    if let Some(queue) = get_buddy_message_queue() {
                        if let Ok(mut queue) = queue.lock() {
                            queue.add_request(req);
                        }
                    }
                }

                if let Some(slot) = get_gamespy_info() {
                    if let Ok(mut info) = slot.lock() {
                        info.remove_buddy(profile_id);
                        info.remove_buddy_request(profile_id);
                    }
                }
                update_buddy_info();
                populate_lobby_player_listbox();
                refresh_game_list_boxes();
            } else if control_id == rc_ignore {
                if is_gamespy_user {
                    if let Some(slot) = get_gamespy_info() {
                        if let Ok(mut info) = slot.lock() {
                            if info.is_saved_ignored(profile_id) {
                                info.remove_from_saved_ignore_list(profile_id);
                            } else {
                                info.add_to_saved_ignore_list(profile_id, nick.clone());
                            }
                        }
                    }
                } else if let Some(slot) = get_gamespy_info() {
                    if let Ok(mut info) = slot.lock() {
                        if info.is_ignored(nick.clone()) {
                            info.remove_from_ignore_list(nick.clone());
                        } else {
                            info.add_to_ignore_list(nick.clone());
                        }
                    }
                }
                update_buddy_info();
                populate_lobby_player_listbox();
                refresh_ignore_list();
                refresh_game_list_boxes();
            } else if control_id == rc_stats {
                crate::gui::callbacks::wol_welcome_menu::set_look_at_player(
                    profile_id,
                    nick.as_str(),
                );
                close_overlay(GameSpyOverlayType::PlayerInfo);
                open_overlay(GameSpyOverlayType::PlayerInfo);
                let mut req = PSRequest::default();
                req.request_type = PSRequestType::ReadPlayerStats;
                req.player.id = profile_id;
                if let Some(queue) = get_ps_message_queue() {
                    if let Ok(mut queue) = queue.lock() {
                        queue.add_request(req);
                    }
                }
            }

            close_right_click_menu(window);
            return WindowMsgHandled::Handled;
        }
        _ => return WindowMsgHandled::Ignored,
    }
}

fn buddy_control_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
) -> WindowMsgHandled {
    let (
        listbox_buddies_id,
        text_entry_edit_id,
        listbox_buddies,
        listbox_chat,
        text_entry_edit,
        is_init,
    ) = {
        let controls_slot = buddy_controls();
        let controls = controls_slot.borrow_mut();
        (
            controls.listbox_buddies_id,
            controls.text_entry_edit_id,
            controls.listbox_buddies.clone(),
            controls.listbox_chat.clone(),
            controls.text_entry_edit.clone(),
            controls.is_init,
        )
    };

    let Some((local_profile_id, local_name, buddy_names)) =
        crate::gui::callbacks::online_callback_support::with_gamespy_info(|info| {
            let names = info
                .get_buddy_map()
                .iter()
                .map(|(id, buddy)| (*id, buddy.name.clone()))
                .collect::<std::collections::HashMap<_, _>>();
            (
                info.get_local_profile_id(),
                info.get_local_base_name().clone(),
                names,
            )
        })
    else {
        return WindowMsgHandled::Ignored;
    };

    if local_profile_id == 0 || !is_init {
        return WindowMsgHandled::Ignored;
    }

    match msg {
        WindowMessage::GadgetRightClick => {
            let control_id = data1 as i32;
            if control_id != listbox_buddies_id {
                return WindowMsgHandled::Ignored;
            }
            let Some(listbox_window) = listbox_buddies.as_ref() else {
                return WindowMsgHandled::Handled;
            };
            let (rc, mouse_x, mouse_y) = match listbox_right_click_info(&listbox_window.borrow()) {
                Some(info) => info,
                None => return WindowMsgHandled::Handled,
            };
            if rc.index < 0 {
                return WindowMsgHandled::Handled;
            }

            let index = rc.index as usize;
            let profile_id = listbox_window
                .borrow()
                .widget()
                .and_then(|widget| match widget {
                    WindowWidget::ListBox(listbox) => listbox.get_item_column_user_data(index, 0),
                    _ => None,
                })
                .and_then(|data| match data {
                    ListBoxItemData::Integer(id) => Some(*id),
                    _ => None,
                })
                .unwrap_or(0);

            let item_type = listbox_window
                .borrow()
                .widget()
                .and_then(|widget| match widget {
                    WindowWidget::ListBox(listbox) => listbox.get_item_column_user_data(index, 1),
                    _ => None,
                })
                .and_then(|data| match data {
                    ListBoxItemData::Integer(0) => Some(RcItemType::Buddy),
                    ListBoxItemData::Integer(1) => Some(RcItemType::Request),
                    _ => Some(RcItemType::NonBuddy),
                })
                .unwrap_or(RcItemType::NonBuddy);

            let nick = listbox_item_text(&listbox_window.borrow(), index)
                .map(|s| AsciiString::from(s.as_str()))
                .unwrap_or_default();

            if let Some(widget) = listbox_window.borrow_mut().list_box_mut() {
                widget.set_selected_indices(&[index]);
            }

            let layout_name = match item_type {
                RcItemType::Buddy => "Menus/RCBuddiesMenu.wnd",
                RcItemType::Request => "Menus/RCBuddyRequestMenu.wnd",
                RcItemType::NonBuddy => "Menus/RCNonBuddiesMenu.wnd",
            };
            let layout =
                with_window_manager(|manager| manager.create_layout_with_windows(layout_name).ok());
            if let Some((layout, _)) = layout {
                layout.borrow().run_init(None);
                let rc_menu = layout.borrow().get_first_window();
                if let Some(rc_menu) = rc_menu {
                    rc_menu.borrow_mut().hide(false);
                    rc_menu.borrow_mut().bring_to_front();
                    let (win_w, win_h) = rc_menu.borrow().get_size();
                    let (screen_w, screen_h) = with_window_manager(|manager| manager.screen_size());
                    let mut pos_x = mouse_x;
                    let mut pos_y = mouse_y;
                    if pos_x + win_w > screen_w {
                        pos_x = screen_w - win_w;
                    }
                    if pos_y + win_h > screen_h {
                        pos_y = screen_h - win_h;
                    }
                    let _ = rc_menu.borrow_mut().set_position(pos_x, pos_y);

                    let rc_data = GameSpyRcMenuData {
                        id: profile_id,
                        nick: nick.clone(),
                        item_type,
                    };
                    set_unignore_text(&layout.borrow(), &rc_data.nick, rc_data.id);
                    rc_menu.borrow_mut().set_user_data(rc_data);
                    with_window_manager(|manager| manager.set_lone_window(Some(&rc_menu)));
                    let state_slot = wol_buddy_state();
                    let mut state = state_slot.borrow_mut();
                    state.rc_menu = Some(rc_menu);
                    state.rc_layout = Some(layout);
                }
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => {
            let control_id = data1 as i32;
            if control_id != text_entry_edit_id {
                return WindowMsgHandled::Ignored;
            }

            let Some(listbox_window) = listbox_buddies.as_ref() else {
                return WindowMsgHandled::Handled;
            };
            let selected = listbox_selected_profile(&listbox_window.borrow())
                .map(|(idx, profile, _)| (idx, profile));
            if selected.is_none() {
                if let Some(listbox_window) = listbox_chat.as_ref() {
                    if let Some(listbox) = listbox_window.borrow_mut().list_box_mut() {
                        listbox.add_item_with_color(
                            &GameText::fetch("Buddy:SelectBuddyToChat"),
                            packed_ui_color(
                                default_gamespy_colors()[GameSpyColor::Default as usize],
                            ),
                        );
                    }
                }
                return WindowMsgHandled::Handled;
            }

            let (selected_index, selected_profile) = selected.unwrap();
            let recipient_nick = buddy_names
                .get(&selected_profile)
                .cloned()
                .unwrap_or_default();

            let in_progress =
                with_gamespy_game_info(|game| game.is_in_game() && game.is_game_in_progress());
            if in_progress {
                if let Ok(list) = gamelogic::player::ThePlayerList().read() {
                    if let Some(player) = list.get_local_player() {
                        if let Ok(player) = player.read() {
                            if !player.is_player_active() {
                                let mut in_same_game = false;
                                let nick_str = recipient_nick.as_str().to_string();
                                with_gamespy_game_info(|game| {
                                    for i in 0..game_network::MAX_SLOTS {
                                        if let Some(slot) = game.get_slot(i) {
                                            if slot.get_name() == nick_str {
                                                in_same_game = true;
                                                break;
                                            }
                                        }
                                    }
                                });
                                if in_same_game {
                                    if let Some(listbox_window) = listbox_chat.as_ref() {
                                        if let Some(listbox) =
                                            listbox_window.borrow_mut().list_box_mut()
                                        {
                                            listbox.add_item_with_color(
                                                &GameText::fetch("Buddy:CantTalkToIngameBuddy"),
                                                packed_ui_color(
                                                    default_gamespy_colors()
                                                        [GameSpyColor::Default as usize],
                                                ),
                                            );
                                        }
                                    }
                                    return WindowMsgHandled::Handled;
                                }
                            }
                        }
                    }
                }
            }

            let entry = text_entry_edit.as_ref();
            let Some(entry) = entry else {
                return WindowMsgHandled::Handled;
            };
            let mut entry_guard = entry.borrow_mut();
            let Some(widget) = entry_guard.text_entry_mut() else {
                return WindowMsgHandled::Handled;
            };
            let mut text = widget.text().trim().to_string();
            widget.set_text("");
            if text.is_empty() {
                return WindowMsgHandled::Handled;
            }

            if text.len() >= MAX_BUDDY_CHAT_LEN {
                text.truncate(MAX_BUDDY_CHAT_LEN - 1);
            }

            let mut req = BuddyRequest::default();
            req.request_type = BuddyRequestType::Message;
            req.recipient = selected_profile;
            req.message = text.clone();
            if let Some(queue) = get_buddy_message_queue() {
                if let Ok(mut queue) = queue.lock() {
                    queue.add_request(req);
                }
            }

            let buddy_msg = BuddyMessage::new(
                local_profile_id,
                local_name.clone(),
                selected_profile,
                recipient_nick,
                text,
            );
            let _ = crate::gui::callbacks::online_callback_support::with_gamespy_info_mut(|info| {
                info.push_buddy_message(buddy_msg.clone());
            });
            insert_chat_with_local(buddy_msg, local_profile_id, local_name);

            let _ = selected_index;
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_string_from_owned_listbox_text_uses_as_str() {
        let text = String::from("NickName");
        let nick = AsciiString::from(text.as_str());
        assert_eq!(nick.as_str(), "NickName");
    }

    #[test]
    fn rc_item_type_discriminants_are_stable() {
        assert_eq!(RcItemType::Buddy as i32, 0);
        assert_eq!(RcItemType::Request as i32, 1);
        assert_eq!(RcItemType::NonBuddy as i32, 2);
    }
}
