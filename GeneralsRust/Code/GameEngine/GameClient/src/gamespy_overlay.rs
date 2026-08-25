//! GameSpy overlay state and layout helpers.

use crate::gui::callbacks::message_box::{
    MessageBoxFunc, message_box_ok, message_box_ok_cancel, message_box_yes_no,
};
use crate::gui::{GameWindow, WindowLayout, with_window_manager};
use game_network::gamespy::buddy_thread::{
    BuddyRequest, BuddyRequestType, get_buddy_message_queue,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameSpyOverlayType {
    PlayerInfo,
    MapSelect,
    Buddy,
    Page,
    GameOptions,
    GamePassword,
    LadderSelect,
    LocaleSelect,
    Options,
}

#[derive(Debug, Clone, Default)]
pub struct GameSpyHostRequest {
    pub game_name: String,
    pub game_description: String,
    pub game_password: String,
    pub allow_observers: bool,
    pub use_stats: bool,
    pub limit_armies: bool,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub game_version: u32,
    pub restrict_game_list: bool,
    pub ladder_ip: String,
    pub ladder_port: u16,
    pub host_ping_str: String,
}

#[derive(Debug, Clone)]
pub struct GameSpyStagingRoom {
    pub id: i32,
    pub game_name: String,
}

#[derive(Default)]
struct GameSpyOverlayState {
    overlays: HashMap<GameSpyOverlayType, Rc<RefCell<WindowLayout>>>,
    lobby_attempt_host_join: bool,
    current_staging_room_id: Option<i32>,
    staging_rooms: HashMap<i32, GameSpyStagingRoom>,
    last_join_request: Option<(i32, String)>,
    last_host_request: Option<GameSpyHostRequest>,
    message_box_window: Option<Rc<RefCell<GameWindow>>>,
    message_box_ok: Option<MessageBoxFunc>,
    message_box_cancel: Option<MessageBoxFunc>,
    reopen_player_info: bool,
}

thread_local! {
    static OVERLAY_STATE: Rc<RefCell<GameSpyOverlayState>> = Rc::new(RefCell::new(GameSpyOverlayState::default()));
}

fn overlay_state() -> Rc<RefCell<GameSpyOverlayState>> {
    OVERLAY_STATE.with(Rc::clone)
}

fn overlay_script(overlay: GameSpyOverlayType) -> &'static str {
    match overlay {
        GameSpyOverlayType::PlayerInfo => "Menus/PopupPlayerInfo.wnd",
        GameSpyOverlayType::MapSelect => "Menus/WOLMapSelectMenu.wnd",
        GameSpyOverlayType::Buddy => "Menus/WOLBuddyOverlay.wnd",
        GameSpyOverlayType::Page => "Menus/WOLPageOverlay.wnd",
        GameSpyOverlayType::GameOptions => "Menus/PopupHostGame.wnd",
        GameSpyOverlayType::GamePassword => "Menus/PopupJoinGame.wnd",
        GameSpyOverlayType::LadderSelect => "Menus/PopupLadderSelect.wnd",
        GameSpyOverlayType::LocaleSelect => "Menus/PopupLocaleSelect.wnd",
        GameSpyOverlayType::Options => "Menus/OptionsMenu.wnd",
    }
}

fn clear_gs_message_boxes() {
    let window = {
        let state_slot = overlay_state();
        let mut state = state_slot.borrow_mut();
        state.message_box_ok = None;
        state.message_box_cancel = None;
        state.message_box_window.take()
    };

    if let Some(window) = window {
        with_window_manager(|manager| {
            let _ = manager.destroy_window(window);
        });
    }
}

fn message_box_ok_clicked() {
    let callback = {
        let state_slot = overlay_state();
        let mut state = state_slot.borrow_mut();
        state.message_box_window = None;
        state.message_box_ok.take()
    };
    if let Some(mut cb) = callback {
        cb();
    }
}

fn message_box_cancel_clicked() {
    let callback = {
        let state_slot = overlay_state();
        let mut state = state_slot.borrow_mut();
        state.message_box_window = None;
        state.message_box_cancel.take()
    };
    if let Some(mut cb) = callback {
        cb();
    }
}

