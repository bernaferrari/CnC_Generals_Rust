//! Wave 89 residual peels: rank skill-point application / experience tables /
//! hotkey CommandMap / host chat / local replay / options residual.
//!
//! Orthogonal to Waves 80 (Rank.ini table), 84 (Veterancy enum), 86 (MP options).
//! Host-testable packs for GeneralsExperience + local UI residual honesty.
//!
//! Sources (retail ZH INI + C++):
//! - Player.cpp addSkillPoints / setRankLevel / resetRank / addSkillPointsForKill
//! - ExperienceTracker.cpp / ThingTemplate ExperienceRequired+Value / SkillPointValue
//! - GameData.ini HealthBonus_Veteran/Elite/Heroic
//! - English CommandMap.ini meta map + HotKeyManager
//! - InGameChat.cpp chat types / labels / replay block
//! - Recorder.h/.cpp local replay modes / paths (not network)
//! - OptionPreferences defaults (OptionsMenu.cpp + AudioSettings.ini)
//!
//! Fail-closed:
//! - Not full RankInfoStore live INI load / GeneralsExperience skill-point UI GPU
//! - Not full ExperienceTracker exclusive module matrix / XP sink live path
//! - Not full HotKeyManager WND binding / MetaEvent message stream GPU
//! - Not full InGameChat.wnd GPU / network chat replication
//! - Not full Recorder .rep I/O / TiVo playback GPU residual
//! - Not full OptionsMenu.wnd GPU / Options.ini write residual
//! - Shell `playable_claim` stays false; network deferred

use crate::game_logic::host_science_rank::{
    retail_cumulative_science_points_through, retail_rank_for_level,
    retail_rank_level_for_skill_points, RANK1_SKILL_POINTS_NEEDED, RANK2_SKILL_POINTS_NEEDED,
    RANK3_SKILL_POINTS_NEEDED, RANK4_SKILL_POINTS_NEEDED, RANK5_SKILL_POINTS_NEEDED,
    RANK5_SCIENCE_POINTS_GRANTED, RANK_SCIENCE_POINTS_DEFAULT, RETAIL_RANK_COUNT,
    RETAIL_RANK_TABLE,
};

// ---------------------------------------------------------------------------
// 1. Rank residual deepen — skill-points application residual
// ---------------------------------------------------------------------------

/// C++ `Player::m_skillPointsModifier` default residual.
pub const SKILL_POINTS_MODIFIER_DEFAULT_RESIDUAL: f32 = 1.0;
/// C++ `GameLogic::m_rankLevelLimit` default residual (reset every game).
pub const RANK_LEVEL_LIMIT_DEFAULT_RESIDUAL: i32 = 1000;
/// Intrinsic science purchase points residual when no PlayerTemplate (resetRank).
pub const INTRINSIC_SCIENCE_PURCHASE_POINTS_DEFAULT_RESIDUAL: i32 = 0;
/// ControlBar rank progress residual: full bar is 100.
pub const RANK_PROGRESS_FULL_PERCENT_RESIDUAL: i32 = 100;

/// Host residual rank state after skill-point application (mirrors Player rank fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankSkillStateResidual {
    pub rank_level: u32,
    pub skill_points: i32,
    pub science_purchase_points: i32,
    pub level_up: i32,
    pub level_down: i32,
}

/// C++ REAL_TO_INT_CEIL residual for skill-point modifier scaling.
pub fn skill_points_delta_after_modifier(delta: i32, modifier: f32) -> i32 {
    let scaled = (modifier as f64) * (delta as f64);
    scaled.ceil() as i32
}

/// Skill-point cap residual: Rank.ini SkillPointsNeeded at the effective level cap.
///
/// C++: `pointCap = getRankInfo(min(rankLevelLimit, rankCount))->m_skillPointsNeeded`
/// — caps at the **lowest** point of the cap level (not highest).
pub fn skill_point_cap_residual(rank_level_limit: i32) -> i32 {
    let level_cap = rank_level_limit
        .max(1)
        .min(RETAIL_RANK_COUNT as i32) as u32;
    retail_rank_for_level(level_cap)
        .map(|r| r.skill_points_needed)
        .unwrap_or(RANK5_SKILL_POINTS_NEEDED)
}

/// Next rank threshold residual (`m_levelUp`); INT_MAX-style when at top rank.
pub fn rank_level_up_threshold_residual(rank_level: u32) -> i32 {
    retail_rank_for_level(rank_level + 1)
        .map(|r| r.skill_points_needed)
        .unwrap_or(i32::MAX)
}

/// Current rank floor residual (`m_levelDown`).
pub fn rank_level_down_threshold_residual(rank_level: u32) -> i32 {
    retail_rank_for_level(rank_level)
        .map(|r| r.skill_points_needed)
        .unwrap_or(0)
}

/// C++ `Player::resetRank` residual: rank 1, skill 0, SPP = intrinsic + Rank1 grant.
pub fn reset_rank_residual(intrinsic_spp: i32) -> RankSkillStateResidual {
    let rank1_spp = retail_rank_for_level(1)
        .map(|r| r.science_purchase_points_granted)
        .unwrap_or(RANK_SCIENCE_POINTS_DEFAULT);
    RankSkillStateResidual {
        rank_level: 1,
        skill_points: 0,
        science_purchase_points: intrinsic_spp.max(0) + rank1_spp,
        level_up: rank_level_up_threshold_residual(1),
        level_down: 0,
    }
}

