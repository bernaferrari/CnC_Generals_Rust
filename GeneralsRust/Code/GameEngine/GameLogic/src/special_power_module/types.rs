//! Special Power Types and Enumerations

use crate::common::*;
use std::fmt;

/// OCL create location type (matching C++ OclCreateLocType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OclCreateLocType {
    /// Create at edge nearest to source object
    CreateAtEdgeNearSource,
    /// Create at edge nearest to target location
    CreateAtEdgeNearTarget,
    /// Create at exact target location
    CreateAtLocation,
    /// Use owner object's location
    UseOwnerObject,
    /// Create above target location (airborne)
    CreateAboveLocation,
    /// Create at edge farthest from target
    CreateAtEdgeFarthestFromTarget,
}

impl Default for OclCreateLocType {
    fn default() -> Self {
        OclCreateLocType::CreateAtEdgeNearSource
    }
}

/// Special power identifier type
pub type SpecialPowerID = UnsignedInt;

/// Invalid special power ID constant
pub const INVALID_SPECIAL_POWER_ID: SpecialPowerID = 0;

/// Special power types - all supported superweapons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialPowerKind {
    /// Object Creation List - spawns units or effects
    OCL,
    /// Fire weapon from structure or unit
    FireWeapon,
    /// Grant cash bonus to player
    CashBounty,
    /// Turn enemy units to your side
    Defector,
    /// Cause fear effect on enemy units
    Demoralize,
    /// Reveal area of map temporarily
    SpyVision,
    /// Steal money from enemy player
    CashHack,
    /// Remove hazards from area (mines, toxins, etc)
    CleanupArea,
    /// Launch nuclear missile
    BaikonurLaunch,
    /// A-10 Strike
    A10Strike,
    /// Carpet Bomb
    CarpetBomb,
    /// Daisy Cutter
    DaisyCutter,
    /// Particle Cannon
    ParticleCannon,
    /// SCUD Storm
    ScudStorm,
    /// Artillery Barrage
    ArtilleryBarrage,
    /// Napalm Strike
    NapalmStrike,
    /// Cluster Mines
    ClusterMines,
    /// Ambulance heal
    Ambulance,
    /// Radar scan
    RadarScan,
    /// Emergency repair
    EmergencyRepair,
    /// Sneak Attack
    SneakAttack,
    /// Rebel Ambush
    Ambush,
    /// Custom power type
    Custom(u32),
}

impl fmt::Display for SpecialPowerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OCL => write!(f, "OCL"),
            Self::FireWeapon => write!(f, "FireWeapon"),
            Self::CashBounty => write!(f, "CashBounty"),
            Self::Defector => write!(f, "Defector"),
            Self::Demoralize => write!(f, "Demoralize"),
            Self::SpyVision => write!(f, "SpyVision"),
            Self::CashHack => write!(f, "CashHack"),
            Self::CleanupArea => write!(f, "CleanupArea"),
            Self::BaikonurLaunch => write!(f, "BaikonurLaunch"),
            Self::A10Strike => write!(f, "A10Strike"),
            Self::CarpetBomb => write!(f, "CarpetBomb"),
            Self::DaisyCutter => write!(f, "DaisyCutter"),
            Self::ParticleCannon => write!(f, "ParticleCannon"),
            Self::ScudStorm => write!(f, "ScudStorm"),
            Self::ArtilleryBarrage => write!(f, "ArtilleryBarrage"),
            Self::NapalmStrike => write!(f, "NapalmStrike"),
            Self::ClusterMines => write!(f, "ClusterMines"),
            Self::Ambulance => write!(f, "Ambulance"),
            Self::RadarScan => write!(f, "RadarScan"),
            Self::EmergencyRepair => write!(f, "EmergencyRepair"),
            Self::SneakAttack => write!(f, "SneakAttack"),
            Self::Ambush => write!(f, "Ambush"),
            Self::Custom(id) => write!(f, "Custom({})", id),
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SpecialPowerFlags: u32 {
        /// Power requires targeting
        const REQUIRES_TARGETING = 0x00000001;
        /// Power affects friendly units
        const AFFECTS_FRIENDLY = 0x00000002;
        /// Power affects enemy units
        const AFFECTS_ENEMY = 0x00000004;
        /// Power affects neutral units
        const AFFECTS_NEUTRAL = 0x00000008;
        /// Power affects buildings
        const AFFECTS_BUILDINGS = 0x00000010;
        /// Power affects terrain
        const AFFECTS_TERRAIN = 0x00000020;
        /// Power is instant (no targeting phase)
        const INSTANT = 0x00000040;
        /// Power shows radar effect
        const RADAR_EFFECT = 0x00000080;
        /// Power requires line of sight
        const REQUIRES_LOS = 0x00000100;
        /// Power can be cancelled during activation
        const CANCELLABLE = 0x00000200;
        /// Power is a superweapon
        const SUPERWEAPON = 0x00000400;
        /// Power is shared across team
        const SHARED_TEAM = 0x00000800;
        /// Power is one-shot (no recharge)
        const ONE_SHOT = 0x00001000;
        /// Power requires science/tech
        const REQUIRES_SCIENCE = 0x00002000;
        /// Power is player-specific
        const PLAYER_SPECIFIC = 0x00004000;
    }
}

