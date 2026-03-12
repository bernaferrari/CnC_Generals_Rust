//! GeneralsExpPoints.cpp callback port.

use crate::gui::{GameWindow, WindowMessage, WindowMsgData, WindowMsgHandled};
use crate::helpers::{TheControlBar, TheInGameUI};
use game_engine::common::name_key_generator::NameKeyGenerator;

const KEY_ESC: u32 = 0x1B;
const GGM_LEFT_DRAG: u32 = 16384;
const GBM_SELECTED: u32 = GGM_LEFT_DRAG + 8;

pub fn generals_exp_points_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::MouseEntering => {
            TheInGameUI::place_build_available(None, None);
        }
        WindowMessage::Char => {
            if data1 == KEY_ESC {
                TheControlBar::hide_purchase_science();
            }
        }
        _ => {}
    }

    WindowMsgHandled::Handled
}

pub fn generals_exp_points_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => {
            if data1 != 0 {
                let _ = data2;
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            let button_exit_id =
                NameKeyGenerator::name_to_key("GeneralsExpPoints.wnd:ButtonExit") as u32;

            if control_id == button_exit_id {
                TheControlBar::hide_purchase_science();
            } else {
                TheControlBar::process_context_sensitive_button_click(control_id, GBM_SELECTED);
            }

            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
