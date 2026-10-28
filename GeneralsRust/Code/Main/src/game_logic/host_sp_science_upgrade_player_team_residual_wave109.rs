//! Wave 109 residual peels: SpecialPower template store / Science store /
//! Upgrade store / Player / Team residual deepen (host-testable game-logic residual).
//!
//! Orthogonal to Wave 80 (SP enum), Wave 92 (science names), Wave 94 (upgrade
//! names + special abilities), Wave 95 (team/player dict keys), Wave 103
//! (superweapon reload table). Host residual only — shell `playable_claim`
//! stays false; network deferred.
//!
//! Sources (retail ZH C++ / INI):
//! - SpecialPower.h/.cpp SpecialPowerTemplate ctor defaults + FieldParse /
//!   SpecialPowerStore m_nextSpecialPowerID / DEFAULT_DEFECTION_DETECTION
//! - SpecialPower.ini residual template deepen (RequiredScience / PublicTimer /
//!   SharedSyncedTimer / ViewObject* / RadiusCursor / ShortcutPower)
//! - Science.h/.cpp ScienceInfo defaults + ScienceStore purchase cost residual
//! - Science.ini residual PointCost / PrerequisiteSciences sample rows
//! - Upgrade.h/.cpp UpgradeType / UpgradeStatusType / UPGRADE_MAX_COUNT /
//!   UpgradeTemplate ctor + calcTimeToBuild residual
//! - Upgrade.ini residual Type/BuildCost/BuildTime sample rows
//! - Player.h ScienceAvailabilityType / NUM_HOTKEY_SQUADS /
//!   SpecialPowerReadyTimerType / PlayerList AllowPlayerRelationship
//! - Team.h TeamFactory unique ID counters / Team instance active residual
//!
//! Fail-closed:
//! - Not full SpecialPowerStore SharedSyncedTimer UI / live canUseSpecialPower
//! - Not full ScienceStore NameKey purchase graph / getPurchasableSciences UI
//! - Not full UpgradeCenter multipleyer replication residual
//! - Not full Player energy / science purchase exclusive matrix residual
//! - Not full TeamFactory production / AI recruit residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

/// C++ `LOGICFRAMES_PER_SECOND` residual.
pub const LOGICFRAMES_PER_SECOND_RESIDUAL: u32 = 30;

/// C++ `ConvertDurationFromMsecsToFrames` residual: ceil(msec * 30 / 1000).
#[inline]
pub fn duration_ms_to_logic_frames_wave109(msec: u32) -> u32 {
    if msec == 0 {
        return 0;
    }
    ((msec as u64 * LOGICFRAMES_PER_SECOND_RESIDUAL as u64 + 999) / 1000) as u32
}

// ---------------------------------------------------------------------------
// 1. SpecialPower residual template store residual
// ---------------------------------------------------------------------------

/// C++ `SPECIAL_INVALID` residual (SpecialPowerType first enum).
pub const SPECIAL_INVALID_RESIDUAL: i32 = 0;
/// C++ `SCIENCE_INVALID` residual (-1).
pub const SCIENCE_INVALID_RESIDUAL: i32 = -1;
/// C++ `INVALID_ID` residual (GameType.h).
pub const INVALID_ID_RESIDUAL: u32 = 0;

/// C++ `DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT`
/// = `LOGICFRAMES_PER_SECOND * 10` (SpecialPower.cpp).
pub const DEFAULT_DETECTION_TIME_FRAMES_RESIDUAL: u32 = LOGICFRAMES_PER_SECOND_RESIDUAL * 10;

/// C++ SpecialPowerTemplate FieldParse residual field names (order from
/// SpecialPower.cpp `m_specialPowerFieldParse`).
pub const SPECIAL_POWER_TEMPLATE_FIELD_PARSE_NAMES_WAVE109: &[&str] = &[
    "ReloadTime",
    "RequiredScience",
    "InitiateSound",
    "InitiateAtLocationSound",
    "PublicTimer",
    "Enum",
    "DetectionTime",
    "SharedSyncedTimer",
    "ViewObjectDuration",
    "ViewObjectRange",
    "RadiusCursorRadius",
    "ShortcutPower",
    "AcademyClassify",
];

/// C++ SpecialPowerTemplate ctor residual defaults (SpecialPower.cpp).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecialPowerTemplateCtorDefaultsResidual {
    pub id: u32,
    pub type_enum: i32,
    pub reload_time_frames: u32,
    pub required_science: i32,
    pub public_timer: bool,
    pub detection_time_frames: u32,
    pub shared_n_sync: bool,
    pub view_object_duration_frames: u32,
    pub view_object_range: f32,
    pub radius_cursor_radius: f32,
    pub shortcut_power: bool,
}

/// Ctor residual defaults.
pub const SPECIAL_POWER_TEMPLATE_CTOR_DEFAULTS_WAVE109: SpecialPowerTemplateCtorDefaultsResidual =
    SpecialPowerTemplateCtorDefaultsResidual {
        id: 0,
        type_enum: SPECIAL_INVALID_RESIDUAL,
        reload_time_frames: 0,
        required_science: SCIENCE_INVALID_RESIDUAL,
        public_timer: false,
        detection_time_frames: DEFAULT_DETECTION_TIME_FRAMES_RESIDUAL,
        shared_n_sync: false,
        view_object_duration_frames: 0,
        view_object_range: 0.0,
        radius_cursor_radius: 0.0,
        shortcut_power: false,
    };

/// SpecialPower.ini residual template row (store residual deepen beyond Wave 103
/// reload-only table). Durations stored as retail INI milliseconds; host converts
/// to logic frames via `duration_ms_to_logic_frames_wave109`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecialPowerTemplateResidualRowWave109 {
    pub template_name: &'static str,
    pub enum_name: &'static str,
    pub reload_ms: u32,
    pub required_science: &'static str,
    pub public_timer: bool,
    pub shared_synced_timer: bool,
    pub view_object_duration_ms: u32,
    pub view_object_range: f32,
    pub radius_cursor_radius: f32,
    pub shortcut_power: bool,
}