/// Special power activation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationState {
    /// Power is ready to use
    Ready,
    /// Power is on cooldown
    Cooldown,
    /// Power is being targeted
    Targeting,
    /// Power is being activated
    Activating,
    /// Power is active (for duration-based powers)
    Active,
    /// Power is unavailable (missing requirements)
    Unavailable,
    /// Power is disabled
    Disabled,
}

/// Result of special power activation attempt
#[derive(Debug, Clone, PartialEq)]
pub enum ActivationResult {
    /// Power activated successfully
    Success,
    /// Power is on cooldown
    OnCooldown { remaining: Real },
    /// Player cannot afford power
    InsufficientFunds { cost: Int, available: Int },
    /// Invalid target location
    InvalidTarget { reason: String },
    /// Missing prerequisites
    MissingPrerequisites { required: Vec<AsciiString> },
    /// Power is disabled
    Disabled,
    /// Generic failure
    Failed { reason: String },
}

impl ActivationResult {
    pub fn is_success(&self) -> bool {
        matches!(self, ActivationResult::Success)
    }
}

/// Special power statistics for tracking
#[derive(Debug, Clone, Default)]
pub struct SpecialPowerStats {
    /// Total times activated
    pub activation_count: UnsignedInt,
    /// Total damage dealt
    pub total_damage: Real,
    /// Total units affected
    pub units_affected: UnsignedInt,
    /// Total buildings affected
    pub buildings_affected: UnsignedInt,
    /// Total money spent
    pub money_spent: Int,
    /// Last activation frame
    pub last_activation_frame: UnsignedInt,
}

impl SpecialPowerStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_activation(&mut self, current_frame: UnsignedInt, cost: Int) {
        self.activation_count += 1;
        self.money_spent += cost;
        self.last_activation_frame = current_frame;
    }

    pub fn record_damage(&mut self, damage: Real) {
        self.total_damage += damage;
    }

    pub fn record_unit_affected(&mut self) {
        self.units_affected += 1;
    }

    pub fn record_building_affected(&mut self) {
        self.buildings_affected += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_power_kind_display() {
        assert_eq!(format!("{}", SpecialPowerKind::OCL), "OCL");
        assert_eq!(format!("{}", SpecialPowerKind::FireWeapon), "FireWeapon");
        assert_eq!(
            format!("{}", SpecialPowerKind::BaikonurLaunch),
            "BaikonurLaunch"
        );
    }

    #[test]
    fn test_special_power_flags() {
        let flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::SUPERWEAPON;

        assert!(flags.contains(SpecialPowerFlags::REQUIRES_TARGETING));
        assert!(flags.contains(SpecialPowerFlags::AFFECTS_ENEMY));
        assert!(flags.contains(SpecialPowerFlags::SUPERWEAPON));
        assert!(!flags.contains(SpecialPowerFlags::AFFECTS_FRIENDLY));
    }

    #[test]
    fn test_activation_result() {
        let result = ActivationResult::Success;
        assert!(result.is_success());

        let result = ActivationResult::OnCooldown { remaining: 30.0 };
        assert!(!result.is_success());
    }

    #[test]
    fn test_special_power_stats() {
        let mut stats = SpecialPowerStats::new();
        assert_eq!(stats.activation_count, 0);

        stats.record_activation(100, 1000);
        assert_eq!(stats.activation_count, 1);
        assert_eq!(stats.money_spent, 1000);
        assert_eq!(stats.last_activation_frame, 100);

        stats.record_damage(500.0);
        assert_eq!(stats.total_damage, 500.0);

        stats.record_unit_affected();
        assert_eq!(stats.units_affected, 1);
    }
}
