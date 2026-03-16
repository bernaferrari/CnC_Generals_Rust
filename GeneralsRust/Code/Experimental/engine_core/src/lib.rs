pub mod input;
pub mod world;

#[cfg(feature = "wgpu-backend")]
pub mod render_wgpu;

use input::ViewportInput;
use world::{Transform, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Editor,
    Play,
}

#[derive(Debug, Clone)]
pub struct EngineSnapshot {
    pub mode: EngineMode,
    pub simulation_seconds: f32,
    pub entity_count: usize,
    pub clear_color: [f32; 3],
    pub selected_entity: Option<u64>,
    pub camera_center_xz: [f32; 2],
    pub camera_zoom: f32,
    pub cursor_xz: [f32; 2],
}

pub struct Engine {
    mode: EngineMode,
    simulation_seconds: f32,
    world: World,
    input: ViewportInput,
    selected_entity: Option<u64>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            mode: EngineMode::Editor,
            simulation_seconds: 0.0,
            world: World::default(),
            input: ViewportInput::default(),
            selected_entity: None,
        }
    }

    pub fn mode(&self) -> EngineMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: EngineMode) {
        self.mode = mode;
    }

    pub fn simulation_seconds(&self) -> f32 {
        self.simulation_seconds
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn set_entity_transform(&mut self, id: u64, transform: Transform) -> bool {
        if let Some(entity) = self.world.entity_mut(id) {
            entity.transform = transform;
            true
        } else {
            false
        }
    }

    pub fn selected_entity(&self) -> Option<u64> {
        self.selected_entity
    }

    pub fn set_selected_entity(&mut self, entity_id: Option<u64>) {
        self.selected_entity = entity_id;
    }

    pub fn on_viewport_mouse_down(&mut self, x: f32, y: f32) {
        self.input.pointer_down(x, y);
        let world = self.input.screen_to_world_xz(x, y);
        self.selected_entity = self.world.nearest_entity_at(world[0], world[1], 1.75);
    }

    pub fn on_viewport_mouse_up(&mut self) {
        self.input.pointer_up();
    }

    pub fn on_viewport_mouse_move(&mut self, x: f32, y: f32) {
        if let Some(delta) = self.input.pointer_move(x, y) {
            if self.mode == EngineMode::Editor {
                self.input.camera_center_x -= delta[0] * 0.02;
                self.input.camera_center_z -= delta[1] * 0.02;
            }
        }
    }

    pub fn on_viewport_zoom_delta(&mut self, delta: f32) {
        self.input.camera_zoom = (self.input.camera_zoom + delta).clamp(0.2, 4.0);
    }

    pub fn update(&mut self, dt_seconds: f32) {
        let dt = dt_seconds.max(0.0);
        self.simulation_seconds += dt;
        if self.mode == EngineMode::Play {
            self.world.update_play(dt, self.simulation_seconds);
        }
    }

    pub fn snapshot(&self) -> EngineSnapshot {
        let t = self.simulation_seconds;
        let base = if self.mode == EngineMode::Play {
            [0.08, 0.14, 0.23]
        } else {
            [0.06, 0.08, 0.11]
        };
        let pulse = (t * 0.8).sin() * 0.03;
        EngineSnapshot {
            mode: self.mode,
            simulation_seconds: self.simulation_seconds,
            entity_count: self.world.entities().len(),
            clear_color: [
                (base[0] + pulse).clamp(0.0, 1.0),
                (base[1] + pulse * 0.5).clamp(0.0, 1.0),
                (base[2] + pulse * 0.25).clamp(0.0, 1.0),
            ],
            selected_entity: self.selected_entity,
            camera_center_xz: [self.input.camera_center_x, self.input.camera_center_z],
            camera_zoom: self.input.camera_zoom,
            cursor_xz: self.input.cursor_world_xz,
        }
    }
}
