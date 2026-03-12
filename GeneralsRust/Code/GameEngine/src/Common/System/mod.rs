// FILE: mod.rs (System module)
// Author: Ported from C++
// Desc: System-level subsystem exports including save/load and upgrade systems

pub mod SaveGame;
pub mod upgrade;

// Re-export from SaveGame
pub use SaveGame::{
    Xfer,
    XferMode,
    XferStatus,
    XferSave,
    XferLoad,
    Snapshot,
    GameState,
    GameStateMap,
    SaveGameInfo,
    SaveFileType,
    SaveCode,
    SnapshotType,
    Coord3D,
    ICoord3D,
    Region3D,
    IRegion3D,
    Coord2D,
    ICoord2D,
    Region2D,
    IRegion2D,
    RealRange,
    RGBColor,
    RGBAColorReal,
    RGBAColorInt,
};

// Re-export from upgrade
pub use upgrade::{
    // Constants
    UPGRADE_MAX_COUNT,
    NAMEKEY_INVALID,

    // Enums
    UpgradeStatusType,
    UpgradeType,
    VeterancyLevel,
    AcademyClassificationType,

    // Type aliases
    NameKeyType,

    // Core types
    UpgradeMaskType,
    Upgrade,
    UpgradeTemplate,
    UpgradeCenter,

    // Helper functions
    test_upgrade_mask,
    test_upgrade_mask_any,
    test_upgrade_mask_multi,
    upgrade_mask_any_set,
    clear_upgrade_mask,
    set_all_upgrade_mask_bits,
    flip_upgrade_mask,

    // Interfaces
    PlayerInterface,
    MoneyInterface,
    ImageCollectionInterface,
    InGameUIInterface,

    // Global functions
    init_upgrade_center,
    get_upgrade_center,

    // String constants
    UPGRADE_TYPE_NAMES,
    VETERANCY_NAMES,
};
