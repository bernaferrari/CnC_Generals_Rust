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
use crate::game_logic::KindOf;
use crate::game_logic::host_enum_table_residual::weapon_bonus_condition_name_index;
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
        // C++ UnpauseSpecialPowerUpgrade matches SpecialPowerTemplate
        // Nuke_SpecialAbilityHelixNukeBomb — not the NuclearMissile SW.
        return Some(SpecialPowerType::HelixNukeBomb);
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
            | SpecialPowerType::HelixNukeBomb
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
    pub weapon_set_applications: u32,
    pub armor_set_applications: u32,
    pub locomotor_set_applications: u32,
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
    pub fn record_weapon_set(&mut self, upgrade: &str) {
        self.weapon_set_applications = self.weapon_set_applications.saturating_add(1);
        self.last_upgrade = upgrade.to_string();
    }
    pub fn record_armor_set(&mut self, upgrade: &str) {
        self.armor_set_applications = self.armor_set_applications.saturating_add(1);
        self.last_upgrade = upgrade.to_string();
    }
    pub fn record_locomotor_set(&mut self, upgrade: &str) {
        self.locomotor_set_applications = self.locomotor_set_applications.saturating_add(1);
        self.last_upgrade = upgrade.to_string();
    }
    pub fn honesty_ok(&self) -> bool {
        self.cost_modifier_applications
            .saturating_add(self.unpause_applications)
            .saturating_add(self.weapon_bonus_applications)
            .saturating_add(self.weapon_set_applications)
            .saturating_add(self.armor_set_applications)
            .saturating_add(self.locomotor_set_applications)
            > 0
    }
}

/// Apply a modified build cost with C++ `ThingTemplate::calcCostToBuild`
/// integer-return semantics.
///
/// C++ multiplies the authored integer cost by the Player's real modifiers,
/// then converts the result to `Int` at the return boundary.  That conversion
/// truncates toward zero; rounding would overcharge values such as
/// `101 * 0.8` to 81 instead of the retail 80.
pub fn apply_production_cost_factor(base_supplies: u32, factor: f32) -> u32 {
    if base_supplies == 0 {
        return 0;
    }
    let f = factor.max(0.0);
    let v = (base_supplies as f32) * f;
    // C++ `Real` cost then `Int` return conversion.
    v.trunc().max(0.0) as u32
}

/// KindOf token match helper from host KindOf flags.
pub fn tokens_from_kindof(k: KindOf) -> Vec<&'static str> {
    // KindOf is bitflags-like in host — use is_* helpers via match on known bits.
    // Callers pass explicit booleans when KindOf API differs.
    let _ = k;
    Vec::new()
}

// ---------------------------------------------------------------------------
// WeaponSetUpgrade / ArmorUpgrade / LocomotorSetUpgrade residuals
// ---------------------------------------------------------------------------

/// C++ `WeaponSetUpgrade::upgradeImplementation` → `setWeaponSetFlag(WEAPONSET_PLAYER_UPGRADE)`.
pub fn is_weapon_set_upgrade(upgrade: &str) -> bool {
    let n = upgrade.to_ascii_lowercase();
    // Retail WeaponSetUpgrade TriggeredBy peels (non-exhaustive playability set).
    n.contains("flashbang")
        || n.contains("sentrydronegun")
        || n.contains("scorpionrocket")
        || n.contains("comancherocket")
        || n.contains("rocketpods")
        || n.contains("blacknapalm")
        || n.contains("tacticalnukemig")
        || n.contains("buggyammo")
        || n.contains("armthemob")
        || n.contains("anthrax")
        || n.contains("autoloader")
        || n.contains("quadcannonsnipe")
        || n.contains("suicidebomb")
        // AdvancedTraining also has WeaponSetUpgrade on some units
        || n.contains("advancedtraining")
}

/// C++ `ArmorUpgrade::upgradeImplementation` → `setArmorSetFlag(ARMORSET_PLAYER_UPGRADE)`.
pub fn is_armor_upgrade(upgrade: &str) -> bool {
    let n = upgrade.to_ascii_lowercase();
    n.contains("chemicalsuits")
        || n.contains("compositearmor")
        || n.contains("countermeasures")
        || n.contains("fortifiedstructure")
        || n.contains("chinasmines")
        || n.contains("chinaempmines")
        || (n.contains("mines") && n.contains("china"))
        // AdvancedTraining also ArmorUpgrade on some USA units
        || n.contains("advancedtraining")
}

/// Chemical suits unique terrain decal residual.
pub fn is_chemical_suits_upgrade(upgrade: &str) -> bool {
    upgrade.to_ascii_lowercase().contains("chemicalsuits")
}

