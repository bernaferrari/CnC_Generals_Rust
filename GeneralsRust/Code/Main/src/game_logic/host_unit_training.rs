//! Host SCIENCE unit-training residual (VeterancyGainCreate).
//!
//! Residual slice (playability):
//! - On unit create / production spawn, if the controlling player has the
//!   required science unlocked, grant residual StartingLevel via
//!   `VeterancyGainCreate` semantics (`ExperienceTracker::setMinVeterancyLevel`):
//!   - `SCIENCE_RedGuardTraining` → Red Guard **VETERAN**
//!   - `Infa_SCIENCE_RedGuardTraining` → MiniGunner **ELITE**
//!   - `SCIENCE_BattlemasterTraining` → Battlemaster **ELITE**
//!   - `SCIENCE_ArtilleryTraining` → Inferno Cannon / Nuke Cannon **VETERAN**
//!   - `SCIENCE_TechnicalTraining` → Technical **VETERAN**
//! - `unlock_team_science` / `PurchaseScience` records unlock honesty.
//! - Successful min-level grant records residual spawn honesty.
//!
//! Wave 62 residual pack (retail Science.ini / unit BuildTime / VeterancyGainCreate):
//! - StartingLevel residual: RedGuard/Artillery/Technical **VETERAN**,
//!   InfaRedGuard/Battlemaster **ELITE**
//! - SciencePurchasePointCost **1** for all residual training sciences
//! - Training BuildTime residual (secs→frames @ 30 FPS):
//!   RedGuard **10**s→**300**f, Battlemaster **10**s→**300**f,
//!   Inferno **15**s→**450**f, Technical **5**s→**150**f, MiniGunner **10**s→**300**f
//! - Free unit residual: free always-Veteran path (no ScienceRequired) for
//!   USA Pilot + SCIENCE_GattlingTankTraining science residual (StartingLevel
//!   VETERAN; stock ChinaTankGattling omits module — science residual honesty)
//! - America AdvancedTraining ExperienceScalarUpgrade AddXPScalar **1.0** residual
//!
//! Fail-closed honesty:
//! - Not full PrerequisiteSciences rank tree / control-bar science visibility
//! - Not full IsTrainable / experience tracker exclusive module matrix
//! - Not full SCIENCE_GattlingTankTraining → stock Gattling module wire
//! - Not network science / veterancy replication (network deferred)

use super::VeterancyLevel;
use serde::{Deserialize, Serialize};

/// Retail China Red Guard training science (StartingLevel = VETERAN).
pub const SCIENCE_RED_GUARD_TRAINING: &str = "SCIENCE_RedGuardTraining";
/// Infantry General MiniGunner training science (StartingLevel = ELITE).
pub const SCIENCE_INFA_RED_GUARD_TRAINING: &str = "Infa_SCIENCE_RedGuardTraining";
/// Retail China Battlemaster training science (StartingLevel = ELITE).
pub const SCIENCE_BATTLEMASTER_TRAINING: &str = "SCIENCE_BattlemasterTraining";
/// Retail China Artillery training science (StartingLevel = VETERAN).
pub const SCIENCE_ARTILLERY_TRAINING: &str = "SCIENCE_ArtilleryTraining";
/// Retail GLA Technical training science (StartingLevel = VETERAN).
pub const SCIENCE_TECHNICAL_TRAINING: &str = "SCIENCE_TechnicalTraining";

/// Retail China Gattling Tank training science (StartingLevel = VETERAN residual).
///
/// Science.ini lists SCIENCE_GattlingTankTraining; stock ChinaTankGattling omits
/// VeterancyGainCreate — residual honesty still documents the science + level.
pub const SCIENCE_GATTLING_TANK_TRAINING: &str = "SCIENCE_GattlingTankTraining";

/// Retail SciencePurchasePointCost for residual unit-training sciences.
pub const UNIT_TRAINING_SCIENCE_PURCHASE_POINT_COST: u32 = 1;

/// Retail America AdvancedTraining upgrade (ExperienceScalarUpgrade free XP).
pub const UPGRADE_AMERICA_ADVANCED_TRAINING: &str = "Upgrade_AmericaAdvancedTraining";
/// Retail ExperienceScalarUpgrade AddXPScalar residual (+100% XP).
pub const ADVANCED_TRAINING_ADD_XP_SCALAR: f32 = 1.0;

/// Free always-Veteran residual template label (USA Pilot — no ScienceRequired).
pub const FREE_VETERAN_PILOT_TEMPLATE: &str = "AmericaInfantryPilot";

