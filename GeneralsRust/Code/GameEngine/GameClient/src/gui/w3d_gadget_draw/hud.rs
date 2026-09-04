use super::*;

pub fn w3d_clock_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    crate::gui::game_window::default_draw_callback(window, inst_data);

    let datestr = Local::now().format("%H:%M:%S").to_string();
    let font = get_font_library()
        .get_font(&FontDesc::new("Arial", 16, false))
        .ok();
    let text_width = font
        .as_ref()
        .map(|font| font.measure_text(&datestr))
        .unwrap_or((datestr.len() as i32 * 10).max(1));
    let text_height = font
        .as_ref()
        .map(|font| font.get_line_height())
        .unwrap_or(16);

    let (pos_x, pos_y) = window.get_screen_position();
    let (size_x, size_y) = window.get_size();
    let text_x = pos_x + (size_x / 2) - (text_width / 2);
    let text_y = pos_y + (size_y / 2) - (text_height / 2);
    let scissor = UIRect::new(
        (pos_x + 1) as f32,
        (pos_y + 1) as f32,
        (size_x - 2).max(0) as f32,
        (size_y - 2).max(0) as f32,
    );

    let _ = with_ui_renderer_mut(|renderer| {
        let (point_size, font_name) = match font.as_ref() {
            Some(font) => (font.desc.size as f32, font.desc.name.as_str()),
            None => (16.0, "Arial"),
        };
        let _ = renderer.draw_text_simple_named_with_scissor(
            &datestr,
            glam::Vec2::new((text_x + 1) as f32, (text_y + 1) as f32),
            point_size,
            [0.0, 0.0, 0.0, 1.0],
            font_name,
            false,
            scissor,
        );
        let _ = renderer.draw_text_simple_named_with_scissor(
            &datestr,
            glam::Vec2::new(text_x as f32, text_y as f32),
            point_size,
            [1.0, 1.0, 1.0, 1.0],
            font_name,
            false,
            scissor,
        );
    });
    note_shipped_ui_draw_commands(1);
}

pub fn w3d_cameo_movie_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    draw_video_buffer(window, inst_data);
}

/// Check if radar should be drawn (helper function to avoid lifetime issues)
pub(super) fn should_draw_radar_check() -> bool {
    let radar_system = get_radar_system();
    let Ok(radar) = radar_system.read() else {
        return false;
    };

    if radar.is_radar_forced() {
        return true;
    }

    if radar.is_radar_hidden() {
        return false;
    }

    // Live host stamps Player::hasRadar onto TheRadar each update.
    if radar.local_has_radar() {
        return true;
    }

    // Leftover PlayerList fallback (C++ ThePlayerList->getLocalPlayer()->hasRadar).
    let Ok(list) = ThePlayerList().read() else {
        return false;
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

    if let Some(player_arc) = player_arc {
        if let Ok(player) = player_arc.read() {
            return player.has_radar();
        }
    }

    false
}

pub fn w3d_left_hud_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    // First check for video buffer (in-game movies)
    if inst_data
        .video_buffer
        .as_ref()
        .and_then(read_video_frame)
        .is_some()
    {
        draw_video_buffer(window, inst_data);
        return;
    }

    // C++ parity: check if radar should be drawn
    // W3DLeftHUDDraw draws radar when:
    // - TheRadar->isRadarForced() OR
    // - (!TheRadar->isRadarHidden() AND player->hasRadar())
    if should_draw_radar_check() {
        // Get window position and size for radar drawing
        let (pos_x, pos_y) = window.get_screen_position();
        let (size_x, size_y) = window.get_size();

        // Draw radar with 1-pixel border (matching C++ TheRadar->draw(pos.x + 1, pos.y + 1, size.x - 2, size.y - 2))
        draw_radar_in_hud(
            pos_x + 1,
            pos_y + 1,
            size_x.saturating_sub(2),
            size_y.saturating_sub(2),
        );
    } else {
        // Fall back to default drawing when no radar
        crate::gui::game_window::default_draw_callback(window, inst_data);
        let (x, y) = window.get_screen_position();
        let (width, height) = window.get_size();
        if width > 0 && height > 0 {
            draw_visible_fill(
                x,
                y,
                width,
                height,
                visible_enabled_color(window, inst_data, FALLBACK_HUD_FILL),
                Some(FALLBACK_BORDER),
            );
        }
    }
}

