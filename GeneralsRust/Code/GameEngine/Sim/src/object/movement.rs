//! Actor-owned movement and optimized path cursor.
use crate::pathfinding::{PathCursor, PathRoute};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Movement and pathfinding state
#[derive(Debug, Clone, Serialize)]
pub struct Movement {
    pub target_position: Option<Vec3>,
    pub velocity: Vec3,
    pub max_speed: f32,
    /// C++ LocomotorTemplate::m_maxSpeedDamaged residual.
    pub max_speed_damaged: f32,
    pub acceleration: f32,
    /// C++ LocomotorTemplate::m_accelerationDamaged residual.
    pub acceleration_damaged: f32,
    pub turn_rate: f32,
    /// C++ LocomotorTemplate::m_maxTurnRateDamaged residual.
    pub turn_rate_damaged: f32,
    pub path: PathRoute,
    pub path_cursor: PathCursor,
}

impl<'de> Deserialize<'de> for Movement {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct StoredCursor {
            previous: Option<usize>,
            current: usize,
        }
        #[derive(Deserialize)]
        struct StoredMovement {
            target_position: Option<Vec3>,
            velocity: Vec3,
            max_speed: f32,
            max_speed_damaged: f32,
            acceleration: f32,
            acceleration_damaged: f32,
            turn_rate: f32,
            turn_rate_damaged: f32,
            path: PathRoute,
            path_cursor: StoredCursor,
        }
        let stored = StoredMovement::deserialize(deserializer)?;
        let current =
            (stored.path_cursor.current != usize::MAX).then_some(stored.path_cursor.current);
        let path_cursor = stored
            .path
            .cursor_from_raw(stored.path_cursor.previous, current)
            .ok_or_else(|| serde::de::Error::custom("invalid path cursor"))?;
        Ok(Self {
            target_position: stored.target_position,
            velocity: stored.velocity,
            max_speed: stored.max_speed,
            max_speed_damaged: stored.max_speed_damaged,
            acceleration: stored.acceleration,
            acceleration_damaged: stored.acceleration_damaged,
            turn_rate: stored.turn_rate,
            turn_rate_damaged: stored.turn_rate_damaged,
            path: stored.path,
            path_cursor,
        })
    }
}

impl Movement {
    /// Replace the route and its original PathNode layers together.
    pub fn set_path_with_layers(&mut self, path: Vec<Vec3>, layers: Vec<u8>) {
        self.path = if path.len() == layers.len() {
            PathRoute::from_layered(path, layers)
        } else {
            PathRoute::from_coords(path)
        };
        self.path_cursor = self.path.cursor_start();
    }

    /// Install a coordinate-only path; forget prior node layers.
    pub fn set_path_coords(&mut self, path: Vec<Vec3>) {
        self.path = PathRoute::from_coords(path);
        self.path_cursor = self.path.cursor_start();
    }

    pub fn clear_path(&mut self) {
        self.path.clear();
        self.path_cursor = self.path.cursor_start();
    }

    pub fn current_path_position(&self) -> Option<Vec3> {
        self.path_cursor
            .current()
            .and_then(|i| self.path.node(i))
            .map(|n| n.position)
    }

    pub fn has_next_path_node(&self) -> bool {
        self.path_cursor
            .current()
            .and_then(|i| self.path.node(i))
            .and_then(|n| n.next_optimized)
            .is_some()
    }

    /// Layer of the final optimized PathNode, when known.
    pub fn last_path_layer(&self) -> Option<u8> {
        self.path
            .len()
            .checked_sub(1)
            .and_then(|i| self.path.optimized_layer(i))
    }
}

impl Default for Movement {
    fn default() -> Self {
        Self {
            target_position: None,
            velocity: Vec3::ZERO,
            max_speed: 10.0,
            // Retail default often ~ half pristine when damaged; host uses 50%.
            max_speed_damaged: 5.0,
            acceleration: 5.0,
            acceleration_damaged: 2.5,
            turn_rate: std::f32::consts::PI,
            turn_rate_damaged: std::f32::consts::PI * 0.5,
            path: Default::default(),
            path_cursor: Default::default(),
        }
    }
}
