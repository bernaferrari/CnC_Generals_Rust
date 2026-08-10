use super::*;

pub fn w3d_command_bar_top_draw(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    // C++ callback is effectively no-op in W3DControlBar.cpp.
}

pub fn w3d_command_bar_background_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    ensure_scheme_draw_registered();

    let manager_handle = get_control_bar_scheme_manager();
    let Some(manager_handle) = manager_handle else {
        crate::gui::game_window::default_draw_callback(window, inst_data);
        return;
    };

    let manager = manager_handle.read();

    let base_pos = manager.get_background_marker_pos();
    let win_name = "ControlBar.wnd:BackgroundMarker";
    let marker_window = with_window_manager_ref(|wm| wm.find_window_by_name(win_name));
    let marker_window = match marker_window {
        Some(w) => w,
        None => {
            crate::gui::game_window::default_draw_callback(window, inst_data);
            return;
        }
    };

    let (pos_x, pos_y) = marker_window.borrow().get_screen_position();
    let offset = ICoord2D {
        x: pos_x - base_pos.x,
        y: pos_y - base_pos.y,
    };

    manager.draw_background(offset);
}

pub fn w3d_command_bar_foreground_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    ensure_scheme_draw_registered();

    let manager_handle = get_control_bar_scheme_manager();
    let Some(manager_handle) = manager_handle else {
        crate::gui::game_window::default_draw_callback(window, inst_data);
        return;
    };

    let manager = manager_handle.read();

    let base_pos = manager.get_foreground_marker_pos();
    let win_name = "ControlBar.wnd:BackgroundMarker";
    let marker_window = with_window_manager_ref(|wm| wm.find_window_by_name(win_name));
    let marker_window = match marker_window {
        Some(w) => w,
        None => {
            crate::gui::game_window::default_draw_callback(window, inst_data);
            return;
        }
    };

    let (pos_x, pos_y) = marker_window.borrow().get_screen_position();
    let offset = ICoord2D {
        x: pos_x - base_pos.x,
        y: pos_y - base_pos.y,
    };

    manager.draw_foreground(offset);
}

pub fn w3d_command_bar_grid_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    if window.get_status().contains(WindowStatus::IMAGE) {
        crate::gui::game_window::default_draw_callback(window, inst_data);
        return;
    }

    crate::gui::game_window::default_draw_callback(window, inst_data);
    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    let color = window
        .get_enabled_draw_data(0)
        .map(|entry| entry.border_color)
        .filter(|color| *color != WIN_COLOR_UNDEFINED)
        .unwrap_or(0xFF808080);

    with_window_manager_ref(|manager| {
        manager.win_draw_line(
            color,
            1.0,
            x,
            y + (height as f32 * 0.33) as i32,
            x + width,
            y + (height as f32 * 0.33) as i32,
        );
        manager.win_draw_line(
            color,
            1.0,
            x,
            y + (height as f32 * 0.66) as i32,
            x + width,
            y + (height as f32 * 0.66) as i32,
        );
        manager.win_draw_line(
            color,
            1.0,
            x + (width as f32 * 0.33) as i32,
            y,
            x + (width as f32 * 0.33) as i32,
            y + height,
        );
        manager.win_draw_line(
            color,
            1.0,
            x + (width as f32 * 0.66) as i32,
            y,
            x + (width as f32 * 0.66) as i32,
            y + height,
        );
    });
}

pub fn w3d_command_bar_gen_exp_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let _ = inst_data;
    let Ok(list) = ThePlayerList().read() else {
        return;
    };
    let Some(player_arc) = list.get_local_player().cloned() else {
        return;
    };
    let Ok(player) = player_arc.read() else {
        return;
    };
    if !player.is_player_active() {
        return;
    }
    let Some(rank_progress) = RankProgressInfo::from_player(&player) else {
        return;
    };
    let mut progress = (rank_progress.progress_percentage * 100.0).round() as i32;
    progress = progress.clamp(0, 100);
    if progress <= 0 {
        return;
    }

    let (_, height) = window.get_size();
    let filled_height = (height * progress) / 100;
    draw_vertical_meter(
        window,
        "GenExpBarTop1",
        "GenExpBarBottom1",
        "GenExpBar1",
        filled_height,
    );
}

pub fn w3d_command_bar_help_popup_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let _ = inst_data;
    let (_, height) = window.get_size();
    draw_vertical_meter(
        window,
        "Helpbox-top",
        "Helpbox-bottom",
        "Helpbox-middle",
        height,
    );
}

