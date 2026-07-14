//! Wave 85 residual peels: faction side table / player template / starting cash /
//! skirmish AI personality (SideInfo) / victory condition residual.
//!
//! Orthogonal to Waves 82–83 enum/structure-economy residual. Host-testable packs
//! for skirmish lobby + multiplayer setup residual honesty.
//!
//! Sources (retail ZH INI + C++):
//! - PlayerTemplate.ini — FactionAmerica/China/GLA + ZH generals + Civilian/Observer/Boss
//! - multiplayer.ini MultiplayerStartingMoneyChoice 5000/10000(default)/20000/50000
//! - GameData.ini DefaultStartingCash **10000**; HumanSoloPlayerHealthBonus Easy/Normal/Hard
//! - AIData.ini SideInfo ResourceGatherers / BaseDefenseStructure1 / SkillSet residual
//! - VictoryConditions.h VictoryType NOBUILDINGS=1 / NOUNITS=2; default both set
//! - GameCommon.h MAX_PLAYER_COUNT **16**
//!
//! Fail-closed:
//! - Not full PlayerTemplateStore INI parse / ControlBarScheme side binding
//! - Not full MultiplayerSettings live lobby combo wiring
//! - Not full AIPlayer SideInfo skill-set purchase / SkirmishBuildList path
//! - Not full VictoryConditions multiplayer killPlayer / alliance reveal residual
//! - Shell `playable_claim` stays false; network deferred

use crate::ai::{AIDifficulty, AIPersonality};
use crate::game_logic::game_logic::Player;
use crate::game_logic::victory_conditions::VictoryType;
use crate::game_logic::Team;

// ---------------------------------------------------------------------------
// 1. Faction side residual table (base + ZH generals)
// ---------------------------------------------------------------------------

/// C++ GameCommon.h MAX_PLAYER_COUNT residual.
pub const MAX_PLAYER_COUNT_RESIDUAL: usize = 16;

/// Number of PlayerTemplate entries in retail PlayerTemplate.ini residual.
pub const PLAYER_TEMPLATE_COUNT_RESIDUAL: usize = 15;
/// Playable sides residual (excludes Civilian + Observer).
pub const PLAYABLE_FACTION_COUNT_RESIDUAL: usize = 13;
/// Original Generals (OldFaction=Yes) playable residual count: America/China/GLA.
pub const OLD_PLAYABLE_FACTION_COUNT_RESIDUAL: usize = 3;
/// ZH general + Boss residual playable count (OldFaction=No).
pub const ZH_PLAYABLE_FACTION_COUNT_RESIDUAL: usize = 10;

/// BaseSide residual strings used by PlayerTemplate (USA/China/GLA).
pub const BASE_SIDE_USA: &str = "USA";
pub const BASE_SIDE_CHINA: &str = "China";
pub const BASE_SIDE_GLA: &str = "GLA";

/// Display Side residual for base factions (PlayerTemplate Side=).
pub const SIDE_AMERICA: &str = "America";
pub const SIDE_CHINA: &str = "China";
pub const SIDE_GLA: &str = "GLA";
pub const SIDE_CIVILIAN: &str = "Civilian";
pub const SIDE_OBSERVER: &str = "Observer";
pub const SIDE_BOSS: &str = "Boss";

/// ZH general Side residual names (PlayerTemplate Side=).
pub const SIDE_AMERICA_SUPERWEAPON: &str = "AmericaSuperWeaponGeneral";
pub const SIDE_AMERICA_LASER: &str = "AmericaLaserGeneral";
pub const SIDE_AMERICA_AIRFORCE: &str = "AmericaAirForceGeneral";
pub const SIDE_CHINA_TANK: &str = "ChinaTankGeneral";
pub const SIDE_CHINA_INFANTRY: &str = "ChinaInfantryGeneral";
pub const SIDE_CHINA_NUKE: &str = "ChinaNukeGeneral";
pub const SIDE_GLA_TOXIN: &str = "GLAToxinGeneral";
pub const SIDE_GLA_DEMOLITION: &str = "GLADemolitionGeneral";
pub const SIDE_GLA_STEALTH: &str = "GLAStealthGeneral";

