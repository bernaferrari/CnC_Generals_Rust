// GameClient draw/display presentation helpers.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

/// Counts for the live InGameUI postDraw / Drawable::drawIconUI submit.
///
/// C++ InGameUI.cpp:1571 preDraw (floating text) and :3426 postDraw
/// (messages, military subtitle, superweapon timers) plus
/// Drawable::drawIconUI health/pips/chevrons/construct%.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LiveInGameHudDrawCounts {
    pub messages: u32,
    pub military_subtitles: u32,
    pub superweapon_timers: u32,
    pub floating_texts: u32,
    pub icon_overlays: u32,
}

impl LiveInGameHudDrawCounts {
    pub fn total(self) -> u32 {
        self.messages
            + self.military_subtitles
            + self.superweapon_timers
            + self.floating_texts
            + self.icon_overlays
    }
}

impl GameClient {
    pub fn draw_display(&mut self) -> GameClientResult<()> {
        if let Some(display) = &self.subsystem_manager.display {
            display.lock().unwrap_or_else(|e| e.into_inner()).draw()?;
        }
        Ok(())
    }

    fn install_load_screen_presentation_pump(display: Arc<Mutex<GraphicsDisplay>>) {
        register_load_screen_presentation_pump(move || {
            let mut display = display.lock().unwrap_or_else(|e| e.into_inner());
            if let Err(err) = display.update() {
                log::warn!("Load-screen display update failed: {err}");
                return;
            }
            if let Err(err) = display.draw() {
                log::warn!("Load-screen display draw failed: {err}");
            }
        });
    }

    fn draw_drawable_icon_ui(&mut self) {
        let mut text_ids = Vec::new();
        for (id, drawable) in self.drawable_map.iter_mut() {
            if drawable.is_visible() {
                drawable.draw_icon_ui();
                if let Some(basic) = drawable.downcast_ref::<crate::drawable::drawable::BasicDrawable>()
                {
                    if basic.overlay_data.queue_ui_text {
                        text_ids.push(*id);
                    }
                }
            }
        }
        for id in text_ids {
            self.add_text_bearing_drawable(id);
        }
    }


    pub fn last_live_ingame_hud_draw(&self) -> LiveInGameHudDrawCounts {
        self.last_live_ingame_hud_draw
    }

    /// Queue already-packed InGameUI HUD + icon overlays onto the live UIRenderer.
    ///
    /// Called from the presentation-shell / host present path so
    /// `flush_ui_to_frame` submits them after the 3D scene (C++ postDraw).
    pub fn draw_live_ingame_hud(&mut self) -> LiveInGameHudDrawCounts {
        let frame = gamelogic::helpers::TheGameLogic::get_frame();
        if let Some(ui) = &self.subsystem_manager.in_game_ui {
            if let Ok(mut guard) = ui.lock() {
                guard.expire_hud_messages(frame);
            }
        }
        let mut counts = LiveInGameHudDrawCounts::default();
        self.draw_ingame_post_draw_hud(&mut counts);
        self.draw_drawable_icon_overlays(&mut counts);
        let _ = self.flush_text_bearing_drawables();
        self.last_live_ingame_hud_draw = counts;
        counts
    }



