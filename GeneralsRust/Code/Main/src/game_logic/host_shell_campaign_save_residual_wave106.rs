//! Wave 106 residual peels: GameState / campaign mission tables / MainMenu /
//! GameWindow / WindowLayout residual deepen (host-testable shell residual).
//!
//! Orthogonal to Wave 100 (SaveFileType / SnapshotType / SaveCode Xfer peels),
//! existing Campaign.ini USA + CHALLENGE_0 seed, ControlBar.wnd Wave 76 layout,
//! and GameClient WindowStatus bitflags.
//! Host residual only — shell `playable_claim` stays false; network deferred.
//!
//! Sources (retail ZH C++ / INI / .wnd):
//! - GameState.h/.cpp SaveLoadLayoutType / CHUNK_* snapshot block names /
//!   GAME_STATE_BLOCK_STRING / CAMPAIGN_BLOCK_STRING / DeepCRC logic-only subset
//! - Campaign.ini USA / GLA / China / TRAINING / CHALLENGE_0..8 mission map tables
//! - MainMenu.cpp + Menus/MainMenu.wnd button / faction window residual names
//! - GameWindow.h WIN_STATUS_* bits / GameWindowMessage GWM_* / GWM_USER
//! - WindowLayout.h init/update/shutdown + hide residual + shell layout filenames
//!
//! Fail-closed:
//! - Not full GameState xferSaveData file I/O / deep CRC network residual
//! - Not full CampaignManager live INI parse residual
//! - Not full MainMenu.wnd W3D TransitionHandler retail UI residual
//! - Not full GameWindow GPU draw / WindowManager exclusive residual
//! - Not full WindowLayout load from .wnd script residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

// ---------------------------------------------------------------------------
// 1. GameState residual deepen (beyond Wave 100 SaveFileType/SnapshotType)
// ---------------------------------------------------------------------------

/// C++ `SaveLoadLayoutType` residual (GameState.h).
pub const SLLT_INVALID: u32 = 0;
pub const SLLT_SAVE_AND_LOAD: u32 = 1;
pub const SLLT_LOAD_ONLY: u32 = 2;
pub const SLLT_SAVE_ONLY: u32 = 3;
pub const SLLT_NUM_TYPES: usize = 4;

/// Ordered SaveLoadLayoutType residual names.
pub const SAVE_LOAD_LAYOUT_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "SLLT_INVALID",
    "SLLT_SAVE_AND_LOAD",
    "SLLT_LOAD_ONLY",
    "SLLT_SAVE_ONLY",
];

/// C++ `GAME_STATE_BLOCK_STRING` residual.
pub const GAME_STATE_BLOCK_STRING: &str = "CHUNK_GameState";
/// C++ `CAMPAIGN_BLOCK_STRING` residual.
pub const CAMPAIGN_BLOCK_STRING: &str = "CHUNK_Campaign";

/// C++ `GameState::init` SNAPSHOT_SAVELOAD residual block names (GameState.cpp).
/// Order matches retail addSnapshotBlock registration for save/load.
pub const GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS: &[&str] = &[
    "CHUNK_GameState",
    "CHUNK_Campaign",
    "CHUNK_GameStateMap",
    "CHUNK_TerrainLogic",
    "CHUNK_TeamFactory",
    "CHUNK_Players",
    "CHUNK_GameLogic",
    "CHUNK_Radar",
    "CHUNK_ScriptEngine",
    "CHUNK_SidesList",
    "CHUNK_TacticalView",
    "CHUNK_GameClient",
    "CHUNK_InGameUI",
    "CHUNK_Partition",
    "CHUNK_ParticleSystem",
    "CHUNK_TerrainVisual",
    "CHUNK_GhostObject",
];

/// C++ SNAPSHOT_DEEPCRC_LOGICONLY residual block names.
pub const GAME_STATE_SNAPSHOT_DEEPCRC_LOGICONLY_BLOCKS: &[&str] = &[
    "CHUNK_TeamFactory",
    "CHUNK_Players",
    "CHUNK_GameLogic",
    "CHUNK_ScriptEngine",
    "CHUNK_SidesList",
    "CHUNK_Partition",
];

/// C++ save-directory leaf residual (GameState save root relative leaf).
pub const GAME_STATE_SAVE_DIRECTORY_LEAF: &str = "Save";

/// C++ mission-save residual filename token (between-mission save).
pub const GAME_STATE_MISSION_SAVE_TOKEN: &str = "MissionSave";

/// SaveDate residual field count (year..milliseconds, GameState.h SaveDate).
pub const SAVE_DATE_FIELD_COUNT_RESIDUAL: usize = 8;