/// Wave 109 SpecialPower residual template store sample rows.
pub const SPECIAL_POWER_TEMPLATE_STORE_TABLE_WAVE109: &[SpecialPowerTemplateResidualRowWave109] = &[
    SpecialPowerTemplateResidualRowWave109 {
        template_name: "SuperweaponDaisyCutter",
        enum_name: "SPECIAL_DAISY_CUTTER",
        reload_ms: 360_000,
        required_science: "SCIENCE_DaisyCutter",
        public_timer: false,
        shared_synced_timer: true,
        view_object_duration_ms: 30_000,
        view_object_range: 250.0,
        radius_cursor_radius: 170.0,
        shortcut_power: true,
    },
    SpecialPowerTemplateResidualRowWave109 {
        template_name: "SuperweaponA10ThunderboltMissileStrike",
        enum_name: "SPECIAL_A10_THUNDERBOLT_STRIKE",
        reload_ms: 240_000,
        required_science: "SCIENCE_A10ThunderboltMissileStrike1",
        public_timer: false,
        shared_synced_timer: true,
        view_object_duration_ms: 30_000,
        view_object_range: 250.0,
        radius_cursor_radius: 50.0,
        shortcut_power: true,
    },
    SpecialPowerTemplateResidualRowWave109 {
        template_name: "SuperweaponScudStorm",
        enum_name: "SPECIAL_SCUD_STORM",
        reload_ms: 300_000,
        required_science: "",
        public_timer: true,
        shared_synced_timer: false, // omitted in INI → ctor default false
        view_object_duration_ms: 40_000,
        view_object_range: 250.0,
        radius_cursor_radius: 200.0,
        shortcut_power: true,
    },
    SpecialPowerTemplateResidualRowWave109 {
        template_name: "SuperweaponParticleUplinkCannon",
        enum_name: "SPECIAL_PARTICLE_UPLINK_CANNON",
        reload_ms: 240_000,
        required_science: "",
        public_timer: true,
        shared_synced_timer: false,
        view_object_duration_ms: 30_000,
        view_object_range: 250.0,
        radius_cursor_radius: 0.0,
        shortcut_power: true,
    },
    SpecialPowerTemplateResidualRowWave109 {
        template_name: "SpecialPowerSpySatellite",
        enum_name: "SPECIAL_SPY_SATELLITE",
        reload_ms: 60_000,
        required_science: "",
        public_timer: false,
        shared_synced_timer: true,
        view_object_duration_ms: 0,
        view_object_range: 0.0,
        radius_cursor_radius: 300.0,
        shortcut_power: true,
    },
    SpecialPowerTemplateResidualRowWave109 {
        template_name: "SpecialPowerRadarVanScan",
        enum_name: "SPECIAL_RADAR_VAN_SCAN",
        reload_ms: 30_000,
        required_science: "",
        public_timer: false,
        shared_synced_timer: false,
        view_object_duration_ms: 0,
        view_object_range: 0.0,
        radius_cursor_radius: 150.0,
        shortcut_power: true,
    },
    SpecialPowerTemplateResidualRowWave109 {
        template_name: "SuperweaponCarpetBomb",
        enum_name: "SPECIAL_CARPET_BOMB",
        reload_ms: 150_000,
        required_science: "",
        public_timer: true,
        shared_synced_timer: true,
        view_object_duration_ms: 40_000,
        view_object_range: 250.0,
        radius_cursor_radius: 100.0,
        shortcut_power: true,
    },
    SpecialPowerTemplateResidualRowWave109 {
        template_name: "SuperweaponClusterMines",
        enum_name: "SPECIAL_CLUSTER_MINES",
        reload_ms: 240_000,
        required_science: "SCIENCE_ClusterMines",
        public_timer: false,
        shared_synced_timer: true,
        view_object_duration_ms: 30_000,
        view_object_range: 250.0,
        radius_cursor_radius: 100.0,
        shortcut_power: true,
    },
    SpecialPowerTemplateResidualRowWave109 {
        template_name: "SuperweaponParadropAmerica",
        enum_name: "SPECIAL_PARADROP_AMERICA",
        reload_ms: 240_000,
        required_science: "SCIENCE_Paradrop1",
        public_timer: false,
        shared_synced_timer: true,
        view_object_duration_ms: 0,
        view_object_range: 0.0,
        radius_cursor_radius: 50.0,
        shortcut_power: true,
    },
    SpecialPowerTemplateResidualRowWave109 {
        template_name: "SpecialPowerSpyDrone",
        enum_name: "SPECIAL_SPY_DRONE",
        reload_ms: 90_000,
        required_science: "SCIENCE_SpyDrone",
        public_timer: false,
        shared_synced_timer: true,
        view_object_duration_ms: 0,
        view_object_range: 0.0,
        radius_cursor_radius: 250.0,
        shortcut_power: true,
    },
];

/// Lookup SpecialPower residual template row by name.
pub fn special_power_template_row_wave109(
    name: &str,
) -> Option<&'static SpecialPowerTemplateResidualRowWave109> {
    SPECIAL_POWER_TEMPLATE_STORE_TABLE_WAVE109
        .iter()
        .find(|r| r.template_name == name)
}

/// Host SpecialPowerStore residual: next ID starts 0; parse assigns ++id.
#[derive(Debug, Clone, Default)]
pub struct HostSpecialPowerStoreResidualWave109 {
    pub next_special_power_id: u32,
    pub templates: Vec<(&'static str, u32)>,
}

impl HostSpecialPowerStoreResidualWave109 {
    pub fn new() -> Self {
        Self {
            next_special_power_id: 0,
            templates: Vec::new(),
        }
    }

    /// C++ parseSpecialPowerDefinition residual: `++m_nextSpecialPowerID`.
    pub fn register_template(&mut self, name: &'static str) -> u32 {
        self.next_special_power_id = self.next_special_power_id.saturating_add(1);
        let id = self.next_special_power_id;
        self.templates.push((name, id));
        id
    }

    pub fn find_by_name(&self, name: &str) -> Option<u32> {
        self.templates
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, id)| *id)
    }

    pub fn find_by_id(&self, id: u32) -> Option<&'static str> {
        self.templates
            .iter()
            .find(|(_, i)| *i == id)
            .map(|(n, _)| *n)
    }

    pub fn get_num_special_powers(&self) -> usize {
        self.templates.len()
    }
}

/// Wave 109 honesty: SpecialPower residual template store residual pack.
///
/// Freezes FieldParse names, ctor defaults (DetectionTime **300**f), template
/// residual table (RequiredScience / PublicTimer / SharedSyncedTimer /
/// ViewObject / RadiusCursor / ShortcutPower), and store ++id residual.
/// Fail-closed: not full SharedSyncedTimer UI / live canUseSpecialPower.

// ---------------------------------------------------------------------------
// 2. Science residual store residual deepen
// ---------------------------------------------------------------------------

/// C++ ScienceInfo ctor residual: cost **0** means "cannot be purchased".
pub const SCIENCE_PURCHASE_COST_UNPURCHASABLE_RESIDUAL: i32 = 0;
/// C++ ScienceInfo ctor residual: m_grantable default **true**.
pub const SCIENCE_GRANTABLE_DEFAULT_RESIDUAL: bool = true;

/// C++ ScienceAvailabilityType residual names (Player.h DEFINE_SCIENCE_AVAILABILITY_NAMES).
pub const SCIENCE_AVAILABILITY_NAME_TABLE_WAVE109: &[&str] = &["Available", "Disabled", "Hidden"];
pub const SCIENCE_AVAILABILITY_INVALID: i32 = -1;
pub const SCIENCE_AVAILABLE: u32 = 0;
pub const SCIENCE_DISABLED: u32 = 1;
pub const SCIENCE_HIDDEN: u32 = 2;
pub const SCIENCE_AVAILABILITY_COUNT: usize = 3;