/// C++ `Player::setRankLevel` residual (upgrade-only path; downgrade resets first).
///
/// Grants SciencePurchasePoints + floors skill_points to each crossed rank threshold.
pub fn set_rank_level_residual(
    mut state: RankSkillStateResidual,
    new_level: u32,
    rank_level_limit: i32,
) -> RankSkillStateResidual {
    let mut target = new_level.max(1);
    let hard_cap = rank_level_limit
        .max(1)
        .min(RETAIL_RANK_COUNT as i32) as u32;
    if target > hard_cap {
        target = hard_cap;
    }
    if target == state.rank_level {
        return state;
    }
    if target < state.rank_level {
        state = reset_rank_residual(INTRINSIC_SCIENCE_PURCHASE_POINTS_DEFAULT_RESIDUAL);
        if target == 1 {
            return state;
        }
    }
    for level in (state.rank_level + 1)..=target {
        if let Some(rank) = retail_rank_for_level(level) {
            state.science_purchase_points =
                (state.science_purchase_points + rank.science_purchase_points_granted).max(0);
            if state.skill_points < rank.skill_points_needed {
                state.skill_points = rank.skill_points_needed;
            }
            state.level_down = rank.skill_points_needed;
        }
    }
    state.rank_level = target;
    state.level_up = rank_level_up_threshold_residual(target);
    state
}

/// C++ `Player::addSkillPoints` residual (modifier + pointCap + rank-up while loop).
///
/// Returns `(new_state, level_gained)`.
pub fn add_skill_points_residual(
    mut state: RankSkillStateResidual,
    delta: i32,
    modifier: f32,
    rank_level_limit: i32,
) -> (RankSkillStateResidual, bool) {
    let scaled = skill_points_delta_after_modifier(delta, modifier);
    if scaled == 0 {
        return (state, false);
    }
    let point_cap = skill_point_cap_residual(rank_level_limit);
    let next = state.skill_points.saturating_add(scaled);
    state.skill_points = next.min(point_cap);
    let mut level_gained = false;
    while state.skill_points >= state.level_up && state.level_up != i32::MAX {
        let before = state.rank_level;
        state = set_rank_level_residual(state, state.rank_level + 1, rank_level_limit);
        if state.rank_level > before {
            level_gained = true;
        } else {
            break;
        }
    }
    (state, level_gained)
}

/// ControlBar rank progress residual:
/// `((skill - levelDown) * 100) / (levelUp - levelDown)` when span > 0.
pub fn rank_progress_percent_residual(state: &RankSkillStateResidual) -> i32 {
    let span = state.level_up - state.level_down;
    if span <= 0 || state.level_up == i32::MAX {
        return RANK_PROGRESS_FULL_PERCENT_RESIDUAL;
    }
    let raw = (state.skill_points - state.level_down) * RANK_PROGRESS_FULL_PERCENT_RESIDUAL;
    (raw / span).clamp(0, RANK_PROGRESS_FULL_PERCENT_RESIDUAL)
}

/// Wave 89 honesty: rank skill-points application residual deepen.
pub fn honesty_rank_skill_points_application_residual_pack_wave89() -> bool {
    let reset = reset_rank_residual(0);
    let at_799 = {
        let (s, gained) = add_skill_points_residual(
            reset,
            799,
            SKILL_POINTS_MODIFIER_DEFAULT_RESIDUAL,
            RANK_LEVEL_LIMIT_DEFAULT_RESIDUAL,
        );
        s.rank_level == 1
            && s.skill_points == 799
            && !gained
            && s.level_up == RANK2_SKILL_POINTS_NEEDED
            && s.science_purchase_points == RANK_SCIENCE_POINTS_DEFAULT
    };
    let to_rank2 = {
        let (s, gained) = add_skill_points_residual(
            reset,
            800,
            SKILL_POINTS_MODIFIER_DEFAULT_RESIDUAL,
            RANK_LEVEL_LIMIT_DEFAULT_RESIDUAL,
        );
        s.rank_level == 2
            && s.skill_points == 800
            && gained
            && s.level_down == RANK2_SKILL_POINTS_NEEDED
            && s.level_up == RANK3_SKILL_POINTS_NEEDED
            && s.science_purchase_points == RANK_SCIENCE_POINTS_DEFAULT * 2
    };
    let multi_rank = {
        let (s, gained) = add_skill_points_residual(
            reset,
            5000,
            SKILL_POINTS_MODIFIER_DEFAULT_RESIDUAL,
            RANK_LEVEL_LIMIT_DEFAULT_RESIDUAL,
        );
        // Cap at Rank5 SkillPointsNeeded (5000), not beyond.
        s.rank_level == 5
            && s.skill_points == RANK5_SKILL_POINTS_NEEDED
            && gained
            && s.level_up == i32::MAX
            && s.science_purchase_points
                == retail_cumulative_science_points_through(5)
    };
    let capped_by_limit = {
        // Rank level limit 3 → pointCap = Rank3 SkillPointsNeeded (1500).
        let (s, _) = add_skill_points_residual(reset, 99999, 1.0, 3);
        s.skill_points == RANK3_SKILL_POINTS_NEEDED
            && s.rank_level == 3
            && skill_point_cap_residual(3) == RANK3_SKILL_POINTS_NEEDED
    };
    let modifier_ceil = skill_points_delta_after_modifier(1, 1.5) == 2
        && skill_points_delta_after_modifier(2, 0.5) == 1
        && skill_points_delta_after_modifier(1, 0.0) == 0
        && skill_points_delta_after_modifier(100, 1.0) == 100;
    let progress = {
        let mid = RankSkillStateResidual {
            rank_level: 2,
            skill_points: 1150, // midway 800..1500
            science_purchase_points: 2,
            level_up: RANK3_SKILL_POINTS_NEEDED,
            level_down: RANK2_SKILL_POINTS_NEEDED,
        };
        // (1150-800)*100 / (1500-800) = 35000/700 = 50
        rank_progress_percent_residual(&mid) == 50
            && rank_progress_percent_residual(&reset) == 0
    };
    let downgrade = {
        let high = set_rank_level_residual(reset, 4, RANK_LEVEL_LIMIT_DEFAULT_RESIDUAL);
        let down = set_rank_level_residual(high, 1, RANK_LEVEL_LIMIT_DEFAULT_RESIDUAL);
        high.rank_level == 4
            && high.skill_points == RANK4_SKILL_POINTS_NEEDED
            && high.science_purchase_points == 4
            && down.rank_level == 1
            && down.skill_points == 0
            && down.science_purchase_points == RANK_SCIENCE_POINTS_DEFAULT
    };
    SKILL_POINTS_MODIFIER_DEFAULT_RESIDUAL == 1.0
        && RANK_LEVEL_LIMIT_DEFAULT_RESIDUAL == 1000
        && skill_point_cap_residual(1000) == RANK5_SKILL_POINTS_NEEDED
        && skill_point_cap_residual(5) == RANK5_SKILL_POINTS_NEEDED
        && skill_point_cap_residual(1) == RANK1_SKILL_POINTS_NEEDED
        && rank_level_up_threshold_residual(5) == i32::MAX
        && rank_level_down_threshold_residual(1) == 0
        && reset.level_up == RANK2_SKILL_POINTS_NEEDED
        && retail_rank_level_for_skill_points(800) == 2
        && RETAIL_RANK_TABLE.len() == RETAIL_RANK_COUNT as usize
        && RANK5_SCIENCE_POINTS_GRANTED == 3
        && at_799
        && to_rank2
        && multi_rank
        && capped_by_limit
        && modifier_ceil
        && progress
        && downgrade
}