/// Wave 106 honesty: GameState residual deepen pack.
///
/// Freezes SaveLoadLayoutType, CHUNK_* snapshot block tables, block string
/// anchors, DeepCRC logic-only subset, and save-directory residual.
/// Fail-closed: not full xferSaveData / AvailableGameInfo list residual.
pub fn honesty_game_state_residual_deepen_pack_wave106() -> bool {
    // SaveLoadLayoutType residual.
    let sllt_ok = SLLT_NUM_TYPES == 4
        && SAVE_LOAD_LAYOUT_TYPE_NAME_TABLE_RESIDUAL.len() == 4
        && residual_name_index(SAVE_LOAD_LAYOUT_TYPE_NAME_TABLE_RESIDUAL, "SLLT_INVALID")
            == Some(0)
        && residual_name_index(
            SAVE_LOAD_LAYOUT_TYPE_NAME_TABLE_RESIDUAL,
            "SLLT_SAVE_AND_LOAD",
        ) == Some(1)
        && residual_name_index(SAVE_LOAD_LAYOUT_TYPE_NAME_TABLE_RESIDUAL, "SLLT_LOAD_ONLY")
            == Some(2)
        && residual_name_index(SAVE_LOAD_LAYOUT_TYPE_NAME_TABLE_RESIDUAL, "SLLT_SAVE_ONLY")
            == Some(3)
        && SLLT_INVALID == 0
        && SLLT_SAVE_AND_LOAD == 1
        && SLLT_LOAD_ONLY == 2
        && SLLT_SAVE_ONLY == 3;

    // Block string anchors.
    let block_strings_ok = GAME_STATE_BLOCK_STRING == "CHUNK_GameState"
        && CAMPAIGN_BLOCK_STRING == "CHUNK_Campaign"
        && residual_name_index(GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS, GAME_STATE_BLOCK_STRING)
            == Some(0)
        && residual_name_index(GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS, CAMPAIGN_BLOCK_STRING)
            == Some(1);

    // SNAPSHOT_SAVELOAD block table residual (≥17 retail blocks).
    let saveload_ok = GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS.len() >= 17
        && residual_name_index(GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS, "CHUNK_GameLogic")
            .is_some()
        && residual_name_index(GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS, "CHUNK_Partition")
            .is_some()
        && residual_name_index(GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS, "CHUNK_GhostObject")
            == Some(GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS.len() - 1);

    // Unique block names.
    let mut names: Vec<&str> = GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS.to_vec();
    names.sort_unstable();
    let unique_ok = !names.windows(2).any(|w| w[0] == w[1]);

    // DeepCRC logic-only is a proper subset of saveload blocks.
    let deepcrc_ok = GAME_STATE_SNAPSHOT_DEEPCRC_LOGICONLY_BLOCKS.len() == 6
        && GAME_STATE_SNAPSHOT_DEEPCRC_LOGICONLY_BLOCKS
            .iter()
            .all(|b| residual_name_index(GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS, b).is_some())
        && residual_name_index(
            GAME_STATE_SNAPSHOT_DEEPCRC_LOGICONLY_BLOCKS,
            "CHUNK_GameLogic",
        )
        .is_some()
        // Client/UI chunks must NOT appear in logic-only DeepCRC residual.
        && residual_name_index(
            GAME_STATE_SNAPSHOT_DEEPCRC_LOGICONLY_BLOCKS,
            "CHUNK_GameClient",
        )
        .is_none()
        && residual_name_index(
            GAME_STATE_SNAPSHOT_DEEPCRC_LOGICONLY_BLOCKS,
            "CHUNK_InGameUI",
        )
        .is_none()
        && residual_name_index(
            GAME_STATE_SNAPSHOT_DEEPCRC_LOGICONLY_BLOCKS,
            "CHUNK_TacticalView",
        )
        .is_none();

    let path_ok = GAME_STATE_SAVE_DIRECTORY_LEAF == "Save"
        && !GAME_STATE_MISSION_SAVE_TOKEN.is_empty()
        && SAVE_DATE_FIELD_COUNT_RESIDUAL == 8;

    sllt_ok && block_strings_ok && saveload_ok && unique_ok && deepcrc_ok && path_ok
}

// ---------------------------------------------------------------------------
// 2. Campaign residual deepen (mission residual tables)
// ---------------------------------------------------------------------------

/// Campaign.ini residual mission map leaf (basename without .map).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignMissionMapResidual {
    pub campaign: &'static str,
    pub mission_index: u8,
    pub map_leaf: &'static str,
    pub intro_movie: &'static str,
}

/// Campaign.ini USA residual mission table (MD_USA01..05).
pub const CAMPAIGN_USA_MISSION_TABLE_WAVE106: &[CampaignMissionMapResidual] = &[
    CampaignMissionMapResidual {
        campaign: "USA",
        mission_index: 1,
        map_leaf: "MD_USA01",
        intro_movie: "MD_USA01",
    },
    CampaignMissionMapResidual {
        campaign: "USA",
        mission_index: 2,
        map_leaf: "MD_USA02",
        intro_movie: "MD_USA02",
    },
    CampaignMissionMapResidual {
        campaign: "USA",
        mission_index: 3,
        map_leaf: "MD_USA03",
        intro_movie: "MD_USA03",
    },
    CampaignMissionMapResidual {
        campaign: "USA",
        mission_index: 4,
        map_leaf: "MD_USA04",
        intro_movie: "MD_USA04",
    },
    CampaignMissionMapResidual {
        campaign: "USA",
        mission_index: 5,
        map_leaf: "MD_USA05",
        intro_movie: "MD_USA05",
    },
];

/// Campaign.ini GLA residual mission table (MD_GLA01..05).
pub const CAMPAIGN_GLA_MISSION_TABLE_WAVE106: &[CampaignMissionMapResidual] = &[
    CampaignMissionMapResidual {
        campaign: "GLA",
        mission_index: 1,
        map_leaf: "MD_GLA01",
        intro_movie: "MD_GLA01",
    },
    CampaignMissionMapResidual {
        campaign: "GLA",
        mission_index: 2,
        map_leaf: "MD_GLA02",
        intro_movie: "MD_GLA02",
    },
    CampaignMissionMapResidual {
        campaign: "GLA",
        mission_index: 3,
        map_leaf: "MD_GLA03",
        intro_movie: "MD_GLA03",
    },
    CampaignMissionMapResidual {
        campaign: "GLA",
        mission_index: 4,
        map_leaf: "MD_GLA04",
        intro_movie: "MD_GLA04",
    },
    CampaignMissionMapResidual {
        campaign: "GLA",
        mission_index: 5,
        map_leaf: "MD_GLA05",
        intro_movie: "MD_GLA05",
    },
];