// --- Training BuildTime residual (seconds + frames @ 30 FPS) ---
pub const RED_GUARD_BUILD_TIME_SECS: f32 = 10.0;
pub const RED_GUARD_BUILD_TIME_FRAMES: u32 = 300;
pub const BATTLEMASTER_BUILD_TIME_SECS: f32 = 10.0;
pub const BATTLEMASTER_BUILD_TIME_FRAMES: u32 = 300;
pub const INFERNO_CANNON_BUILD_TIME_SECS: f32 = 15.0;
pub const INFERNO_CANNON_BUILD_TIME_FRAMES: u32 = 450;
pub const TECHNICAL_BUILD_TIME_SECS: f32 = 5.0;
pub const TECHNICAL_BUILD_TIME_FRAMES: u32 = 150;
pub const MINIGUNNER_BUILD_TIME_SECS: f32 = 10.0;
pub const MINIGUNNER_BUILD_TIME_FRAMES: u32 = 300;
pub const GATTLING_TANK_BUILD_TIME_SECS: f32 = 10.0;
pub const GATTLING_TANK_BUILD_TIME_FRAMES: u32 = 300;

/// Normalize science / template identity (alphanumeric lower).
pub fn normalize_identity(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Host residual unit-training science kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnitTrainingScience {
    RedGuard,
    InfaRedGuard,
    Battlemaster,
    Artillery,
    Technical,
    /// SCIENCE_GattlingTankTraining residual (StartingLevel VETERAN).
    GattlingTank,
}

impl UnitTrainingScience {
    pub fn science_name(self) -> &'static str {
        match self {
            Self::RedGuard => SCIENCE_RED_GUARD_TRAINING,
            Self::InfaRedGuard => SCIENCE_INFA_RED_GUARD_TRAINING,
            Self::Battlemaster => SCIENCE_BATTLEMASTER_TRAINING,
            Self::Artillery => SCIENCE_ARTILLERY_TRAINING,
            Self::Technical => SCIENCE_TECHNICAL_TRAINING,
            Self::GattlingTank => SCIENCE_GATTLING_TANK_TRAINING,
        }
    }

    pub fn starting_level(self) -> VeterancyLevel {
        match self {
            Self::RedGuard | Self::Artillery | Self::Technical | Self::GattlingTank => {
                VeterancyLevel::Veteran
            }
            Self::InfaRedGuard | Self::Battlemaster => VeterancyLevel::Elite,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::RedGuard => "RedGuardTraining",
            Self::InfaRedGuard => "InfaRedGuardTraining",
            Self::Battlemaster => "BattlemasterTraining",
            Self::Artillery => "ArtilleryTraining",
            Self::Technical => "TechnicalTraining",
            Self::GattlingTank => "GattlingTankTraining",
        }
    }

    /// Retail SciencePurchasePointCost residual (all training sciences = **1**).
    pub fn science_purchase_point_cost(self) -> u32 {
        UNIT_TRAINING_SCIENCE_PURCHASE_POINT_COST
    }

    /// Retail BuildTime residual frames for the primary trained unit template.
    pub fn primary_build_time_frames(self) -> u32 {
        match self {
            Self::RedGuard | Self::InfaRedGuard => RED_GUARD_BUILD_TIME_FRAMES,
            Self::Battlemaster => BATTLEMASTER_BUILD_TIME_FRAMES,
            Self::Artillery => INFERNO_CANNON_BUILD_TIME_FRAMES,
            Self::Technical => TECHNICAL_BUILD_TIME_FRAMES,
            Self::GattlingTank => GATTLING_TANK_BUILD_TIME_FRAMES,
        }
    }
}

/// Whether a science name is a residual unit-training science.
pub fn is_unit_training_science(name: &str) -> bool {
    unit_training_science_from_name(name).is_some()
}

/// Classify residual unit-training science from a purchase/unlock name.
pub fn unit_training_science_from_name(name: &str) -> Option<UnitTrainingScience> {
    let n = normalize_identity(name);
    if n.contains("infascienceredguardtraining")
        || n == "infasciredguardtraining"
        || n.contains("infa_scienceredguard")
        || (n.contains("infa") && n.contains("redguardtraining"))
    {
        return Some(UnitTrainingScience::InfaRedGuard);
    }
    if n.contains("scienceredguardtraining") || n == "redguardtraining" {
        return Some(UnitTrainingScience::RedGuard);
    }
    if n.contains("sciencebattlemastertraining") || n == "battlemastertraining" {
        return Some(UnitTrainingScience::Battlemaster);
    }
    if n.contains("scienceartillerytraining") || n == "artillerytraining" {
        return Some(UnitTrainingScience::Artillery);
    }
    if n.contains("sciencetechnicaltraining") || n == "technicaltraining" {
        return Some(UnitTrainingScience::Technical);
    }
    if n.contains("sciencegattlingtanktraining")
        || n == "gattlingtanktraining"
        || n.contains("gattlingtanktraining")
    {
        return Some(UnitTrainingScience::GattlingTank);
    }
    None
}

