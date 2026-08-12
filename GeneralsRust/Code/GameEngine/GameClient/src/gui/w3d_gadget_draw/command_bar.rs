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

/// C++ BackgroundMarker is 5×5; HUD chrome lives on ControlBarParent (~183px).
pub(super) fn control_bar_hud_strip_rect(window: &GameWindow) -> (i32, i32, i32, i32) {
    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    if width >= 64 && height >= 32 {
        return (x, y, width, height);
    }

    let parent = with_window_manager_ref(|wm| {
        wm.find_window_by_name("ControlBar.wnd:ControlBarParent")
            .map(|win| {
                let win = win.borrow();
                let (px, py) = win.get_screen_position();
                let (pw, ph) = win.get_size();
                (px, py, pw, ph)
            })
    });
    if let Some((px, py, pw, ph)) = parent {
        if pw >= 64 && ph >= 32 {
            return (px, py, pw, ph);
        }
    }

    let screen_h = ui_screen_height().max(600);
    let screen_w = with_ui_renderer_mut(|renderer| renderer.screen_size().0 as i32).unwrap_or(800);
    let strip_h = ((screen_h as f32) * 0.23).round() as i32;
    (0, screen_h - strip_h, screen_w.max(800), strip_h.max(96))
}

pub(super) fn draw_control_bar_hud_fallback(window: &GameWindow, inst_data: &WindowInstanceData) {
    let (x, y, width, height) = control_bar_hud_strip_rect(window);
    let color = visible_enabled_color(window, inst_data, FALLBACK_HUD_FILL);
    draw_visible_fill(x, y, width, height, color, Some(FALLBACK_BORDER));
    draw_visible_label(x, y, "HUD", FALLBACK_LABEL);
}

pub fn w3d_command_bar_top_draw(_window: &GameWindow, _inst_data: &WindowInstanceData) {
    // C++ callback is effectively no-op in W3DControlBar.cpp.
}

pub fn w3d_command_bar_background_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    ensure_scheme_draw_registered();

    let manager_handle = get_control_bar_scheme_manager();
    let Some(manager_handle) = manager_handle else {
        draw_control_bar_hud_fallback(window, inst_data);
        return;
    };

    let manager = manager_handle.read();

    let base_pos = manager.get_background_marker_pos();
    let win_name = "ControlBar.wnd:BackgroundMarker";
    let marker_window = with_window_manager_ref(|wm| wm.find_window_by_name(win_name));
    let Some(marker_window) = marker_window else {
        draw_control_bar_hud_fallback(window, inst_data);
        return;
    };

    let (pos_x, pos_y) = marker_window.borrow().get_screen_position();
    let offset = ICoord2D {
        x: pos_x - base_pos.x,
        y: pos_y - base_pos.y,
    };

    let before = shipped_ui_draw_command_count();
    manager.draw_background(offset);
    if shipped_ui_draw_command_count() == before {
        draw_control_bar_hud_fallback(window, inst_data);
    }
}

pub fn w3d_command_bar_foreground_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    ensure_scheme_draw_registered();

    let manager_handle = get_control_bar_scheme_manager();
    let Some(manager_handle) = manager_handle else {
        draw_control_bar_hud_fallback(window, inst_data);
        return;
    };

    let manager = manager_handle.read();

    let base_pos = manager.get_foreground_marker_pos();
    let win_name = "ControlBar.wnd:BackgroundMarker";
    let marker_window = with_window_manager_ref(|wm| wm.find_window_by_name(win_name));
    let Some(marker_window) = marker_window else {
        draw_control_bar_hud_fallback(window, inst_data);
        return;
    };

    let (pos_x, pos_y) = marker_window.borrow().get_screen_position();
    let offset = ICoord2D {
        x: pos_x - base_pos.x,
        y: pos_y - base_pos.y,
    };

    let before = shipped_ui_draw_command_count();
    manager.draw_foreground(offset);
    if shipped_ui_draw_command_count() == before {
        draw_control_bar_hud_fallback(window, inst_data);
    }
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
    note_shipped_ui_draw_commands(4);
}

pub fn w3d_command_bar_gen_exp_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    let _ = inst_data;
    let Ok(list) = ThePlayerList().read() else {
        draw_vertical_meter_fallback(window, 0);
        return;
    };
    let Some(player_arc) = list.get_local_player().cloned() else {
        draw_vertical_meter_fallback(window, 0);
        return;
    };
    let Ok(player) = player_arc.read() else {
        draw_vertical_meter_fallback(window, 0);
        return;
    };
    if !player.is_player_active() {
        draw_vertical_meter_fallback(window, 0);
        return;
    }
    let Some(rank_progress) = RankProgressInfo::from_player(&player) else {
        draw_vertical_meter_fallback(window, 0);
        return;
    };
    let mut progress = (rank_progress.progress_percentage * 100.0).round() as i32;
    progress = progress.clamp(0, 100);
    if progress <= 0 {
        draw_vertical_meter_fallback(window, 0);
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