/// Science.ini residual store row (name / PointCost / prereqs / grantable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScienceStoreResidualRowWave109 {
    pub name: &'static str,
    pub point_cost: i32,
    pub prereq_a: &'static str,
    pub prereq_b: &'static str,
    pub grantable: bool,
}

/// Wave 109 Science residual store sample rows (beyond Wave 92 name-only table).
pub const SCIENCE_STORE_TABLE_WAVE109: &[ScienceStoreResidualRowWave109] = &[
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_AMERICA",
        point_cost: 0,
        prereq_a: "",
        prereq_b: "",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_CHINA",
        point_cost: 0,
        prereq_a: "",
        prereq_b: "",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_GLA",
        point_cost: 0,
        prereq_a: "",
        prereq_b: "",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_Rank1",
        point_cost: 0,
        prereq_a: "",
        prereq_b: "",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_PaladinTank",
        point_cost: 1,
        prereq_a: "SCIENCE_AMERICA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_DaisyCutter",
        point_cost: 1,
        prereq_a: "SCIENCE_AMERICA",
        prereq_b: "",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_MOAB",
        point_cost: 0, // not purchasable (upgrade path residual)
        prereq_a: "SCIENCE_AMERICA",
        prereq_b: "",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_ClusterMines",
        point_cost: 1,
        prereq_a: "SCIENCE_CHINA",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_ArtilleryBarrage1",
        point_cost: 1,
        prereq_a: "SCIENCE_CHINA",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_CashBounty1",
        point_cost: 1,
        prereq_a: "SCIENCE_GLA",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    // Retail Science.ini CashBounty chain: 1 → 2 → 3 (each purchase cost 1,
    // level N requires level N-1 + faction + Rank3). CashBounty3 below already
    // requires CashBounty2, so the middle link must be purchasable too.
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_CashBounty2",
        point_cost: 1,
        prereq_a: "SCIENCE_CashBounty1",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_CashBounty3",
        point_cost: 1,
        prereq_a: "SCIENCE_CashBounty2",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    // Rank.ini SciencesGranted — cost 0 (not purchasable).
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_Rank2",
        point_cost: 0,
        prereq_a: "",
        prereq_b: "",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_Rank3",
        point_cost: 0,
        prereq_a: "",
        prereq_b: "",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_Rank4",
        point_cost: 0,
        prereq_a: "",
        prereq_b: "",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_Rank5",
        point_cost: 0,
        prereq_a: "",
        prereq_b: "",
        grantable: true,
    },
    // America Rank1 purchasables (Science.ini + PlayerTemplate command set).
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_StealthFighter",
        point_cost: 1,
        prereq_a: "SCIENCE_AMERICA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_Pathfinder",
        point_cost: 1,
        prereq_a: "SCIENCE_AMERICA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_SpyDrone",
        point_cost: 1,
        prereq_a: "SCIENCE_AMERICA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    // America Rank3 purchasables.
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_A10ThunderboltMissileStrike1",
        point_cost: 1,
        prereq_a: "SCIENCE_AMERICA",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_A10ThunderboltMissileStrike2",
        point_cost: 1,
        prereq_a: "SCIENCE_A10ThunderboltMissileStrike1",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_A10ThunderboltMissileStrike3",
        point_cost: 1,
        prereq_a: "SCIENCE_A10ThunderboltMissileStrike2",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_EmergencyRepair1",
        point_cost: 1,
        prereq_a: "SCIENCE_AMERICA",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_Paradrop1",
        point_cost: 1,
        prereq_a: "SCIENCE_AMERICA",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    // China Rank1 / Rank3 purchasables.
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_RedGuardTraining",
        point_cost: 1,
        prereq_a: "SCIENCE_CHINA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_BattlemasterTraining",
        point_cost: 1,
        prereq_a: "SCIENCE_CHINA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_ArtilleryTraining",
        point_cost: 1,
        prereq_a: "SCIENCE_CHINA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_NukeCannon",
        point_cost: 1,
        prereq_a: "SCIENCE_CHINA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_ArtilleryBarrage2",
        point_cost: 1,
        prereq_a: "SCIENCE_ArtilleryBarrage1",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_ArtilleryBarrage3",
        point_cost: 1,
        prereq_a: "SCIENCE_ArtilleryBarrage2",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    // GLA Rank1 / Rank3 purchasables.
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_RebelAmbush1",
        point_cost: 1,
        prereq_a: "SCIENCE_GLA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_MarauderTank",
        point_cost: 1,
        prereq_a: "SCIENCE_GLA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_ScudLauncher",
        point_cost: 1,
        prereq_a: "SCIENCE_GLA",
        prereq_b: "SCIENCE_Rank1",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_AnthraxBomb",
        point_cost: 1,
        prereq_a: "SCIENCE_GLA",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_SneakAttack",
        point_cost: 1,
        prereq_a: "SCIENCE_GLA",
        prereq_b: "SCIENCE_Rank3",
        grantable: true,
    },
    // C++ ScienceInfo::m_grantable = No — PLAYER_GRANT_SCIENCE must refuse.
    ScienceStoreResidualRowWave109 {
        name: "SCIENCE_NonGrantableTest",
        point_cost: 0,
        prereq_a: "",
        prereq_b: "",
        grantable: false,
    },
];

/// C++ residual: cost **0** → cannot be purchased (not free).
#[inline]

/// C++ `ScienceStore::getSciencePurchaseCost` (Science.cpp:213-223).
///
/// Missing Science.ini / residual row → **0** (not purchasable). Never invent cost 1.

/// Normalize host/UI science tokens to retail SCIENCE_* residual names when known.
pub fn normalize_science_name_residual(science_name: &str) -> String {
    let raw = science_name.trim();
    if raw.is_empty() {
        return String::new();
    }
    // Already a SCIENCE_ / faction-prefix science token.
    let lower = raw.to_ascii_lowercase().replace('-', "_");
    if lower.starts_with("science_")
        || lower.starts_with("airf_science_")
        || lower.starts_with("nuke_science_")
        || lower.starts_with("early_science_")
        || lower.starts_with("supw_science_")
        || lower.starts_with("chem_science_")
        || lower.starts_with("slth_science_")
        || lower.starts_with("infa_science_")
        || lower.starts_with("tank_science_")
    {
        // Preserve original casing style for SCIENCE_ tokens.
        if raw.starts_with("SCIENCE_") || raw.contains("_SCIENCE_") {
            return raw.to_string();
        }
        // Rebuild SCIENCE_ prefix from lower form when possible.
        if lower.starts_with("science_") {
            return format!(
                "SCIENCE_{}",
                &raw[raw.find('_').map(|i| i + 1).unwrap_or(0)..]
            );
        }
        return raw.to_string();
    }
    // Compact command tokens residual (ControlBar / test aliases).
    match lower.as_str() {
        "a10strike1" | "a10_strike_1" | "a10thunderboltmissilestrike1" => {
            "SCIENCE_A10ThunderboltMissileStrike1".into()
        }
        "a10strike2" | "a10_strike_2" => "SCIENCE_A10ThunderboltMissileStrike2".into(),
        "a10strike3" | "a10_strike_3" => "SCIENCE_A10ThunderboltMissileStrike3".into(),
        "daisycutter" | "daisy_cutter" => "SCIENCE_DaisyCutter".into(),
        "spydrone" | "spy_drone" => "SCIENCE_SpyDrone".into(),
        "cashbounty1" | "cash_bounty_1" | "cashbounty" => "SCIENCE_CashBounty1".into(),
        "cashbounty2" | "cash_bounty_2" => "SCIENCE_CashBounty2".into(),
        "cashbounty3" | "cash_bounty_3" => "SCIENCE_CashBounty3".into(),
        "paladintank" | "paladin_tank" => "SCIENCE_PaladinTank".into(),
        "clustermines" | "cluster_mines" => "SCIENCE_ClusterMines".into(),
        "emergencyrepair1" | "emergency_repair_1" | "emergencyrepair" => {
            "SCIENCE_EmergencyRepair1".into()
        }
        "paradrop1" | "paradrop" => "SCIENCE_Paradrop1".into(),
        "spectregunship" | "spectre_gunship" => "SCIENCE_SpectreGunshipSolo".into(),
        "fuelairbomb" | "fuel_air_bomb" | "moab" => "SCIENCE_DaisyCutter".into(), // MOAB upgrade path
        other => {
            // Title-case SCIENCE_Other residual.
            let mut out = String::from("SCIENCE_");
            let mut cap = true;
            for ch in other.chars() {
                if ch == '_' {
                    out.push('_');
                    cap = true;
                } else if cap {
                    out.extend(ch.to_uppercase());
                    cap = false;
                } else {
                    out.push(ch);
                }
            }
            out
        }
    }
}

