// Minimap bounds, unit icons, and ping animations.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

impl InGameUI {
    pub fn set_minimap_world_bounds(&mut self, min: Vec2, max: Vec2) {
        self.minimap.world_bounds = (min, max);
    }

    /// Update minimap camera position
    pub fn update_minimap_unit(&mut self, id: u32, world_pos: Vec2, color: [f32; 4]) {
        self.minimap.update_icon(DrawableID(id), world_pos, color);
    }

    /// Remove unit from minimap
    pub fn remove_minimap_unit(&mut self, id: u32) {
        self.minimap.remove_icon(DrawableID(id));
    }

    /// Select object
    fn draw_minimap_pings(&self, renderer: &mut UIRenderer) -> std::result::Result<(), String> {
        for ping in &self.minimap_pings {
            let elapsed = self.current_frame.saturating_sub(ping.creation_frame);
            if elapsed >= ping.lifetime_frames {
                continue;
            }

            let progress = elapsed as f32 / ping.lifetime_frames as f32;
            let alpha = 1.0 - progress;
            let minimap_pos = self.minimap.world_to_minimap(ping.world_pos);
            let max_radius = 15.0f32;
            let radius = max_radius * progress;

            let segments = 16u32;
            for i in 0..segments {
                let a1 = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
                let a2 = ((i + 1) as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
                let x1 = minimap_pos.x + radius * a1.cos();
                let y1 = minimap_pos.y + radius * a1.sin();
                let x2 = minimap_pos.x + radius * a2.cos();
                let y2 = minimap_pos.y + radius * a2.sin();
                let color = [
                    ping.color[0],
                    ping.color[1],
                    ping.color[2],
                    alpha * ping.color[3],
                ];
                renderer.draw_line(
                    Vec2::new(x1, y1),
                    Vec2::new(x2, y2),
                    1.5,
                    [0.0, 1.0, 0.0, 0.6],
                    0.0,
                );
            }
        }
        Ok(())
    }

    /// Add a minimap ping at the given world position.
    pub fn add_minimap_ping(&mut self, world_pos: Vec2, color: [f32; 4], lifetime_frames: u32) {
        self.minimap_pings.push(MinimapPing {
            world_pos,
            color,
            creation_frame: self.current_frame,
            lifetime_frames,
        });
    }

    /// Expire old minimap pings.
    pub fn expire_minimap_pings(&mut self) {
        self.minimap_pings
            .retain(|p| self.current_frame < p.creation_frame + p.lifetime_frames);
    }

    // ── Control group methods ────────────────────────────────────────
    // C++: InGameUI has 10 control groups (0-9), mapped to Ctrl+0 through Ctrl+9

    /// Add a single object to a control group. C++: binds object to group number.
    fn render_minimap(&self, renderer: &mut UIRenderer) -> Result<()> {
        let minimap_rect = UIRect::new(
            self.minimap.position.x,
            self.minimap.position.y,
            self.minimap.size.x,
            self.minimap.size.y,
        );

        // Draw minimap background
        renderer.draw_rect_with_scissor(minimap_rect, [0.1, 0.1, 0.1, 0.8], None)?;

        // Draw border
        renderer.draw_rect_outline_with_scissor(minimap_rect, 2.0, [0.5, 0.5, 0.5, 1.0], None)?;

        // Draw camera viewport indicator
        let cam_pos_2d = Vec2::new(
            self.minimap.camera_position.x,
            self.minimap.camera_position.z,
        );
        let cam_minimap = self.minimap.world_to_minimap(cam_pos_2d);
        let viewport_size = self.minimap.camera_viewport
            * (self.minimap.size / (self.minimap.world_bounds.1 - self.minimap.world_bounds.0));

        let viewport_rect = UIRect::new(
            cam_minimap.x - viewport_size.x / 2.0,
            cam_minimap.y - viewport_size.y / 2.0,
            viewport_size.x,
            viewport_size.y,
        );

        renderer.draw_rect_outline_with_scissor(viewport_rect, 1.0, [1.0, 1.0, 1.0, 0.8], None)?;

        // Draw unit icons
        for icon in self.minimap.unit_icons.values() {
            renderer.draw_rect_with_scissor(
                UIRect::new(
                    icon.position.x - icon.size / 2.0,
                    icon.position.y - icon.size / 2.0,
                    icon.size,
                    icon.size,
                ),
                icon.color,
                None,
            )?;
        }

        Ok(())
    }
}
