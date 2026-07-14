//! Wave 87 residual peels: weather / water / bridge / tunnel deepen /
//! garrison / transport residual packs.
//!
//! Orthogonal environment + contain residual honesty for map presentation and
//! infantry/vehicle embark systems. Host-testable constants from retail ZH INI
//! and C++ module defaults.
//!
//! Sources (retail ZH INI + C++):
//! - Weather.ini / Snow.h WeatherSetting defaults (snow residual)
//! - Water.ini WaterSet MORNING..NIGHT + WaterTransparency defaults (Water.h)
//! - BridgeBehavior.cpp scaffold speeds; TerrainRoads.h BridgeTowerType /
//!   BRIDGE_MAX_TOWERS / MAX_BRIDGE_BODY_FX
//! - TunnelContain / TunnelTracker / GameData MaxTunnelCapacity (deepen Wave 64)
//! - GarrisonContain.h/cpp + FactionBuilding bunker/firebase + CivilianBuilding
//! - TransportContain.cpp defaults + host unit transport slot tables
//!
//! Fail-closed:
//! - Not full SnowManager GPU point-sprite / noise-table residual
//! - Not full W3DWater reflection / skybox mesh residual
//! - Not full BridgeBehavior scaffolding motion / dozer repair path residual
//! - Not full TunnelTracker last-tunnel cave-in / CaveSystem multi-index residual
//! - Not full GarrisonContain fire-point bone matrix / mobile garrison residual
//! - Not full TransportContain exit-door / extra-slots-in-use residual matrix
//! - Shell `playable_claim` stays false; network deferred

use crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS;
use crate::game_logic::host_combat_chinook::{
    COMBAT_CHINOOK_EXIT_DELAY_FRAMES, COMBAT_CHINOOK_EXIT_DELAY_MS, COMBAT_CHINOOK_TRANSPORT_SLOTS,
};
use crate::game_logic::host_heal::AMBULANCE_TRANSPORT_HEALTH_REGEN_PERCENT_PER_SEC;
use crate::game_logic::host_humvee::{
    HUMVEE_EXIT_DELAY_FRAMES, HUMVEE_EXIT_DELAY_MS, HUMVEE_TRANSPORT_SLOTS,
};
use crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_TRANSPORT_SLOTS;
use crate::game_logic::host_technical::TECHNICAL_TRANSPORT_SLOTS;
use crate::game_logic::host_troop_crawler::{
    TROOP_CRAWLER_HEALTH_REGEN_PERCENT_PER_SEC, TROOP_CRAWLER_TRANSPORT_SLOTS,
};
use crate::game_logic::host_tunnel_network::{
    MAX_TUNNEL_CAPACITY, TUNNEL_FULL_HEAL_FRAMES, TUNNEL_FULL_HEAL_MS,
};

/// Logic frames per second residual (GameCommon.h LOGICFRAMES_PER_SECOND).
pub const ENV_CONTAIN_LOGIC_FPS: f32 = 30.0;
/// C++ SECONDS_PER_LOGICFRAME_REAL residual.
pub const SECONDS_PER_LOGICFRAME_REAL: f32 = 1.0 / ENV_CONTAIN_LOGIC_FPS;

/// Convert residual milliseconds → logic frames @ 30 FPS (ceil, parseDuration style).
pub fn env_contain_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * (ENV_CONTAIN_LOGIC_FPS / 1000.0)).ceil() as u32
}

// ---------------------------------------------------------------------------
// 1. Weather residual (Weather.ini + Snow.h WeatherSetting defaults)
// ---------------------------------------------------------------------------

/// Retail Weather.ini SnowEnabled residual (system default off).
pub const WEATHER_SNOW_ENABLED_DEFAULT: bool = false;
/// Retail SnowTexture residual.
pub const WEATHER_SNOW_TEXTURE: &str = "ExSnowFlake.tga";
/// Retail SnowBoxDimensions residual (world units).
pub const WEATHER_SNOW_BOX_DIMENSIONS: f32 = 200.0;
/// Retail SnowBoxDensity residual (emitters per world unit).
pub const WEATHER_SNOW_BOX_DENSITY: f32 = 1.0;
/// Retail SnowFrequencyScaleX residual.
pub const WEATHER_SNOW_FREQUENCY_SCALE_X: f32 = 0.0533;
/// Retail SnowFrequencyScaleY residual.
pub const WEATHER_SNOW_FREQUENCY_SCALE_Y: f32 = 0.0275;
/// Retail SnowAmplitude residual (world units).
pub const WEATHER_SNOW_AMPLITUDE: f32 = 5.0;
/// Retail SnowVelocity residual (world units/sec).
pub const WEATHER_SNOW_VELOCITY: f32 = 4.0;
/// Retail SnowPointSize residual.
pub const WEATHER_SNOW_POINT_SIZE: f32 = 1.0;
/// Retail SnowMaxPointSize residual (min-spec <= 64).
pub const WEATHER_SNOW_MAX_POINT_SIZE: f32 = 64.0;
/// Retail SnowMinPointSize residual.
pub const WEATHER_SNOW_MIN_POINT_SIZE: f32 = 0.0;
/// Retail SnowPointSprites residual (hardware point sprites on by default).
pub const WEATHER_SNOW_POINT_SPRITES: bool = true;
/// Retail SnowQuadSize residual (emulation quad size).
pub const WEATHER_SNOW_QUAD_SIZE: f32 = 0.5;
/// C++ SnowManager SNOW_NOISE_X residual.
pub const WEATHER_SNOW_NOISE_X: u32 = 64;
/// C++ SnowManager SNOW_NOISE_Y residual.
pub const WEATHER_SNOW_NOISE_Y: u32 = 64;

