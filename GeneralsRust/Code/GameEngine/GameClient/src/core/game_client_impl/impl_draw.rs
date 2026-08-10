// GameClient draw/display presentation helpers.
// Split from `core/game_client.rs` dump. Included by `game_client_impl/mod.rs`
// so this stays one logical `game_client` module (public API identical).

impl GameClient {
    pub fn draw_display(&mut self) -> GameClientResult<()> {
        if let Some(ref display) = self.subsystem_manager.display {
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

    /// Wave 978: host presentation selection health bars (InGameUI residual).
    ///
    /// Full InGameUI::draw is not on the presentation shell path; stamp residual
    /// selection HUD here so empty dual-world still shows selection health.
    fn draw_presentation_selection_residual(&mut self) {
        let Some(ref ui) = self.subsystem_manager.in_game_ui else {
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

        let _ = with_ui_renderer_mut(|renderer| {
            for u in &units {
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