/// Residual name peels used only when Object INI is unavailable.
/// Retail LocomotorSetUpgrade TriggeredBy: WorkerShoes, NuclearTanks.
/// Heroic rank (`Upgrade_Veterancy_HEROIC`) and AutoLoader are not modules —
/// C++ only swaps via `Behavior = LocomotorSetUpgrade`.
fn is_residual_locomotor_set_upgrade_peel(upgrade: &str) -> bool {
    let n = upgrade.to_ascii_lowercase();
    n.contains("workershoes") || n.contains("nucleartanks")
}

/// C++ `LocomotorSetUpgrade` residual peel (no template). Heroic / AutoLoader
/// are not peels — they only fire when the object authors the module.
pub fn is_locomotor_set_upgrade(upgrade: &str) -> bool {
    is_residual_locomotor_set_upgrade_peel(upgrade)
}

/// Authored `Behavior = LocomotorSetUpgrade` + `TriggeredBy` on `template_name`.
/// `None` when the asset catalog is missing or the template is unknown.
pub fn template_locomotor_set_upgrade_triggered(
    template_name: &str,
    upgrade_name: &str,
) -> Option<bool> {
    let am = crate::assets::get_asset_manager()?;
    let mgr = am.lock().ok()?;
    let def = mgr
        .get_object_definition(template_name)
        .or_else(|| mgr.resolve_object_definition(template_name, None))?;
    Some(def.behavior_modules.iter().any(|module| {
        module
            .class_name
            .eq_ignore_ascii_case("LocomotorSetUpgrade")
            && module.attribute("TriggeredBy").is_some_and(|triggered| {
                crate::game_logic::host_status_bits_upgrade::triggered_by_lists_upgrade(
                    triggered,
                    upgrade_name,
                )
            })
    }))
}

/// C++ only `LocomotorSetUpgrade::upgradeImplementation` swaps. INI module
/// wins when the template is known; residual peels cover catalog-less tests.
pub fn should_apply_locomotor_set_upgrade(template_name: &str, upgrade: &str) -> bool {
    match template_locomotor_set_upgrade_triggered(template_name, upgrade) {
        Some(has_module) => has_module,
        None => is_residual_locomotor_set_upgrade_peel(upgrade),
    }
}

/// C++ `LocomotorSetType` residual used by `chooseLocomotorSet`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostLocomotorSetKind {
    Normal,
    NormalUpgraded,
    Panic,
    Wander,
    Taxiing,
    Supersonic,
    Sluggish,
}

/// C++ `TheLocomotorSetNames` token for `getCurLocomotorSetType`.
pub fn locomotor_set_kind_token(kind: HostLocomotorSetKind) -> &'static str {
    match kind {
        HostLocomotorSetKind::Normal | HostLocomotorSetKind::NormalUpgraded => "SET_NORMAL",
        HostLocomotorSetKind::Panic => "SET_PANIC",
        HostLocomotorSetKind::Wander => "SET_WANDER",
        HostLocomotorSetKind::Taxiing => "SET_TAXIING",
        HostLocomotorSetKind::Supersonic => "SET_SUPERSONIC",
        HostLocomotorSetKind::Sluggish => "SET_SLUGGISH",
    }
}

/// Whole-template swap C++ `chooseLocomotorSetExplicit` installs
/// (`AIUpdate.cpp:813` + `LocomotorSetUpgrade.cpp:30`). Not a speed peel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocomotorSetSwap {
    pub locomotor_name: &'static str,
    pub max_speed: f32,
    pub max_speed_damaged: f32,
    pub acceleration: f32,
    pub acceleration_damaged: f32,
    pub turn_rate: f32,
    pub turn_rate_damaged: f32,
    pub braking: f32,
    pub locomotor_surfaces: u32,
}

const LOCO_BIGNUM_BRAKE: f32 = 99999.0;
const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;

