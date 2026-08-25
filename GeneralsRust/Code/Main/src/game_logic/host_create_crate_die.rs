//! Host CreateCrateDie residual (onDie crate spawn).
//!
//! C++ CreateCrateDie.cpp residual:
//! - For each CrateData name on the dying template, look up a crate template
//! - testCreationChance via GameLogicRandomValueReal
//! - Weighted possible-crate pick
//! - Spawn money crate object + register in HostMoneyCrateRegistry
//! - notifyCrate for computer killers
//!
//! Live path now applies CreationChance / VeterancyLevel / KilledByType /
//! KillerScience / OwnedByMaker from parsed Crate.ini when present.

use super::host_gamedata_lobby_residual::{
    ELITE_TANK_CRATE_CREATION_CHANCE_RESIDUAL, HEROIC_TANK_CRATE_CREATION_CHANCE_RESIDUAL,
    SALVAGE_CREATION_CHANCE_RESIDUAL, SALVAGE_MAX_MONEY_RESIDUAL, SALVAGE_MIN_MONEY_RESIDUAL,
    dollar_crate_money_residual,
};

use super::ObjectId;
use super::host_money_crate::{
    DOLLAR_CRATE_1000_MONEY, DOLLAR_CRATE_1000_OBJECT, DOLLAR_CRATE_2500_MONEY,
    DOLLAR_CRATE_2500_OBJECT, SUPPLY_DROP_CRATE_MONEY_PROVIDED, SUPPLY_DROP_ZONE_CRATE_OBJECT,
};
use super::host_rng_residual::pure_logic_random_real;

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
    pub is_shroud_crate: bool,
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
    is_shroud_crate: false,
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
    is_shroud_crate: false,
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
    is_shroud_crate: false,
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
    is_shroud_crate: false,
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
    is_shroud_crate: false,
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
    is_shroud_crate: false,
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
    is_shroud_crate: false,
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
    is_shroud_crate: false,
}];

static SHROUD_CRATE_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: "ShroudCrate",
    crate_chance: 1.0,
    money_provided: 0,
    building_pickup: false,
    is_veterancy: false,
    veterancy_effect_range: 0.0,
    veterancy_levels: 1,
    is_unit_crate: false,
    unit_crate_type: "",
    unit_crate_count: 0,
    is_heal_crate: false,
    is_shroud_crate: true,
}];

static ELITE_TANK_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: "EliteTankCrate",
    crate_chance: 1.0,
    money_provided: 0,
    building_pickup: false,
    is_veterancy: false,
    veterancy_effect_range: 0.0,
    veterancy_levels: 1,
    is_unit_crate: false,
    unit_crate_type: "",
    unit_crate_count: 0,
    is_heal_crate: false,
    is_shroud_crate: false,
}];

static HEROIC_TANK_POSSIBLE: &[HostCrateCreationEntry] = &[HostCrateCreationEntry {
    crate_object_name: "HeroicTankCrate",
    crate_chance: 1.0,
    money_provided: 0,
    building_pickup: false,
    is_veterancy: false,
    veterancy_effect_range: 0.0,
    veterancy_levels: 1,
    is_unit_crate: false,
    unit_crate_type: "",
    unit_crate_count: 0,
    is_heal_crate: false,
    is_shroud_crate: false,
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
    HostCrateTemplate {
        name: "ShroudCrateData",
        creation_chance: 1.0,
        possible: SHROUD_CRATE_POSSIBLE,
    },
    HostCrateTemplate {
        name: "EliteTankCrateData",
        creation_chance: ELITE_TANK_CRATE_CREATION_CHANCE_RESIDUAL,
        possible: ELITE_TANK_POSSIBLE,
    },
    HostCrateTemplate {
        name: "HeroicTankCrateData",
        creation_chance: HEROIC_TANK_CRATE_CREATION_CHANCE_RESIDUAL,
        possible: HEROIC_TANK_POSSIBLE,
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
    pub is_shroud_crate: bool,
    pub owned_by_maker: bool,
}

/// Gates matching C++ CreateCrateDie::testVeterancyLevel / testKillerType / testKillerScience.
pub struct CrateDieGates<'a> {
    pub victim_veterancy: Option<&'a str>,
    pub killer_kindof_names: &'a [&'a str],
    pub killer_sciences: &'a [String],
}

pub fn try_roll_crate_spawn(
    crate_data_name: &str,
    seed: u32,
    draw_base: u32,
) -> Option<HostCrateSpawnRequest> {
    try_roll_crate_spawn_gated(crate_data_name, seed, draw_base, None)
}

