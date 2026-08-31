//! Wave 97 residual peels: Radar residual deepen / Spotter residual peels /
//! Stealth residual deepen / Detector residual deepen / Vision residual peels.
//!
//! Orthogonal to Waves 63 (radar provider/van), 72 (radar scan), 79 (StealthLook
//! ordinals), 93 (drawable opacity/shroud), and unit-level stealth packs.
//! Host-testable packs for radar/spotter/stealth/detector/vision residual honesty.
//!
//! Sources (retail ZH C++ / INI):
//! - Common/Radar.h RadarEventType / RadarPriorityType / RADAR_CELL_* / MAX_RADAR_EVENTS
//! - Common/System/Radar.cpp color table / createEvent / tryEvent residual
//! - RadarUpdate / RadarUpgrade (RadarExtendTime **4000**ms CC residual)
//! - StealthUpdate.h StealthLevel bits + ctor defaults + INVALID_OPACITY
//! - StealthDetectorUpdate.h ctor defaults + DetectionRate/Range residual
//! - StealthDetectorUpdate.cpp markAsDetected(updateRate+1/+2) + radar spot feedback
//! - Drawable.h StealthLookType ordinals; GlobalData StealthFriendlyOpacity **50%**
//! - ThingTemplate VisionRange / ShroudClearingRange / ShroudRevealToAll defaults
//! - AI.h AI_VISIONFACTOR_* + AIData.ini Guard/Alert/Aggressive vision modifiers
//! - DynamicShroudClearingRangeUpdate defaults + GRID_FX_DECAL_COUNT **30**
//!
//! Fail-closed:
//! - Not full W3DRadar GPU atlas / event marker draw residual
//! - Not full StealthUpdate exclusive allowedToStealth matrix / disguise path
//! - Not full StealthDetectorUpdate partition iterate / IR particle GPU residual
//! - Not full PartitionManager looker refresh / FOW multi-layer streaming
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Logic frames per second residual (host fixed step).
/// C++ MiscAudio m_radarInfiltrationSound residual honesty.
pub const RADAR_INFILTRATION_AUDIO: &str = "RadarInfiltration";

/// C++ RADAR:UnitUnderAttack residual.
pub const RADAR_MSG_UNIT_UNDER_ATTACK: &str = "RADAR:UnitUnderAttack";
/// C++ RADAR:HarvesterUnderAttack residual.
pub const RADAR_MSG_HARVESTER_UNDER_ATTACK: &str = "RADAR:HarvesterUnderAttack";
/// C++ RADAR:StructureUnderAttack residual.
pub const RADAR_MSG_STRUCTURE_UNDER_ATTACK: &str = "RADAR:StructureUnderAttack";
/// C++ RADAR:UnderAttack residual (generic).
pub const RADAR_MSG_UNDER_ATTACK: &str = "RADAR:UnderAttack";
/// C++ MiscAudio m_radarStructureUnderAttackSound residual.
pub const RADAR_AUDIO_STRUCTURE_UNDER_ATTACK: &str = "RadarStructureUnderAttack";
/// C++ MiscAudio m_radarHarvesterUnderAttackSound residual.
pub const RADAR_AUDIO_HARVESTER_UNDER_ATTACK: &str = "RadarHarvesterUnderAttack";

pub const RADAR_STEALTH_VISION_LOGIC_FPS: f32 = 30.0;

/// Convert msec residual → logic frames @ 30 FPS (exact for multiples of 100/1000).
#[inline]
pub fn residual_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * RADAR_STEALTH_VISION_LOGIC_FPS / 1000.0).round() as u32
}

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

// ---------------------------------------------------------------------------
// 1. Radar residual deepen (beyond Wave 63 provider / Wave 72 scan)
// ---------------------------------------------------------------------------

/// C++ `RADAR_CELL_WIDTH` residual (Radar.h).
pub const RADAR_CELL_WIDTH_RESIDUAL: u32 = 128;
/// C++ `RADAR_CELL_HEIGHT` residual (Radar.h).
pub const RADAR_CELL_HEIGHT_RESIDUAL: u32 = 128;
/// C++ `MAX_RADAR_EVENTS` residual (Radar.h).
pub const MAX_RADAR_EVENTS_RESIDUAL: usize = 64;
/// C++ `createEvent` default `secondsToLive` residual.
pub const RADAR_EVENT_DEFAULT_SECONDS_TO_LIVE_RESIDUAL: f32 = 4.0;
/// C++ `internalCreateEvent` fade residual (seconds before die to start fade).
pub const RADAR_EVENT_FADE_SECONDS_BEFORE_DIE_RESIDUAL: f32 = 0.5;
/// C++ `RADAR_QUEUE_TERRAIN_REFRESH_DELAY` residual (LOGICFRAMES_PER_SECOND * 3).
pub const RADAR_TERRAIN_REFRESH_DELAY_FRAMES_RESIDUAL: u32 = 90;
/// C++ player-event color darkScale residual.
pub const RADAR_PLAYER_EVENT_DARK_SCALE_RESIDUAL: f32 = 0.75;
/// Retail CommandCenter RadarExtendTime residual (msec).
pub const RADAR_EXTEND_TIME_MS_RESIDUAL: u32 = 4000;
/// RadarExtendTime frames residual (4000 ms → 120).
pub const RADAR_EXTEND_TIME_FRAMES_RESIDUAL: u32 = 120;
/// C++ RadarUpgrade DisableProof default residual.
pub const RADAR_UPGRADE_DISABLE_PROOF_DEFAULT_RESIDUAL: bool = false;
/// C++ RadarUpdate m_radarActive / m_extendComplete ctor residual.
pub const RADAR_UPDATE_ACTIVE_CTOR_RESIDUAL: bool = false;

/// C++ `RADAR_EVENT_NUM_EVENTS` residual (keep-last sentinel).
pub const RADAR_EVENT_NUM_EVENTS_RESIDUAL: usize = 11;

/// Ordered C++ `RadarEventType` residual names (index = discriminant).
pub const RADAR_EVENT_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "RADAR_EVENT_INVALID",             // 0
    "RADAR_EVENT_CONSTRUCTION",        // 1
    "RADAR_EVENT_UPGRADE",             // 2
    "RADAR_EVENT_UNDER_ATTACK",        // 3
    "RADAR_EVENT_INFORMATION",         // 4
    "RADAR_EVENT_BEACON_PULSE",        // 5
    "RADAR_EVENT_INFILTRATION",        // 6
    "RADAR_EVENT_BATTLE_PLAN",         // 7
    "RADAR_EVENT_STEALTH_DISCOVERED",  // 8
    "RADAR_EVENT_STEALTH_NEUTRALIZED", // 9
    "RADAR_EVENT_FAKE",                // 10
];

/// C++ `RADAR_PRIORITY_NUM_PRIORITIES` residual (keep-last sentinel).
pub const RADAR_PRIORITY_NUM_PRIORITIES_RESIDUAL: usize = 5;

/// Ordered C++ `RadarPriorityNames` residual (Radar.h DEFINE_RADAR_PRIORITY_NAMES).
pub const RADAR_PRIORITY_NAME_TABLE_RESIDUAL: &[&str] = &[
    "INVALID",         // 0 RADAR_PRIORITY_INVALID
    "NOT_ON_RADAR",    // 1
    "STRUCTURE",       // 2
    "UNIT",            // 3
    "LOCAL_UNIT_ONLY", // 4
];

/// Radar event color residual (RGBA u8 pairs from Radar.cpp radarColorLookupTable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarEventColorResidual {
    pub event_index: u8,
    pub color1: [u8; 4],
    pub color2: [u8; 4],
}