/// Ordered residual faction side table: (template name, side, base_side, playable, old_faction).
/// Order matches retail PlayerTemplate.ini declaration order.
pub const FACTION_SIDE_RESIDUAL_TABLE: &[(&str, &str, &str, bool, bool)] = &[
    ("FactionCivilian", SIDE_CIVILIAN, SIDE_CIVILIAN, false, true),
    ("FactionObserver", SIDE_OBSERVER, SIDE_OBSERVER, false, true),
    ("FactionAmerica", SIDE_AMERICA, BASE_SIDE_USA, true, true),
    ("FactionChina", SIDE_CHINA, BASE_SIDE_CHINA, true, true),
    ("FactionGLA", SIDE_GLA, BASE_SIDE_GLA, true, true),
    (
        "FactionAmericaSuperWeaponGeneral",
        SIDE_AMERICA_SUPERWEAPON,
        BASE_SIDE_USA,
        true,
        false,
    ),
    (
        "FactionAmericaLaserGeneral",
        SIDE_AMERICA_LASER,
        BASE_SIDE_USA,
        true,
        false,
    ),
    (
        "FactionAmericaAirForceGeneral",
        SIDE_AMERICA_AIRFORCE,
        BASE_SIDE_USA,
        true,
        false,
    ),
    (
        "FactionChinaTankGeneral",
        SIDE_CHINA_TANK,
        BASE_SIDE_CHINA,
        true,
        false,
    ),
    (
        "FactionChinaInfantryGeneral",
        SIDE_CHINA_INFANTRY,
        BASE_SIDE_CHINA,
        true,
        false,
    ),
    (
        "FactionChinaNukeGeneral",
        SIDE_CHINA_NUKE,
        BASE_SIDE_CHINA,
        true,
        false,
    ),
    (
        "FactionGLAToxinGeneral",
        SIDE_GLA_TOXIN,
        BASE_SIDE_GLA,
        true,
        false,
    ),
    (
        "FactionGLADemolitionGeneral",
        SIDE_GLA_DEMOLITION,
        BASE_SIDE_GLA,
        true,
        false,
    ),
    (
        "FactionGLAStealthGeneral",
        SIDE_GLA_STEALTH,
        BASE_SIDE_GLA,
        true,
        false,
    ),
    (
        "FactionBossGeneral",
        SIDE_BOSS,
        BASE_SIDE_CHINA,
        true,
        false,
    ),
];

/// Preferred RGB residual from PlayerTemplate (R,G,B).
pub const AMERICA_PREFERRED_COLOR_RGB: (u8, u8, u8) = (0, 0, 255);
pub const CHINA_PREFERRED_COLOR_RGB: (u8, u8, u8) = (255, 0, 0);
pub const GLA_PREFERRED_COLOR_RGB: (u8, u8, u8) = (0, 255, 0);
pub const CIVILIAN_PREFERRED_COLOR_RGB: (u8, u8, u8) = (255, 255, 255);

/// Pack residual RGB as 0x00RRGGBB (host color residual).
pub fn pack_preferred_color_rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Map display Side residual → base side residual.
pub fn base_side_for_display_side(side: &str) -> Option<&'static str> {
    for &(_, s, base, _, _) in FACTION_SIDE_RESIDUAL_TABLE {
        if s == side {
            return Some(base);
        }
    }
    None
}

/// Whether a Side residual is an OldFaction (original Generals).
pub fn is_old_faction_side(side: &str) -> bool {
    FACTION_SIDE_RESIDUAL_TABLE
        .iter()
        .find(|&&(_, s, _, _, _)| s == side)
        .map(|&(_, _, _, _, old)| old)
        .unwrap_or(false)
}

/// Wave 85 honesty: faction side residual table.
pub fn honesty_faction_side_residual_table_wave85() -> bool {
    FACTION_SIDE_RESIDUAL_TABLE.len() == PLAYER_TEMPLATE_COUNT_RESIDUAL
        && FACTION_SIDE_RESIDUAL_TABLE
            .iter()
            .filter(|&&(_, _, _, playable, _)| playable)
            .count()
            == PLAYABLE_FACTION_COUNT_RESIDUAL
        && FACTION_SIDE_RESIDUAL_TABLE
            .iter()
            .filter(|&&(_, _, _, playable, old)| playable && old)
            .count()
            == OLD_PLAYABLE_FACTION_COUNT_RESIDUAL
        && FACTION_SIDE_RESIDUAL_TABLE
            .iter()
            .filter(|&&(_, _, _, playable, old)| playable && !old)
            .count()
            == ZH_PLAYABLE_FACTION_COUNT_RESIDUAL
        && OLD_PLAYABLE_FACTION_COUNT_RESIDUAL + ZH_PLAYABLE_FACTION_COUNT_RESIDUAL
            == PLAYABLE_FACTION_COUNT_RESIDUAL
        && base_side_for_display_side(SIDE_AMERICA) == Some(BASE_SIDE_USA)
        && base_side_for_display_side(SIDE_CHINA) == Some(BASE_SIDE_CHINA)
        && base_side_for_display_side(SIDE_GLA) == Some(BASE_SIDE_GLA)
        && base_side_for_display_side(SIDE_AMERICA_AIRFORCE) == Some(BASE_SIDE_USA)
        && base_side_for_display_side(SIDE_CHINA_NUKE) == Some(BASE_SIDE_CHINA)
        && base_side_for_display_side(SIDE_GLA_STEALTH) == Some(BASE_SIDE_GLA)
        && base_side_for_display_side(SIDE_BOSS) == Some(BASE_SIDE_CHINA)
        && is_old_faction_side(SIDE_AMERICA)
        && is_old_faction_side(SIDE_CHINA)
        && is_old_faction_side(SIDE_GLA)
        && !is_old_faction_side(SIDE_AMERICA_SUPERWEAPON)
        && !is_old_faction_side(SIDE_BOSS)
        && pack_preferred_color_rgb(
            AMERICA_PREFERRED_COLOR_RGB.0,
            AMERICA_PREFERRED_COLOR_RGB.1,
            AMERICA_PREFERRED_COLOR_RGB.2,
        ) == 0x0000_00FF
        && pack_preferred_color_rgb(
            CHINA_PREFERRED_COLOR_RGB.0,
            CHINA_PREFERRED_COLOR_RGB.1,
            CHINA_PREFERRED_COLOR_RGB.2,
        ) == 0x00FF_0000
        && pack_preferred_color_rgb(
            GLA_PREFERRED_COLOR_RGB.0,
            GLA_PREFERRED_COLOR_RGB.1,
            GLA_PREFERRED_COLOR_RGB.2,
        ) == 0x0000_FF00
        && MAX_PLAYER_COUNT_RESIDUAL == 16
}