pub fn science_purchase_point_cost_residual(science_name: &str) -> Option<i32> {
    let name_owned = normalize_science_name_residual(science_name);
    let name = name_owned.as_str();
    if name.is_empty() {
        return None;
    }
    if let Some(cost) = science_ini_purchase_cost(name) {
        return Some(cost);
    }
    if let Some(row) = science_store_row_wave109(name) {
        return Some(row.point_cost);
    }
    // C++ Science.cpp:220-222 — unknown science costs 0 (cannot purchase).
    if is_science_token_name(name) {
        return Some(SCIENCE_PURCHASE_COST_UNPURCHASABLE_RESIDUAL);
    }
    None
}

fn is_science_token_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("science_")
        || n.starts_with("airf_science_")
        || n.starts_with("nuke_science_")
        || n.starts_with("early_science_")
        || n.starts_with("supw_science_")
        || n.starts_with("chem_science_")
        || n.starts_with("slth_science_")
        || n.starts_with("infa_science_")
        || n.starts_with("tank_science_")
}

fn science_ini_purchase_cost(name: &str) -> Option<i32> {
    if let Some(cost) = science_ini_store_cost(name) {
        return Some(cost);
    }
    if let Some(store) = game_engine::common::rts::get_science_store() {
        let science = store.get_science_from_internal_name(name);
        if science != game_engine::common::rts::SCIENCE_INVALID {
            return Some(store.get_science_purchase_cost(science));
        }
    }
    None
}

fn science_ini_store_cost(name: &str) -> Option<i32> {
    let store = game_engine::common::ini::ini_science::get_science_store();
    let science = store.get_science_from_internal_name(name)?;
    Some(store.get_science_purchase_cost(science))
}

