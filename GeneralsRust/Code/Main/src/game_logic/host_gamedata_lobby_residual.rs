//! Wave 86 residual peels: GameData.ini constants / multiplayer options host-only /
//! map selection residual / crate residual deepen.
//!
//! Orthogonal to Waves 82–85 enum / structure-economy / faction-skirmish residual.
//! Host-testable packs for INI-backed lobby + world defaults residual honesty.
//!
//! Sources (retail ZH INI + C++):
//! - GameData.ini — FPS/camera/scroll/gravity/shroud/economy residual constants
//! - multiplayer.ini MultiplayerSettings + MultiplayerColor table (host-only; not network play)
//! - GameData ShellMapName / MapCache.ini official multiplayer anchors (Defcon6, etc.)
//! - Crate.ini SalvageCrate / dollar matrix / veterancy EffectRange / UnitCrate residual
//!
//! Fail-closed:
//! - Not full GlobalData live INI reload / View camera GPU path
//! - Not full MultiplayerSettings lobby combo / network matchmaking residual
//! - Not full MapCache.ini parse / MapSelect UI GPU residual
//! - Not full SalvageCrateCollide W3D subobject / weapon-set upgrade matrix
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// 1. GameData.ini FPS + camera residual
// ---------------------------------------------------------------------------

/// Retail GameData.ini UseFPSLimit residual.
pub const USE_FPS_LIMIT_RESIDUAL: bool = true;
/// Retail GameData.ini FramesPerSecondLimit residual (logic 30 FPS).
pub const FRAMES_PER_SECOND_LIMIT_RESIDUAL: i32 = 30;

/// Retail GameData.ini CameraPitch residual (degrees).
pub const CAMERA_PITCH_RESIDUAL: f32 = 37.5;
/// Retail GameData.ini CameraYaw residual.
pub const CAMERA_YAW_RESIDUAL: f32 = 0.0;
/// Retail GameData.ini CameraHeight residual.
pub const CAMERA_HEIGHT_RESIDUAL: f32 = 232.0;
/// Retail GameData.ini MaxCameraHeight residual.
pub const MAX_CAMERA_HEIGHT_RESIDUAL: f32 = 310.0;
/// Retail GameData.ini MinCameraHeight residual.
pub const MIN_CAMERA_HEIGHT_RESIDUAL: f32 = 120.0;
/// Retail GameData.ini CameraAdjustSpeed residual (0..1 snap rate).
pub const CAMERA_ADJUST_SPEED_RESIDUAL: f32 = 0.3;
/// Retail GameData.ini ScrollAmountCutoff residual.
pub const SCROLL_AMOUNT_CUTOFF_RESIDUAL: f32 = 50.0;
/// Retail GameData.ini EnforceMaxCameraHeight residual.
pub const ENFORCE_MAX_CAMERA_HEIGHT_RESIDUAL: bool = false;
/// Retail GameData.ini KeyboardCameraRotateSpeed residual.
pub const KEYBOARD_CAMERA_ROTATE_SPEED_RESIDUAL: f32 = 0.1;
/// Retail GameData.ini CameraAudibleRadius residual.
pub const CAMERA_AUDIBLE_RADIUS_RESIDUAL: f32 = 250.0;

/// Wave 86 honesty: FPS + camera residual pack.
pub fn honesty_gamedata_camera_fps_residual_pack_wave86() -> bool {
    USE_FPS_LIMIT_RESIDUAL
        && FRAMES_PER_SECOND_LIMIT_RESIDUAL == 30
        && (CAMERA_PITCH_RESIDUAL - 37.5).abs() < 1e-5
        && CAMERA_YAW_RESIDUAL == 0.0
        && (CAMERA_HEIGHT_RESIDUAL - 232.0).abs() < 1e-5
        && (MAX_CAMERA_HEIGHT_RESIDUAL - 310.0).abs() < 1e-5
        && (MIN_CAMERA_HEIGHT_RESIDUAL - 120.0).abs() < 1e-5
        && MAX_CAMERA_HEIGHT_RESIDUAL > CAMERA_HEIGHT_RESIDUAL
        && CAMERA_HEIGHT_RESIDUAL > MIN_CAMERA_HEIGHT_RESIDUAL
        && (CAMERA_ADJUST_SPEED_RESIDUAL - 0.3).abs() < 1e-5
        && CAMERA_ADJUST_SPEED_RESIDUAL > 0.0
        && CAMERA_ADJUST_SPEED_RESIDUAL < 1.0
        && (SCROLL_AMOUNT_CUTOFF_RESIDUAL - 50.0).abs() < 1e-5
        && !ENFORCE_MAX_CAMERA_HEIGHT_RESIDUAL
        && (KEYBOARD_CAMERA_ROTATE_SPEED_RESIDUAL - 0.1).abs() < 1e-5
        && (CAMERA_AUDIBLE_RADIUS_RESIDUAL - 250.0).abs() < 1e-5
}