// ---------------------------------------------------------------------------
// 2. Player template residual peels (starting building / unit / science / shortcut)
// ---------------------------------------------------------------------------

/// PlayerTemplate residual seed: name, side, base, starting building, starting unit0,
/// intrinsic science, shortcut button count, playable, old_faction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerTemplateResidual {
    pub template_name: &'static str,
    pub side: &'static str,
    pub base_side: &'static str,
    pub starting_building: &'static str,
    pub starting_unit0: &'static str,
    pub intrinsic_science: &'static str,
    pub shortcut_button_count: i32,
    pub playable: bool,
    pub old_faction: bool,
}

/// Retail PlayerTemplate residual seeds for host skirmish lobby honesty.
pub const PLAYER_TEMPLATE_RESIDUAL_SEEDS: &[PlayerTemplateResidual] = &[
    PlayerTemplateResidual {
        template_name: "FactionCivilian",
        side: SIDE_CIVILIAN,
        base_side: SIDE_CIVILIAN,
        starting_building: "",
        starting_unit0: "",
        intrinsic_science: "None",
        shortcut_button_count: 0,
        playable: false,
        old_faction: true,
    },
    PlayerTemplateResidual {
        template_name: "FactionObserver",
        side: SIDE_OBSERVER,
        base_side: SIDE_OBSERVER,
        starting_building: "",
        starting_unit0: "",
        intrinsic_science: "None",
        shortcut_button_count: 0,
        playable: false,
        old_faction: true,
    },
    PlayerTemplateResidual {
        template_name: "FactionAmerica",
        side: SIDE_AMERICA,
        base_side: BASE_SIDE_USA,
        starting_building: "AmericaCommandCenter",
        starting_unit0: "AmericaVehicleDozer",
        intrinsic_science: "SCIENCE_AMERICA",
        shortcut_button_count: 10,
        playable: true,
        old_faction: true,
    },
    PlayerTemplateResidual {
        template_name: "FactionChina",
        side: SIDE_CHINA,
        base_side: BASE_SIDE_CHINA,
        starting_building: "ChinaCommandCenter",
        starting_unit0: "ChinaVehicleDozer",
        intrinsic_science: "SCIENCE_CHINA",
        shortcut_button_count: 10,
        playable: true,
        old_faction: true,
    },
    PlayerTemplateResidual {
        template_name: "FactionGLA",
        side: SIDE_GLA,
        base_side: BASE_SIDE_GLA,
        starting_building: "GLACommandCenter",
        starting_unit0: "GLAInfantryWorker",
        intrinsic_science: "SCIENCE_GLA",
        shortcut_button_count: 10,
        playable: true,
        old_faction: true,
    },
    PlayerTemplateResidual {
        template_name: "FactionAmericaSuperWeaponGeneral",
        side: SIDE_AMERICA_SUPERWEAPON,
        base_side: BASE_SIDE_USA,
        starting_building: "SupW_AmericaCommandCenter",
        starting_unit0: "SupW_AmericaVehicleDozer",
        intrinsic_science: "SCIENCE_AMERICA",
        shortcut_button_count: 11,
        playable: true,
        old_faction: false,
    },
    PlayerTemplateResidual {
        template_name: "FactionAmericaLaserGeneral",
        side: SIDE_AMERICA_LASER,
        base_side: BASE_SIDE_USA,
        starting_building: "Lazr_AmericaCommandCenter",
        starting_unit0: "Lazr_AmericaVehicleDozer",
        intrinsic_science: "SCIENCE_AMERICA",
        shortcut_button_count: 10,
        playable: true,
        old_faction: false,
    },
    PlayerTemplateResidual {
        template_name: "FactionAmericaAirForceGeneral",
        side: SIDE_AMERICA_AIRFORCE,
        base_side: BASE_SIDE_USA,
        starting_building: "AirF_AmericaCommandCenter",
        starting_unit0: "AirF_AmericaVehicleDozer",
        intrinsic_science: "SCIENCE_AMERICA",
        shortcut_button_count: 11,
        playable: true,
        old_faction: false,
    },
    PlayerTemplateResidual {
        template_name: "FactionChinaTankGeneral",
        side: SIDE_CHINA_TANK,
        base_side: BASE_SIDE_CHINA,
        starting_building: "Tank_ChinaCommandCenter",
        starting_unit0: "Tank_ChinaVehicleDozer",
        intrinsic_science: "SCIENCE_CHINA",
        shortcut_button_count: 10,
        playable: true,
        old_faction: false,
    },
    PlayerTemplateResidual {
        template_name: "FactionChinaInfantryGeneral",
        side: SIDE_CHINA_INFANTRY,
        base_side: BASE_SIDE_CHINA,
        starting_building: "Infa_ChinaCommandCenter",
        starting_unit0: "Infa_ChinaVehicleDozer",
        intrinsic_science: "SCIENCE_CHINA",
        shortcut_button_count: 10,
        playable: true,
        old_faction: false,
    },
    PlayerTemplateResidual {
        template_name: "FactionChinaNukeGeneral",
        side: SIDE_CHINA_NUKE,
        base_side: BASE_SIDE_CHINA,
        starting_building: "Nuke_ChinaCommandCenter",
        starting_unit0: "Nuke_ChinaVehicleDozer",
        intrinsic_science: "SCIENCE_CHINA",
        shortcut_button_count: 10,
        playable: true,
        old_faction: false,
    },
    PlayerTemplateResidual {
        template_name: "FactionGLAToxinGeneral",
        side: SIDE_GLA_TOXIN,
        base_side: BASE_SIDE_GLA,
        starting_building: "Chem_GLACommandCenter",
        starting_unit0: "Chem_GLAInfantryWorker",
        intrinsic_science: "SCIENCE_GLA",
        shortcut_button_count: 10,
        playable: true,
        old_faction: false,
    },
    PlayerTemplateResidual {
        template_name: "FactionGLADemolitionGeneral",
        side: SIDE_GLA_DEMOLITION,
        base_side: BASE_SIDE_GLA,
        starting_building: "Demo_GLACommandCenter",
        starting_unit0: "Demo_GLAInfantryWorker",
        intrinsic_science: "SCIENCE_GLA",
        shortcut_button_count: 10,
        playable: true,
        old_faction: false,
    },
    PlayerTemplateResidual {
        template_name: "FactionGLAStealthGeneral",
        side: SIDE_GLA_STEALTH,
        base_side: BASE_SIDE_GLA,
        starting_building: "Slth_GLACommandCenter",
        starting_unit0: "Slth_GLAInfantryWorker",
        intrinsic_science: "SCIENCE_GLA",
        shortcut_button_count: 10,
        playable: true,
        old_faction: false,
    },
    PlayerTemplateResidual {
        template_name: "FactionBossGeneral",
        side: SIDE_BOSS,
        base_side: BASE_SIDE_CHINA,
        starting_building: "Boss_CommandCenter",
        starting_unit0: "Boss_VehicleDozer",
        intrinsic_science: "SCIENCE_CHINA",
        shortcut_button_count: 9,
        playable: true,
        old_faction: false,
    },
];

