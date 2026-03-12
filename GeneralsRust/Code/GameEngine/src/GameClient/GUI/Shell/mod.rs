// FILE: mod.rs
// Description: Shell UI modules
//
// This module contains the complete UI implementation for shell menus including
// skirmish game setup, replay browsing/playback, map selection, preferences,
// battle honors tracking, and options/settings configuration.

// Skirmish/Singleplayer menus
pub mod skirmish_game_options_menu;
pub mod skirmish_map_select_menu;
pub mod skirmish_preferences;
pub mod skirmish_battle_honors;

// Multiplayer menus
pub mod lan_lobby_menu;
pub mod lan_game_options_menu;
pub mod lan_map_select_menu;
pub mod wol_lobby_menu;

// General menus
pub mod replay_menu;
pub mod replay_controls;
pub mod options_menu;
pub mod keyboard_options_menu;

// Campaign and Challenge Mode menus
pub mod campaign_manager;
pub mod challenge_generals;
pub mod challenge_menu;
pub mod map_select_menu;
pub mod difficulty_select;

// Re-export main types for convenience
pub use skirmish_game_options_menu::{
    SkirmishGameOptionsMenu,
    SkirmishGameInfo,
    GameSlot,
    SlotState,
    Money,
    MapMetaData,
    ICoord2D,
    MAX_SLOTS,
    PLAYERTEMPLATE_RANDOM,
    PLAYERTEMPLATE_MIN,
};

pub use skirmish_map_select_menu::{
    SkirmishMapSelectMenu,
    MapCache,
    MapListEntry,
    MapDifficulty,
    get_default_map,
    is_valid_map,
    get_map_preview_image_path,
};

pub use skirmish_preferences::{
    SkirmishPreferences,
    UserPreferences,
    game_info_to_ascii_string,
    parse_ascii_string_to_game_info,
};

pub use skirmish_battle_honors::{
    SkirmishBattleHonors,
    BattleHonorInfo,
    Difficulty,
    BATTLE_HONOR_CAMPAIGN_CHINA,
    BATTLE_HONOR_CAMPAIGN_GLA,
    BATTLE_HONOR_CAMPAIGN_USA,
    BATTLE_HONOR_CHALLENGE_MODE,
    BATTLE_HONOR_AIR_WING,
    BATTLE_HONOR_BATTLE_TANK,
    BATTLE_HONOR_ENDURANCE,
    BATTLE_HONOR_APOCALYPSE,
    BATTLE_HONOR_BLITZ10,
    BATTLE_HONOR_BLITZ5,
    BATTLE_HONOR_STREAK,
    BATTLE_HONOR_DOMINATION,
    BATTLE_HONOR_ULTIMATE,
    BATTLE_HONOR_OFFICERSCLUB,
};

pub use replay_menu::{
    ReplayMenu,
    ReplayHeader,
    ReplayGameInfo,
    ReplayListEntry,
    Color,
    SystemTimeValue,
    KeyCode,
    KeyState,
    parse_ascii_string_to_game_info,
    get_unicode_time_buffer,
};

pub use replay_controls::{
    ReplayControls,
    GameWindow,
    WindowMsg,
    WindowMsgHandledType,
    replay_control_input,
    replay_control_system,
};

pub use options_menu::{
    OptionsMenu,
    OptionPreferences,
    DisplaySettings,
    StaticGameLODLevel,
    IPEnumeration,
    EnumeratedIP,
    DIFFICULTY_EASY,
    DIFFICULTY_MEDIUM,
    DIFFICULTY_HARD,
};

pub use keyboard_options_menu::{
    KeyboardOptionsMenu,
    MetaMap,
    MetaMapRec,
    MappableKeyType,
    MappableKeyCategory,
    HotkeyBinding,
    KeyModifiers,
    CATEGORY_NAMES,
};

pub use lan_lobby_menu::{
    LANLobbyMenu,
    LANPreferences,
    WindowLayout,
};

pub use lan_game_options_menu::{
    LANGameOptionsMenu,
    SlotState,
    MapMetaData as LANMapMetaData,
};

pub use lan_map_select_menu::{
    LANMapSelectMenu,
    PostToLanGameType,
    MapImage,
};

pub use wol_lobby_menu::{
    WOLLobbyMenu,
    PeerResponseType,
    PeerResponse,
    PlayerInfo,
    GameSpyGroupRoom,
    GameSpyStagingRoom,
    RoomType,
};

// Campaign and Challenge exports
pub use campaign_manager::{
    CampaignManager,
    Campaign,
    Mission,
    GameDifficulty,
    AudioEventRTS,
    MAX_OBJECTIVE_LINES,
    MAX_DISPLAYED_UNITS,
    INVALID_MISSION_NUMBER,
    init_campaign_manager,
    get_campaign_manager,
    get_campaign_manager_mut,
};

pub use challenge_generals::{
    ChallengeGenerals,
    GeneralPersona,
    NUM_GENERALS,
    create_challenge_generals,
    init_challenge_generals,
    get_challenge_generals,
    get_challenge_generals_mut,
};

pub use challenge_menu::{
    ChallengeMenuState,
    ChallengeMenuAction,
    challenge_menu_init,
    challenge_menu_update,
    challenge_menu_shutdown,
    handle_button_selected as challenge_handle_button_selected,
    handle_mouse_entering,
    handle_mouse_leaving,
};

pub use map_select_menu::{
    MapSelectMenuState,
    MapSelectMenuAction,
    map_select_menu_init,
    map_select_menu_update,
    map_select_menu_shutdown,
    setup_game_start,
    do_game_start,
    handle_button_selected as map_handle_button_selected,
    handle_map_double_click,
};

pub use difficulty_select::{
    DifficultySelectState,
    DifficultySelectAction,
    difficulty_select_init,
    set_difficulty_radio_button,
    handle_button_selected as difficulty_handle_button_selected,
    setup_game_start_with_difficulty,
};