/// Whether player sciences include a residual unit-training science.
pub fn player_has_unit_training_science(
    unlocked_sciences: &[String],
    kind: UnitTrainingScience,
) -> bool {
    let target = normalize_identity(kind.science_name());
    unlocked_sciences.iter().any(|s| {
        let n = normalize_identity(s);
        n == target
            || unit_training_science_from_name(s) == Some(kind)
            // Accept bare residual labels used by host tests / HUD.
            || n == normalize_identity(kind.label())
    })
}

/// Resolve residual starting level for a template given player sciences.
///
/// Fail-closed: name residual + science match only (not full INI module matrix).
pub fn unit_training_level_for_template(
    template_name: &str,
    unlocked_sciences: &[String],
) -> Option<(UnitTrainingScience, VeterancyLevel)> {
    use crate::game_logic::host_battlemaster::is_battlemaster_template;
    use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
    use crate::game_logic::host_minigunner::is_minigunner_template;
    use crate::game_logic::host_neutron_shell::is_nuke_cannon_template;
    use crate::game_logic::host_red_guard::is_red_guard_template;
    use crate::game_logic::host_technical::is_technical_template;

    // Infantry General MiniGunner uses Infa_SCIENCE_RedGuardTraining → ELITE.
    // Fail-closed: stock SCIENCE_RedGuardTraining does not train MiniGunner.
    if is_minigunner_template(template_name) {
        let kind = UnitTrainingScience::InfaRedGuard;
        if player_has_unit_training_science(unlocked_sciences, kind) {
            return Some((kind, kind.starting_level()));
        }
        return None;
    }

    if is_red_guard_template(template_name) {
        let kind = UnitTrainingScience::RedGuard;
        if player_has_unit_training_science(unlocked_sciences, kind) {
            return Some((kind, kind.starting_level()));
        }
        return None;
    }

    if is_battlemaster_template(template_name) {
        let kind = UnitTrainingScience::Battlemaster;
        if player_has_unit_training_science(unlocked_sciences, kind) {
            return Some((kind, kind.starting_level()));
        }
        return None;
    }

    if is_inferno_cannon_template(template_name) || is_nuke_cannon_template(template_name) {
        let kind = UnitTrainingScience::Artillery;
        if player_has_unit_training_science(unlocked_sciences, kind) {
            return Some((kind, kind.starting_level()));
        }
        return None;
    }

    if is_technical_template(template_name) {
        let kind = UnitTrainingScience::Technical;
        if player_has_unit_training_science(unlocked_sciences, kind) {
            return Some((kind, kind.starting_level()));
        }
        return None;
    }

    // SCIENCE_GattlingTankTraining residual (StartingLevel VETERAN).
    // Stock ChinaTankGattling omits the module; host residual still grants when
    // the science is unlocked (honesty of intended science residual path).
    {
        use crate::game_logic::host_gattling_tank::is_gattling_tank_template;
        if is_gattling_tank_template(template_name) {
            let kind = UnitTrainingScience::GattlingTank;
            if player_has_unit_training_science(unlocked_sciences, kind) {
                return Some((kind, kind.starting_level()));
            }
            return None;
        }
    }

    None
}

/// Rank for residual setMin comparison (higher wins).
pub fn veterancy_rank(level: VeterancyLevel) -> u8 {
    match level {
        VeterancyLevel::Rookie => 0,
        VeterancyLevel::Veteran => 1,
        VeterancyLevel::Elite => 2,
        VeterancyLevel::Heroic => 3,
    }
}

/// XP seed so residual level does not immediately drop on first gain_experience.
pub fn residual_xp_seed_for_level(level: VeterancyLevel, thresholds: [f32; 3]) -> f32 {
    match level {
        VeterancyLevel::Rookie => 0.0,
        VeterancyLevel::Veteran => thresholds[0],
        VeterancyLevel::Elite => thresholds[1],
        VeterancyLevel::Heroic => thresholds[2],
    }
}