// ---------------------------------------------------------------------------
// 2. GameData.ini scroll / gravity / physics / economy / shroud residual
// ---------------------------------------------------------------------------

/// Retail GameData.ini HorizontalScrollSpeedFactor residual.
pub const HORIZONTAL_SCROLL_SPEED_FACTOR_RESIDUAL: f32 = 1.6;
/// Retail GameData.ini VerticalScrollSpeedFactor residual.
pub const VERTICAL_SCROLL_SPEED_FACTOR_RESIDUAL: f32 = 2.0;
/// Retail GameData.ini KeyboardScrollSpeedFactor residual.
pub const KEYBOARD_SCROLL_SPEED_FACTOR_RESIDUAL: f32 = 2.0;

/// Retail GameData.ini Gravity residual (dist/sec²; feels better than -32).
pub const GRAVITY_RESIDUAL: f32 = -64.0;
/// Retail GameData.ini PartitionCellSize residual.
pub const PARTITION_CELL_SIZE_RESIDUAL: f32 = 40.0;
/// Retail GameData.ini TerrainHeightAtEdgeOfMap residual.
pub const TERRAIN_HEIGHT_AT_EDGE_OF_MAP_RESIDUAL: f32 = 100.0;
/// Retail GameData.ini DefaultStructureRubbleHeight residual.
pub const DEFAULT_STRUCTURE_RUBBLE_HEIGHT_RESIDUAL: f32 = 10.0;
/// Retail GameData.ini DefaultOcclusionDelay residual (ms).
pub const DEFAULT_OCCLUSION_DELAY_MS_RESIDUAL: i32 = 3000;
/// Default occlusion delay in logic frames (30 FPS: 3000ms → 90f).
pub const DEFAULT_OCCLUSION_DELAY_FRAMES_RESIDUAL: i32 =
    DEFAULT_OCCLUSION_DELAY_MS_RESIDUAL * FRAMES_PER_SECOND_LIMIT_RESIDUAL / 1000;

/// Retail GameData.ini UnitDamagedThreshold residual.
pub const UNIT_DAMAGED_THRESHOLD_RESIDUAL: f32 = 0.7;
/// Retail GameData.ini UnitReallyDamagedThreshold residual.
pub const UNIT_REALLY_DAMAGED_THRESHOLD_RESIDUAL: f32 = 0.35;
/// Retail GameData.ini MovementPenaltyDamageState residual label.
pub const MOVEMENT_PENALTY_DAMAGE_STATE_RESIDUAL: &str = "REALLYDAMAGED";

/// Retail GameData.ini MinDistFromEdgeOfMapForBuild residual.
pub const MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD_RESIDUAL: f32 = 30.0;
/// Retail GameData.ini SupplyBuildBorder residual.
pub const SUPPLY_BUILD_BORDER_RESIDUAL: f32 = 20.0;
/// Retail GameData.ini AllowedHeightVariationForBuilding residual.
pub const ALLOWED_HEIGHT_VARIATION_FOR_BUILDING_RESIDUAL: f32 = 10.0;
/// Retail GameData.ini SellPercentage residual (fraction).
pub const SELL_PERCENTAGE_RESIDUAL: f32 = 0.50;
/// Retail GameData.ini StealthFriendlyOpacity residual (fraction).
pub const STEALTH_FRIENDLY_OPACITY_RESIDUAL: f32 = 0.50;
/// Retail GameData.ini BaseRegenHealthPercentPerSecond residual (0.3% → 0.003).
pub const BASE_REGEN_HEALTH_PERCENT_PER_SECOND_RESIDUAL: f32 = 0.003;
/// Retail GameData.ini BaseRegenDelay residual (ms).
pub const BASE_REGEN_DELAY_MS_RESIDUAL: i32 = 3000;
/// Base regen delay in logic frames.
pub const BASE_REGEN_DELAY_FRAMES_RESIDUAL: i32 =
    BASE_REGEN_DELAY_MS_RESIDUAL * FRAMES_PER_SECOND_LIMIT_RESIDUAL / 1000;
