//! Typed particle-system continuation stored by the versioned save archive.
//!
//! The field domain mirrors C++ `ParticleSystemManager::xfer` and the nested
//! system/particle xfers, including the parts those streams deliberately do
//! not persist. This module contains only archive data and canonicalization;
//! construction and native-load side effects remain on the particle owners.

use glam::{Mat3, Vec3};
use serde::{Deserialize, Serialize};

use super::particle_manager::{
    EmissionVelocity, EmissionVelocityType, EmissionVolume, EmissionVolumeType,
    ParticleSystemInfo,
};
use super::particle_system::ParticleInfo;

pub type ParticleSystemIdArchive = u32;
pub type ParticleRandomSeedArchive = [u32; 6];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleSystemManagerArchive {
    pub next_system_id: ParticleSystemIdArchive,
    pub random_seed: ParticleRandomSeedArchive,
    /// Saveable, non-destroyed systems in the manager's original creation order.
    pub systems: Vec<ParticleSystemArchive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleSystemArchive {
    pub template_name: String,
    pub info: ParticleSystemInfo,
    pub system_id: ParticleSystemIdArchive,
    pub random_seed: ParticleRandomSeedArchive,
    pub attached_drawable_id: u32,
    pub attached_object_id: u32,
    pub is_local_identity: bool,
    pub local_transform: Mat3,
    pub local_translation: Vec3,
    pub is_identity: bool,
    pub transform: Mat3,
    pub transform_translation: Vec3,
    pub burst_delay_left: u32,
    pub delay_left: u32,
    pub start_timestamp: u32,
    pub system_lifetime_left: u32,
    pub personality_counter: u32,
    pub is_forever: bool,
    pub accumulated_size_bonus: f32,
    pub is_stopped: bool,
    pub vel_coeff: Vec3,
    pub count_coeff: f32,
    pub delay_coeff: f32,
    pub size_coeff: f32,
    pub position: Vec3,
    pub last_position: Vec3,
    pub is_first_pos: bool,
    pub slave_system: Option<ParticleSystemIdArchive>,
    pub master_system: Option<ParticleSystemIdArchive>,
    pub particles: Vec<ParticleArchive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleArchive {
    pub info: ParticleInfo,
    pub personality: u32,
    pub acceleration: Vec3,
    pub last_position: Vec3,
    pub lifetime_left: u32,
    pub create_timestamp: u32,
    pub alpha: f32,
    pub alpha_rate: f32,
    /// Stored as the original signed wire domain after validation; valid values
    /// are zero through MAX_KEYFRAMES inclusive.
    pub alpha_target_key: u32,
    pub color: [f32; 3],
    pub color_rate: [f32; 3],
    pub color_target_key: u32,
    pub controlled_system: Option<ParticleSystemIdArchive>,
}

fn default_random() -> super::particle_manager::GameClientRandomVariable {
    super::particle_manager::GameClientRandomVariable::default()
}

fn canonical_velocity_for_save(
    velocity_type: EmissionVelocityType,
    velocity: &EmissionVelocity,
) -> EmissionVelocity {
    match velocity_type {
        // No velocity payload exists for INVALID_VELOCITY. Restore keeps the
        // receiving template's union, so the archived value is inert.
        EmissionVelocityType::Invalid => *velocity,
        EmissionVelocityType::Ortho => match *velocity {
            EmissionVelocity::Ortho { x, y, z } => EmissionVelocity::Ortho { x, y, z },
            _ => EmissionVelocity::Ortho {
                x: default_random(),
                y: default_random(),
                z: default_random(),
            },
        },
        EmissionVelocityType::Spherical => match *velocity {
            EmissionVelocity::Spherical { speed } => EmissionVelocity::Spherical { speed },
            _ => EmissionVelocity::Spherical {
                speed: default_random(),
            },
        },
        EmissionVelocityType::Hemispherical => match *velocity {
            EmissionVelocity::Hemispherical { speed } => {
                EmissionVelocity::Hemispherical { speed }
            }
            _ => EmissionVelocity::Hemispherical {
                speed: default_random(),
            },
        },
        EmissionVelocityType::Cylindrical => match *velocity {
            EmissionVelocity::Cylindrical { radial, normal } => {
                EmissionVelocity::Cylindrical { radial, normal }
            }
            _ => EmissionVelocity::Cylindrical {
                radial: default_random(),
                normal: default_random(),
            },
        },
        EmissionVelocityType::Outward => match *velocity {
            EmissionVelocity::Outward {
                speed,
                other_speed,
            } => EmissionVelocity::Outward {
                speed,
                other_speed,
            },
            _ => EmissionVelocity::Outward {
                speed: default_random(),
                other_speed: default_random(),
            },
        },
    }
}

fn canonical_volume_for_save(
    volume_type: EmissionVolumeType,
    volume: &EmissionVolume,
) -> EmissionVolume {
    match volume_type {
        // Both no-payload cases load as Point.
        EmissionVolumeType::Invalid | EmissionVolumeType::Point => EmissionVolume::Point,
        EmissionVolumeType::Line => match *volume {
            EmissionVolume::Line { start, end } => EmissionVolume::Line { start, end },
            _ => EmissionVolume::Line {
                start: Vec3::ZERO,
                end: Vec3::ZERO,
            },
        },
        EmissionVolumeType::Box => match *volume {
            EmissionVolume::Box { half_size } => EmissionVolume::Box { half_size },
            _ => EmissionVolume::Box {
                half_size: Vec3::ZERO,
            },
        },
        EmissionVolumeType::Sphere => match *volume {
            EmissionVolume::Sphere { radius } => EmissionVolume::Sphere { radius },
            _ => EmissionVolume::Sphere { radius: 0.0 },
        },
        EmissionVolumeType::Cylinder => match *volume {
            EmissionVolume::Cylinder { radius, length } => {
                EmissionVolume::Cylinder { radius, length }
            }
            _ => EmissionVolume::Cylinder {
                radius: 0.0,
                length: 0.0,
            },
        },
    }
}

/// Canonicalize a live runtime info object to exactly the payload the native
/// writer would have encoded. Runtime-only fields remain marked `serde(skip)`
/// on the shared info type.
pub(crate) fn canonical_info_for_archive(info: &ParticleSystemInfo) -> ParticleSystemInfo {
    let mut info = info.clone();
    info.emission_velocity =
        canonical_velocity_for_save(info.emission_velocity_type, &info.emission_velocity);
    info.emission_volume =
        canonical_volume_for_save(info.emission_volume_type, &info.emission_volume);
    info
}

/// Reject union states that no canonical writer can produce. Invalid/no-payload
/// combinations are not rejected here because restore explicitly reproduces
/// the original template/point fallback behavior.
pub(crate) fn validate_archive_info(info: &ParticleSystemInfo) -> Result<(), String> {
    let velocity_matches = matches!(
        (info.emission_velocity_type, &info.emission_velocity),
        (EmissionVelocityType::Invalid, _)
            | (EmissionVelocityType::Ortho, EmissionVelocity::Ortho { .. })
            | (
                EmissionVelocityType::Spherical,
                EmissionVelocity::Spherical { .. }
            )
            | (
                EmissionVelocityType::Hemispherical,
                EmissionVelocity::Hemispherical { .. }
            )
            | (
                EmissionVelocityType::Cylindrical,
                EmissionVelocity::Cylindrical { .. }
            )
            | (
                EmissionVelocityType::Outward,
                EmissionVelocity::Outward { .. }
            )
    );
    if !velocity_matches {
        return Err("particle emission velocity payload does not match its type".into());
    }
    let volume_matches = matches!(
        (info.emission_volume_type, &info.emission_volume),
        (EmissionVolumeType::Invalid | EmissionVolumeType::Point, _)
            | (EmissionVolumeType::Line, EmissionVolume::Line { .. })
            | (EmissionVolumeType::Box, EmissionVolume::Box { .. })
            | (EmissionVolumeType::Sphere, EmissionVolume::Sphere { .. })
            | (
                EmissionVolumeType::Cylinder,
                EmissionVolume::Cylinder { .. }
            )
    );
    if !volume_matches {
        return Err("particle emission volume payload does not match its type".into());
    }
    Ok(())
}
