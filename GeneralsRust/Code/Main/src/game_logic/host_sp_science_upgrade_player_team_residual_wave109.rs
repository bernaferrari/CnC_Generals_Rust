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
pub fn honesty_special_power_template_store_residual_wave109() -> bool {
    let fields_ok = SPECIAL_POWER_TEMPLATE_FIELD_PARSE_NAMES_WAVE109.len() == 13
        && residual_name_index(
            SPECIAL_POWER_TEMPLATE_FIELD_PARSE_NAMES_WAVE109,
            "ReloadTime",
        ) == Some(0)
        && residual_name_index(
            SPECIAL_POWER_TEMPLATE_FIELD_PARSE_NAMES_WAVE109,
            "RequiredScience",
        ) == Some(1)
        && residual_name_index(SPECIAL_POWER_TEMPLATE_FIELD_PARSE_NAMES_WAVE109, "Enum")
            == Some(5)
        && residual_name_index(
            SPECIAL_POWER_TEMPLATE_FIELD_PARSE_NAMES_WAVE109,
            "SharedSyncedTimer",
        )
        .is_some()
        && residual_name_index(
            SPECIAL_POWER_TEMPLATE_FIELD_PARSE_NAMES_WAVE109,
            "ShortcutPower",
        )
        .is_some()
        && residual_name_index(
            SPECIAL_POWER_TEMPLATE_FIELD_PARSE_NAMES_WAVE109,
            "AcademyClassify",
        ) == Some(12);

    let ctor = SPECIAL_POWER_TEMPLATE_CTOR_DEFAULTS_WAVE109;
    let ctor_ok = ctor.id == 0
        && ctor.type_enum == SPECIAL_INVALID_RESIDUAL
        && ctor.reload_time_frames == 0
        && ctor.required_science == SCIENCE_INVALID_RESIDUAL
        && !ctor.public_timer
        && ctor.detection_time_frames == 300
        && !ctor.shared_n_sync
        && ctor.view_object_duration_frames == 0
        && (ctor.view_object_range - 0.0).abs() < 1e-6
        && (ctor.radius_cursor_radius - 0.0).abs() < 1e-6
        && !ctor.shortcut_power
        && DEFAULT_DETECTION_TIME_FRAMES_RESIDUAL == 300;

    // Template residual table deepen.
    let table_ok = SPECIAL_POWER_TEMPLATE_STORE_TABLE_WAVE109.len() >= 10;
    let mut names: Vec<&str> = SPECIAL_POWER_TEMPLATE_STORE_TABLE_WAVE109
        .iter()
        .map(|r| r.template_name)
        .collect();
    names.sort_unstable();
    let unique_ok = !names.windows(2).any(|w| w[0] == w[1]);

    let daisy = special_power_template_row_wave109("SuperweaponDaisyCutter");
    let scud = special_power_template_row_wave109("SuperweaponScudStorm");
    let spy = special_power_template_row_wave109("SpecialPowerSpySatellite");
    let radar = special_power_template_row_wave109("SpecialPowerRadarVanScan");
    let anchors_ok = matches!(
        daisy,
        Some(SpecialPowerTemplateResidualRowWave109 {
            reload_ms: 360_000,
            required_science: "SCIENCE_DaisyCutter",
            public_timer: false,
            shared_synced_timer: true,
            radius_cursor_radius: 170.0,
            shortcut_power: true,
            ..
        })
    ) && matches!(
        scud,
        Some(SpecialPowerTemplateResidualRowWave109 {
            reload_ms: 300_000,
            public_timer: true,
            radius_cursor_radius: 200.0,
            ..
        })
    ) && matches!(
        spy,
        Some(SpecialPowerTemplateResidualRowWave109 {
            reload_ms: 60_000,
            shared_synced_timer: true,
            radius_cursor_radius: 300.0,
            ..
        })
    ) && matches!(
        radar,
        Some(SpecialPowerTemplateResidualRowWave109 {
            reload_ms: 30_000,
            radius_cursor_radius: 150.0,
            ..
        })
    );

    // ms → frames residual for key rows.
    let frames_ok = duration_ms_to_logic_frames_wave109(360_000) == 10_800
        && duration_ms_to_logic_frames_wave109(240_000) == 7_200
        && duration_ms_to_logic_frames_wave109(300_000) == 9_000
        && duration_ms_to_logic_frames_wave109(60_000) == 1_800
        && duration_ms_to_logic_frames_wave109(30_000) == 900
        && duration_ms_to_logic_frames_wave109(30_000) // ViewObjectDuration
            == 900;

    // Store residual: next ID 0 → register assigns 1, 2, …
    let mut store = HostSpecialPowerStoreResidualWave109::new();
    let store_empty_ok = store.next_special_power_id == 0 && store.get_num_special_powers() == 0;
    let id1 = store.register_template("SuperweaponDaisyCutter");
    let id2 = store.register_template("SuperweaponA10ThunderboltMissileStrike");
    let store_ok = store_empty_ok
        && id1 == 1
        && id2 == 2
        && store.next_special_power_id == 2
        && store.get_num_special_powers() == 2
        && store.find_by_name("SuperweaponDaisyCutter") == Some(1)
        && store.find_by_id(2) == Some("SuperweaponA10ThunderboltMissileStrike")
        && store.find_by_name("missing").is_none()
        && store.find_by_id(0).is_none();

    fields_ok && ctor_ok && table_ok && unique_ok && anchors_ok && frames_ok && store_ok
}