/// Campaign.ini China residual mission table (MD_CHI01..05 maps / MD_China* movies).
pub const CAMPAIGN_CHINA_MISSION_TABLE_WAVE106: &[CampaignMissionMapResidual] = &[
    CampaignMissionMapResidual {
        campaign: "China",
        mission_index: 1,
        map_leaf: "MD_CHI01",
        intro_movie: "MD_China01",
    },
    CampaignMissionMapResidual {
        campaign: "China",
        mission_index: 2,
        map_leaf: "MD_CHI02",
        intro_movie: "MD_China02",
    },
    CampaignMissionMapResidual {
        campaign: "China",
        mission_index: 3,
        map_leaf: "MD_CHI03",
        intro_movie: "MD_China03",
    },
    CampaignMissionMapResidual {
        campaign: "China",
        mission_index: 4,
        map_leaf: "MD_CHI04",
        intro_movie: "MD_China04",
    },
    CampaignMissionMapResidual {
        campaign: "China",
        mission_index: 5,
        map_leaf: "MD_CHI05",
        intro_movie: "MD_China05",
    },
];

/// Campaign.ini TRAINING residual (single mission Training01).
pub const CAMPAIGN_TRAINING_MISSION_TABLE_WAVE106: &[CampaignMissionMapResidual] =
    &[CampaignMissionMapResidual {
        campaign: "TRAINING",
        mission_index: 1,
        map_leaf: "Training01",
        intro_movie: "TrainingCampaign",
    }];

/// Campaign.ini CHALLENGE_0 residual map chain (Generals Challenge air-force order).
pub const CAMPAIGN_CHALLENGE_0_MAP_CHAIN_WAVE106: &[&str] = &[
    "GC_ChemGeneral",
    "GC_NukeGeneral",
    "GC_SuperWeaponsGeneral",
    "GC_TankGeneral",
    "GC_Stealth",
    "GC_LaserGeneral",
    "GC_ChinaBoss",
];

/// Campaign.ini campaign label residual table (CampaignNameLabel).
pub const CAMPAIGN_NAME_LABEL_TABLE_WAVE106: &[(&str, &str)] = &[
    ("TRAINING", "CAMPAIGN:TRAINING"),
    ("USA", "CAMPAIGN:USA"),
    ("GLA", "CAMPAIGN:GLA"),
    ("China", "CAMPAIGN:China"),
    ("CHALLENGE_0", "CAMPAIGN:CHALLENGE_0"),
    ("CHALLENGE_1", "CAMPAIGN:CHALLENGE_1"),
    ("CHALLENGE_2", "CAMPAIGN:CHALLENGE_2"),
    ("CHALLENGE_3", "CAMPAIGN:CHALLENGE_3"),
    ("CHALLENGE_4", "CAMPAIGN:CHALLENGE_4"),
    ("CHALLENGE_5", "CAMPAIGN:CHALLENGE_5"),
    ("CHALLENGE_6", "CAMPAIGN:CHALLENGE_6"),
    ("CHALLENGE_7", "CAMPAIGN:CHALLENGE_7"),
    ("CHALLENGE_8", "CAMPAIGN:CHALLENGE_8"),
];

/// Lookup Campaign.ini residual map leaf by campaign + mission index.
pub fn campaign_mission_map_leaf_wave106(
    campaign: &str,
    mission_index: u8,
) -> Option<&'static str> {
    let table: &[CampaignMissionMapResidual] = match campaign {
        "USA" => CAMPAIGN_USA_MISSION_TABLE_WAVE106,
        "GLA" => CAMPAIGN_GLA_MISSION_TABLE_WAVE106,
        "China" => CAMPAIGN_CHINA_MISSION_TABLE_WAVE106,
        "TRAINING" => CAMPAIGN_TRAINING_MISSION_TABLE_WAVE106,
        _ => return None,
    };
    table
        .iter()
        .find(|m| m.mission_index == mission_index)
        .map(|m| m.map_leaf)
}

