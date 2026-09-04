use super::*;

/// Wire WND-named ControlBar chrome to the shipped W3D draw callbacks.
/// Called from ControlBar::update (not from inside draw_all — that holds WM mut).
pub fn ensure_control_bar_wnd_draw_callbacks() {
    ensure_scheme_draw_registered();
    with_window_manager(|wm| {
        let assign = |name: &str, cb: fn(&GameWindow, &WindowInstanceData)| {
            if let Some(win) = wm.find_window_by_name(name) {
                win.borrow_mut().set_draw_callback(cb);
            }
        };
        assign(
            "ControlBar.wnd:BackgroundMarker",
            w3d_command_bar_background_draw,
        );
        assign(
            "ControlBar.wnd:ForegroundMarker",
            w3d_command_bar_foreground_draw,
        );
        assign("ControlBar.wnd:PowerWindow", w3d_power_draw);
        assign("ControlBar.wnd:LeftHUD", w3d_left_hud_draw);
        assign("ControlBar.wnd:RightHUD", w3d_right_hud_draw);
        assign("ControlBar.wnd:GeneralsExp", w3d_command_bar_gen_exp_draw);
    });
}


/// Feed the draw-time marker position into the scheme manager's stability
/// latch (C++ ControlBar.cpp:1222-1225 captures the marker base at init;
/// the port latches on the first stable observation window instead).
fn capture_marker_base_once(
    manager: &mut game_engine::common::ini::ControlBarSchemeManager,
    pos_x: i32,
    pos_y: i32,
) {
    manager.note_marker_observation(pos_x, pos_y);
}

pub fn w3d_command_bar_background_draw(window: &GameWindow, _inst_data: &WindowInstanceData) {
    ensure_scheme_draw_registered();

    let Some(manager_handle) = get_control_bar_scheme_manager() else {
        return;
    };

    // C++ W3DControlBar.cpp:612-633: the callback IS assigned on
    // ControlBar.wnd:BackgroundMarker, so read the marker screen position
    // from the callback window itself (the by-name re-lookup C++ performs is
    // a same-window idiom; in the port's runtime-host draw context the
    // current WM instance can differ from the wire-time one, where the
    // by-name lookup misses).
    let (pos_x, pos_y) = window.get_screen_position();
    {
        let mut manager = manager_handle.write();
        capture_marker_base_once(&mut manager, pos_x, pos_y);
    }

    let manager = manager_handle.read();
    let base_pos = manager.get_background_marker_pos();
    let offset = ICoord2D {
        x: pos_x - base_pos.x,
        y: pos_y - base_pos.y,
    };

    manager.draw_background(offset);
}
pub fn w3d_command_bar_foreground_draw(window: &GameWindow, _inst_data: &WindowInstanceData) {
    ensure_scheme_draw_registered();

    // C++ W3DControlBar.cpp:639-641: no scheme manager -> silent return.
    let Some(manager_handle) = get_control_bar_scheme_manager() else {
        return;
    };
    // C++ captures BOTH marker base positions from the BackgroundMarker
    // window at init (ControlBar.cpp:1222-1225), so only the background draw
    // feeds the latch; the foreground just reads its latched base. (This
    // callback's window is ForegroundMarker, whose position must not reset
    // the shared observation state.)
    let (pos_x, pos_y) = window.get_screen_position();

    let manager = manager_handle.read();
    let base_pos = manager.get_foreground_marker_pos();
    let offset = ICoord2D {
        x: pos_x - base_pos.x,
        y: pos_y - base_pos.y,
    };
    manager.draw_foreground(offset);
}

pub fn w3d_command_bar_top_draw(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    // C++ callback is effectively no-op in W3DControlBar.cpp.
}

pub fn w3d_command_bar_grid_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    // C++ W3DCommandBarGridDraw (W3DControlBar.cpp:442-466): image windows use
    // the default draw; otherwise the border color grids the command table.
    if window.get_status().contains(WindowStatus::IMAGE) {
        crate::gui::game_window::default_draw_callback(window, inst_data);
        return;
    }

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
    note_shipped_ui_draw_commands(4);
}

pub fn w3d_command_bar_gen_exp_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let _ = inst_data;
    // C++ W3DCommandBarGenExpDraw (W3DControlBar.cpp:468-495): every early-out
    // returns without painting — no fallback meter track.
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
