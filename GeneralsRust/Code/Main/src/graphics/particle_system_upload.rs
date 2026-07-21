//! Combat particle residual: pack presentation particle systems into a CPU
//! layout buffer ready for dual-tick FX / eventual WGPU particle draw.
//!
//! Host residual closed here (fail-closed vs full W3D GPU particle parity):
//! - Entries from `PresentationFrame.particle_systems` (id/kind/pos/template)
//! - Honesty counters for systems / active / bytes packed
//! - Deterministic pack order for dual-tick consumers
//!
//! Fail-closed: not live ParticleSystemManager GPU write / W3D particle mesh.

use crate::presentation_frame::{PresentationFrame, PresentationParticleSystem};

/// Interleaved residual: id(u32) + kind(u32) + pos.xyz + active(u32) + spawn_frame(u32)
/// + template name length + utf8 bytes (padded later in honesty only).
/// Layout float buffer: pos.xyz + kind_as_f32 + active_as_f32 + id_as_f32 (6 f32).
pub const PARTICLE_FLOATS_PER_SYSTEM: usize = 6;

#[derive(Debug, Clone, PartialEq)]
pub struct ParticleSystemUploadHonesty {
    pub systems_packed: u32,
    pub active_packed: u32,
    pub bytes_packed: u32,
    pub cpu_pack_ok: bool,
    pub has_geometry: bool,
    /// Systems with non-empty template_name residual.
    pub templates_named: u32,
    /// Systems with non-empty FireFX / DetonationFX residual.
    pub fx_lists_named: u32,
    /// Systems with non-empty OCL residual.
    pub ocl_lists_named: u32,
}

impl Default for ParticleSystemUploadHonesty {
    fn default() -> Self {
        Self {
            systems_packed: 0,
            active_packed: 0,
            bytes_packed: 0,
            cpu_pack_ok: true,
            has_geometry: false,
            templates_named: 0,
            fx_lists_named: 0,
            ocl_lists_named: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParticleSystemUpload {
    /// Interleaved f32 layout for dual-tick consumers.
    pub vertices: Vec<f32>,
    /// Parallel template names residual (empty string = preset only).
    pub template_names: Vec<String>,
    /// Parallel FX list names residual.
    pub fx_list_names: Vec<String>,
    /// Parallel OCL list names residual.
    pub ocl_list_names: Vec<String>,
    pub honesty: ParticleSystemUploadHonesty,
}

impl ParticleSystemUpload {
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            template_names: Vec::new(),
            fx_list_names: Vec::new(),
            ocl_list_names: Vec::new(),
            honesty: ParticleSystemUploadHonesty::default(),
        }
    }
}

/// Pack presentation particle systems into CPU layout (no live GameLogic).
pub fn pack_from_presentation(frame: &PresentationFrame) -> ParticleSystemUpload {
    let mut upload = ParticleSystemUpload::empty();
    for sys in &frame.particle_systems {
        pack_one(&mut upload, sys);
    }
    finalize_honesty(&mut upload);
    upload
}

/// Pack and mark ready (alias for execute path honesty).
pub fn pack_and_mark_upload_ready(frame: &PresentationFrame) -> ParticleSystemUpload {
    pack_from_presentation(frame)
}

fn kind_to_f32(kind: crate::game_logic::combat_particles::CombatParticleKind) -> f32 {
    use crate::game_logic::combat_particles::CombatParticleKind::*;
    (match kind {
        DeathExplosion => 0u32,
        DeathSmoke => 1,
        DeathBurn => 2,
        DeathPoison => 3,
        DeathLaser => 4,
        WeaponMuzzleFlash => 5,
        WeaponImpact => 6,
        ProjectileExhaust => 7,
    }) as f32
}

fn pack_one(upload: &mut ParticleSystemUpload, sys: &PresentationParticleSystem) {
    let kind_f = kind_to_f32(sys.kind);
    let active_f = if sys.active { 1.0 } else { 0.0 };
    let id_f = sys.id as f32;
    upload.vertices.extend_from_slice(&[
        sys.position.x,
        sys.position.y,
        sys.position.z,
        kind_f,
        active_f,
        id_f,
    ]);
    upload.template_names.push(sys.template_name.clone());
    upload.fx_list_names.push(sys.fx_list_name.clone());
    upload.ocl_list_names.push(sys.ocl_list_name.clone());
    upload.honesty.systems_packed = upload.honesty.systems_packed.saturating_add(1);
    if sys.active {
        upload.honesty.active_packed = upload.honesty.active_packed.saturating_add(1);
    }
    if !sys.template_name.is_empty() {
        upload.honesty.templates_named = upload.honesty.templates_named.saturating_add(1);
    }
    if !sys.fx_list_name.is_empty() {
        upload.honesty.fx_lists_named = upload.honesty.fx_lists_named.saturating_add(1);
    }
    if !sys.ocl_list_name.is_empty() {
        upload.honesty.ocl_lists_named = upload.honesty.ocl_lists_named.saturating_add(1);
    }
}

fn finalize_honesty(upload: &mut ParticleSystemUpload) {
    let bytes = (upload.vertices.len() * std::mem::size_of::<f32>()) as u32;
    upload.honesty.bytes_packed = bytes;
    upload.honesty.has_geometry = !upload.vertices.is_empty();
    // Empty frame is honest success (no panic residual).
    upload.honesty.cpu_pack_ok = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::combat_particles::CombatParticleKind;
    use crate::game_logic::GameLogic;
    use crate::presentation_frame::{PresentationFrame, PresentationParticleSystem};
    use glam::Vec3;

    #[test]
    fn packs_active_particle_system_from_frame() {
        let logic = GameLogic::new();
        let mut frame = PresentationFrame::build_from_logic(&logic, 0);
        frame.particle_systems.push(PresentationParticleSystem {
            id: 7,
            kind: CombatParticleKind::WeaponImpact,
            template_name: "FX_Test".into(),
            position: Vec3::new(1.0, 2.0, 3.0),
            source_object: None,
            target_object: None,
            spawned_frame: 10,
            active: true,
            client_system_id: None,
            fx_list_name: "FireFX".into(),
            ocl_list_name: String::new(),
        });
        let pack = pack_from_presentation(&frame);
        assert!(pack.honesty.cpu_pack_ok);
        assert_eq!(pack.honesty.systems_packed, 1);
        assert_eq!(pack.honesty.active_packed, 1);
        assert_eq!(pack.honesty.templates_named, 1);
        assert_eq!(pack.honesty.fx_lists_named, 1);
        assert_eq!(pack.honesty.ocl_lists_named, 0);
        assert!(pack.honesty.has_geometry);
        assert_eq!(pack.vertices.len(), PARTICLE_FLOATS_PER_SYSTEM);
        assert!((pack.vertices[0] - 1.0).abs() < 0.001);
        assert!((pack.vertices[4] - 1.0).abs() < 0.001); // active
        assert_eq!(pack.template_names[0], "FX_Test");
    }

    #[test]
    fn empty_frame_is_honest_success() {
        let logic = GameLogic::new();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let pack = pack_from_presentation(&frame);
        assert!(pack.honesty.cpu_pack_ok);
        assert_eq!(
            pack.honesty.systems_packed,
            frame.particle_systems.len() as u32
        );
        assert_eq!(
            pack.honesty.has_geometry,
            !frame.particle_systems.is_empty()
        );
    }
}