/// Ordered residual color table (excludes INVALID terminator).
pub const RADAR_EVENT_COLOR_TABLE_RESIDUAL: &[RadarEventColorResidual] = &[
    RadarEventColorResidual {
        event_index: 1, // CONSTRUCTION
        color1: [128, 128, 255, 255],
        color2: [128, 255, 255, 255],
    },
    RadarEventColorResidual {
        event_index: 2, // UPGRADE
        color1: [128, 0, 64, 255],
        color2: [255, 185, 220, 255],
    },
    RadarEventColorResidual {
        event_index: 3, // UNDER_ATTACK
        color1: [255, 0, 0, 255],
        color2: [255, 128, 128, 255],
    },
    RadarEventColorResidual {
        event_index: 4, // INFORMATION
        color1: [255, 255, 0, 255],
        color2: [255, 255, 128, 255],
    },
    RadarEventColorResidual {
        event_index: 5, // BEACON_PULSE
        color1: [255, 255, 0, 255],
        color2: [255, 255, 128, 255],
    },
    RadarEventColorResidual {
        event_index: 6, // INFILTRATION
        color1: [0, 255, 255, 255],
        color2: [128, 255, 255, 255],
    },
    RadarEventColorResidual {
        event_index: 7, // BATTLE_PLAN
        color1: [255, 255, 255, 255],
        color2: [255, 255, 255, 255],
    },
    RadarEventColorResidual {
        event_index: 8, // STEALTH_DISCOVERED
        color1: [0, 255, 0, 255],
        color2: [0, 128, 0, 255],
    },
    RadarEventColorResidual {
        event_index: 9, // STEALTH_NEUTRALIZED
        color1: [0, 255, 0, 255],
        color2: [0, 128, 0, 255],
    },
    RadarEventColorResidual {
        event_index: 10, // FAKE
        color1: [0, 0, 0, 0],
        color2: [0, 0, 0, 0],
    },
];

/// Residual die-frame offset from create: `LOGICFRAMES_PER_SECOND * secondsToLive`.
#[inline]
pub fn radar_event_die_frame_offset_residual(seconds_to_live: f32) -> u32 {
    (RADAR_STEALTH_VISION_LOGIC_FPS * seconds_to_live).round() as u32
}

/// Residual fade-frame offset before die: `LOGICFRAMES_PER_SECOND * 0.5`.
#[inline]
pub fn radar_event_fade_frame_offset_residual() -> u32 {
    (RADAR_STEALTH_VISION_LOGIC_FPS * RADAR_EVENT_FADE_SECONDS_BEFORE_DIE_RESIDUAL).round() as u32
}

/// Visible radar priorities residual (isPriorityVisible): STRUCTURE / UNIT / LOCAL_UNIT_ONLY.
#[inline]
pub fn radar_priority_is_visible_residual(priority_index: usize) -> bool {
    matches!(priority_index, 2 | 3 | 4)
}

/// Wave 97 radar residual deepen honesty pack.
pub fn honesty_radar_residual_deepen_pack_wave97() -> bool {
    RADAR_CELL_WIDTH_RESIDUAL == 128
        && RADAR_CELL_HEIGHT_RESIDUAL == 128
        && MAX_RADAR_EVENTS_RESIDUAL == 64
        && (RADAR_EVENT_DEFAULT_SECONDS_TO_LIVE_RESIDUAL - 4.0).abs() < 1e-6
        && (RADAR_EVENT_FADE_SECONDS_BEFORE_DIE_RESIDUAL - 0.5).abs() < 1e-6
        && RADAR_TERRAIN_REFRESH_DELAY_FRAMES_RESIDUAL == 90
        && (RADAR_PLAYER_EVENT_DARK_SCALE_RESIDUAL - 0.75).abs() < 1e-6
        && RADAR_EXTEND_TIME_MS_RESIDUAL == 4000
        && RADAR_EXTEND_TIME_FRAMES_RESIDUAL == residual_ms_to_frames(RADAR_EXTEND_TIME_MS_RESIDUAL)
        && RADAR_EXTEND_TIME_FRAMES_RESIDUAL == 120
        && !RADAR_UPGRADE_DISABLE_PROOF_DEFAULT_RESIDUAL
        && !RADAR_UPDATE_ACTIVE_CTOR_RESIDUAL
        && RADAR_EVENT_NUM_EVENTS_RESIDUAL == 11
        && RADAR_EVENT_TYPE_NAME_TABLE_RESIDUAL.len() == 11
        && residual_name_index(RADAR_EVENT_TYPE_NAME_TABLE_RESIDUAL, "RADAR_EVENT_INVALID")
            == Some(0)
        && residual_name_index(
            RADAR_EVENT_TYPE_NAME_TABLE_RESIDUAL,
            "RADAR_EVENT_STEALTH_DISCOVERED",
        ) == Some(8)
        && residual_name_index(
            RADAR_EVENT_TYPE_NAME_TABLE_RESIDUAL,
            "RADAR_EVENT_STEALTH_NEUTRALIZED",
        ) == Some(9)
        && residual_name_index(RADAR_EVENT_TYPE_NAME_TABLE_RESIDUAL, "RADAR_EVENT_FAKE") == Some(10)
        && RADAR_PRIORITY_NUM_PRIORITIES_RESIDUAL == 5
        && RADAR_PRIORITY_NAME_TABLE_RESIDUAL.len() == 5
        && residual_name_index(RADAR_PRIORITY_NAME_TABLE_RESIDUAL, "STRUCTURE") == Some(2)
        && residual_name_index(RADAR_PRIORITY_NAME_TABLE_RESIDUAL, "UNIT") == Some(3)
        && residual_name_index(RADAR_PRIORITY_NAME_TABLE_RESIDUAL, "LOCAL_UNIT_ONLY") == Some(4)
        && !radar_priority_is_visible_residual(0)
        && !radar_priority_is_visible_residual(1)
        && radar_priority_is_visible_residual(2)
        && radar_priority_is_visible_residual(3)
        && radar_priority_is_visible_residual(4)
        && radar_event_die_frame_offset_residual(4.0) == 120
        && radar_event_fade_frame_offset_residual() == 15
        && RADAR_EVENT_COLOR_TABLE_RESIDUAL.len() == 10
        && RADAR_EVENT_COLOR_TABLE_RESIDUAL[0].event_index == 1
        && RADAR_EVENT_COLOR_TABLE_RESIDUAL[0].color1 == [128, 128, 255, 255]
        && RADAR_EVENT_COLOR_TABLE_RESIDUAL[2].event_index == 3
        && RADAR_EVENT_COLOR_TABLE_RESIDUAL[2].color1 == [255, 0, 0, 255]
        && RADAR_EVENT_COLOR_TABLE_RESIDUAL[7].event_index == 8
        && RADAR_EVENT_COLOR_TABLE_RESIDUAL[7].color1 == [0, 255, 0, 255]
        && RADAR_EVENT_COLOR_TABLE_RESIDUAL[8].event_index == 9
        && RADAR_EVENT_COLOR_TABLE_RESIDUAL[9].event_index == 10
}

// ---------------------------------------------------------------------------
// 2. Spotter residual peels (stealth discovery feedback path)
// ---------------------------------------------------------------------------

/// UI message residual when detector spots enemy stealth (local detector player).
pub const SPOTTER_MESSAGE_STEALTH_DISCOVERED: &str = "MESSAGE:StealthDiscovered";
/// UI message residual when own stealth is neutralized (local owner player).
pub const SPOTTER_MESSAGE_STEALTH_NEUTRALIZED: &str = "MESSAGE:StealthNeutralized";
/// MiscAudio residual key for discover feedback.
pub const SPOTTER_AUDIO_STEALTH_DISCOVERED: &str = "StealthDiscoveredSound";
/// MiscAudio residual key for neutralize feedback.
pub const SPOTTER_AUDIO_STEALTH_NEUTRALIZED: &str = "StealthNeutralizedSound";

/// C++ `tryEvent` closeEnoughDistance residual (world units).
pub const SPOTTER_TRY_EVENT_CLOSE_ENOUGH_DISTANCE_RESIDUAL: f32 = 250.0;
/// C++ `tryEvent` closeEnoughDistanceSq residual.
pub const SPOTTER_TRY_EVENT_CLOSE_ENOUGH_DISTANCE_SQ_RESIDUAL: f32 = 250.0 * 250.0;
/// C++ `tryEvent` framesBetweenEvents residual (10 seconds @ 30 FPS).
pub const SPOTTER_TRY_EVENT_FRAMES_BETWEEN_EVENTS_RESIDUAL: u32 = 300;

/// Detector primary markAsDetected frames residual: `updateRate + 1`.
#[inline]
pub fn spotter_mark_detected_primary_frames_residual(update_rate_frames: u32) -> u32 {
    update_rate_frames.saturating_add(1)
}

