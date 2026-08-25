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

fn with_replay_window(f: impl FnOnce(&std::rc::Rc<std::cell::RefCell<GameWindow>>)) {
    let parent_id = NameKeyGenerator::name_to_key(PARENT_REPLAY_CONTROL) as i32;
    with_window_manager(|manager| {
        if let Some(win) = manager.get_window_by_id(parent_id) {
            f(&win);
        }
    });
}

/// C++ InGameUI.cpp:203-210 `showReplayControls` — show only in replay.
pub fn show_replay_controls() {
    with_replay_window(|win| {
        let show = TheGameLogic::is_in_replay_game();
        let _ = win.borrow_mut().hide(!show);
    });
}

/// C++ InGameUI.cpp:214-220 `hideReplayControls`.
pub fn hide_replay_controls() {
    with_replay_window(|win| {
        let _ = win.borrow_mut().hide(true);
    });
}

/// C++ InGameUI.cpp:224-231 `toggleReplayControls`.
pub fn toggle_replay_controls() {
    with_replay_window(|win| {
        let show = TheGameLogic::is_in_replay_game() && win.borrow().is_hidden();
        let _ = win.borrow_mut().hide(!show);
    });
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