/// Wave 106 honesty: campaign mission residual tables deepen pack.
///
/// Freezes USA/GLA/China 5-mission chains, TRAINING, CHALLENGE_0 map order,
/// and CampaignNameLabel residual for challenge campaigns 0..8.
/// Fail-closed: not full CampaignManager INI parse / live start residual.
pub fn honesty_campaign_mission_residual_deepen_pack_wave106() -> bool {
    let usa_ok = CAMPAIGN_USA_MISSION_TABLE_WAVE106.len() == 5
        && campaign_mission_map_leaf_wave106("USA", 1) == Some("MD_USA01")
        && campaign_mission_map_leaf_wave106("USA", 5) == Some("MD_USA05")
        && CAMPAIGN_USA_MISSION_TABLE_WAVE106
            .iter()
            .all(|m| m.map_leaf.starts_with("MD_USA") && m.intro_movie == m.map_leaf);

    let gla_ok = CAMPAIGN_GLA_MISSION_TABLE_WAVE106.len() == 5
        && campaign_mission_map_leaf_wave106("GLA", 1) == Some("MD_GLA01")
        && campaign_mission_map_leaf_wave106("GLA", 5) == Some("MD_GLA05")
        && CAMPAIGN_GLA_MISSION_TABLE_WAVE106
            .iter()
            .all(|m| m.map_leaf.starts_with("MD_GLA"));

    let china_ok = CAMPAIGN_CHINA_MISSION_TABLE_WAVE106.len() == 5
        && campaign_mission_map_leaf_wave106("China", 1) == Some("MD_CHI01")
        && campaign_mission_map_leaf_wave106("China", 5) == Some("MD_CHI05")
        && CAMPAIGN_CHINA_MISSION_TABLE_WAVE106
            .iter()
            .all(|m| m.map_leaf.starts_with("MD_CHI") && m.intro_movie.starts_with("MD_China"));

    let training_ok = CAMPAIGN_TRAINING_MISSION_TABLE_WAVE106.len() == 1
        && campaign_mission_map_leaf_wave106("TRAINING", 1) == Some("Training01")
        && CAMPAIGN_TRAINING_MISSION_TABLE_WAVE106[0].intro_movie == "TrainingCampaign";

    // CHALLENGE_0 chain residual: 7 maps, Chem first, ChinaBoss last, unique.
    let mut challenge: Vec<&str> = CAMPAIGN_CHALLENGE_0_MAP_CHAIN_WAVE106.to_vec();
    let challenge_len = challenge.len();
    challenge.sort_unstable();
    let challenge_ok = challenge_len == 7
        && CAMPAIGN_CHALLENGE_0_MAP_CHAIN_WAVE106[0] == "GC_ChemGeneral"
        && CAMPAIGN_CHALLENGE_0_MAP_CHAIN_WAVE106[6] == "GC_ChinaBoss"
        && CAMPAIGN_CHALLENGE_0_MAP_CHAIN_WAVE106.contains(&"GC_Stealth")
        && CAMPAIGN_CHALLENGE_0_MAP_CHAIN_WAVE106.contains(&"GC_LaserGeneral")
        && !challenge.windows(2).any(|w| w[0] == w[1]);

    // Campaign labels residual: standard sides + CHALLENGE_0..8.
    let labels_ok = CAMPAIGN_NAME_LABEL_TABLE_WAVE106.len() >= 13
        && CAMPAIGN_NAME_LABEL_TABLE_WAVE106
            .iter()
            .any(|(c, l)| *c == "USA" && *l == "CAMPAIGN:USA")
        && CAMPAIGN_NAME_LABEL_TABLE_WAVE106
            .iter()
            .any(|(c, l)| *c == "China" && *l == "CAMPAIGN:China")
        && CAMPAIGN_NAME_LABEL_TABLE_WAVE106
            .iter()
            .any(|(c, l)| *c == "CHALLENGE_0" && *l == "CAMPAIGN:CHALLENGE_0")
        && CAMPAIGN_NAME_LABEL_TABLE_WAVE106
            .iter()
            .any(|(c, l)| *c == "CHALLENGE_8" && *l == "CAMPAIGN:CHALLENGE_8")
        && CAMPAIGN_NAME_LABEL_TABLE_WAVE106
            .iter()
            .filter(|(c, _)| c.starts_with("CHALLENGE_"))
            .count()
            == 9;

    // Sequential mission_index residual 1..N.
    let seq_ok = [
        CAMPAIGN_USA_MISSION_TABLE_WAVE106,
        CAMPAIGN_GLA_MISSION_TABLE_WAVE106,
        CAMPAIGN_CHINA_MISSION_TABLE_WAVE106,
    ]
    .iter()
    .all(|table| {
        table
            .iter()
            .enumerate()
            .all(|(i, m)| m.mission_index as usize == i + 1)
    });

    usa_ok && gla_ok && china_ok && training_ok && challenge_ok && labels_ok && seq_ok
}

// ---------------------------------------------------------------------------
// 3. MainMenu residual deepen
// ---------------------------------------------------------------------------

/// Retail MainMenu.wnd NAME= window residual count (Menus/MainMenu.wnd).
pub const MAIN_MENU_RETAIL_WINDOW_COUNT: usize = 63;

/// MainMenu.wnd primary button residual names (MainMenu.cpp nameToKey tokens).
pub const MAIN_MENU_BUTTON_NAME_TABLE_WAVE106: &[&str] = &[
    "ButtonSinglePlayer",
    "ButtonMultiplayer",
    "ButtonSkirmish",
    "ButtonOnline",
    "ButtonNetwork",
    "ButtonOptions",
    "ButtonExit",
    "ButtonChallenge",
    "ButtonUSA",
    "ButtonGLA",
    "ButtonChina",
    "ButtonReplay",
    "ButtonLoadReplay",
    "ButtonLoadGame",
    "ButtonCredits",
    "ButtonUSARecentSave",
    "ButtonUSALoadGame",
    "ButtonGLARecentSave",
    "ButtonGLALoadGame",
    "ButtonChinaRecentSave",
    "ButtonChinaLoadGame",
    "ButtonSingleBack",
    "ButtonMultiBack",
    "ButtonLoadReplayBack",
    "ButtonDiffBack",
    "ButtonEasy",
    "ButtonMedium",
    "ButtonHard",
    "ButtonGetUpdate",
    "ButtonGetMapPack",
];

/// MainMenu.wnd faction / training residual window names.
pub const MAIN_MENU_FACTION_WINDOW_TABLE_WAVE106: &[&str] = &[
    "WinFactionUS",
    "WinFactionUSSmall",
    "WinFactionUSMedium",
    "WinFactionGLA",
    "WinFactionGLASmall",
    "WinFactionGLAMedium",
    "WinFactionChina",
    "WinFactionChinaSmall",
    "WinFactionChinaMedium",
    "WinFactionTraining",
    "WinFactionTrainingSmall",
    "WinFactionTrainingMedium",
    "WinFactionSkirmish",
    "WinFactionSkirmishSmall",
    "WinFactionSkirmishMedium",
    "WinGrowMarker",
];

/// MainMenu shell residual root / scheme names.
pub const MAIN_MENU_SHELL_ROOT_NAMES_WAVE106: &[&str] =
    &["MainMenuParent", "ShellMenuScheme", "MainMenuRuler", "Logo"];

/// MainMenu TransitionHandler residual group name sample (MainMenu.cpp).
pub const MAIN_MENU_TRANSITION_GROUP_RESIDUAL: &[&str] = &[
    "MainMenuDefaultMenuLogoFade",
    "MainMenuDifficultyMenuTraining",
    "MainMenuDifficultyMenuBack",
];