/// Retail GameData.ini UnlookPersistDuration residual (ms).
pub const UNLOOK_PERSIST_DURATION_MS_RESIDUAL: i32 = 5000;
/// Unlook persist in logic frames.
pub const UNLOOK_PERSIST_DURATION_FRAMES_RESIDUAL: i32 =
    UNLOOK_PERSIST_DURATION_MS_RESIDUAL * FRAMES_PER_SECOND_LIMIT_RESIDUAL / 1000;

/// Retail GameData.ini ShroudColor residual RGB.
pub const SHROUD_COLOR_RGB_RESIDUAL: (u8, u8, u8) = (255, 255, 255);
/// Retail GameData.ini ClearAlpha residual (255 = clear).
pub const CLEAR_ALPHA_RESIDUAL: u8 = 255;
/// Retail GameData.ini FogAlpha residual (127 mid).
pub const FOG_ALPHA_RESIDUAL: u8 = 127;
/// Retail GameData.ini ShroudAlpha residual (0 opaque).
pub const SHROUD_ALPHA_RESIDUAL: u8 = 0;

/// Retail GameData.ini MaxParticleCount residual.
pub const MAX_PARTICLE_COUNT_RESIDUAL: i32 = 2500;
/// Retail GameData.ini MaxFieldParticleCount residual.
pub const MAX_FIELD_PARTICLE_COUNT_RESIDUAL: i32 = 30;
/// Retail GameData.ini MaxLineBuildObjects residual.
pub const MAX_LINE_BUILD_OBJECTS_RESIDUAL: i32 = 50;
/// Retail GameData.ini CommandCenterHealRange residual.
pub const COMMAND_CENTER_HEAL_RANGE_RESIDUAL: f32 = 500.0;
/// Retail GameData.ini CommandCenterHealAmount residual (per logic frame).
pub const COMMAND_CENTER_HEAL_AMOUNT_RESIDUAL: f32 = 0.01;

/// Host residual: unit damage state from health fraction (mirrors thresholds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitDamageStateResidual {
    Pristine,
    Damaged,
    ReallyDamaged,
}

/// Map health fraction residual → damage state using GameData thresholds.
pub fn unit_damage_state_from_health_fraction(health_frac: f32) -> UnitDamageStateResidual {
    if health_frac <= UNIT_REALLY_DAMAGED_THRESHOLD_RESIDUAL {
        UnitDamageStateResidual::ReallyDamaged
    } else if health_frac <= UNIT_DAMAGED_THRESHOLD_RESIDUAL {
        UnitDamageStateResidual::Damaged
    } else {
        UnitDamageStateResidual::Pristine
    }
}

/// Host residual sell refund cash (SellPercentage × build cost).
pub fn sell_refund_residual(build_cost: u32) -> u32 {
    ((build_cost as f32) * SELL_PERCENTAGE_RESIDUAL).round() as u32
}