/// Retail SET_NORMAL / SET_NORMAL_UPGRADED / SET_PANIC / SET_TAXIING /
/// SET_SUPERSONIC / SET_SLUGGISH names for known templates.
pub fn locomotor_name_for_set_kind(
    template_name: &str,
    kind: HostLocomotorSetKind,
) -> Option<&'static str> {
    let t = template_name.to_ascii_lowercase();
    if t.contains("combatbike") {
        return Some(match kind {
            HostLocomotorSetKind::Normal | HostLocomotorSetKind::NormalUpgraded => {
                crate::game_logic::locomotor_bootstrap::COMBAT_BIKE_GROUND_LOCOMOTOR
            }
            HostLocomotorSetKind::Sluggish => {
                crate::game_logic::locomotor_bootstrap::COMBAT_BIKE_TERRORIST_GROUND_LOCOMOTOR
            }
            _ => return None,
        });
    }
    if crate::game_logic::host_aurora_bomb::is_aurora_aircraft_template(template_name) {
        return Some(match kind {
            HostLocomotorSetKind::Normal | HostLocomotorSetKind::NormalUpgraded => {
                crate::game_logic::locomotor_bootstrap::locomotor_name_for_unit(template_name)
                    .unwrap_or(crate::game_logic::locomotor_bootstrap::RAPTOR_JET_LOCOMOTOR)
            }
            HostLocomotorSetKind::Taxiing => {
                crate::game_logic::locomotor_bootstrap::RAPTOR_TAXIING_LOCOMOTOR
            }
            HostLocomotorSetKind::Supersonic => {
                crate::game_logic::locomotor_bootstrap::AURORA_SUPERSONIC_LOCOMOTOR
            }
            _ => return None,
        });
    }
    if t.contains("raptor") || t.contains("stealthfighter") || t.contains("stealth_fighter") {
        return Some(match kind {
            HostLocomotorSetKind::Normal | HostLocomotorSetKind::NormalUpgraded => {
                crate::game_logic::locomotor_bootstrap::RAPTOR_JET_LOCOMOTOR
            }
            HostLocomotorSetKind::Taxiing => {
                crate::game_logic::locomotor_bootstrap::RAPTOR_TAXIING_LOCOMOTOR
            }
            HostLocomotorSetKind::Supersonic => {
                crate::game_logic::locomotor_bootstrap::RAPTOR_SUPERSONIC_LOCOMOTOR
            }
            _ => return None,
        });
    }
    if t.contains("mig") {
        return Some(match kind {
            HostLocomotorSetKind::Normal | HostLocomotorSetKind::NormalUpgraded => {
                crate::game_logic::locomotor_bootstrap::MIG_LOCOMOTOR
            }
            HostLocomotorSetKind::Taxiing => {
                crate::game_logic::locomotor_bootstrap::AIRPLANE_TAXIING_LOCOMOTOR
            }
            HostLocomotorSetKind::Supersonic => {
                crate::game_logic::locomotor_bootstrap::MIG_SUPERSONIC_LOCOMOTOR
            }
            _ => return None,
        });
    }
    if t.contains("chinook") || t.contains("a10") || t.contains("thunderbolt") || t.contains("b52")
    {
        return Some(match kind {
            HostLocomotorSetKind::Normal | HostLocomotorSetKind::NormalUpgraded => {
                crate::game_logic::locomotor_bootstrap::locomotor_name_for_unit(template_name)
                    .unwrap_or(crate::game_logic::locomotor_bootstrap::CHINOOK_LOCOMOTOR)
            }
            HostLocomotorSetKind::Taxiing => {
                crate::game_logic::locomotor_bootstrap::AIRPLANE_TAXIING_LOCOMOTOR
            }
            _ => return None,
        });
    }
    if t.contains("jet") {
        return Some(match kind {
            HostLocomotorSetKind::Normal | HostLocomotorSetKind::NormalUpgraded => {
                crate::game_logic::locomotor_bootstrap::locomotor_name_for_unit(template_name)
                    .unwrap_or(crate::game_logic::locomotor_bootstrap::RAPTOR_JET_LOCOMOTOR)
            }
            HostLocomotorSetKind::Taxiing => {
                crate::game_logic::locomotor_bootstrap::AIRPLANE_TAXIING_LOCOMOTOR
            }
            HostLocomotorSetKind::Supersonic => {
                crate::game_logic::locomotor_bootstrap::RAPTOR_SUPERSONIC_LOCOMOTOR
            }
            _ => return None,
        });
    }
    if t.contains("worker") {
        return Some(match kind {
            HostLocomotorSetKind::Normal => "FastHumanLocomotor",
            HostLocomotorSetKind::NormalUpgraded => "WorkerShoesLocomotor",
            HostLocomotorSetKind::Panic => "PanicHumanLocomotor",
            HostLocomotorSetKind::Wander => "WanderHumanLocomotor",
            _ => return None,
        });
    }
    if crate::game_logic::host_nuclear_tanks::is_overlord_chassis_for_nuclear_speed(template_name) {
        return Some(match kind {
            HostLocomotorSetKind::Normal => "OverlordLocomotor",
            HostLocomotorSetKind::NormalUpgraded => "NuclearOverlordLocomotor",
            HostLocomotorSetKind::Panic | HostLocomotorSetKind::Wander => "OverlordLocomotor",
            _ => return None,
        });
    }
    if crate::game_logic::host_nuclear_tanks::is_nuclear_tanks_eligible(template_name)
        || t.contains("battlemaster")
    {
        return Some(match kind {
            HostLocomotorSetKind::Normal => "BattleMasterLocomotor",
            HostLocomotorSetKind::NormalUpgraded => "NuclearBattleMasterLocomotor",
            HostLocomotorSetKind::Panic | HostLocomotorSetKind::Wander => "BattleMasterLocomotor",
            _ => return None,
        });
    }
    // Civilian / infantry / angry-mob members share C++ PanicHuman / WanderHuman sets.
    if is_human_panic_wander_template(&t) {
        return Some(match kind {
            HostLocomotorSetKind::Normal | HostLocomotorSetKind::NormalUpgraded => {
                "FastHumanLocomotor"
            }
            HostLocomotorSetKind::Panic => "PanicHumanLocomotor",
            HostLocomotorSetKind::Wander => "WanderHumanLocomotor",
            _ => return None,
        });
    }
    None
}

