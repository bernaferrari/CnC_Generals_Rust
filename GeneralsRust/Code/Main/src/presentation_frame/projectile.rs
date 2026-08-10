use super::*;

/// Presentation-owned projectile mesh pass input (no live GameLogic).
///
/// Fail-closed: not full W3D projectile drawable / trail GPU instance parity.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileRenderInput {
    pub id: ObjectId,
    pub projectile_object_name: String,
    pub model_key: String,
    pub position: Vec3,
    pub velocity: Vec3,
    pub target_position: Vec3,
    pub is_homing: bool,
    pub mesh_scale: f32,
}

impl ProjectileRenderInput {
    pub fn from_presentation(p: &PresentationProjectile) -> Option<Self> {
        let model_key = if p.model_key.is_empty() {
            crate::assets::mesh_asset_resolve::model_key_from_projectile_object(
                &p.projectile_object_name,
            )
        } else {
            p.model_key.clone()
        };
        if model_key.is_empty() {
            return None;
        }
        Some(Self {
            id: p.id,
            projectile_object_name: p.projectile_object_name.clone(),
            model_key,
            position: p.position,
            velocity: p.velocity,
            target_position: p.target_position,
            is_homing: p.is_homing,
            mesh_scale: 1.0,
        })
    }

    /// Orient projectile mesh along velocity (fallback toward target).
    pub fn world_matrix(&self) -> glam::Mat4 {
        let dir = if self.velocity.length_squared() > 1e-6 {
            self.velocity.normalize()
        } else {
            let d = self.target_position - self.position;
            if d.length_squared() > 1e-6 {
                d.normalize()
            } else {
                glam::Vec3::Z
            }
        };
        // Y-up world: yaw from XZ, pitch from Y.
        let yaw = dir.x.atan2(dir.z);
        let pitch = -dir
            .y
            .asin()
            .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
        let scale = if self.mesh_scale.is_finite() && self.mesh_scale > 0.0 {
            self.mesh_scale
        } else {
            1.0
        };
        glam::Mat4::from_translation(self.position)
            * glam::Mat4::from_rotation_y(yaw)
            * glam::Mat4::from_rotation_x(pitch)
            * glam::Mat4::from_scale(glam::Vec3::splat(scale))
    }
}

/// Snapshot-owned in-flight projectile for presentation/client observe path.
/// Fail-closed: not full W3D projectile mesh / trail GPU parity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationProjectile {
    pub id: ObjectId,
    pub position: Vec3,
    pub velocity: Vec3,
    pub target_position: Vec3,
    pub shooter_id: ObjectId,
    pub target_id: Option<ObjectId>,
    pub damage: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub is_homing: bool,
    /// C++ ProjectileObject residual (W3D mesh key / template name).
    pub projectile_object_name: String,
    /// Resolved W3D model key residual from ProjectileObject (empty = trail-only).
    pub model_key: String,
    /// C++ Weapon.ini ProjectileExhaust residual PSys name (empty = none).
    #[serde(default)]
    pub exhaust_name: String,
}

impl PresentationProjectile {
    pub fn from_combat(p: &crate::game_logic::combat::Projectile) -> Self {
        let projectile_object_name = p.projectile_object_name.clone();
        let model_key = crate::assets::mesh_asset_resolve::model_key_from_projectile_object(
            &projectile_object_name,
        );
        Self {
            id: p.id,
            position: p.position,
            velocity: p.velocity,
            target_position: p.target_position,
            shooter_id: p.shooter_id,
            target_id: p.target_id,
            damage: p.damage,
            lifetime: p.lifetime,
            max_lifetime: p.max_lifetime,
            is_homing: p.is_homing,
            projectile_object_name,
            model_key,
            exhaust_name: p.exhaust_name.clone(),
        }
    }
}

/// Immutable feed for GameClient / renderer after each authoritative logic step.
/// C++ ProjectileStreamUpdate residual frozen for presentation trail draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationProjectileStream {
    pub shooter_id: ObjectId,
    pub stream_name: String,
    pub points: Vec<(f32, f32, f32)>,
    pub target_id: Option<ObjectId>,
}
