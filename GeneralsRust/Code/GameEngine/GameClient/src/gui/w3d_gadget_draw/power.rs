use super::*;

pub fn w3d_power_draw_a(window: &GameWindow, _inst_data: &WindowInstanceData) {
    // C++ W3DPowerDrawA (W3DControlBar.cpp:261-440) has no fallback painter:
    // every missing-input branch simply returns and paints nothing.
    let Some(global) = get_global_data() else {
        return;
    };
    let global = global.read();
    let power_bar_base = global.power_bar_base.max(2) as f32;
    let power_bar_intervals = global.power_bar_intervals.max(1.0);
    let yellow_range = global.power_bar_yellow_range;
    drop(global);

    let Ok(list) = ThePlayerList().read() else {
        return;
    };
    let player_arc = TheControlBar::get_observer_look_at_player_index()
        .and_then(|index| {
            if index >= 0 {
                list.get_player(index).cloned()
            } else {
                None
            }
        })
        .or_else(|| list.get_local_player().cloned());
    let Some(player_arc) = player_arc else {
        return;
    };
    let Ok(player) = player_arc.read() else {
        return;
    };
    let energy = player.get_energy();
    let consumption = energy.consumption();
    let production = energy.production();
    drop(player);

    let (end_bar, begin_bar, center_bar) =
        if consumption > production - yellow_range && consumption <= production {
            ("PowerBarYellowEndR", "PowerBarYellowEndL", "PowerBarYellow")
        } else if consumption > production {
            ("PowerBarRedEndR", "PowerBarRedEndL", "PowerBarRed")
        } else {
            ("PowerBarGreenEndR", "PowerBarGreenEndL", "PowerBarGreen")
        };

    let (end_bar, begin_bar, center_bar, slider) = with_window_manager_ref(|manager| {
        (
            manager.win_find_image(end_bar),
            manager.win_find_image(begin_bar),
            manager.win_find_image(center_bar),
            manager.win_find_image("PowerBarSlider"),
        )
    });
    let (Some(end_bar), Some(begin_bar), Some(center_bar), Some(slider)) =
        (end_bar, begin_bar, center_bar, slider)
    else {
        // C++ W3DControlBar.cpp:322 — `if(!slider || !endBar || !beginBar ||
        // !centerBar) return;` — missing art paints nothing.
        return;
    };

    let (pos_x, pos_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    if size_x <= 0 || size_y <= 0 {
        return;
    }

    let prod_for_log = production.max(1) as f32;
    let mut range = (log_n(prod_for_log, power_bar_base) * (size_x as f32 / power_bar_intervals))
        .round() as i32;
    range = range.clamp(0, size_x);

    let begin_w = begin_bar.width.max(1);
    let end_w = end_bar.width.max(1);
    if range < begin_w + end_w {
        range = begin_w + end_w;
    }

    let left_end_x = pos_x + begin_w;
    let right_start_x = pos_x + range - end_w;

    if right_start_x <= left_end_x {
        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                &begin_bar,
                pos_x,
                pos_y,
                pos_x + range / 2,
                pos_y + size_y,
                WIN_COLOR_UNDEFINED,
            );
            manager.win_draw_image(
                &end_bar,
                pos_x + range / 2,
                pos_y,
                pos_x + range,
                pos_y + size_y,
                WIN_COLOR_UNDEFINED,
            );
        });
    } else {
        let center_w = center_bar.width.max(1);
        let center_width = right_start_x - left_end_x;
        let pieces = center_width / center_w;
        let mut x = left_end_x;
        for _ in 0..pieces {
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    &center_bar,
                    x,
                    pos_y,
                    x + center_w,
                    pos_y + size_y,
                    WIN_COLOR_UNDEFINED,
                );
            });
            x += center_w;
        }

        let remaining = right_start_x - x;
        if remaining > 0 {
            with_window_manager_ref(|manager| {
                manager.win_draw_image(
                    &center_bar,
                    x,
                    pos_y,
                    x + center_w,
                    pos_y + size_y,
                    WIN_COLOR_UNDEFINED,
                );
            });
        }

        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                &begin_bar,
                pos_x,
                pos_y,
                left_end_x,
                pos_y + size_y,
                WIN_COLOR_UNDEFINED,
            );
        });

        with_window_manager_ref(|manager| {
            manager.win_draw_image(
                &end_bar,
                right_start_x,
                pos_y,
                right_start_x + end_w,
                pos_y + size_y,
                WIN_COLOR_UNDEFINED,
            );
        });
    }

    let consumption_for_needle = if consumption == 1 {
        1.5f32
    } else {
        consumption.max(1) as f32
    };
    let mut needle = (log_n(consumption_for_needle, power_bar_base)
        * (size_x as f32 / power_bar_intervals)) as i32;
    needle = needle.clamp(0, size_x);

    let slider_w = slider.width.max(1);
    let slider_h = slider.height.max(1);
    let mut slider_start = if needle >= size_x {
        pos_x + size_x - slider_w
    } else {
        pos_x + needle - slider_w / 2
    };
    if slider_start <= pos_x {
        slider_start = pos_x;
    }

    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            &slider,
            slider_start,
            pos_y + size_y - slider_h,
            slider_start + slider_w,
            pos_y + size_y,
            WIN_COLOR_UNDEFINED,
        );
    });
}