// ---------------------------------------------------------------------------
// 2. Experience residual tables
// ---------------------------------------------------------------------------

/// C++ VeterancyLevel residual (mirrors Wave 84 enum table honesty).
pub const LEVEL_REGULAR_RESIDUAL: i32 = 0;
pub const LEVEL_VETERAN_RESIDUAL: i32 = 1;
pub const LEVEL_ELITE_RESIDUAL: i32 = 2;
pub const LEVEL_HEROIC_RESIDUAL: i32 = 3;
pub const LEVEL_COUNT_RESIDUAL: i32 = 4;
pub const LEVEL_LAST_RESIDUAL: i32 = LEVEL_HEROIC_RESIDUAL;

/// C++ `USE_EXP_VALUE_FOR_SKILL_VALUE` residual sentinel (−999).
pub const USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL: i32 = -999;

/// Default ExperienceTracker scalar residual.
pub const EXPERIENCE_SCALAR_DEFAULT_RESIDUAL: f32 = 1.0;

/// Retail GameData.ini HealthBonus residual (Regular defaults 100% in C++ ctor).
pub const HEALTH_BONUS_REGULAR_RESIDUAL: f32 = 1.0;
pub const HEALTH_BONUS_VETERAN_RESIDUAL: f32 = 1.20;
pub const HEALTH_BONUS_ELITE_RESIDUAL: f32 = 1.30;
pub const HEALTH_BONUS_HEROIC_RESIDUAL: f32 = 1.50;

/// Common ExperienceRequired residual ladders (Regular/Vet/Elite/Heroic thresholds).
pub const EXP_REQUIRED_LIGHT_INFANTRY_RESIDUAL: [i32; 4] = [0, 40, 60, 120];
pub const EXP_REQUIRED_STANDARD_RESIDUAL: [i32; 4] = [0, 100, 200, 400];
pub const EXP_REQUIRED_HEAVY_RESIDUAL: [i32; 4] = [0, 200, 400, 800];
pub const EXP_REQUIRED_VEHICLE_ALT_RESIDUAL: [i32; 4] = [0, 100, 150, 300];

/// Common ExperienceValue residual packs (what killer earns at victim's level).
pub const EXP_VALUE_RANGER_RESIDUAL: [i32; 4] = [20, 20, 40, 60];
pub const EXP_VALUE_STANDARD_AIR_RESIDUAL: [i32; 4] = [50, 100, 150, 200];
pub const EXP_VALUE_STRUCTURE_FLAT_RESIDUAL: [i32; 4] = [200, 200, 200, 200];

/// AmericaInfantryRanger ExperienceRequired residual (retail AmericaInfantry.ini).
pub const RANGER_EXPERIENCE_REQUIRED_RESIDUAL: [i32; 4] = [0, 40, 60, 120];
/// AmericaInfantryRanger ExperienceValue residual.
pub const RANGER_EXPERIENCE_VALUE_RESIDUAL: [i32; 4] = [20, 20, 40, 60];

/// Resolve skill-point value residual (ThingTemplate::getSkillPointValue).
pub fn skill_point_value_residual(skill_point_values: [i32; 4], experience_values: [i32; 4], level: i32) -> i32 {
    if !(0..LEVEL_COUNT_RESIDUAL).contains(&level) {
        return 0;
    }
    let v = skill_point_values[level as usize];
    if v == USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL {
        experience_values[level as usize]
    } else {
        v
    }
}

/// Experience level from current XP vs ExperienceRequired residual table.
pub fn experience_level_for_points(required: [i32; 4], experience: i32) -> i32 {
    if experience < 0 {
        return LEVEL_REGULAR_RESIDUAL;
    }
    let mut level = LEVEL_REGULAR_RESIDUAL;
    while (level + 1) < LEVEL_COUNT_RESIDUAL
        && experience >= required[(level + 1) as usize]
    {
        level += 1;
    }
    level
}