/// Garrisoned rider markAsDetected frames residual: `updateRate + 2`.
#[inline]
pub fn spotter_mark_detected_garrison_rider_frames_residual(update_rate_frames: u32) -> u32 {
    update_rate_frames.saturating_add(2)
}

/// StealthUpdate::markAsDetected residual expire frame.
///
/// - `num_frames == 0` → `now + stealth_delay` (INI StealthDelay residual)
/// - else → max(current_expires, now + num_frames)
#[inline]
pub fn spotter_detection_expires_frame_residual(
    now: u32,
    current_expires: u32,
    num_frames: u32,
    stealth_delay_frames: u32,
) -> u32 {
    if num_frames == 0 {
        now.saturating_add(stealth_delay_frames)
    } else {
        let candidate = now.saturating_add(num_frames);
        current_expires.max(candidate)
    }
}

/// Heat-vision second-pass opacity residual when spotted (non-mine).
pub const SPOTTER_SECOND_MATERIAL_PASS_OPACITY_RESIDUAL: f32 = 1.0;

/// RADAR_EVENT_STEALTH_DISCOVERED discriminant residual.
pub const SPOTTER_RADAR_EVENT_STEALTH_DISCOVERED_INDEX: usize = 8;
/// RADAR_EVENT_STEALTH_NEUTRALIZED discriminant residual.
pub const SPOTTER_RADAR_EVENT_STEALTH_NEUTRALIZED_INDEX: usize = 9;

/// Wave 97 spotter residual honesty pack.
pub fn honesty_spotter_residual_pack_wave97() -> bool {
    SPOTTER_MESSAGE_STEALTH_DISCOVERED == "MESSAGE:StealthDiscovered"
        && SPOTTER_MESSAGE_STEALTH_NEUTRALIZED == "MESSAGE:StealthNeutralized"
        && SPOTTER_AUDIO_STEALTH_DISCOVERED == "StealthDiscoveredSound"
        && SPOTTER_AUDIO_STEALTH_NEUTRALIZED == "StealthNeutralizedSound"
        && (SPOTTER_TRY_EVENT_CLOSE_ENOUGH_DISTANCE_RESIDUAL - 250.0).abs() < 1e-6
        && (SPOTTER_TRY_EVENT_CLOSE_ENOUGH_DISTANCE_SQ_RESIDUAL - 62_500.0).abs() < 1e-3
        && SPOTTER_TRY_EVENT_FRAMES_BETWEEN_EVENTS_RESIDUAL == 300
        && spotter_mark_detected_primary_frames_residual(15) == 16
        && spotter_mark_detected_primary_frames_residual(27) == 28
        && spotter_mark_detected_garrison_rider_frames_residual(15) == 17
        && spotter_detection_expires_frame_residual(100, 0, 0, 60) == 160
        && spotter_detection_expires_frame_residual(100, 200, 16, 60) == 200
        && spotter_detection_expires_frame_residual(100, 110, 16, 60) == 116
        && (SPOTTER_SECOND_MATERIAL_PASS_OPACITY_RESIDUAL - 1.0).abs() < 1e-6
        && SPOTTER_RADAR_EVENT_STEALTH_DISCOVERED_INDEX == 8
        && SPOTTER_RADAR_EVENT_STEALTH_NEUTRALIZED_INDEX == 9
        && RADAR_MSG_UNIT_UNDER_ATTACK == "RADAR:UnitUnderAttack"
        && RADAR_MSG_STRUCTURE_UNDER_ATTACK == "RADAR:StructureUnderAttack"
        && SPOTTER_TRY_EVENT_FRAMES_BETWEEN_EVENTS_RESIDUAL == 300
        && residual_name_index(
            RADAR_EVENT_TYPE_NAME_TABLE_RESIDUAL,
            "RADAR_EVENT_STEALTH_DISCOVERED",
        ) == Some(SPOTTER_RADAR_EVENT_STEALTH_DISCOVERED_INDEX)
        && residual_name_index(
            RADAR_EVENT_TYPE_NAME_TABLE_RESIDUAL,
            "RADAR_EVENT_STEALTH_NEUTRALIZED",
        ) == Some(SPOTTER_RADAR_EVENT_STEALTH_NEUTRALIZED_INDEX)
}

/// C++ AmericaJetStealthFighter StealthDelay residual (msec).
pub const STEALTH_FIGHTER_STEALTH_DELAY_MS_RESIDUAL: u32 = 2_000;
/// StealthDelay 2000ms → 60 frames @ 30 FPS.
pub const STEALTH_FIGHTER_STEALTH_DELAY_FRAMES_RESIDUAL: u32 = 60;
/// Retail StealthForbiddenConditions = ATTACKING residual.
pub const STEALTH_FIGHTER_FORBIDDEN_ATTACKING_RESIDUAL: bool = true;

/// C++ mine / demo-trap StealthDelay residual (immediate cloak).
pub const MINE_STEALTH_DELAY_FRAMES_RESIDUAL: u32 = 0;
/// C++ StealthUpdate::update mine special-case `setEffectiveOpacity(0, 0)`.
pub const MINE_STEALTH_OPACITY_RESIDUAL: f32 = 0.0;

/// C++ StealthDetectorUpdate.cpp:139-166 containment gate.
/// Garrisonable container uses CanDetectWhileGarrisoned; any other container
/// uses CanDetectWhileTransported. Both default false.
#[inline]
pub fn detector_can_scan_while_contained_residual(
    is_contained: bool,
    container_is_garrisonable: bool,
    can_detect_while_garrisoned: bool,
    can_detect_while_transported: bool,
) -> bool {
    if !is_contained {
        return true;
    }
    if container_is_garrisonable {
        can_detect_while_garrisoned
    } else {
        can_detect_while_transported
    }
}

/// Retail hero StealthDelay frames (Burton/Kell 60, Lotus/Saboteur/Hijacker 75).
pub fn hero_stealth_delay_frames_residual(template_name: &str) -> u32 {
    let n = template_name.to_ascii_lowercase();
    if n.contains("burton") || n.contains("colonel") {
        crate::game_logic::host_colonel_burton::BURTON_STEALTH_DELAY_FRAMES
    } else if n.contains("jarmen") || n.contains("kell") {
        crate::game_logic::host_jarmen_kell::JARMEN_STEALTH_DELAY_FRAMES
    } else if crate::game_logic::host_hero_abilities::is_black_lotus_template(template_name)
        || n.contains("lotus")
    {
        crate::game_logic::host_hero_abilities::LOTUS_STEALTH_DELAY_FRAMES
    } else {
        crate::game_logic::host_saboteur::SABOTEUR_STEALTH_DELAY_FRAMES
    }
}

/// C++ allowedToStealth ATTACKING residual for Stealth Fighter.
#[inline]
pub fn stealth_fighter_allowed_to_stealth_residual(attacking: bool) -> bool {
    !STEALTH_FIGHTER_FORBIDDEN_ATTACKING_RESIDUAL || !attacking
}

/// Whether a mine-kind object should use the 0-opacity stealth residual.
#[inline]
pub fn is_mine_stealth_kind_residual(
    is_mine: bool,
    is_demo_trap: bool,
    has_mine_data: bool,
) -> bool {
    is_mine || is_demo_trap || has_mine_data
}

// ---------------------------------------------------------------------------
// 3. Stealth residual deepen (StealthUpdate ctor + level bits + samples)
// ---------------------------------------------------------------------------

/// C++ `INVALID_OPACITY` residual (StealthUpdate.h).
pub const STEALTH_INVALID_OPACITY_RESIDUAL: f32 = -1.0;

