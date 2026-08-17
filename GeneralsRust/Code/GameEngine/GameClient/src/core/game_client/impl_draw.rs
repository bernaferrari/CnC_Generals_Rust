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
        for drawable in self.drawable_map.values_mut() {
            if drawable.is_visible() {
                drawable.draw_icon_ui();
            }
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
        let mut counts = LiveInGameHudDrawCounts::default();
        self.draw_ingame_post_draw_hud(&mut counts);
        self.draw_drawable_icon_overlays(&mut counts);
        self.last_live_ingame_hud_draw = counts;
        counts
    }

    fn packed_ingame_hud_snapshot(
        &self,
    ) -> (
        Vec<String>,
        Option<String>,
        Vec<(String, String, bool)>,
        Vec<(String, [f32; 3], (u8, u8, u8), u32, u32)>,
    ) {
        let Some(ui) = &self.subsystem_manager.in_game_ui else {
            return (Vec::new(), None, Vec::new(), Vec::new());
        };
        let Ok(guard) = ui.lock() else {
            return (Vec::new(), None, Vec::new(), Vec::new());
        };
        let messages: Vec<String> = guard
            .hud_messages()
            .iter()
            .rev()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let subtitle = guard
            .military_subtitles()
            .back()
            .map(|(text, _)| text.clone());
        let timers: Vec<(String, String, bool)> = guard
            .presentation_superweapon_timers()
            .iter()
            .map(|t| (t.name.clone(), t.countdown_text.clone(), t.ready))
            .collect();
        let floating = guard.presentation_floating_texts().to_vec();
        (messages, subtitle, timers, floating)
    }

    fn draw_ingame_post_draw_hud(&self, counts: &mut LiveInGameHudDrawCounts) {
        use crate::display::view::{with_tactical_view_ref, Point3};
        use crate::gui::ui_globals::with_ui_renderer_mut;
        use glam::Vec2;

        let (messages, subtitle, timers, floating) = self.packed_ingame_hud_snapshot();
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
            for text in &messages {
                let _ = renderer.draw_text_simple(
                    text,
                    Vec2::new(10.0, y),
                    10.0,
                    [1.0, 1.0, 1.0, 1.0],
                );
                y += 14.0;
            }

            // C++ military subtitle (InGameUI.cpp:3461-3484). Default pos (10,380)
            // scaled from 800x600 like InGameUI::militarySubtitle.
            if let Some(text) = &subtitle {
                let pos_x = 10.0 * (screen_w / 800.0);
                let pos_y = 380.0 * (screen_h / 600.0);
                let _ = renderer.draw_text_simple(
                    text,
                    Vec2::new(pos_x, pos_y),
                    12.0,
                    [200.0 / 255.0, 200.0 / 255.0, 30.0 / 255.0, 1.0],
                );
            }

            // C++ superweapon timers (InGameUI.cpp:3487-3522). Default 0.7, 0.7.
            let mut sw_y = 0.7 * screen_h;
            let sw_x = 0.7 * screen_w;
            for (name, countdown, ready) in &timers {
                let color = if *ready {
                    [1.0, 1.0, 0.2, 1.0]
                } else {
                    [1.0, 1.0, 1.0, 1.0]
                };
                let line = format!("{name} {countdown}");
                let _ = renderer.draw_text_simple(&line, Vec2::new(sw_x, sw_y), 10.0, color);
                sw_y += 16.0;
            }

            // C++ InGameUI::drawFloatingText (InGameUI.cpp:5082-5115).
            for (text, pos, color, spawn_frame, _timeout) in &floating {
                let age = self.frame.saturating_sub(*spawn_frame) as f32;
                let world = Point3::new(pos[0], pos[1], pos[2]);
                let Some(screen) = with_tactical_view_ref(|view| {
                    view.world_to_screen(&world)
                        .map(|pt| (pt.x as f32, pt.y as f32 - age))
                }) else {
                    continue;
                };
                let rgba = [
                    color.0 as f32 / 255.0,
                    color.1 as f32 / 255.0,
                    color.2 as f32 / 255.0,
                    1.0,
                ];
                let char_w = 8.0 * 0.6;
                let text_w = text.len() as f32 * char_w;
                let _ = renderer.draw_text_simple(
                    text,
                    Vec2::new(screen.0 - text_w * 0.5, screen.1),
                    8.0,
                    rgba,
                );
            }
        });
    }

    fn draw_drawable_icon_overlays(&self, counts: &mut LiveInGameHudDrawCounts) {
        use crate::gui::ui_globals::with_ui_renderer_mut;
        use crate::gui::ui_renderer::UIRect;
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
            .filter(|overlay| overlay.visible && overlay.health_region.is_some())
            .collect();
        counts.icon_overlays = overlays.len() as u32;

        let _ = with_ui_renderer_mut(|renderer| {
            for overlay in &overlays {
                let Some(region) = overlay.health_region else {
                    continue;
                };
                let bar_x = region.lo.x as f32;
                let bar_y = region.lo.y as f32;
                let bar_w = region.width().max(1) as f32;
                let bar_h = region.height().max(3) as f32;

                if overlay.health_ratio > 0.0 {
                    let _ = renderer.draw_rect_with_scissor(
                        UIRect::new(bar_x, bar_y, bar_w, bar_h),
                        [0.2, 0.2, 0.2, 0.7],
                        None,
                    );
                    let fill = if overlay.health_ratio > 0.66 {
                        [0.0, 1.0, 0.0, 0.9]
                    } else if overlay.health_ratio > 0.33 {
                        [1.0, 1.0, 0.0, 0.9]
                    } else {
                        [1.0, 0.0, 0.0, 0.9]
                    };
                    let _ = renderer.draw_rect_with_scissor(
                        UIRect::new(bar_x, bar_y, bar_w * overlay.health_ratio, bar_h),
                        fill,
                        None,
                    );
                }

                if overlay.is_under_construction {
                    let pct = (overlay.construction_percent * 100.0).round() as i32;
                    let _ = renderer.draw_text_simple(
                        &format!("{pct}%"),
                        Vec2::new(bar_x, bar_y - 12.0),
                        10.0,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }

                if overlay.veterancy_level > 0 {
                    let pip = 4.0;
                    let pip_x = bar_x + bar_w + 2.0;
                    for i in 0..overlay.veterancy_level.min(3) {
                        let _ = renderer.draw_rect_with_scissor(
                            UIRect::new(pip_x, bar_y - (i as f32) * (pip + 1.0), pip, pip),
                            [1.0, 0.85, 0.0, 1.0],
                            None,
                        );
                    }
                }

                if overlay.show_ammo && overlay.ammo_total > 0 {
                    let pip = 3.0;
                    let pip_y = bar_y + bar_h + 2.0;
                    for i in 0..overlay.ammo_total.min(12) {
                        let filled = i < overlay.ammo_full;
                        let color = if filled {
                            [0.95, 0.85, 0.2, 1.0]
                        } else {
                            [0.25, 0.25, 0.25, 0.8]
                        };
                        let _ = renderer.draw_rect_with_scissor(
                            UIRect::new(bar_x + (i as f32) * (pip + 1.0), pip_y, pip, pip),
                            color,
                            None,
                        );
                    }
                }

                if overlay.show_contained && overlay.contained_total > 0 {
                    let pip = 3.0;
                    let pip_y = bar_y + bar_h + 6.0;
                    for i in 0..overlay.contained_total.min(12) {
                        let filled = i < overlay.contained_full;
                        let infantry = i < overlay.contained_infantry_count;
                        let color = if !filled {
                            [0.25, 0.25, 0.25, 0.8]
                        } else if infantry {
                            [0.2, 0.85, 0.2, 1.0]
                        } else {
                            [0.2, 0.45, 0.95, 1.0]
                        };
                        let _ = renderer.draw_rect_with_scissor(
                            UIRect::new(bar_x + (i as f32) * (pip + 1.0), pip_y, pip, pip),
                            color,
                            None,
                        );
                    }
                }

                if let Some(caption) = overlay.caption.as_deref() {
                    if !caption.is_empty() {
                        let _ = renderer.draw_text_simple(
                            caption,
                            Vec2::new(bar_x, bar_y - 24.0),
                            10.0,
                            [1.0, 1.0, 1.0, 1.0],
                        );
                    }
                }
            }
        });
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
