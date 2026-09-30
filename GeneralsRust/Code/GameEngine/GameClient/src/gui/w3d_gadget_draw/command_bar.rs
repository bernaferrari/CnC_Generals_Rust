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

/// C++ W3DControlBar.cpp reads BackgroundMarker for both scheme layers.
/// During `draw_all`, the window manager is mutably borrowed, so a fresh
/// global lookup can fail closed; the callback window's parent owns the
/// sibling marker in the same WND hierarchy.
fn background_marker_screen_position(window: &GameWindow) -> Option<(i32, i32)> {
    if window
        .get_name()
        .eq_ignore_ascii_case("ControlBar.wnd:BackgroundMarker")
    {
        return Some(window.get_screen_position());
    }
    let parent = window.get_parent()?;
    let background = parent
        .borrow()
        .children()
        .iter()
        .find(|child| {
            child
                .borrow()
                .get_name()
                .eq_ignore_ascii_case("ControlBar.wnd:BackgroundMarker")
        })
        .cloned()?;
    Some(background.borrow().get_screen_position())
}

pub fn w3d_command_bar_background_draw(window: &GameWindow, _inst_data: &WindowInstanceData) {
    ensure_scheme_draw_registered();

    let Some(manager_handle) = get_control_bar_scheme_manager() else {
        return;
    };

    let Some((pos_x, pos_y)) = background_marker_screen_position(window) else {
        return;
    };
    let manager = manager_handle.read();
    if !manager.marker_base_captured() {
        return;
    }
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
    let Some((pos_x, pos_y)) = background_marker_screen_position(window) else {
        return;
    };

    let manager = manager_handle.read();
    if !manager.marker_base_captured() {
        return;
    }
    let base_pos = manager.get_foreground_marker_pos();
    let offset = ICoord2D {
        x: pos_x - base_pos.x,
        y: pos_y - base_pos.y,
    };
    manager.draw_foreground(offset);
}

#[cfg(test)]
mod marker_tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn both_scheme_layers_follow_background_marker_when_bar_moves() {
        let parent = Rc::new(RefCell::new(GameWindow::new()));
        parent.borrow_mut().set_position(0, 332).unwrap();
        let background = Rc::new(RefCell::new(GameWindow::new()));
        background
            .borrow_mut()
            .set_name("ControlBar.wnd:BackgroundMarker");
        background.borrow_mut().set_position(6, 144).unwrap();
        background.borrow_mut().set_parent(Some(&parent));
        parent.borrow_mut().add_child(background.clone());
        let foreground = Rc::new(RefCell::new(GameWindow::new()));
        foreground
            .borrow_mut()
            .set_name("ControlBar.wnd:ForegroundMarker");
        foreground.borrow_mut().set_position(0, 144).unwrap();
        foreground.borrow_mut().set_parent(Some(&parent));
        parent.borrow_mut().add_child(foreground.clone());

        assert_eq!(
            background_marker_screen_position(&foreground.borrow()),
            Some((6, 476)),
            "C++ foreground callback also reads BackgroundMarker"
        );
        parent.borrow_mut().set_position(0, 432).unwrap();
        assert_eq!(
            background_marker_screen_position(&background.borrow()),
            Some((6, 576))
        );
        assert_eq!(
            background_marker_screen_position(&foreground.borrow()),
            Some((6, 576))
        );
    }
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