/// Apply experience gain residual (ExperienceTracker::addExperiencePoints core).
pub fn add_experience_points_residual(
    current_experience: i32,
    gain: i32,
    scalar: f32,
    can_scale: bool,
    required: [i32; 4],
) -> (i32, i32) {
    let amount = if can_scale {
        (gain as f32 * scalar) as i32
    } else {
        gain
    };
    let new_xp = current_experience + amount;
    let new_level = experience_level_for_points(required, new_xp);
    (new_xp, new_level)
}

/// Health bonus residual for a veterancy level.
pub fn health_bonus_for_level_residual(level: i32) -> f32 {
    match level {
        LEVEL_VETERAN_RESIDUAL => HEALTH_BONUS_VETERAN_RESIDUAL,
        LEVEL_ELITE_RESIDUAL => HEALTH_BONUS_ELITE_RESIDUAL,
        LEVEL_HEROIC_RESIDUAL => HEALTH_BONUS_HEROIC_RESIDUAL,
        _ => HEALTH_BONUS_REGULAR_RESIDUAL,
    }
}

/// Ally kill experience residual is always 0 (ExperienceTracker::getExperienceValue).
pub fn experience_value_for_kill_residual(
    experience_values: [i32; 4],
    victim_level: i32,
    killer_is_ally: bool,
) -> i32 {
    if killer_is_ally {
        return 0;
    }
    if !(0..LEVEL_COUNT_RESIDUAL).contains(&victim_level) {
        return 0;
    }
    experience_values[victim_level as usize]
}

/// Wave 89 honesty: experience residual tables pack.
pub fn honesty_experience_residual_tables_pack_wave89() -> bool {
    let ranger_level = experience_level_for_points(RANGER_EXPERIENCE_REQUIRED_RESIDUAL, 40)
        == LEVEL_VETERAN_RESIDUAL
        && experience_level_for_points(RANGER_EXPERIENCE_REQUIRED_RESIDUAL, 39)
            == LEVEL_REGULAR_RESIDUAL
        && experience_level_for_points(RANGER_EXPERIENCE_REQUIRED_RESIDUAL, 60)
            == LEVEL_ELITE_RESIDUAL
        && experience_level_for_points(RANGER_EXPERIENCE_REQUIRED_RESIDUAL, 120)
            == LEVEL_HEROIC_RESIDUAL
        && experience_level_for_points(RANGER_EXPERIENCE_REQUIRED_RESIDUAL, 999)
            == LEVEL_HEROIC_RESIDUAL;
    let skill_from_exp = skill_point_value_residual(
        [
            USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL,
            USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL,
            USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL,
            USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL,
        ],
        RANGER_EXPERIENCE_VALUE_RESIDUAL,
        LEVEL_ELITE_RESIDUAL,
    ) == 40
        && skill_point_value_residual([5, 5, 5, 5], RANGER_EXPERIENCE_VALUE_RESIDUAL, 0) == 5;
    let add_xp = {
        let (xp, lvl) = add_experience_points_residual(
            0,
            40,
            EXPERIENCE_SCALAR_DEFAULT_RESIDUAL,
            true,
            RANGER_EXPERIENCE_REQUIRED_RESIDUAL,
        );
        xp == 40 && lvl == LEVEL_VETERAN_RESIDUAL
    };
    let scaled = {
        // AdvancedTraining AddXPScalar 1.0 → total scalar 2.0 residual path example.
        let (xp, lvl) = add_experience_points_residual(
            0,
            40,
            2.0,
            true,
            RANGER_EXPERIENCE_REQUIRED_RESIDUAL,
        );
        xp == 80 && lvl == LEVEL_ELITE_RESIDUAL
    };
    let no_scale = {
        let (xp, _) = add_experience_points_residual(
            0,
            40,
            2.0,
            false,
            RANGER_EXPERIENCE_REQUIRED_RESIDUAL,
        );
        xp == 40
    };
    let ally_zero = experience_value_for_kill_residual(
        RANGER_EXPERIENCE_VALUE_RESIDUAL,
        LEVEL_HEROIC_RESIDUAL,
        true,
    ) == 0
        && experience_value_for_kill_residual(
            RANGER_EXPERIENCE_VALUE_RESIDUAL,
            LEVEL_REGULAR_RESIDUAL,
            false,
        ) == 20;
    let ladders_monotonic = EXP_REQUIRED_LIGHT_INFANTRY_RESIDUAL
        .windows(2)
        .all(|w| w[0] < w[1])
        && EXP_REQUIRED_STANDARD_RESIDUAL.windows(2).all(|w| w[0] < w[1])
        && EXP_REQUIRED_HEAVY_RESIDUAL.windows(2).all(|w| w[0] < w[1])
        && EXP_REQUIRED_VEHICLE_ALT_RESIDUAL
            .windows(2)
            .all(|w| w[0] < w[1]);
    LEVEL_COUNT_RESIDUAL == 4
        && LEVEL_LAST_RESIDUAL == 3
        && USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL == -999
        && (EXPERIENCE_SCALAR_DEFAULT_RESIDUAL - 1.0).abs() < 1e-5
        && (HEALTH_BONUS_REGULAR_RESIDUAL - 1.0).abs() < 1e-5
        && (HEALTH_BONUS_VETERAN_RESIDUAL - 1.20).abs() < 1e-5
        && (HEALTH_BONUS_ELITE_RESIDUAL - 1.30).abs() < 1e-5
        && (HEALTH_BONUS_HEROIC_RESIDUAL - 1.50).abs() < 1e-5
        && health_bonus_for_level_residual(LEVEL_VETERAN_RESIDUAL) == HEALTH_BONUS_VETERAN_RESIDUAL
        && health_bonus_for_level_residual(LEVEL_HEROIC_RESIDUAL) == HEALTH_BONUS_HEROIC_RESIDUAL
        && RANGER_EXPERIENCE_REQUIRED_RESIDUAL == EXP_REQUIRED_LIGHT_INFANTRY_RESIDUAL
        && RANGER_EXPERIENCE_VALUE_RESIDUAL == EXP_VALUE_RANGER_RESIDUAL
        && EXP_VALUE_STANDARD_AIR_RESIDUAL == [50, 100, 150, 200]
        && EXP_VALUE_STRUCTURE_FLAT_RESIDUAL == [200, 200, 200, 200]
        && ranger_level
        && skill_from_exp
        && add_xp
        && scaled
        && no_scale
        && ally_zero
        && ladders_monotonic
}

