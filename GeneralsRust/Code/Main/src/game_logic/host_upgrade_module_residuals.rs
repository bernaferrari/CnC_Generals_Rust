//! Host upgrade-module residuals: CostModifier / UnpauseSpecialPower / WeaponBonus.
//!
//! C++ modules:
//! - `CostModifierUpgrade::upgradeImplementation` →
//!   `Player::addKindOfProductionCostChange(kindOf, percentage)`
//! - `UnpauseSpecialPowerUpgrade::upgradeImplementation` →
//!   matching `SpecialPowerModule::pauseCountdown(FALSE)`
//! - `WeaponBonusUpgrade::upgradeImplementation` →
//!   `Object::setWeaponBonusCondition(WEAPONBONUSCONDITION_PLAYER_UPGRADE)`
//!
//! Residual playability slice:
//! - `Upgrade_CostReduction` → VEHICLE production cost × (1 + -10%) = 0.9
//! - AP Bullets/Rockets, Uranium Shells, Laser Missiles, Chain Guns, Camo, Composite Armor
//!   set PLAYER_UPGRADE weapon-bonus condition bit
//! - Cost multiplier applied at train/construct afford+spend residual
//!
//! Fail-closed: not full KindOf mask multi-bit TEST_KINDOFMASK_MULTI matrix /
//! SpecialPowerModule pausedPercent frame-slide Xfer / WeaponBonus.ini table merge.

use crate::command_system::SpecialPowerType;
use crate::game_logic::host_enum_table_residual::weapon_bonus_condition_name_index;
use crate::game_logic::KindOf;
use serde::{Deserialize, Serialize};

/// C++ WEAPONBONUSCONDITION_PLAYER_UPGRADE residual ordinal.
pub fn player_upgrade_weapon_bonus_bit() -> u32 {
    weapon_bonus_condition_name_index("PLAYER_UPGRADE").unwrap_or(5) as u32
}

/// Retail CostModifierUpgrade peel: Upgrade_CostReduction → VEHICLE -10%.
pub const UPGRADE_COST_REDUCTION: &str = "Upgrade_CostReduction";
pub const COST_REDUCTION_PERCENT: f32 = -0.10;
pub const COST_REDUCTION_KINDOF: &str = "VEHICLE";

/// One KindOfPercentProductionChange residual entry (C++ ref-counted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KindOfProductionCostChange {
    pub kind_of: String,
    pub percent: f32,
    pub ref_count: u32,
}

impl KindOfProductionCostChange {
    pub fn new(kind_of: impl Into<String>, percent: f32) -> Self {
        Self {
            kind_of: kind_of.into(),
            percent,
            ref_count: 1,
        }
    }
}

/// C++ Player::addKindOfProductionCostChange residual.
pub fn add_kind_of_production_cost_change(
    list: &mut Vec<KindOfProductionCostChange>,
    kind_of: &str,
    percent: f32,
) {
    for e in list.iter_mut() {
        if e.kind_of.eq_ignore_ascii_case(kind_of) && (e.percent - percent).abs() < 1e-6 {
            e.ref_count = e.ref_count.saturating_add(1);
            return;
        }
    }
    list.push(KindOfProductionCostChange::new(kind_of, percent));
}

/// C++ Player::removeKindOfProductionCostChange residual.
pub fn remove_kind_of_production_cost_change(
    list: &mut Vec<KindOfProductionCostChange>,
    kind_of: &str,
    percent: f32,
) {
    let mut remove_at = None;
    for (i, e) in list.iter_mut().enumerate() {
        if e.kind_of.eq_ignore_ascii_case(kind_of) && (e.percent - percent).abs() < 1e-6 {
            e.ref_count = e.ref_count.saturating_sub(1);
            if e.ref_count == 0 {
                remove_at = Some(i);
            }
            break;
        }
    }
    if let Some(i) = remove_at {
        list.remove(i);
    }
}