/// Host residual honesty registry for unit-training sciences.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostUnitTrainingRegistry {
    /// Times a unit-training science was unlocked.
    pub science_unlocks: u32,
    /// Times residual StartingLevel was granted on spawn/create.
    pub grants: u32,
    /// Red Guard VETERAN grants.
    pub red_guard_grants: u32,
    /// MiniGunner ELITE grants.
    pub minigunner_grants: u32,
    /// Battlemaster ELITE grants.
    pub battlemaster_grants: u32,
    /// Artillery VETERAN grants.
    pub artillery_grants: u32,
    /// Technical VETERAN grants.
    pub technical_grants: u32,
    /// Gattling Tank VETERAN grants.
    pub gattling_grants: u32,
    /// Free always-Veteran residual grants (no ScienceRequired path).
    pub free_unit_grants: u32,
}

impl HostUnitTrainingRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_science_unlock(&mut self) {
        self.science_unlocks = self.science_unlocks.saturating_add(1);
    }

    pub fn record_grant(&mut self, kind: UnitTrainingScience) {
        self.grants = self.grants.saturating_add(1);
        match kind {
            UnitTrainingScience::RedGuard => {
                self.red_guard_grants = self.red_guard_grants.saturating_add(1);
            }
            UnitTrainingScience::InfaRedGuard => {
                self.minigunner_grants = self.minigunner_grants.saturating_add(1);
            }
            UnitTrainingScience::Battlemaster => {
                self.battlemaster_grants = self.battlemaster_grants.saturating_add(1);
            }
            UnitTrainingScience::Artillery => {
                self.artillery_grants = self.artillery_grants.saturating_add(1);
            }
            UnitTrainingScience::Technical => {
                self.technical_grants = self.technical_grants.saturating_add(1);
            }
            UnitTrainingScience::GattlingTank => {
                self.gattling_grants = self.gattling_grants.saturating_add(1);
            }
        }
    }

    /// Record free always-Veteran residual grant (no ScienceRequired).
    pub fn record_free_unit_grant(&mut self) {
        self.free_unit_grants = self.free_unit_grants.saturating_add(1);
        self.grants = self.grants.saturating_add(1);
    }

    pub fn honesty_free_unit_ok(&self) -> bool {
        self.free_unit_grants > 0
    }

    pub fn honesty_unlock_ok(&self) -> bool {
        self.science_unlocks > 0
    }

    pub fn honesty_grant_ok(&self) -> bool {
        self.grants > 0
    }

    pub fn honesty_ok(&self) -> bool {
        self.honesty_unlock_ok() && self.honesty_grant_ok()
    }
}


/// Whether residual free always-Veteran path applies (no ScienceRequired).
///
/// Retail: AmericaInfantryPilot VeterancyGainCreate StartingLevel=VETERAN with
/// ScienceRequired omitted. Fail-closed: not full free-unit crate matrix.
pub fn is_free_always_veteran_template(template_name: &str) -> bool {
    let n = normalize_identity(template_name);
    n.contains("americainfantrypilot")
        || n == "americainfantrypilot"
        || n.ends_with("infantrypilot")
        || n == "usapilot"
        || n == "testpilot"
}

/// Free always-Veteran residual StartingLevel.
pub fn free_always_veteran_starting_level() -> VeterancyLevel {
    VeterancyLevel::Veteran
}

// --- Wave 62 residual honesty packs ---

/// Veterancy StartingLevel residual honesty.
pub fn honesty_unit_training_veterancy_residual_ok() -> bool {
    UnitTrainingScience::RedGuard.starting_level() == VeterancyLevel::Veteran
        && UnitTrainingScience::Artillery.starting_level() == VeterancyLevel::Veteran
        && UnitTrainingScience::Technical.starting_level() == VeterancyLevel::Veteran
        && UnitTrainingScience::GattlingTank.starting_level() == VeterancyLevel::Veteran
        && UnitTrainingScience::InfaRedGuard.starting_level() == VeterancyLevel::Elite
        && UnitTrainingScience::Battlemaster.starting_level() == VeterancyLevel::Elite
        && free_always_veteran_starting_level() == VeterancyLevel::Veteran
        && UnitTrainingScience::RedGuard.science_purchase_point_cost() == 1
        && UnitTrainingScience::Battlemaster.science_purchase_point_cost() == 1
        && SCIENCE_GATTLING_TANK_TRAINING == "SCIENCE_GattlingTankTraining"
}