/// Wave 87 honesty: weather residual pack.
pub fn honesty_weather_residual_pack_wave87() -> bool {
    !WEATHER_SNOW_ENABLED_DEFAULT
        && WEATHER_SNOW_TEXTURE.eq_ignore_ascii_case("ExSnowFlake.tga")
        && (WEATHER_SNOW_BOX_DIMENSIONS - 200.0).abs() < 0.001
        && (WEATHER_SNOW_BOX_DENSITY - 1.0).abs() < 0.001
        && (WEATHER_SNOW_FREQUENCY_SCALE_X - 0.0533).abs() < 0.0001
        && (WEATHER_SNOW_FREQUENCY_SCALE_Y - 0.0275).abs() < 0.0001
        && (WEATHER_SNOW_AMPLITUDE - 5.0).abs() < 0.001
        && (WEATHER_SNOW_VELOCITY - 4.0).abs() < 0.001
        && (WEATHER_SNOW_POINT_SIZE - 1.0).abs() < 0.001
        && (WEATHER_SNOW_MAX_POINT_SIZE - 64.0).abs() < 0.001
        && (WEATHER_SNOW_MIN_POINT_SIZE - 0.0).abs() < 0.001
        && WEATHER_SNOW_POINT_SPRITES
        && (WEATHER_SNOW_QUAD_SIZE - 0.5).abs() < 0.001
        && WEATHER_SNOW_NOISE_X == 64
        && WEATHER_SNOW_NOISE_Y == 64
}

// ---------------------------------------------------------------------------
// 2. Water residual (Water.ini WaterSet + WaterTransparency + TimeOfDay)
// ---------------------------------------------------------------------------

/// C++ TIME_OF_DAY_INVALID residual.
pub const TIME_OF_DAY_INVALID: u32 = 0;
/// C++ TIME_OF_DAY_FIRST / MORNING residual.
pub const TIME_OF_DAY_MORNING: u32 = 1;
/// C++ TIME_OF_DAY_AFTERNOON residual.
pub const TIME_OF_DAY_AFTERNOON: u32 = 2;
/// C++ TIME_OF_DAY_EVENING residual.
pub const TIME_OF_DAY_EVENING: u32 = 3;
/// C++ TIME_OF_DAY_NIGHT residual.
pub const TIME_OF_DAY_NIGHT: u32 = 4;
/// C++ TIME_OF_DAY_COUNT residual (keep last).
pub const TIME_OF_DAY_COUNT: u32 = 5;

/// C++ TimeOfDayNames residual ordered table (incl. NONE at index 0).
pub const TIME_OF_DAY_NAMES: &[&str] = &["NONE", "MORNING", "AFTERNOON", "EVENING", "NIGHT"];

/// Retail WaterTransparency TransparentWaterDepth residual (Water.h default + Water.ini).
pub const WATER_TRANSPARENT_DEPTH: f32 = 3.0;
/// Retail TransparentWaterMinOpacity residual.
pub const WATER_MIN_OPACITY: f32 = 1.0;
/// Retail StandingWaterTexture residual.
pub const WATER_STANDING_TEXTURE: &str = "TWWater01.tga";
/// Retail AdditiveBlending residual (no).
pub const WATER_ADDITIVE_BLEND: bool = false;
/// Retail RadarWaterColor residual (R140 G140 B255) — Water.h stores as floats in 0..255 space.
pub const WATER_RADAR_COLOR_R: f32 = 140.0;
pub const WATER_RADAR_COLOR_G: f32 = 140.0;
pub const WATER_RADAR_COLOR_B: f32 = 255.0;
/// Retail StandingWaterColor residual (white tint).
pub const WATER_STANDING_COLOR_R: f32 = 1.0;
pub const WATER_STANDING_COLOR_G: f32 = 1.0;
pub const WATER_STANDING_COLOR_B: f32 = 1.0;

/// Retail WaterSet shared residual: WaterTexture + WaterRepeatCount.
pub const WATER_TEXTURE_TS: &str = "TSWater.tga";
/// Retail WaterRepeatCount residual (all TOD).
pub const WATER_REPEAT_COUNT: i32 = 32;
/// Retail UScrollPerMS / VScrollPerMS residual for day periods (not NIGHT).
pub const WATER_SCROLL_PER_MS_DAY: f32 = 0.002;
/// Retail NIGHT scroll residual (frozen surface).
pub const WATER_SCROLL_PER_MS_NIGHT: f32 = 0.0;
/// Retail SkyTexelsPerUnit residual for MORNING/AFTERNOON/EVENING.
pub const WATER_SKY_TEXELS_DAY: f32 = 0.8;
/// Retail SkyTexelsPerUnit residual for NIGHT.
pub const WATER_SKY_TEXELS_NIGHT: f32 = 1.6;