pub fn w3d_power_draw(window: &GameWindow, _inst_data: &WindowInstanceData) {
    // C++ W3DPowerDraw (W3DControlBar.cpp:94-259) has no fallback painter:
    // missing inputs (no player/energy/images) return and paint nothing.
    let Some(global) = get_global_data() else {
        return;
    };
    let global = global.read();
    let power_bar_base = global.power_bar_base.max(2) as f32;
    let power_bar_intervals = global.power_bar_intervals.max(1.0);
    let yellow_range = global.power_bar_yellow_range;
    drop(global);

    let Ok(list) = ThePlayerList().read() else {
        return;
    };
    let player_arc = TheControlBar::get_observer_look_at_player_index()
        .and_then(|index| {
            if index >= 0 {
                list.get_player(index).cloned()
            } else {
                None
            }
        })
        .or_else(|| list.get_local_player().cloned());
    let Some(player_arc) = player_arc else {
        return;
    };
    let Ok(player) = player_arc.read() else {
        return;
    };
    let energy = player.get_energy();
    let consumption = energy.consumption();
    let production = energy.production();
    drop(player);

    let center_name = if consumption > production - yellow_range && consumption <= production {
        "PowerPointY"
    } else if consumption > production {
        "PowerPointR"
    } else {
        "PowerPointG"
    };

    let (center_bar, slider) = with_window_manager_ref(|manager| {
        (
            manager.win_find_image(center_name),
            manager.win_find_image("PowerBarSlider"),
        )
    });
    let (Some(center_bar), Some(slider)) = (center_bar, slider) else {
        // C++ W3DControlBar.cpp:155-156 — `if(!slider || !centerBar) return;`
        // — missing bar art paints nothing (no fallback meter).
        return;
    };

    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    if width <= 0 || height <= 0 {
        return;
    }

    let prod_for_log = production.max(1) as f32;
    let mut power_range =
        (log_n(prod_for_log, power_bar_base) * (width as f32 / power_bar_intervals)).round() as i32;
    power_range = power_range.clamp(0, width);
    if power_range > 0 {
        draw_tiled_horiz(&center_bar, x, y, power_range, height);
    }

    let consumption_for_needle = if consumption == 1 {
        1.5
    } else {
        consumption.max(1) as f32
    };
    let mut needle = (log_n(consumption_for_needle, power_bar_base)
        * (width as f32 / power_bar_intervals))
        .round() as i32;
    needle = needle.clamp(0, width);
    // C++ W3DControlBar.cpp:241-242 — `if(centerWidth <= 0 && range <= 0)
    // return;` — a player with no power production and no consumption (GLA,
    // or no plants yet) draws no power bar at all.
    if power_range <= 0 && needle <= 0 {
        return;
    }
    if std::env::var_os("GENERALS_RIGHTHUD_PROBE").is_some() {
        static ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !ONCE.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!(
                "[RHUDPROBE] w3d_power_draw consumption={consumption} production={production} power_range={power_range} needle={needle} center={} slider={}",
                center_name,
                "PowerBarSlider",
            );
        }
    }

    let slider_w = slider.width.max(1);
    let slider_h = slider.height.max(1);
    let mut slider_start = if needle >= width {
        x + width - slider_w
    } else {
        x + needle - slider_w / 2
    };
    if slider_w >= width {
        slider_start = x;
    } else {
        slider_start = slider_start.max(x).min(x + width - slider_w);
    }
    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            &slider,
            slider_start,
            y + height - slider_h,
            slider_start + slider_w,
            y + height,
            WIN_COLOR_UNDEFINED,
        );
    });
}

pub(super) fn draw_vertical_meter(
    window: &GameWindow,
    top_name: &str,
    bottom_name: &str,
    center_name: &str,
    filled_height: i32,
) {
    let (top, bottom, center) = with_window_manager_ref(|manager| {
        (
            manager.win_find_image(top_name),
            manager.win_find_image(bottom_name),
            manager.win_find_image(center_name),
        )
    });
    let (Some(top), Some(bottom), Some(center)) = (top, bottom, center) else {
        // C++ W3DCommandBarGenExpDraw (W3DControlBar.cpp:494-495) returns when
        // the meter images are missing — no fallback track is painted.
        return;
    };

    let (x, y) = window.get_screen_position();
    let (width, height) = window.get_size();
    if width <= 0 || height <= 0 {
        return;
    }

    let fill = filled_height.clamp(0, height);
    if fill <= 0 {
        return;
    }

    let top_h = top.height.max(1);
    let bottom_h = bottom.height.max(1);
    let fill_top = y + height - fill;

    let bottom_start = y + height - bottom_h;
    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            &bottom,
            x,
            bottom_start,
            x + width,
            y + height,
            WIN_COLOR_UNDEFINED,
        );
    });

    let top_start = (fill_top - top_h).max(y);
    with_window_manager_ref(|manager| {
        manager.win_draw_image(
            &top,
            x,
            top_start,
            x + width,
            top_start + top_h,
            WIN_COLOR_UNDEFINED,
        );
    });

    let center_start = top_start + top_h;
    let center_end = bottom_start;
    if center_end > center_start {
        draw_tiled_vert(&center, x, center_start, width, center_end - center_start);
    }
    note_shipped_ui_draw_commands(1);
}