/// StealthLevel bit residual flags (StealthUpdate.h).
pub const STEALTH_NOT_WHILE_ATTACKING_RESIDUAL: u32 = 0x0000_0001;
pub const STEALTH_NOT_WHILE_MOVING_RESIDUAL: u32 = 0x0000_0002;
pub const STEALTH_NOT_WHILE_USING_ABILITY_RESIDUAL: u32 = 0x0000_0004;
pub const STEALTH_NOT_WHILE_FIRING_PRIMARY_RESIDUAL: u32 = 0x0000_0008;
pub const STEALTH_NOT_WHILE_FIRING_SECONDARY_RESIDUAL: u32 = 0x0000_0010;
pub const STEALTH_NOT_WHILE_FIRING_TERTIARY_RESIDUAL: u32 = 0x0000_0020;
pub const STEALTH_ONLY_WITH_BLACK_MARKET_RESIDUAL: u32 = 0x0000_0040;
pub const STEALTH_NOT_WHILE_TAKING_DAMAGE_RESIDUAL: u32 = 0x0000_0080;
pub const STEALTH_NOT_WHILE_FIRING_WEAPON_RESIDUAL: u32 = STEALTH_NOT_WHILE_FIRING_PRIMARY_RESIDUAL
    | STEALTH_NOT_WHILE_FIRING_SECONDARY_RESIDUAL
    | STEALTH_NOT_WHILE_FIRING_TERTIARY_RESIDUAL;
pub const STEALTH_NOT_WHILE_RIDERS_ATTACKING_RESIDUAL: u32 = 0x0000_0100;

/// C++ `TheStealthLevelNames` residual (DEFINE_STEALTHLEVEL_NAMES order).
pub const STEALTH_LEVEL_NAME_TABLE_RESIDUAL: &[&str] = &[
    "ATTACKING",
    "MOVING",
    "USING_ABILITY",
    "FIRING_PRIMARY",
    "FIRING_SECONDARY",
    "FIRING_TERTIARY",
    "NO_BLACK_MARKET",
    "TAKING_DAMAGE",
    "RIDERS_ATTACKING",
];

/// C++ StealthUpdateModuleData ctor residual defaults.
pub const STEALTH_DELAY_CTOR_DEFAULT_RESIDUAL: u32 = u32::MAX;
pub const STEALTH_LEVEL_CTOR_DEFAULT_RESIDUAL: u32 = 0;
pub const STEALTH_SPEED_CTOR_DEFAULT_RESIDUAL: f32 = 0.0;
pub const STEALTH_FRIENDLY_OPACITY_MIN_CTOR_RESIDUAL: f32 = 0.5;
pub const STEALTH_FRIENDLY_OPACITY_MAX_CTOR_RESIDUAL: f32 = 1.0;
pub const STEALTH_PULSE_FRAMES_CTOR_RESIDUAL: u32 = 30;
pub const STEALTH_INNATE_CTOR_DEFAULT_RESIDUAL: bool = true;
pub const STEALTH_TEAM_DISGUISED_CTOR_DEFAULT_RESIDUAL: bool = false;
pub const STEALTH_PULSE_PHASE_RATE_CTOR_RESIDUAL: f32 = 0.2;
/// GameData / GlobalData StealthFriendlyOpacity residual (50%).
pub const STEALTH_FRIENDLY_OPACITY_GAMEDATA_RESIDUAL: f32 = 0.5;

/// Sample unit residual rows (template, delay_ms, delay_frames, forbidden bits).
#[derive(Debug, Clone, Copy)]
pub struct StealthUnitSampleResidual {
    pub template: &'static str,
    pub delay_ms: u32,
    pub delay_frames: u32,
    pub forbidden_bits: u32,
    pub innate: bool,
    pub friendly_opacity_min: f32,
    pub friendly_opacity_max: f32,
}

/// Representative retail StealthUpdate residual samples.
pub const STEALTH_UNIT_SAMPLE_TABLE_RESIDUAL: &[StealthUnitSampleResidual] = &[
    // AmericaInfantry Pathfinder
    StealthUnitSampleResidual {
        template: "AmericaInfantryPathfinder",
        delay_ms: 0,
        delay_frames: 0,
        forbidden_bits: STEALTH_NOT_WHILE_MOVING_RESIDUAL,
        innate: true,
        friendly_opacity_min: 0.30,
        friendly_opacity_max: 0.80,
    },
    // AmericaInfantry ColonelBurton
    StealthUnitSampleResidual {
        template: "AmericaInfantryColonelBurton",
        delay_ms: 2000,
        delay_frames: 60,
        forbidden_bits: STEALTH_NOT_WHILE_FIRING_PRIMARY_RESIDUAL,
        innate: true,
        friendly_opacity_min: 0.5,
        friendly_opacity_max: 1.0,
    },
    // GLAInfantry Rebel (ATTACKING | USING_ABILITY)
    StealthUnitSampleResidual {
        template: "GLAInfantryRebel",
        delay_ms: 2500,
        delay_frames: 75,
        forbidden_bits: STEALTH_NOT_WHILE_ATTACKING_RESIDUAL
            | STEALTH_NOT_WHILE_USING_ABILITY_RESIDUAL,
        innate: true,
        friendly_opacity_min: 0.5,
        friendly_opacity_max: 1.0,
    },
    // ChinaInfantry BlackLotus
    StealthUnitSampleResidual {
        template: "ChinaInfantryBlackLotus",
        delay_ms: 2500,
        delay_frames: 75,
        forbidden_bits: STEALTH_NOT_WHILE_USING_ABILITY_RESIDUAL,
        innate: true,
        friendly_opacity_min: 0.5,
        friendly_opacity_max: 1.0,
    },
    // GLAVehicleRadarVan CamoNetting residual (StealthDelay 2500 when upgraded)
    StealthUnitSampleResidual {
        template: "GLACamoNetting",
        delay_ms: 2500,
        delay_frames: 75,
        forbidden_bits: 0,
        innate: true,
        friendly_opacity_min: 0.5,
        friendly_opacity_max: 1.0,
    },
];

/// C++ StealthLookType residual names (Drawable.h order).
pub const STEALTH_LOOK_NAME_TABLE_RESIDUAL: &[&str] = &[
    "STEALTHLOOK_NONE",                      // 0
    "STEALTHLOOK_VISIBLE_FRIENDLY",          // 1
    "STEALTHLOOK_DISGUISED_ENEMY",           // 2
    "STEALTHLOOK_VISIBLE_DETECTED",          // 3
    "STEALTHLOOK_VISIBLE_FRIENDLY_DETECTED", // 4
    "STEALTHLOOK_INVISIBLE",                 // 5
];