/// Retail WaterSet sky texture residual per TOD.
pub const WATER_SKY_MORNING: &str = "TSCloudWis.tga";
pub const WATER_SKY_AFTERNOON: &str = "TSCloudWis.tga";
pub const WATER_SKY_EVENING: &str = "TSCloudSun.tga";
pub const WATER_SKY_NIGHT: &str = "TSStarFeld.tga";

/// Retail WaterSet DiffuseColor residual (RGBA) per TOD.
pub const WATER_DIFFUSE_MORNING: (u8, u8, u8, u8) = (175, 175, 175, 255);
pub const WATER_DIFFUSE_AFTERNOON: (u8, u8, u8, u8) = (185, 185, 185, 255);
pub const WATER_DIFFUSE_EVENING: (u8, u8, u8, u8) = (225, 225, 225, 255);
pub const WATER_DIFFUSE_NIGHT: (u8, u8, u8, u8) = (100, 100, 100, 255);

/// Retail TransparentDiffuseColor alpha residual per TOD.
pub const WATER_TRANSPARENT_ALPHA_MORNING: u8 = 128;
pub const WATER_TRANSPARENT_ALPHA_AFTERNOON: u8 = 128;
pub const WATER_TRANSPARENT_ALPHA_EVENING: u8 = 96;
pub const WATER_TRANSPARENT_ALPHA_NIGHT: u8 = 128;

/// Residual WaterSet entry for honesty table.
#[derive(Debug, Clone, Copy)]
pub struct WaterSetResidual {
    pub tod: u32,
    pub name: &'static str,
    pub sky_texture: &'static str,
    pub diffuse: (u8, u8, u8, u8),
    pub transparent_alpha: u8,
    pub u_scroll: f32,
    pub sky_texels: f32,
}

/// Ordered residual WaterSet table (MORNING..NIGHT).
pub const WATER_SET_RESIDUAL_TABLE: &[WaterSetResidual] = &[
    WaterSetResidual {
        tod: TIME_OF_DAY_MORNING,
        name: "MORNING",
        sky_texture: WATER_SKY_MORNING,
        diffuse: WATER_DIFFUSE_MORNING,
        transparent_alpha: WATER_TRANSPARENT_ALPHA_MORNING,
        u_scroll: WATER_SCROLL_PER_MS_DAY,
        sky_texels: WATER_SKY_TEXELS_DAY,
    },
    WaterSetResidual {
        tod: TIME_OF_DAY_AFTERNOON,
        name: "AFTERNOON",
        sky_texture: WATER_SKY_AFTERNOON,
        diffuse: WATER_DIFFUSE_AFTERNOON,
        transparent_alpha: WATER_TRANSPARENT_ALPHA_AFTERNOON,
        u_scroll: WATER_SCROLL_PER_MS_DAY,
        sky_texels: WATER_SKY_TEXELS_DAY,
    },
    WaterSetResidual {
        tod: TIME_OF_DAY_EVENING,
        name: "EVENING",
        sky_texture: WATER_SKY_EVENING,
        diffuse: WATER_DIFFUSE_EVENING,
        transparent_alpha: WATER_TRANSPARENT_ALPHA_EVENING,
        u_scroll: WATER_SCROLL_PER_MS_DAY,
        sky_texels: WATER_SKY_TEXELS_DAY,
    },
    WaterSetResidual {
        tod: TIME_OF_DAY_NIGHT,
        name: "NIGHT",
        sky_texture: WATER_SKY_NIGHT,
        diffuse: WATER_DIFFUSE_NIGHT,
        transparent_alpha: WATER_TRANSPARENT_ALPHA_NIGHT,
        u_scroll: WATER_SCROLL_PER_MS_NIGHT,
        sky_texels: WATER_SKY_TEXELS_NIGHT,
    },
];

/// Wave 87 honesty: water residual pack.
pub fn honesty_water_residual_pack_wave87() -> bool {
    TIME_OF_DAY_INVALID == 0
        && TIME_OF_DAY_MORNING == 1
        && TIME_OF_DAY_AFTERNOON == 2
        && TIME_OF_DAY_EVENING == 3
        && TIME_OF_DAY_NIGHT == 4
        && TIME_OF_DAY_COUNT == 5
        && TIME_OF_DAY_NAMES.len() == 5
        && TIME_OF_DAY_NAMES[0] == "NONE"
        && TIME_OF_DAY_NAMES[1] == "MORNING"
        && TIME_OF_DAY_NAMES[4] == "NIGHT"
        && (WATER_TRANSPARENT_DEPTH - 3.0).abs() < 0.001
        && (WATER_MIN_OPACITY - 1.0).abs() < 0.001
        && WATER_STANDING_TEXTURE == "TWWater01.tga"
        && !WATER_ADDITIVE_BLEND
        && (WATER_RADAR_COLOR_R - 140.0).abs() < 0.001
        && (WATER_RADAR_COLOR_G - 140.0).abs() < 0.001
        && (WATER_RADAR_COLOR_B - 255.0).abs() < 0.001
        && (WATER_STANDING_COLOR_R - 1.0).abs() < 0.001
        && WATER_REPEAT_COUNT == 32
        && WATER_TEXTURE_TS == "TSWater.tga"
        && WATER_SET_RESIDUAL_TABLE.len() == 4
        && WATER_SET_RESIDUAL_TABLE[0].sky_texture == "TSCloudWis.tga"
        && WATER_SET_RESIDUAL_TABLE[2].sky_texture == "TSCloudSun.tga"
        && WATER_SET_RESIDUAL_TABLE[3].sky_texture == "TSStarFeld.tga"
        && WATER_SET_RESIDUAL_TABLE[0].diffuse == (175, 175, 175, 255)
        && WATER_SET_RESIDUAL_TABLE[3].diffuse == (100, 100, 100, 255)
        && WATER_SET_RESIDUAL_TABLE[2].transparent_alpha == 96
        && (WATER_SET_RESIDUAL_TABLE[3].u_scroll - 0.0).abs() < 0.0001
        && (WATER_SET_RESIDUAL_TABLE[3].sky_texels - 1.6).abs() < 0.001
        && (WATER_SET_RESIDUAL_TABLE[0].u_scroll - 0.002).abs() < 0.0001
}