// ---------------------------------------------------------------------------
// 2. Science residual store residual deepen
// ---------------------------------------------------------------------------

/// C++ ScienceInfo ctor residual: cost **0** means "cannot be purchased".
pub const SCIENCE_PURCHASE_COST_UNPURCHASABLE_RESIDUAL: i32 = 0;
/// C++ ScienceInfo ctor residual: m_grantable default **true**.
pub const SCIENCE_GRANTABLE_DEFAULT_RESIDUAL: bool = true;

/// C++ ScienceAvailabilityType residual names (Player.h DEFINE_SCIENCE_AVAILABILITY_NAMES).
pub const SCIENCE_AVAILABILITY_NAME_TABLE_WAVE109: &[&str] =
    &["Available", "Disabled", "Hidden"];
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
];

/// C++ residual: cost **0** → cannot be purchased (not free).
#[inline]
pub fn science_is_purchasable_residual(point_cost: i32) -> bool {
    point_cost > SCIENCE_PURCHASE_COST_UNPURCHASABLE_RESIDUAL
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
pub fn honesty_science_store_residual_deepen_pack_wave109() -> bool {
    let invalid_ok = SCIENCE_INVALID_RESIDUAL == -1
        && SCIENCE_PURCHASE_COST_UNPURCHASABLE_RESIDUAL == 0
        && SCIENCE_GRANTABLE_DEFAULT_RESIDUAL;

    let avail_ok = SCIENCE_AVAILABILITY_NAME_TABLE_WAVE109.len() == SCIENCE_AVAILABILITY_COUNT
        && residual_name_index(SCIENCE_AVAILABILITY_NAME_TABLE_WAVE109, "Available")
            == Some(0)
        && residual_name_index(SCIENCE_AVAILABILITY_NAME_TABLE_WAVE109, "Disabled")
            == Some(1)
        && residual_name_index(SCIENCE_AVAILABILITY_NAME_TABLE_WAVE109, "Hidden") == Some(2)
        && SCIENCE_AVAILABILITY_INVALID == -1
        && SCIENCE_AVAILABLE == 0
        && SCIENCE_DISABLED == 1
        && SCIENCE_HIDDEN == 2;

    let table_ok = SCIENCE_STORE_TABLE_WAVE109.len() >= 12;
    let mut names: Vec<&str> = SCIENCE_STORE_TABLE_WAVE109.iter().map(|r| r.name).collect();
    names.sort_unstable();
    let unique_ok = !names.windows(2).any(|w| w[0] == w[1]);

    let america = science_store_row_wave109("SCIENCE_AMERICA");
    let paladin = science_store_row_wave109("SCIENCE_PaladinTank");
    let moab = science_store_row_wave109("SCIENCE_MOAB");
    let cash3 = science_store_row_wave109("SCIENCE_CashBounty3");
    let anchors_ok = matches!(
        america,
        Some(ScienceStoreResidualRowWave109 {
            point_cost: 0,
            grantable: true,
            ..
        })
    ) && matches!(
        paladin,
        Some(ScienceStoreResidualRowWave109 {
            point_cost: 1,
            prereq_a: "SCIENCE_AMERICA",
            prereq_b: "SCIENCE_Rank1",
            ..
        })
    ) && matches!(
        moab,
        Some(ScienceStoreResidualRowWave109 {
            point_cost: 0,
            prereq_a: "SCIENCE_AMERICA",
            ..
        })
    ) && matches!(
        cash3,
        Some(ScienceStoreResidualRowWave109 {
            point_cost: 1,
            prereq_a: "SCIENCE_CashBounty2",
            prereq_b: "SCIENCE_Rank3",
            ..
        })
    );

    // Purchase residual: cost 0 unpurchasable; cost 1 purchasable.
    let purchase_ok = !science_is_purchasable_residual(0)
        && science_is_purchasable_residual(1)
        && science_is_purchasable_residual(3)
        && !science_is_purchasable_residual(SCIENCE_PURCHASE_COST_UNPURCHASABLE_RESIDUAL);

    // Host store residual.
    let store = HostScienceStoreResidualWave109::from_table();
    let store_ok = store.count() >= 12
        && store.get_purchase_cost("SCIENCE_PaladinTank") == Some(1)
        && store.get_purchase_cost("SCIENCE_AMERICA") == Some(0)
        && store.is_grantable("SCIENCE_ClusterMines") == Some(true)
        && store.find("missing").is_none()
        && store.get_purchase_cost("missing").is_none();

    // Cross-link: DaisyCutter SpecialPower RequiredScience present in science store.
    let cross_ok = science_store_row_wave109("SCIENCE_DaisyCutter").is_some()
        && science_store_row_wave109("SCIENCE_ClusterMines").is_some()
        && special_power_template_row_wave109("SuperweaponDaisyCutter")
            .map(|r| r.required_science == "SCIENCE_DaisyCutter")
            .unwrap_or(false);

    invalid_ok
        && avail_ok
        && table_ok
        && unique_ok
        && anchors_ok
        && purchase_ok
        && store_ok
        && cross_ok
}

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
pub const UPGRADE_STATUS_NAME_TABLE_WAVE109: &[&str] =
    &["UPGRADE_STATUS_INVALID", "UPGRADE_STATUS_IN_PRODUCTION", "UPGRADE_STATUS_COMPLETE"];

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
pub fn honesty_upgrade_store_residual_deepen_pack_wave109() -> bool {
    let type_ok = UPGRADE_TYPE_NAME_TABLE_WAVE109.len() == NUM_UPGRADE_TYPES
        && residual_name_index(UPGRADE_TYPE_NAME_TABLE_WAVE109, "PLAYER") == Some(0)
        && residual_name_index(UPGRADE_TYPE_NAME_TABLE_WAVE109, "OBJECT") == Some(1)
        && UPGRADE_TYPE_PLAYER == 0
        && UPGRADE_TYPE_OBJECT == 1;

    let status_ok = UPGRADE_STATUS_NAME_TABLE_WAVE109.len() == 3
        && UPGRADE_STATUS_INVALID == 0
        && UPGRADE_STATUS_IN_PRODUCTION == 1
        && UPGRADE_STATUS_COMPLETE == 2
        && residual_name_index(
            UPGRADE_STATUS_NAME_TABLE_WAVE109,
            "UPGRADE_STATUS_COMPLETE",
        ) == Some(2);

    let caps_ok = UPGRADE_MAX_COUNT_RESIDUAL == 128
        && NAMEKEY_INVALID_RESIDUAL == 0
        && UPGRADE_TEMPLATE_CTOR_TYPE_DEFAULT == UPGRADE_TYPE_PLAYER
        && UPGRADE_TEMPLATE_CTOR_COST_DEFAULT == 0
        && (UPGRADE_TEMPLATE_CTOR_BUILD_TIME_DEFAULT - 0.0).abs() < 1e-6;

    let fields_ok = UPGRADE_TEMPLATE_FIELD_PARSE_NAMES_WAVE109.len() >= 6
        && residual_name_index(UPGRADE_TEMPLATE_FIELD_PARSE_NAMES_WAVE109, "Type")
            .is_some()
        && residual_name_index(UPGRADE_TEMPLATE_FIELD_PARSE_NAMES_WAVE109, "BuildTime")
            .is_some()
        && residual_name_index(UPGRADE_TEMPLATE_FIELD_PARSE_NAMES_WAVE109, "BuildCost")
            .is_some();

    let table_ok = UPGRADE_STORE_TABLE_WAVE109.len() >= 10;
    let mut names: Vec<&str> = UPGRADE_STORE_TABLE_WAVE109.iter().map(|r| r.name).collect();
    names.sort_unstable();
    let unique_ok = !names.windows(2).any(|w| w[0] == w[1]);

    let nationalism = upgrade_store_row_wave109("Upgrade_Nationalism");
    let radar = upgrade_store_row_wave109("Upgrade_AmericaRadar");
    let composite = upgrade_store_row_wave109("Upgrade_AmericaCompositeArmor");
    let anchors_ok = matches!(
        nationalism,
        Some(UpgradeStoreResidualRowWave109 {
            build_cost: 2000,
            build_time_sec: 60.0,
            upgrade_type: UPGRADE_TYPE_PLAYER,
            ..
        })
    ) && matches!(
        radar,
        Some(UpgradeStoreResidualRowWave109 {
            build_cost: 500,
            build_time_sec: 10.0,
            upgrade_type: UPGRADE_TYPE_OBJECT,
            ..
        })
    ) && matches!(
        composite,
        Some(UpgradeStoreResidualRowWave109 {
            build_cost: 2000,
            build_time_sec: 60.0,
            ..
        })
    );

    // calcTimeToBuild residual: 60s → 1800f, 30s → 900f, 10s → 300f, 40s → 1200f.
    let time_ok = upgrade_calc_time_to_build_frames_residual(60.0) == 1_800
        && upgrade_calc_time_to_build_frames_residual(30.0) == 900
        && upgrade_calc_time_to_build_frames_residual(10.0) == 300
        && upgrade_calc_time_to_build_frames_residual(40.0) == 1_200
        && upgrade_calc_cost_to_build_residual(2000) == 2000
        && upgrade_calc_cost_to_build_residual(500) == 500;

    // Host UpgradeCenter residual mask bits 0..n-1, cap 128.
    let mut center = HostUpgradeCenterResidualWave109::new();
    let b0 = center.register_upgrade("Upgrade_Nationalism");
    let b1 = center.register_upgrade("Upgrade_AmericaRadar");
    let center_ok = b0 == Some(0)
        && b1 == Some(1)
        && center.count() == 2
        && center.find_mask_bit("Upgrade_Nationalism") == Some(0)
        && center.find_mask_bit("missing").is_none()
        && center.next_mask_bit == 2;

    // Mask bit capacity residual: bit 127 ok, bit 128 rejected.
    let mut full = HostUpgradeCenterResidualWave109::new();
    full.next_mask_bit = 127;
    let last = full.register_upgrade("Upgrade_Last");
    let overflow = full.register_upgrade("Upgrade_Overflow");
    let cap_ok = last == Some(127) && overflow.is_none() && full.count() == 1;

    type_ok
        && status_ok
        && caps_ok
        && fields_ok
        && table_ok
        && unique_ok
        && anchors_ok
        && time_ok
        && center_ok
        && cap_ok
}

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

/// C++ Player ctor residual deepen (beyond Wave 95 skill/rank zeros).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerCtorResidualWave109 {
    pub player_index: i32,
    pub player_type: u32,
    pub skill_points_modifier: f32,
    pub rank_level: i32,
    pub skill_points: i32,
    pub science_purchase_points: i32,
    pub is_local: bool,
}

