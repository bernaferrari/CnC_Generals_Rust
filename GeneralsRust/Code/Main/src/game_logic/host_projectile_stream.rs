//! Host residual for C++ ProjectileStreamUpdate (Weapon.ini ProjectileStreamName).
//!
//! Tracks recent projectile positions per shooter so presentation can draw
//! stream/trail segments (flamethrower, toxin spray, machine-gun tracers).

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// C++ MAX_PROJECTILE_STREAM residual (ProjectileStreamUpdate.h).
pub const MAX_PROJECTILE_STREAM: usize = 20;

/// C++ `INVALID_ID` hole written by `getAllPoints` as (0,0,0).
pub const STREAM_HOLE: Vec3 = Vec3::ZERO;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// C++ ProjectileStreamUpdate::addProjectile residual — ring of recent
    /// positions. A retarget (object or position) inserts a (0,0,0) hole so
    /// presentation can break the ribbon.
    pub fn add_point(
        &mut self,
        pos: Vec3,
        target_id: Option<ObjectId>,
        target_pos: Option<Vec3>,
        frame: u32,
    ) {
        if let Some(vid) = target_id {
            if self.target_id != Some(vid) {
                self.push_ring(STREAM_HOLE);
                self.target_id = Some(vid);
            }
            self.target_pos = None;
        } else if let Some(pos_tgt) = target_pos {
            if self.target_pos != Some(pos_tgt) {
                self.push_ring(STREAM_HOLE);
                self.target_pos = Some(pos_tgt);
            }
            self.target_id = None;
        }
        self.push_ring(pos);
        self.last_frame = frame;
    }

    fn push_ring(&mut self, pos: Vec3) {
        self.points.push(pos);
        if self.points.len() > MAX_PROJECTILE_STREAM {
            let overflow = self.points.len() - MAX_PROJECTILE_STREAM;
            self.points.drain(0..overflow);
        }
    }

    /// C++ `ProjectileStreamUpdate::getAllPoints` vehicle-roof skim.
    ///
    /// Host Y-up: C++ raises Z to `maxHeightAbovePosition + pos.z + 0.5` when
    /// the point is within 1.5× major radius of a KINDOF_VEHICLE owner.
    pub fn skim_vehicle_roof(
        &self,
        owner_pos: Vec3,
        max_height_above: f32,
        major_radius: f32,
    ) -> Vec<Vec3> {
        skim_stream_points_for_vehicle(&self.points, owner_pos, max_height_above, major_radius)
    }
}

/// True when a stream point is the C++ INVALID_ID hole.
#[inline]
pub fn is_stream_hole(p: Vec3) -> bool {
    p == STREAM_HOLE
}

/// C++ getAllPoints vehicle-roof raise (host Y-up).
pub fn skim_stream_points_for_vehicle(
    points: &[Vec3],
    owner_pos: Vec3,
    max_height_above: f32,
    major_radius: f32,
) -> Vec<Vec3> {
    let my_top = max_height_above + owner_pos.y + 0.5;
    let limit = major_radius.max(0.0) * 1.5;
    let limit_sq = limit * limit;
    points
        .iter()
        .copied()
        .map(|p| {
            if is_stream_hole(p) {
                return p;
            }
            let dx = owner_pos.x - p.x;
            let dz = owner_pos.z - p.z;
            if dx * dx + dz * dz <= limit_sq {
                Vec3::new(p.x, p.y.max(my_top), p.z)
            } else {
                p
            }
        })
        .collect()
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

    /// C++ `getAllPoints` vehicle-roof skim applied to a live stream.
    pub fn apply_vehicle_roof_skim(
        &mut self,
        shooter: ObjectId,
        owner_pos: Vec3,
        major_radius: f32,
        max_height_above: f32,
    ) {
        if let Some(stream) = self.streams.get_mut(&shooter) {
            stream.points = skim_stream_points_for_vehicle(
                &stream.points,
                owner_pos,
                max_height_above,
                major_radius,
            );
        }
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

    /// Replace live streams from a save payload. Does not call `add_projectile`
    /// so a load cannot insert a retarget hole or re-create the trail.
    pub fn restore(&mut self, streams: HashMap<ObjectId, ProjectileStreamState>) {
        self.streams = streams;
    }

    pub fn is_empty(&self) -> bool {
        self.streams.is_empty()
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

    #[test]
    fn retarget_inserts_hole_so_ribbon_breaks() {
        let mut state = ProjectileStreamState::new("DragonTankFlameStream".into());
        state.add_point(Vec3::new(1.0, 2.0, 3.0), Some(ObjectId(10)), None, 1);
        state.add_point(Vec3::new(2.0, 2.0, 3.0), Some(ObjectId(10)), None, 2);
        state.add_point(Vec3::new(8.0, 2.0, 3.0), Some(ObjectId(11)), None, 3);
        assert!(is_stream_hole(state.points[0]));
        assert_eq!(state.points[1], Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(state.points[2], Vec3::new(2.0, 2.0, 3.0));
        assert!(is_stream_hole(state.points[3]));
        assert_eq!(state.points[4], Vec3::new(8.0, 2.0, 3.0));
    }

    #[test]
    fn position_retarget_inserts_hole() {
        let mut state = ProjectileStreamState::new("ToxinStream".into());
        let a = Vec3::new(4.0, 0.0, 0.0);
        let b = Vec3::new(9.0, 0.0, 0.0);
        state.add_point(Vec3::new(1.0, 0.0, 0.0), None, Some(a), 1);
        state.add_point(Vec3::new(2.0, 0.0, 0.0), None, Some(a), 2);
        state.add_point(Vec3::new(3.0, 0.0, 0.0), None, Some(b), 3);
        assert!(state.points.iter().any(|p| is_stream_hole(*p)));
        assert_eq!(state.points.last().copied(), Some(Vec3::new(3.0, 0.0, 0.0)));
    }

    #[test]
    fn vehicle_roof_skim_raises_near_hull() {
        let owner = Vec3::new(0.0, 0.0, 0.0);
        let near = Vec3::new(1.0, 0.0, 0.0);
        let far = Vec3::new(50.0, 0.0, 0.0);
        let skimmed = skim_stream_points_for_vehicle(&[near, far, STREAM_HOLE], owner, 10.0, 5.0);
        assert!((skimmed[0].y - 10.5).abs() < 1e-3);
        assert!((skimmed[1].y - 0.0).abs() < 1e-3);
        assert!(is_stream_hole(skimmed[2]));
    }
}