// ---------------------------------------------------------------------------
// 3. Bridge residual (BridgeBehavior + BridgeTowerType)
// ---------------------------------------------------------------------------

/// C++ BridgeTowerType residual ordinals.
pub const BRIDGE_TOWER_FROM_LEFT: u32 = 0;
pub const BRIDGE_TOWER_FROM_RIGHT: u32 = 1;
pub const BRIDGE_TOWER_TO_LEFT: u32 = 2;
pub const BRIDGE_TOWER_TO_RIGHT: u32 = 3;
/// C++ BRIDGE_MAX_TOWERS residual (keep last).
pub const BRIDGE_MAX_TOWERS: u32 = 4;

/// C++ BridgeTowerType residual name table.
pub const BRIDGE_TOWER_TYPE_NAMES: &[&str] = &[
    "BRIDGE_TOWER_FROM_LEFT",
    "BRIDGE_TOWER_FROM_RIGHT",
    "BRIDGE_TOWER_TO_LEFT",
    "BRIDGE_TOWER_TO_RIGHT",
];

/// C++ MAX_BRIDGE_BODY_FX residual.
pub const MAX_BRIDGE_BODY_FX: u32 = 3;

/// C++ BridgeBehaviorModuleData default LateralScaffoldSpeed residual.
pub const BRIDGE_LATERAL_SCAFFOLD_SPEED: f32 = 1.0;
/// C++ BridgeBehaviorModuleData default VerticalScaffoldSpeed residual.
pub const BRIDGE_VERTICAL_SCAFFOLD_SPEED: f32 = 1.0;

/// Whether a tower type ordinal is valid residual.
pub fn bridge_tower_type_valid(tower: u32) -> bool {
    tower < BRIDGE_MAX_TOWERS
}

/// Wave 87 honesty: bridge residual pack.
pub fn honesty_bridge_residual_pack_wave87() -> bool {
    BRIDGE_TOWER_FROM_LEFT == 0
        && BRIDGE_TOWER_FROM_RIGHT == 1
        && BRIDGE_TOWER_TO_LEFT == 2
        && BRIDGE_TOWER_TO_RIGHT == 3
        && BRIDGE_MAX_TOWERS == 4
        && BRIDGE_TOWER_TYPE_NAMES.len() == BRIDGE_MAX_TOWERS as usize
        && BRIDGE_TOWER_TYPE_NAMES[0] == "BRIDGE_TOWER_FROM_LEFT"
        && BRIDGE_TOWER_TYPE_NAMES[3] == "BRIDGE_TOWER_TO_RIGHT"
        && MAX_BRIDGE_BODY_FX == 3
        && (BRIDGE_LATERAL_SCAFFOLD_SPEED - 1.0).abs() < 0.001
        && (BRIDGE_VERTICAL_SCAFFOLD_SPEED - 1.0).abs() < 0.001
        && bridge_tower_type_valid(0)
        && bridge_tower_type_valid(3)
        && !bridge_tower_type_valid(4)
        && !bridge_tower_type_valid(BRIDGE_MAX_TOWERS)
}

// ---------------------------------------------------------------------------
// 4. Tunnel residual deepen (beyond Wave 64 pack)
// ---------------------------------------------------------------------------

/// C++ OpenContain CONTAIN_MAX_UNKNOWN residual (-1 = infinite/unassigned).
pub const CONTAIN_MAX_UNKNOWN: i32 = -1;

/// C++ TunnelContain isKickOutOnCapture residual (tunnels do NOT kick on capture).
pub const TUNNEL_KICK_OUT_ON_CAPTURE: bool = false;
/// C++ TunnelContain isImmuneToClearBuildingAttacks residual.
pub const TUNNEL_IMMUNE_TO_CLEAR_BUILDING_ATTACKS: bool = true;
/// C++ TunnelContain isGarrisonable residual.
pub const TUNNEL_IS_GARRISONABLE: bool = false;
/// C++ TunnelContain isBustable residual.
pub const TUNNEL_IS_BUSTABLE: bool = true;
/// C++ TunnelContain isTunnelContain residual.
pub const TUNNEL_IS_TUNNEL_CONTAIN: bool = true;

/// C++ TunnelTracker nemesis expiry residual: 4 * LOGICFRAMES_PER_SECOND.
pub const TUNNEL_NEMESIS_EXPIRY_FRAMES: u32 = 4 * (ENV_CONTAIN_LOGIC_FPS as u32);
/// C++ TunnelContainModuleData default TimeForFullHeal residual (1 frame before INI).
pub const TUNNEL_DEFAULT_FULL_HEAL_FRAMES: f32 = 1.0;