    fn packed_ingame_hud_snapshot(
        &self,
    ) -> (
        Vec<(String, [f32; 4])>,
        Option<String>,
        Vec<(String, String, bool)>,
        Vec<(String, [f32; 3], (u8, u8, u8, u8), u32, u32)>,
        Vec<(String, [f32; 3], f32, f32, bool, u32)>,
    ) {
        let Some(ui) = &self.subsystem_manager.in_game_ui else {
            return (
                Vec::new(),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
        };
        let Ok(guard) = ui.lock() else {
            return (
                Vec::new(),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
        };
        let messages: Vec<(String, [f32; 4])> = guard
            .hud_messages()
            .iter()
            .rev()
            .take(6)
            .map(|m| {
                let a = ((m.color >> 24) & 0xFF) as f32 / 255.0;
                let r = ((m.color >> 16) & 0xFF) as f32 / 255.0;
                let g = ((m.color >> 8) & 0xFF) as f32 / 255.0;
                let b = (m.color & 0xFF) as f32 / 255.0;
                (m.text.clone(), [r, g, b, a])
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let frame = gamelogic::helpers::TheGameLogic::get_frame();
        crate::gui::ingame_ui::step_live_hud(frame);
        let subtitle = crate::gui::ingame_ui::live_military_subtitle_draw(frame)
            .map(|(text, _, _, _, _)| text);
        // C++ draws named timers (0.05,0.7) and superweapon timers (0.7,0.7)
        // independently. Named lines must not replace the SW strip.
        let timers: Vec<(String, String, bool)> = guard
            .presentation_superweapon_timers()
            .iter()
            .map(|t| (t.name.clone(), t.countdown_text.clone(), t.ready))
            .collect();
        let floating = guard.presentation_floating_texts().to_vec();
        let world_anims = guard.presentation_world_anims().to_vec();
        (messages, subtitle, timers, floating, world_anims)
    }

    fn draw_ingame_post_draw_hud(&self, counts: &mut LiveInGameHudDrawCounts) {
        use crate::display::view::{with_tactical_view_ref, Point3};
        use crate::gui::ui_globals::with_ui_renderer_mut;
        use glam::Vec2;

        let (messages, subtitle, timers, floating, world_anims) =
            self.packed_ingame_hud_snapshot();
        counts.messages = messages.len() as u32;
        counts.military_subtitles = u32::from(subtitle.is_some());
        counts.superweapon_timers = timers.len() as u32;
        counts.floating_texts = floating.len() as u32;

        let _ = with_ui_renderer_mut(|renderer| {
            let (screen_w, screen_h) = renderer.screen_size();
            let screen_w = if screen_w == 0 {
                1024.0
            } else {
                screen_w as f32
            };
            let screen_h = if screen_h == 0 {
                768.0
            } else {
                screen_h as f32
            };

            // C++ InGameUI::postDraw messages (InGameUI.cpp:3429-3458).
            let mut y = 10.0;
            for (text, color) in &messages {
                // C++ dropColor=black with fill alpha; W3DDisplayString::draw uses +1,+1.
                let drop = [0.0, 0.0, 0.0, color[3]];
                let _ = renderer.draw_text_simple(
                    text,
                    Vec2::new(11.0, y + 1.0),
                    10.0,
                    drop,
                );
                let _ = renderer.draw_text_simple(text, Vec2::new(10.0, y), 10.0, *color);
                y += 14.0;
            }


            // C++ military subtitle (InGameUI.cpp:3461-3484) — typed lines + block.
            let frame = gamelogic::helpers::TheGameLogic::get_frame();
            if let Some((text, block_drawn, color, pos, block_pos)) =
                crate::gui::ingame_ui::live_military_subtitle_draw(frame)
            {
                let a = ((color >> 24) & 0xFF) as f32 / 255.0;
                let r = ((color >> 16) & 0xFF) as f32 / 255.0;
                let g = ((color >> 8) & 0xFF) as f32 / 255.0;
                let b = (color & 0xFF) as f32 / 255.0;
                let rgba = [r, g, b, a];
                let mut y = pos.1 * (screen_h / 600.0);
                let x = pos.0 * (screen_w / 800.0);
                for line in text.split('\n') {
                    let drop = [0.0, 0.0, 0.0, a];
                    let _ = renderer.draw_text_simple(
                        line,
                        Vec2::new(x + 1.0, y + 1.0),
                        12.0,
                        drop,
                    );
                    let _ = renderer.draw_text_simple(line, Vec2::new(x, y), 12.0, rgba);
                    y += 12.0;
                }
                if block_drawn {
                    let _ = renderer.draw_rect(
                        crate::gui::ui_renderer::UIRect::new(
                            block_pos.0 * (screen_w / 800.0),
                            block_pos.1 * (screen_h / 600.0),
                            10.0,
                            12.0,
                        ),
                        rgba,
                        0.0,
                    );
                }
            }

            // Named timers (C++ InGameUI.cpp:3699-3784) at constructor pos 0.05, 0.7.
            let named = crate::gui::ingame_ui::live_named_timer_draw(frame);
            let mut nt_y = 0.7 * screen_h;
            let nt_x = 0.05 * screen_w;
            for (text, color, ready) in &named {
                let a = ((color >> 24) & 0xFF) as f32 / 255.0;
                let r = ((color >> 16) & 0xFF) as f32 / 255.0;
                let g = ((color >> 8) & 0xFF) as f32 / 255.0;
                let b = (color & 0xFF) as f32 / 255.0;
                let size = if *ready { 10.0 } else { 10.0 };
                let _ = renderer.draw_text_simple(text, Vec2::new(nt_x, nt_y), size, [r, g, b, a]);
                nt_y -= 12.0;
            }

            // C++ superweapon timers (InGameUI.cpp:3487-3678). Default 0.7, 0.7.
            // READY blinks flash color vs default (color 0) and uses ready font size.
            let mut sw_y = 0.7 * screen_h;
            let sw_x = 0.7 * screen_w;
            for (name, countdown, ready) in &timers {
                let (color, size) =
                    crate::gui::ingame_ui::live_superweapon_draw_style(frame, *ready);
                let line = format!("{name} {countdown}");
                let _ = renderer.draw_text_simple(&line, Vec2::new(sw_x, sw_y), size, color);
                sw_y += 16.0;
            }

            // C++ InGameUI::drawFloatingText (InGameUI.cpp:5082-5115).
            for (text, pos, color, spawn_frame, timeout) in &floating {
                let timeout_frames = (*timeout).max(1);
                let frame_timeout = spawn_frame.saturating_add(timeout_frames);
                let spawn_alpha = color.3;
                let alpha_u8 = crate::gui::ingame_ui::InGameUI::floating_text_alpha_at_frame(
                    spawn_alpha,
                    self.frame,
                    frame_timeout,
                    0.1,
                );
                if alpha_u8 == 0 {
                    continue;
                }
                let coord = crate::system::Coord3D::new(pos[0], pos[1], pos[2]);
                if self.get_shroud_status_for_player(self.local_player_id, &coord)
                    != ShroudStatus::Clear
                {
                    continue;
                }
                let lift = crate::gui::ingame_ui::InGameUI::floating_text_screen_offset_y(
                    self.frame.saturating_sub(*spawn_frame),
                    1.0,
                );
                let world = Point3::new(pos[0], pos[1], pos[2]);
                let Some(screen) = with_tactical_view_ref(|view| {
                    view.world_to_screen(&world)
                        .map(|pt| (pt.x as f32, pt.y as f32 - lift))
                }) else {
                    continue;
                };
                let rgba = crate::gui::ingame_ui::InGameUI::floating_text_draw_rgba(
                    (color.0, color.1, color.2),
                    alpha_u8,
                );
                let drop = [0.0, 0.0, 0.0, rgba[3]];
                let char_w = 8.0 * 0.6;
                let text_w = text.len() as f32 * char_w;
                let x = screen.0 - text_w * 0.5;
                let _ = renderer.draw_text_simple(
                    text,
                    Vec2::new(x + 1.0, screen.1 + 1.0),
                    8.0,
                    drop,
                );
                let _ = renderer.draw_text_simple(text, Vec2::new(x, screen.1), 8.0, rgba);
            }

            // C++ InGameUI::updateAndDrawWorldAnimations (InGameUI.cpp:5323-5418).
            for (template, pos, display, z_rise, fades, spawn_frame) in &world_anims {
                let expire = crate::gui::ingame_ui::InGameUI::world_anim_expire_frame(
                    *spawn_frame,
                    *display,
                );
                if self.frame >= expire {
                    continue;
                }
                let age = self.frame.saturating_sub(*spawn_frame);
                let lift =
                    crate::gui::ingame_ui::InGameUI::world_anim_z_lift(age, *z_rise);
                let world = Point3::new(pos[0], pos[1], pos[2] + lift);
                let coord = crate::system::Coord3D::new(pos[0], pos[1], pos[2] + lift);
                if self.get_shroud_status_for_player(self.local_player_id, &coord)
                    != ShroudStatus::Clear
                {
                    continue;
                }
                let frames_till = expire.saturating_sub(self.frame);
                let alpha = crate::gui::ingame_ui::InGameUI::world_anim_fade_alpha(
                    frames_till,
                    *fades,
                );
                if alpha <= 0.0 {
                    continue;
                }
                let Some(screen) = with_tactical_view_ref(|view| view.world_to_screen(&world))
                else {
                    continue;
                };
                let zoom_scale = with_tactical_view_ref(|view| {
                    let zoom = view.zoom();
                    if zoom > 0.0 {
                        view.max_zoom() / zoom
                    } else {
                        1.0
                    }
                });
                draw_money_pickup_anim2d(
                    template,
                    screen.x as f32,
                    screen.y as f32,
                    zoom_scale,
                    alpha,
                    age,
                    renderer,
                );
            }


            // C++ W3DMouse.cpp:565-567 / Mouse.cpp:963-1023 — tooltip after delay.
            crate::gui::ui_globals::tick_cursor_tooltip();
            let _ = crate::gui::ui_globals::submit_cursor_tooltip(renderer);
        });
    }

    fn draw_drawable_icon_overlays(&self, counts: &mut LiveInGameHudDrawCounts) {
        use crate::drawable::drawable::{format_under_construction_desc, health_bar_colors};
        use crate::gui::ui_globals::with_ui_renderer_mut;
        use crate::gui::ui_renderer::UIRect;
        use crate::gui::window_manager::with_window_manager_ref;
        use glam::Vec2;

        let overlays: Vec<crate::drawable::drawable::DrawableOverlayData> = self
            .drawable_map
            .values()
            .filter(|drawable| drawable.is_visible())
            .filter_map(|drawable| {
                drawable
                    .downcast_ref::<crate::drawable::drawable::BasicDrawable>()
                    .map(|basic| basic.overlay_data.clone())
            })
            .filter(|overlay| {
                overlay.visible
                    && (overlay.health_region.is_some()
                        || overlay.caption.as_ref().is_some_and(|c| !c.is_empty()))
            })
            .collect();
        counts.icon_overlays = overlays.len() as u32;

        let _ = with_ui_renderer_mut(|renderer| {
            for overlay in &overlays {
                let Some(region) = overlay.health_region else {
                    draw_cpp_drawable_caption(renderer, overlay);
                    continue;
                };
                let bar_x = region.lo.x as f32;
                let bar_y = region.lo.y as f32;
                let bar_w = region.width().max(1) as f32;
                let bar_h = region.height().max(3) as f32;

                if overlay.health_bar_visible && overlay.health_ratio > 0.0 {
                    let (fill, outline) = if overlay.health_fill[3] > 0.0 {
                        (overlay.health_fill, overlay.health_outline)
                    } else {
                        health_bar_colors(overlay.health_ratio, false, false, false)
                    };
                    let _ = renderer.draw_rect_outline_with_scissor(
                        UIRect::new(bar_x, bar_y, bar_w, bar_h),
                        1.0,
                        outline,
                        None,
                    );
                    let _ = renderer.draw_rect_with_scissor(
                        UIRect::new(bar_x + 1.0, bar_y + 1.0, (bar_w - 2.0) * overlay.health_ratio, bar_h - 2.0),
                        fill,
                        None,
                    );
                }

                if overlay.is_under_construction {
                    let text = overlay.construct_text.clone().unwrap_or_else(|| {
                        format_under_construction_desc(overlay.construction_percent)
                    });
                    let char_w = 6.0;
                    let text_x = bar_x + bar_w * 0.5 - text.len() as f32 * char_w * 0.5;
                    let _ = renderer.draw_text_simple(
                        &text,
                        Vec2::new(text_x + 1.0, bar_y - 11.0),
                        10.0,
                        [0.0, 0.0, 0.0, 1.0],
                    );
                    let _ = renderer.draw_text_simple(
                        &text,
                        Vec2::new(text_x, bar_y - 12.0),
                        10.0,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }

                if overlay.veterancy_level > 0 {
                    if let Some(name) = crate::drawable::drawable::BasicDrawable::veterancy_image_name(
                        overlay.veterancy_level,
                    ) {
                        // C++ Drawable.cpp:3785-3818 — one SCVeter1/2/3 image
                        // anchored at the health-box right edge + (1,1).
                        // SCALE_ICONS_WITH_ZOOM_ML is off in retail ZH, so
                        // objScale is 1.0 (native mapped-image size).
                        let _ = with_window_manager_ref(|manager| {
                            if let Some(image) = manager.win_find_image(name) {
                                let w = image.width.max(1);
                                let h = image.height.max(1);
                                let x1 = (bar_x + bar_w).round() as i32 + 1;
                                let y1 = bar_y.round() as i32 + 1;
                                manager.win_draw_image(
                                    &image,
                                    x1,
                                    y1,
                                    x1 + w,
                                    y1 + h,
                                    crate::gui::WIN_COLOR_UNDEFINED,
                                );
                            }
                        });
                    }
                }

                if overlay.show_ammo && overlay.ammo_total > 0 {
                    let pip = 8.0;
                    let pip_y = bar_y + bar_h + 2.0;
                    for i in 0..overlay.ammo_total.min(12) {
                        let filled = i < overlay.ammo_full;
                        let name = if filled { "SCPAmmoFull" } else { "SCPAmmoEmpty" };
                        let x = bar_x + (i as f32) * (pip + 1.0);
                        let drew = with_window_manager_ref(|manager| {
                            if let Some(image) = manager.win_find_image(name) {
                                manager.win_draw_image(
                                    &image,
                                    x as i32,
                                    pip_y as i32,
                                    (x + pip) as i32,
                                    (pip_y + pip) as i32,
                                    crate::gui::WIN_COLOR_UNDEFINED,
                                );
                                true
                            } else {
                                false
                            }
                        });
                        if !drew {
                            let color = if filled {
                                [0.95, 0.85, 0.2, 1.0]
                            } else {
                                [0.25, 0.25, 0.25, 0.8]
                            };
                            let _ = renderer.draw_rect_with_scissor(
                                UIRect::new(x, pip_y, 3.0, 3.0),
                                color,
                                None,
                            );
                        }
                    }
                }

                if overlay.show_contained && overlay.contained_total > 0 {
                    let pip = 8.0;
                    let pip_y = bar_y + bar_h + 12.0;
                    for i in 0..overlay.contained_total.min(12) {
                        let filled = i < overlay.contained_full;
                        let infantry = i < overlay.contained_infantry_count;
                        let name = if filled { "SCPPipFull" } else { "SCPPipEmpty" };
                        let x = bar_x + (i as f32) * (pip + 1.0);
                        let tint = if !filled {
                            crate::gui::WIN_COLOR_UNDEFINED
                        } else if infantry {
                            0xFF00_FF00 // C++ INFANTRY_COLOR GameMakeColor(0,255,0,255)
                        } else {
                            0xFF00_00FF // C++ NON_INFANTRY_COLOR GameMakeColor(0,0,255,255)
                        };
                        let drew = with_window_manager_ref(|manager| {
                            if let Some(image) = manager.win_find_image(name) {
                                manager.win_draw_image(
                                    &image,
                                    x as i32,
                                    pip_y as i32,
                                    (x + pip) as i32,
                                    (pip_y + pip) as i32,
                                    tint,
                                );
                                true
                            } else {
                                false
                            }
                        });
                        if !drew {
                            let color = if !filled {
                                [0.25, 0.25, 0.25, 0.8]
                            } else if infantry {
                                [0.2, 0.85, 0.2, 1.0]
                            } else {
                                [0.2, 0.45, 0.95, 1.0]
                            };
                            let _ = renderer.draw_rect_with_scissor(
                                UIRect::new(x, pip_y, 3.0, 3.0),
                                color,
                                None,
                            );
                        }
                    }
                }

                // Anim2D overlays are submitted after this lock (Anim2D::draw
                // acquires the UI renderer itself). Colored rects stay as a
                // fallback when the Anim2D template is missing.


                if let Some(numeral) = overlay.group_numeral.as_deref() {
                    let _ = renderer.draw_text_simple(
                        numeral,
                        Vec2::new(bar_x + 1.0, bar_y + bar_h + 2.0),
                        12.0,
                        [0.0, 0.0, 0.0, 1.0],
                    );
                    let _ = renderer.draw_text_simple(
                        numeral,
                        Vec2::new(bar_x, bar_y + bar_h + 1.0),
                        12.0,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }
                if let Some(letter) = overlay.formation_letter.as_deref() {
                    let _ = renderer.draw_text_simple(
                        letter,
                        Vec2::new(bar_x + 12.0, bar_y + bar_h + 1.0),
                        12.0,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }

                draw_cpp_drawable_caption(renderer, overlay);
            }
        });

        for overlay in &overlays {
            draw_overlay_anim2d_icons(overlay);
        }
    }


    fn icon_overlay_object_ids(&self) -> std::collections::HashSet<u32> {
        self.drawable_map
            .values()
            .filter_map(|drawable| {
                let basic = drawable.downcast_ref::<crate::drawable::drawable::BasicDrawable>()?;
                if basic.overlay_data.visible && basic.overlay_data.health_region.is_some() {
                    drawable.get_object_id()
                } else {
                    None
                }
            })
            .collect()
    }

    /// Wave 978: host presentation selection health bars (InGameUI residual).
    ///
    /// Full InGameUI::draw is not on the presentation shell path; stamp residual
    /// selection HUD here so empty dual-world still shows selection health.
    fn draw_presentation_selection_residual(&mut self) {
        let Some(ui) = &self.subsystem_manager.in_game_ui else {
            return;
        };
        let Ok(guard) = ui.lock() else {
            return;
        };
        let units: Vec<_> = guard.presentation_selection_residual().to_vec();
        drop(guard);
        if units.is_empty() {
            return;
        }

        use crate::display::view::{with_tactical_view_ref, Point3};
        use crate::gui::ui_globals::with_ui_renderer_mut;
        use crate::gui::ui_renderer::UIRect;

        let overlay_ids = self.icon_overlay_object_ids();
        let _ = with_ui_renderer_mut(|renderer| {
            for u in &units {
                if overlay_ids.contains(&u.object_id) {
                    continue;
                }
                let world_pt = Point3::new(u.position[0], u.position[1], u.position[2]);
                let Some(screen) = with_tactical_view_ref(|view| {
                    view.world_to_screen(&world_pt)
                        .map(|pt| (pt.x as f32, pt.y as f32))
                }) else {
                    continue;
                };
                let health_pct = u.health_pct.clamp(0.0, 1.0);
                if health_pct <= 0.0 {
                    continue;
                }
                let bar_width = 40.0;
                let bar_height = 4.0;
                let bar_x = screen.0 - bar_width / 2.0;
                let bar_y = screen.1 - 30.0;
                let _ = renderer.draw_rect_with_scissor(
                    UIRect::new(bar_x, bar_y, bar_width, bar_height),
                    [0.2, 0.2, 0.2, 0.7],
                    None,
                );
                let fill_color = if health_pct > 0.66 {
                    [0.0, 1.0, 0.0, 0.9]
                } else if health_pct > 0.33 {
                    [1.0, 1.0, 0.0, 0.9]
                } else {
                    [1.0, 0.0, 0.0, 0.9]
                };
                let _ = renderer.draw_rect_with_scissor(
                    UIRect::new(bar_x, bar_y, bar_width * health_pct, bar_height),
                    fill_color,
                    None,
                );
            }
        });
    }
}

/// C++ `Anim2D::draw` for drawable HUD icons (`Drawable.cpp` heal/bomb/disabled/enthusiastic/emoticon).
fn draw_overlay_anim2d_icons(overlay: &crate::drawable::drawable::DrawableOverlayData) {
    use crate::drawable::drawable::{Anim2DIcon, ICoord2D, Vector3};
    use crate::gui::ui_globals::with_ui_renderer_mut;
    use crate::gui::ui_renderer::UIRect;

    let Some(region) = overlay.health_region else {
        return;
    };
    let bar_w = region.width().max(1) as f32;
    let bar_h = region.height().max(1) as f32;
    let lo = region.lo;
    let hi = ICoord2D::new(region.lo.x + region.width(), region.lo.y + region.height());

    let mut try_icon = |template: &str, x: f32, y: f32, w: f32, h: f32, fallback: [f32; 4]| {
        if let Ok(icon) = Anim2DIcon::from_template_name(template) {
            icon.render(Vector3::new(x, y, 0.0), Vector3::new(w.max(1.0), h.max(1.0), 0.0));
            return;
        }
        let _ = with_ui_renderer_mut(|renderer| {
            let _ = renderer.draw_rect_with_scissor(UIRect::new(x, y, w.max(4.0), h.max(4.0)), fallback, None);
        });
    };

    if overlay.show_healing {
        let (name, scale) = match overlay.healing_icon_type {
            1 => ("StructureHeal", 0.33),
            2 => ("VehicleHeal", 0.7),
            _ => ("DefaultHeal", 0.7),
        };
        let size = bar_w * scale;
        let x = lo.x as f32 + bar_w * 0.75 - size * 0.5;
        let y = lo.y as f32 - size;
        try_icon(name, x, y, size, size, [0.35, 1.0, 0.45, 0.95]);
    }
    if overlay.show_bombed {
        let (name, scale) = match overlay.bomb_type {
            2 => ("BombRemote", 0.65),
            3 => ("CarBomb", 0.5),
            _ => ("BombTimed", 0.65),
        };
        let size = bar_w * scale;
        let x = lo.x as f32 + bar_w * 0.5 - size * 0.5;
        let y = lo.y as f32 + bar_h * 0.5 + 5.0;
        try_icon(name, x, y, size, size, [1.0, 0.75, 0.1, 0.95]);
        if overlay.bomb_type == 1 {
            try_icon(
                "BombRemote",
                x,
                y,
                size,
                size,
                [1.0, 0.45, 0.1, 0.95],
            );
        }
    }
    if overlay.show_disabled {
        let size = bar_w * 0.3;
        let x = lo.x as f32;
        let y = hi.y as f32 - (size + bar_h);
        try_icon("Disabled", x, y, size, size, [0.55, 0.85, 1.0, 0.95]);
    }
    if overlay.show_enthusiastic {
        let name = if overlay.show_subliminal {
            "Subliminal"
        } else {
            "Enthusiastic"
        };
        let size = bar_w * 0.5;
        let x = lo.x as f32 + bar_w * 0.25 - size * 0.5;
        let y = hi.y as f32 + size * 0.25;
        try_icon(name, x, y, size, size, [1.0, 0.9, 0.2, 0.95]);
    }
    if overlay.show_emoticon {
        let size = bar_w * 0.3;
        let x = lo.x as f32 + bar_w * 0.5 - size * 0.5;
        let y = hi.y as f32 - size;
        try_icon("Emoticon", x, y, size, size, [1.0, 0.75, 0.85, 0.95]);
    }
}

/// C++ `Anim2D::draw` for InGameUI world animations (`MoneyPickUp` SCPDollar loop).
fn draw_money_pickup_anim2d(
    template: &str,
    screen_x: f32,
    screen_y: f32,
    zoom_scale: f32,
    alpha: f32,
    age_frames: u32,
    renderer: &mut crate::gui::ui_renderer::UIRenderer,
) {
    use crate::system::Anim2D;
    use game_engine::common::ascii_string::AsciiString;
    use game_engine::common::ini::get_anim2d_collection;
    use glam::Vec2;

    if let Some(collection) = get_anim2d_collection() {
        if let Some(tmpl) = collection
            .read()
            .find_template(&AsciiString::from(template))
        {
            let num_frames = tmpl.read().get_num_frames().max(1);
            let anim = Anim2D::new(tmpl, None);
            let mut guard = anim.lock();
            let frame = (age_frames % u32::from(num_frames)) as u16;
            guard.set_current_frame(frame);
            guard.set_alpha(alpha);
            let width = guard.get_current_frame_width() as f32 * zoom_scale;
            let height = guard.get_current_frame_height() as f32 * zoom_scale;
            if width > 0.0 && height > 0.0 {
                guard.draw_sized(
                    (screen_x - width * 0.5) as i32,
                    (screen_y - height * 0.5) as i32,
                    width as i32,
                    height as i32,
                );
                return;
            }
        }
    }
    let size = (24.0 * zoom_scale).max(8.0);
    let _ = renderer.draw_text_simple(
        "$",
        Vec2::new(screen_x - size * 0.25, screen_y - size * 0.5),
        size,
        [0.2, 0.85, 0.25, alpha],
    );
}

/// C++ `Drawable::drawCaption` (Drawable.cpp:3737-3768).
fn draw_cpp_drawable_caption(
    renderer: &mut crate::gui::ui_renderer::UIRenderer,
    overlay: &crate::drawable::drawable::DrawableOverlayData,
) {
    use crate::display::view::{with_tactical_view_ref, Point3};
    use crate::gui::ui_renderer::UIRect;
    use glam::Vec2;

    let Some(caption) = overlay.caption.as_deref().filter(|c| !c.is_empty()) else {
        return;
    };
    let Some(world) = overlay
        .caption_world
        .map(|p| Point3::new(p[0], p[1], p[2]))
    else {
        return;
    };
    let Some(screen) = with_tactical_view_ref(|view| {
        view.world_to_screen(&world)
            .map(|pt| (pt.x as f32, pt.y as f32))
    }) else {
        return;
    };
    // C++ constructor defaults (InGameUI.cpp:1017-1020): Arial 10 white.
    let point_size = 10.0;
    let color_u32 = 0xFFFF_FFFFu32;
    let a = ((color_u32 >> 24) & 0xFF) as f32 / 255.0;
    let r = ((color_u32 >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color_u32 >> 8) & 0xFF) as f32 / 255.0;
    let b = (color_u32 & 0xFF) as f32 / 255.0;
    let rgba = [r, g, b, a];
    let char_w = point_size * 0.6;
    let text_w = caption.len() as f32 * char_w;
    let text_h = point_size;
    let x = screen.0 - text_w * 0.5;
    let y = screen.1;
    let _ = renderer.draw_rect(
        UIRect::new(x - 1.0, y - 1.0, text_w + 2.0, text_h + 2.0),
        [0.0, 0.0, 0.0, 125.0 / 255.0],
        0.0,
    );
    let _ = renderer.draw_rect_outline(
        UIRect::new(x - 1.0, y - 1.0, text_w + 2.0, text_h + 2.0),
        1.0,
        [20.0 / 255.0, 20.0 / 255.0, 20.0 / 255.0, 1.0],
        0.0,
    );
    let _ = renderer.draw_text_simple(
        caption,
        Vec2::new(x + 1.0, y + 1.0),
        point_size,
        [0.0, 0.0, 0.0, a],
    );
    let _ = renderer.draw_text_simple(caption, Vec2::new(x, y), point_size, rgba);
}


