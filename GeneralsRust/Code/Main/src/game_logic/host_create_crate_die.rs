//! Host CreateCrateDie residual (onDie crate spawn).
//!
//! C++ CreateCrateDie.cpp residual:
//! - For each CrateData name on the dying template, look up a crate template
//! - testCreationChance via GameLogicRandomValueReal
//! - Weighted possible-crate pick
//! - Spawn money crate object + register in HostMoneyCrateRegistry
//! - notifyCrate for computer killers
//!
//! Fail-closed: not full KindOf multi / science / veterancy gates, not full
//! PartitionManager findPositionAround (spawns at death position + small offset).

use super::host_gamedata_lobby_residual::{
    dollar_crate_money_residual, SALVAGE_CREATION_CHANCE_RESIDUAL, SALVAGE_MAX_MONEY_RESIDUAL,
    SALVAGE_MIN_MONEY_RESIDUAL,
};
use super::host_money_crate::{
    DOLLAR_CRATE_1000_MONEY, DOLLAR_CRATE_1000_OBJECT, DOLLAR_CRATE_2500_MONEY,
    DOLLAR_CRATE_2500_OBJECT, SUPPLY_DROP_CRATE_MONEY_PROVIDED, SUPPLY_DROP_ZONE_CRATE_OBJECT,
};
use super::host_rng_residual::pure_logic_random_real;
use super::ObjectId;

/// One weighted entry in CrateTemplate::m_possibleCrates residual.
#[derive(Debug, Clone)]
pub struct HostCrateCreationEntry {
    pub crate_object_name: &'static str,
    pub crate_chance: f32,
    pub money_provided: u32,
    pub building_pickup: bool,
    pub is_veterancy: bool,
    pub veterancy_effect_range: f32,
    pub veterancy_levels: u8,
    pub is_unit_crate: bool,
    pub unit_crate_type: &'static str,
    pub unit_crate_count: u32,
    pub is_heal_crate: bool,
}

/// Host residual CrateTemplate subset.
#[derive(Debug, Clone)]
pub struct HostCrateTemplate {
    pub name: &'static str,
    pub creation_chance: f32,
    pub possible: &'static [HostCrateCreationEntry],
}

/// Retail SalvageCrateData residual → salvage money crate object.
static SALVAGE_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: "SalvageCrate",
    crate_chance: 1.0,
    money_provided: 50, // midpoint residual [25,75] default
    building_pickup: false,
    is_veterancy: false,
    veterancy_effect_range: 0.0,
    veterancy_levels: 1,
    is_unit_crate: false,
    unit_crate_type: "",
    unit_crate_count: 0,
    is_heal_crate: false,
}];

static DOLLAR_1000_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: DOLLAR_CRATE_1000_OBJECT,
    crate_chance: 1.0,
    money_provided: DOLLAR_CRATE_1000_MONEY,
    building_pickup: false,
    is_veterancy: false,
    veterancy_effect_range: 0.0,
    veterancy_levels: 1,
    is_unit_crate: false,
    unit_crate_type: "",
    unit_crate_count: 0,
    is_heal_crate: false,
}];

static DOLLAR_2500_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: DOLLAR_CRATE_2500_OBJECT,
    crate_chance: 1.0,
    money_provided: DOLLAR_CRATE_2500_MONEY,
    building_pickup: false,
    is_veterancy: false,
    veterancy_effect_range: 0.0,
    veterancy_levels: 1,
    is_unit_crate: false,
    unit_crate_type: "",
    unit_crate_count: 0,
    is_heal_crate: false,
}];

static SUPPLY_DROP_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: SUPPLY_DROP_ZONE_CRATE_OBJECT,
    crate_chance: 1.0,
    money_provided: SUPPLY_DROP_CRATE_MONEY_PROVIDED,
    building_pickup: true,
    is_veterancy: false,
    veterancy_effect_range: 0.0,
    veterancy_levels: 1,
    is_unit_crate: false,
    unit_crate_type: "",
    unit_crate_count: 0,
    is_heal_crate: false,
}];

