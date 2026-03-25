// FILE: mod.rs (System module)
// Author: Ported from C++
// Desc: System subsystem module exports

pub mod SaveGame;
pub mod xfer;
pub mod xfer_load;
pub mod xfer_save;

pub use xfer::{
    bit_clear,
    // Helper functions
    bit_set,
    bit_test,
    xfer_options,
    Coord2D,
    // Geometric types
    Coord3D,
    DrawableID,
    ICoord2D,
    ICoord3D,
    IRegion2D,
    IRegion3D,
    ObjectID,
    RGBAColorInt,
    RGBAColorReal,
    RGBColor,
    RealRange,
    Region2D,
    Region3D,
    Snapshot,
    Xfer,
    XferBlockSize,
    XferFilePos,
    XferMode,
    XferStatus,
    XferVersion,
};

pub use xfer_load::XferLoad;
pub use xfer_save::XferSave;

pub use SaveGame::{
    get_game_state, get_runtime_drawable_id_counter, get_runtime_object_id_counter,
    init_game_state, register_drawable_id_counter_hooks, register_object_id_counter_hooks,
    register_save_load_lifecycle_hooks, register_save_load_skirmish_hooks,
    set_runtime_drawable_id_counter, set_runtime_object_id_counter, AvailableGameInfo, GameState,
    GameStateMap, SaveCode, SaveDate, SaveFileType, SaveGameInfo, SaveLoadLayoutType, SnapshotType,
};