/// Residual heal sliver when contained frames < full-heal frames:
/// amount = max_health / frames_for_full_heal (TunnelTracker::healObject).
pub fn tunnel_heal_sliver_amount(max_health: f32, frames_for_full_heal: f32) -> f32 {
    if frames_for_full_heal <= 0.0 {
        return max_health;
    }
    max_health / frames_for_full_heal
}

/// Whether contained unit is fully healed residual (contained duration >= full heal).
pub fn tunnel_heal_is_complete(contained_frames: u32, frames_for_full_heal: u32) -> bool {
    contained_frames >= frames_for_full_heal
}

/// Wave 87 honesty: tunnel residual deepen pack.
pub fn honesty_tunnel_residual_deepen_wave87() -> bool {
    // Wave 64 anchors still hold.
    MAX_TUNNEL_CAPACITY == 10
        && TUNNEL_FULL_HEAL_MS == 5000
        && TUNNEL_FULL_HEAL_FRAMES == 150
        && env_contain_ms_to_frames(TUNNEL_FULL_HEAL_MS) == TUNNEL_FULL_HEAL_FRAMES
        // Deepen residual.
        && CONTAIN_MAX_UNKNOWN == -1
        && !TUNNEL_KICK_OUT_ON_CAPTURE
        && TUNNEL_IMMUNE_TO_CLEAR_BUILDING_ATTACKS
        && !TUNNEL_IS_GARRISONABLE
        && TUNNEL_IS_BUSTABLE
        && TUNNEL_IS_TUNNEL_CONTAIN
        && TUNNEL_NEMESIS_EXPIRY_FRAMES == 120
        && (TUNNEL_DEFAULT_FULL_HEAL_FRAMES - 1.0).abs() < 0.001
        // Heal residual: 100 HP / 150 frames → ~0.6667 per frame sliver.
        && (tunnel_heal_sliver_amount(100.0, 150.0) - (100.0 / 150.0)).abs() < 0.0001
        && (tunnel_heal_sliver_amount(1000.0, 150.0) - (1000.0 / 150.0)).abs() < 0.0001
        && tunnel_heal_is_complete(150, 150)
        && tunnel_heal_is_complete(200, 150)
        && !tunnel_heal_is_complete(149, 150)
        // Capacity residual: full at 10.
        && MAX_TUNNEL_CAPACITY > 0
        && MAX_TUNNEL_CAPACITY <= 10
}

// ---------------------------------------------------------------------------
// 5. Garrison residual peels
// ---------------------------------------------------------------------------

/// C++ MAX_GARRISON_POINTS residual.
pub const MAX_GARRISON_POINTS: u32 = 40;
/// C++ GARRISON_INDEX_INVALID residual.
pub const GARRISON_INDEX_INVALID: i32 = -1;

/// C++ GARRISON_POINT_* condition residual ordinals.
pub const GARRISON_POINT_PRISTINE: u32 = 0;
pub const GARRISON_POINT_DAMAGED: u32 = 1;
pub const GARRISON_POINT_REALLY_DAMAGED: u32 = 2;
/// C++ MAX_GARRISON_POINT_CONDITIONS residual (keep last).
pub const MAX_GARRISON_POINT_CONDITIONS: u32 = 3;

/// C++ MUZZLE_FLASH_LIFETIME residual: LOGICFRAMES_PER_SECOND / 7 → 4 (integer).
pub const GARRISON_MUZZLE_FLASH_LIFETIME_FRAMES: u32 =
    (ENV_CONTAIN_LOGIC_FPS as u32) / 7;

/// Retail faction bunker ContainMax residual (ChinaBunker / GLAPalace).
pub const GARRISON_BUNKER_CONTAIN_MAX: i32 = 5;
/// Retail AmericaFireBase ContainMax residual.
pub const GARRISON_FIREBASE_CONTAIN_MAX: i32 = 4;
/// Retail civilian building GarrisonContain ContainMax residual.
pub const GARRISON_CIVILIAN_CONTAIN_MAX: i32 = 10;

/// Retail bunker ImmuneToClearBuildingAttacks residual.
pub const GARRISON_BUNKER_IMMUNE_TO_CLEAR: bool = true;
/// C++ GarrisonContain default IsEnclosingContainer residual (TRUE).
pub const GARRISON_DEFAULT_IS_ENCLOSING: bool = true;
/// Retail AmericaFireBase IsEnclosingContainer residual (No).
pub const GARRISON_FIREBASE_IS_ENCLOSING: bool = false;
/// Retail AmericaFireBase DamagePercentToUnits residual (100%).
pub const GARRISON_FIREBASE_DAMAGE_PERCENT_TO_UNITS: f32 = 1.0;

/// Retail EnterSound / ExitSound residual names.
pub const GARRISON_ENTER_SOUND: &str = "GarrisonEnter";
pub const GARRISON_EXIT_SOUND: &str = "GarrisonExit";

/// C++ GarrisonContainModuleData defaults.
pub const GARRISON_DEFAULT_MOBILE: bool = false;
pub const GARRISON_DEFAULT_HEAL_OBJECTS: bool = false;
pub const GARRISON_DEFAULT_FULL_HEAL_FRAMES: f32 = 1.0;
pub const GARRISON_DEFAULT_IMMUNE_TO_CLEAR: bool = false;

