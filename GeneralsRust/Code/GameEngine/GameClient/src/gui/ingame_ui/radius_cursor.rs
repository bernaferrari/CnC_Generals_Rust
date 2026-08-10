// Radius cursor targeting overlay.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

impl InGameUI {
    pub fn set_radius_cursor(
        &mut self,
        cursor_type: RadiusCursorType,
        position: Coord3D,
        radius: f32,
    ) {
        if cursor_type == self.radius_cursor.cursor_type && self.radius_cursor.active {
            return;
        }
        if cursor_type == RadiusCursorType::None {
            self.clear_radius_cursor();
            return;
        }
        if radius <= 0.0 {
            return;
        }
        self.radius_cursor.cursor_type = cursor_type;
        self.radius_cursor.active = true;
        self.radius_cursor.position = position;
        self.radius_cursor.radius = radius;
    }

    pub fn clear_radius_cursor(&mut self) {
        self.radius_cursor.cursor_type = RadiusCursorType::None;
        self.radius_cursor.active = false;
        self.radius_cursor.radius = 0.0;
    }

    pub fn is_radius_cursor_active(&self) -> bool {
        self.radius_cursor.active
    }

    pub fn get_radius_cursor_type(&self) -> RadiusCursorType {
        self.radius_cursor.cursor_type
    }

    pub fn update_radius_cursor(&mut self, mouse_pos: Coord3D) {
        if !self.radius_cursor.active {
            return;
        }
        self.radius_cursor.position = mouse_pos;
    }

}