static SMALL_LEVEL_UP_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: "SmallLevelUpCrate",
    crate_chance: 1.0,
    money_provided: 0,
    building_pickup: false,
    is_veterancy: true,
    veterancy_effect_range: 100.0,
    veterancy_levels: 1,
    is_unit_crate: false,
    unit_crate_type: "",
    unit_crate_count: 0,
    is_heal_crate: false,
}];

static MEDIUM_LEVEL_UP_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: "MediumLevelUpCrate",
    crate_chance: 1.0,
    money_provided: 0,
    building_pickup: false,
    is_veterancy: true,
    veterancy_effect_range: 250.0,
    veterancy_levels: 1,
    is_unit_crate: false,
    unit_crate_type: "",
    unit_crate_count: 0,
    is_heal_crate: false,
}];

static FREE_CRUSADERS_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: "2FreeCrusadersCrate",
    crate_chance: 1.0,
    money_provided: 0,
    building_pickup: false,
    is_veterancy: false,
    veterancy_effect_range: 0.0,
    veterancy_levels: 1,
    is_unit_crate: true,
    unit_crate_type: "AmericaTankCrusader",
    unit_crate_count: 2,
    is_heal_crate: false,
}];

static HEAL_CRATE_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: "HealCrate",
    crate_chance: 1.0,
    money_provided: 0,
    building_pickup: false,
    is_veterancy: false,
    veterancy_effect_range: 0.0,
    veterancy_levels: 1,
    is_unit_crate: false,
    unit_crate_type: "",
    unit_crate_count: 0,
    is_heal_crate: true,
}];

/// Built-in host crate templates (Crate.ini name residual keys).
pub static HOST_CRATE_TEMPLATES: &[HostCrateTemplate] = &[
    HostCrateTemplate {
        name: "SalvageCrateData",
        creation_chance: SALVAGE_CREATION_CHANCE_RESIDUAL,
        possible: SALVAGE_POSSIBLE,
    },
    HostCrateTemplate {
        name: "SalvageCrate",
        creation_chance: SALVAGE_CREATION_CHANCE_RESIDUAL,
        possible: SALVAGE_POSSIBLE,
    },
    HostCrateTemplate {
        name: "1000DollarCrateData",
        creation_chance: 1.0,
        possible: DOLLAR_1000_POSSIBLE,
    },
    HostCrateTemplate {
        name: "2500DollarCrateData",
        creation_chance: 1.0,
        possible: DOLLAR_2500_POSSIBLE,
    },
    HostCrateTemplate {
        name: "SupplyDropZoneCrateData",
        creation_chance: 1.0,
        possible: SUPPLY_DROP_POSSIBLE,
    },
    HostCrateTemplate {
        name: "SmallLevelUpCrateData",
        creation_chance: 1.0,
        possible: SMALL_LEVEL_UP_POSSIBLE,
    },
    HostCrateTemplate {
        name: "MediumLevelUpCrateData",
        creation_chance: 1.0,
        possible: MEDIUM_LEVEL_UP_POSSIBLE,
    },
    HostCrateTemplate {
        name: "2FreeCrusadersCrateData",
        creation_chance: 1.0,
        possible: FREE_CRUSADERS_POSSIBLE,
    },
    HostCrateTemplate {
        name: "HealCrateData",
        creation_chance: 1.0,
        possible: HEAL_CRATE_POSSIBLE,
    },
];

pub fn find_host_crate_template(name: &str) -> Option<&'static HostCrateTemplate> {
    HOST_CRATE_TEMPLATES
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(name))
}

/// C++ testCreationChance residual.
pub fn test_creation_chance(tmpl: &HostCrateTemplate, seed: u32, draw: u32) -> bool {
    let roll = pure_logic_random_real(seed, draw, 0.0, 1.0);
    roll < tmpl.creation_chance
}

