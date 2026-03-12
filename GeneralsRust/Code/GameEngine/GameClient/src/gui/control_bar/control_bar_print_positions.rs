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