/// Lookup residual player template by template name.
pub fn find_player_template_residual(name: &str) -> Option<&'static PlayerTemplateResidual> {
    PLAYER_TEMPLATE_RESIDUAL_SEEDS
        .iter()
        .find(|t| t.template_name == name)
}

/// Lookup residual player template by Side string.
pub fn find_player_template_by_side(side: &str) -> Option<&'static PlayerTemplateResidual> {
    PLAYER_TEMPLATE_RESIDUAL_SEEDS
        .iter()
        .find(|t| t.side == side)
}

/// Wave 85 honesty: player template residual peels.
pub fn honesty_player_template_residual_pack_wave85() -> bool {
    PLAYER_TEMPLATE_RESIDUAL_SEEDS.len() == PLAYER_TEMPLATE_COUNT_RESIDUAL
        && FACTION_SIDE_RESIDUAL_TABLE.len() == PLAYER_TEMPLATE_RESIDUAL_SEEDS.len()
        && PLAYER_TEMPLATE_RESIDUAL_SEEDS
            .iter()
            .zip(FACTION_SIDE_RESIDUAL_TABLE.iter())
            .all(|(pt, &(name, side, base, playable, old))| {
                pt.template_name == name
                    && pt.side == side
                    && pt.base_side == base
                    && pt.playable == playable
                    && pt.old_faction == old
            })
        && find_player_template_residual("FactionAmerica")
            .map(|t| {
                t.starting_building == "AmericaCommandCenter"
                    && t.starting_unit0 == "AmericaVehicleDozer"
                    && t.intrinsic_science == "SCIENCE_AMERICA"
                    && t.shortcut_button_count == 10
            })
            .unwrap_or(false)
        && find_player_template_residual("FactionChina")
            .map(|t| {
                t.starting_building == "ChinaCommandCenter"
                    && t.starting_unit0 == "ChinaVehicleDozer"
                    && t.intrinsic_science == "SCIENCE_CHINA"
            })
            .unwrap_or(false)
        && find_player_template_residual("FactionGLA")
            .map(|t| {
                t.starting_building == "GLACommandCenter"
                    && t.starting_unit0 == "GLAInfantryWorker"
                    && t.intrinsic_science == "SCIENCE_GLA"
            })
            .unwrap_or(false)
        // Superweapon / AirForce residual shortcut button count **11**.
        && find_player_template_residual("FactionAmericaSuperWeaponGeneral")
            .map(|t| t.shortcut_button_count == 11)
            .unwrap_or(false)
        && find_player_template_residual("FactionAmericaAirForceGeneral")
            .map(|t| t.shortcut_button_count == 11)
            .unwrap_or(false)
        // Boss residual shortcut button count **9**.
        && find_player_template_residual("FactionBossGeneral")
            .map(|t| t.shortcut_button_count == 9 && t.base_side == BASE_SIDE_CHINA)
            .unwrap_or(false)
        // Prefixed starting building residual for ZH generals.
        && find_player_template_by_side(SIDE_CHINA_TANK)
            .map(|t| t.starting_building.starts_with("Tank_"))
            .unwrap_or(false)
        && find_player_template_by_side(SIDE_GLA_TOXIN)
            .map(|t| t.starting_building.starts_with("Chem_"))
            .unwrap_or(false)
        && find_player_template_by_side(SIDE_GLA_STEALTH)
            .map(|t| t.starting_building.starts_with("Slth_"))
            .unwrap_or(false)
        // Civilian / Observer residual: not playable, no starting building.
        && find_player_template_residual("FactionCivilian")
            .map(|t| !t.playable && t.starting_building.is_empty())
            .unwrap_or(false)
        && find_player_template_residual("FactionObserver")
            .map(|t| !t.playable && t.starting_building.is_empty())
            .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// 3. Starting cash residual (+ difficulty-related residual)
// ---------------------------------------------------------------------------

/// GameData.ini DefaultStartingCash residual.
pub const DEFAULT_STARTING_CASH: u32 = 10_000;

/// multiplayer.ini MultiplayerStartingMoneyChoice residual list (ordered).
pub const MULTIPLAYER_STARTING_CASH_CHOICES: &[u32] = &[5_000, 10_000, 20_000, 50_000];

/// multiplayer.ini default MultiplayerStartingMoneyChoice residual (Default=Yes).
pub const MULTIPLAYER_DEFAULT_STARTING_CASH: u32 = 10_000;

/// PlayerTemplate StartMoney residual (all factions deposit **0**; lobby cash wins).
pub const PLAYER_TEMPLATE_START_MONEY: u32 = 0;

/// GameData.ini HumanSoloPlayerHealthBonus residual by difficulty (percent → factor).
pub const HUMAN_SOLO_HEALTH_BONUS_EASY: f32 = 1.50;
pub const HUMAN_SOLO_HEALTH_BONUS_NORMAL: f32 = 1.00;
pub const HUMAN_SOLO_HEALTH_BONUS_HARD: f32 = 0.80;

/// Whether a cash amount is a valid multiplayer starting-cash choice residual.
pub fn is_valid_starting_cash_choice(amount: u32) -> bool {
    MULTIPLAYER_STARTING_CASH_CHOICES.contains(&amount)
}

/// Index of starting cash in multiplayer residual list (None if invalid).
pub fn starting_cash_choice_index(amount: u32) -> Option<usize> {
    MULTIPLAYER_STARTING_CASH_CHOICES
        .iter()
        .position(|&v| v == amount)
}

/// Human solo health bonus residual for difficulty (Easy/Normal/Hard; Brutal→Hard).
pub fn human_solo_health_bonus_for_difficulty(diff: AIDifficulty) -> f32 {
    match diff {
        AIDifficulty::Easy => HUMAN_SOLO_HEALTH_BONUS_EASY,
        AIDifficulty::Medium => HUMAN_SOLO_HEALTH_BONUS_NORMAL,
        AIDifficulty::Hard | AIDifficulty::Brutal => HUMAN_SOLO_HEALTH_BONUS_HARD,
    }
}

/// Effective starting cash residual: multiplayer choice if valid, else default.
pub fn effective_starting_cash(choice: Option<u32>) -> u32 {
    match choice {
        Some(v) if is_valid_starting_cash_choice(v) => v,
        _ => DEFAULT_STARTING_CASH,
    }
}

/// Wave 85 honesty: starting cash residual (+ difficulty health residual).
pub fn honesty_starting_cash_residual_pack_wave85() -> bool {
    DEFAULT_STARTING_CASH == 10_000
        && Player::DEFAULT_STARTING_MONEY == DEFAULT_STARTING_CASH
        && MULTIPLAYER_DEFAULT_STARTING_CASH == DEFAULT_STARTING_CASH
        && MULTIPLAYER_STARTING_CASH_CHOICES == [5_000, 10_000, 20_000, 50_000]
        && is_valid_starting_cash_choice(5_000)
        && is_valid_starting_cash_choice(10_000)
        && is_valid_starting_cash_choice(20_000)
        && is_valid_starting_cash_choice(50_000)
        && !is_valid_starting_cash_choice(12_500)
        && !is_valid_starting_cash_choice(0)
        && starting_cash_choice_index(10_000) == Some(1)
        && starting_cash_choice_index(50_000) == Some(3)
        && starting_cash_choice_index(7_500).is_none()
        && PLAYER_TEMPLATE_START_MONEY == 0
        && effective_starting_cash(None) == DEFAULT_STARTING_CASH
        && effective_starting_cash(Some(20_000)) == 20_000
        && effective_starting_cash(Some(999)) == DEFAULT_STARTING_CASH
        // Difficulty residual health bonus anchors.
        && (human_solo_health_bonus_for_difficulty(AIDifficulty::Easy) - 1.50).abs() < 0.001
        && (human_solo_health_bonus_for_difficulty(AIDifficulty::Medium) - 1.00).abs() < 0.001
        && (human_solo_health_bonus_for_difficulty(AIDifficulty::Hard) - 0.80).abs() < 0.001
        && (human_solo_health_bonus_for_difficulty(AIDifficulty::Brutal) - 0.80).abs() < 0.001
        && HUMAN_SOLO_HEALTH_BONUS_EASY > HUMAN_SOLO_HEALTH_BONUS_NORMAL
        && HUMAN_SOLO_HEALTH_BONUS_NORMAL > HUMAN_SOLO_HEALTH_BONUS_HARD
}

// ---------------------------------------------------------------------------
// 4. Skirmish AI personality residual (AIData SideInfo)
// ---------------------------------------------------------------------------

/// AIData SideInfo residual for skirmish AI personality / gatherers / defenses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkirmishAiSideInfoResidual {
    pub side: &'static str,
    pub resource_gatherers_easy: u32,
    pub resource_gatherers_normal: u32,
    pub resource_gatherers_hard: u32,
    pub base_defense_structure1: &'static str,
    pub skill_set1_first: &'static str,
    pub skill_set2_first: &'static str,
}