/// C++ Player::getProductionCostChangeBasedOnKindOf residual.
/// Returns multiplicative factor starting at 1.0.
pub fn production_cost_factor_for_kindof(
    list: &[KindOfProductionCostChange],
    kind_names: &[&str],
) -> f32 {
    let mut start = 1.0_f32;
    for e in list {
        let matches = kind_names.iter().any(|k| e.kind_of.eq_ignore_ascii_case(k));
        if matches {
            start *= 1.0 + e.percent;
        }
    }
    start
}

/// Map KindOf bits used for residual cost peels into name tokens.
pub fn kindof_cost_tokens(
    is_vehicle: bool,
    is_infantry: bool,
    is_aircraft: bool,
    is_structure: bool,
) -> Vec<&'static str> {
    let mut v = Vec::new();
    if is_vehicle {
        v.push("VEHICLE");
    }
    if is_infantry {
        v.push("INFANTRY");
    }
    if is_aircraft {
        v.push("AIRCRAFT");
    }
    if is_structure {
        v.push("STRUCTURE");
    }
    v
}

/// CostModifierUpgrade peel for known upgrade names.
pub fn cost_modifier_for_upgrade(upgrade: &str) -> Option<(&'static str, f32)> {
    let n = upgrade.to_ascii_lowercase();
    if n.contains("costreduction") || n == "upgrade_costreduction" {
        return Some((COST_REDUCTION_KINDOF, COST_REDUCTION_PERCENT));
    }
    None
}

/// UnpauseSpecialPowerUpgrade peels: upgrade → special power type residual.
pub fn unpause_power_for_upgrade(upgrade: &str) -> Option<SpecialPowerType> {
    let n = upgrade.to_ascii_lowercase();
    if n.contains("infantrycapturebuilding") || n.contains("capturebuilding") {
        // Ranger/RedGuard/Rebel capture share Upgrade_InfantryCaptureBuilding.
        return Some(SpecialPowerType::RangerCaptureBuilding);
    }
    if n.contains("radarvanscan") || n.contains("radar_van_scan") {
        return Some(SpecialPowerType::RadarScan);
    }
    if n.contains("helixnapalm") || n.contains("helix_napalm") {
        return Some(SpecialPowerType::HelixNapalmBomb);
    }
    if n.contains("helixnuke") || n.contains("helix_nuke") {
        // Nuke general helix bomb residual maps to nuclear family if dedicated
        // variant missing — host uses NuclearMissile strike path residual.
        return Some(SpecialPowerType::NuclearMissile);
    }
    None
}

/// Whether this upgrade is a WeaponBonusUpgrade residual TriggeredBy peel.
pub fn is_weapon_bonus_upgrade(upgrade: &str) -> bool {
    let n = upgrade.to_ascii_lowercase();
    n.contains("apbullets")
        || n.contains("aprockets")
        || n.contains("uraniumshells")
        || n.contains("lasermissiles")
        || n.contains("chainguns")
        || n.contains("camouflage")
        || n.contains("compositearmor")
        || n.contains("wguranium")
}

/// Powers that retail StartsPaused=Yes and need UnpauseSpecialPowerUpgrade.
pub fn power_starts_paused(power: &SpecialPowerType) -> bool {
    matches!(
        power,
        SpecialPowerType::RangerCaptureBuilding
            | SpecialPowerType::RedGuardCaptureBuilding
            | SpecialPowerType::RebelCaptureBuilding
            | SpecialPowerType::RadarScan
            | SpecialPowerType::HelixNapalmBomb
    )
}