/// MainMenu.wnd layout filename residual.
pub const MAIN_MENU_LAYOUT_FILENAME: &str = "Menus/MainMenu.wnd";

/// Host MainMenuState residual names (Rust ui::main_menu::MainMenuState).
pub const MAIN_MENU_STATE_NAME_TABLE_WAVE106: &[&str] = &[
    "Main",
    "SinglePlayer",
    "Multiplayer",
    "Options",
    "Credits",
];

/// Wave 106 honesty: MainMenu residual deepen pack.
///
/// Freezes MainMenu.wnd button / faction window residual tables, window count,
/// transition group residual, layout filename, and host MainMenuState names.
/// Fail-closed: not full W3D TransitionHandler / GameSpy online residual.
pub fn honesty_main_menu_residual_deepen_pack_wave106() -> bool {
    let count_ok = MAIN_MENU_RETAIL_WINDOW_COUNT == 63;

    let buttons_ok = MAIN_MENU_BUTTON_NAME_TABLE_WAVE106.len() >= 28
        && residual_name_index(MAIN_MENU_BUTTON_NAME_TABLE_WAVE106, "ButtonSinglePlayer")
            == Some(0)
        && residual_name_index(MAIN_MENU_BUTTON_NAME_TABLE_WAVE106, "ButtonSkirmish")
            .is_some()
        && residual_name_index(MAIN_MENU_BUTTON_NAME_TABLE_WAVE106, "ButtonChallenge")
            .is_some()
        && residual_name_index(MAIN_MENU_BUTTON_NAME_TABLE_WAVE106, "ButtonUSA").is_some()
        && residual_name_index(MAIN_MENU_BUTTON_NAME_TABLE_WAVE106, "ButtonExit").is_some()
        && residual_name_index(MAIN_MENU_BUTTON_NAME_TABLE_WAVE106, "ButtonHard").is_some();

    // Unique button names.
    let mut btn: Vec<&str> = MAIN_MENU_BUTTON_NAME_TABLE_WAVE106.to_vec();
    btn.sort_unstable();
    let btn_unique = !btn.windows(2).any(|w| w[0] == w[1]);

    let faction_ok = MAIN_MENU_FACTION_WINDOW_TABLE_WAVE106.len() >= 16
        && residual_name_index(MAIN_MENU_FACTION_WINDOW_TABLE_WAVE106, "WinFactionUS")
            .is_some()
        && residual_name_index(MAIN_MENU_FACTION_WINDOW_TABLE_WAVE106, "WinFactionGLA")
            .is_some()
        && residual_name_index(MAIN_MENU_FACTION_WINDOW_TABLE_WAVE106, "WinFactionChina")
            .is_some()
        && residual_name_index(
            MAIN_MENU_FACTION_WINDOW_TABLE_WAVE106,
            "WinFactionTraining",
        )
        .is_some()
        && residual_name_index(
            MAIN_MENU_FACTION_WINDOW_TABLE_WAVE106,
            "WinFactionSkirmish",
        )
        .is_some()
        && residual_name_index(MAIN_MENU_FACTION_WINDOW_TABLE_WAVE106, "WinGrowMarker")
            .is_some();

    let shell_ok = residual_name_index(MAIN_MENU_SHELL_ROOT_NAMES_WAVE106, "MainMenuParent")
        == Some(0)
        && residual_name_index(MAIN_MENU_SHELL_ROOT_NAMES_WAVE106, "ShellMenuScheme")
            .is_some()
        && MAIN_MENU_LAYOUT_FILENAME == "Menus/MainMenu.wnd"
        && MAIN_MENU_TRANSITION_GROUP_RESIDUAL.len() >= 3
        && MAIN_MENU_TRANSITION_GROUP_RESIDUAL[0] == "MainMenuDefaultMenuLogoFade";

    let state_ok = MAIN_MENU_STATE_NAME_TABLE_WAVE106.len() == 5
        && residual_name_index(MAIN_MENU_STATE_NAME_TABLE_WAVE106, "Main") == Some(0)
        && residual_name_index(MAIN_MENU_STATE_NAME_TABLE_WAVE106, "SinglePlayer")
            == Some(1)
        && residual_name_index(MAIN_MENU_STATE_NAME_TABLE_WAVE106, "Credits") == Some(4);

    // Button + faction + shell roots ≤ retail window count residual.
    let coverage_ok = MAIN_MENU_BUTTON_NAME_TABLE_WAVE106.len()
        + MAIN_MENU_FACTION_WINDOW_TABLE_WAVE106.len()
        + MAIN_MENU_SHELL_ROOT_NAMES_WAVE106.len()
        <= MAIN_MENU_RETAIL_WINDOW_COUNT;

    count_ok && buttons_ok && btn_unique && faction_ok && shell_ok && state_ok && coverage_ok
}

// ---------------------------------------------------------------------------
// 4. GameWindow residual deepen
// ---------------------------------------------------------------------------

/// C++ WIN_STATUS residual bit value + name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WinStatusResidual {
    pub name: &'static str,
    pub bit: u32,
}

