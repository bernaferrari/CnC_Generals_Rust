//! Combat projectile residual: pack presentation projectiles into a CPU line
//! buffer ready for dual-tick UI / eventual WGPU trail draw.
//!
//! Host residual closed here (fail-closed vs full W3D projectile mesh/trail):
//! - Interleaved vertices from `PresentationFrame.projectiles` (pos → target)
//! - Honesty counters for projectiles / vertices / bytes
//! - Deterministic pack order for dual-tick consumers
//!
//! Fail-closed: not live SegLineRenderer GPU write / mesh instance draw.

use crate::presentation_frame::{PresentationFrame, PresentationProjectile};

/// Interleaved vertex: pos.xyz + color.rgba (7 f32).
pub const PROJECTILE_FLOATS_PER_VERTEX: usize = 7;
pub const PROJECTILE_VERTICES_PER_SEGMENT: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileSegmentUploadHonesty {
    pub projectiles_packed: u32,
    pub vertices_packed: u32,
    pub bytes_packed: u32,
    pub cpu_pack_ok: bool,
    pub has_geometry: bool,
    /// Count of projectiles with non-empty ProjectileObject residual.
    pub projectile_objects_named: u32,
    /// Count of projectiles with resolved W3D model_key residual.
    pub mesh_keys_nonempty: u32,
}

impl Default for ProjectileSegmentUploadHonesty {
    fn default() -> Self {
        Self {
            projectiles_packed: 0,
            vertices_packed: 0,
            bytes_packed: 0,
            cpu_pack_ok: true,
            has_geometry: false,
            projectile_objects_named: 0,
            mesh_keys_nonempty: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileSegmentUpload {
    /// Interleaved f32 buffer (pos3 + color4 per vertex).
    pub vertices: Vec<f32>,
    /// Parallel mesh keys residual (empty string = trail-only / hitscan).
    pub mesh_keys: Vec<String>,
    /// Parallel ProjectileObject names residual.
    pub projectile_object_names: Vec<String>,
    pub honesty: ProjectileSegmentUploadHonesty,
}

impl ProjectileSegmentUpload {
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            mesh_keys: Vec::new(),
            projectile_object_names: Vec::new(),
            honesty: ProjectileSegmentUploadHonesty::default(),
        }
    }

    pub fn pack_from_presentation(frame: &PresentationFrame) -> Self {
        Self::pack_projectiles(&frame.projectiles)
    }

    pub fn pack_projectiles(projectiles: &[PresentationProjectile]) -> Self {
        if projectiles.is_empty() {
            return Self::empty();
        }
        let mut vertices = Vec::with_capacity(
            projectiles.len() * PROJECTILE_VERTICES_PER_SEGMENT * PROJECTILE_FLOATS_PER_VERTEX,
        );
        let mut mesh_keys = Vec::with_capacity(projectiles.len());
        let mut projectile_object_names = Vec::with_capacity(projectiles.len());
        let mut named = 0u32;
        let mut keyed = 0u32;
        // Warm yellow trail residual (not retail material).
        let color = [1.0f32, 0.85, 0.2, 0.9];
        for p in projectiles {
            push_vertex(&mut vertices, p.position, color);
            push_vertex(&mut vertices, p.target_position, color);
            let obj_name = p.projectile_object_name.clone();
            if !obj_name.is_empty() {
                named = named.saturating_add(1);
            }
            let key = if p.model_key.is_empty() {
                crate::assets::mesh_asset_resolve::model_key_from_projectile_object(&obj_name)
            } else {
                p.model_key.clone()
            };
            if !key.is_empty() {
                keyed = keyed.saturating_add(1);
            }
            projectile_object_names.push(obj_name);
            mesh_keys.push(key);
        }
        let vertices_packed = (projectiles.len() * PROJECTILE_VERTICES_PER_SEGMENT) as u32;
        let bytes_packed = (vertices.len() * std::mem::size_of::<f32>()) as u32;
        Self {
            vertices,
            mesh_keys,
            projectile_object_names,
            honesty: ProjectileSegmentUploadHonesty {
                projectiles_packed: projectiles.len() as u32,
                vertices_packed,
                bytes_packed,
                cpu_pack_ok: true,
                has_geometry: true,
                projectile_objects_named: named,
                mesh_keys_nonempty: keyed,
            },
        }
    }

    pub fn is_upload_ready(&self) -> bool {
        self.honesty.cpu_pack_ok && (!self.honesty.has_geometry || !self.vertices.is_empty())
    }
}

fn push_vertex(out: &mut Vec<f32>, pos: glam::Vec3, color: [f32; 4]) {
    out.extend_from_slice(&[pos.x, pos.y, pos.z, color[0], color[1], color[2], color[3]]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, ObjectId, Weapon};
    use crate::presentation_frame::PresentationFrame;

    #[test]
    fn empty_pack_is_honest() {
        let logic = GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let pack = ProjectileSegmentUpload::pack_from_presentation(&frame);
        assert!(pack.honesty.cpu_pack_ok);
        assert!(!pack.honesty.has_geometry);
        assert!(pack.vertices.is_empty());
        assert!(pack.is_upload_ready());
    }

    #[test]
    fn packs_presentation_projectiles() {
        let mut logic = GameLogic::new();
        let weapon = Weapon::default();
        let _ = logic.combat_system_mut().fire_projectile(
            glam::Vec3::new(0.0, 1.0, 0.0),
            glam::Vec3::new(50.0, 1.0, 0.0),
            &weapon,
            ObjectId(1),
            Some(ObjectId(2)),
            100.0,
        );
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(!frame.projectiles.is_empty());
        let pack = ProjectileSegmentUpload::pack_from_presentation(&frame);
        assert!(pack.honesty.has_geometry);
        assert_eq!(
            pack.honesty.projectiles_packed,
            frame.projectiles.len() as u32
        );
        assert_eq!(
            pack.honesty.vertices_packed,
            (frame.projectiles.len() * PROJECTILE_VERTICES_PER_SEGMENT) as u32
        );
        assert_eq!(
            pack.vertices.len(),
            frame.projectiles.len()
                * PROJECTILE_VERTICES_PER_SEGMENT
                * PROJECTILE_FLOATS_PER_VERTEX
        );
        assert!(pack.is_upload_ready());
    }

    #[test]
    fn packs_projectile_mesh_keys_from_object_name() {
        let projectiles = vec![PresentationProjectile {
            id: ObjectId(1),
            position: glam::Vec3::ZERO,
            velocity: glam::Vec3::X,
            target_position: glam::Vec3::new(10.0, 0.0, 0.0),
            shooter_id: ObjectId(2),
            target_id: None,
            damage: 10.0,
            lifetime: 0.0,
            max_lifetime: 1.0,
            is_homing: false,
            projectile_object_name: "GenericTankShell".into(),
            model_key: String::new(),
            exhaust_name: String::new(),
        }];
        let pack = ProjectileSegmentUpload::pack_projectiles(&projectiles);
        assert_eq!(pack.honesty.projectiles_packed, 1);
        assert_eq!(pack.honesty.projectile_objects_named, 1);
        assert_eq!(pack.honesty.mesh_keys_nonempty, 1);
        assert_eq!(pack.mesh_keys[0].to_ascii_lowercase(), "pmgntankshell");
        assert_eq!(pack.projectile_object_names[0], "GenericTankShell");
    }
}