/// Wave 97 stealth residual deepen honesty pack.
pub fn honesty_stealth_residual_deepen_pack_wave97() -> bool {
    (STEALTH_INVALID_OPACITY_RESIDUAL + 1.0).abs() < 1e-6
        && STEALTH_NOT_WHILE_ATTACKING_RESIDUAL == 0x1
        && STEALTH_NOT_WHILE_MOVING_RESIDUAL == 0x2
        && STEALTH_NOT_WHILE_USING_ABILITY_RESIDUAL == 0x4
        && STEALTH_NOT_WHILE_FIRING_PRIMARY_RESIDUAL == 0x8
        && STEALTH_NOT_WHILE_FIRING_SECONDARY_RESIDUAL == 0x10
        && STEALTH_NOT_WHILE_FIRING_TERTIARY_RESIDUAL == 0x20
        && STEALTH_ONLY_WITH_BLACK_MARKET_RESIDUAL == 0x40
        && STEALTH_NOT_WHILE_TAKING_DAMAGE_RESIDUAL == 0x80
        && STEALTH_NOT_WHILE_FIRING_WEAPON_RESIDUAL == 0x38
        && STEALTH_NOT_WHILE_RIDERS_ATTACKING_RESIDUAL == 0x100
        && STEALTH_LEVEL_NAME_TABLE_RESIDUAL.len() == 9
        && residual_name_index(STEALTH_LEVEL_NAME_TABLE_RESIDUAL, "ATTACKING") == Some(0)
        && residual_name_index(STEALTH_LEVEL_NAME_TABLE_RESIDUAL, "MOVING") == Some(1)
        && residual_name_index(STEALTH_LEVEL_NAME_TABLE_RESIDUAL, "USING_ABILITY") == Some(2)
        && residual_name_index(STEALTH_LEVEL_NAME_TABLE_RESIDUAL, "FIRING_PRIMARY") == Some(3)
        && residual_name_index(STEALTH_LEVEL_NAME_TABLE_RESIDUAL, "RIDERS_ATTACKING") == Some(8)
        && STEALTH_DELAY_CTOR_DEFAULT_RESIDUAL == u32::MAX
        && STEALTH_LEVEL_CTOR_DEFAULT_RESIDUAL == 0
        && (STEALTH_SPEED_CTOR_DEFAULT_RESIDUAL - 0.0).abs() < 1e-6
        && (STEALTH_FRIENDLY_OPACITY_MIN_CTOR_RESIDUAL - 0.5).abs() < 1e-6
        && (STEALTH_FRIENDLY_OPACITY_MAX_CTOR_RESIDUAL - 1.0).abs() < 1e-6
        && STEALTH_PULSE_FRAMES_CTOR_RESIDUAL == 30
        && STEALTH_INNATE_CTOR_DEFAULT_RESIDUAL
        && !STEALTH_TEAM_DISGUISED_CTOR_DEFAULT_RESIDUAL
        && (STEALTH_PULSE_PHASE_RATE_CTOR_RESIDUAL - 0.2).abs() < 1e-6
        && (STEALTH_FRIENDLY_OPACITY_GAMEDATA_RESIDUAL - 0.5).abs() < 1e-6
        && STEALTH_UNIT_SAMPLE_TABLE_RESIDUAL.len() == 5
        && STEALTH_UNIT_SAMPLE_TABLE_RESIDUAL[0].template == "AmericaInfantryPathfinder"
        && STEALTH_UNIT_SAMPLE_TABLE_RESIDUAL[0].delay_frames == 0
        && STEALTH_UNIT_SAMPLE_TABLE_RESIDUAL[0].forbidden_bits == STEALTH_NOT_WHILE_MOVING_RESIDUAL
        && (STEALTH_UNIT_SAMPLE_TABLE_RESIDUAL[0].friendly_opacity_min - 0.30).abs() < 1e-6
        && residual_ms_to_frames(STEALTH_UNIT_SAMPLE_TABLE_RESIDUAL[1].delay_ms)
            == STEALTH_UNIT_SAMPLE_TABLE_RESIDUAL[1].delay_frames
        && residual_ms_to_frames(2500) == 75
        && residual_ms_to_frames(2000) == 60
        && STEALTH_LOOK_NAME_TABLE_RESIDUAL.len() == 6
        && residual_name_index(STEALTH_LOOK_NAME_TABLE_RESIDUAL, "STEALTHLOOK_NONE") == Some(0)
        && residual_name_index(STEALTH_LOOK_NAME_TABLE_RESIDUAL, "STEALTHLOOK_INVISIBLE") == Some(5)
}

// ---------------------------------------------------------------------------
// 4. Detector residual deepen (StealthDetectorUpdate defaults + samples)
// ---------------------------------------------------------------------------

/// C++ StealthDetectorUpdateModuleData ctor residual defaults.
pub const DETECTOR_UPDATE_RATE_CTOR_DEFAULT_RESIDUAL: u32 = 1;
pub const DETECTOR_RANGE_CTOR_DEFAULT_RESIDUAL: f32 = 0.0;
pub const DETECTOR_INITIALLY_DISABLED_CTOR_DEFAULT_RESIDUAL: bool = false;
pub const DETECTOR_CAN_DETECT_WHILE_GARRISONED_CTOR_DEFAULT_RESIDUAL: bool = false;
pub const DETECTOR_CAN_DETECT_WHILE_TRANSPORTED_CTOR_DEFAULT_RESIDUAL: bool = false;

/// C++ `PingSound` residual (Listening Outpost / Troop Crawler / Sentry / Pathfinder).
pub const DETECTOR_IR_PING_SOUND: &str = "IRPing";
/// C++ `LoudPingSound` residual when the scan spotted someone.
pub const DETECTOR_IR_LOUD_PING_SOUND: &str = "IRPingLoud";
/// C++ `IRBeaconParticleSysName` residual.
pub const DETECTOR_IR_BEACON_PARTICLE: &str = "IRLenzflare";
/// C++ `IRParticleSysName` residual (quiet scan).
pub const DETECTOR_IR_PING_PARTICLE: &str = "IRDetectPing";
/// C++ `IRBrightParticleSysName` residual (spot scan).
pub const DETECTOR_IR_BRIGHT_PARTICLE: &str = "IRDetectPingBright";
/// C++ `IRGridParticleSysName` residual — leftover-refreshed every DetectionRate scan.
pub const DETECTOR_IR_GRID_PARTICLE: &str = "IRDetectGrid";

/// Common DetectionRate residual (msec) — most infantry/vehicle detectors.
pub const DETECTOR_RATE_COMMON_MS_RESIDUAL: u32 = 500;
/// Common DetectionRate frames residual (500 ms → 15).
pub const DETECTOR_RATE_COMMON_FRAMES_RESIDUAL: u32 = 15;
/// Listening Outpost / mine DetectionRate residual (msec).
pub const DETECTOR_RATE_SLOW_MS_RESIDUAL: u32 = 900;
/// Slow DetectionRate frames residual (900 ms → 27).
pub const DETECTOR_RATE_SLOW_FRAMES_RESIDUAL: u32 = 27;

/// Sample detector residual rows.
#[derive(Debug, Clone, Copy)]
pub struct DetectorUnitSampleResidual {
    pub template: &'static str,
    pub rate_ms: u32,
    pub rate_frames: u32,
    /// 0.0 means fallback to VisionRange residual.
    pub detection_range: f32,
    pub vision_range_fallback: f32,
    pub can_garrisoned: bool,
    pub can_transported: bool,
}

/// Representative retail StealthDetectorUpdate residual samples.
pub const DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL: &[DetectorUnitSampleResidual] = &[
    // AmericaInfantry Pathfinder — DetectionRange unset → Vision 200
    DetectorUnitSampleResidual {
        template: "AmericaInfantryPathfinder",
        rate_ms: 500,
        rate_frames: 15,
        detection_range: 0.0,
        vision_range_fallback: 200.0,
        can_garrisoned: false,
        can_transported: false,
    },
    // GLAInfantry Hijacker — DetectionRange 200
    DetectorUnitSampleResidual {
        template: "GLAInfantryHijacker",
        rate_ms: 500,
        rate_frames: 15,
        detection_range: 200.0,
        vision_range_fallback: 150.0,
        can_garrisoned: false,
        can_transported: false,
    },
    // ChinaVehicleListeningOutpost — DetectionRate 900, range unset → Vision
    DetectorUnitSampleResidual {
        template: "ChinaVehicleListeningOutpost",
        rate_ms: 900,
        rate_frames: 27,
        detection_range: 0.0,
        vision_range_fallback: 175.0,
        can_garrisoned: false,
        can_transported: false,
    },
    // AmericaVehicleSentryDrone — DetectionRate 900 / range 225 residual
    DetectorUnitSampleResidual {
        template: "AmericaVehicleSentryDrone",
        rate_ms: 900,
        rate_frames: 27,
        detection_range: 225.0,
        vision_range_fallback: 150.0,
        can_garrisoned: false,
        can_transported: false,
    },
    // ChinaStrategyCenter ModuleTag_16 — DetectionRate 500 / range 150
    DetectorUnitSampleResidual {
        template: "ChinaStrategyCenter",
        rate_ms: 500,
        rate_frames: 15,
        detection_range: 150.0,
        vision_range_fallback: 300.0,
        can_garrisoned: false,
        can_transported: false,
    },
    // RadarVanPing OCL residual — DetectionRate 500 / range = Vision 150
    DetectorUnitSampleResidual {
        template: "RadarVanPing",
        rate_ms: 500,
        rate_frames: 15,
        detection_range: 0.0,
        vision_range_fallback: 150.0,
        can_garrisoned: false,
        can_transported: false,
    },
];

/// Effective detection range residual: DetectionRange if > 0 else VisionRange.
#[inline]
pub fn detector_effective_range_residual(detection_range: f32, vision_range: f32) -> f32 {
    if detection_range > 0.0 {
        detection_range
    } else {
        vision_range
    }
}

/// C++ `PartitionFilterAcceptByKindOf` / leftover `stealth_detector_kindof_allows`.
#[inline]
pub fn detector_accepts_kindof_residual(
    target_mask: u128,
    extra_required: u128,
    extra_forbidden: u128,
) -> bool {
    gamelogic::object::behavior::stealth_detector_kindof_allows(
        target_mask,
        extra_required,
        extra_forbidden,
    )
}