/// Whether garrison capacity residual can accept another infantry occupant.
pub fn garrison_can_enter(current_count: i32, contain_max: i32) -> bool {
    if contain_max == CONTAIN_MAX_UNKNOWN {
        return true;
    }
    current_count < contain_max
}

/// Garrison heal residual shares tunnel sliver formula when HealObjects is on.
pub fn garrison_heal_sliver_amount(max_health: f32, frames_for_full_heal: f32) -> f32 {
    tunnel_heal_sliver_amount(max_health, frames_for_full_heal)
}

/// Wave 87 honesty: garrison residual pack.
pub fn honesty_garrison_residual_pack_wave87() -> bool {
    MAX_GARRISON_POINTS == 40
        && GARRISON_INDEX_INVALID == -1
        && GARRISON_POINT_PRISTINE == 0
        && GARRISON_POINT_DAMAGED == 1
        && GARRISON_POINT_REALLY_DAMAGED == 2
        && MAX_GARRISON_POINT_CONDITIONS == 3
        && GARRISON_MUZZLE_FLASH_LIFETIME_FRAMES == 4
        && GARRISON_BUNKER_CONTAIN_MAX == 5
        && GARRISON_FIREBASE_CONTAIN_MAX == 4
        && GARRISON_CIVILIAN_CONTAIN_MAX == 10
        && GARRISON_BUNKER_IMMUNE_TO_CLEAR
        && GARRISON_DEFAULT_IS_ENCLOSING
        && !GARRISON_FIREBASE_IS_ENCLOSING
        && (GARRISON_FIREBASE_DAMAGE_PERCENT_TO_UNITS - 1.0).abs() < 0.001
        && GARRISON_ENTER_SOUND == "GarrisonEnter"
        && GARRISON_EXIT_SOUND == "GarrisonExit"
        && !GARRISON_DEFAULT_MOBILE
        && !GARRISON_DEFAULT_HEAL_OBJECTS
        && (GARRISON_DEFAULT_FULL_HEAL_FRAMES - 1.0).abs() < 0.001
        && !GARRISON_DEFAULT_IMMUNE_TO_CLEAR
        // Capacity residual matrix.
        && garrison_can_enter(0, GARRISON_BUNKER_CONTAIN_MAX)
        && garrison_can_enter(4, GARRISON_BUNKER_CONTAIN_MAX)
        && !garrison_can_enter(5, GARRISON_BUNKER_CONTAIN_MAX)
        && garrison_can_enter(3, GARRISON_FIREBASE_CONTAIN_MAX)
        && !garrison_can_enter(4, GARRISON_FIREBASE_CONTAIN_MAX)
        && garrison_can_enter(9, GARRISON_CIVILIAN_CONTAIN_MAX)
        && !garrison_can_enter(10, GARRISON_CIVILIAN_CONTAIN_MAX)
        && garrison_can_enter(100, CONTAIN_MAX_UNKNOWN)
        // Heal residual shares tunnel formula.
        && (garrison_heal_sliver_amount(200.0, 100.0) - 2.0).abs() < 0.0001
        // Firebase is non-enclosing exception; bunkers enclose.
        && GARRISON_DEFAULT_IS_ENCLOSING != GARRISON_FIREBASE_IS_ENCLOSING
        // Comment residual: max units inside any garrisoned structure is 10.
        && GARRISON_CIVILIAN_CONTAIN_MAX == 10
}

// ---------------------------------------------------------------------------
// 6. Transport residual peels (defaults + cross-unit slot table)
// ---------------------------------------------------------------------------

/// C++ TransportContainModuleData default Slots residual.
pub const TRANSPORT_DEFAULT_SLOTS: i32 = 0;
/// C++ default ScatterNearbyOnExit residual.
pub const TRANSPORT_DEFAULT_SCATTER_NEARBY_ON_EXIT: bool = true;
/// C++ default OrientLikeContainerOnExit residual.
pub const TRANSPORT_DEFAULT_ORIENT_LIKE_CONTAINER: bool = false;
/// C++ default KeepContainerVelocityOnExit residual.
pub const TRANSPORT_DEFAULT_KEEP_VELOCITY: bool = false;
/// C++ default GoAggressiveOnExit residual.
pub const TRANSPORT_DEFAULT_GO_AGGRESSIVE: bool = false;
/// C++ default ResetMoodCheckTimeOnExit residual.
pub const TRANSPORT_DEFAULT_RESET_MOOD: bool = true;
/// C++ default DestroyRidersWhoAreNotFreeToExit residual.
pub const TRANSPORT_DEFAULT_DESTROY_RIDERS_NOT_FREE: bool = false;
/// C++ default HealthRegen%PerSec residual.
pub const TRANSPORT_DEFAULT_HEALTH_REGEN_PERCENT: f32 = 0.0;
/// C++ default ExitDelay residual (frames after parseDuration).
pub const TRANSPORT_DEFAULT_EXIT_DELAY_FRAMES: u32 = 0;
/// C++ default DelayExitInAir residual.
pub const TRANSPORT_DEFAULT_DELAY_EXIT_IN_AIR: bool = false;
/// C++ default ArmedRidersUpgradeMyWeaponSet residual.
pub const TRANSPORT_DEFAULT_ARMED_RIDERS_UPGRADE: bool = false;