fn is_human_panic_wander_template(t: &str) -> bool {
    t.contains("infantry")
        || t.contains("civilian")
        || t.contains("civ")
        || t.contains("angry")
        || t.contains("mob")
        || t.contains("human")
        || t.contains("rebel")
        || t.contains("pilot")
        || t.contains("terror")
        || t.contains("hijacker")
        || t.contains("saboteur")
        || t.contains("ranger")
        || t.contains("pathfinder")
        || t.contains("colonel")
        || t.contains("lotus")
        || t.contains("redguard")
        || t.contains("minigun")
        || t.contains("hacker")
        || t.contains("tankhunter")
        || t.contains("worker")
        || t.contains("flee")
}

fn seed_set_switch_locomotor(name: &str) {
    use game_engine::common::ini::ini_locomotor::{
        get_locomotor_store, get_locomotor_store_mut, parse_locomotor_template_definition,
    };
    let _ = crate::game_logic::ensure_host_locomotor_store();
    if get_locomotor_store().find_template(name).is_some() {
        return;
    }
    let (speed, accel, turn_deg, speed_dmg, accel_dmg, braking, appearance) = match name {
        // INIZH Locomotor.ini retail values (Speed 25 / SpeedDamaged 10).
        "FastHumanLocomotor" => ("25", "100", "500", "15", "50", None, "TWO_LEGS"),
        "WorkerShoesLocomotor" => ("30", "100", "500", "15", "50", None, "TWO_LEGS"),
        "PanicHumanLocomotor" => ("25", "100", "350", "10", "50", Some("100"), "TWO_LEGS"),
        "WanderHumanLocomotor" => ("10", "100", "350", "10", "50", None, "TWO_LEGS"),
        "NuclearOverlordLocomotor" => ("30", "15", "60", "30", "15", None, "TREADS"),
        "NuclearBattleMasterLocomotor" => ("35", "1000", "180", "32", "1000", None, "TREADS"),
        "AirplaneTaxiingLocomotor" | "RaptorTaxiingLocomotor" => {
            ("25", "40", "90", "25", "40", Some("50"), "WHEELS")
        }
        _ => return,
    };
    let mut props = std::collections::HashMap::new();
    props.insert("Speed".to_string(), speed.to_string());
    props.insert("Acceleration".to_string(), accel.to_string());
    props.insert("TurnRate".to_string(), turn_deg.to_string());
    props.insert("SpeedDamaged".to_string(), speed_dmg.to_string());
    props.insert("AccelerationDamaged".to_string(), accel_dmg.to_string());
    props.insert("Surfaces".to_string(), "GROUND".to_string());
    props.insert("Appearance".to_string(), appearance.to_string());
    props.insert("ZAxisBehavior".to_string(), "NO_Z_MOTIVE_FORCE".to_string());
    if let Some(brake) = braking {
        props.insert("Braking".to_string(), brake.to_string());
    }
    if let Ok(template) = parse_locomotor_template_definition(name, &props) {
        let _ = get_locomotor_store_mut().add_template(template);
    }
}