/// AIData SideInfo residual seeds (base + ZH generals with SideInfo blocks).
pub const SKIRMISH_AI_SIDE_INFO_RESIDUAL: &[SkirmishAiSideInfoResidual] = &[
    SkirmishAiSideInfoResidual {
        side: SIDE_AMERICA,
        resource_gatherers_easy: 2,
        resource_gatherers_normal: 2,
        resource_gatherers_hard: 2,
        base_defense_structure1: "AmericaPatriotBattery",
        skill_set1_first: "SCIENCE_PaladinTank",
        skill_set2_first: "SCIENCE_PaladinTank",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_CHINA,
        resource_gatherers_easy: 2,
        resource_gatherers_normal: 2,
        resource_gatherers_hard: 2,
        base_defense_structure1: "ChinaGattlingCannon",
        skill_set1_first: "SCIENCE_NukeLauncher",
        skill_set2_first: "SCIENCE_RedGuardTraining",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_GLA,
        resource_gatherers_easy: 5,
        resource_gatherers_normal: 5,
        resource_gatherers_hard: 5,
        base_defense_structure1: "GLAStingerSite",
        skill_set1_first: "SCIENCE_ScudLauncher",
        skill_set2_first: "SCIENCE_ScudLauncher",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_AMERICA_AIRFORCE,
        resource_gatherers_easy: 2,
        resource_gatherers_normal: 2,
        resource_gatherers_hard: 2,
        base_defense_structure1: "AirF_AmericaPatriotBattery",
        skill_set1_first: "AirF_SCIENCE_A10ThunderboltMissileStrike1",
        skill_set2_first: "SCIENCE_SpectreGunship1",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_AMERICA_LASER,
        resource_gatherers_easy: 2,
        resource_gatherers_normal: 2,
        resource_gatherers_hard: 2,
        base_defense_structure1: "Lazr_AmericaPatriotBattery",
        skill_set1_first: "SCIENCE_StealthFighter",
        skill_set2_first: "SCIENCE_StealthFighter",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_AMERICA_SUPERWEAPON,
        resource_gatherers_easy: 2,
        resource_gatherers_normal: 2,
        resource_gatherers_hard: 2,
        base_defense_structure1: "SupW_AmericaPatriotBattery",
        skill_set1_first: "SCIENCE_StealthFighter",
        skill_set2_first: "SCIENCE_StealthFighter",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_CHINA_TANK,
        resource_gatherers_easy: 2,
        resource_gatherers_normal: 2,
        resource_gatherers_hard: 2,
        base_defense_structure1: "Tank_ChinaGattlingCannon",
        skill_set1_first: "SCIENCE_BattlemasterTraining",
        skill_set2_first: "SCIENCE_BattlemasterTraining",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_CHINA_INFANTRY,
        resource_gatherers_easy: 2,
        resource_gatherers_normal: 2,
        resource_gatherers_hard: 2,
        base_defense_structure1: "Infa_ChinaGattlingCannon",
        skill_set1_first: "Infa_SCIENCE_RedGuardTraining",
        skill_set2_first: "Infa_SCIENCE_RedGuardTraining",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_CHINA_NUKE,
        resource_gatherers_easy: 2,
        resource_gatherers_normal: 2,
        resource_gatherers_hard: 2,
        base_defense_structure1: "Nuke_ChinaGattlingCannon",
        skill_set1_first: "SCIENCE_RedGuardTraining",
        skill_set2_first: "SCIENCE_RedGuardTraining",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_GLA_DEMOLITION,
        resource_gatherers_easy: 5,
        resource_gatherers_normal: 5,
        resource_gatherers_hard: 5,
        base_defense_structure1: "Demo_GLAStingerSite",
        skill_set1_first: "SCIENCE_ScudLauncher",
        skill_set2_first: "SCIENCE_ScudLauncher",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_GLA_STEALTH,
        resource_gatherers_easy: 5,
        resource_gatherers_normal: 5,
        resource_gatherers_hard: 5,
        base_defense_structure1: "Slth_GLAStingerSite",
        skill_set1_first: "Slth_SCIENCE_GPSScrambler",
        skill_set2_first: "Slth_SCIENCE_GPSScrambler",
    },
    SkirmishAiSideInfoResidual {
        side: SIDE_GLA_TOXIN,
        resource_gatherers_easy: 5,
        resource_gatherers_normal: 5,
        resource_gatherers_hard: 5,
        base_defense_structure1: "Chem_GLAStingerSite",
        skill_set1_first: "SCIENCE_ScudLauncher",
        skill_set2_first: "SCIENCE_ScudLauncher",
    },
];

