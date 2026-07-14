//! Movement order residual: pack presentation move destinations / path waypoints
//! into a CPU line buffer for dual-tick UI / eventual WGPU path ribbon draw.
//!
//! Host residual closed here (fail-closed vs full retail path-line GPU):
//! - Unit position → move_destination segments
//! - Optional waypoint polyline (capped)
//! - Honesty counters for lines / vertices / bytes
//!
//! Fail-closed: not live InGameUI path ribbon / terrain-clamped draw.

use crate::presentation_frame::{PresentationFrame, RenderableObject};
use glam::Vec3;

pub const MOVE_LINE_FLOATS_PER_VERTEX: usize = 7;
pub const MOVE_LINE_VERTICES_PER_SEGMENT: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct MoveLineUploadHonesty {
    pub lines_packed: u32,
    pub waypoints_packed: u32,
    pub vertices_packed: u32,
    pub bytes_packed: u32,
    pub cpu_pack_ok: bool,
    pub has_geometry: bool,
}

impl Default for MoveLineUploadHonesty {
    fn default() -> Self {
        Self {
            lines_packed: 0,
            waypoints_packed: 0,
            vertices_packed: 0,
            bytes_packed: 0,
            cpu_pack_ok: true,
            has_geometry: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveLineUpload {
    pub vertices: Vec<f32>,
    pub honesty: MoveLineUploadHonesty,
}

impl MoveLineUpload {
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            honesty: MoveLineUploadHonesty::default(),
        }
    }

    pub fn pack_from_presentation(frame: &PresentationFrame) -> Self {
        Self::pack_objects(&frame.objects)
    }

    pub fn pack_objects(objects: &[RenderableObject]) -> Self {
        let mut vertices = Vec::new();
        let mut lines = 0u32;
        let mut waypoints = 0u32;
        // Cyan order-line residual.
        let color = [0.2f32, 0.9, 1.0, 0.85];
        for o in objects {
            if o.destroyed {
                continue;
            }
            if let Some(dest) = o.move_destination {
                push_vertex(&mut vertices, o.position, color);
                push_vertex(&mut vertices, dest, color);
                lines += 1;
            }
            if o.path_waypoints.len() >= 2 {
                for w in o.path_waypoints.windows(2) {
                    push_vertex(&mut vertices, w[0], color);
                    push_vertex(&mut vertices, w[1], color);
                    waypoints += 1;
                }
            }
        }
        if vertices.is_empty() {
            return Self::empty();
        }
        let vertices_packed = (vertices.len() / MOVE_LINE_FLOATS_PER_VERTEX) as u32;
        let bytes_packed = (vertices.len() * std::mem::size_of::<f32>()) as u32;
        Self {
            vertices,
            honesty: MoveLineUploadHonesty {
                lines_packed: lines,
                waypoints_packed: waypoints,
                vertices_packed,
                bytes_packed,
                cpu_pack_ok: true,
                has_geometry: true,
            },
        }
    }

    pub fn is_upload_ready(&self) -> bool {
        self.honesty.cpu_pack_ok && (!self.honesty.has_geometry || !self.vertices.is_empty())
    }
}

fn push_vertex(out: &mut Vec<f32>, pos: Vec3, color: [f32; 4]) {
    out.extend_from_slice(&[pos.x, pos.y, pos.z, color[0], color[1], color[2], color[3]]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;
    use glam::Vec3;

    #[test]
    fn empty_pack_is_honest() {
        let logic = GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let pack = MoveLineUpload::pack_from_presentation(&frame);
        assert!(pack.honesty.cpu_pack_ok);
        assert!(!pack.honesty.has_geometry);
        assert!(pack.is_upload_ready());
    }

    #[test]
    fn packs_move_destination_line() {
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("MoveLineU");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("MoveLineU".into(), t);
        let id = logic
            .create_object("MoveLineU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("u");
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            obj.movement.target_position = Some(Vec3::new(30.0, 0.0, 10.0));
            obj.movement.path = vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(15.0, 0.0, 5.0),
                Vec3::new(30.0, 0.0, 10.0),
            ];
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
        assert_eq!(ro.move_destination, Some(Vec3::new(30.0, 0.0, 10.0)));
        assert_eq!(ro.path_waypoints.len(), 3);
        let pack = MoveLineUpload::pack_from_presentation(&frame);
        assert!(pack.honesty.has_geometry);
        assert!(pack.honesty.lines_packed >= 1);
        assert!(pack.honesty.waypoints_packed >= 2);
        assert!(pack.is_upload_ready());
    }
}
