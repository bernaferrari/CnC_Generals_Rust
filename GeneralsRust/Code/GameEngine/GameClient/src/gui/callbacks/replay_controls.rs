//! ReplayControls.cpp callback bridge.

use crate::gui::callbacks::get_ingame_ui_system;
use crate::gui::with_window_manager;
use crate::gui::{GameWindow, WindowMessage, WindowMsgData, WindowMsgHandled};
use game_engine::common::name_key_generator::NameKeyGenerator;
use gamelogic::helpers::TheGameLogic;

const PARENT_REPLAY_CONTROL: &str = "ReplayControl.wnd:ParentReplayControl";

/// C++ InGameUI::createReplayControl — load ReplayControl.wnd if missing.
pub fn create_replay_control() -> bool {
    let parent_id = NameKeyGenerator::name_to_key(PARENT_REPLAY_CONTROL) as i32;
    with_window_manager(|manager| {
        if manager.get_window_by_id(parent_id).is_some() {
            return true;
        }
        manager
            .create_windows_from_script("ReplayControl.wnd")
            .is_ok()
    })
}

/// C++ RecorderClass::initControls — `winHide(getMode() != PLAYBACK)`.
/// Creates ReplayControl.wnd if missing so playback can show the transport.
pub fn apply_replay_control_visibility(hide: bool) {
    let parent_id = NameKeyGenerator::name_to_key(PARENT_REPLAY_CONTROL) as i32;
    with_window_manager(|manager| {
        if manager.get_window_by_id(parent_id).is_none() {
            let _ = manager.create_windows_from_script("ReplayControl.wnd");
        }
        if let Some(win) = manager.get_window_by_id(parent_id) {
            let _ = win.borrow_mut().hide(hide);
        }
    });
}

/// Borrowed native replay controls; missing authored windows remain absent.
pub fn show_replay_controls_owned(
    manager: &mut crate::gui::window_manager::WindowManager,
    in_replay: bool,
) {
    let parent_id = NameKeyGenerator::name_to_key(PARENT_REPLAY_CONTROL) as i32;
    if let Some(window) = manager.get_window_by_id(parent_id) {
        let _ = window.borrow_mut().hide(!in_replay);
    }
}

pub fn hide_replay_controls_owned(
    manager: &mut crate::gui::window_manager::WindowManager,
    _in_replay: bool,
) {
    let parent_id = NameKeyGenerator::name_to_key(PARENT_REPLAY_CONTROL) as i32;
    if let Some(window) = manager.get_window_by_id(parent_id) {
        let _ = window.borrow_mut().hide(true);
    }
}

pub fn toggle_replay_controls_owned(
    manager: &mut crate::gui::window_manager::WindowManager,
    in_replay: bool,
) {
    let parent_id = NameKeyGenerator::name_to_key(PARENT_REPLAY_CONTROL) as i32;
    if let Some(window) = manager.get_window_by_id(parent_id) {
        let show = in_replay && window.borrow().is_hidden();
        let _ = window.borrow_mut().hide(!show);
    }
}

/// Standalone Core adapter reads its legacy mode only at this boundary.
pub fn show_replay_controls() {
    let replay = TheGameLogic::is_in_replay_game();
    with_window_manager(|manager| show_replay_controls_owned(manager, replay));
}

pub fn hide_replay_controls() {
    with_window_manager(|manager| hide_replay_controls_owned(manager, false));
}

pub fn toggle_replay_controls() {
    let replay = TheGameLogic::is_in_replay_game();
    with_window_manager(|manager| toggle_replay_controls_owned(manager, replay));
}

pub fn replay_control_input(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let replay = {
        let system = get_ingame_ui_system();
        let system = system.read().unwrap_or_else(|e| e.into_inner());
        system.get_replay()
    };
    let mut replay = replay.write().unwrap_or_else(|e| e.into_inner());
    replay.input(window, msg, data1, data2)
}

pub fn replay_control_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let replay = {
        let system = get_ingame_ui_system();
        let system = system.read().unwrap_or_else(|e| e.into_inner());
        system.get_replay()
    };
    let mut replay = replay.write().unwrap_or_else(|e| e.into_inner());
    replay.system(window, msg, data1, data2)
}

#[cfg(test)]
mod owned_tests {
    use super::*;
    use crate::gui::window_manager::WindowManager;

    #[test]
    fn replay_visibility_uses_receiving_manager_and_explicit_mode() {
        let mut owner = WindowManager::new();
        let mut peer = WindowManager::new();
        let id = NameKeyGenerator::name_to_key(PARENT_REPLAY_CONTROL) as i32;
        let parent = owner
            .create_window_with_id(None, 0, 0, 100, 40, id)
            .unwrap();
        let peer_parent = peer.create_window_with_id(None, 0, 0, 100, 40, id).unwrap();
        let _ = parent.borrow_mut().hide(true);
        let _ = peer_parent.borrow_mut().hide(true);
        show_replay_controls_owned(&mut owner, true);
        assert!(!parent.borrow().is_hidden());
        assert!(peer_parent.borrow().is_hidden());
        toggle_replay_controls_owned(&mut owner, true);
        assert!(parent.borrow().is_hidden());
        toggle_replay_controls_owned(&mut owner, true);
        assert!(!parent.borrow().is_hidden());
        hide_replay_controls_owned(&mut owner, true);
        assert!(parent.borrow().is_hidden());
        show_replay_controls_owned(&mut owner, false);
        assert!(parent.borrow().is_hidden());
        toggle_replay_controls_owned(&mut owner, false);
        assert!(parent.borrow().is_hidden());
        show_replay_controls_owned(&mut peer, true);
        assert!(!peer_parent.borrow().is_hidden());
        assert!(parent.borrow().is_hidden());
        hide_replay_controls_owned(&mut peer, false);
        assert!(peer_parent.borrow().is_hidden());
    }

    #[test]
    fn missing_replay_control_is_not_created_by_visibility_callbacks() {
        let mut manager = WindowManager::new();
        let id = NameKeyGenerator::name_to_key(PARENT_REPLAY_CONTROL) as i32;
        show_replay_controls_owned(&mut manager, true);
        hide_replay_controls_owned(&mut manager, false);
        toggle_replay_controls_owned(&mut manager, true);
        assert!(manager.get_window_by_id(id).is_none());
    }
}
