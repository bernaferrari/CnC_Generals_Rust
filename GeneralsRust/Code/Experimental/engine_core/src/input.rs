#[derive(Debug, Clone)]
pub struct ViewportInput {
    pub camera_center_x: f32,
    pub camera_center_z: f32,
    pub camera_zoom: f32,
    pub dragging: bool,
    last_pointer_xy: Option<[f32; 2]>,
    pub cursor_world_xz: [f32; 2],
}

impl Default for ViewportInput {
    fn default() -> Self {
        Self {
            camera_center_x: 0.0,
            camera_center_z: 0.0,
            camera_zoom: 1.0,
            dragging: false,
            last_pointer_xy: None,
            cursor_world_xz: [0.0, 0.0],
        }
    }
}

impl ViewportInput {
    pub fn screen_to_world_xz(&self, x: f32, y: f32) -> [f32; 2] {
        let world_x = self.camera_center_x + ((x - 390.0) / 48.0) / self.camera_zoom;
        let world_z = self.camera_center_z + ((y - 390.0) / 48.0) / self.camera_zoom;
        [world_x, world_z]
    }

    pub fn pointer_down(&mut self, x: f32, y: f32) {
        self.dragging = true;
        self.last_pointer_xy = Some([x, y]);
        self.cursor_world_xz = self.screen_to_world_xz(x, y);
    }

    pub fn pointer_up(&mut self) {
        self.dragging = false;
        self.last_pointer_xy = None;
    }

    pub fn pointer_move(&mut self, x: f32, y: f32) -> Option<[f32; 2]> {
        let world = self.screen_to_world_xz(x, y);
        self.cursor_world_xz = world;
        let last = self.last_pointer_xy.replace([x, y])?;
        if self.dragging {
            Some([x - last[0], y - last[1]])
        } else {
            None
        }
    }
}