fn science_ini_prereq_names(name: &str) -> Option<Vec<String>> {
    {
        let leftover = game_engine::common::ini::ini_science::get_science_store();
        if let Some(st) = leftover.get_science_from_internal_name(name) {
            if let Some(info) = leftover.find_science_info(st) {
                return Some(
                    info.prereq_sciences
                        .iter()
                        .filter_map(|p| leftover.get_internal_name_for_science(*p))
                        .map(|a| a.to_string())
                        .collect(),
                );
            }
        }
    }
    let rts = game_engine::common::rts::get_science_store()?;
    let st = rts.get_science_from_internal_name(name);
    if st == game_engine::common::rts::SCIENCE_INVALID {
        return None;
    }
    let info = rts.find_science_info(st)?;
    Some(
        info.prereq_sciences
            .iter()
            .map(|&p| rts.get_internal_name_for_science(p).to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

/// C++ `ScienceStore::playerHasPrereqsForScience` (Science.cpp:257-274).
///
/// Missing Science.ini / residual row → **false** (fail-closed).
pub fn science_prereqs_met_residual(
    unlocked: &std::collections::HashSet<String>,
    science_name: &str,
) -> bool {
    let science_name = normalize_science_name_residual(science_name);
    let science_name = science_name.as_str();
    let has = |req: &str| -> bool {
        if req.is_empty() {
            return true;
        }
        let r = req.to_ascii_lowercase().replace('-', "_");
        unlocked.iter().any(|u| {
            let n = u.to_ascii_lowercase().replace('-', "_");
            n == r
        })
    };
    if let Some(prereqs) = science_ini_prereq_names(science_name) {
        return prereqs.iter().all(|p| has(p));
    }
    if let Some(row) = science_store_row_wave109(science_name.trim()) {
        return has(row.prereq_a) && has(row.prereq_b);
    }
    // C++ Science.cpp:271-273 — unknown science has no prereq info → false.
    false
}

/// C++ `Player::isCapableOfPurchasingScience` residual.
pub fn is_capable_of_purchasing_science_residual(
    unlocked: &std::collections::HashSet<String>,
    science_purchase_points: i32,
    science_name: &str,
) -> bool {
    let name_owned = normalize_science_name_residual(science_name);
    let name = name_owned.as_str();
    if name.is_empty() {
        return false;
    }
    // Already owned.
    let key = name.to_ascii_lowercase().replace('-', "_");
    if unlocked.iter().any(|u| {
        let n = u.to_ascii_lowercase().replace('-', "_");
        n == key
    }) {
        return false;
    }
    let Some(cost) = science_purchase_point_cost_residual(name) else {
        // Unknown non-SCIENCE name: not purchasable residual.
        return false;
    };
    if !science_is_purchasable_residual(cost) {
        return false;
    }
    if cost > science_purchase_points {
        return false;
    }
    science_prereqs_met_residual(unlocked, name)
}

pub fn science_is_purchasable_residual(point_cost: i32) -> bool {
    point_cost > SCIENCE_PURCHASE_COST_UNPURCHASABLE_RESIDUAL
}

/// C++ `ScienceStore::getPurchasableSciences` first capable (Science.cpp:301-328).
/// Walks leftover leftover / live Science.ini store order — not a hardcoded 5-name list.
pub fn first_capable_purchase_science_residual(
    unlocked: &std::collections::HashSet<String>,
    science_purchase_points: i32,
) -> Option<String> {
    if let Some(name) = first_capable_from_leftover_science_store(unlocked, science_purchase_points)
    {
        return Some(name);
    }
    if let Some(name) = first_capable_from_live_science_store(unlocked, science_purchase_points) {
        return Some(name);
    }
    // Residual sample table only when neither store has Science.ini.
    for row in SCIENCE_STORE_TABLE_WAVE109 {
        if !science_is_purchasable_residual(row.point_cost) {
            continue;
        }
        if is_capable_of_purchasing_science_residual(unlocked, science_purchase_points, row.name) {
            return Some(row.name.to_string());
        }
    }
    None
}

fn leftover_unlocked_science_types(
    unlocked: &std::collections::HashSet<String>,
) -> std::collections::HashSet<game_engine::common::ini::ini_science::ScienceType> {
    use game_engine::common::ini::ini_science::ScienceType;
    use game_engine::common::name_key_generator::NameKeyGenerator;
    unlocked
        .iter()
        .map(|name| {
            let canon = normalize_science_name_residual(name);
            ScienceType(NameKeyGenerator::name_to_key(&canon) as i32)
        })
        .collect()
}

fn first_capable_from_leftover_science_store(
    unlocked: &std::collections::HashSet<String>,
    science_purchase_points: i32,
) -> Option<String> {
    let leftover = game_engine::common::ini::ini_science::get_science_store();
    if leftover.is_empty() {
        return None;
    }
    let owned = leftover_unlocked_science_types(unlocked);
    let has = |st| owned.contains(&st);
    let (purchasable, _) = leftover.get_purchasable_sciences(&has);
    for science in purchasable {
        let Some(name) = leftover.get_internal_name_for_science(science) else {
            continue;
        };
        if is_capable_of_purchasing_science_residual(
            unlocked,
            science_purchase_points,
            name.as_str(),
        ) {
            return Some(name.to_string());
        }
    }
    None
}

fn first_capable_from_live_science_store(
    unlocked: &std::collections::HashSet<String>,
    science_purchase_points: i32,
) -> Option<String> {
    use game_engine::common::rts::science::{ScienceAccess, ScienceType};
    struct UnlockedAccess<'a>(&'a std::collections::HashSet<String>);
    impl ScienceAccess for UnlockedAccess<'_> {
        fn has_science(&self, science: ScienceType) -> bool {
            let Some(store) = game_engine::common::rts::get_science_store() else {
                return false;
            };
            let name = store.get_internal_name_for_science(science);
            if name.is_empty() {
                return false;
            }
            let key = normalize_science_name_residual(&name).to_ascii_lowercase();
            self.0
                .iter()
                .any(|owned| normalize_science_name_residual(owned).to_ascii_lowercase() == key)
        }
    }
    let store = game_engine::common::rts::get_science_store()?;
    if store.get_science_names().is_empty() {
        return None;
    }
    let (purchasable, _) = store.get_purchasable_sciences(&UnlockedAccess(unlocked));
    for science in purchasable {
        let name = store.get_internal_name_for_science(science).to_string();
        if name.is_empty() {
            continue;
        }
        if is_capable_of_purchasing_science_residual(unlocked, science_purchase_points, &name) {
            return Some(name);
        }
    }
    None
}

/// C++ ControlBarCommand.cpp:1219-1261 — COMMAND_RESTRICTED unless player owns ScienceVec.
pub fn host_command_button_science_vec_met(
    unlocked: &std::collections::HashSet<String>,
    required: &[&str],
) -> bool {
    required.iter().all(|req| {
        if req.trim().is_empty() {
            return true;
        }
        let want = normalize_science_name_residual(req).to_ascii_lowercase();
        unlocked
            .iter()
            .any(|owned| normalize_science_name_residual(owned).to_ascii_lowercase() == want)
    })
}

/// Mirror host `Player::addScience` onto crate `Player` ScienceVec so
/// ControlBar `sciences_ids` (impl_command_context.rs) sees the purchase.
pub fn sync_host_science_to_crate_player(player_id: u32, science_name: &str) -> bool {
    let canonical = normalize_science_name_residual(science_name);
    if canonical.is_empty() {
        return false;
    }
    let science = science_type_from_name(&canonical);
    if science == game_engine::common::rts::SCIENCE_INVALID {
        return false;
    }
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return false;
    };
    let player_arc = list
        .get_player(player_id as gamelogic::player::PlayerIndex)
        .cloned()
        .or_else(|| list.get_local_player().cloned());
    let Some(player_arc) = player_arc else {
        return false;
    };
    drop(list);
    let Ok(mut player) = player_arc.write() else {
        return false;
    };
    player.add_science(science)
}

/// Mirror host SPP onto crate Player so ControlBar remaining-points matches.
pub fn sync_host_spp_to_crate_player(player_id: u32, spp: i32) -> bool {
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return false;
    };
    let player_arc = list
        .get_player(player_id as gamelogic::player::PlayerIndex)
        .cloned()
        .or_else(|| list.get_local_player().cloned());
    let Some(player_arc) = player_arc else {
        return false;
    };
    drop(list);
    let Ok(mut player) = player_arc.write() else {
        return false;
    };
    let current = player.get_science_purchase_points();
    let delta = spp - current;
    if delta != 0 {
        player.add_science_purchase_points(delta);
    }
    true
}

/// C++ `Player::isScienceDisabled` / `isScienceHidden` from leftover Player
/// written by `PLAYER_SCIENCE_AVAILABILITY` (ScriptActions).
pub fn leftover_science_hidden_or_disabled(player_id: u32, science_name: &str) -> bool {
    let canonical = normalize_science_name_residual(science_name);
    if canonical.is_empty() {
        return false;
    }
    let science = science_type_from_name(&canonical);
    if science == game_engine::common::rts::SCIENCE_INVALID {
        return false;
    }
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return false;
    };
    let Some(player_arc) = list
        .get_player(player_id as gamelogic::player::PlayerIndex)
        .cloned()
    else {
        return false;
    };
    drop(list);
    let Ok(player) = player_arc.read() else {
        return false;
    };
    player.is_science_disabled(science) || player.is_science_hidden(science)
}

fn science_type_from_name(name: &str) -> game_engine::common::rts::ScienceType {
    if let Some(store) = game_engine::common::rts::get_science_store() {
        let science = store.get_science_from_internal_name(name);
        if science != game_engine::common::rts::SCIENCE_INVALID {
            return science;
        }
    }
    if let Some(science) = game_engine::common::ini::ini_science::get_science_store()
        .get_science_from_internal_name(name)
    {
        return science.0;
    }
    game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(name)
        as game_engine::common::rts::ScienceType
}

/// Lookup Science residual store row by name.
pub fn science_store_row_wave109(name: &str) -> Option<&'static ScienceStoreResidualRowWave109> {
    SCIENCE_STORE_TABLE_WAVE109.iter().find(|r| r.name == name)
}

/// Host ScienceStore residual: find by name, purchase cost, grantable.
#[derive(Debug, Clone, Default)]
pub struct HostScienceStoreResidualWave109 {
    pub sciences: Vec<&'static ScienceStoreResidualRowWave109>,
}

impl HostScienceStoreResidualWave109 {
    pub fn from_table() -> Self {
        Self {
            sciences: SCIENCE_STORE_TABLE_WAVE109.iter().collect(),
        }
    }

