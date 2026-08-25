//! GeneralsExpPoints.cpp callback port.

use crate::gui::control_bar::publish_host_cancel_structure_placement;
use crate::gui::{
    GameWindow, WindowMessage, WindowMsgData, WindowMsgHandled, write_input_focus_response,
};
use crate::helpers::{TheControlBar, TheInGameUI};
use game_engine::common::name_key_generator::NameKeyGenerator;

const KEY_ESC: usize = 0x1B;
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
            if !publish_host_cancel_structure_placement() {
                TheInGameUI::place_build_available(None, None);
            }
        }
        WindowMessage::Char if data1 == KEY_ESC => {
            TheControlBar::hide_purchase_science();
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
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, false),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            let button_exit_id = NameKeyGenerator::name_to_key("GeneralsExpPoints.wnd:ButtonExit");

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

/// Residual: last GeneralsExpPoints action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualGeneralsExpAction {
    None = 0,
    Bind = 1,
    Exit = 2,
    ScienceClick = 3,
    Esc = 4,
}

static RESIDUAL_GENEXP_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_GENEXP_VISIBLE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn residual_genexp_action_store(action: ResidualGeneralsExpAction) {
    RESIDUAL_GENEXP_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last GeneralsExp residual action.
pub fn residual_generals_exp_last_action() -> ResidualGeneralsExpAction {
    match RESIDUAL_GENEXP_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualGeneralsExpAction::Bind,
        2 => ResidualGeneralsExpAction::Exit,
        3 => ResidualGeneralsExpAction::ScienceClick,
        4 => ResidualGeneralsExpAction::Esc,
        _ => ResidualGeneralsExpAction::None,
    }
}

/// Residual: GeneralsExp / purchase-science visibility latch.
pub fn residual_generals_exp_is_visible() -> bool {
    RESIDUAL_GENEXP_VISIBLE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: bind GeneralsExpPoints control IDs (no layout load).
pub fn simulate_generals_exp_bind_controls() -> bool {
    let _ = NameKeyGenerator::name_to_key("GeneralsExpPoints.wnd:ButtonExit");
    residual_genexp_action_store(ResidualGeneralsExpAction::Bind);
    true
}

/// Residual: show purchase-science residual without ControlBar layout create.
pub fn simulate_generals_exp_show() -> bool {
    let _ = simulate_generals_exp_bind_controls();
    RESIDUAL_GENEXP_VISIBLE.store(true, std::sync::atomic::Ordering::Relaxed);
    residual_generals_exp_is_visible()
}

/// Residual: fire ButtonExit without hide_purchase_science side effects.
pub fn simulate_generals_exp_exit_button_gadget_selected() -> bool {
    let _ = simulate_generals_exp_bind_controls();
    RESIDUAL_GENEXP_VISIBLE.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_genexp_action_store(ResidualGeneralsExpAction::Exit);
    !residual_generals_exp_is_visible()
}

/// Residual: ESC hide residual (same exit path).
pub fn simulate_generals_exp_esc() -> bool {
    RESIDUAL_GENEXP_VISIBLE.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_genexp_action_store(ResidualGeneralsExpAction::Esc);
    !residual_generals_exp_is_visible()
}

/// Residual: context-sensitive science button click without purchase apply.
pub fn simulate_generals_exp_science_button_gadget_selected(control_id: u32) -> bool {
    let _ = control_id;
    let _ = simulate_generals_exp_bind_controls();
    residual_genexp_action_store(ResidualGeneralsExpAction::ScienceClick);
    true
}

/// Residual: show + Exit composite.
pub fn simulate_generals_exp_prepare_exit() -> bool {
    if !simulate_generals_exp_show() {
        return false;
    }
    simulate_generals_exp_exit_button_gadget_selected()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_entering_keeps_legacy_placement_cancel_when_bridge_is_disabled() {
        let _guard = crate::gui::control_bar::acquire_host_control_bar_bridge_test_guard();
        TheInGameUI::place_build_available(Some("TestStructure".to_string()), Some(77));
        assert_eq!(
            TheInGameUI::get_pending_place_template().as_deref(),
            Some("TestStructure")
        );

        let window = GameWindow::new();
        assert_eq!(
            generals_exp_points_input(&window, WindowMessage::MouseEntering, 0, 0),
            WindowMsgHandled::Handled
        );
        assert!(
            TheInGameUI::get_pending_place_template().is_none(),
            "standalone GameClient must retain C++ placement cancellation"
        );
        assert_eq!(TheInGameUI::get_pending_place_source_object_id(), 0);
        assert!(crate::gui::control_bar::take_host_control_bar_requests().is_empty());
    }

    #[test]
    fn mouse_entering_publishes_host_placement_cancel_when_bridge_is_enabled() {
        let _guard = crate::gui::control_bar::acquire_host_control_bar_bridge_test_guard();
        crate::gui::control_bar::set_host_control_bar_bridge_enabled(true);

        let window = GameWindow::new();
        assert_eq!(
            generals_exp_points_input(&window, WindowMessage::MouseEntering, 0, 0),
            WindowMsgHandled::Handled
        );
        assert!(matches!(
            crate::gui::control_bar::take_host_control_bar_requests().as_slice(),
            [crate::gui::control_bar::HostControlBarRequest::CancelStructurePlacement]
        ));
    }
}