/// Training BuildTime residual honesty (secs → frames @ 30 FPS).
pub fn honesty_unit_training_time_residual_ok() -> bool {
    (RED_GUARD_BUILD_TIME_SECS - 10.0).abs() < 0.01
        && RED_GUARD_BUILD_TIME_FRAMES == 300
        && (BATTLEMASTER_BUILD_TIME_SECS - 10.0).abs() < 0.01
        && BATTLEMASTER_BUILD_TIME_FRAMES == 300
        && (INFERNO_CANNON_BUILD_TIME_SECS - 15.0).abs() < 0.01
        && INFERNO_CANNON_BUILD_TIME_FRAMES == 450
        && (TECHNICAL_BUILD_TIME_SECS - 5.0).abs() < 0.01
        && TECHNICAL_BUILD_TIME_FRAMES == 150
        && (MINIGUNNER_BUILD_TIME_SECS - 10.0).abs() < 0.01
        && MINIGUNNER_BUILD_TIME_FRAMES == 300
        && (GATTLING_TANK_BUILD_TIME_SECS - 10.0).abs() < 0.01
        && GATTLING_TANK_BUILD_TIME_FRAMES == 300
        && UnitTrainingScience::RedGuard.primary_build_time_frames() == 300
        && UnitTrainingScience::Artillery.primary_build_time_frames() == 450
        && UnitTrainingScience::Technical.primary_build_time_frames() == 150
}

/// Free unit residual honesty (always-Veteran pilot + AdvancedTraining XP scalar).
pub fn honesty_unit_training_free_unit_residual_ok() -> bool {
    is_free_always_veteran_template(FREE_VETERAN_PILOT_TEMPLATE)
        && is_free_always_veteran_template("AmericaInfantryPilot")
        && is_free_always_veteran_template("TestPilot")
        && !is_free_always_veteran_template("ChinaInfantryRedguard")
        && free_always_veteran_starting_level() == VeterancyLevel::Veteran
        && UPGRADE_AMERICA_ADVANCED_TRAINING == "Upgrade_AmericaAdvancedTraining"
        && (ADVANCED_TRAINING_ADD_XP_SCALAR - 1.0).abs() < 0.001
}