/// Wave 86 honesty: GameData scroll/physics/economy/shroud residual pack.
pub fn honesty_gamedata_world_constants_residual_pack_wave86() -> bool {
    (HORIZONTAL_SCROLL_SPEED_FACTOR_RESIDUAL - 1.6).abs() < 1e-5
        && (VERTICAL_SCROLL_SPEED_FACTOR_RESIDUAL - 2.0).abs() < 1e-5
        && (KEYBOARD_SCROLL_SPEED_FACTOR_RESIDUAL - 2.0).abs() < 1e-5
        && (GRAVITY_RESIDUAL - (-64.0)).abs() < 1e-5
        && GRAVITY_RESIDUAL < 0.0
        && (PARTITION_CELL_SIZE_RESIDUAL - 40.0).abs() < 1e-5
        && (TERRAIN_HEIGHT_AT_EDGE_OF_MAP_RESIDUAL - 100.0).abs() < 1e-5
        && (DEFAULT_STRUCTURE_RUBBLE_HEIGHT_RESIDUAL - 10.0).abs() < 1e-5
        && DEFAULT_OCCLUSION_DELAY_MS_RESIDUAL == 3000
        && DEFAULT_OCCLUSION_DELAY_FRAMES_RESIDUAL == 90
        && (UNIT_DAMAGED_THRESHOLD_RESIDUAL - 0.7).abs() < 1e-5
        && (UNIT_REALLY_DAMAGED_THRESHOLD_RESIDUAL - 0.35).abs() < 1e-5
        && UNIT_REALLY_DAMAGED_THRESHOLD_RESIDUAL < UNIT_DAMAGED_THRESHOLD_RESIDUAL
        && MOVEMENT_PENALTY_DAMAGE_STATE_RESIDUAL == "REALLYDAMAGED"
        && unit_damage_state_from_health_fraction(1.0) == UnitDamageStateResidual::Pristine
        && unit_damage_state_from_health_fraction(0.71) == UnitDamageStateResidual::Pristine
        && unit_damage_state_from_health_fraction(0.70) == UnitDamageStateResidual::Damaged
        && unit_damage_state_from_health_fraction(0.50) == UnitDamageStateResidual::Damaged
        && unit_damage_state_from_health_fraction(0.35) == UnitDamageStateResidual::ReallyDamaged
        && unit_damage_state_from_health_fraction(0.10) == UnitDamageStateResidual::ReallyDamaged
        && (MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD_RESIDUAL - 30.0).abs() < 1e-5
        && (SUPPLY_BUILD_BORDER_RESIDUAL - 20.0).abs() < 1e-5
        && (ALLOWED_HEIGHT_VARIATION_FOR_BUILDING_RESIDUAL - 10.0).abs() < 1e-5
        && (SELL_PERCENTAGE_RESIDUAL - 0.50).abs() < 1e-5
        && sell_refund_residual(1000) == 500
        && sell_refund_residual(2000) == 1000
        && (STEALTH_FRIENDLY_OPACITY_RESIDUAL - 0.50).abs() < 1e-5
        && (BASE_REGEN_HEALTH_PERCENT_PER_SECOND_RESIDUAL - 0.003).abs() < 1e-6
        && BASE_REGEN_DELAY_MS_RESIDUAL == 3000
        && BASE_REGEN_DELAY_FRAMES_RESIDUAL == 90
        && UNLOOK_PERSIST_DURATION_MS_RESIDUAL == 5000
        && UNLOOK_PERSIST_DURATION_FRAMES_RESIDUAL == 150
        && SHROUD_COLOR_RGB_RESIDUAL == (255, 255, 255)
        && CLEAR_ALPHA_RESIDUAL == 255
        && FOG_ALPHA_RESIDUAL == 127
        && SHROUD_ALPHA_RESIDUAL == 0
        && SHROUD_ALPHA_RESIDUAL < FOG_ALPHA_RESIDUAL
        && FOG_ALPHA_RESIDUAL < CLEAR_ALPHA_RESIDUAL
        && MAX_PARTICLE_COUNT_RESIDUAL == 2500
        && MAX_FIELD_PARTICLE_COUNT_RESIDUAL == 30
        && MAX_LINE_BUILD_OBJECTS_RESIDUAL == 50
        && (COMMAND_CENTER_HEAL_RANGE_RESIDUAL - 500.0).abs() < 1e-5
        && (COMMAND_CENTER_HEAL_AMOUNT_RESIDUAL - 0.01).abs() < 1e-5
}

// ---------------------------------------------------------------------------
// 3. Multiplayer options residual (host-only; not network play)
// ---------------------------------------------------------------------------

/// Retail multiplayer.ini MultiplayerSettings StartCountdownTimer residual (seconds).
pub const MP_START_COUNTDOWN_TIMER_SECONDS_RESIDUAL: i32 = 5;
/// Retail multiplayer.ini MaxBeaconsPerPlayer residual.
pub const MP_MAX_BEACONS_PER_PLAYER_RESIDUAL: i32 = 3;
/// Retail multiplayer.ini UseShroud residual (No for multiplayer default).
pub const MP_USE_SHROUD_RESIDUAL: bool = false;
/// Retail multiplayer.ini ShowRandomPlayerTemplate residual.
pub const MP_SHOW_RANDOM_PLAYER_TEMPLATE_RESIDUAL: bool = true;
/// Retail multiplayer.ini ShowRandomStartPos residual.
pub const MP_SHOW_RANDOM_START_POS_RESIDUAL: bool = true;
/// Retail multiplayer.ini ShowRandomColor residual.
pub const MP_SHOW_RANDOM_COLOR_RESIDUAL: bool = true;