/// C++ weighted possibleCrates pick residual.
pub fn pick_possible_crate(
    tmpl: &HostCrateTemplate,
    seed: u32,
    draw: u32,
) -> Option<&'static HostCrateCreationEntry> {
    if tmpl.possible.is_empty() {
        return None;
    }
    let pick = pure_logic_random_real(seed, draw, 0.0, 1.0);
    let mut running = 0.0f32;
    for entry in tmpl.possible {
        running += entry.crate_chance;
        if running > pick {
            return Some(entry);
        }
    }
    // Designer sum < 1 fail-closed: last entry if any chance mass.
    tmpl.possible.last()
}

/// Salvage money roll residual [Min, Max] for SalvageCrate object.
pub fn salvage_money_roll(seed: u32, draw: u32) -> u32 {
    let lo = SALVAGE_MIN_MONEY_RESIDUAL as f32;
    let hi = SALVAGE_MAX_MONEY_RESIDUAL as f32;
    let v = pure_logic_random_real(seed, draw, lo, hi);
    v.round().clamp(lo, hi) as u32
}

/// Resolve money for a picked crate entry (salvage rolls; dollar uses fixed).
pub fn money_for_entry(entry: &HostCrateCreationEntry, seed: u32, draw: u32) -> u32 {
    if entry.crate_object_name.eq_ignore_ascii_case("SalvageCrate") {
        return salvage_money_roll(seed, draw);
    }
    if let Some(m) = dollar_crate_money_residual(entry.crate_object_name) {
        return m;
    }
    entry.money_provided
}

/// Spawn decision residual for one CrateData name.
pub struct HostCrateSpawnRequest {
    pub object_name: String,
    pub money_provided: u32,
    pub building_pickup: bool,
    pub is_veterancy: bool,
    pub veterancy_effect_range: f32,
    pub veterancy_levels: u8,
    pub is_unit_crate: bool,
    pub unit_crate_type: String,
    pub unit_crate_count: u32,
    pub is_heal_crate: bool,
}

pub fn try_roll_crate_spawn(
    crate_data_name: &str,
    seed: u32,
    draw_base: u32,
) -> Option<HostCrateSpawnRequest> {
    let tmpl = find_host_crate_template(crate_data_name)?;
    if !test_creation_chance(tmpl, seed, draw_base) {
        return None;
    }
    let entry = pick_possible_crate(tmpl, seed, draw_base.wrapping_add(1))?;
    let money = money_for_entry(entry, seed, draw_base.wrapping_add(2));
    Some(HostCrateSpawnRequest {
        object_name: entry.crate_object_name.to_string(),
        money_provided: money,
        building_pickup: entry.building_pickup,
        is_veterancy: entry.is_veterancy,
        veterancy_effect_range: entry.veterancy_effect_range,
        veterancy_levels: entry.veterancy_levels,
        is_unit_crate: entry.is_unit_crate,
        unit_crate_type: entry.unit_crate_type.to_string(),
        unit_crate_count: entry.unit_crate_count,
        is_heal_crate: entry.is_heal_crate,
    })
}

/// Seed helper from victim/killer ids + frame.
pub fn crate_die_seed(victim: ObjectId, killer: Option<ObjectId>, frame: u32) -> u32 {
    victim
        .0
        .wrapping_mul(2654435761)
        .wrapping_add(killer.map(|k| k.0).unwrap_or(0).wrapping_mul(40503))
        .wrapping_add(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn salvage_template_always_passes_chance() {
        let t = find_host_crate_template("SalvageCrateData").unwrap();
        assert!((t.creation_chance - 1.0).abs() < f32::EPSILON);
        assert!(test_creation_chance(t, 1, 0));
    }

    #[test]
    fn salvage_money_in_retail_range() {
        for s in 0..20u32 {
            let m = salvage_money_roll(s, 3);
            assert!(m >= SALVAGE_MIN_MONEY_RESIDUAL && m <= SALVAGE_MAX_MONEY_RESIDUAL);
        }
    }

    #[test]
    fn roll_spawn_salvage() {
        let req = try_roll_crate_spawn("SalvageCrateData", 42, 0).expect("spawn");
        assert_eq!(req.object_name, "SalvageCrate");
        assert!(req.money_provided >= 25 && req.money_provided <= 75);
    }
}
