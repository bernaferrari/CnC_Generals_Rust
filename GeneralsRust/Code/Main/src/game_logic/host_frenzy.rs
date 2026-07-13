//! Host China Frenzy ("Rage") special-power residual — temporary ally attack buff.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(Frenzy)` at a world location applies a temporary weapon-bonus
//!   enrage (retail SuperweaponFrenzy → Frenzy_InvisibleMarker_Level* →
//!   WeaponBonusUpdate → doTempWeaponBonus(FRENZY_ONE/TWO/THREE)).
//! - Nearby **same-team non-structure CAN_ATTACK residual** units receive a
//!   temporary damage multiplier (110% / 120% / 130%) for BonusDuration.
//! - Honesty counters/flags for residual gates and tests.
//!
//! Fail-closed honesty:
//! - Not full OCL Frenzy_InvisibleMarker spawn / DeletionUpdate lifetime object
//! - Not full KindOf multi-mask (CAN_ATTACK / STRUCTURE) beyond residual filters
//! - Not full ally relationship filter (uses same-team residual)
//! - Not full science tier upgrade matrix (default Level 1; optional level param)
//! - Not full FrenzyCloud particle / red TINT_STATUS_FRENZY drawable path
//! - Not network Frenzy replication (network deferred)
//!
//! Note: Retail Frenzy is a **China** Generals power (not GLA). Host residual
//! still applies to whatever team activates it (playability / tests).

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const FRENZY_LOGIC_FPS: f32 = 30.0;

/// Retail SuperweaponFrenzy RadiusCursorRadius residual (= 200).
/// Also matches Frenzy_InvisibleMarker WeaponBonusUpdate BonusRange = 200.
pub const HOST_FRENZY_RADIUS: f32 = 200.0;

/// Retail Frenzy_InvisibleMarker_Level1 BonusDuration = 10000 ms → 300 frames.
pub const FRENZY_LEVEL1_DURATION_MS: u32 = 10_000;
/// Retail Frenzy_InvisibleMarker_Level2 BonusDuration = 20000 ms → 600 frames.
pub const FRENZY_LEVEL2_DURATION_MS: u32 = 20_000;
/// Retail Frenzy_InvisibleMarker_Level3 BonusDuration = 30000 ms → 900 frames.
pub const FRENZY_LEVEL3_DURATION_MS: u32 = 30_000;

/// Retail GameData.ini WeaponBonus FRENZY_ONE DAMAGE 110%.
pub const FRENZY_LEVEL1_DAMAGE_MULT: f32 = 1.10;
/// Retail GameData.ini WeaponBonus FRENZY_TWO DAMAGE 120%.
pub const FRENZY_LEVEL2_DAMAGE_MULT: f32 = 1.20;
/// Retail GameData.ini WeaponBonus FRENZY_THREE DAMAGE 130%.
pub const FRENZY_LEVEL3_DAMAGE_MULT: f32 = 1.30;

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound = FrenzyActivate).
pub const FRENZY_ACTIVATE_AUDIO: &str = "FrenzyActivate";

/// Residual Frenzy science tier → FRENZY_ONE / TWO / THREE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostFrenzyLevel {
    /// SCIENCE_Frenzy1 → Frenzy_InvisibleMarker_Level1 (FRENZY_ONE).
    One = 1,
    /// SCIENCE_Frenzy2 → Frenzy_InvisibleMarker_Level2 (FRENZY_TWO).
    Two = 2,
    /// SCIENCE_Frenzy3 → Frenzy_InvisibleMarker_Level3 (FRENZY_THREE).
    Three = 3,
}

impl HostFrenzyLevel {
    /// Parse residual level from 1..=3 (fail-closed: unknown → One).
    pub fn from_u8(level: u8) -> Self {
        match level {
            2 => HostFrenzyLevel::Two,
            3 => HostFrenzyLevel::Three,
            _ => HostFrenzyLevel::One,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Retail BonusDuration in logic frames.
    pub fn duration_frames(self) -> u32 {
        let ms = match self {
            HostFrenzyLevel::One => FRENZY_LEVEL1_DURATION_MS,
            HostFrenzyLevel::Two => FRENZY_LEVEL2_DURATION_MS,
            HostFrenzyLevel::Three => FRENZY_LEVEL3_DURATION_MS,
        };
        (ms * 30) / 1000
    }

    /// Retail DAMAGE weapon-bonus multiplier.
    pub fn damage_multiplier(self) -> f32 {
        match self {
            HostFrenzyLevel::One => FRENZY_LEVEL1_DAMAGE_MULT,
            HostFrenzyLevel::Two => FRENZY_LEVEL2_DAMAGE_MULT,
            HostFrenzyLevel::Three => FRENZY_LEVEL3_DAMAGE_MULT,
        }
    }
}

/// Whether residual target can receive Frenzy / Rage attack buff.
///
/// Retail WeaponBonusUpdate filters:
/// - allies (host residual: same-team)
/// - alive
/// - RequiredAffectKindOf = CAN_ATTACK
/// - ForbiddenAffectKindOf = STRUCTURE
pub fn is_legal_frenzy_target(
    is_structure: bool,
    is_alive: bool,
    same_team: bool,
    can_attack: bool,
    under_construction: bool,
) -> bool {
    !is_structure && is_alive && same_team && can_attack && !under_construction
}

/// 2D distance check residual (C++ FROM_CENTER_2D / BonusRange).
pub fn in_frenzy_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// One active residual Frenzy activation bookkeeping entry (honesty / debug).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostFrenzy {
    pub id: u32,
    pub player_id: u32,
    pub location: Vec3,
    pub radius: f32,
    pub level: HostFrenzyLevel,
    pub activate_frame: u32,
    pub expire_frame: u32,
    pub caster_id: Option<ObjectId>,
    /// Ally units that received FRENZY weapon-bonus residual this activation.
    pub buffs: u32,
}

/// Host residual registry for Frenzy / Rage special power activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostFrenzyRegistry {
    next_id: u32,
    /// Recent activations (bookkeeping; buff timers live on objects).
    activations: Vec<HostFrenzy>,
    /// Total activations (honesty).
    pub activation_count: u32,
    /// Total FRENZY weapon-bonus grants applied.
    pub buff_count: u32,
}