/// C++ WIN_STATUS_* residual table (GameWindow.h) — NONE is 0 sentinel.
pub const WIN_STATUS_RESIDUAL_TABLE_WAVE106: &[WinStatusResidual] = &[
    WinStatusResidual {
        name: "NONE",
        bit: 0x0000_0000,
    },
    WinStatusResidual {
        name: "ACTIVE",
        bit: 0x0000_0001,
    },
    WinStatusResidual {
        name: "TOGGLE",
        bit: 0x0000_0002,
    },
    WinStatusResidual {
        name: "DRAGABLE",
        bit: 0x0000_0004,
    },
    WinStatusResidual {
        name: "ENABLED",
        bit: 0x0000_0008,
    },
    WinStatusResidual {
        name: "HIDDEN",
        bit: 0x0000_0010,
    },
    WinStatusResidual {
        name: "ABOVE",
        bit: 0x0000_0020,
    },
    WinStatusResidual {
        name: "BELOW",
        bit: 0x0000_0040,
    },
    WinStatusResidual {
        name: "IMAGE",
        bit: 0x0000_0080,
    },
    WinStatusResidual {
        name: "TAB_STOP",
        bit: 0x0000_0100,
    },
    WinStatusResidual {
        name: "NO_INPUT",
        bit: 0x0000_0200,
    },
    WinStatusResidual {
        name: "NO_FOCUS",
        bit: 0x0000_0400,
    },
    WinStatusResidual {
        name: "DESTROYED",
        bit: 0x0000_0800,
    },
    WinStatusResidual {
        name: "BORDER",
        bit: 0x0000_1000,
    },
    WinStatusResidual {
        name: "SMOOTH_TEXT",
        bit: 0x0000_2000,
    },
    WinStatusResidual {
        name: "ONE_LINE",
        bit: 0x0000_4000,
    },
    WinStatusResidual {
        name: "NO_FLUSH",
        bit: 0x0000_8000,
    },
    WinStatusResidual {
        name: "SEE_THRU",
        bit: 0x0001_0000,
    },
    WinStatusResidual {
        name: "RIGHT_CLICK",
        bit: 0x0002_0000,
    },
    WinStatusResidual {
        name: "WRAP_CENTERED",
        bit: 0x0004_0000,
    },
    WinStatusResidual {
        name: "CHECK_LIKE",
        bit: 0x0008_0000,
    },
    WinStatusResidual {
        name: "HOTKEY_TEXT",
        bit: 0x0010_0000,
    },
    WinStatusResidual {
        name: "USE_OVERLAY_STATES",
        bit: 0x0020_0000,
    },
    WinStatusResidual {
        name: "NOT_READY",
        bit: 0x0040_0000,
    },
    WinStatusResidual {
        name: "FLASHING",
        bit: 0x0080_0000,
    },
    WinStatusResidual {
        name: "ALWAYS_COLOR",
        bit: 0x0100_0000,
    },
    WinStatusResidual {
        name: "ON_MOUSE_DOWN",
        bit: 0x0200_0000,
    },
    WinStatusResidual {
        name: "SHORTCUT_BUTTON",
        bit: 0x0400_0000,
    },
];

/// C++ GameWindowMessage residual names (GWM_*; GameWindow.h).
pub const GAME_WINDOW_MESSAGE_NAME_TABLE_WAVE106: &[&str] = &[
    "GWM_NONE",
    "GWM_CREATE",
    "GWM_DESTROY",
    "GWM_ACTIVATE",
    "GWM_ENABLE",
    "GWM_LEFT_DOWN",
    "GWM_LEFT_UP",
    "GWM_LEFT_DOUBLE_CLICK",
    "GWM_LEFT_DRAG",
    "GWM_MIDDLE_DOWN",
    "GWM_MIDDLE_UP",
    "GWM_MIDDLE_DOUBLE_CLICK",
    "GWM_MIDDLE_DRAG",
    "GWM_RIGHT_DOWN",
    "GWM_RIGHT_UP",
    "GWM_RIGHT_DOUBLE_CLICK",
    "GWM_RIGHT_DRAG",
    "GWM_MOUSE_ENTERING",
    "GWM_MOUSE_LEAVING",
    "GWM_WHEEL_UP",
    "GWM_WHEEL_DOWN",
    "GWM_CHAR",
    "GWM_SCRIPT_CREATE",
    "GWM_INPUT_FOCUS",
    "GWM_MOUSE_POS",
    "GWM_IME_CHAR",
    "GWM_IME_STRING",
];

/// C++ `GWM_USER` residual base for user-defined messages.
pub const GWM_USER_RESIDUAL: u32 = 32768;

/// C++ `WindowMsgHandledType` residual.
pub const WINDOW_MSG_IGNORED: u32 = 0;
pub const WINDOW_MSG_HANDLED: u32 = 1;
pub const WINDOW_MSG_HANDLED_TYPE_NAMES: &[&str] = &["MSG_IGNORED", "MSG_HANDLED"];

/// Lookup WIN_STATUS residual bit by name.
pub fn win_status_bit_wave106(name: &str) -> Option<u32> {
    WIN_STATUS_RESIDUAL_TABLE_WAVE106
        .iter()
        .find(|e| e.name == name)
        .map(|e| e.bit)
}