/// Host AIPersonality residual for base Team (ai.rs for_team).
pub fn host_personality_for_team(team: Team) -> AIPersonality {
    AIPersonality::for_team(team)
}

/// Resource gatherer residual for side + difficulty (AIData SideInfo).
pub fn resource_gatherers_for_side_difficulty(side: &str, diff: AIDifficulty) -> Option<u32> {
    let info = SKIRMISH_AI_SIDE_INFO_RESIDUAL
        .iter()
        .find(|s| s.side == side)?;
    Some(match diff {
        AIDifficulty::Easy => info.resource_gatherers_easy,
        AIDifficulty::Medium => info.resource_gatherers_normal,
        AIDifficulty::Hard | AIDifficulty::Brutal => info.resource_gatherers_hard,
    })
}

/// Wave 85 honesty: skirmish AI personality residual peels.
pub fn honesty_skirmish_ai_personality_residual_pack_wave85() -> bool {
    SKIRMISH_AI_SIDE_INFO_RESIDUAL.len() == 12
        // Base side gatherer residual: USA/China **2**, GLA **5**.
        && resource_gatherers_for_side_difficulty(SIDE_AMERICA, AIDifficulty::Medium) == Some(2)
        && resource_gatherers_for_side_difficulty(SIDE_CHINA, AIDifficulty::Hard) == Some(2)
        && resource_gatherers_for_side_difficulty(SIDE_GLA, AIDifficulty::Easy) == Some(5)
        && resource_gatherers_for_side_difficulty(SIDE_GLA_TOXIN, AIDifficulty::Brutal) == Some(5)
        && resource_gatherers_for_side_difficulty(SIDE_AMERICA_AIRFORCE, AIDifficulty::Easy)
            == Some(2)
        // Base defense residual anchors.
        && SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|s| s.side == SIDE_AMERICA)
            .map(|s| s.base_defense_structure1 == "AmericaPatriotBattery")
            .unwrap_or(false)
        && SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|s| s.side == SIDE_CHINA)
            .map(|s| s.base_defense_structure1 == "ChinaGattlingCannon")
            .unwrap_or(false)
        && SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|s| s.side == SIDE_GLA)
            .map(|s| s.base_defense_structure1 == "GLAStingerSite")
            .unwrap_or(false)
        && SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|s| s.side == SIDE_CHINA_TANK)
            .map(|s| s.base_defense_structure1.starts_with("Tank_"))
            .unwrap_or(false)
        && SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|s| s.side == SIDE_GLA_STEALTH)
            .map(|s| s.base_defense_structure1.starts_with("Slth_"))
            .unwrap_or(false)
        // SkillSet residual first science anchors.
        && SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|s| s.side == SIDE_AMERICA)
            .map(|s| s.skill_set1_first == "SCIENCE_PaladinTank")
            .unwrap_or(false)
        && SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|s| s.side == SIDE_CHINA)
            .map(|s| s.skill_set1_first == "SCIENCE_NukeLauncher")
            .unwrap_or(false)
        && SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|s| s.side == SIDE_GLA)
            .map(|s| s.skill_set1_first == "SCIENCE_ScudLauncher")
            .unwrap_or(false)
        // Host personality residual for base teams.
        && host_personality_for_team(Team::USA) == AIPersonality::Aggressive
        && host_personality_for_team(Team::China) == AIPersonality::Defensive
        && host_personality_for_team(Team::GLA) == AIPersonality::Rush
        && host_personality_for_team(Team::Neutral) == AIPersonality::Balanced
        // Unknown side residual fails closed.
        && resource_gatherers_for_side_difficulty("NotASide", AIDifficulty::Medium).is_none()
}