/// Ctor residual for a non-neutral human local player at index 1.
pub fn player_human_local_ctor_residual(player_index: i32) -> PlayerCtorResidualWave109 {
    PlayerCtorResidualWave109 {
        player_index,
        player_type: PLAYER_TYPE_HUMAN,
        skill_points_modifier: 1.0,
        rank_level: 0,
        skill_points: 0,
        science_purchase_points: 0,
        is_local: true,
    }
}

/// Ctor residual for computer AI player.
pub fn player_computer_ctor_residual(player_index: i32) -> PlayerCtorResidualWave109 {
    PlayerCtorResidualWave109 {
        player_index,
        player_type: PLAYER_TYPE_COMPUTER,
        skill_points_modifier: 1.0,
        rank_level: 0,
        skill_points: 0,
        science_purchase_points: 0,
        is_local: false,
    }
}

/// C++ player mask residual: `1 << playerIndex`.
#[inline]
pub fn player_mask_wave109(player_index: u32) -> u32 {
    1u32 << player_index
}

/// Host PlayerList residual: fixed MAX_PLAYER_COUNT slots, neutral at 0.
#[derive(Debug, Clone)]
pub struct HostPlayerListResidualWave109 {
    pub players: [Option<PlayerCtorResidualWave109>; PLAYER_MAX_COUNT_WAVE109],
    pub player_count: usize,
    pub local_index: Option<usize>,
}

