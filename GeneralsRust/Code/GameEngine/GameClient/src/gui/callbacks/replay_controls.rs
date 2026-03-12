//! ReplayControls.cpp callback bridge.

use crate::gui::callbacks::get_ingame_ui_system;
use crate::gui::{GameWindow, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn replay_control_input(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let replay = {
        let system = get_ingame_ui_system();
        let system = system.read().unwrap();
        system.get_replay()
    };
    let mut replay = replay.write().unwrap();
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
        let system = system.read().unwrap();
        system.get_replay()
    };
    let mut replay = replay.write().unwrap();
    replay.system(window, msg, data1, data2)
}