/// C++ `KindOf.h` bit for one live host KindOf token.
#[inline]
pub fn live_kind_of_cpp_mask(kind: crate::game_logic::KindOf) -> u128 {
    use crate::game_logic::KindOf;
    use game_engine::common::system::kind_of::KindOfMask as M;
    match kind {
        KindOf::Structure => M::STRUCTURE.bits(),
        KindOf::Infantry => M::INFANTRY.bits(),
        KindOf::Vehicle => M::VEHICLE.bits(),
        KindOf::Aircraft => M::AIRCRAFT.bits(),
        KindOf::Projectile => M::PROJECTILE.bits(),
        KindOf::Selectable => M::SELECTABLE.bits(),
        KindOf::Attackable => M::CAN_ATTACK.bits(),
        KindOf::CommandCenter => M::COMMANDCENTER.bits(),
        KindOf::Hero => M::HERO.bits(),
        KindOf::SupplyCenter => M::FS_SUPPLY_CENTER.bits(),
        KindOf::PowerPlant => M::FS_POWER.bits(),
        KindOf::FSBarracks => M::FS_BARRACKS.bits(),
        KindOf::FSWarFactory => M::FS_WARFACTORY.bits(),
        KindOf::FSAirfield => M::FS_AIRFIELD.bits(),
        KindOf::FSInternetCenter => M::FS_INTERNET_CENTER.bits(),
        KindOf::FSPower => M::FS_POWER.bits(),
        KindOf::FSBaseDefense => M::FS_BASE_DEFENSE.bits(),
        KindOf::FSSupplyDropzone => M::FS_SUPPLY_DROPZONE.bits(),
        KindOf::FSSupplyCenter => M::FS_SUPPLY_CENTER.bits(),
        KindOf::FSSuperweapon => M::FS_SUPERWEAPON.bits(),
        KindOf::FSStrategyCenter => M::FS_STRATEGY_CENTER.bits(),
        KindOf::FSFake => M::FS_FAKE.bits(),
        KindOf::FSTechnology => M::FS_TECHNOLOGY.bits(),
        KindOf::FSBlackMarket => M::FS_BLACK_MARKET.bits(),
        KindOf::FSAdvancedTech => M::FS_ADVANCED_TECH.bits(),
        KindOf::Powered => M::POWERED.bits(),
        KindOf::AttackNeedsLineOfSight => M::ATTACK_NEEDS_LINE_OF_SIGHT.bits(),
        KindOf::Immobile => M::IMMOBILE.bits(),
        KindOf::CanBeRepulsed => M::CAN_BE_REPULSED.bits(),
        KindOf::CannotRetaliate => M::CANNOT_RETALIATE.bits(),
        KindOf::Drone => M::DRONE.bits(),
        KindOf::IgnoredInGui => M::IGNORED_IN_GUI.bits(),
        KindOf::Salvager => M::SALVAGER.bits(),
        KindOf::WeaponSalvager => M::WEAPON_SALVAGER.bits(),
        KindOf::ArmorSalvager => M::ARMOR_SALVAGER.bits(),
        KindOf::AircraftPathAround => M::AIRCRAFT_PATH_AROUND.bits(),
        KindOf::WaveGuide => M::WAVEGUIDE.bits(),
        KindOf::Dozer => M::DOZER.bits(),
        KindOf::Harvester => M::HARVESTER.bits(),
        KindOf::Unattackable => M::UNATTACKABLE.bits(),
        KindOf::Mine => M::MINE.bits(),
        KindOf::DemoTrap => M::DEMOTRAP.bits(),
        KindOf::SmallMissile => M::SMALL_MISSILE.bits(),
        KindOf::BallisticMissile => M::BALLISTIC_MISSILE.bits(),
        KindOf::Parachute => M::PARACHUTE.bits(),
        KindOf::Disguiser => M::DISGUISER.bits(),
        KindOf::RepairPad => M::REPAIR_PAD.bits(),
        KindOf::HealPad => M::HEAL_PAD.bits(),
        KindOf::MoneyHacker => M::MONEY_HACKER.bits(),
        KindOf::SupplySource => M::SUPPLY_SOURCE.bits(),
        KindOf::CannotBuildNearSupplies => M::CANNOT_BUILD_NEAR_SUPPLIES.bits(),
        KindOf::Crate => M::CRATE.bits(),
        KindOf::IgnoresSelectAll => M::IGNORES_SELECT_ALL.bits(),
        KindOf::AlwaysSelectable => M::ALWAYS_SELECTABLE.bits(),
        KindOf::MpCountForVictory => M::MP_COUNT_FOR_VICTORY.bits(),
        KindOf::Score => M::SCORE.bits(),
        KindOf::ScoreCreate => M::SCORE_CREATE.bits(),
        KindOf::ScoreDestroy => M::SCORE_DESTROY.bits(),
        KindOf::CanSeeThrough => M::CAN_SEE_THROUGH_STRUCTURE.bits(),
        KindOf::NoGarrison => M::NO_GARRISON.bits(),
        KindOf::GarrisonableUntilDestroyed => M::GARRISONABLE_UNTIL_DESTROYED.bits(),
        KindOf::Boat => M::BOAT.bits(),
        KindOf::Transport => M::TRANSPORT.bits(),
        KindOf::ImmuneToCapture => M::IMMUNE_TO_CAPTURE.bits(),
        KindOf::DefensiveWall => M::DEFENSIVE_WALL.bits(),
        KindOf::WalkOnTopOfWall => M::WALK_ON_TOP_OF_WALL.bits(),
        KindOf::Bridge => M::BRIDGE.bits(),
        KindOf::BridgeTower => M::BRIDGE_TOWER.bits(),
        KindOf::StealthGarrison => M::STEALTH_GARRISON.bits(),
        KindOf::TechBuilding => M::TECH_BUILDING.bits(),
        KindOf::LandmarkBridge => M::LANDMARK_BRIDGE.bits(),
        KindOf::AutoRallypoint => M::AUTO_RALLYPOINT.bits(),
        KindOf::MobNexus => M::MOB_NEXUS.bits(),
        KindOf::NoCollide => M::NO_COLLIDE.bits(),
        KindOf::ForceAttackable => M::FORCEATTACKABLE.bits(),
        KindOf::Shrubbery => M::SHRUBBERY.bits(),
        KindOf::ClearedByBuild => M::CLEARED_BY_BUILD.bits(),
        KindOf::Inert => M::INERT.bits(),
        KindOf::BlastCrater => M::BLAST_CRATER.bits(),
        KindOf::HugeVehicle => M::HUGE_VEHICLE.bits(),
        KindOf::CleanupHazard => M::CLEANUP_HAZARD.bits(),

        KindOf::Resource | KindOf::Worker | KindOf::Harvestable => 0,
    }
}

/// C++ `ThingTemplate::m_kindof` mask from live host KindOf bits.
#[inline]
pub fn live_object_kind_of_cpp_mask<'a>(
    kinds: impl IntoIterator<Item = &'a crate::game_logic::KindOf>,
) -> u128 {
    kinds
        .into_iter()
        .fold(0u128, |acc, k| acc | live_kind_of_cpp_mask(*k))
}

fn parse_extra_detect_kindof_ini(value: &str) -> u128 {
    use game_engine::common::system::kind_of::KindOfMask;
    KindOfMask::parse_ini(KindOfMask::empty(), value)
        .map(|m| m.bits())
        .unwrap_or(0)
}

/// Leftover ThingFactory `StealthDetectorUpdate` ExtraRequired/ForbiddenKindOf.
pub fn extra_detect_kindof_from_leftover_template(template_name: &str) -> Option<(u128, u128)> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    for entry in tmpl.get_behavior_module_info().iter() {
        if !entry
            .name
            .as_str()
            .eq_ignore_ascii_case("StealthDetectorUpdate")
        {
            continue;
        }
        if let Some(data) = entry
            .data
            .downcast_ref::<gamelogic::object::behavior::StealthDetectorUpdateModuleData>()
        {
            return Some((data.extra_detect_kindof, data.extra_detect_kindof_not));
        }
        let required = parse_extra_detect_kindof_ini(
            entry
                .data
                .get_ini_field("ExtraRequiredKindOf")
                .unwrap_or(""),
        );
        let forbidden = parse_extra_detect_kindof_ini(
            entry
                .data
                .get_ini_field("ExtraForbiddenKindOf")
                .unwrap_or(""),
        );
        return Some((required, forbidden));
    }
    None
}