    pub fn find(&self, name: &str) -> Option<&'static ScienceStoreResidualRowWave109> {
        self.sciences.iter().copied().find(|r| r.name == name)
    }

    pub fn get_purchase_cost(&self, name: &str) -> Option<i32> {
        self.find(name).map(|r| r.point_cost)
    }

    pub fn is_grantable(&self, name: &str) -> Option<bool> {
        self.find(name).map(|r| r.grantable)
    }

    pub fn count(&self) -> usize {
        self.sciences.len()
    }
}

/// Wave 109 honesty: Science residual store residual deepen pack.
///
/// Freezes SCIENCE_INVALID, availability names, PointCost unpurchasable=0,
/// sample prereq/cost residual rows, and host store find residual.
/// Fail-closed: not full NameKey graph / getPurchasableSciences UI residual.

// ---------------------------------------------------------------------------
// 3. Upgrade residual store residual deepen
// ---------------------------------------------------------------------------

/// C++ `UpgradeType` residual.
pub const UPGRADE_TYPE_PLAYER: u32 = 0;
pub const UPGRADE_TYPE_OBJECT: u32 = 1;
pub const NUM_UPGRADE_TYPES: usize = 2;
pub const UPGRADE_TYPE_NAME_TABLE_WAVE109: &[&str] = &["PLAYER", "OBJECT"];

/// C++ `UpgradeStatusType` residual.
pub const UPGRADE_STATUS_INVALID: u32 = 0;
pub const UPGRADE_STATUS_IN_PRODUCTION: u32 = 1;
pub const UPGRADE_STATUS_COMPLETE: u32 = 2;
pub const UPGRADE_STATUS_NAME_TABLE_WAVE109: &[&str] = &[
    "UPGRADE_STATUS_INVALID",
    "UPGRADE_STATUS_IN_PRODUCTION",
    "UPGRADE_STATUS_COMPLETE",
];

/// C++ `UPGRADE_MAX_COUNT` residual (Upgrade.h).
pub const UPGRADE_MAX_COUNT_RESIDUAL: usize = 128;

/// C++ UpgradeTemplate FieldParse residual field names.
pub const UPGRADE_TEMPLATE_FIELD_PARSE_NAMES_WAVE109: &[&str] = &[
    "DisplayName",
    "Type",
    "BuildTime",
    "BuildCost",
    "ButtonImage",
    "ResearchSound",
    "UnitSpecificSound",
    "AcademyClassify",
];

/// C++ UpgradeTemplate ctor residual defaults.
pub const UPGRADE_TEMPLATE_CTOR_TYPE_DEFAULT: u32 = UPGRADE_TYPE_PLAYER;
pub const UPGRADE_TEMPLATE_CTOR_COST_DEFAULT: i32 = 0;
pub const UPGRADE_TEMPLATE_CTOR_BUILD_TIME_DEFAULT: f32 = 0.0;
pub const NAMEKEY_INVALID_RESIDUAL: u32 = 0;

/// Upgrade.ini residual store row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UpgradeStoreResidualRowWave109 {
    pub name: &'static str,
    pub upgrade_type: u32, // PLAYER=0 OBJECT=1 (default PLAYER when omitted)
    pub build_time_sec: f32,
    pub build_cost: i32,
    pub display_name: &'static str,
}

/// Wave 109 Upgrade residual store sample rows (beyond Wave 94 name-only table).
pub const UPGRADE_STORE_TABLE_WAVE109: &[UpgradeStoreResidualRowWave109] = &[
    UpgradeStoreResidualRowWave109 {
        name: "Upgrade_Nationalism",
        upgrade_type: UPGRADE_TYPE_PLAYER,
        build_time_sec: 60.0,
        build_cost: 2000,
        display_name: "UPGRADE:Nationalism",
    },
    UpgradeStoreResidualRowWave109 {
        name: "Upgrade_AmericaRadar",
        upgrade_type: UPGRADE_TYPE_OBJECT,
        build_time_sec: 10.0,
        build_cost: 500,
        display_name: "UPGRADE:Radar",
    },
    UpgradeStoreResidualRowWave109 {
        name: "Upgrade_AmericaAdvancedControlRods",
        upgrade_type: UPGRADE_TYPE_OBJECT,
        build_time_sec: 30.0,
        build_cost: 500,
        display_name: "UPGRADE:ControlRods",
    },
    UpgradeStoreResidualRowWave109 {
        name: "Upgrade_AmericaSupplyLines",
        upgrade_type: UPGRADE_TYPE_PLAYER,
        build_time_sec: 30.0,
        build_cost: 800,
        display_name: "UPGRADE:SupplyLines",
    },
    UpgradeStoreResidualRowWave109 {
        name: "Upgrade_AmericaRangerFlashBangGrenade",
        upgrade_type: UPGRADE_TYPE_PLAYER,
        build_time_sec: 30.0,
        build_cost: 800,
        display_name: "UPGRADE:RangerFlashBangGrenade",
    },
    UpgradeStoreResidualRowWave109 {
        name: "Upgrade_AmericaTOWMissile",
        upgrade_type: UPGRADE_TYPE_PLAYER,
        build_time_sec: 30.0,
        build_cost: 800,
        display_name: "UPGRADE:TOWMissile",
    },
    UpgradeStoreResidualRowWave109 {
        name: "Upgrade_ComancheRocketPods",
        upgrade_type: UPGRADE_TYPE_PLAYER,
        build_time_sec: 40.0,
        build_cost: 800,
        display_name: "UPGRADE:ComancheRocketPods",
    },
    UpgradeStoreResidualRowWave109 {
        name: "Upgrade_AmericaCompositeArmor",
        upgrade_type: UPGRADE_TYPE_PLAYER,
        build_time_sec: 60.0,
        build_cost: 2000,
        display_name: "UPGRADE:CompositeArmor",
    },
    UpgradeStoreResidualRowWave109 {
        name: "Upgrade_ChinaNuclearTanks",
        upgrade_type: UPGRADE_TYPE_PLAYER,
        build_time_sec: 60.0,
        build_cost: 2000,
        display_name: "UPGRADE:NuclearTanks",
    },
    UpgradeStoreResidualRowWave109 {
        name: "Upgrade_GLACamouflage",
        upgrade_type: UPGRADE_TYPE_PLAYER,
        build_time_sec: 60.0,
        build_cost: 2000,
        display_name: "UPGRADE:Camouflage",
    },
];

/// C++ `UpgradeTemplate::calcTimeToBuild` residual: `m_buildTime * LOGICFRAMES_PER_SECOND`.
#[inline]
pub fn upgrade_calc_time_to_build_frames_residual(build_time_sec: f32) -> u32 {
    (build_time_sec * LOGICFRAMES_PER_SECOND_RESIDUAL as f32) as u32
}

/// C++ `UpgradeTemplate::calcCostToBuild` residual: returns m_cost.
#[inline]
pub fn upgrade_calc_cost_to_build_residual(build_cost: i32) -> i32 {
    build_cost
}