fn residual_swap_for_name(name: &'static str) -> Option<LocomotorSetSwap> {
    let (speed, accel, turn_deg, speed_dmg, accel_dmg, braking) = match name {
        // INIZH Locomotor.ini: PanicHuman Speed 25 (SpeedDamaged 10, TurnRate
        // 350, Accel 100/50, Braking 100); WanderHuman Speed 10.
        "FastHumanLocomotor" => (25.0, 100.0, 500.0, 15.0, 50.0, LOCO_BIGNUM_BRAKE),
        "WorkerShoesLocomotor" => (30.0, 100.0, 500.0, 15.0, 50.0, LOCO_BIGNUM_BRAKE),
        "PanicHumanLocomotor" => (25.0, 100.0, 350.0, 10.0, 50.0, 100.0),
        "WanderHumanLocomotor" => (10.0, 100.0, 350.0, 10.0, 50.0, 100.0),
        "NuclearOverlordLocomotor" => (30.0, 15.0, 60.0, 30.0, 15.0, LOCO_BIGNUM_BRAKE),
        "NuclearBattleMasterLocomotor" => (35.0, 1000.0, 180.0, 32.0, 1000.0, LOCO_BIGNUM_BRAKE),
        "BattleMasterLocomotor" => (25.0, 1000.0, 180.0, 25.0, 1000.0, LOCO_BIGNUM_BRAKE),
        "OverlordLocomotor" => (20.0, 15.0, 60.0, 20.0, 15.0, LOCO_BIGNUM_BRAKE),
        "AirplaneTaxiingLocomotor" | "RaptorTaxiingLocomotor" => {
            (25.0, 40.0, 90.0, 25.0, 40.0, 50.0)
        }
        "RaptorSupersonicLocomotor" => (250.0, 180.0, 90.0, 250.0, 180.0, LOCO_BIGNUM_BRAKE),
        "AuroraSupersonicLocomotor" => (320.0, 200.0, 80.0, 320.0, 200.0, LOCO_BIGNUM_BRAKE),
        "MIGSupersonicLocomotor" => (230.0, 160.0, 90.0, 230.0, 160.0, LOCO_BIGNUM_BRAKE),
        // INIZH Locomotor.ini: SET_SLUGGISH is heterogeneous — ground
        // ";25% slower" Speed 90 (SpeedDamaged 68, Accel 68/60) vs cliff
        // Speed 60 (SpeedDamaged 45, Accel 50/40); TurnRate 120, Braking 60.
        "CombatBikeTerroristGroundLocomotor" => (90.0, 68.0, 120.0, 68.0, 60.0, 60.0),
        "CombatBikeTerroristCliffLocomotor" => (60.0, 50.0, 120.0, 45.0, 40.0, 60.0),
        _ => return None,
    };
    let turn = turn_deg * DEG_TO_RAD;
    let locomotor_surfaces = if name.contains("Supersonic") {
        crate::game_logic::object::LOCO_SURFACE_AIR
    } else {
        crate::game_logic::object::LOCO_SURFACE_GROUND
    };
    Some(LocomotorSetSwap {
        locomotor_name: name,
        max_speed: speed,
        max_speed_damaged: speed_dmg,
        acceleration: accel,
        acceleration_damaged: accel_dmg,
        turn_rate: turn,
        turn_rate_damaged: turn,
        braking,
        locomotor_surfaces,
    })
}

fn binding_to_swap(
    name: &'static str,
    binding: &crate::game_logic::locomotor_bootstrap::HostLocomotorBinding,
) -> LocomotorSetSwap {
    LocomotorSetSwap {
        locomotor_name: name,
        max_speed: binding.movement.max_speed,
        max_speed_damaged: binding.max_speed_damaged,
        acceleration: binding.movement.acceleration,
        acceleration_damaged: binding.acceleration_damaged,
        turn_rate: binding.movement.turn_rate,
        turn_rate_damaged: binding.turn_rate_damaged,
        braking: binding.braking,
        locomotor_surfaces: binding.locomotor_surfaces,
    }
}

/// Resolve the whole SET_* template (turn/accel/brake/surfaces/speed).
pub fn locomotor_set_swap_for_kind(
    template_name: &str,
    kind: HostLocomotorSetKind,
) -> Option<LocomotorSetSwap> {
    let name = locomotor_name_for_set_kind(template_name, kind)?;
    seed_set_switch_locomotor(name);
    if let Some(binding) =
        crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding(name)
    {
        return Some(binding_to_swap(name, &binding));
    }
    residual_swap_for_name(name)
}

fn upgrade_set_kind(upgrade: &str) -> Option<HostLocomotorSetKind> {
    let u = upgrade.to_ascii_lowercase();
    if is_residual_locomotor_set_upgrade_peel(&u) {
        Some(HostLocomotorSetKind::NormalUpgraded)
    } else if u.contains("panic") {
        Some(HostLocomotorSetKind::Panic)
    } else {
        None
    }
}

/// C++ `LocomotorSetUpgrade` / panic / veteran set switch — whole template.
pub fn locomotor_upgrade_set(upgrade: &str, template_name: &str) -> Option<LocomotorSetSwap> {
    let kind = upgrade_set_kind(upgrade)?;
    locomotor_set_swap_for_kind(template_name, kind)
}

/// Retail LocomotorSetUpgrade speed (derived from the whole-template swap).
pub fn locomotor_upgrade_speed(upgrade: &str, template_name: &str) -> Option<f32> {
    locomotor_upgrade_set(upgrade, template_name).map(|swap| swap.max_speed)
}

fn apply_swap_fields(obj: &mut crate::game_logic::object::Object, swap: &LocomotorSetSwap) {
    if let Some(binding) =
        crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding(swap.locomotor_name)
    {
        crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(obj, &binding);
        return;
    }
    obj.movement.max_speed = swap.max_speed;
    obj.movement.max_speed_damaged = swap.max_speed_damaged;
    obj.movement.acceleration = swap.acceleration;
    obj.movement.acceleration_damaged = swap.acceleration_damaged;
    obj.movement.turn_rate = swap.turn_rate;
    obj.movement.turn_rate_damaged = swap.turn_rate_damaged;
    obj.braking = swap.braking;
    obj.locomotor_surfaces = swap.locomotor_surfaces;
    obj.record_host_locomotor();
    obj.record_host_movement();
}