/// ExtraRequired/ForbiddenKindOf for a detector: leftover module, else host fields.
#[inline]
pub fn extra_detect_kindof_for_detector(
    template_name: &str,
    extra_required: u128,
    extra_forbidden: u128,
) -> (u128, u128) {
    extra_detect_kindof_from_leftover_template(template_name)
        .unwrap_or((extra_required, extra_forbidden))
}

/// Wave 97 detector residual deepen honesty pack.
pub fn honesty_detector_residual_deepen_pack_wave97() -> bool {
    DETECTOR_UPDATE_RATE_CTOR_DEFAULT_RESIDUAL == 1
        && (DETECTOR_RANGE_CTOR_DEFAULT_RESIDUAL - 0.0).abs() < 1e-6
        && !DETECTOR_INITIALLY_DISABLED_CTOR_DEFAULT_RESIDUAL
        && !DETECTOR_CAN_DETECT_WHILE_GARRISONED_CTOR_DEFAULT_RESIDUAL
        && !DETECTOR_CAN_DETECT_WHILE_TRANSPORTED_CTOR_DEFAULT_RESIDUAL
        && DETECTOR_RATE_COMMON_MS_RESIDUAL == 500
        && DETECTOR_RATE_COMMON_FRAMES_RESIDUAL
            == residual_ms_to_frames(DETECTOR_RATE_COMMON_MS_RESIDUAL)
        && DETECTOR_RATE_COMMON_FRAMES_RESIDUAL == 15
        && DETECTOR_RATE_SLOW_MS_RESIDUAL == 900
        && DETECTOR_RATE_SLOW_FRAMES_RESIDUAL == residual_ms_to_frames(DETECTOR_RATE_SLOW_MS_RESIDUAL)
        && DETECTOR_RATE_SLOW_FRAMES_RESIDUAL == 27
        && DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL.len() == 6
        && DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL[0].template == "AmericaInfantryPathfinder"
        && (detector_effective_range_residual(
            DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL[0].detection_range,
            DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL[0].vision_range_fallback,
        ) - 200.0)
            .abs()
            < 1e-6
        && (detector_effective_range_residual(
            DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL[1].detection_range,
            DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL[1].vision_range_fallback,
        ) - 200.0)
            .abs()
            < 1e-6
        && (detector_effective_range_residual(0.0, 150.0) - 150.0).abs() < 1e-6
        && residual_ms_to_frames(DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL[2].rate_ms)
            == DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL[2].rate_frames
        && (DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL[3].detection_range - 225.0).abs() < 1e-6
        && (DETECTOR_UNIT_SAMPLE_TABLE_RESIDUAL[4].detection_range - 150.0).abs() < 1e-6
        // Spotter cross-link: detector scan keeps target detected until next wake.
        && spotter_mark_detected_primary_frames_residual(DETECTOR_RATE_COMMON_FRAMES_RESIDUAL)
            == 16
}

// ---------------------------------------------------------------------------
// 5. Vision residual peels (VisionRange / AI vision factors / DSCRU)
// ---------------------------------------------------------------------------

/// C++ ThingTemplate ctor residual: VisionRange default **0**.
pub const VISION_RANGE_TEMPLATE_DEFAULT_RESIDUAL: f32 = 0.0;
/// C++ ThingTemplate ctor residual: ShroudClearingRange default **-1** (→ VisionRange).
pub const SHROUD_CLEARING_RANGE_TEMPLATE_DEFAULT_RESIDUAL: f32 = -1.0;
/// C++ ThingTemplate ctor residual: ShroudRevealToAllRange default **-1**.
pub const SHROUD_REVEAL_TO_ALL_RANGE_TEMPLATE_DEFAULT_RESIDUAL: f32 = -1.0;

/// C++ `AI_VISIONFACTOR_OWNERTYPE` residual.
pub const AI_VISIONFACTOR_OWNERTYPE_RESIDUAL: u32 = 0x01;
/// C++ `AI_VISIONFACTOR_MOOD` residual.
pub const AI_VISIONFACTOR_MOOD_RESIDUAL: u32 = 0x02;
/// C++ `AI_VISIONFACTOR_GUARDINNER` residual.
pub const AI_VISIONFACTOR_GUARDINNER_RESIDUAL: u32 = 0x04;

/// Retail AIData.ini GuardInnerModifierAI residual.
pub const VISION_GUARD_INNER_MODIFIER_AI_RESIDUAL: f32 = 1.1;
/// Retail AIData.ini GuardOuterModifierAI residual.
pub const VISION_GUARD_OUTER_MODIFIER_AI_RESIDUAL: f32 = 1.333;
/// Retail AIData.ini GuardInnerModifierHuman residual.
pub const VISION_GUARD_INNER_MODIFIER_HUMAN_RESIDUAL: f32 = 1.8;
/// Retail AIData.ini GuardOuterModifierHuman residual.
pub const VISION_GUARD_OUTER_MODIFIER_HUMAN_RESIDUAL: f32 = 2.2;
/// Retail AIData.ini AlertRangeModifier residual.
pub const VISION_ALERT_RANGE_MODIFIER_RESIDUAL: f32 = 1.1;
/// Retail AIData.ini AggressiveRangeModifier residual.
pub const VISION_AGGRESSIVE_RANGE_MODIFIER_RESIDUAL: f32 = 1.5;
/// Retail AIData.ini GuardChaseUnitsDuration residual (4s × 30 fps).
pub const GUARD_CHASE_UNIT_FRAMES_RESIDUAL: u32 = 120;

/// C++ DynamicShroudClearingRangeUpdate GRID_FX_DECAL_COUNT residual.
pub const DYNAMIC_SHROUD_GRID_FX_DECAL_COUNT_RESIDUAL: u32 = 30;

/// DSCRU state residual names (DynamicShroudClearingRangeUpdate.h DSCRU_STATE).
pub const DYNAMIC_SHROUD_STATE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "DSCRU_NOT_STARTED_YET", // 0
    "DSCRU_GROWING",         // 1
    "DSCRU_SUSTAINING",      // 2
    "DSCRU_SHRINKING",       // 3
    "DSCRU_DONE_FOREVER",    // 4
    "DSCRU_SLEEPING",        // 5
];

/// Apply Object ctor shroud residual: -1 → VisionRange.
#[inline]
pub fn vision_resolve_shroud_clearing_range_residual(
    shroud_clearing_range: f32,
    vision_range: f32,
) -> f32 {
    if (shroud_clearing_range + 1.0).abs() < 1e-6 {
        vision_range
    } else {
        shroud_clearing_range
    }
}

/// Residual AI::getAdjustedVisionRangeForObject (owner + optional guard-inner).
///
/// Fail-closed: not full mood matrix Sleep/Passive/Alert/Aggressive path when
/// `apply_mood` is false; when true applies Alert/Aggressive only for AI controller.
#[inline]
pub fn vision_adjusted_range_residual(
    base_vision: f32,
    player_is_human: bool,
    guard_inner: bool,
    contained: bool,
    largest_weapon_range: f32,
    mood_alert: bool,
    mood_aggressive: bool,
    mood_sleep: bool,
    apply_mood: bool,
) -> f32 {
    if contained {
        return largest_weapon_range;
    }
    let mut range = base_vision;
    // OWNERTYPE residual always multiplies when considered.
    if player_is_human {
        if guard_inner {
            range *= VISION_GUARD_INNER_MODIFIER_HUMAN_RESIDUAL;
        } else {
            range *= VISION_GUARD_OUTER_MODIFIER_HUMAN_RESIDUAL;
        }
    } else if guard_inner {
        range *= VISION_GUARD_INNER_MODIFIER_AI_RESIDUAL;
    } else {
        range *= VISION_GUARD_OUTER_MODIFIER_AI_RESIDUAL;
    }
    // MOOD residual only for AI controllers.
    if apply_mood && !player_is_human {
        if mood_sleep {
            return 0.0;
        }
        if mood_alert {
            range *= VISION_ALERT_RANGE_MODIFIER_RESIDUAL;
        } else if mood_aggressive {
            range *= VISION_AGGRESSIVE_RANGE_MODIFIER_RESIDUAL;
        }
    }
    range
}