/// Lookup Upgrade residual store row by name.
pub fn upgrade_store_row_wave109(name: &str) -> Option<&'static UpgradeStoreResidualRowWave109> {
    UPGRADE_STORE_TABLE_WAVE109.iter().find(|r| r.name == name)
}

/// Host UpgradeCenter residual: linked-list-style registration + mask bit index.
#[derive(Debug, Clone, Default)]
pub struct HostUpgradeCenterResidualWave109 {
    pub upgrades: Vec<(&'static str, u32)>, // (name, mask_bit_index)
    pub next_mask_bit: u32,
}

impl HostUpgradeCenterResidualWave109 {
    pub fn new() -> Self {
        Self {
            upgrades: Vec::new(),
            next_mask_bit: 0,
        }
    }

    /// C++ UpgradeCenter assigns unique UpgradeMask bit per template.
    pub fn register_upgrade(&mut self, name: &'static str) -> Option<u32> {
        if self.next_mask_bit as usize >= UPGRADE_MAX_COUNT_RESIDUAL {
            return None;
        }
        let bit = self.next_mask_bit;
        self.next_mask_bit += 1;
        self.upgrades.push((name, bit));
        Some(bit)
    }

    pub fn find_mask_bit(&self, name: &str) -> Option<u32> {
        self.upgrades
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, b)| *b)
    }

    pub fn count(&self) -> usize {
        self.upgrades.len()
    }
}

/// Wave 109 honesty: Upgrade residual store residual deepen pack.
///
/// Freezes UpgradeType/Status, UPGRADE_MAX_COUNT **128**, FieldParse names,
/// ctor defaults, sample BuildCost/BuildTime/Type residual, calcTimeToBuild
/// frames residual, and host mask-bit registration residual.
/// Fail-closed: not full UpgradeCenter multipleyer replication residual.

// ---------------------------------------------------------------------------
// 4. Player residual deepen
// ---------------------------------------------------------------------------

/// C++ `PlayerType` residual (GameCommon.h).
pub const PLAYER_TYPE_HUMAN: u32 = 0;
pub const PLAYER_TYPE_COMPUTER: u32 = 1;
pub const PLAYER_TYPE_COUNT: usize = 2;
pub const PLAYER_TYPE_NAME_TABLE_WAVE109: &[&str] = &["PLAYER_HUMAN", "PLAYER_COMPUTER"];

/// C++ `PLAYER_INDEX_INVALID` residual.
pub const PLAYER_INDEX_INVALID_RESIDUAL: i32 = -1;
/// C++ `MAX_PLAYER_COUNT` residual (cross-link Wave 95).
pub const PLAYER_MAX_COUNT_WAVE109: usize = 16;
/// C++ neutral player list slot residual.
pub const PLAYER_NEUTRAL_INDEX_WAVE109: usize = 0;
/// C++ `NUM_HOTKEY_SQUADS` residual.
pub const NUM_HOTKEY_SQUADS_RESIDUAL: usize = 10;
/// C++ `NO_HOTKEY_SQUAD` residual.
pub const NO_HOTKEY_SQUAD_RESIDUAL: i32 = -1;

/// C++ `AllowPlayerRelationship` residual bits (PlayerList.h).
pub const ALLOW_SAME_PLAYER: u32 = 0x01;
pub const ALLOW_ALLIES: u32 = 0x02;
pub const ALLOW_ENEMIES: u32 = 0x04;
pub const ALLOW_NEUTRAL: u32 = 0x08;
pub const ALLOW_PLAYER_RELATIONSHIP_NAME_TABLE_WAVE109: &[&str] = &[
    "ALLOW_SAME_PLAYER",
    "ALLOW_ALLIES",
    "ALLOW_ENEMIES",
    "ALLOW_NEUTRAL",
];

/// C++ `SpecialPowerReadyTimerType::clear` residual.
pub const SPECIAL_POWER_READY_TIMER_DEFAULT_READY_FRAME: u32 = 0xffff_ffff;
pub const SPECIAL_POWER_READY_TIMER_DEFAULT_TEMPLATE_ID: u32 = INVALID_ID_RESIDUAL;

/// C++ player mask residual: `1 << playerIndex`.
#[inline]
pub fn player_mask_wave109(player_index: u32) -> u32 {
    1u32 << player_index
}

/// SpecialPowerReadyTimer residual clear defaults.
#[inline]
pub fn special_power_ready_timer_clear_residual() -> (u32, u32) {
    (
        SPECIAL_POWER_READY_TIMER_DEFAULT_READY_FRAME,
        SPECIAL_POWER_READY_TIMER_DEFAULT_TEMPLATE_ID,
    )
}

/// Wave 109 honesty: Player residual deepen pack.
///
/// Freezes PlayerType, PLAYER_INDEX_INVALID, hotkey squads, AllowPlayerRelationship
/// bits and SpecialPowerReadyTimer clear residual.
/// Fail-closed: not full science purchase / energy matrix residual.

// ---------------------------------------------------------------------------
// 5. Team residual deepen
// ---------------------------------------------------------------------------

/// C++ `TEAM_ID_INVALID` / `TEAM_PROTOTYPE_ID_INVALID` residual.
pub const TEAM_ID_INVALID_WAVE109: u32 = 0;
pub const TEAM_PROTOTYPE_ID_INVALID_WAVE109: u32 = 0;
/// C++ `TeamTemplateInfo::MAX_UNIT_TYPES` residual.
pub const TEAM_MAX_UNIT_TYPES_WAVE109: usize = 7;
/// C++ `MAX_GENERIC_SCRIPTS` residual.
pub const TEAM_MAX_GENERIC_SCRIPTS_WAVE109: usize = 16;

/// C++ TeamTemplateInfo TBehavior residual.
pub const TEAM_BEHAVIOR_NAME_TABLE_WAVE109: &[&str] =
    &["NORMAL", "IGNORE_DISTRACTIONS", "DEAL_AGGRESSIVELY"];
pub const TEAM_BEHAVIOR_NORMAL_WAVE109: u32 = 0;
pub const TEAM_BEHAVIOR_IGNORE_DISTRACTIONS_WAVE109: u32 = 1;
pub const TEAM_BEHAVIOR_DEAL_AGGRESSIVELY_WAVE109: u32 = 2;

/// C++ Team instance residual ctor flags (Team.h).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamInstanceResidualWave109 {
    pub id: u32,
    pub prototype_id: u32,
    pub name: String,
    pub active: bool,
    pub created: bool,
    pub member_count: u32,
}

/// C++ TeamFactory residual: unique ID counters start INVALID (0); create uses ++id.
#[derive(Debug, Clone, Default)]
pub struct HostTeamFactoryResidualWave109 {
    pub unique_team_prototype_id: u32,
    pub unique_team_id: u32,
    pub prototypes: Vec<(String, u32, bool)>, // (name, proto_id, is_singleton)
    pub teams: Vec<TeamInstanceResidualWave109>,
}