/// Draw radar in the HUD area (matches C++ TheRadar->draw())
pub(super) fn draw_radar_in_hud(x: i32, y: i32, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }

    let radar_system = get_radar_system();
    let Ok(mut radar) = radar_system.write() else {
        return;
    };
    // C++ `W3DRadar::draw` already passed hasRadar/forced; chirp here.
    if !radar.is_radar_shown() && !radar.is_radar_hidden() {
        radar.set_local_has_radar(true);
    }
    let events = radar.draw_events();

    // Draw terrain texture from radar system
    let terrain_texture = radar.get_terrain_texture();
    if terrain_texture.is_empty() {
        return;
    }

    let (ul, lr) = radar_draw_positions(x, y, width, height, radar.map_extent());
    let scaled_width = lr.x - ul.x;
    let scaled_height = lr.y - ul.y;
    if scaled_width <= 0 || scaled_height <= 0 {
        return;
    }

    let current_frame = radar.current_frame();
    let _ = with_ui_renderer_mut(|renderer| {
        let texture = renderer.create_texture_from_rgba(
            game_engine::common::system::radar::RADAR_CELL_WIDTH,
            game_engine::common::system::radar::RADAR_CELL_HEIGHT,
            terrain_texture,
        );

        let fill_color = [0.0, 0.0, 0.0, 1.0];
        let line_color = [50.0 / 255.0, 50.0 / 255.0, 50.0 / 255.0, 1.0];
        if radar.map_extent().width() / width as f32 >= radar.map_extent().height() / height as f32
        {
            if ul.y > y {
                renderer.draw_rect(
                    UIRect::new(
                        x as f32,
                        y as f32,
                        width as f32,
                        (ul.y - y - 1).max(0) as f32,
                    ),
                    fill_color,
                    0.0,
                );
                renderer.draw_line(
                    glam::Vec2::new(x as f32, ul.y as f32),
                    glam::Vec2::new((x + width) as f32, ul.y as f32),
                    1.0,
                    line_color,
                    0.0,
                );
            }
            if lr.y < y + height {
                renderer.draw_rect(
                    UIRect::new(
                        x as f32,
                        (lr.y + 1) as f32,
                        width as f32,
                        (y + height - lr.y - 1).max(0) as f32,
                    ),
                    fill_color,
                    0.0,
                );
                renderer.draw_line(
                    glam::Vec2::new(x as f32, (lr.y + 1) as f32),
                    glam::Vec2::new((x + width) as f32, (lr.y + 1) as f32),
                    1.0,
                    line_color,
                    0.0,
                );
            }
        } else {
            if ul.x > x {
                renderer.draw_rect(
                    UIRect::new(
                        x as f32,
                        y as f32,
                        (ul.x - x - 1).max(0) as f32,
                        height as f32,
                    ),
                    fill_color,
                    0.0,
                );
                renderer.draw_line(
                    glam::Vec2::new(ul.x as f32, y as f32),
                    glam::Vec2::new(ul.x as f32, (y + height) as f32),
                    1.0,
                    line_color,
                    0.0,
                );
            }
            if lr.x < x + width {
                renderer.draw_rect(
                    UIRect::new(
                        (lr.x + 1) as f32,
                        y as f32,
                        (width - (lr.x - x) - 1).max(0) as f32,
                        height as f32,
                    ),
                    fill_color,
                    0.0,
                );
                renderer.draw_line(
                    glam::Vec2::new((lr.x + 1) as f32, y as f32),
                    glam::Vec2::new((lr.x + 1) as f32, (y + height) as f32),
                    1.0,
                    line_color,
                    0.0,
                );
            }
        }

        let rect = UIRect::new(
            ul.x as f32,
            ul.y as f32,
            scaled_width as f32,
            scaled_height as f32,
        );
        let radar_uv = radar_layer_vflip_uv();
        renderer.draw_textured_rect(rect, texture, [1.0, 1.0, 1.0, 1.0], Some(radar_uv), 0.0);

        let mut overlay_cache = radar_object_overlay_texture_cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let map_extent_signature = radar_map_extent_signature(radar.map_extent());
        if overlay_cache.texture.is_none()
            || overlay_cache.map_extent_signature != Some(map_extent_signature)
            || should_refresh_w3d_object_overlay(current_frame)
        {
            let object_overlay = radar.build_object_overlay_texture_rgba();
            overlay_cache.texture = Some(renderer.create_texture_from_rgba(
                game_engine::common::system::radar::RADAR_CELL_WIDTH,
                game_engine::common::system::radar::RADAR_CELL_HEIGHT,
                &object_overlay,
            ));
            overlay_cache.hero_object_ids = radar.build_hero_reticle_object_ids();
            overlay_cache.map_extent_signature = Some(map_extent_signature);
        }
        if let Some(object_overlay) = overlay_cache.texture.clone() {
            renderer.draw_textured_rect(
                rect,
                object_overlay,
                [1.0, 1.0, 1.0, 1.0],
                Some(radar_uv),
                0.0,
            );
        }
        let hero_object_ids = overlay_cache.hero_object_ids.clone();
        drop(overlay_cache);

        let shroud_texture = radar.build_shroud_texture_rgba();
        let shroud_texture = renderer.create_texture_from_rgba(
            game_engine::common::system::radar::RADAR_CELL_WIDTH,
            game_engine::common::system::radar::RADAR_CELL_HEIGHT,
            &shroud_texture,
        );
        renderer.draw_textured_rect(
            rect,
            shroud_texture,
            [1.0, 1.0, 1.0, 1.0],
            Some(radar_uv),
            0.0,
        );

        if !hero_object_ids.is_empty() {
            with_window_manager_ref(|manager| {
                if let Some(image) = manager.win_find_image("HeroReticle") {
                    let hero_reticles = radar.build_hero_reticle_rects_for_objects(
                        &hero_object_ids,
                        ul.x,
                        ul.y,
                        scaled_width,
                        scaled_height,
                        image.width,
                        image.height,
                    );
                    for reticle in hero_reticles {
                        manager.win_draw_image(
                            &image,
                            reticle.x1,
                            reticle.y1,
                            reticle.x2,
                            reticle.y2,
                            WIN_COLOR_UNDEFINED,
                        );
                    }
                }
            });
        }

        // Draw active radar events (chirp already consumed by draw_events).
        for event in &events {
            let marker_kind = if event.event_type == RadarEventType::BeaconPulse {
                RadarEventMarkerKind::Beacon
            } else {
                RadarEventMarkerKind::Generic
            };
            let marker = radar_event_marker(
                event,
                current_frame,
                ul.x,
                ul.y,
                scaled_width,
                scaled_height,
                marker_kind,
            );
            let color1 = rgba_int_to_rgba(marker.color1);
            let color2 = rgba_int_to_rgba(marker.color2);
            let points = marker.points;

            renderer.draw_line(
                glam::Vec2::new(points[0].x as f32, points[0].y as f32),
                glam::Vec2::new(points[1].x as f32, points[1].y as f32),
                1.0,
                color1,
                0.0,
            );
            renderer.draw_line(
                glam::Vec2::new(points[1].x as f32, points[1].y as f32),
                glam::Vec2::new(points[2].x as f32, points[2].y as f32),
                1.0,
                color2,
                0.0,
            );
            renderer.draw_line(
                glam::Vec2::new(points[2].x as f32, points[2].y as f32),
                glam::Vec2::new(points[0].x as f32, points[0].y as f32),
                1.0,
                color1,
                0.0,
            );
        }

        let view_box_lines = with_tactical_view_ref(|view| {
            let terrain_z = radar.terrain_average_z();
            let (origin_x, origin_y) = view.origin();
            let origin_world =
                view.screen_to_world_at_z(&IPoint2::new(origin_x, origin_y), terrain_z);
            let corners = view.get_screen_corner_world_points_at_z(terrain_z);
            match (origin_world, corners) {
                (Ok(origin_world), Ok(corners)) => {
                    let to_coord = |point: crate::display::view::Point3| {
                        Coord3D::new(point.x, point.y, point.z)
                    };
                    radar.build_view_box_lines(
                        to_coord(origin_world),
                        [
                            to_coord(corners[0]),
                            to_coord(corners[1]),
                            to_coord(corners[3]),
                            to_coord(corners[2]),
                        ],
                        ul.x,
                        ul.y,
                        scaled_width,
                        scaled_height,
                        x,
                        y,
                        width,
                        height,
                    )
                }
                _ => Vec::new(),
            }
        });
        for line in view_box_lines {
            renderer.draw_line_gradient(
                glam::Vec2::new(line.start.x as f32, line.start.y as f32),
                glam::Vec2::new(line.end.x as f32, line.end.y as f32),
                1.0,
                rgba_int_to_rgba(line.start_color),
                rgba_int_to_rgba(line.end_color),
                0.0,
            );
        }
    });
}