// ---------------------------------------------------------------------------
// 3. Hotkey residual table (CommandMap.ini)
// ---------------------------------------------------------------------------

/// Retail English CommandMap.ini active CommandMap count residual.
pub const COMMAND_MAP_ACTIVE_COUNT_RESIDUAL: u32 = 96;
/// Control groups residual (CREATE_TEAM0..9 / SELECT_TEAM0..9).
pub const CONTROL_GROUP_COUNT_RESIDUAL: u32 = 10;
/// Save-view slots residual (SAVE_VIEW1..8 / VIEW_VIEW1..8).
pub const SAVE_VIEW_COUNT_RESIDUAL: u32 = 8;

/// CommandMap meta anchors residual (name, key token, modifiers token).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandMapAnchorResidual {
    pub name: &'static str,
    pub key: &'static str,
    pub modifiers: &'static str,
}

/// Host residual CommandMap anchors (EnglishZH CommandMap.ini).
pub const COMMAND_MAP_ANCHORS_RESIDUAL: [CommandMapAnchorResidual; 12] = [
    CommandMapAnchorResidual {
        name: "CHAT_ALLIES",
        key: "KEY_BACKSPACE",
        modifiers: "NONE",
    },
    CommandMapAnchorResidual {
        name: "CHAT_EVERYONE",
        key: "KEY_ENTER",
        modifiers: "NONE",
    },
    CommandMapAnchorResidual {
        name: "SELECT_MATCHING_UNITS",
        key: "KEY_E",
        modifiers: "NONE",
    },
    CommandMapAnchorResidual {
        name: "CREATE_TEAM0",
        key: "KEY_0",
        modifiers: "CTRL",
    },
    CommandMapAnchorResidual {
        name: "SELECT_TEAM0",
        key: "KEY_0",
        modifiers: "NONE",
    },
    CommandMapAnchorResidual {
        name: "PLACE_BEACON",
        key: "KEY_B",
        modifiers: "CTRL",
    },
    CommandMapAnchorResidual {
        name: "TOGGLE_CONTROL_BAR",
        key: "KEY_F9",
        modifiers: "NONE",
    },
    CommandMapAnchorResidual {
        name: "TOGGLE_FAST_FORWARD_REPLAY",
        key: "KEY_F",
        modifiers: "NONE",
    },
    CommandMapAnchorResidual {
        name: "DIPLOMACY",
        key: "KEY_TAB",
        modifiers: "NONE",
    },
    CommandMapAnchorResidual {
        name: "SAVE_VIEW1",
        key: "KEY_F1",
        modifiers: "CTRL",
    },
    CommandMapAnchorResidual {
        name: "VIEW_VIEW1",
        key: "KEY_F1",
        modifiers: "NONE",
    },
    CommandMapAnchorResidual {
        name: "DEMO_INSTANT_QUIT",
        key: "KEY_BACKSPACE",
        modifiers: "SHIFT_CTRL",
    },
];

/// HotKeyManager residual: keys stored lowercased; execute rejects modifier combos
/// on the raw key translator path (CTRL/SHIFT/ALT must be 0).
pub const HOTKEY_STORE_LOWERCASE_RESIDUAL: bool = true;
pub const HOTKEY_RAW_KEY_REQUIRES_NO_MODIFIERS_RESIDUAL: bool = true;

/// CHAT_PLAYERS CommandMap residual is **commented out** in retail English map.
pub const CHAT_PLAYERS_COMMANDMAP_COMMENTED_RESIDUAL: bool = true;

/// Lookup residual CommandMap anchor by name.
pub fn command_map_anchor_residual(name: &str) -> Option<&'static CommandMapAnchorResidual> {
    COMMAND_MAP_ANCHORS_RESIDUAL.iter().find(|a| a.name == name)
}

/// Wave 89 honesty: hotkey residual table pack.
pub fn honesty_hotkey_residual_table_pack_wave89() -> bool {
    COMMAND_MAP_ACTIVE_COUNT_RESIDUAL == 96
        && CONTROL_GROUP_COUNT_RESIDUAL == 10
        && SAVE_VIEW_COUNT_RESIDUAL == 8
        && COMMAND_MAP_ANCHORS_RESIDUAL.len() == 12
        && HOTKEY_STORE_LOWERCASE_RESIDUAL
        && HOTKEY_RAW_KEY_REQUIRES_NO_MODIFIERS_RESIDUAL
        && CHAT_PLAYERS_COMMANDMAP_COMMENTED_RESIDUAL
        && command_map_anchor_residual("CHAT_ALLIES")
            .map(|a| a.key == "KEY_BACKSPACE" && a.modifiers == "NONE")
            == Some(true)
        && command_map_anchor_residual("CHAT_EVERYONE")
            .map(|a| a.key == "KEY_ENTER")
            == Some(true)
        && command_map_anchor_residual("SELECT_MATCHING_UNITS")
            .map(|a| a.key == "KEY_E")
            == Some(true)
        && command_map_anchor_residual("CREATE_TEAM0")
            .map(|a| a.modifiers == "CTRL")
            == Some(true)
        && command_map_anchor_residual("SELECT_TEAM0")
            .map(|a| a.modifiers == "NONE")
            == Some(true)
        && command_map_anchor_residual("PLACE_BEACON")
            .map(|a| a.key == "KEY_B" && a.modifiers == "CTRL")
            == Some(true)
        && command_map_anchor_residual("TOGGLE_FAST_FORWARD_REPLAY")
            .map(|a| a.key == "KEY_F")
            == Some(true)
        && command_map_anchor_residual("CHAT_PLAYERS").is_none()
        // SAVE_VIEW1 vs VIEW_VIEW1 share KEY_F1, distinguished by modifiers.
        && command_map_anchor_residual("SAVE_VIEW1").map(|a| a.modifiers) == Some("CTRL")
        && command_map_anchor_residual("VIEW_VIEW1").map(|a| a.modifiers) == Some("NONE")
}