impl HostTeamFactoryResidualWave109 {
    pub fn new() -> Self {
        Self {
            unique_team_prototype_id: TEAM_PROTOTYPE_ID_INVALID_WAVE109,
            unique_team_id: TEAM_ID_INVALID_WAVE109,
            prototypes: Vec::new(),
            teams: Vec::new(),
        }
    }

    /// C++ `initTeam` residual: `++m_uniqueTeamPrototypeID`.
    pub fn init_team(&mut self, name: &str, is_singleton: bool) -> u32 {
        self.unique_team_prototype_id = self.unique_team_prototype_id.saturating_add(1);
        let id = self.unique_team_prototype_id;
        self.prototypes.push((name.to_string(), id, is_singleton));
        id
    }

    /// C++ `createTeam` residual: active true after create.
    pub fn create_team(&mut self, name: &str) -> Option<u32> {
        let proto = self
            .prototypes
            .iter()
            .find(|(n, _, _)| n == name)
            .cloned()?;
        self.unique_team_id = self.unique_team_id.saturating_add(1);
        let id = self.unique_team_id;
        self.teams.push(TeamInstanceResidualWave109 {
            id,
            prototype_id: proto.1,
            name: name.to_string(),
            active: true,
            created: true,
            member_count: 0,
        });
        Some(id)
    }

    /// C++ `createInactiveTeam` residual: active false while members added.
    pub fn create_inactive_team(&mut self, name: &str) -> Option<u32> {
        let proto = self
            .prototypes
            .iter()
            .find(|(n, _, _)| n == name)
            .cloned()?;
        self.unique_team_id = self.unique_team_id.saturating_add(1);
        let id = self.unique_team_id;
        self.teams.push(TeamInstanceResidualWave109 {
            id,
            prototype_id: proto.1,
            name: name.to_string(),
            active: false,
            created: false,
            member_count: 0,
        });
        Some(id)
    }

    /// Activate inactive team residual (members complete).
    pub fn activate_team(&mut self, team_id: u32) -> bool {
        if let Some(t) = self.teams.iter_mut().find(|t| t.id == team_id) {
            t.active = true;
            t.created = true;
            true
        } else {
            false
        }
    }

    pub fn find_team_by_id(&self, id: u32) -> Option<&TeamInstanceResidualWave109> {
        self.teams.iter().find(|t| t.id == id)
    }

    pub fn find_prototype_id(&self, name: &str) -> Option<u32> {
        self.prototypes
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, id, _)| *id)
    }

    /// Default team name residual: `"team" + playerName`.
    pub fn default_player_team_name(player_name: &str) -> String {
        format!("team{player_name}")
    }
}

/// Wave 109 honesty: Team residual deepen pack.
///
/// Freezes TeamFactory unique ID counters (start 0, ++ on create), inactive
/// vs active team residual, MAX_UNIT_TYPES / MAX_GENERIC_SCRIPTS, TBehavior
/// names, default team name residual.
/// Fail-closed: not full TeamFactory production / AI recruit residual.

// ---------------------------------------------------------------------------
// Combined Wave 109 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 109 SP/Science/Upgrade/Player/Team residual honesty pack.
///
/// Fail-closed: not full retail store UI / production / network residual.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_power_template_store_residual_honesty_wave109() {
        assert_eq!(DEFAULT_DETECTION_TIME_FRAMES_RESIDUAL, 300);
        assert_eq!(
            special_power_template_row_wave109("SuperweaponDaisyCutter")
                .map(|r| r.radius_cursor_radius),
            Some(170.0)
        );
    }

    #[test]
    fn science_store_residual_deepen_honesty_wave109() {
        assert!(!science_is_purchasable_residual(0));
        assert!(science_is_purchasable_residual(1));
        assert_eq!(
            science_store_row_wave109("SCIENCE_CashBounty1").map(|r| r.point_cost),
            Some(1)
        );
    }

    #[test]
    fn missing_science_ini_row_is_fail_closed() {
        // C++ Science.cpp:220-222 + 271-273: unknown science cost 0, prereqs false.
        assert_eq!(
            science_purchase_point_cost_residual("SCIENCE_DefinitelyNotInIni"),
            Some(0)
        );
        let unlocked = std::collections::HashSet::new();
        assert!(!science_prereqs_met_residual(
            &unlocked,
            "SCIENCE_DefinitelyNotInIni"
        ));
        assert!(!is_capable_of_purchasing_science_residual(
            &unlocked,
            99,
            "SCIENCE_DefinitelyNotInIni"
        ));
    }

    #[test]
    fn artillery_barrage3_requires_chain_not_rank1() {
        // C++ Player.cpp:2611-2635 — ArtilleryBarrage3 needs Barrage2 + Rank3.
        let mut unlocked = std::collections::HashSet::new();
        unlocked.insert("SCIENCE_CHINA".into());
        unlocked.insert("SCIENCE_Rank1".into());
        assert!(!is_capable_of_purchasing_science_residual(
            &unlocked,
            5,
            "SCIENCE_ArtilleryBarrage3"
        ));
        unlocked.insert("SCIENCE_ArtilleryBarrage1".into());
        unlocked.insert("SCIENCE_ArtilleryBarrage2".into());
        unlocked.insert("SCIENCE_Rank3".into());
        assert!(is_capable_of_purchasing_science_residual(
            &unlocked,
            1,
            "SCIENCE_ArtilleryBarrage3"
        ));
    }

    #[test]
    fn first_capable_is_ini_order_not_hardcoded_five() {
        let mut unlocked = std::collections::HashSet::new();
        unlocked.insert("SCIENCE_AMERICA".into());
        unlocked.insert("SCIENCE_Rank1".into());
        let first = first_capable_purchase_science_residual(&unlocked, 1)
            .expect("rank1 America should have a purchasable science");
        assert_eq!(first, "SCIENCE_PaladinTank");
        assert_ne!(first, "SCIENCE_A10ThunderboltMissileStrike1");
    }

    #[test]
    fn command_button_science_vec_requires_owned_science() {
        // C++ ControlBarCommand.cpp:1219-1261
        let mut unlocked = std::collections::HashSet::new();
        assert!(!host_command_button_science_vec_met(
            &unlocked,
            &["SCIENCE_StealthFighter"]
        ));
        unlocked.insert("SCIENCE_StealthFighter".into());
        assert!(host_command_button_science_vec_met(
            &unlocked,
            &["SCIENCE_StealthFighter"]
        ));
    }

    #[test]
    fn upgrade_store_residual_deepen_honesty_wave109() {
        assert_eq!(upgrade_calc_time_to_build_frames_residual(60.0), 1_800);
        assert_eq!(UPGRADE_MAX_COUNT_RESIDUAL, 128);
    }

    #[test]
    fn player_residual_deepen_honesty_wave109() {
        assert_eq!(NUM_HOTKEY_SQUADS_RESIDUAL, 10);
        assert_eq!(player_mask_wave109(3), 8);
    }

    #[test]
    fn team_residual_deepen_honesty_wave109() {
        assert_eq!(
            HostTeamFactoryResidualWave109::default_player_team_name("America"),
            "teamAmerica"
        );
    }
}