pub(super) fn rgba_int_to_rgba(color: RGBAColorInt) -> [f32; 4] {
    [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.a as f32 / 255.0,
    ]
}

pub fn w3d_right_hud_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    // C++ W3DRightHUDDraw (GeneralsMD W3DControlBar.cpp:74-81): draw the
    // default window art only when WIN_STATUS_IMAGE is set, and paint NOTHING
    // otherwise. The right-HUD frame is scheme background art (ControlBarScheme
    // RightHUDImage), not a window fill; an imageless RightHUD window
    // (ControlBar.wnd authors it without the IMAGE status flag) must stay
    // invisible. The previous fallback slab painted a grey rectangle where
    // retail paints nothing.
    if window.get_status().contains(WindowStatus::IMAGE) {
        crate::gui::game_window::default_draw_callback(window, inst_data);
    }
    if std::env::var_os("GENERALS_RIGHTHUD_PROBE").is_some() {
        static ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !ONCE.swap(true, std::sync::atomic::Ordering::Relaxed) {
            let (x, y) = window.get_screen_position();
            let (w, h) = window.get_size();
            eprintln!(
                "[RHUDPROBE] w3d_right_hud_draw pos=({x},{y}) size=({w},{h}) image_status={}",
                window.get_status().contains(WindowStatus::IMAGE),
            );
        }
    }
}