impl HostPlayerListResidualWave109 {
    pub fn new_with_neutral() -> Self {
        let mut players = [None; PLAYER_MAX_COUNT_WAVE109];
        // Neutral at index 0 residual.
        players[0] = Some(PlayerCtorResidualWave109 {
            player_index: 0,
            player_type: PLAYER_TYPE_COMPUTER,
            skill_points_modifier: 1.0,
            rank_level: 0,
            skill_points: 0,
            science_purchase_points: 0,
            is_local: false,
        });
        Self {
            players,
            player_count: 1,
            local_index: None,
        }
    }

    pub fn get_neutral(&self) -> Option<&PlayerCtorResidualWave109> {
        self.players[PLAYER_NEUTRAL_INDEX_WAVE109].as_ref()
    }

    pub fn set_local(&mut self, index: usize) {
        if index < PLAYER_MAX_COUNT_WAVE109 && self.players[index].is_some() {
            // Clear previous local.
            if let Some(prev) = self.local_index {
                if let Some(p) = self.players[prev].as_mut() {
                    p.is_local = false;
                }
            }
            if let Some(p) = self.players[index].as_mut() {
                p.is_local = true;
            }
            self.local_index = Some(index);
        } else {
            // C++ setLocalPlayer(null) → neutral.
            self.local_index = Some(PLAYER_NEUTRAL_INDEX_WAVE109);
            if let Some(p) = self.players[PLAYER_NEUTRAL_INDEX_WAVE109].as_mut() {
                p.is_local = true;
            }
        }
    }