// ---------------------------------------------------------------------------
// 5. Victory condition residual peels
// ---------------------------------------------------------------------------

/// C++ VictoryType::VICTORY_NOBUILDINGS residual value.
pub const VICTORY_NO_BUILDINGS_BIT: u32 = 1;
/// C++ VictoryType::VICTORY_NOUNITS residual value.
pub const VICTORY_NO_UNITS_BIT: u32 = 2;
/// C++ default m_victoryConditions residual: both flags set.
pub const VICTORY_DEFAULT_MASK: u32 = VICTORY_NO_BUILDINGS_BIT | VICTORY_NO_UNITS_BIT;

/// Defeat residual matrix (mirrors VictoryConditions::hasSinglePlayerBeenDefeated).
///
/// Inputs: (no_units_flag, no_buildings_flag, has_any_objects, has_units, has_structures)
/// Output: defeated?
pub fn is_defeated_residual(
    no_units: bool,
    no_buildings: bool,
    has_any_objects: bool,
    has_units: bool,
    has_structures: bool,
) -> bool {
    match (no_units, no_buildings) {
        (true, true) => !has_any_objects,
        (true, false) => !has_units,
        (false, true) => !has_structures,
        (false, false) => !has_any_objects,
    }
}

/// Wave 85 honesty: victory condition residual peels.
pub fn honesty_victory_condition_residual_pack_wave85() -> bool {
    // Bit residual matches C++ VictoryType enum values.
    VictoryType::NO_BUILDINGS.bits() == VICTORY_NO_BUILDINGS_BIT
        && VictoryType::NO_UNITS.bits() == VICTORY_NO_UNITS_BIT
        && VictoryType::default().bits() == VICTORY_DEFAULT_MASK
        && VictoryType::default().requires_units()
        && VictoryType::default().requires_buildings()
        && VictoryType::from_requirements(true, true) == VictoryType::default()
        && VictoryType::from_requirements(true, false) == VictoryType::NO_UNITS
        && VictoryType::from_requirements(false, true) == VictoryType::NO_BUILDINGS
        && VictoryType::from_requirements(false, false).is_empty()
        // Defeat residual matrix (both flags — hasAnyObjects).
        && is_defeated_residual(true, true, false, false, false)
        && !is_defeated_residual(true, true, true, false, false)
        && !is_defeated_residual(true, true, true, true, true)
        // NOUNITS only — hasAnyUnits residual.
        && is_defeated_residual(true, false, true, false, true)
        && !is_defeated_residual(true, false, true, true, false)
        // NOBUILDINGS only — hasAnyBuildings residual.
        && is_defeated_residual(false, true, true, true, false)
        && !is_defeated_residual(false, true, true, false, true)
        // Neither flag residual falls back to hasAnyObjects.
        && is_defeated_residual(false, false, false, false, false)
        && !is_defeated_residual(false, false, true, false, false)
        && MAX_PLAYER_COUNT_RESIDUAL == 16
}