pub(super) fn log_n(value: f32, base: f32) -> f32 {
    if value <= 0.0 || base <= 1.0 {
        return 0.0;
    }
    value.log10() / base.log10()
}

/// C++ `W3DRadar` Image UV (`lo.y=1`, `hi.y=0`): texture (0,0) is radar south
/// and must draw at the HUD bottom so terrain/overlay/shroud match `radar_to_pixel`.
pub fn radar_layer_vflip_uv() -> UIRect {
    UIRect::new(0.0, 1.0, 1.0, -1.0)
}

pub(super) fn draw_tiled_horiz(
    image: &crate::gui::game_window::Image,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    if width <= 0 || height <= 0 {
        return;
    }
    let tile_width = image.width.max(1);
    with_window_manager_ref(|manager| {
        let mut draw_x = x;
        let end_x = x + width;
        while draw_x < end_x {
            let next_x = (draw_x + tile_width).min(end_x);
            manager.win_draw_image(image, draw_x, y, next_x, y + height, WIN_COLOR_UNDEFINED);
            draw_x += tile_width;
        }
    });
}

pub(super) fn draw_tiled_vert(
    image: &crate::gui::game_window::Image,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    if width <= 0 || height <= 0 {
        return;
    }
    let tile_height = image.height.max(1);
    with_window_manager_ref(|manager| {
        let mut draw_y = y;
        let end_y = y + height;
        while draw_y < end_y {
            let next_y = (draw_y + tile_height).min(end_y);
            manager.win_draw_image(image, x, draw_y, x + width, next_y, WIN_COLOR_UNDEFINED);
            draw_y += tile_height;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::radar_layer_vflip_uv;

    #[test]
    fn radar_layer_uv_puts_texture_origin_at_hud_bottom() {
        let uv = radar_layer_vflip_uv();
        assert!((uv.x - 0.0).abs() < f32::EPSILON);
        assert!((uv.y - 1.0).abs() < f32::EPSILON);
        assert!((uv.width - 1.0).abs() < f32::EPSILON);
        assert!((uv.height + 1.0).abs() < f32::EPSILON);
        let top_left_v = uv.y;
        let bottom_left_v = uv.y + uv.height;
        assert!((top_left_v - 1.0).abs() < f32::EPSILON);
        assert!(bottom_left_v.abs() < f32::EPSILON);
    }
}