/// Wave 106 honesty: GameWindow residual deepen pack.
///
/// Freezes WIN_STATUS bit table, GWM_* message residual names, GWM_USER, and
/// MSG_IGNORED/MSG_HANDLED residual.
/// Fail-closed: not full GameWindow GPU draw / exclusive WindowManager residual.
pub fn honesty_game_window_residual_deepen_pack_wave106() -> bool {
    // 1 NONE + 27 status bits = 28 entries; highest bit SHORTCUT_BUTTON.
    let status_ok = WIN_STATUS_RESIDUAL_TABLE_WAVE106.len() == 28
        && win_status_bit_wave106("NONE") == Some(0)
        && win_status_bit_wave106("ACTIVE") == Some(0x0000_0001)
        && win_status_bit_wave106("ENABLED") == Some(0x0000_0008)
        && win_status_bit_wave106("HIDDEN") == Some(0x0000_0010)
        && win_status_bit_wave106("SEE_THRU") == Some(0x0001_0000)
        && win_status_bit_wave106("FLASHING") == Some(0x0080_0000)
        && win_status_bit_wave106("SHORTCUT_BUTTON") == Some(0x0400_0000);

    // Non-NONE bits are unique powers of two (bit 0..26).
    let mut bits: Vec<u32> = WIN_STATUS_RESIDUAL_TABLE_WAVE106
        .iter()
        .filter(|e| e.name != "NONE")
        .map(|e| e.bit)
        .collect();
    bits.sort_unstable();
    let powers_ok = bits.len() == 27
        && !bits.windows(2).any(|w| w[0] == w[1])
        && bits.iter().all(|b| b.count_ones() == 1)
        && bits[0] == 1
        && *bits.last().unwrap() == 0x0400_0000;

    // GameWindowMessage residual (GWM_NONE..GWM_IME_STRING).
    let msg_ok = GAME_WINDOW_MESSAGE_NAME_TABLE_WAVE106.len() == 27
        && residual_name_index(GAME_WINDOW_MESSAGE_NAME_TABLE_WAVE106, "GWM_NONE")
            == Some(0)
        && residual_name_index(GAME_WINDOW_MESSAGE_NAME_TABLE_WAVE106, "GWM_CREATE")
            == Some(1)
        && residual_name_index(GAME_WINDOW_MESSAGE_NAME_TABLE_WAVE106, "GWM_DESTROY")
            == Some(2)
        && residual_name_index(GAME_WINDOW_MESSAGE_NAME_TABLE_WAVE106, "GWM_LEFT_DOWN")
            .is_some()
        && residual_name_index(GAME_WINDOW_MESSAGE_NAME_TABLE_WAVE106, "GWM_IME_STRING")
            == Some(26)
        && GWM_USER_RESIDUAL == 32768
        && GWM_USER_RESIDUAL > (GAME_WINDOW_MESSAGE_NAME_TABLE_WAVE106.len() as u32);

    let handled_ok = WINDOW_MSG_IGNORED == 0
        && WINDOW_MSG_HANDLED == 1
        && WINDOW_MSG_HANDLED_TYPE_NAMES.len() == 2
        && residual_name_index(WINDOW_MSG_HANDLED_TYPE_NAMES, "MSG_IGNORED") == Some(0)
        && residual_name_index(WINDOW_MSG_HANDLED_TYPE_NAMES, "MSG_HANDLED") == Some(1);

    // Composite residual: ENABLED|IMAGE|BORDER used by default push-button peels.
    let composite = win_status_bit_wave106("ENABLED").unwrap()
        | win_status_bit_wave106("IMAGE").unwrap()
        | win_status_bit_wave106("BORDER").unwrap();
    let composite_ok = composite == (0x0000_0008 | 0x0000_0080 | 0x0000_1000);

    status_ok && powers_ok && msg_ok && handled_ok && composite_ok
}

// ---------------------------------------------------------------------------
// 5. WindowLayout residual deepen
// ---------------------------------------------------------------------------

/// WindowLayout residual callback step names (init / update / shutdown).
pub const WINDOW_LAYOUT_CALLBACK_STEPS_WAVE106: &[&str] =
    &["INIT", "UPDATE", "SHUTDOWN"];

/// WindowLayout residual operation names (load / hide / bringForward / destroy).
pub const WINDOW_LAYOUT_OPERATION_NAME_TABLE_WAVE106: &[&str] = &[
    "LOAD",
    "HIDE",
    "UNHIDE",
    "BRING_FORWARD",
    "ADD_WINDOW",
    "REMOVE_WINDOW",
    "DESTROY_WINDOWS",
    "GET_FIRST_WINDOW",
    "RUN_INIT",
    "RUN_UPDATE",
    "RUN_SHUTDOWN",
];

/// Shell layout filename residual table (WindowZH/Window/Menus/*.wnd).
pub const SHELL_LAYOUT_FILENAME_TABLE_WAVE106: &[&str] = &[
    "Menus/MainMenu.wnd",
    "Menus/OptionsMenu.wnd",
    "Menus/CreditsMenu.wnd",
    "Menus/MapSelectMenu.wnd",
    "Menus/SinglePlayerMenu.wnd",
    "Menus/DifficultySelect.wnd",
    "Menus/ChallengeMenu.wnd",
    "Menus/PopupSaveLoad.wnd",
    "Menus/LanLobbyMenu.wnd",
    "Menus/LanGameOptionsMenu.wnd",
    "Menus/MessageBox.wnd",
    "ControlBar.wnd",
];

/// WindowLayout pool residual name (MEMORY_POOL_GLUE WindowLayoutPool).
pub const WINDOW_LAYOUT_POOL_NAME: &str = "WindowLayoutPool";

/// WindowLayout ctor residual: m_hidden starts false, m_windowCount starts 0.
pub const WINDOW_LAYOUT_CTOR_HIDDEN_DEFAULT: bool = false;
pub const WINDOW_LAYOUT_CTOR_WINDOW_COUNT_DEFAULT: i32 = 0;

/// Layout hide residual: hide(true) → isHidden true; hide(false) → visible.
#[inline]
pub fn window_layout_hide_residual(hide: bool) -> bool {
    // C++ WindowLayout::hide sets m_hidden = hide (and propagates to windows).
    hide
}