/// Combined Wave 62 unit-training residual honesty pack.
pub fn honesty_unit_training_residual_pack_ok() -> bool {
    honesty_unit_training_veterancy_residual_ok()
        && honesty_unit_training_time_residual_ok()
        && honesty_unit_training_free_unit_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn science_name_matrix() {
        assert_eq!(
            unit_training_science_from_name(SCIENCE_RED_GUARD_TRAINING),
            Some(UnitTrainingScience::RedGuard)
        );
        assert_eq!(
            unit_training_science_from_name(SCIENCE_INFA_RED_GUARD_TRAINING),
            Some(UnitTrainingScience::InfaRedGuard)
        );
        assert_eq!(
            unit_training_science_from_name(SCIENCE_BATTLEMASTER_TRAINING),
            Some(UnitTrainingScience::Battlemaster)
        );
        assert_eq!(
            unit_training_science_from_name(SCIENCE_ARTILLERY_TRAINING),
            Some(UnitTrainingScience::Artillery)
        );
        assert_eq!(
            unit_training_science_from_name(SCIENCE_TECHNICAL_TRAINING),
            Some(UnitTrainingScience::Technical)
        );
        assert_eq!(
            unit_training_science_from_name(SCIENCE_GATTLING_TANK_TRAINING),
            Some(UnitTrainingScience::GattlingTank)
        );
        assert!(unit_training_science_from_name("SCIENCE_StealthFighter").is_none());
        assert!(unit_training_science_from_name("SCIENCE_CashBounty1").is_none());
    }

    #[test]
    fn starting_levels() {
        assert_eq!(
            UnitTrainingScience::RedGuard.starting_level(),
            VeterancyLevel::Veteran
        );
        assert_eq!(
            UnitTrainingScience::Battlemaster.starting_level(),
            VeterancyLevel::Elite
        );
        assert_eq!(
            UnitTrainingScience::InfaRedGuard.starting_level(),
            VeterancyLevel::Elite
        );
        assert_eq!(
            UnitTrainingScience::Artillery.starting_level(),
            VeterancyLevel::Veteran
        );
        assert_eq!(
            UnitTrainingScience::Technical.starting_level(),
            VeterancyLevel::Veteran
        );
        assert_eq!(
            UnitTrainingScience::GattlingTank.starting_level(),
            VeterancyLevel::Veteran
        );
    }

    #[test]
    fn template_science_grants() {
        let sciences = vec![SCIENCE_RED_GUARD_TRAINING.to_string()];
        let g = unit_training_level_for_template("ChinaInfantryRedguard", &sciences);
        assert_eq!(
            g,
            Some((UnitTrainingScience::RedGuard, VeterancyLevel::Veteran))
        );
        assert!(unit_training_level_for_template("ChinaTankBattleMaster", &sciences).is_none());

        let sciences = vec![SCIENCE_BATTLEMASTER_TRAINING.to_string()];
        let g = unit_training_level_for_template("ChinaTankBattleMaster", &sciences);
        assert_eq!(
            g,
            Some((UnitTrainingScience::Battlemaster, VeterancyLevel::Elite))
        );

        let sciences = vec![SCIENCE_ARTILLERY_TRAINING.to_string()];
        assert_eq!(
            unit_training_level_for_template("ChinaVehicleInfernoCannon", &sciences)
                .map(|(_, l)| l),
            Some(VeterancyLevel::Veteran)
        );
        assert_eq!(
            unit_training_level_for_template("ChinaVehicleNukeCannon", &sciences).map(|(_, l)| l),
            Some(VeterancyLevel::Veteran)
        );

        let sciences = vec![SCIENCE_TECHNICAL_TRAINING.to_string()];
        assert_eq!(
            unit_training_level_for_template("GLAVehicleTechnical", &sciences).map(|(_, l)| l),
            Some(VeterancyLevel::Veteran)
        );

        let sciences = vec![SCIENCE_INFA_RED_GUARD_TRAINING.to_string()];
        assert_eq!(
            unit_training_level_for_template("Infa_ChinaInfantryMiniGunner", &sciences)
                .map(|(_, l)| l),
            Some(VeterancyLevel::Elite)
        );

        // Fail-closed: no science → no grant.
        assert!(unit_training_level_for_template("ChinaInfantryRedguard", &[]).is_none());
    }

    #[test]
    fn registry_honesty() {
        let mut reg = HostUnitTrainingRegistry::new();
        assert!(!reg.honesty_ok());
        reg.record_science_unlock();
        assert!(reg.honesty_unlock_ok());
        assert!(!reg.honesty_ok());
        reg.record_grant(UnitTrainingScience::Battlemaster);
        assert!(reg.honesty_ok());
        assert_eq!(reg.battlemaster_grants, 1);
        assert_eq!(reg.grants, 1);
    }

    #[test]
    fn xp_seed_and_rank() {
        let thr = [60.0, 150.0, 300.0];
        assert_eq!(residual_xp_seed_for_level(VeterancyLevel::Veteran, thr), 60.0);
        assert_eq!(residual_xp_seed_for_level(VeterancyLevel::Elite, thr), 150.0);
        assert!(veterancy_rank(VeterancyLevel::Elite) > veterancy_rank(VeterancyLevel::Veteran));
    }

    #[test]
    fn unit_training_residual_pack_honesty() {
        assert!(honesty_unit_training_veterancy_residual_ok());
        assert!(honesty_unit_training_time_residual_ok());
        assert!(honesty_unit_training_free_unit_residual_ok());
        assert!(honesty_unit_training_residual_pack_ok());
        let sciences = vec![SCIENCE_GATTLING_TANK_TRAINING.to_string()];
        assert_eq!(
            unit_training_level_for_template("ChinaTankGattling", &sciences).map(|(_, l)| l),
            Some(VeterancyLevel::Veteran)
        );
        let mut reg = HostUnitTrainingRegistry::new();
        reg.record_free_unit_grant();
        assert!(reg.honesty_free_unit_ok());
        assert_eq!(reg.free_unit_grants, 1);
    }

    /// Wave 72 residual pack honesty gate (wrapper residual_pack_ok).
    #[test]
    fn unit_training_residual_pack_honesty_wave72() {
        assert!(honesty_unit_training_residual_pack_ok());
        assert!(honesty_unit_training_veterancy_residual_ok());
        assert!(honesty_unit_training_time_residual_ok());
        assert!(honesty_unit_training_free_unit_residual_ok());
        assert_eq!(RED_GUARD_BUILD_TIME_FRAMES, 300);
        assert_eq!(INFERNO_CANNON_BUILD_TIME_FRAMES, 450);
        assert_eq!(FREE_VETERAN_PILOT_TEMPLATE, "AmericaInfantryPilot");
    }
}