// ---------------------------------------------------------------------------
// Combined Wave 85 pack
// ---------------------------------------------------------------------------

/// Combined Wave 85 honesty pack (all five residual peels).
pub fn honesty_faction_skirmish_residual_pack_wave85() -> bool {
    honesty_faction_side_residual_table_wave85()
        && honesty_player_template_residual_pack_wave85()
        && honesty_starting_cash_residual_pack_wave85()
        && honesty_skirmish_ai_personality_residual_pack_wave85()
        && honesty_victory_condition_residual_pack_wave85()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faction_side_residual_table_wave85_honesty() {
        assert!(honesty_faction_side_residual_table_wave85());
    }

    #[test]
    fn player_template_residual_pack_wave85_honesty() {
        assert!(honesty_player_template_residual_pack_wave85());
    }

    #[test]
    fn starting_cash_residual_pack_wave85_honesty() {
        assert!(honesty_starting_cash_residual_pack_wave85());
    }

    #[test]
    fn skirmish_ai_personality_residual_pack_wave85_honesty() {
        assert!(honesty_skirmish_ai_personality_residual_pack_wave85());
    }

    #[test]
    fn victory_condition_residual_pack_wave85_honesty() {
        assert!(honesty_victory_condition_residual_pack_wave85());
    }

    #[test]
    fn faction_skirmish_residual_pack_wave85_honesty() {
        assert!(honesty_faction_skirmish_residual_pack_wave85());
    }
}