/// Sample VisionRange residual anchors (template, vision, shroud).
pub const VISION_UNIT_SAMPLE_TABLE_RESIDUAL: &[(&str, f32, f32)] = &[
    ("AmericaInfantryPathfinder", 200.0, 400.0),
    ("AmericaInfantryRanger", 100.0, 400.0),
    ("GLAVehicleRadarVan", 200.0, 500.0),
    ("RadarVanPing", 150.0, 150.0),
    ("AmericaCommandCenter", 300.0, 300.0),
];

/// Wave 97 vision residual honesty pack.
pub fn honesty_vision_residual_pack_wave97() -> bool {
    (VISION_RANGE_TEMPLATE_DEFAULT_RESIDUAL - 0.0).abs() < 1e-6
        && (SHROUD_CLEARING_RANGE_TEMPLATE_DEFAULT_RESIDUAL + 1.0).abs() < 1e-6
        && (SHROUD_REVEAL_TO_ALL_RANGE_TEMPLATE_DEFAULT_RESIDUAL + 1.0).abs() < 1e-6
        && (vision_resolve_shroud_clearing_range_residual(-1.0, 150.0) - 150.0).abs() < 1e-6
        && (vision_resolve_shroud_clearing_range_residual(400.0, 150.0) - 400.0).abs() < 1e-6
        && AI_VISIONFACTOR_OWNERTYPE_RESIDUAL == 0x01
        && AI_VISIONFACTOR_MOOD_RESIDUAL == 0x02
        && AI_VISIONFACTOR_GUARDINNER_RESIDUAL == 0x04
        && (VISION_GUARD_INNER_MODIFIER_AI_RESIDUAL - 1.1).abs() < 1e-6
        && (VISION_GUARD_OUTER_MODIFIER_AI_RESIDUAL - 1.333).abs() < 1e-3
        && (VISION_GUARD_INNER_MODIFIER_HUMAN_RESIDUAL - 1.8).abs() < 1e-6
        && (VISION_GUARD_OUTER_MODIFIER_HUMAN_RESIDUAL - 2.2).abs() < 1e-6
        && (VISION_ALERT_RANGE_MODIFIER_RESIDUAL - 1.1).abs() < 1e-6
        && (VISION_AGGRESSIVE_RANGE_MODIFIER_RESIDUAL - 1.5).abs() < 1e-6
        // Human guard outer: 100 * 2.2 = 220
        && (vision_adjusted_range_residual(
            100.0, true, false, false, 0.0, false, false, false, false,
        ) - 220.0)
            .abs()
            < 1e-3
        // AI guard inner: 100 * 1.1 = 110
        && (vision_adjusted_range_residual(
            100.0, false, true, false, 0.0, false, false, false, false,
        ) - 110.0)
            .abs()
            < 1e-3
        // Contained → largest weapon range residual
        && (vision_adjusted_range_residual(
            100.0, false, false, true, 75.0, false, false, false, false,
        ) - 75.0)
            .abs()
            < 1e-6
        // AI mood sleep → 0
        && (vision_adjusted_range_residual(
            100.0, false, false, false, 0.0, false, false, true, true,
        ) - 0.0)
            .abs()
            < 1e-6
        // AI aggressive outer: 100 * 1.333 * 1.5
        && (vision_adjusted_range_residual(
            100.0, false, false, false, 0.0, false, true, false, true,
        ) - (100.0 * 1.333 * 1.5))
            .abs()
            < 1e-2
        && DYNAMIC_SHROUD_GRID_FX_DECAL_COUNT_RESIDUAL == 30
        && DYNAMIC_SHROUD_STATE_NAME_TABLE_RESIDUAL.len() == 6
        && residual_name_index(
            DYNAMIC_SHROUD_STATE_NAME_TABLE_RESIDUAL,
            "DSCRU_SUSTAINING",
        ) == Some(2)
        && residual_name_index(
            DYNAMIC_SHROUD_STATE_NAME_TABLE_RESIDUAL,
            "DSCRU_SHRINKING",
        ) == Some(3)
        && VISION_UNIT_SAMPLE_TABLE_RESIDUAL.len() == 5
        && VISION_UNIT_SAMPLE_TABLE_RESIDUAL[0].0 == "AmericaInfantryPathfinder"
        && (VISION_UNIT_SAMPLE_TABLE_RESIDUAL[0].1 - 200.0).abs() < 1e-6
        && (VISION_UNIT_SAMPLE_TABLE_RESIDUAL[2].2 - 500.0).abs() < 1e-6
}

// ---------------------------------------------------------------------------
// Combined Wave 97 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 97 residual honesty pack.
pub fn honesty_radar_stealth_vision_residual_pack_wave97() -> bool {
    honesty_radar_residual_deepen_pack_wave97()
        && honesty_spotter_residual_pack_wave97()
        && honesty_stealth_residual_deepen_pack_wave97()
        && honesty_detector_residual_deepen_pack_wave97()
        && honesty_vision_residual_pack_wave97()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radar_residual_deepen_wave97_honesty() {
        assert!(honesty_radar_residual_deepen_pack_wave97());
    }

    #[test]
    fn spotter_residual_wave97_honesty() {
        assert!(honesty_spotter_residual_pack_wave97());
    }

    #[test]
    fn stealth_residual_deepen_wave97_honesty() {
        assert!(honesty_stealth_residual_deepen_pack_wave97());
    }

    #[test]
    fn detector_residual_deepen_wave97_honesty() {
        assert!(honesty_detector_residual_deepen_pack_wave97());
    }

    #[test]
    fn vision_residual_wave97_honesty() {
        assert!(honesty_vision_residual_pack_wave97());
    }

    #[test]
    fn radar_stealth_vision_residual_pack_wave97_honesty() {
        assert!(honesty_radar_stealth_vision_residual_pack_wave97());
    }

    #[test]
    fn stealth_fighter_and_mine_cloak_residuals() {
        assert_eq!(STEALTH_FIGHTER_STEALTH_DELAY_MS_RESIDUAL, 2_000);
        assert_eq!(
            STEALTH_FIGHTER_STEALTH_DELAY_FRAMES_RESIDUAL,
            residual_ms_to_frames(STEALTH_FIGHTER_STEALTH_DELAY_MS_RESIDUAL)
        );
        assert!(stealth_fighter_allowed_to_stealth_residual(false));
        assert!(!stealth_fighter_allowed_to_stealth_residual(true));
        assert_eq!(MINE_STEALTH_DELAY_FRAMES_RESIDUAL, 0);
        assert!((MINE_STEALTH_OPACITY_RESIDUAL - 0.0).abs() < 1e-6);
        assert!(is_mine_stealth_kind_residual(true, false, false));
        assert!(is_mine_stealth_kind_residual(false, true, false));
        assert!(is_mine_stealth_kind_residual(false, false, true));
        assert!(!is_mine_stealth_kind_residual(false, false, false));
    }

    #[test]
    fn contained_detector_and_hero_delay_residuals() {
        assert!(detector_can_scan_while_contained_residual(
            false, false, false, false
        ));
        assert!(!detector_can_scan_while_contained_residual(
            true, true, false, false
        ));
        assert!(!detector_can_scan_while_contained_residual(
            true, false, false, false
        ));
        assert!(detector_can_scan_while_contained_residual(
            true, true, true, false
        ));
        assert!(detector_can_scan_while_contained_residual(
            true, false, false, true
        ));
        assert_eq!(
            hero_stealth_delay_frames_residual("AmericaInfantryColonelBurton"),
            60
        );
        assert_eq!(
            hero_stealth_delay_frames_residual("GLAInfantryJarmenKell"),
            60
        );
        assert_eq!(
            hero_stealth_delay_frames_residual("ChinaInfantryBlackLotus"),
            75
        );
        assert_eq!(
            hero_stealth_delay_frames_residual("GLAInfantrySaboteur"),
            75
        );
    }
}