/// C++ OpenContain default NumberOfExitPaths residual.
pub const OPEN_CONTAIN_DEFAULT_EXIT_PATHS: i32 = 1;
/// C++ OpenContain default PassengersAllowedToFire residual.
pub const OPEN_CONTAIN_DEFAULT_PASSENGERS_FIRE: bool = false;
/// C++ OpenContain default DamagePercentToUnits residual.
pub const OPEN_CONTAIN_DEFAULT_DAMAGE_PERCENT: f32 = 0.0;
/// C++ OpenContain default DoorOpenTime residual (1 frame).
pub const OPEN_CONTAIN_DEFAULT_DOOR_OPEN_TIME: u32 = 1;
/// C++ OpenContain default AllowAllies/Enemies/Neutral residual.
pub const OPEN_CONTAIN_DEFAULT_ALLOW_ALLIES: bool = true;
pub const OPEN_CONTAIN_DEFAULT_ALLOW_ENEMIES: bool = true;
pub const OPEN_CONTAIN_DEFAULT_ALLOW_NEUTRAL: bool = true;

/// Transport residual slot table entry: (template residual name, slots).
pub const TRANSPORT_SLOT_RESIDUAL_TABLE: &[(&str, usize)] = &[
    ("AmericaVehicleHumvee", 5),
    ("GLAVehicleTechnical", 5),
    ("ChinaVehicleTroopCrawler", 8),
    ("AmericaVehicleChinook", 8),
    ("GLAVehicleBattleBus", 8),
    ("ChinaVehicleListeningOutpost", 2),
    ("AmericaVehicleAmbulance", 3),
];

/// Transport residual ExitDelay table (ms → frames): (name, ms, frames).
pub const TRANSPORT_EXIT_DELAY_RESIDUAL_TABLE: &[(&str, u32, u32)] = &[
    ("AmericaVehicleHumvee", 250, 8),
    ("AmericaVehicleChinook", 100, 3),
    ("GLAVehicleBattleBus", 250, 8),
];

/// Transport residual HealthRegen%PerSec table.
pub const TRANSPORT_HEALTH_REGEN_RESIDUAL_TABLE: &[(&str, f32)] = &[
    ("AmericaVehicleAmbulance", 25.0),
    ("ChinaVehicleTroopCrawler", 10.0),
    ("ChinaVehicleListeningOutpost", 10.0),
];

/// C++ TransportContain health regen residual per logic frame:
/// max_health * (regen% / 100) * SECONDS_PER_LOGICFRAME_REAL.
pub fn transport_health_regen_per_frame(max_health: f32, regen_percent_per_sec: f32) -> f32 {
    max_health * (regen_percent_per_sec / 100.0) * SECONDS_PER_LOGICFRAME_REAL
}

/// Whether transport slots residual can accept a unit taking `unit_slots`.
pub fn transport_can_load(used_slots: usize, unit_slots: usize, capacity: usize) -> bool {
    used_slots.saturating_add(unit_slots) <= capacity
}