pub fn gs_message_box_ok(title: &str, body: &str, ok_callback: Option<MessageBoxFunc>) {
    clear_gs_message_boxes();
    let window = message_box_ok(title, body, Some(Box::new(message_box_ok_clicked)));
    let state_slot = overlay_state();
    let mut state = state_slot.borrow_mut();
    state.message_box_window = window;
    state.message_box_ok = ok_callback;
}

pub fn gs_message_box_ok_cancel(
    title: &str,
    body: &str,
    ok_callback: Option<MessageBoxFunc>,
    cancel_callback: Option<MessageBoxFunc>,
) {
    clear_gs_message_boxes();
    let window = message_box_ok_cancel(
        title,
        body,
        Some(Box::new(message_box_ok_clicked)),
        Some(Box::new(message_box_cancel_clicked)),
    );
    let state_slot = overlay_state();
    let mut state = state_slot.borrow_mut();
    state.message_box_window = window;
    state.message_box_ok = ok_callback;
    state.message_box_cancel = cancel_callback;
}

pub fn gs_message_box_yes_no(
    title: &str,
    body: &str,
    yes_callback: Option<MessageBoxFunc>,
    no_callback: Option<MessageBoxFunc>,
) {
    clear_gs_message_boxes();
    let window = message_box_yes_no(
        title,
        body,
        Some(Box::new(message_box_ok_clicked)),
        Some(Box::new(message_box_cancel_clicked)),
    );
    let state_slot = overlay_state();
    let mut state = state_slot.borrow_mut();
    state.message_box_window = window;
    state.message_box_ok = yes_callback;
    state.message_box_cancel = no_callback;
}

pub fn raise_gs_message_box() {
    raise_overlays();
    let slot = overlay_state();
    let window = slot.borrow().message_box_window.clone();
    if let Some(window) = window {
        let _ = window.borrow_mut().bring_to_front();
    }
}

fn buddy_try_reconnect() {
    let Some(queue) = get_buddy_message_queue() else {
        return;
    };
    if let Ok(mut queue) = queue.lock() {
        let mut req = BuddyRequest::default();
        req.request_type = BuddyRequestType::Relogin;
        queue.add_request(req);
    }
}

pub fn open_overlay(overlay: GameSpyOverlayType) {
    if overlay == GameSpyOverlayType::Buddy {
        if let Some(queue) = get_buddy_message_queue() {
            if let Ok(queue) = queue.lock() {
                if !queue.is_connected() {
                    if queue.get_local_profile_id() != 0 {
                        gs_message_box_yes_no(
                            &crate::game_text::GameText::fetch("GUI:GPErrorTitle"),
                            &crate::game_text::GameText::fetch("GUI:GPDisconnected"),
                            Some(Box::new(buddy_try_reconnect)),
                            None,
                        );
                    } else {
                        gs_message_box_ok(
                            &crate::game_text::GameText::fetch("GUI:GPErrorTitle"),
                            &crate::game_text::GameText::fetch("GUI:GPNoProfile"),
                            None,
                        );
                    }
                    return;
                }
            }
        }
    }

    let layout = {
        let state_slot = overlay_state();
        let mut state = state_slot.borrow_mut();
        if let Some(layout) = state.overlays.get(&overlay).cloned() {
            layout.borrow_mut().hide(false);
            Some(layout)
        } else {
            let script = overlay_script(overlay);
            let layout = with_window_manager(|manager| {
                manager
                    .create_layout_with_windows(script)
                    .ok()
                    .map(|(layout, _)| layout)
            });
            if let Some(layout) = layout.clone() {
                layout.borrow().run_init(None);
                layout.borrow_mut().hide(false);
                state.overlays.insert(overlay, layout.clone());
            }
            layout
        }
    };

    if let Some(layout) = layout {
        for window in layout.borrow().windows() {
            let _ = window.borrow_mut().bring_to_front();
        }
    }
}