/// Multiplayer color residual entry: (name, day RGB, night RGB, tooltip).
pub type MultiplayerColorResidual = (&'static str, (u8, u8, u8), (u8, u8, u8), &'static str);

/// Retail multiplayer.ini MultiplayerColor declaration order residual table.
pub const MULTIPLAYER_COLOR_RESIDUAL_TABLE: &[MultiplayerColorResidual] = &[
    ("ColorGold", (221, 226, 13), (221, 226, 13), "Color:Gold"),
    ("ColorRed", (255, 0, 0), (255, 0, 0), "Color:Red"),
    ("ColorBlue", (67, 104, 254), (67, 104, 254), "Color:Blue"),
    ("ColorGreen", (62, 209, 46), (62, 209, 46), "Color:Green"),
    (
        "ColorOrange",
        (255, 160, 25),
        (255, 160, 25),
        "Color:Orange",
    ),
    (
        "ColorSkyBlue",
        (50, 215, 230),
        (50, 215, 230),
        "Color:SkyBlue",
    ),
    ("ColorPurple", (150, 0, 200), (223, 0, 156), "Color:Purple"),
    ("ColorPink", (255, 150, 255), (255, 130, 248), "Color:Pink"),
];

/// Number of MultiplayerColor residual entries.
pub const MULTIPLAYER_COLOR_COUNT_RESIDUAL: usize = 8;

/// Pack residual RGB as 0x00RRGGBB.
pub fn pack_mp_color_rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Lookup multiplayer color residual by name.
pub fn multiplayer_color_by_name(name: &str) -> Option<MultiplayerColorResidual> {
    MULTIPLAYER_COLOR_RESIDUAL_TABLE
        .iter()
        .copied()
        .find(|(n, _, _, _)| *n == name)
}

/// Host residual: beacon placement allowed when current count < MaxBeaconsPerPlayer.
pub fn can_place_beacon_residual(current_beacons: i32) -> bool {
    current_beacons < MP_MAX_BEACONS_PER_PLAYER_RESIDUAL
}

/// Wave 86 honesty: multiplayer options residual pack (host-only).
pub fn honesty_multiplayer_options_residual_pack_wave86() -> bool {
    MP_START_COUNTDOWN_TIMER_SECONDS_RESIDUAL == 5
        && MP_MAX_BEACONS_PER_PLAYER_RESIDUAL == 3
        && !MP_USE_SHROUD_RESIDUAL
        && MP_SHOW_RANDOM_PLAYER_TEMPLATE_RESIDUAL
        && MP_SHOW_RANDOM_START_POS_RESIDUAL
        && MP_SHOW_RANDOM_COLOR_RESIDUAL
        && MULTIPLAYER_COLOR_RESIDUAL_TABLE.len() == MULTIPLAYER_COLOR_COUNT_RESIDUAL
        && multiplayer_color_by_name("ColorGold")
            == Some(("ColorGold", (221, 226, 13), (221, 226, 13), "Color:Gold"))
        && multiplayer_color_by_name("ColorRed")
            == Some(("ColorRed", (255, 0, 0), (255, 0, 0), "Color:Red"))
        && multiplayer_color_by_name("ColorBlue")
            == Some(("ColorBlue", (67, 104, 254), (67, 104, 254), "Color:Blue"))
        && multiplayer_color_by_name("ColorPurple")
            == Some(("ColorPurple", (150, 0, 200), (223, 0, 156), "Color:Purple"))
        // Purple is the residual entry with distinct night color.
        && MULTIPLAYER_COLOR_RESIDUAL_TABLE
            .iter()
            .filter(|&&(_, day, night, _)| day != night)
            .count()
            == 2 // Purple + Pink have distinct night colors
        && pack_mp_color_rgb(255, 0, 0) == 0x00FF_0000
        && pack_mp_color_rgb(0, 255, 0) == 0x0000_FF00
        && can_place_beacon_residual(0)
        && can_place_beacon_residual(2)
        && !can_place_beacon_residual(3)
        && !can_place_beacon_residual(4)
}

// ---------------------------------------------------------------------------
// 4. Map selection residual
// ---------------------------------------------------------------------------