pub fn try_roll_crate_spawn_gated(
    crate_data_name: &str,
    seed: u32,
    draw_base: u32,
    gates: Option<&CrateDieGates<'_>>,
) -> Option<HostCrateSpawnRequest> {
    let tmpl = lookup_crate_template(crate_data_name)?;
    let chance_roll = pure_logic_random_real(seed, draw_base, 0.0, 1.0);
    let pick_roll = pure_logic_random_real(seed, draw_base.wrapping_add(1), 0.0, 1.0);
    let victim_veterancy = gates
        .and_then(|g| g.victim_veterancy)
        .map(gamelogic::object::crate_system::veterancy_level_from_ini_name)
        .unwrap_or_else(|| {
            gamelogic::object::crate_system::veterancy_level_from_ini_name("Regular")
        });

    let killer_kindof = match gates {
        Some(g) if !g.killer_kindof_names.is_empty() => {
            Some(host_kind_names_to_common_mask(g.killer_kindof_names))
        }
        _ => None,
    };
    let science_names: Vec<&str> = gates
        .map(|g| g.killer_sciences.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let eval = gamelogic::object::crate_system::CrateDieEval {
        chance_roll,
        pick_roll,
        victim_veterancy,
        killer_kindof,
        killer_has_science: false,
        killer_sciences: &science_names,
    };
    let pick = tmpl.evaluate_on_die(&eval)?;
    let mut req = spawn_request_from_object_name(&pick.crate_object_name, seed, draw_base);
    req.owned_by_maker = pick.is_owned_by_maker;
    Some(req)
}

fn lookup_crate_template(name: &str) -> Option<gamelogic::object::crate_system::CrateTemplate> {
    if let Ok(guard) = gamelogic::object::crate_system::get_crate_system().read() {
        if let Some(tmpl) = guard.find_crate_template_ci(name) {
            return Some(tmpl.clone());
        }
    }
    if let Some(parsed) = find_parsed_crate_template(name) {
        return Some(gamelogic::object::crate_system::CrateSystem::template_from_parsed(&parsed));
    }
    find_host_crate_template(name).map(residual_as_crate_template)
}

fn residual_as_crate_template(
    host: &HostCrateTemplate,
) -> gamelogic::object::crate_system::CrateTemplate {
    let mut tmpl = gamelogic::object::crate_system::CrateTemplate::new(host.name.to_string());
    tmpl.creation_chance = host.creation_chance;
    for entry in host.possible {
        tmpl.add_possible_crate(entry.crate_object_name.to_string(), entry.crate_chance);
    }
    if host.name.eq_ignore_ascii_case("SalvageCrateData")
        || host.name.eq_ignore_ascii_case("SalvageCrate")
    {
        tmpl.killed_by_type_kindof = salvage_killed_by_mask();
    }
    if host.name.eq_ignore_ascii_case("EliteTankCrateData") {
        tmpl.veterancy_level =
            Some(gamelogic::object::crate_system::veterancy_level_from_ini_name("Elite"));
        tmpl.creation_chance = ELITE_TANK_CRATE_CREATION_CHANCE_RESIDUAL;
    }
    if host.name.eq_ignore_ascii_case("HeroicTankCrateData") {
        tmpl.veterancy_level =
            Some(gamelogic::object::crate_system::veterancy_level_from_ini_name("Heroic"));
        tmpl.creation_chance = HEROIC_TANK_CRATE_CREATION_CHANCE_RESIDUAL;
    }
    tmpl
}

fn host_kind_names_to_common_mask(names: &[&str]) -> u64 {
    let bits = game_engine::common::system::kind_of::KIND_OF_BIT_NAMES;
    let mut mask = 0u64;
    for (index, bit_name) in bits.iter().enumerate() {
        if index >= 64 {
            break;
        }
        if names.iter().any(|have| have.eq_ignore_ascii_case(bit_name)) {
            mask |= 1u64 << index;
        }
    }
    mask
}

fn salvage_killed_by_mask() -> u64 {
    let names = game_engine::common::system::kind_of::KIND_OF_BIT_NAMES;
    names
        .iter()
        .position(|n| n.eq_ignore_ascii_case("SALVAGER"))
        .filter(|i| *i < 64)
        .map(|i| 1u64 << i)
        .unwrap_or(0)
}

fn find_parsed_crate_template(name: &str) -> Option<game_engine::common::ini::ParsedCrateTemplate> {
    let store = game_engine::common::ini::get_crate_system()?;
    let guard = store.read();
    if let Some(t) = guard.get(name) {
        return Some(t.clone());
    }
    guard
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(name))
        .cloned()
}

fn test_parsed_creation_chance(
    parsed: &game_engine::common::ini::ParsedCrateTemplate,
    seed: u32,
    draw: u32,
) -> bool {
    let roll = pure_logic_random_real(seed, draw, 0.0, 1.0);
    roll < parsed.creation_chance
}