/// C++ `AIUpdateInterface::chooseLocomotorSet` residual — swap the whole template.
pub fn apply_locomotor_set_kind(
    obj: &mut crate::game_logic::object::Object,
    kind: HostLocomotorSetKind,
) -> bool {
    obj.jet_ai.cur_locomotor_set = Some(locomotor_set_kind_token(kind).to_string());
    let swap = locomotor_set_swap_for_kind(&obj.template_name, kind).or_else(|| {
        if obj.is_kind_of(crate::game_logic::KindOf::Infantry)
            || obj.is_kind_of(crate::game_logic::KindOf::CanBeRepulsed)
            || obj.angry_mob_member
        {
            locomotor_set_swap_for_kind("CivilianInfantry", kind)
        } else if kind == HostLocomotorSetKind::Taxiing
            && (obj.is_kind_of(crate::game_logic::KindOf::Aircraft)
                || obj.object_type == crate::game_logic::ObjectType::Aircraft)
        {
            locomotor_set_swap_for_kind("AmericaJetRaptor", kind)
        } else {
            None
        }
    });
    let Some(swap) = swap else {
        return false;
    };
    apply_swap_fields(obj, &swap);
    true
}

/// C++ `chooseLocomotorSet` + `setModelConditionState(MODELCONDITION_PANICKING)`.
pub fn apply_choose_locomotor_set(
    obj: &mut crate::game_logic::object::Object,
    kind: HostLocomotorSetKind,
    panicking: bool,
) -> bool {
    let applied = apply_locomotor_set_kind(obj, kind);
    obj.is_panicking = panicking;
    let bit = crate::game_logic::host_enum_table_residual::panicking_model_bit();
    if panicking {
        obj.model_condition_bits |= 1u128 << bit;
    } else {
        obj.model_condition_bits &= !(1u128 << bit);
    }
    obj.record_host_model_condition();
    applied
}

/// C++ `LocomotorSetUpgrade::upgradeImplementation` live apply.
/// Sets `m_upgradedLocomotors` and installs SET_NORMAL_UPGRADED only when this
/// object authors the module (or a WorkerShoes / NuclearTanks residual peel).
pub fn apply_locomotor_set_upgrade(
    obj: &mut crate::game_logic::object::Object,
    upgrade: &str,
) -> bool {
    if !should_apply_locomotor_set_upgrade(&obj.template_name, upgrade) {
        return false;
    }
    obj.set_locomotor_upgrade(true);
    let swap =
        locomotor_set_swap_for_kind(&obj.template_name, HostLocomotorSetKind::NormalUpgraded)
            .or_else(|| locomotor_upgrade_set(upgrade, &obj.template_name));
    let Some(swap) = swap else {
        return true;
    };
    apply_swap_fields(obj, &swap);
    true
}