/// Retail GameData.ini ShellMapName residual (ZH MD shell map).
pub const SHELL_MAP_NAME_RESIDUAL: &str = r"Maps\ShellMapMD\ShellMapMD.map";
/// Retail GameData.ini MapName residual default (legacy Assault).
pub const DEFAULT_MAP_NAME_RESIDUAL: &str = "Assault.map";
/// Host default skirmish map residual (cnc_game_engine DEFAULT_SKIRMISH_MAP).
pub const DEFAULT_SKIRMISH_MAP_RESIDUAL: &str = "Defcon6";
/// MapCache nameLookupTag residual for default skirmish map.
pub const DEFAULT_SKIRMISH_MAP_LOOKUP_TAG_RESIDUAL: &str = "MAP:Defcon6";

/// Map selection residual entry: (display key, lookup tag, num_players, multiplayer).
pub type MapSelectResidual = (&'static str, &'static str, u8, bool);

/// Official multiplayer MapCache residual anchors (sample table; not full parse).
pub const MAP_SELECT_RESIDUAL_TABLE: &[MapSelectResidual] = &[
    ("AlpineAssault", "MAP:AlpineAssault", 2, true),
    ("ArmoredFury", "MAP:ArmoredFury", 6, true),
    ("Defcon6", "MAP:Defcon6", 6, true),
    ("TournamentCity", "MAP:TournamentCity", 6, true),
    ("TournamentContinent", "MAP:TournamentContinent", 4, true),
    ("TournamentPlains", "MAP:TournamentPlains", 2, true),
    ("BarrenBadlands", "MAP:BarrenBadlands", 2, true),
    ("BitterWinter", "MAP:BitterWinter", 2, true),
];

/// Number of map selection residual anchors in host sample table.
pub const MAP_SELECT_RESIDUAL_COUNT: usize = 8;

/// Lookup residual map by display key.
pub fn map_select_by_key(key: &str) -> Option<MapSelectResidual> {
    MAP_SELECT_RESIDUAL_TABLE
        .iter()
        .copied()
        .find(|(k, _, _, _)| *k == key)
}

/// Whether residual map supports the requested player count.
pub fn map_supports_player_count(key: &str, players: u8) -> bool {
    map_select_by_key(key)
        .map(|(_, _, n, mp)| mp && players >= 2 && players <= n)
        .unwrap_or(false)
}

/// Wave 86 honesty: map selection residual pack.
pub fn honesty_map_selection_residual_pack_wave86() -> bool {
    SHELL_MAP_NAME_RESIDUAL == r"Maps\ShellMapMD\ShellMapMD.map"
        && SHELL_MAP_NAME_RESIDUAL.contains("ShellMapMD")
        && DEFAULT_MAP_NAME_RESIDUAL == "Assault.map"
        && DEFAULT_SKIRMISH_MAP_RESIDUAL == "Defcon6"
        && DEFAULT_SKIRMISH_MAP_LOOKUP_TAG_RESIDUAL == "MAP:Defcon6"
        && MAP_SELECT_RESIDUAL_TABLE.len() == MAP_SELECT_RESIDUAL_COUNT
        && map_select_by_key("Defcon6") == Some(("Defcon6", "MAP:Defcon6", 6, true))
        && map_select_by_key("AlpineAssault")
            == Some(("AlpineAssault", "MAP:AlpineAssault", 2, true))
        && map_select_by_key("ArmoredFury") == Some(("ArmoredFury", "MAP:ArmoredFury", 6, true))
        && map_select_by_key("TournamentPlains")
            == Some(("TournamentPlains", "MAP:TournamentPlains", 2, true))
        && map_supports_player_count("Defcon6", 2)
        && map_supports_player_count("Defcon6", 6)
        && !map_supports_player_count("Defcon6", 8)
        && map_supports_player_count("AlpineAssault", 2)
        && !map_supports_player_count("AlpineAssault", 4)
        && MAP_SELECT_RESIDUAL_TABLE.iter().all(|&(_, tag, n, mp)| {
            tag.starts_with("MAP:") && mp && (2..=8).contains(&n)
        })
        // Default skirmish residual is present in the sample table.
        && MAP_SELECT_RESIDUAL_TABLE
            .iter()
            .any(|&(k, tag, _, _)| {
                k == DEFAULT_SKIRMISH_MAP_RESIDUAL
                    && tag == DEFAULT_SKIRMISH_MAP_LOOKUP_TAG_RESIDUAL
            })
}

// ---------------------------------------------------------------------------
// 5. Crate residual deepen (Salvage / dollar matrix / veterancy / unit crate)
// ---------------------------------------------------------------------------

