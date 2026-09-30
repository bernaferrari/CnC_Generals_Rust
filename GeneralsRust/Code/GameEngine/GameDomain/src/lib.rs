//! Small, renderer-independent game-domain values shared across engine layers.

use serde::{Deserialize, Serialize};

pub mod difficulty;
pub use difficulty::AIDifficulty;

/// Unique identifier for Main host game objects.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub struct ObjectId(pub u32);

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Faction identity used by Main host gameplay and frozen presentation values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Team {
    GLA,
    USA,
    China,
    Neutral,
}

impl Team {
    pub fn from_player_id(player_id: u32) -> Self {
        match player_id {
            0 => Self::USA,
            1 => Self::China,
            2 => Self::GLA,
            _ => Self::Neutral,
        }
    }

    pub fn get_color(&self) -> [f32; 4] {
        match self {
            Self::USA => [0.2, 0.4, 0.8, 1.0],
            Self::China => [0.8, 0.2, 0.2, 1.0],
            Self::GLA => [0.8, 0.6, 0.2, 1.0],
            Self::Neutral => [0.5, 0.5, 0.5, 1.0],
        }
    }

    pub fn get_name(&self) -> &'static str {
        match self {
            Self::USA => "USA",
            Self::China => "China",
            Self::GLA => "GLA",
            Self::Neutral => "Neutral",
        }
    }

    pub fn get_highlight_color(&self) -> [f32; 4] {
        match self {
            Self::USA => [0.4, 0.6, 1.0, 1.0],
            Self::China => [1.0, 0.4, 0.4, 1.0],
            Self::GLA => [1.0, 0.8, 0.4, 1.0],
            Self::Neutral => [0.7, 0.7, 0.7, 1.0],
        }
    }
}

/// Kind of combat feedback particle system (host registry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CombatParticleKind {
    DeathExplosion,
    DeathSmoke,
    DeathBurn,
    DeathPoison,
    DeathLaser,
    WeaponMuzzleFlash,
    WeaponImpact,
    ProjectileExhaust,
    ParticleSysBone,
    BodyFire,
    BodySmoke,
    DisableFx,
}

impl CombatParticleKind {
    /// Template name matching GameClient particle_presets where applicable.
    pub fn template_name(self) -> &'static str {
        match self {
            Self::DeathExplosion => "MediumExplosion",
            Self::DeathSmoke | Self::DeathBurn | Self::DeathPoison | Self::ParticleSysBone => {
                "SmokePlume"
            }
            Self::DeathLaser | Self::WeaponImpact => "BulletImpact",
            Self::WeaponMuzzleFlash => "MuzzleFlash",
            Self::ProjectileExhaust => "MissileExhaust",
            Self::BodyFire => "FireSmall",
            Self::BodySmoke => "SmokeSmall",
            Self::DisableFx => "DisabledEffectBinaryShower0",
        }
    }

    /// Host sweep lifetime (logic frames) for one-shot feedback kinds.
    pub fn sweep_system_lifetime(self) -> Option<u32> {
        match self {
            Self::DeathExplosion
            | Self::DeathLaser
            | Self::WeaponMuzzleFlash
            | Self::WeaponImpact => Some(2),
            Self::DeathSmoke | Self::DeathBurn | Self::DeathPoison => Some(600),
            Self::DisableFx => Some(30),
            Self::ProjectileExhaust | Self::ParticleSysBone | Self::BodyFire | Self::BodySmoke => {
                None
            }
        }
    }
}