impl HostFrenzyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn activation_count(&self) -> u32 {
        self.activation_count
    }

    pub fn buff_count(&self) -> u32 {
        self.buff_count
    }

    pub fn activations(&self) -> &[HostFrenzy] {
        &self.activations
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a successful residual Frenzy activation.
    pub fn record_activation(&mut self, frenzy: HostFrenzy) {
        self.activation_count = self.activation_count.saturating_add(1);
        self.buff_count = self.buff_count.saturating_add(frenzy.buffs);
        self.activations.push(frenzy);
        // Keep bookkeeping bounded (residual, not full history Xfer).
        if self.activations.len() > 32 {
            let drain = self.activations.len() - 32;
            self.activations.drain(0..drain);
        }
    }

    /// Residual honesty: at least one Frenzy activated.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activation_count > 0
    }

    /// Residual honesty: at least one unit received FRENZY attack buff.
    pub fn honesty_buff_ok(&self) -> bool {
        self.buff_count > 0
    }

    /// Combined host path: activated and applied at least one buff.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_buff_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frenzy_constants_match_retail_residual() {
        assert!((HOST_FRENZY_RADIUS - 200.0).abs() < 0.01);
        assert_eq!(HostFrenzyLevel::One.duration_frames(), 300);
        assert_eq!(HostFrenzyLevel::Two.duration_frames(), 600);
        assert_eq!(HostFrenzyLevel::Three.duration_frames(), 900);
        assert!((HostFrenzyLevel::One.damage_multiplier() - 1.10).abs() < 0.001);
        assert!((HostFrenzyLevel::Two.damage_multiplier() - 1.20).abs() < 0.001);
        assert!((HostFrenzyLevel::Three.damage_multiplier() - 1.30).abs() < 0.001);
        assert!(!FRENZY_ACTIVATE_AUDIO.is_empty());
    }

    #[test]
    fn legal_frenzy_target_matrix() {
        // structure, alive, same_team, can_attack, under_construction
        assert!(is_legal_frenzy_target(false, true, true, true, false));
        assert!(!is_legal_frenzy_target(true, true, true, true, false)); // structure
        assert!(!is_legal_frenzy_target(false, false, true, true, false)); // dead
        assert!(!is_legal_frenzy_target(false, true, false, true, false)); // enemy
        assert!(!is_legal_frenzy_target(false, true, true, false, false)); // cannot attack
        assert!(!is_legal_frenzy_target(false, true, true, true, true)); // under construction
    }

    #[test]
    fn frenzy_radius_and_level_parse() {
        assert!(in_frenzy_radius_2d((0.0, 0.0), (100.0, 0.0), 200.0));
        assert!(!in_frenzy_radius_2d((0.0, 0.0), (250.0, 0.0), 200.0));
        assert_eq!(HostFrenzyLevel::from_u8(1), HostFrenzyLevel::One);
        assert_eq!(HostFrenzyLevel::from_u8(2), HostFrenzyLevel::Two);
        assert_eq!(HostFrenzyLevel::from_u8(3), HostFrenzyLevel::Three);
        assert_eq!(HostFrenzyLevel::from_u8(99), HostFrenzyLevel::One); // fail-closed
    }

    #[test]
    fn honesty_registry_records_buffs() {
        let mut reg = HostFrenzyRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_activation(HostFrenzy {
            id,
            player_id: 1,
            location: Vec3::ZERO,
            radius: HOST_FRENZY_RADIUS,
            level: HostFrenzyLevel::One,
            activate_frame: 0,
            expire_frame: 300,
            caster_id: None,
            buffs: 2,
        });
        assert!(reg.honesty_activate_ok());
        assert!(reg.honesty_buff_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.activation_count(), 1);
        assert_eq!(reg.buff_count(), 2);
    }
}