/// Retail SalvageCrateCollide WeaponChance residual (fraction).
pub const SALVAGE_WEAPON_CHANCE_RESIDUAL: f32 = 1.00;
/// Retail SalvageCrateCollide LevelChance residual.
pub const SALVAGE_LEVEL_CHANCE_RESIDUAL: f32 = 0.25;
/// Retail SalvageCrateCollide MoneyChance residual.
pub const SALVAGE_MONEY_CHANCE_RESIDUAL: f32 = 0.75;
/// Retail SalvageCrateCollide MinMoney residual.
pub const SALVAGE_MIN_MONEY_RESIDUAL: u32 = 25;
/// Retail SalvageCrateCollide MaxMoney residual.
pub const SALVAGE_MAX_MONEY_RESIDUAL: u32 = 75;
/// Retail SalvageCrateData CreationChance residual.
pub const SALVAGE_CREATION_CHANCE_RESIDUAL: f32 = 1.0;
/// Retail SalvageCrate PickupScience residual.
pub const SALVAGE_PICKUP_SCIENCE_RESIDUAL: &str = "SCIENCE_GLA";
/// Retail SalvageCrate KilledByType residual.
pub const SALVAGE_KILLED_BY_TYPE_RESIDUAL: &str = "SALVAGER";
/// Retail SalvageCrate DeletionUpdate Min/MaxLifetime residual (ms).
pub const SALVAGE_MIN_LIFETIME_MS_RESIDUAL: i32 = 30000;
pub const SALVAGE_MAX_LIFETIME_MS_RESIDUAL: i32 = 35000;

/// Dollar crate residual money matrix (name, money provided).
pub const DOLLAR_CRATE_MONEY_MATRIX_RESIDUAL: &[(&str, u32)] = &[
    ("100DollarCrate", 100),
    ("200DollarCrate", 200),
    ("1000DollarCrate", 1000),
    ("1500DollarCrate", 1500),
    ("2500DollarCrate", 2500),
    ("SupplyDropZoneCrate", 250),
];

/// Retail EliteTankCrateData CreationChance residual.
pub const ELITE_TANK_CRATE_CREATION_CHANCE_RESIDUAL: f32 = 0.75;
/// Retail HeroicTankCrateData CreationChance residual.
pub const HEROIC_TANK_CRATE_CREATION_CHANCE_RESIDUAL: f32 = 1.0;

/// Retail SmallLevelUpCrate VeterancyCrateCollide EffectRange residual.
pub const SMALL_LEVEL_UP_EFFECT_RANGE_RESIDUAL: f32 = 100.0;
/// Retail MediumLevelUpCrate VeterancyCrateCollide EffectRange residual.
pub const MEDIUM_LEVEL_UP_EFFECT_RANGE_RESIDUAL: f32 = 250.0;

/// Retail 2FreeCrusadersCrate UnitCrateCollide residual.
pub const FREE_CRUSADERS_UNIT_COUNT_RESIDUAL: u32 = 2;
pub const FREE_CRUSADERS_UNIT_NAME_RESIDUAL: &str = "AmericaTankCrusader";

/// Host residual: salvage money roll is in [MinMoney, MaxMoney] inclusive.
pub fn salvage_money_in_range_residual(money: u32) -> bool {
    money >= SALVAGE_MIN_MONEY_RESIDUAL && money <= SALVAGE_MAX_MONEY_RESIDUAL
}

/// Host residual: LevelChance + MoneyChance must sum to 100% (weapon independent).
pub fn salvage_level_money_chances_sum_residual() -> f32 {
    SALVAGE_LEVEL_CHANCE_RESIDUAL + SALVAGE_MONEY_CHANCE_RESIDUAL
}

/// Lookup dollar crate money residual by object name.
pub fn dollar_crate_money_residual(name: &str) -> Option<u32> {
    DOLLAR_CRATE_MONEY_MATRIX_RESIDUAL
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, m)| *m)
}