fn spawn_request_from_object_name(
    object_name: &str,
    seed: u32,
    draw_base: u32,
) -> HostCrateSpawnRequest {
    let lower = object_name.to_ascii_lowercase();
    let host = HOST_CRATE_TEMPLATES.iter().find_map(|t| {
        t.possible
            .iter()
            .find(|e| e.crate_object_name.eq_ignore_ascii_case(object_name))
    });
    if let Some(entry) = host {
        return HostCrateSpawnRequest {
            object_name: entry.crate_object_name.to_string(),
            money_provided: money_for_entry(entry, seed, draw_base.wrapping_add(2)),
            building_pickup: entry.building_pickup,
            is_veterancy: entry.is_veterancy,
            veterancy_effect_range: entry.veterancy_effect_range,
            veterancy_levels: entry.veterancy_levels,
            is_unit_crate: entry.is_unit_crate,
            unit_crate_type: entry.unit_crate_type.to_string(),
            unit_crate_count: entry.unit_crate_count,
            is_heal_crate: entry.is_heal_crate,
            is_shroud_crate: entry.is_shroud_crate,
            owned_by_maker: false,
        };
    }
    HostCrateSpawnRequest {
        object_name: object_name.to_string(),
        money_provided: crate::game_logic::host_money_crate::money_provided_for_crate_object(
            object_name,
        )
        .unwrap_or(0),
        building_pickup: crate::game_logic::host_money_crate::building_pickup_for_crate_object(
            object_name,
        ),
        is_veterancy: lower.contains("levelup") || lower.contains("veteran"),
        veterancy_effect_range: if lower.contains("medium") {
            250.0
        } else if lower.contains("levelup") {
            100.0
        } else {
            0.0
        },
        veterancy_levels: 1,
        is_unit_crate: lower.contains("crusader")
            || lower.contains("unitcrate")
            || lower.contains("free"),
        unit_crate_type: if lower.contains("crusader") {
            "AmericaTankCrusader".to_string()
        } else {
            String::new()
        },
        unit_crate_count: if lower.contains("crusader") { 2 } else { 0 },
        is_heal_crate: lower.contains("heal"),
        is_shroud_crate: lower.contains("shroud"),
        owned_by_maker: false,
    }
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
    fn roll_spawn_salvage_requires_salvager_killer() {
        // C++ CreateCrateDie.cpp:72-73 — KilledByType SALVAGER.
        assert!(
            try_roll_crate_spawn("SalvageCrateData", 42, 0).is_none(),
            "no killer must fail SalvageCrateData"
        );
        let infantry = CrateDieGates {
            victim_veterancy: Some("Regular"),
            killer_kindof_names: &["Infantry"],
            killer_sciences: &[],
        };
        assert!(
            try_roll_crate_spawn_gated("SalvageCrateData", 42, 0, Some(&infantry)).is_none(),
            "non-salvager must not drop salvage crate"
        );
        let salvager = CrateDieGates {
            victim_veterancy: Some("Regular"),
            killer_kindof_names: &["Salvager"],
            killer_sciences: &[],
        };
        let req = try_roll_crate_spawn_gated("SalvageCrateData", 42, 0, Some(&salvager))
            .expect("salvager spawn");
        assert_eq!(req.object_name, "SalvageCrate");
        assert!(req.money_provided >= 25 && req.money_provided <= 75);
    }

    #[test]
    fn elite_tank_crate_data_requires_elite_victim() {
        // Retail EliteTankCrateData: CreationChance 0.75, VeterancyLevel ELITE.
        let regular = CrateDieGates {
            victim_veterancy: Some("Regular"),
            killer_kindof_names: &["Vehicle"],
            killer_sciences: &[],
        };
        assert!(try_roll_crate_spawn_gated("EliteTankCrateData", 1, 0, Some(&regular)).is_none());
        let elite = CrateDieGates {
            victim_veterancy: Some("Elite"),
            killer_kindof_names: &["Vehicle"],
            killer_sciences: &[],
        };
        // Seed 1 / draw 0 is a mid roll; chance 0.75 should usually pass.
        let req = try_roll_crate_spawn_gated("EliteTankCrateData", 1, 0, Some(&elite));
        if let Some(req) = req {
            assert_eq!(req.object_name, "EliteTankCrate");
        }
        let heroic = CrateDieGates {
            victim_veterancy: Some("Heroic"),
            killer_kindof_names: &["Vehicle"],
            killer_sciences: &[],
        };
        let req = try_roll_crate_spawn_gated("HeroicTankCrateData", 1, 0, Some(&heroic))
            .expect("heroic tank crate");
        assert_eq!(req.object_name, "HeroicTankCrate");
    }

    #[test]
    fn owned_by_maker_comes_from_parsed_crate_data() {
        let mut parsed = game_engine::common::ini::ParsedCrateTemplate::new("OwnedCrate".into());
        parsed.creation_chance = 1.0;
        parsed.is_owned_by_maker = true;
        parsed
            .possible_crates
            .push(game_engine::common::ini::ParsedCrateCreationEntry {
                crate_name: "1000DollarCrate".into(),
                crate_chance: 1.0,
            });
        let tmpl = gamelogic::object::crate_system::CrateSystem::template_from_parsed(&parsed);
        let eval = gamelogic::object::crate_system::CrateDieEval {
            chance_roll: 0.0,
            pick_roll: 0.0,
            victim_veterancy: gamelogic::object::crate_system::veterancy_level_from_ini_name(
                "Regular",
            ),
            killer_kindof: None,
            killer_has_science: false,
            killer_sciences: &[],
        };
        let pick = tmpl.evaluate_on_die(&eval).expect("owned crate");
        assert!(pick.is_owned_by_maker);
        assert_eq!(pick.crate_object_name, "1000DollarCrate");
    }
}
