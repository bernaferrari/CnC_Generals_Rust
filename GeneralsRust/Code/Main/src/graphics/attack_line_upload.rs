//! Attack-order residual: pack presentation attack targets into a CPU line
//! buffer (attacker position → target position) for dual-tick UI / eventual
//! WGPU order-line draw.
//!
//! Host residual closed here (fail-closed vs full retail attack-line GPU):
//! - Resolve target id → position from the same PresentationFrame objects
//! - Honesty counters for lines / vertices / bytes
//!
//! Fail-closed: not live InGameUI attack cursor / terrain-clamped ribbon.

use crate::presentation_frame::{PresentationFrame, RenderableObject};
use glam::Vec3;
use std::collections::HashMap;

pub const ATTACK_LINE_FLOATS_PER_VERTEX: usize = 7;
pub const ATTACK_LINE_VERTICES_PER_SEGMENT: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct AttackLineUploadHonesty {
    pub lines_packed: u32,
    pub unresolved_targets: u32,
    pub vertices_packed: u32,
    pub bytes_packed: u32,
    pub cpu_pack_ok: bool,
    pub has_geometry: bool,
}

impl Default for AttackLineUploadHonesty {
    fn default() -> Self {
        Self {
            lines_packed: 0,
            unresolved_targets: 0,
            vertices_packed: 0,
            bytes_packed: 0,
            cpu_pack_ok: true,
            has_geometry: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttackLineUpload {
    pub vertices: Vec<f32>,
    pub honesty: AttackLineUploadHonesty,
}

impl AttackLineUpload {
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            honesty: AttackLineUploadHonesty::default(),
        }
    }

    pub fn pack_from_presentation(frame: &PresentationFrame) -> Self {
        Self::pack_objects(&frame.objects)
    }

    pub fn pack_objects(objects: &[RenderableObject]) -> Self {
        let mut by_id: HashMap<u32, Vec3> = HashMap::new();
        for o in objects {
            if !o.destroyed {
                by_id.insert(o.id.0, o.position);
            }
        }
        let mut vertices = Vec::new();
        let mut lines = 0u32;
        let mut unresolved = 0u32;
        // Red attack-order residual.
        let color = [1.0f32, 0.25, 0.15, 0.9];
        for o in objects {
            if o.destroyed {
                continue;
            }
            let Some(tid) = o.attack_target else {
                continue;
            };
            let Some(&tpos) = by_id.get(&tid.0) else {
                unresolved += 1;
                continue;
            };
            push_vertex(&mut vertices, o.position, color);
            push_vertex(&mut vertices, tpos, color);
            lines += 1;
        }
        if vertices.is_empty() {
            let mut empty = Self::empty();
            empty.honesty.unresolved_targets = unresolved;
            return empty;
        }
        let vertices_packed = (vertices.len() / ATTACK_LINE_FLOATS_PER_VERTEX) as u32;
        let bytes_packed = (vertices.len() * std::mem::size_of::<f32>()) as u32;
        Self {
            vertices,
            honesty: AttackLineUploadHonesty {
                lines_packed: lines,
                unresolved_targets: unresolved,
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
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;
    use glam::Vec3;

    #[test]
    fn empty_pack_is_honest() {
        let logic = GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let pack = AttackLineUpload::pack_from_presentation(&frame);
        assert!(pack.honesty.cpu_pack_ok);
        assert!(!pack.honesty.has_geometry);
        assert!(pack.is_upload_ready());
    }

    #[test]
    fn packs_attack_line_between_units() {
        let mut logic = GameLogic::new();
        for (name, pos) in [("AtkA", Vec3::ZERO), ("AtkB", Vec3::new(20.0, 0.0, 0.0))] {
            let mut t = ThingTemplate::new(name);
            t.set_health(40.0);
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert(name.into(), t);
            let _ = logic.create_object(name, Team::USA, pos).expect("spawn");
        }
        let ids: Vec<_> = logic.get_objects().keys().copied().collect();
        assert!(ids.len() >= 2);
        let (a, b) = (ids[0], ids[1]);
        if let Some(obj) = logic.get_objects_mut().get_mut(&a) {
            obj.target = Some(b);
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let ro = frame.objects.iter().find(|o| o.id == a).expect("a");
        assert_eq!(ro.attack_target, Some(b));
        let pack = AttackLineUpload::pack_from_presentation(&frame);
        assert!(pack.honesty.has_geometry);
        assert_eq!(pack.honesty.lines_packed, 1);
        assert_eq!(pack.honesty.unresolved_targets, 0);
        assert!(pack.is_upload_ready());
        assert_eq!(
            pack.vertices.len(),
            ATTACK_LINE_VERTICES_PER_SEGMENT * ATTACK_LINE_FLOATS_PER_VERTEX
        );
    }
}