pub fn close_overlay(overlay: GameSpyOverlayType) {
    let layout = {
        let state_slot = overlay_state();
        let mut state = state_slot.borrow_mut();
        state.overlays.remove(&overlay)
    };
    if let Some(layout) = layout {
        layout.borrow().run_shutdown(None);
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
}

pub fn close_all_overlays() {
    let overlays = {
        let state_slot = overlay_state();
        let mut state = state_slot.borrow_mut();
        let overlays = state.overlays.drain().map(|(_, v)| v).collect::<Vec<_>>();
        overlays
    };
    for layout in overlays {
        layout.borrow().run_shutdown(None);
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
    clear_gs_message_boxes();
}

pub fn is_overlay_open(overlay: GameSpyOverlayType) -> bool {
    let slot = overlay_state();
    slot.borrow().overlays.contains_key(&overlay)
}

pub fn toggle_overlay(overlay: GameSpyOverlayType) {
    if is_overlay_open(overlay) {
        close_overlay(overlay);
    } else {
        open_overlay(overlay);
    }
}

pub fn update_overlays() {
    let slot = overlay_state();
    let overlays = slot.borrow().overlays.values().cloned().collect::<Vec<_>>();
    for layout in overlays {
        layout.borrow().run_update(None);
    }
}

fn raise_overlays() {
    let slot = overlay_state();
    let overlays = slot.borrow().overlays.values().cloned().collect::<Vec<_>>();
    for layout in overlays {
        for window in layout.borrow().windows() {
            let _ = window.borrow_mut().bring_to_front();
        }
    }
}

pub fn reopen_player_info() {
    let state_slot = overlay_state();
    let mut state = state_slot.borrow_mut();
    state.reopen_player_info = true;
}

pub fn check_reopen_player_info() {
    let reopen = {
        let state_slot = overlay_state();
        let mut state = state_slot.borrow_mut();
        if state.reopen_player_info {
            state.reopen_player_info = false;
            true
        } else {
            false
        }
    };
    if reopen {
        open_overlay(GameSpyOverlayType::PlayerInfo);
    }
}

pub fn set_overlay_visible(overlay: GameSpyOverlayType, visible: bool) {
    if visible {
        open_overlay(overlay);
    } else {
        close_overlay(overlay);
    }
}

pub fn is_overlay_visible(overlay: GameSpyOverlayType) -> bool {
    is_overlay_open(overlay)
}

pub fn set_lobby_attempt_host_join(enabled: bool) {
    let state_slot = overlay_state();
    let mut state = state_slot.borrow_mut();
    state.lobby_attempt_host_join = enabled;
}

pub fn lobby_attempt_host_join() -> bool {
    let slot = overlay_state();
    slot.borrow().lobby_attempt_host_join
}

pub fn set_current_staging_room_id(id: Option<i32>) {
    let state_slot = overlay_state();
    let mut state = state_slot.borrow_mut();
    state.current_staging_room_id = id;
}

pub fn current_staging_room_id() -> Option<i32> {
    let slot = overlay_state();
    slot.borrow().current_staging_room_id
}

pub fn register_staging_room(room: GameSpyStagingRoom) {
    let state_slot = overlay_state();
    let mut state = state_slot.borrow_mut();
    state.staging_rooms.insert(room.id, room);
}

pub fn remove_staging_room(id: i32) {
    let state_slot = overlay_state();
    let mut state = state_slot.borrow_mut();
    state.staging_rooms.remove(&id);
}

pub fn find_staging_room_by_id(id: i32) -> Option<GameSpyStagingRoom> {
    let slot = overlay_state();
    slot.borrow().staging_rooms.get(&id).cloned()
}

pub fn queue_join_request(room_id: i32, password: String) {
    let state_slot = overlay_state();
    let mut state = state_slot.borrow_mut();
    state.last_join_request = Some((room_id, password));
}

pub fn last_join_request() -> Option<(i32, String)> {
    let slot = overlay_state();
    slot.borrow().last_join_request.clone()
}

pub fn queue_host_request(request: GameSpyHostRequest) {
    let state_slot = overlay_state();
    let mut state = state_slot.borrow_mut();
    state.last_host_request = Some(request);
}

pub fn last_host_request() -> Option<GameSpyHostRequest> {
    let slot = overlay_state();
    slot.borrow().last_host_request.clone()
}
