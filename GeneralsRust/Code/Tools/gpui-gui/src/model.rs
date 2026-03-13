use bitflags::bitflags;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameWindowMessage {
    None = 0,
    Create,
    Destroy,
    Activate,
    Enable,
    LeftDown,
    LeftUp,
    LeftDoubleClick,
    LeftDrag,
    MiddleDown,
    MiddleUp,
    MiddleDoubleClick,
    MiddleDrag,
    RightDown,
    RightUp,
    RightDoubleClick,
    RightDrag,
    MouseEntering,
    MouseLeaving,
    WheelUp,
    WheelDown,
    Char,
    ScriptCreate,
    InputFocus,
    MousePos,
    ImeChar,
    ImeString,
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct WindowStatus: u32 {
        const ACTIVE = 0x0000_0001;
        const TOGGLE = 0x0000_0002;
        const DRAGABLE = 0x0000_0004;
        const ENABLED = 0x0000_0008;
        const HIDDEN = 0x0000_0010;
        const ABOVE = 0x0000_0020;
        const BELOW = 0x0000_0040;
        const IMAGE = 0x0000_0080;
        const TAB_STOP = 0x0000_0100;
        const NO_INPUT = 0x0000_0200;
        const NO_FOCUS = 0x0000_0400;
        const DESTROYED = 0x0000_0800;
        const BORDER = 0x0000_1000;
        const SMOOTH_TEXT = 0x0000_2000;
        const ONE_LINE = 0x0000_4000;
        const NO_FLUSH = 0x0000_8000;
        const SEE_THRU = 0x0001_0000;
        const RIGHT_CLICK = 0x0002_0000;
        const WRAP_CENTERED = 0x0004_0000;
        const CHECK_LIKE = 0x0008_0000;
        const HOTKEY_TEXT = 0x0010_0000;
        const USE_OVERLAY_STATES = 0x0020_0000;
        const NOT_READY = 0x0040_0000;
        const FLASHING = 0x0080_0000;
        const ALWAYS_COLOR = 0x0100_0000;
        const ON_MOUSE_DOWN = 0x0200_0000;
        const SHORTCUT_BUTTON = 0x0400_0000;
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct GadgetWindowStyle: u32 {
        const PUSH_BUTTON = 0x0000_0001;
        const RADIO_BUTTON = 0x0000_0002;
        const CHECK_BOX = 0x0000_0004;
        const VERT_SLIDER = 0x0000_0008;
        const HORZ_SLIDER = 0x0000_0010;
        const SCROLL_LISTBOX = 0x0000_0020;
        const ENTRY_FIELD = 0x0000_0040;
        const STATIC_TEXT = 0x0000_0080;
        const PROGRESS_BAR = 0x0000_0100;
        const USER_WINDOW = 0x0000_0200;
        const MOUSE_TRACK = 0x0000_0400;
        const ANIMATED = 0x0000_0800;
        const TAB_STOP = 0x0000_1000;
        const TAB_CONTROL = 0x0000_2000;
        const TAB_PANE = 0x0000_4000;
        const COMBO_BOX = 0x0000_8000;
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CommandOption: u32 {
        const NEED_TARGET_ENEMY_OBJECT = 0x0000_0001;
        const NEED_TARGET_NEUTRAL_OBJECT = 0x0000_0002;
        const NEED_TARGET_ALLY_OBJECT = 0x0000_0004;
        const ALLOW_SHRUBBERY_TARGET = 0x0000_0010;
        const NEED_TARGET_POS = 0x0000_0020;
        const NEED_UPGRADE = 0x0000_0040;
        const NEED_SPECIAL_POWER_SCIENCE = 0x0000_0080;
        const OK_FOR_MULTI_SELECT = 0x0000_0100;
        const CONTEXTMODE_COMMAND = 0x0000_0200;
        const CHECK_LIKE = 0x0000_0400;
        const ALLOW_MINE_TARGET = 0x0000_0800;
        const ATTACK_OBJECTS_POSITION = 0x0000_1000;
        const OPTION_ONE = 0x0000_2000;
        const OPTION_TWO = 0x0000_4000;
        const OPTION_THREE = 0x0000_8000;
        const NOT_QUEUEABLE = 0x0001_0000;
        const SINGLE_USE_COMMAND = 0x0002_0000;
        const COMMAND_FIRED_BY_SCRIPT = 0x0004_0000;
        const SCRIPT_ONLY = 0x0008_0000;
        const IGNORES_UNDERPOWERED = 0x0010_0000;
        const USES_MINE_CLEARING_WEAPONSET = 0x0020_0000;
        const CAN_USE_WAYPOINTS = 0x0040_0000;
        const MUST_BE_STOPPED = 0x0080_0000;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiCommandType {
    None,
    DozerConstruct,
    DozerConstructCancel,
    UnitBuild,
    CancelUnitBuild,
    PlayerUpgrade,
    ObjectUpgrade,
    CancelUpgrade,
    AttackMove,
    Guard,
    GuardWithoutPursuit,
    GuardFlyingUnitsOnly,
    Stop,
    Waypoints,
    ExitContainer,
    Evacuate,
    ExecuteRailedTransport,
    BeaconDelete,
    SetRallyPoint,
    Sell,
    FireWeapon,
    SpecialPower,
    PurchaseScience,
    HackInternet,
    ToggleOvercharge,
    CombatDrop,
    SwitchWeapon,
    HijackVehicle,
    ConvertToCarbomb,
    SabotageBuilding,
    PlaceBeacon,
    SpecialPowerFromShortcut,
    SpecialPowerConstruct,
    SpecialPowerConstructFromShortcut,
    SelectAllUnitsOfType,
}

impl GuiCommandType {
    pub fn title(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::DozerConstruct => "Dozer Construct",
            Self::DozerConstructCancel => "Cancel Construct",
            Self::UnitBuild => "Build Unit",
            Self::CancelUnitBuild => "Cancel Build",
            Self::PlayerUpgrade => "Player Upgrade",
            Self::ObjectUpgrade => "Object Upgrade",
            Self::CancelUpgrade => "Cancel Upgrade",
            Self::AttackMove => "Attack Move",
            Self::Guard => "Guard",
            Self::GuardWithoutPursuit => "Guard Area",
            Self::GuardFlyingUnitsOnly => "Guard Air",
            Self::Stop => "Stop",
            Self::Waypoints => "Waypoints",
            Self::ExitContainer => "Exit Container",
            Self::Evacuate => "Evacuate",
            Self::ExecuteRailedTransport => "Railed Transport",
            Self::BeaconDelete => "Delete Beacon",
            Self::SetRallyPoint => "Set Rally Point",
            Self::Sell => "Sell",
            Self::FireWeapon => "Fire Weapon",
            Self::SpecialPower => "Special Power",
            Self::PurchaseScience => "Purchase Science",
            Self::HackInternet => "Hack Internet",
            Self::ToggleOvercharge => "Toggle Overcharge",
            Self::CombatDrop => "Combat Drop",
            Self::SwitchWeapon => "Switch Weapon",
            Self::HijackVehicle => "Hijack Vehicle",
            Self::ConvertToCarbomb => "Car Bomb",
            Self::SabotageBuilding => "Sabotage",
            Self::PlaceBeacon => "Place Beacon",
            Self::SpecialPowerFromShortcut => "Shortcut Power",
            Self::SpecialPowerConstruct => "Power Construct",
            Self::SpecialPowerConstructFromShortcut => "Shortcut Construct",
            Self::SelectAllUnitsOfType => "Select Type",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LegacyCommandButton {
    pub label: &'static str,
    pub command: GuiCommandType,
    pub options: CommandOption,
    pub progress: f32,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug)]
pub struct LegacyWindowNode {
    pub id: i32,
    pub title: String,
    pub tooltip: String,
    pub rect: LegacyRect,
    pub status: WindowStatus,
    pub style: GadgetWindowStyle,
}

#[derive(Clone, Debug)]
pub struct WindowLayoutState {
    pub filename: String,
    pub hidden: bool,
    pub windows: Vec<LegacyWindowNode>,
}

#[derive(Clone, Debug)]
pub struct ShellState {
    pub stack: Vec<&'static str>,
}

impl WindowLayoutState {
    pub fn preview_for(screen_name: &str) -> Self {
        let spec = preview_spec(screen_name);

        Self {
            filename: format!("Data/English/{}.wnd", spec.filename),
            hidden: false,
            windows: vec![
                LegacyWindowNode {
                    id: 1000,
                    title: spec.title.to_string(),
                    tooltip: format!("Top-level {} layout window", spec.title),
                    rect: LegacyRect {
                        x: 96,
                        y: 64,
                        width: 1120,
                        height: 720,
                    },
                    status: WindowStatus::ACTIVE | WindowStatus::ENABLED | WindowStatus::BORDER,
                    style: GadgetWindowStyle::USER_WINDOW,
                },
                LegacyWindowNode {
                    id: 1100,
                    title: spec.primary_title.to_string(),
                    tooltip: spec.primary_tooltip.to_string(),
                    rect: spec.primary_rect,
                    status: spec.primary_status,
                    style: spec.primary_style,
                },
                LegacyWindowNode {
                    id: 1200,
                    title: spec.secondary_title.to_string(),
                    tooltip: spec.secondary_tooltip.to_string(),
                    rect: spec.secondary_rect,
                    status: spec.secondary_status,
                    style: spec.secondary_style,
                },
            ],
        }
    }
}

struct PreviewSpec {
    filename: &'static str,
    title: &'static str,
    primary_title: &'static str,
    primary_tooltip: &'static str,
    primary_rect: LegacyRect,
    primary_status: WindowStatus,
    primary_style: GadgetWindowStyle,
    secondary_title: &'static str,
    secondary_tooltip: &'static str,
    secondary_rect: LegacyRect,
    secondary_status: WindowStatus,
    secondary_style: GadgetWindowStyle,
}

fn preview_spec(screen_name: &str) -> PreviewSpec {
    match screen_name {
        "MainMenu" => PreviewSpec {
            filename: "MainMenu",
            title: "Main Menu",
            primary_title: "ButtonSinglePlayer",
            primary_tooltip: "Enter the single-player shell flow",
            primary_rect: LegacyRect {
                x: 128,
                y: 610,
                width: 280,
                height: 48,
            },
            primary_status: WindowStatus::ENABLED | WindowStatus::IMAGE,
            primary_style: GadgetWindowStyle::PUSH_BUTTON,
            secondary_title: "MainMenuDefaultPanel",
            secondary_tooltip: "Primary main-menu button stack",
            secondary_rect: LegacyRect {
                x: 756,
                y: 152,
                width: 300,
                height: 428,
            },
            secondary_status: WindowStatus::ENABLED | WindowStatus::SMOOTH_TEXT,
            secondary_style: GadgetWindowStyle::USER_WINDOW,
        },
        "ReplayMenu" => PreviewSpec {
            filename: "ReplayMenu",
            title: "Replay Browser",
            primary_title: "ButtonLoadReplay",
            primary_tooltip: "Load the currently selected replay",
            primary_rect: LegacyRect {
                x: 128,
                y: 610,
                width: 240,
                height: 48,
            },
            primary_status: WindowStatus::ENABLED | WindowStatus::IMAGE,
            primary_style: GadgetWindowStyle::PUSH_BUTTON,
            secondary_title: "ListboxReplayFiles",
            secondary_tooltip: "Replay list with version, map, and date columns",
            secondary_rect: LegacyRect {
                x: 706,
                y: 142,
                width: 350,
                height: 450,
            },
            secondary_status: WindowStatus::ENABLED | WindowStatus::SMOOTH_TEXT,
            secondary_style: GadgetWindowStyle::SCROLL_LISTBOX,
        },
        "LanLobbyMenu" => PreviewSpec {
            filename: "LanLobbyMenu",
            title: "LAN Lobby",
            primary_title: "TextEntryChat",
            primary_tooltip: "Chat entry for LAN lobby communication",
            primary_rect: LegacyRect {
                x: 132,
                y: 628,
                width: 520,
                height: 32,
            },
            primary_status: WindowStatus::ENABLED | WindowStatus::IMAGE | WindowStatus::TAB_STOP,
            primary_style: GadgetWindowStyle::ENTRY_FIELD,
            secondary_title: "ListboxPlayers",
            secondary_tooltip: "Lobby player roster",
            secondary_rect: LegacyRect {
                x: 770,
                y: 160,
                width: 286,
                height: 398,
            },
            secondary_status: WindowStatus::ENABLED | WindowStatus::SMOOTH_TEXT,
            secondary_style: GadgetWindowStyle::SCROLL_LISTBOX,
        },
        "OptionsMenu" => PreviewSpec {
            filename: "OptionsMenu",
            title: "Options",
            primary_title: "ButtonOptionsAccept",
            primary_tooltip: "Apply option changes",
            primary_rect: LegacyRect {
                x: 128,
                y: 614,
                width: 220,
                height: 44,
            },
            primary_status: WindowStatus::ENABLED | WindowStatus::IMAGE,
            primary_style: GadgetWindowStyle::PUSH_BUTTON,
            secondary_title: "OptionsTabs",
            secondary_tooltip: "Tabbed audio, video, and gameplay settings",
            secondary_rect: LegacyRect {
                x: 700,
                y: 136,
                width: 356,
                height: 470,
            },
            secondary_status: WindowStatus::ENABLED | WindowStatus::SMOOTH_TEXT,
            secondary_style: GadgetWindowStyle::TAB_CONTROL,
        },
        _ => PreviewSpec {
            filename: "MappedScreen",
            title: "Mapped Screen",
            primary_title: "PrimaryControl",
            primary_tooltip: "Primary layout action",
            primary_rect: LegacyRect {
                x: 128,
                y: 610,
                width: 280,
                height: 48,
            },
            primary_status: WindowStatus::ENABLED | WindowStatus::IMAGE,
            primary_style: GadgetWindowStyle::PUSH_BUTTON,
            secondary_title: "DetailPanel",
            secondary_tooltip: "Secondary screen panel",
            secondary_rect: LegacyRect {
                x: 756,
                y: 152,
                width: 300,
                height: 428,
            },
            secondary_status: WindowStatus::ENABLED | WindowStatus::SMOOTH_TEXT,
            secondary_style: GadgetWindowStyle::SCROLL_LISTBOX,
        },
    }
}