// ---------------------------------------------------------------------------
// 4. Chat residual host peels (local UI; not network)
// ---------------------------------------------------------------------------

/// C++ `InGameChatType` residual order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum InGameChatTypeResidual {
    Allies = 0,
    Everyone = 1,
    Players = 2,
}

pub const INGAME_CHAT_TYPE_COUNT_RESIDUAL: i32 = 3;
pub const INGAME_CHAT_WND_RESIDUAL: &str = "InGameChat.wnd";
pub const INGAME_CHAT_TEXT_ENTRY_ID_RESIDUAL: &str = "InGameChat.wnd:TextEntryChat";
pub const INGAME_CHAT_TYPE_STATIC_ID_RESIDUAL: &str = "InGameChat.wnd:StaticTextChatType";

pub const CHAT_LABEL_EVERYONE_RESIDUAL: &str = "Chat:Everyone";
pub const CHAT_LABEL_ALLIES_RESIDUAL: &str = "Chat:Allies";
pub const CHAT_LABEL_PLAYERS_RESIDUAL: &str = "Chat:Players";
pub const CHAT_LABEL_OBSERVERS_RESIDUAL: &str = "Chat:Observers";

/// Multiplayer slot residual (MAX_SLOTS = MAX_PLAYER+1 = 8).
pub const MAX_SLOTS_RESIDUAL: i32 = 8;
/// Chat is blocked during local replay residual.
pub const CHAT_BLOCKED_IN_REPLAY_RESIDUAL: bool = true;
/// Default ShowInGameChat sets EVERYONE residual.
pub const CHAT_DEFAULT_TYPE_ON_SHOW_RESIDUAL: InGameChatTypeResidual =
    InGameChatTypeResidual::Everyone;

/// MetaEvent chat command residual names.
pub const META_CHAT_PLAYERS_RESIDUAL: &str = "CHAT_PLAYERS";
pub const META_CHAT_ALLIES_RESIDUAL: &str = "CHAT_ALLIES";
pub const META_CHAT_EVERYONE_RESIDUAL: &str = "CHAT_EVERYONE";

/// Chat type label residual (active player → Everyone; inactive → Observers).
pub fn chat_type_label_residual(
    chat_type: InGameChatTypeResidual,
    local_player_active: bool,
) -> &'static str {
    match chat_type {
        InGameChatTypeResidual::Everyone => {
            if local_player_active {
                CHAT_LABEL_EVERYONE_RESIDUAL
            } else {
                CHAT_LABEL_OBSERVERS_RESIDUAL
            }
        }
        InGameChatTypeResidual::Allies => CHAT_LABEL_ALLIES_RESIDUAL,
        InGameChatTypeResidual::Players => CHAT_LABEL_PLAYERS_RESIDUAL,
    }
}

/// Host residual: may show in-game chat (blocked in replay).
pub fn can_show_ingame_chat_residual(in_replay_game: bool) -> bool {
    // C++ ShowInGameChat / ToggleInGameChat: early-return when isInReplayGame().
    !in_replay_game
}

/// Wave 89 honesty: chat residual host peels pack.
pub fn honesty_chat_residual_host_pack_wave89() -> bool {
    InGameChatTypeResidual::Allies as i32 == 0
        && InGameChatTypeResidual::Everyone as i32 == 1
        && InGameChatTypeResidual::Players as i32 == 2
        && INGAME_CHAT_TYPE_COUNT_RESIDUAL == 3
        && INGAME_CHAT_WND_RESIDUAL == "InGameChat.wnd"
        && INGAME_CHAT_TEXT_ENTRY_ID_RESIDUAL.ends_with("TextEntryChat")
        && INGAME_CHAT_TYPE_STATIC_ID_RESIDUAL.ends_with("StaticTextChatType")
        && CHAT_DEFAULT_TYPE_ON_SHOW_RESIDUAL == InGameChatTypeResidual::Everyone
        && CHAT_BLOCKED_IN_REPLAY_RESIDUAL
        && MAX_SLOTS_RESIDUAL == 8
        && chat_type_label_residual(InGameChatTypeResidual::Everyone, true)
            == "Chat:Everyone"
        && chat_type_label_residual(InGameChatTypeResidual::Everyone, false)
            == "Chat:Observers"
        && chat_type_label_residual(InGameChatTypeResidual::Allies, true) == "Chat:Allies"
        && chat_type_label_residual(InGameChatTypeResidual::Players, true) == "Chat:Players"
        && can_show_ingame_chat_residual(false)
        && !can_show_ingame_chat_residual(true)
        && META_CHAT_ALLIES_RESIDUAL == "CHAT_ALLIES"
        && META_CHAT_EVERYONE_RESIDUAL == "CHAT_EVERYONE"
        && META_CHAT_PLAYERS_RESIDUAL == "CHAT_PLAYERS"
        // Chat meta keys exist in CommandMap residual anchors.
        && command_map_anchor_residual("CHAT_ALLIES").is_some()
        && command_map_anchor_residual("CHAT_EVERYONE").is_some()
}

