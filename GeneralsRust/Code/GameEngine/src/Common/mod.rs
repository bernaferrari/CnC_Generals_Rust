// FILE: mod.rs (Common module)
// Author: Ported from C++
// Desc: Common subsystem module exports

pub mod System;
pub mod object_status;
pub mod message_stream;

// INI parsing system modules
pub mod ini;
pub mod ini_parsers;
pub mod ini_blocks;

// Re-export commonly used types from System
pub use System::{
    // Xfer system
    Xfer,
    XferMode,
    XferStatus,
    XferSave,
    XferLoad,
    Snapshot,
    // Save/Load system
    GameState,
    GameStateMap,
    SaveGameInfo,
    SaveFileType,
    SaveCode,
    SnapshotType,
    // Geometric types
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

// Re-export object status types
pub use object_status::{
    ObjectStatusMaskType,
    OBJECT_STATUS_SCRIPT_DISABLED,
    OBJECT_STATUS_SCRIPT_UNPOWERED,
    OBJECT_STATUS_SCRIPT_UNSELLABLE,
    OBJECT_STATUS_SCRIPT_UNSTEALTHED,
    OBJECT_STATUS_SCRIPT_TARGETABLE,
};

// Re-export message stream types
pub use message_stream::{
    GameMessage,
    GameMessageType,
    GameMessageArgument,
    GameMessageArgumentType,
    GameMessageArgumentDataType,
    GameMessageDisposition,
    GameMessageTranslator,
    GameMessageList,
    MessageStream,
    CommandList,
    TranslatorId,
    TRANSLATOR_ID_INVALID,
    build_region,
};

// Re-export INI parsing system
pub use ini::{
    INI,
    INILoadType,
    INIError,
    INIResult,
    INIException,
    FieldParse,
    MultiIniFieldParse,
    INIFieldParseProc,
    INIBlockParse,
    LookupListRec,
    // Types
    Real,
    Int,
    UnsignedInt,
    UnsignedShort,
    Short,
    UnsignedByte,
    Byte,
    Bool,
    // Conversion functions
    convert_duration_from_msecs_to_frames,
    convert_velocity_in_secs_to_frames,
    convert_acceleration_in_secs_to_frames,
    convert_angular_velocity_in_degrees_per_sec_to_rads_per_frame,
};

pub use ini_blocks::{
    BlockParse,
    get_type_table,
    find_block_parse,
};

pub use ini_parsers::*;