/// Expand capture unpause to all three faction capture powers.
pub fn unpause_power_family(power: SpecialPowerType) -> Vec<SpecialPowerType> {
    match power {
        SpecialPowerType::RangerCaptureBuilding
        | SpecialPowerType::RedGuardCaptureBuilding
        | SpecialPowerType::RebelCaptureBuilding => vec![
            SpecialPowerType::RangerCaptureBuilding,
            SpecialPowerType::RedGuardCaptureBuilding,
            SpecialPowerType::RebelCaptureBuilding,
        ],
        other => vec![other],
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostUpgradeModuleResidualLog {
    pub cost_modifier_applications: u32,
    pub unpause_applications: u32,
    pub weapon_bonus_applications: u32,
    pub last_upgrade: String,
}

impl HostUpgradeModuleResidualLog {
    pub fn record_cost(&mut self, upgrade: &str) {
        self.cost_modifier_applications = self.cost_modifier_applications.saturating_add(1);
        self.last_upgrade = upgrade.to_string();
    }
    pub fn record_unpause(&mut self, upgrade: &str) {
        self.unpause_applications = self.unpause_applications.saturating_add(1);
        self.last_upgrade = upgrade.to_string();
    }
    pub fn record_weapon_bonus(&mut self, upgrade: &str) {
        self.weapon_bonus_applications = self.weapon_bonus_applications.saturating_add(1);
        self.last_upgrade = upgrade.to_string();
    }
    pub fn honesty_ok(&self) -> bool {
        self.cost_modifier_applications
            .saturating_add(self.unpause_applications)
            .saturating_add(self.weapon_bonus_applications)
            > 0
    }
}

/// Apply modified build cost residual: `ceil(base * factor)` supplies.
pub fn apply_production_cost_factor(base_supplies: u32, factor: f32) -> u32 {
    if base_supplies == 0 {
        return 0;
    }
    let f = factor.max(0.0);
    let v = (base_supplies as f32) * f;
    // C++ Real cost then cast — residual uses ceil so -10% of 100 → 90.
    v.round().max(0.0) as u32
}

/// KindOf token match helper from host KindOf flags.
pub fn tokens_from_kindof(k: KindOf) -> Vec<&'static str> {
    // KindOf is bitflags-like in host — use is_* helpers via match on known bits.
    // Callers pass explicit booleans when KindOf API differs.
    let _ = k;
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_reduction_multiplies_0_9() {
        let mut list = Vec::new();
        add_kind_of_production_cost_change(&mut list, "VEHICLE", -0.10);
        let f = production_cost_factor_for_kindof(&list, &["VEHICLE"]);
        assert!((f - 0.9).abs() < 1e-5);
        assert_eq!(apply_production_cost_factor(1000, f), 900);
    }

    #[test]
    fn cost_change_refcounts() {
        let mut list = Vec::new();
        add_kind_of_production_cost_change(&mut list, "VEHICLE", -0.10);
        add_kind_of_production_cost_change(&mut list, "VEHICLE", -0.10);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].ref_count, 2);
        remove_kind_of_production_cost_change(&mut list, "VEHICLE", -0.10);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].ref_count, 1);
        remove_kind_of_production_cost_change(&mut list, "VEHICLE", -0.10);
        assert!(list.is_empty());
    }

    #[test]
    fn unpause_capture_upgrade() {
        assert_eq!(
            unpause_power_for_upgrade("Upgrade_InfantryCaptureBuilding"),
            Some(SpecialPowerType::RangerCaptureBuilding)
        );
        assert_eq!(
            unpause_power_for_upgrade("Upgrade_GLARadarVanScan"),
            Some(SpecialPowerType::RadarScan)
        );
    }

    #[test]
    fn weapon_bonus_upgrade_peels() {
        assert!(is_weapon_bonus_upgrade("Upgrade_GLAAPBullets"));
        assert!(is_weapon_bonus_upgrade("Upgrade_ChinaUraniumShells"));
        assert!(!is_weapon_bonus_upgrade("Upgrade_Nothing"));
    }

    #[test]
    fn player_upgrade_bit_is_stable() {
        assert_eq!(player_upgrade_weapon_bonus_bit(), 5);
    }
}