// ---------------------------------------------------------------------------
// 5. Replay residual host peels (local; not network)
// ---------------------------------------------------------------------------

/// C++ `RecorderModeType` residual order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RecorderModeTypeResidual {
    Record = 0,
    Playback = 1,
    None = 2,
}

pub const REPLAY_EXTENSION_RESIDUAL: &str = ".rep";
pub const REPLAY_DIR_NAME_RESIDUAL: &str = "Replays\\";
pub const LAST_REPLAY_FILE_NAME_RESIDUAL: &str = "00000000";
/// OptionPreferences SaveCameraInReplays default residual (missing key → TRUE).
pub const SAVE_CAMERA_IN_REPLAYS_DEFAULT_RESIDUAL: bool = true;
/// OptionPreferences UseCameraInReplays default residual (missing key → TRUE).
pub const USE_CAMERA_IN_REPLAYS_DEFAULT_RESIDUAL: bool = true;
/// TiVo fast-forward CommandMap residual name.
pub const TOGGLE_FAST_FORWARD_REPLAY_META_RESIDUAL: &str = "TOGGLE_FAST_FORWARD_REPLAY";

/// Host residual: compose replay path under user data.
pub fn replay_path_residual(user_data: &str, file_stem: &str) -> String {
    // C++ concat is literal `Replays\` after the user-data path.
    format!(
        "{}{}{}{}",
        user_data, REPLAY_DIR_NAME_RESIDUAL, file_stem, REPLAY_EXTENSION_RESIDUAL
    )
}

/// Wave 89 honesty: local replay residual pack.
pub fn honesty_replay_residual_host_pack_wave89() -> bool {
    RecorderModeTypeResidual::Record as i32 == 0
        && RecorderModeTypeResidual::Playback as i32 == 1
        && RecorderModeTypeResidual::None as i32 == 2
        && REPLAY_EXTENSION_RESIDUAL == ".rep"
        && REPLAY_DIR_NAME_RESIDUAL == "Replays\\"
        && LAST_REPLAY_FILE_NAME_RESIDUAL == "00000000"
        && SAVE_CAMERA_IN_REPLAYS_DEFAULT_RESIDUAL
        && USE_CAMERA_IN_REPLAYS_DEFAULT_RESIDUAL
        && TOGGLE_FAST_FORWARD_REPLAY_META_RESIDUAL == "TOGGLE_FAST_FORWARD_REPLAY"
        && command_map_anchor_residual("TOGGLE_FAST_FORWARD_REPLAY")
            .map(|a| a.key == "KEY_F" && a.modifiers == "NONE")
            == Some(true)
        && replay_path_residual("UserData\\", LAST_REPLAY_FILE_NAME_RESIDUAL)
            == "UserData\\Replays\\00000000.rep"
        // Chat residual reuses replay block.
        && CHAT_BLOCKED_IN_REPLAY_RESIDUAL
        && !can_show_ingame_chat_residual(true)
}

// ---------------------------------------------------------------------------
// 6. Options residual peels
// ---------------------------------------------------------------------------

/// AudioSettings.ini default volume residual (percent of full, as OptionPreferences 0..100).
pub const DEFAULT_MUSIC_VOLUME_PERCENT_RESIDUAL: f32 = 55.0;
pub const DEFAULT_SPEECH_VOLUME_PERCENT_RESIDUAL: f32 = 70.0;
pub const DEFAULT_SOUND_VOLUME_PERCENT_RESIDUAL: f32 = 80.0;
pub const DEFAULT_3D_SOUND_VOLUME_PERCENT_RESIDUAL: f32 = 80.0;
/// Relative2DVolume residual −10% (AudioSettings.ini).
pub const RELATIVE_2D_VOLUME_RESIDUAL: f32 = -0.10;

/// OptionPreferences Gamma default residual (missing key → 50).
pub const OPTIONS_GAMMA_DEFAULT_RESIDUAL: f32 = 50.0;
/// KeyboardDefaultScrollSpeedFactor / getScrollFactor default residual 0.5 → 50%.
pub const OPTIONS_SCROLL_FACTOR_DEFAULT_RESIDUAL: f32 = 0.5;
/// Language filter checkbox default residual Yes.
pub const OPTIONS_LANGUAGE_FILTER_DEFAULT_RESIDUAL: bool = true;
/// SendDelay checkbox default residual No.
pub const OPTIONS_SEND_DELAY_DEFAULT_RESIDUAL: bool = false;
/// UseSystemMapDir default residual Yes.
pub const OPTIONS_USE_SYSTEM_MAP_DIR_DEFAULT_RESIDUAL: bool = true;
/// FPSLimit default residual follows GameData UseFPSLimit Yes.
pub const OPTIONS_FPS_LIMIT_DEFAULT_RESIDUAL: bool = true;
/// Particle cap clamp residual minimum 100.
pub const OPTIONS_PARTICLE_CAP_MIN_RESIDUAL: i32 = 100;
/// TextureReduction clamp residual max 2.
pub const OPTIONS_TEXTURE_REDUCTION_MAX_RESIDUAL: i32 = 2;

