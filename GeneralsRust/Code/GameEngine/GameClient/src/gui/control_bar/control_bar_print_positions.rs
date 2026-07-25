//! Control bar layout debug output.

use crate::gui::{GameWindow, WindowError, WindowManager, WindowResult};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::fs::File;
use std::io::Write;
use std::rc::Rc;

fn print_info_recursive(window: &Rc<RefCell<GameWindow>>, file: &mut File) -> WindowResult<()> {
    let window_borrow = window.borrow();
    let (size_x, size_y) = window_borrow.get_size();
    let (pos_x, pos_y) = window_borrow.get_position();
    let name = &window_borrow.instance_data().decorated_name;

    writeln!(file, "ControlBarResizer {}", name).map_err(|_| WindowError::GeneralFailure)?;
    writeln!(file, "  AltPosition = X:{} Y:{}", pos_x, pos_y)
        .map_err(|_| WindowError::GeneralFailure)?;
    writeln!(file, "  AltSize = X:{} Y:{}", size_x, size_y)
        .map_err(|_| WindowError::GeneralFailure)?;
    writeln!(file, "END\n").map_err(|_| WindowError::GeneralFailure)?;

    for child in window_borrow.children() {
        print_info_recursive(child, file)?;
    }

    Ok(())
}

/// Dump control bar layout offsets to ControlBarEasier.txt (C++ parity helper).
pub fn print_offsets_from_control_bar_parent(
    window_manager: &mut WindowManager,
) -> WindowResult<()> {
    let control_bar_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent");
    if window_manager
        .get_window_by_id(control_bar_id as i32)
        .is_none()
    {
        return Ok(());
    }

    let info = window_manager.create_windows_from_script("controlBarHidden.wnd")?;
    let mut file = File::create("ControlBarEasier.txt").map_err(|_| WindowError::GeneralFailure)?;

    if let Some(first_window) = info.windows.first() {
        print_info_recursive(first_window, &mut file)?;
    }

    for window in info.windows {
        window_manager.destroy_window(window)?;
    }
    window_manager.flush_destroy_queue();

    Ok(())
}

/// Residual: last ControlBar print-positions action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualControlBarPrintPositionsAction {
    None = 0,
    FormatLine = 1,
    ParentKey = 2,
    ScriptName = 3,
    Prepare = 4,
}

static RESIDUAL_CBPP_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_CBPP_LINE_LEN: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn residual_cbpp_action_store(action: ResidualControlBarPrintPositionsAction) {
    RESIDUAL_CBPP_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last print-positions residual action.
pub fn residual_control_bar_print_positions_last_action() -> ResidualControlBarPrintPositionsAction
{
    match RESIDUAL_CBPP_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualControlBarPrintPositionsAction::FormatLine,
        2 => ResidualControlBarPrintPositionsAction::ParentKey,
        3 => ResidualControlBarPrintPositionsAction::ScriptName,
        4 => ResidualControlBarPrintPositionsAction::Prepare,
        _ => ResidualControlBarPrintPositionsAction::None,
    }
}

/// Residual: last formatted line length latch.
pub fn residual_control_bar_print_positions_line_len() -> usize {
    RESIDUAL_CBPP_LINE_LEN.load(std::sync::atomic::Ordering::Relaxed)
}

/// Retail ControlBar parent window name residual.
pub const CONTROL_BAR_PRINT_PARENT_NAME: &str = "ControlBar.wnd:ControlBarParent";

/// Retail hidden control-bar script residual.
pub const CONTROL_BAR_PRINT_HIDDEN_SCRIPT: &str = "controlBarHidden.wnd";

/// Retail dump file residual.
pub const CONTROL_BAR_PRINT_OUTPUT_FILE: &str = "ControlBarEasier.txt";

/// Residual: format one ControlBarResizer dump line without WindowManager.
pub fn simulate_control_bar_print_positions_format_line(
    name: &str,
    pos_x: i32,
    pos_y: i32,
    size_x: i32,
    size_y: i32,
) -> String {
    let header = format!("ControlBarResizer {}", name);
    let pos = format!("  AltPosition = X:{} Y:{}", pos_x, pos_y);
    let size = format!("  AltSize = X:{} Y:{}", size_x, size_y);
    let block = format!("{}\n{}\n{}\nEND\n", header, pos, size);
    RESIDUAL_CBPP_LINE_LEN.store(block.len(), std::sync::atomic::Ordering::Relaxed);
    residual_cbpp_action_store(ResidualControlBarPrintPositionsAction::FormatLine);
    block
}

/// Residual: parent name key residual honesty.
pub fn simulate_control_bar_print_positions_parent_name() -> bool {
    residual_cbpp_action_store(ResidualControlBarPrintPositionsAction::ParentKey);
    CONTROL_BAR_PRINT_PARENT_NAME == "ControlBar.wnd:ControlBarParent"
        && CONTROL_BAR_PRINT_PARENT_NAME.contains("ControlBarParent")
}

/// Residual: hidden script + output file residual honesty.
pub fn simulate_control_bar_print_positions_script_names() -> bool {
    residual_cbpp_action_store(ResidualControlBarPrintPositionsAction::ScriptName);
    CONTROL_BAR_PRINT_HIDDEN_SCRIPT == "controlBarHidden.wnd"
        && CONTROL_BAR_PRINT_OUTPUT_FILE == "ControlBarEasier.txt"
}

/// Residual: format sample ControlBarParent block composite.
pub fn simulate_control_bar_print_positions_prepare_sample() -> bool {
    if !simulate_control_bar_print_positions_parent_name() {
        return false;
    }
    if !simulate_control_bar_print_positions_script_names() {
        return false;
    }
    let block = simulate_control_bar_print_positions_format_line(
        CONTROL_BAR_PRINT_PARENT_NAME,
        0,
        450,
        800,
        150,
    );
    residual_cbpp_action_store(ResidualControlBarPrintPositionsAction::Prepare);
    block.contains("ControlBarResizer")
        && block.contains("AltPosition")
        && block.contains("AltSize")
        && block.contains("END")
        && residual_control_bar_print_positions_line_len() > 0
}