/// C++ ARMORSET_PLAYER_UPGRADE residual ordinal (ArmorSetType.h).
pub const ARMORSET_PLAYER_UPGRADE: u8 = 1; // after ARMORSET_VETERAN=0 in residual peel; host uses bool

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
        assert_eq!(
            unpause_power_for_upgrade("Upgrade_HelixNapalmBomb"),
            Some(SpecialPowerType::HelixNapalmBomb)
        );
        assert_eq!(
            unpause_power_for_upgrade("Nuke_Upgrade_HelixNukeBomb"),
            Some(SpecialPowerType::HelixNukeBomb)
        );
        assert!(power_starts_paused(&SpecialPowerType::HelixNukeBomb));
        assert_ne!(
            unpause_power_for_upgrade("Nuke_Upgrade_HelixNukeBomb"),
            Some(SpecialPowerType::NuclearMissile)
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

    #[test]
    fn weapon_set_upgrade_peels() {
        assert!(is_weapon_set_upgrade(
            "Upgrade_AmericaRangerFlashBangGrenade"
        ));
        assert!(is_weapon_set_upgrade("Upgrade_GLAScorpionRocket"));
        assert!(!is_weapon_set_upgrade("Upgrade_CostReduction"));
    }

    #[test]
    fn armor_and_locomotor_peels() {
        assert!(is_armor_upgrade("Upgrade_AmericaChemicalSuits"));
        assert!(is_chemical_suits_upgrade("Upgrade_AmericaChemicalSuits"));
        assert!(is_locomotor_set_upgrade("Upgrade_GLAWorkerShoes"));
        assert!(is_locomotor_set_upgrade("Upgrade_ChinaNuclearTanks"));
        assert!(!is_locomotor_set_upgrade("Upgrade_Veterancy_HEROIC"));
        assert!(!is_locomotor_set_upgrade(
            "Tank_Upgrade_ChinaTankAutoLoader"
        ));
        assert!(!should_apply_locomotor_set_upgrade(
            "ChinaTankBattleMaster",
            "Upgrade_Veterancy_HEROIC"
        ));
        assert!(!should_apply_locomotor_set_upgrade(
            "ChinaTankOverlord",
            "Tank_Upgrade_ChinaTankAutoLoader"
        ));
        assert_eq!(
            locomotor_upgrade_speed("Upgrade_GLAWorkerShoes", "GLAInfantryWorker"),
            Some(30.0)
        );
    }

    /// C++ `LocomotorSetUpgrade.cpp:30` + `AIUpdate.cpp:784-803` swaps the
    /// whole SET_NORMAL_UPGRADED template, not a max_speed peel.
    #[test]
    fn locomotor_set_upgrade_swaps_turn_accel_brake_surfaces() {
        let shoes = locomotor_upgrade_set("Upgrade_GLAWorkerShoes", "GLAInfantryWorker")
            .expect("WorkerShoes SET_NORMAL_UPGRADED");
        assert_eq!(shoes.locomotor_name, "WorkerShoesLocomotor");
        assert!((shoes.max_speed - 30.0).abs() < 0.05);
        assert!(shoes.acceleration > 0.0);
        assert!(shoes.turn_rate > 0.0);
        assert!(shoes.braking > 0.0);
        assert_eq!(
            shoes.locomotor_surfaces,
            crate::game_logic::object::LOCO_SURFACE_GROUND
        );

        let nuclear = locomotor_upgrade_set("Upgrade_ChinaNuclearTanks", "ChinaTankBattleMaster")
            .expect("Nuclear Battlemaster SET_NORMAL_UPGRADED");
        assert_eq!(nuclear.locomotor_name, "NuclearBattleMasterLocomotor");
        assert!((nuclear.max_speed - 35.0).abs() < 0.05);
        assert!(
            (nuclear.acceleration - shoes.acceleration).abs() > 1.0
                || (nuclear.turn_rate - shoes.turn_rate).abs() > 0.1,
            "upgraded tank template must differ in accel/turn from worker shoes"
        );
    }

    /// C++ `chooseLocomotorSet(LOCOMOTORSET_PANIC)` (AIStates.cpp:2272) installs
    /// the panic template's turn/accel/brake, not a speed scalar.
    #[test]
    fn panic_and_veteran_set_switch_is_whole_template() {
        let normal = locomotor_set_swap_for_kind("GLAInfantryWorker", HostLocomotorSetKind::Normal)
            .expect("SET_NORMAL");
        let panic = locomotor_set_swap_for_kind("GLAInfantryWorker", HostLocomotorSetKind::Panic)
            .expect("SET_PANIC");
        assert!(
            locomotor_upgrade_set("Upgrade_Veterancy_HEROIC", "GLAInfantryWorker").is_none(),
            "Heroic rank is giveUpgrade, not LocomotorSetUpgrade"
        );
        assert!(
            locomotor_upgrade_set("Tank_Upgrade_ChinaTankAutoLoader", "ChinaTankBattleMaster")
                .is_none(),
            "AutoLoader is WeaponSetUpgrade, not LocomotorSetUpgrade"
        );
        assert_eq!(panic.locomotor_name, "PanicHumanLocomotor");
        assert!(
            (panic.max_speed - normal.max_speed).abs() > 1.0,
            "panic speed must differ from normal"
        );
        assert!(
            (panic.acceleration - normal.acceleration).abs() > 1.0
                || (panic.turn_rate - normal.turn_rate).abs() > 0.1
                || (panic.braking - normal.braking).abs() > 1.0,
            "panic must swap accel/turn/brake, not speed only"
        );
    }

    /// Live apply must write turn/accel/brake/surfaces, not max_speed only.
    #[test]
    fn apply_locomotor_set_upgrade_writes_whole_template() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut t = ThingTemplate::new("GLAInfantryWorker");
        t.add_kind_of(KindOf::Infantry);
        let mut obj = Object::new(t, ObjectId(1), Team::GLA);
        obj.movement.max_speed = 10.0;
        obj.movement.acceleration = 5.0;
        obj.movement.turn_rate = 1.0;
        obj.braking = 50.0;
        obj.locomotor_surfaces = 0;
        assert!(apply_locomotor_set_upgrade(
            &mut obj,
            "Upgrade_GLAWorkerShoes"
        ));
        assert!(obj.locomotor_upgrade);
        assert!(
            (obj.movement.max_speed - 30.0).abs() < 0.05,
            "WorkerShoes speed, got {}",
            obj.movement.max_speed
        );
        assert!(
            obj.movement.acceleration > 5.0,
            "must swap accel, got {}",
            obj.movement.acceleration
        );
        assert!(
            obj.movement.turn_rate > 1.0,
            "must swap turn, got {}",
            obj.movement.turn_rate
        );
        assert_ne!(obj.locomotor_surfaces, 0, "must swap surfaces");
    }

    /// C++ giveUpgrade(HEROIC) / AutoLoader WeaponSetUpgrade must not install
    /// SET_NORMAL_UPGRADED (Nuclear* 25→35 / 20→30) on China tanks.
    #[test]
    fn heroic_and_autoloader_do_not_swap_china_tank_loco() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut t = ThingTemplate::new("ChinaTankBattleMaster");
        t.add_kind_of(KindOf::Vehicle);
        let mut obj = Object::new(t, ObjectId(2), Team::China);
        obj.movement.max_speed = 25.0;
        obj.movement.acceleration = 10.0;
        obj.locomotor_upgrade = false;
        assert!(!apply_locomotor_set_upgrade(
            &mut obj,
            "Upgrade_Veterancy_HEROIC"
        ));
        assert!(!obj.locomotor_upgrade);
        assert!((obj.movement.max_speed - 25.0).abs() < 0.05);
        assert!(!apply_locomotor_set_upgrade(
            &mut obj,
            "Tank_Upgrade_ChinaTankAutoLoader"
        ));
        assert!(!obj.locomotor_upgrade);
        assert!((obj.movement.max_speed - 25.0).abs() < 0.05);
    }

    #[test]
    fn apply_panic_set_swaps_more_than_speed() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut t = ThingTemplate::new("GLAInfantryWorker");
        t.add_kind_of(KindOf::Infantry);
        let mut obj = Object::new(t, ObjectId(2), Team::GLA);
        obj.movement.max_speed = 25.0;
        obj.movement.acceleration = 100.0;
        obj.movement.turn_rate = 500.0 * std::f32::consts::PI / 180.0;
        obj.braking = 99999.0;
        assert!(apply_locomotor_set_kind(
            &mut obj,
            HostLocomotorSetKind::Panic
        ));
        assert!(
            (obj.movement.max_speed - 25.0).abs() > 1.0,
            "panic speed must change"
        );
        assert!(
            (obj.movement.acceleration - 100.0).abs() > 1.0 || (obj.braking - 99999.0).abs() > 1.0,
            "panic must swap accel or brake, not speed only"
        );
    }

    #[test]
    fn taxiing_supersonic_sluggish_sets_swap_whole_template() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let taxi = locomotor_set_swap_for_kind("AmericaJetRaptor", HostLocomotorSetKind::Taxiing)
            .expect("SET_TAXIING");
        let cruise = locomotor_set_swap_for_kind("AmericaJetRaptor", HostLocomotorSetKind::Normal)
            .expect("SET_NORMAL");
        let dash =
            locomotor_set_swap_for_kind("AmericaJetRaptor", HostLocomotorSetKind::Supersonic)
                .expect("SET_SUPERSONIC");
        assert_eq!(taxi.locomotor_name, "RaptorTaxiingLocomotor");
        assert!((taxi.max_speed - 25.0).abs() < 0.05);
        assert!(
            (taxi.max_speed - cruise.max_speed).abs() > 1.0,
            "taxi must be slower than cruise"
        );
        assert!(
            (dash.max_speed - cruise.max_speed).abs() > 1.0,
            "supersonic must be faster than cruise"
        );
        assert_eq!(
            locomotor_set_kind_token(HostLocomotorSetKind::Taxiing),
            "SET_TAXIING"
        );
        assert_eq!(
            locomotor_set_kind_token(HostLocomotorSetKind::Supersonic),
            "SET_SUPERSONIC"
        );
        assert_eq!(
            locomotor_set_kind_token(HostLocomotorSetKind::Sluggish),
            "SET_SLUGGISH"
        );

        let mut t = ThingTemplate::new("AmericaJetRaptor");
        t.add_kind_of(KindOf::Aircraft);
        let mut jet = Object::new(t, ObjectId(3), Team::USA);
        jet.movement.max_speed = 175.0;
        assert!(apply_locomotor_set_kind(
            &mut jet,
            HostLocomotorSetKind::Taxiing
        ));
        assert_eq!(jet.jet_ai.cur_locomotor_set.as_deref(), Some("SET_TAXIING"));
        assert!((jet.movement.max_speed - 25.0).abs() < 0.05);
        assert!(apply_locomotor_set_kind(
            &mut jet,
            HostLocomotorSetKind::Supersonic
        ));
        assert_eq!(
            jet.jet_ai.cur_locomotor_set.as_deref(),
            Some("SET_SUPERSONIC")
        );
        assert!(jet.movement.max_speed > 200.0);

        let sluggish =
            locomotor_set_swap_for_kind("GLAVehicleCombatBike", HostLocomotorSetKind::Sluggish)
                .expect("SET_SLUGGISH");
        assert_eq!(
            sluggish.locomotor_name,
            "CombatBikeTerroristGroundLocomotor"
        );
        // INIZH Locomotor.ini CombatBikeTerroristGroundLocomotor Speed=90
        // (";25% slower" than the SET_NORMAL 120 ground member).
        assert!((sluggish.max_speed - 90.0).abs() < 0.05);
    }
}