/// Option preference key residual anchors.
pub const OPT_KEY_MUSIC_VOLUME: &str = "MusicVolume";
pub const OPT_KEY_SFX_VOLUME: &str = "SFXVolume";
pub const OPT_KEY_SFX3D_VOLUME: &str = "SFX3DVolume";
pub const OPT_KEY_VOICE_VOLUME: &str = "VoiceVolume";
pub const OPT_KEY_GAMMA: &str = "Gamma";
pub const OPT_KEY_SCROLL_FACTOR: &str = "ScrollFactor";
pub const OPT_KEY_SAVE_CAMERA_REPLAYS: &str = "SaveCameraInReplays";
pub const OPT_KEY_USE_CAMERA_REPLAYS: &str = "UseCameraInReplays";
pub const OPT_KEY_FPS_LIMIT: &str = "FPSLimit";
pub const OPT_KEY_SEND_DELAY: &str = "SendDelay";

/// Scroll factor residual clamp 0..100 → 0.0..1.0.
pub fn options_scroll_factor_from_percent_residual(percent: i32) -> f32 {
    percent.clamp(0, 100) as f32 / 100.0
}

/// Default 2D sound volume residual with Relative2DVolume scale (OptionPreferences).
pub fn options_default_sound_volume_residual() -> f32 {
    let relative = RELATIVE_2D_VOLUME_RESIDUAL;
    if relative < 0.0 {
        DEFAULT_SOUND_VOLUME_PERCENT_RESIDUAL * (1.0 + relative)
    } else {
        DEFAULT_SOUND_VOLUME_PERCENT_RESIDUAL
    }
}

/// Wave 89 honesty: options residual peels pack.
pub fn honesty_options_residual_pack_wave89() -> bool {
    (DEFAULT_MUSIC_VOLUME_PERCENT_RESIDUAL - 55.0).abs() < 1e-5
        && (DEFAULT_SPEECH_VOLUME_PERCENT_RESIDUAL - 70.0).abs() < 1e-5
        && (DEFAULT_SOUND_VOLUME_PERCENT_RESIDUAL - 80.0).abs() < 1e-5
        && (DEFAULT_3D_SOUND_VOLUME_PERCENT_RESIDUAL - 80.0).abs() < 1e-5
        && (RELATIVE_2D_VOLUME_RESIDUAL - (-0.10)).abs() < 1e-5
        && (OPTIONS_GAMMA_DEFAULT_RESIDUAL - 50.0).abs() < 1e-5
        && (OPTIONS_SCROLL_FACTOR_DEFAULT_RESIDUAL - 0.5).abs() < 1e-5
        && OPTIONS_LANGUAGE_FILTER_DEFAULT_RESIDUAL
        && !OPTIONS_SEND_DELAY_DEFAULT_RESIDUAL
        && OPTIONS_USE_SYSTEM_MAP_DIR_DEFAULT_RESIDUAL
        && OPTIONS_FPS_LIMIT_DEFAULT_RESIDUAL
        && OPTIONS_PARTICLE_CAP_MIN_RESIDUAL == 100
        && OPTIONS_TEXTURE_REDUCTION_MAX_RESIDUAL == 2
        && options_scroll_factor_from_percent_residual(50) == 0.5
        && options_scroll_factor_from_percent_residual(-10) == 0.0
        && options_scroll_factor_from_percent_residual(150) == 1.0
        && (options_default_sound_volume_residual() - 72.0).abs() < 1e-4
        && OPT_KEY_MUSIC_VOLUME == "MusicVolume"
        && OPT_KEY_SFX_VOLUME == "SFXVolume"
        && OPT_KEY_VOICE_VOLUME == "VoiceVolume"
        && OPT_KEY_GAMMA == "Gamma"
        && OPT_KEY_SCROLL_FACTOR == "ScrollFactor"
        && OPT_KEY_SAVE_CAMERA_REPLAYS == "SaveCameraInReplays"
        && OPT_KEY_USE_CAMERA_REPLAYS == "UseCameraInReplays"
        && OPT_KEY_FPS_LIMIT == "FPSLimit"
        && OPT_KEY_SEND_DELAY == "SendDelay"
        && OPT_KEY_SFX3D_VOLUME == "SFX3DVolume"
        && SAVE_CAMERA_IN_REPLAYS_DEFAULT_RESIDUAL
        && USE_CAMERA_IN_REPLAYS_DEFAULT_RESIDUAL
}

// ---------------------------------------------------------------------------
// Combined Wave 89 pack
// ---------------------------------------------------------------------------

/// Combined Wave 89 honesty pack (all residual peels).
pub fn honesty_rank_ui_residual_pack_wave89() -> bool {
    honesty_rank_skill_points_application_residual_pack_wave89()
        && honesty_experience_residual_tables_pack_wave89()
        && honesty_hotkey_residual_table_pack_wave89()
        && honesty_chat_residual_host_pack_wave89()
        && honesty_replay_residual_host_pack_wave89()
        && honesty_options_residual_pack_wave89()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_skill_points_application_residual_pack_wave89_honesty() {
        assert!(honesty_rank_skill_points_application_residual_pack_wave89());
    }

    #[test]
    fn experience_residual_tables_pack_wave89_honesty() {
        assert!(honesty_experience_residual_tables_pack_wave89());
    }

    #[test]
    fn hotkey_residual_table_pack_wave89_honesty() {
        assert!(honesty_hotkey_residual_table_pack_wave89());
    }

    #[test]
    fn chat_residual_host_pack_wave89_honesty() {
        assert!(honesty_chat_residual_host_pack_wave89());
    }

    #[test]
    fn replay_residual_host_pack_wave89_honesty() {
        assert!(honesty_replay_residual_host_pack_wave89());
    }

    #[test]
    fn options_residual_pack_wave89_honesty() {
        assert!(honesty_options_residual_pack_wave89());
    }

    #[test]
    fn rank_ui_residual_pack_wave89_combined_honesty() {
        assert!(honesty_rank_ui_residual_pack_wave89());
    }
}
