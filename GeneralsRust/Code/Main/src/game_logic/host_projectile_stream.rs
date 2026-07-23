//! Host residual for C++ ProjectileStreamUpdate (Weapon.ini ProjectileStreamName).
//!
//! Tracks recent projectile positions per shooter so presentation can draw
//! stream/trail segments (flamethrower, toxin spray, machine-gun tracers).

use super::ObjectId;
use glam::Vec3;
use std::collections::HashMap;

/// C++ MAX_PROJECTILE_STREAM residual (ProjectileStreamUpdate.h).
pub const MAX_PROJECTILE_STREAM: usize = 20;

#[derive(Debug, Clone)]
pub struct ProjectileStreamState {
    pub stream_name: String,
    pub points: Vec<Vec3>,
    pub target_id: Option<ObjectId>,
    pub target_pos: Option<Vec3>,
    pub last_frame: u32,
}

impl ProjectileStreamState {
    pub fn new(stream_name: String) -> Self {
        Self {
            stream_name,
            points: Vec::new(),
            target_id: None,
            target_pos: None,
            last_frame: 0,
        }
    }

    /// C++ ProjectileStreamUpdate::addProjectile residual — ring of recent positions.
    pub fn add_point(
        &mut self,
        pos: Vec3,
        target_id: Option<ObjectId>,
        target_pos: Option<Vec3>,
        frame: u32,
    ) {
        self.points.push(pos);
        if self.points.len() > MAX_PROJECTILE_STREAM {
            let overflow = self.points.len() - MAX_PROJECTILE_STREAM;
            self.points.drain(0..overflow);
        }
        self.target_id = target_id;
        self.target_pos = target_pos;
        self.last_frame = frame;
    }
}

#[derive(Debug, Default)]
pub struct ProjectileStreamRegistry {
    /// Keyed by shooter ObjectId (one active stream per shooter residual).
    streams: HashMap<ObjectId, ProjectileStreamState>,
}

impl ProjectileStreamRegistry {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    pub fn add_projectile(
        &mut self,
        shooter: ObjectId,
        stream_name: &str,
        pos: Vec3,
        target_id: Option<ObjectId>,
        target_pos: Option<Vec3>,
        frame: u32,
    ) {
        if stream_name.is_empty() {
            return;
        }
        let entry = self
            .streams
            .entry(shooter)
            .or_insert_with(|| ProjectileStreamState::new(stream_name.to_string()));
        if entry.stream_name != stream_name {
            *entry = ProjectileStreamState::new(stream_name.to_string());
        }
        entry.add_point(pos, target_id, target_pos, frame);
    }

    /// Drop streams idle for more than `max_idle_frames`.
    pub fn cull_idle(&mut self, frame: u32, max_idle_frames: u32) {
        self.streams
            .retain(|_, s| frame.saturating_sub(s.last_frame) <= max_idle_frames);
    }

    pub fn snapshot(&self) -> Vec<(ObjectId, &ProjectileStreamState)> {
        self.streams.iter().map(|(k, v)| (*k, v)).collect()
    }

    pub fn clear(&mut self) {
        self.streams.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_rings_points_and_culls() {
        let mut reg = ProjectileStreamRegistry::new();
        let shooter = ObjectId(1);
        for i in 0..25 {
            reg.add_projectile(
                shooter,
                "DragonTankFlameStream",
                Vec3::new(i as f32, 0.0, 0.0),
                None,
                None,
                i,
            );
        }
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].1.points.len(), MAX_PROJECTILE_STREAM);
        assert!((snap[0].1.points[0].x - 5.0).abs() < 1e-3); // 25-20=5 first kept
        reg.cull_idle(100, 10);
        assert!(reg.snapshot().is_empty());
    }
}