    pub fn add_player(&mut self, p: PlayerCtorResidualWave109) -> Option<usize> {
        if self.player_count >= PLAYER_MAX_COUNT_WAVE109 {
            return None;
        }
        let idx = self.player_count;
        self.players[idx] = Some(p);
        self.player_count += 1;
        Some(idx)
    }

    pub fn get_local(&self) -> Option<&PlayerCtorResidualWave109> {
        self.local_index.and_then(|i| self.players[i].as_ref())
    }

    pub fn get_player_from_mask(&self, mask: u32) -> Option<&PlayerCtorResidualWave109> {
        for i in 0..self.player_count {
            if player_mask_wave109(i as u32) == mask {
                return self.players[i].as_ref();
            }
        }
        None
    }
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
/// bits, SpecialPowerReadyTimer clear residual, PlayerList neutral/local residual.
/// Fail-closed: not full science purchase / energy matrix residual.
pub fn honesty_player_residual_deepen_pack_wave109() -> bool {
    let type_ok = PLAYER_TYPE_NAME_TABLE_WAVE109.len() == PLAYER_TYPE_COUNT
        && residual_name_index(PLAYER_TYPE_NAME_TABLE_WAVE109, "PLAYER_HUMAN") == Some(0)
        && residual_name_index(PLAYER_TYPE_NAME_TABLE_WAVE109, "PLAYER_COMPUTER")
            == Some(1)
        && PLAYER_TYPE_HUMAN == 0
        && PLAYER_TYPE_COMPUTER == 1
        && PLAYER_INDEX_INVALID_RESIDUAL == -1;

    let hotkey_ok = NUM_HOTKEY_SQUADS_RESIDUAL == 10 && NO_HOTKEY_SQUAD_RESIDUAL == -1;

    let allow_ok = ALLOW_SAME_PLAYER == 0x01
        && ALLOW_ALLIES == 0x02
        && ALLOW_ENEMIES == 0x04
        && ALLOW_NEUTRAL == 0x08
        && ALLOW_PLAYER_RELATIONSHIP_NAME_TABLE_WAVE109.len() == 4
        && residual_name_index(
            ALLOW_PLAYER_RELATIONSHIP_NAME_TABLE_WAVE109,
            "ALLOW_ALLIES",
        ) == Some(1)
        // Composite residual: allies | enemies | neutral (common filter pack).
        && (ALLOW_ALLIES | ALLOW_ENEMIES | ALLOW_NEUTRAL) == 0x0e;

    let timer_ok = {
        let (ready, tid) = special_power_ready_timer_clear_residual();
        ready == 0xffff_ffff && tid == INVALID_ID_RESIDUAL
    };

    let human = player_human_local_ctor_residual(1);
    let ai = player_computer_ctor_residual(2);
    let ctor_ok = human.player_type == PLAYER_TYPE_HUMAN
        && human.is_local
        && (human.skill_points_modifier - 1.0).abs() < 1e-6
        && human.rank_level == 0
        && human.skill_points == 0
        && human.science_purchase_points == 0
        && ai.player_type == PLAYER_TYPE_COMPUTER
        && !ai.is_local
        && player_mask_wave109(0) == 1
        && player_mask_wave109(1) == 2
        && player_mask_wave109(15) == 0x8000;

    // PlayerList residual: neutral @0, add human @1, set local.
    let mut list = HostPlayerListResidualWave109::new_with_neutral();
    let list_ok = list.player_count == 1
        && list.get_neutral().map(|p| p.player_index) == Some(0)
        && list.local_index.is_none();
    let idx = list.add_player(player_human_local_ctor_residual(1));
    list.set_local(1);
    let list2_ok = idx == Some(1)
        && list.player_count == 2
        && list.get_local().map(|p| p.player_index) == Some(1)
        && list.get_local().map(|p| p.is_local) == Some(true)
        && list.get_player_from_mask(0x02).map(|p| p.player_index) == Some(1)
        && list.get_player_from_mask(0x01).map(|p| p.player_index) == Some(0);

    // setLocal invalid → fall back to neutral residual.
    list.set_local(99);
    let fallback_ok = list.local_index == Some(0)
        && list.get_local().map(|p| p.player_index) == Some(0);

    // Science availability residual cross-link.
    let sci_avail_ok = SCIENCE_AVAILABILITY_NAME_TABLE_WAVE109.len() == 3
        && residual_name_index(SCIENCE_AVAILABILITY_NAME_TABLE_WAVE109, "Hidden")
            == Some(2);

    type_ok
        && hotkey_ok
        && allow_ok
        && timer_ok
        && ctor_ok
        && list_ok
        && list2_ok
        && fallback_ok
        && sci_avail_ok
        && PLAYER_MAX_COUNT_WAVE109 == 16
}

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
        self.prototypes
            .push((name.to_string(), id, is_singleton));
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
pub fn honesty_team_residual_deepen_pack_wave109() -> bool {
    let caps_ok = TEAM_ID_INVALID_WAVE109 == 0
        && TEAM_PROTOTYPE_ID_INVALID_WAVE109 == 0
        && TEAM_MAX_UNIT_TYPES_WAVE109 == 7
        && TEAM_MAX_GENERIC_SCRIPTS_WAVE109 == 16
        && TEAM_BEHAVIOR_NAME_TABLE_WAVE109.len() == 3
        && residual_name_index(TEAM_BEHAVIOR_NAME_TABLE_WAVE109, "NORMAL") == Some(0)
        && residual_name_index(
            TEAM_BEHAVIOR_NAME_TABLE_WAVE109,
            "IGNORE_DISTRACTIONS",
        ) == Some(1)
        && residual_name_index(TEAM_BEHAVIOR_NAME_TABLE_WAVE109, "DEAL_AGGRESSIVELY")
            == Some(2)
        && TEAM_BEHAVIOR_NORMAL_WAVE109 == 0
        && TEAM_BEHAVIOR_IGNORE_DISTRACTIONS_WAVE109 == 1
        && TEAM_BEHAVIOR_DEAL_AGGRESSIVELY_WAVE109 == 2;

    let mut factory = HostTeamFactoryResidualWave109::new();
    let ctor_ok = factory.unique_team_id == 0
        && factory.unique_team_prototype_id == 0
        && factory.prototypes.is_empty()
        && factory.teams.is_empty();

    // Default team names residual (cross-link Wave 95).
    let name_ok = HostTeamFactoryResidualWave109::default_player_team_name("America")
        == "teamAmerica"
        && HostTeamFactoryResidualWave109::default_player_team_name("PlyrCivilian")
            == "teamPlyrCivilian"
        && HostTeamFactoryResidualWave109::default_player_team_name("") == "team"
        && HostTeamFactoryResidualWave109::default_player_team_name("ThePlayer")
            == "teamThePlayer";

    // initTeam residual: ++proto id.
    let proto1 = factory.init_team("teamAmerica", true);
    let proto2 = factory.init_team("teamChina", false);
    let proto_ok = proto1 == 1
        && proto2 == 2
        && factory.unique_team_prototype_id == 2
        && factory.find_prototype_id("teamAmerica") == Some(1)
        && factory.find_prototype_id("missing").is_none();

    // createTeam residual: active true.
    let team_a = factory.create_team("teamAmerica");
    let create_ok = team_a == Some(1)
        && factory.unique_team_id == 1
        && factory
            .find_team_by_id(1)
            .map(|t| t.active && t.created && t.prototype_id == 1)
            .unwrap_or(false);

    // createInactiveTeam residual: active false until activate.
    let team_b = factory.create_inactive_team("teamChina");
    let inactive_ok = team_b == Some(2)
        && factory
            .find_team_by_id(2)
            .map(|t| !t.active && !t.created && t.prototype_id == 2)
            .unwrap_or(false);
    let activated = factory.activate_team(2);
    let activate_ok = activated
        && factory
            .find_team_by_id(2)
            .map(|t| t.active && t.created)
            .unwrap_or(false);

    // Singleton residual: teamAmerica is_singleton true.
    let singleton_ok = factory
        .prototypes
        .iter()
        .find(|(n, _, _)| n == "teamAmerica")
        .map(|(_, _, s)| *s)
        .unwrap_or(false)
        && !factory
            .prototypes
            .iter()
            .find(|(n, _, _)| n == "teamChina")
            .map(|(_, _, s)| *s)
            .unwrap_or(true);

    // create missing prototype fails residual.
    let missing_ok = factory.create_team("teamMissing").is_none();

    caps_ok
        && ctor_ok
        && name_ok
        && proto_ok
        && create_ok
        && inactive_ok
        && activate_ok
        && singleton_ok
        && missing_ok
}

// ---------------------------------------------------------------------------
// Combined Wave 109 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 109 SP/Science/Upgrade/Player/Team residual honesty pack.
///
/// Fail-closed: not full retail store UI / production / network residual.
pub fn honesty_sp_science_upgrade_player_team_residual_pack_wave109() -> bool {
    honesty_special_power_template_store_residual_wave109()
        && honesty_science_store_residual_deepen_pack_wave109()
        && honesty_upgrade_store_residual_deepen_pack_wave109()
        && honesty_player_residual_deepen_pack_wave109()
        && honesty_team_residual_deepen_pack_wave109()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_power_template_store_residual_honesty_wave109() {
        assert!(honesty_special_power_template_store_residual_wave109());
        assert_eq!(DEFAULT_DETECTION_TIME_FRAMES_RESIDUAL, 300);
        assert_eq!(
            special_power_template_row_wave109("SuperweaponDaisyCutter")
                .map(|r| r.radius_cursor_radius),
            Some(170.0)
        );
    }

    #[test]
    fn science_store_residual_deepen_honesty_wave109() {
        assert!(honesty_science_store_residual_deepen_pack_wave109());
        assert!(!science_is_purchasable_residual(0));
        assert!(science_is_purchasable_residual(1));
        assert_eq!(
            science_store_row_wave109("SCIENCE_CashBounty1").map(|r| r.point_cost),
            Some(1)
        );
    }

    #[test]
    fn upgrade_store_residual_deepen_honesty_wave109() {
        assert!(honesty_upgrade_store_residual_deepen_pack_wave109());
        assert_eq!(upgrade_calc_time_to_build_frames_residual(60.0), 1_800);
        assert_eq!(UPGRADE_MAX_COUNT_RESIDUAL, 128);
    }

    #[test]
    fn player_residual_deepen_honesty_wave109() {
        assert!(honesty_player_residual_deepen_pack_wave109());
        assert_eq!(NUM_HOTKEY_SQUADS_RESIDUAL, 10);
        assert_eq!(player_mask_wave109(3), 8);
    }

    #[test]
    fn team_residual_deepen_honesty_wave109() {
        assert!(honesty_team_residual_deepen_pack_wave109());
        assert_eq!(
            HostTeamFactoryResidualWave109::default_player_team_name("America"),
            "teamAmerica"
        );
    }

    #[test]
    fn sp_science_upgrade_player_team_residual_pack_honesty_wave109() {
        assert!(honesty_sp_science_upgrade_player_team_residual_pack_wave109());
    }
}
