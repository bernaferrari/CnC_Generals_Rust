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
pub(crate) struct GameSpyOverlayState {
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

#[cfg(test)]
thread_local! {
    static TEST_OVERLAY_STATE: std::cell::OnceCell<Rc<RefCell<GameSpyOverlayState>>> =
        const { std::cell::OnceCell::new() };
}

fn overlay_state_on_owner(
    owner: Option<&crate::gui::window_manager::WindowManagerOwner>,
) -> Rc<RefCell<GameSpyOverlayState>> {
    if let Some(owner) = owner {
        return owner.gamespy_overlay_state_handle();
    }
    #[cfg(test)]
    {
        return TEST_OVERLAY_STATE.with(|cell| {
            cell.get_or_init(|| Rc::new(RefCell::new(GameSpyOverlayState::default())))
                .clone()
        });
    }
    #[cfg(not(test))]
    {
        Rc::new(RefCell::new(GameSpyOverlayState::default()))
    }
}

fn overlay_state(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) -> Rc<RefCell<GameSpyOverlayState>> {
    overlay_state_on_owner(owner)
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

fn clear_gs_message_boxes(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let window = {
        let state_slot = overlay_state(owner);
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

fn message_box_ok_clicked(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    let callback = {
        let state_slot = overlay_state(effects.window_manager_owner);
        let mut state = state_slot.borrow_mut();
        state.message_box_window = None;
        state.message_box_ok.take()
    };
    if let Some(mut cb) = callback {
        cb(effects);
    }
}

fn message_box_cancel_clicked(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    let callback = {
        let state_slot = overlay_state(effects.window_manager_owner);
        let mut state = state_slot.borrow_mut();
        state.message_box_window = None;
        state.message_box_cancel.take()
    };
    if let Some(mut cb) = callback {
        cb(effects);
    }
}

pub fn gs_message_box_ok(title: &str, body: &str, ok_callback: Option<MessageBoxFunc>, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    clear_gs_message_boxes(owner);
    let window = message_box_ok(title, body, Some(Box::new(message_box_ok_clicked)));
    let state_slot = overlay_state(owner);
    let mut state = state_slot.borrow_mut();
    state.message_box_window = window;
    state.message_box_ok = ok_callback;
}

pub fn gs_message_box_ok_cancel(
    title: &str,
    body: &str,
    ok_callback: Option<MessageBoxFunc>,
    cancel_callback: Option<MessageBoxFunc>,
owner: Option<&crate::gui::window_manager::WindowManagerOwner>,) {
    clear_gs_message_boxes(owner);
    let window = message_box_ok_cancel(
        title,
        body,
        Some(Box::new(message_box_ok_clicked)),
        Some(Box::new(message_box_cancel_clicked)),
    );
    let state_slot = overlay_state(owner);
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
owner: Option<&crate::gui::window_manager::WindowManagerOwner>,) {
    clear_gs_message_boxes(owner);
    let window = message_box_yes_no(
        title,
        body,
        Some(Box::new(message_box_ok_clicked)),
        Some(Box::new(message_box_cancel_clicked)),
    );
    let state_slot = overlay_state(owner);
    let mut state = state_slot.borrow_mut();
    state.message_box_window = window;
    state.message_box_ok = yes_callback;
    state.message_box_cancel = no_callback;
}

pub fn raise_gs_message_box(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    raise_overlays(owner);
    let slot = overlay_state(owner);
    let window = slot.borrow().message_box_window.clone();
    if let Some(window) = window {
        let _ = window.borrow_mut().bring_to_front();
    }
}

fn buddy_try_reconnect(_effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    let Some(queue) = get_buddy_message_queue() else {
        return;
    };
    if let Ok(mut queue) = queue.lock() {
        let mut req = BuddyRequest::default();
        req.request_type = BuddyRequestType::Relogin;
        queue.add_request(req);
    }
}

pub fn open_overlay(overlay: GameSpyOverlayType, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
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
                        owner,);
                    } else {
                        gs_message_box_ok(
                            &crate::game_text::GameText::fetch("GUI:GPErrorTitle"),
                            &crate::game_text::GameText::fetch("GUI:GPNoProfile"),
                            None,
                        owner,);
                    }
                    return;
                }
            }
        }
    }

    let layout = {
        let state_slot = overlay_state(owner);
        let mut state = state_slot.borrow_mut();
        if let Some(layout) = state.overlays.get(&overlay).cloned() {
            layout.borrow_mut().hide_without_manager_side_effects(false);
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
                layout.borrow_mut().hide_without_manager_side_effects(false);
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

pub fn close_overlay(overlay: GameSpyOverlayType, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let layout = {
        let state_slot = overlay_state(owner);
        let mut state = state_slot.borrow_mut();
        state.overlays.remove(&overlay)
    };
    if let Some(layout) = layout {
        layout.borrow().run_shutdown(None);
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
}

pub fn close_all_overlays(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let overlays = {
        let state_slot = overlay_state(owner);
        let mut state = state_slot.borrow_mut();
        let overlays = state.overlays.drain().map(|(_, v)| v).collect::<Vec<_>>();
        overlays
    };
    for layout in overlays {
        layout.borrow().run_shutdown(None);
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
    clear_gs_message_boxes(owner);
}

pub fn is_overlay_open(overlay: GameSpyOverlayType, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) -> bool {
    let slot = overlay_state(owner);
    slot.borrow().overlays.contains_key(&overlay)
}

pub fn toggle_overlay(overlay: GameSpyOverlayType, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    if is_overlay_open(overlay, owner) {
        close_overlay(overlay, owner);
    } else {
        open_overlay(overlay, owner);
    }
}

pub fn update_overlays(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let slot = overlay_state(owner);
    let overlays = slot.borrow().overlays.values().cloned().collect::<Vec<_>>();
    for layout in overlays {
        layout.borrow().run_update(None);
    }
}

fn raise_overlays(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let slot = overlay_state(owner);
    let overlays = slot.borrow().overlays.values().cloned().collect::<Vec<_>>();
    for layout in overlays {
        for window in layout.borrow().windows() {
            let _ = window.borrow_mut().bring_to_front();
        }
    }
}

pub fn reopen_player_info(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let state_slot = overlay_state(owner);
    let mut state = state_slot.borrow_mut();
    state.reopen_player_info = true;
}

pub fn check_reopen_player_info(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let reopen = {
        let state_slot = overlay_state(owner);
        let mut state = state_slot.borrow_mut();
        if state.reopen_player_info {
            state.reopen_player_info = false;
            true
        } else {
            false
        }
    };
    if reopen {
        open_overlay(GameSpyOverlayType::PlayerInfo, owner);
    }
}

pub fn set_overlay_visible(overlay: GameSpyOverlayType, visible: bool, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    if visible {
        open_overlay(overlay, owner);
    } else {
        close_overlay(overlay, owner);
    }
}

pub fn is_overlay_visible(overlay: GameSpyOverlayType, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) -> bool {
    is_overlay_open(overlay, owner)
}

pub fn set_lobby_attempt_host_join(enabled: bool, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let state_slot = overlay_state(owner);
    let mut state = state_slot.borrow_mut();
    state.lobby_attempt_host_join = enabled;
}

pub fn lobby_attempt_host_join(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) -> bool {
    let slot = overlay_state(owner);
    slot.borrow().lobby_attempt_host_join
}

pub fn set_current_staging_room_id(id: Option<i32>, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let state_slot = overlay_state(owner);
    let mut state = state_slot.borrow_mut();
    state.current_staging_room_id = id;
}

pub fn current_staging_room_id(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) -> Option<i32> {
    let slot = overlay_state(owner);
    slot.borrow().current_staging_room_id
}

pub fn register_staging_room(room: GameSpyStagingRoom, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let state_slot = overlay_state(owner);
    let mut state = state_slot.borrow_mut();
    state.staging_rooms.insert(room.id, room);
}

pub fn remove_staging_room(id: i32, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let state_slot = overlay_state(owner);
    let mut state = state_slot.borrow_mut();
    state.staging_rooms.remove(&id);
}

pub fn find_staging_room_by_id(id: i32, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) -> Option<GameSpyStagingRoom> {
    let slot = overlay_state(owner);
    slot.borrow().staging_rooms.get(&id).cloned()
}

pub fn queue_join_request(room_id: i32, password: String, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let state_slot = overlay_state(owner);
    let mut state = state_slot.borrow_mut();
    state.last_join_request = Some((room_id, password));
}

pub fn last_join_request(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) -> Option<(i32, String)> {
    let slot = overlay_state(owner);
    slot.borrow().last_join_request.clone()
}

pub fn queue_host_request(request: GameSpyHostRequest, owner: Option<&crate::gui::window_manager::WindowManagerOwner>) {
    let state_slot = overlay_state(owner);
    let mut state = state_slot.borrow_mut();
    state.last_host_request = Some(request);
}

pub fn last_host_request(owner: Option<&crate::gui::window_manager::WindowManagerOwner>) -> Option<GameSpyHostRequest> {
    let slot = overlay_state(owner);
    slot.borrow().last_host_request.clone()
}