/// Wave 86 honesty: crate residual deepen pack.
pub fn honesty_crate_residual_deepen_pack_wave86() -> bool {
    (SALVAGE_WEAPON_CHANCE_RESIDUAL - 1.00).abs() < 1e-5
        && (SALVAGE_LEVEL_CHANCE_RESIDUAL - 0.25).abs() < 1e-5
        && (SALVAGE_MONEY_CHANCE_RESIDUAL - 0.75).abs() < 1e-5
        && (salvage_level_money_chances_sum_residual() - 1.0).abs() < 1e-5
        && SALVAGE_MIN_MONEY_RESIDUAL == 25
        && SALVAGE_MAX_MONEY_RESIDUAL == 75
        && SALVAGE_MIN_MONEY_RESIDUAL < SALVAGE_MAX_MONEY_RESIDUAL
        && salvage_money_in_range_residual(25)
        && salvage_money_in_range_residual(50)
        && salvage_money_in_range_residual(75)
        && !salvage_money_in_range_residual(24)
        && !salvage_money_in_range_residual(76)
        && (SALVAGE_CREATION_CHANCE_RESIDUAL - 1.0).abs() < 1e-5
        && SALVAGE_PICKUP_SCIENCE_RESIDUAL == "SCIENCE_GLA"
        && SALVAGE_KILLED_BY_TYPE_RESIDUAL == "SALVAGER"
        && SALVAGE_MIN_LIFETIME_MS_RESIDUAL == 30000
        && SALVAGE_MAX_LIFETIME_MS_RESIDUAL == 35000
        && SALVAGE_MIN_LIFETIME_MS_RESIDUAL < SALVAGE_MAX_LIFETIME_MS_RESIDUAL
        && dollar_crate_money_residual("100DollarCrate") == Some(100)
        && dollar_crate_money_residual("200DollarCrate") == Some(200)
        && dollar_crate_money_residual("1000DollarCrate") == Some(1000)
        && dollar_crate_money_residual("1500DollarCrate") == Some(1500)
        && dollar_crate_money_residual("2500DollarCrate") == Some(2500)
        && dollar_crate_money_residual("SupplyDropZoneCrate") == Some(250)
        && dollar_crate_money_residual("MissingCrate").is_none()
        && DOLLAR_CRATE_MONEY_MATRIX_RESIDUAL.len() == 6
        && (ELITE_TANK_CRATE_CREATION_CHANCE_RESIDUAL - 0.75).abs() < 1e-5
        && (HEROIC_TANK_CRATE_CREATION_CHANCE_RESIDUAL - 1.0).abs() < 1e-5
        && (SMALL_LEVEL_UP_EFFECT_RANGE_RESIDUAL - 100.0).abs() < 1e-5
        && (MEDIUM_LEVEL_UP_EFFECT_RANGE_RESIDUAL - 250.0).abs() < 1e-5
        && MEDIUM_LEVEL_UP_EFFECT_RANGE_RESIDUAL > SMALL_LEVEL_UP_EFFECT_RANGE_RESIDUAL
        && FREE_CRUSADERS_UNIT_COUNT_RESIDUAL == 2
        && FREE_CRUSADERS_UNIT_NAME_RESIDUAL == "AmericaTankCrusader"
}

// ---------------------------------------------------------------------------
// Combined Wave 86 pack
// ---------------------------------------------------------------------------

/// Combined Wave 86 honesty pack (all residual peels).
pub fn honesty_gamedata_lobby_residual_pack_wave86() -> bool {
    honesty_gamedata_camera_fps_residual_pack_wave86()
        && honesty_gamedata_world_constants_residual_pack_wave86()
        && honesty_multiplayer_options_residual_pack_wave86()
        && honesty_map_selection_residual_pack_wave86()
        && honesty_crate_residual_deepen_pack_wave86()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamedata_camera_fps_residual_pack_wave86_honesty() {
        assert!(honesty_gamedata_camera_fps_residual_pack_wave86());
    }

    #[test]
    fn gamedata_world_constants_residual_pack_wave86_honesty() {
        assert!(honesty_gamedata_world_constants_residual_pack_wave86());
    }

    #[test]
    fn multiplayer_options_residual_pack_wave86_honesty() {
        assert!(honesty_multiplayer_options_residual_pack_wave86());
    }

    #[test]
    fn map_selection_residual_pack_wave86_honesty() {
        assert!(honesty_map_selection_residual_pack_wave86());
    }

    #[test]
    fn crate_residual_deepen_pack_wave86_honesty() {
        assert!(honesty_crate_residual_deepen_pack_wave86());
    }

    #[test]
    fn gamedata_lobby_residual_pack_wave86_honesty() {
        assert!(honesty_gamedata_lobby_residual_pack_wave86());
    }
}