/// Wave 87 honesty: transport residual pack.
pub fn honesty_transport_residual_pack_wave87() -> bool {
    // C++ TransportContainModuleData defaults.
    TRANSPORT_DEFAULT_SLOTS == 0
        && TRANSPORT_DEFAULT_SCATTER_NEARBY_ON_EXIT
        && !TRANSPORT_DEFAULT_ORIENT_LIKE_CONTAINER
        && !TRANSPORT_DEFAULT_KEEP_VELOCITY
        && !TRANSPORT_DEFAULT_GO_AGGRESSIVE
        && TRANSPORT_DEFAULT_RESET_MOOD
        && !TRANSPORT_DEFAULT_DESTROY_RIDERS_NOT_FREE
        && (TRANSPORT_DEFAULT_HEALTH_REGEN_PERCENT - 0.0).abs() < 0.001
        && TRANSPORT_DEFAULT_EXIT_DELAY_FRAMES == 0
        && !TRANSPORT_DEFAULT_DELAY_EXIT_IN_AIR
        && !TRANSPORT_DEFAULT_ARMED_RIDERS_UPGRADE
        // OpenContain defaults residual.
        && OPEN_CONTAIN_DEFAULT_EXIT_PATHS == 1
        && !OPEN_CONTAIN_DEFAULT_PASSENGERS_FIRE
        && (OPEN_CONTAIN_DEFAULT_DAMAGE_PERCENT - 0.0).abs() < 0.001
        && OPEN_CONTAIN_DEFAULT_DOOR_OPEN_TIME == 1
        && OPEN_CONTAIN_DEFAULT_ALLOW_ALLIES
        && OPEN_CONTAIN_DEFAULT_ALLOW_ENEMIES
        && OPEN_CONTAIN_DEFAULT_ALLOW_NEUTRAL
        // Cross-unit slot residual table matches host unit packs.
        && HUMVEE_TRANSPORT_SLOTS == 5
        && TECHNICAL_TRANSPORT_SLOTS == 5
        && TROOP_CRAWLER_TRANSPORT_SLOTS == 8
        && COMBAT_CHINOOK_TRANSPORT_SLOTS == 8
        && BATTLE_BUS_TRANSPORT_SLOTS == 8
        && LISTENING_OUTPOST_TRANSPORT_SLOTS == 2
        && TRANSPORT_SLOT_RESIDUAL_TABLE
            .iter()
            .any(|(n, s)| *n == "AmericaVehicleHumvee" && *s == HUMVEE_TRANSPORT_SLOTS)
        && TRANSPORT_SLOT_RESIDUAL_TABLE
            .iter()
            .any(|(n, s)| *n == "ChinaVehicleTroopCrawler" && *s == TROOP_CRAWLER_TRANSPORT_SLOTS)
        && TRANSPORT_SLOT_RESIDUAL_TABLE
            .iter()
            .any(|(n, s)| *n == "AmericaVehicleChinook" && *s == COMBAT_CHINOOK_TRANSPORT_SLOTS)
        && TRANSPORT_SLOT_RESIDUAL_TABLE
            .iter()
            .any(|(n, s)| *n == "GLAVehicleBattleBus" && *s == BATTLE_BUS_TRANSPORT_SLOTS)
        && TRANSPORT_SLOT_RESIDUAL_TABLE
            .iter()
            .any(|(n, s)| {
                *n == "ChinaVehicleListeningOutpost" && *s == LISTENING_OUTPOST_TRANSPORT_SLOTS
            })
        && TRANSPORT_SLOT_RESIDUAL_TABLE
            .iter()
            .any(|(n, s)| *n == "AmericaVehicleAmbulance" && *s == 3)
        // Exit delay residual anchors.
        && HUMVEE_EXIT_DELAY_MS == 250
        && HUMVEE_EXIT_DELAY_FRAMES == 8
        && COMBAT_CHINOOK_EXIT_DELAY_MS == 100
        && COMBAT_CHINOOK_EXIT_DELAY_FRAMES == 3
        && env_contain_ms_to_frames(250) == 8
        && env_contain_ms_to_frames(100) == 3
        && TRANSPORT_EXIT_DELAY_RESIDUAL_TABLE
            .iter()
            .any(|(n, ms, f)| *n == "AmericaVehicleHumvee" && *ms == 250 && *f == 8)
        && TRANSPORT_EXIT_DELAY_RESIDUAL_TABLE
            .iter()
            .any(|(n, ms, f)| *n == "AmericaVehicleChinook" && *ms == 100 && *f == 3)
        // Health regen residual formula + anchors.
        && (AMBULANCE_TRANSPORT_HEALTH_REGEN_PERCENT_PER_SEC - 25.0).abs() < 0.001
        && (TROOP_CRAWLER_HEALTH_REGEN_PERCENT_PER_SEC - 10.0).abs() < 0.001
        && (transport_health_regen_per_frame(100.0, 25.0)
            - (100.0 * 0.25 * SECONDS_PER_LOGICFRAME_REAL))
            .abs()
            < 0.0001
        && (transport_health_regen_per_frame(100.0, 10.0)
            - (100.0 * 0.10 * SECONDS_PER_LOGICFRAME_REAL))
            .abs()
            < 0.0001
        && TRANSPORT_HEALTH_REGEN_RESIDUAL_TABLE
            .iter()
            .any(|(n, p)| {
                *n == "AmericaVehicleAmbulance"
                    && (*p - AMBULANCE_TRANSPORT_HEALTH_REGEN_PERCENT_PER_SEC).abs() < 0.001
            })
        // Load residual matrix: Humvee 5 slots, infantry unit_slots=1.
        && transport_can_load(0, 1, HUMVEE_TRANSPORT_SLOTS)
        && transport_can_load(4, 1, HUMVEE_TRANSPORT_SLOTS)
        && !transport_can_load(5, 1, HUMVEE_TRANSPORT_SLOTS)
        && transport_can_load(0, 2, LISTENING_OUTPOST_TRANSPORT_SLOTS)
        && !transport_can_load(1, 2, LISTENING_OUTPOST_TRANSPORT_SLOTS)
}

// ---------------------------------------------------------------------------
// Combined Wave 87 pack
// ---------------------------------------------------------------------------

/// Combined Wave 87 honesty pack (all six residual peels).
pub fn honesty_env_contain_residual_pack_wave87() -> bool {
    honesty_weather_residual_pack_wave87()
        && honesty_water_residual_pack_wave87()
        && honesty_bridge_residual_pack_wave87()
        && honesty_tunnel_residual_deepen_wave87()
        && honesty_garrison_residual_pack_wave87()
        && honesty_transport_residual_pack_wave87()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_residual_pack_wave87_honesty() {
        assert!(honesty_weather_residual_pack_wave87());
    }

    #[test]
    fn water_residual_pack_wave87_honesty() {
        assert!(honesty_water_residual_pack_wave87());
    }

    #[test]
    fn bridge_residual_pack_wave87_honesty() {
        assert!(honesty_bridge_residual_pack_wave87());
    }

    #[test]
    fn tunnel_residual_deepen_wave87_honesty() {
        assert!(honesty_tunnel_residual_deepen_wave87());
    }

    #[test]
    fn garrison_residual_pack_wave87_honesty() {
        assert!(honesty_garrison_residual_pack_wave87());
    }

    #[test]
    fn transport_residual_pack_wave87_honesty() {
        assert!(honesty_transport_residual_pack_wave87());
    }

    #[test]
    fn env_contain_residual_pack_wave87_honesty() {
        assert!(honesty_env_contain_residual_pack_wave87());
    }
}