/// Wave 106 honesty: WindowLayout residual deepen pack.
///
/// Freezes callback/operation residual tables, shell layout filenames,
/// WindowLayoutPool name, ctor defaults, and hide residual pure function.
/// Fail-closed: not full WindowLayout::load .wnd parse residual.
pub fn honesty_window_layout_residual_deepen_pack_wave106() -> bool {
    let callbacks_ok = WINDOW_LAYOUT_CALLBACK_STEPS_WAVE106.len() == 3
        && residual_name_index(WINDOW_LAYOUT_CALLBACK_STEPS_WAVE106, "INIT") == Some(0)
        && residual_name_index(WINDOW_LAYOUT_CALLBACK_STEPS_WAVE106, "UPDATE") == Some(1)
        && residual_name_index(WINDOW_LAYOUT_CALLBACK_STEPS_WAVE106, "SHUTDOWN")
            == Some(2);

    let ops_ok = WINDOW_LAYOUT_OPERATION_NAME_TABLE_WAVE106.len() >= 10
        && residual_name_index(WINDOW_LAYOUT_OPERATION_NAME_TABLE_WAVE106, "LOAD")
            == Some(0)
        && residual_name_index(WINDOW_LAYOUT_OPERATION_NAME_TABLE_WAVE106, "HIDE")
            .is_some()
        && residual_name_index(
            WINDOW_LAYOUT_OPERATION_NAME_TABLE_WAVE106,
            "BRING_FORWARD",
        )
        .is_some()
        && residual_name_index(
            WINDOW_LAYOUT_OPERATION_NAME_TABLE_WAVE106,
            "DESTROY_WINDOWS",
        )
        .is_some()
        && residual_name_index(WINDOW_LAYOUT_OPERATION_NAME_TABLE_WAVE106, "RUN_INIT")
            .is_some();

    let layouts_ok = SHELL_LAYOUT_FILENAME_TABLE_WAVE106.len() >= 12
        && residual_name_index(SHELL_LAYOUT_FILENAME_TABLE_WAVE106, "Menus/MainMenu.wnd")
            == Some(0)
        && residual_name_index(SHELL_LAYOUT_FILENAME_TABLE_WAVE106, "ControlBar.wnd")
            .is_some()
        && residual_name_index(
            SHELL_LAYOUT_FILENAME_TABLE_WAVE106,
            "Menus/PopupSaveLoad.wnd",
        )
        .is_some()
        && residual_name_index(
            SHELL_LAYOUT_FILENAME_TABLE_WAVE106,
            "Menus/ChallengeMenu.wnd",
        )
        .is_some()
        // MainMenu layout filename cross-link.
        && residual_name_index(
            SHELL_LAYOUT_FILENAME_TABLE_WAVE106,
            MAIN_MENU_LAYOUT_FILENAME,
        )
        .is_some();

    // Unique layout filenames.
    let mut layouts: Vec<&str> = SHELL_LAYOUT_FILENAME_TABLE_WAVE106.to_vec();
    layouts.sort_unstable();
    let unique_ok = !layouts.windows(2).any(|w| w[0] == w[1]);

    let ctor_ok = WINDOW_LAYOUT_POOL_NAME == "WindowLayoutPool"
        && !WINDOW_LAYOUT_CTOR_HIDDEN_DEFAULT
        && WINDOW_LAYOUT_CTOR_WINDOW_COUNT_DEFAULT == 0
        && window_layout_hide_residual(true)
        && !window_layout_hide_residual(false);

    callbacks_ok && ops_ok && layouts_ok && unique_ok && ctor_ok
}

// ---------------------------------------------------------------------------
// Combined Wave 106 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 106 shell / campaign / save residual honesty pack.
///
/// GameState + campaign mission tables + MainMenu + GameWindow + WindowLayout.
/// Fail-closed: not full retail W3D shell / save file I/O / network residual.
pub fn honesty_shell_campaign_save_residual_pack_wave106() -> bool {
    honesty_game_state_residual_deepen_pack_wave106()
        && honesty_campaign_mission_residual_deepen_pack_wave106()
        && honesty_main_menu_residual_deepen_pack_wave106()
        && honesty_game_window_residual_deepen_pack_wave106()
        && honesty_window_layout_residual_deepen_pack_wave106()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_state_residual_pack_honesty_wave106() {
        assert!(honesty_game_state_residual_deepen_pack_wave106());
        assert_eq!(
            residual_name_index(GAME_STATE_SNAPSHOT_SAVELOAD_BLOCKS, "CHUNK_GameState"),
            Some(0)
        );
        assert_eq!(GAME_STATE_SNAPSHOT_DEEPCRC_LOGICONLY_BLOCKS.len(), 6);
    }

    #[test]
    fn campaign_mission_residual_pack_honesty_wave106() {
        assert!(honesty_campaign_mission_residual_deepen_pack_wave106());
        assert_eq!(
            campaign_mission_map_leaf_wave106("China", 3),
            Some("MD_CHI03")
        );
        assert_eq!(CAMPAIGN_CHALLENGE_0_MAP_CHAIN_WAVE106[0], "GC_ChemGeneral");
    }

    #[test]
    fn main_menu_residual_pack_honesty_wave106() {
        assert!(honesty_main_menu_residual_deepen_pack_wave106());
        assert_eq!(MAIN_MENU_RETAIL_WINDOW_COUNT, 63);
        assert!(residual_name_index(MAIN_MENU_BUTTON_NAME_TABLE_WAVE106, "ButtonSkirmish").is_some());
    }

    #[test]
    fn game_window_residual_pack_honesty_wave106() {
        assert!(honesty_game_window_residual_deepen_pack_wave106());
        assert_eq!(win_status_bit_wave106("SHORTCUT_BUTTON"), Some(0x0400_0000));
        assert_eq!(GWM_USER_RESIDUAL, 32768);
    }

    #[test]
    fn window_layout_residual_pack_honesty_wave106() {
        assert!(honesty_window_layout_residual_deepen_pack_wave106());
        assert!(window_layout_hide_residual(true));
        assert!(!window_layout_hide_residual(false));
    }

    #[test]
    fn shell_campaign_save_residual_pack_honesty_wave106() {
        assert!(honesty_shell_campaign_save_residual_pack_wave106());
    }
}
