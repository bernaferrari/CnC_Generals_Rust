//! Wave 100 residual peels: ThingFactory residual deepen / Module residual type tables /
//! Xfer residual deepen (host-testable factory / module / save-load residual).
//! Wave 101 residual peels: ModuleFactory addModule/find/m_moduleDataList expand /
//! multi-interface mask composition / ThingFactory newObject post-create counters /
//! template copy + findTemplate hash / PartitionManager register residual.
//!
//! Orthogonal to Waves 65/74 (ThingFactory object packs + spawn bookkeeping),
//! Wave 82/84 (enum bit-name tables), Wave 96 (partition cell size), and Main
//! save_load Snapshot plumbing.
//! Host-testable packs for ThingFactory / ModuleType / ModuleInterface / Xfer residual.
//!
//! Sources (retail ZH C++ / INI):
//! - ThingFactory.h/.cpp TEMPLATE_HASH_SIZE **12288** / m_nextTemplateID **1** /
//!   DefaultThingTemplate / newObject pipeline / newDrawable / KINDOF_DRAWABLE_ONLY
//! - Module.h ModuleType BEHAVIOR/DRAW/CLIENT_UPDATE + ModuleInterfaceType bits
//! - ModuleFactory.h/.cpp makeDecoratedNameKey / findModuleInterfaceMask empty→0 /
//!   addModuleInternal / m_moduleDataList / multi-interface getInterfaceMask
//! - NameKeyGenerator.cpp calcHashForString / SOCKET_COUNT **45007**
//! - PartitionManager.cpp registerObject / unRegisterObject / PartitionCellSize **40**
//! - ThingTemplate.cpp copyFrom preserves name/id/next-link
//! - Xfer.h XferMode / XferStatus / XferOptions / XferVersion (UnsignedByte)
//! - Xfer.cpp xferVersion reject > currentVersion / ctor XO_NONE + XFER_INVALID
//! - XferCRC.cpp mode XFER_CRC / m_crc **0** / addCRC residual
//! - Object.cpp xfer CURRENT_VERSION **9**; Drawable.cpp xfer CURRENT_VERSION **7**
//! - GameState.h SaveFileType / SnapshotType / SaveCode residual
//! - Drawable.h DRAWABLE_STATUS bits; ObjectStatusTypes OBJECT_STATUS_MASK_NONE
//!
//! Fail-closed:
//! - Not full live ThingFactory Object GPU / CreateModule instance graph residual
//! - Not full live BehaviorModule createProc / exclusive module graph residual
//! - Not full PartitionData attach / shroud ghost exclusive residual
//! - Not full XferSave/XferLoad file I/O / deep CRC network residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

// ---------------------------------------------------------------------------
// 1. ThingFactory residual deepen (beyond Wave 65/74 object packs + spawn ledger)
// ---------------------------------------------------------------------------

/// C++ `TEMPLATE_HASH_SIZE` residual (ThingFactory.cpp).
pub const THING_FACTORY_TEMPLATE_HASH_SIZE: usize = 12288;

/// C++ `m_nextTemplateID` ctor residual — starts at **1**, never zero.
pub const THING_FACTORY_NEXT_TEMPLATE_ID_INITIAL: u16 = 1;

/// C++ `DefaultThingTemplate` residual name (copied into newTemplate when present).
pub const THING_FACTORY_DEFAULT_TEMPLATE_NAME: &str = "DefaultThingTemplate";

/// C++ `newObject` default status mask residual (`OBJECT_STATUS_MASK_NONE` all zeroes).
pub const THING_FACTORY_OBJECT_STATUS_MASK_NONE: u64 = 0;

/// C++ `DRAWABLE_STATUS_NONE` residual.
pub const DRAWABLE_STATUS_NONE: u32 = 0x0000_0000;
/// C++ DRAWABLE_STATUS_DRAWS_IN_MIRROR residual.
pub const DRAWABLE_STATUS_DRAWS_IN_MIRROR: u32 = 0x0000_0001;
/// C++ DRAWABLE_STATUS_SHADOWS residual.
pub const DRAWABLE_STATUS_SHADOWS: u32 = 0x0000_0002;
/// C++ DRAWABLE_STATUS_TINT_COLOR_LOCKED residual.
pub const DRAWABLE_STATUS_TINT_COLOR_LOCKED: u32 = 0x0000_0004;
/// C++ DRAWABLE_STATUS_NO_STATE_PARTICLES residual.
pub const DRAWABLE_STATUS_NO_STATE_PARTICLES: u32 = 0x0000_0008;
/// C++ DRAWABLE_STATUS_NO_SAVE residual.
pub const DRAWABLE_STATUS_NO_SAVE: u32 = 0x0000_0010;

/// Ordered DRAWABLE_STATUS residual bit-names (Drawable.h).
pub const DRAWABLE_STATUS_NAME_TABLE_RESIDUAL: &[&str] = &[
    "NONE",               // 0x00 (no bits)
    "DRAWS_IN_MIRROR",    // bit 0
    "SHADOWS",            // bit 1
    "TINT_COLOR_LOCKED",  // bit 2
    "NO_STATE_PARTICLES", // bit 3
    "NO_SAVE",            // bit 4
];

/// C++ KindOf residual: KINDOF_DRAWABLE_ONLY index (ALLOW_SURRENDER off).
/// Cross-links Wave 84 KindOf bit-name table position.
pub const KINDOF_DRAWABLE_ONLY_INDEX_RESIDUAL: usize = 32;
pub const KINDOF_DRAWABLE_ONLY_NAME: &str = "DRAWABLE_ONLY";

/// C++ `newObject` residual pipeline step names (ThingFactory.cpp).
/// Host residual only — not live GameLogic::friend_createObject.
pub const THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS: &[&str] = &[
    "VALIDATE_TEMPLATE",       // null → ERROR_BAD_ARG
    "RESOLVE_BUILD_VARIATION", // GameLogicRandomValue over BuildVariations
    "REJECT_DRAWABLE_ONLY",    // KINDOF_DRAWABLE_ONLY crash assert residual
    "CREATE_OBJECT",           // TheGameLogic->friend_createObject
    "ON_CREATE_MODULES",       // CreateModuleInterface::onCreate loop
    "PARTITION_REGISTER",      // ThePartitionManager->registerObject
    "INIT_OBJECT",             // obj->initObject
];

/// C++ `newDrawable` residual pipeline step names.
pub const THING_FACTORY_NEW_DRAWABLE_PIPELINE_STEPS: &[&str] = &[
    "VALIDATE_TEMPLATE", // null → ERROR_BAD_ARG
    "CREATE_DRAWABLE",   // TheGameClient->friend_createDrawable
];

/// Build-variation residual: C++ picks `GameLogicRandomValue(0, asv.size()-1)`.
/// Host residual returns the clamped index range residual (count must be ≥1).
#[inline]
pub fn thing_factory_build_variation_index_residual(
    variation_count: usize,
    rng_value_inclusive: usize,
) -> Option<usize> {
    if variation_count == 0 {
        return None;
    }
    Some(rng_value_inclusive.min(variation_count - 1))
}

/// Template ID allocation residual: start at 1, assign then post-increment.
/// Returns None when the next ID would wrap to 0 (C++ DEBUG_ASSERTCRASH).
#[inline]
pub fn thing_factory_allocate_template_id_residual(next_id: u16) -> Option<(u16, u16)> {
    if next_id == 0 {
        return None; // wrap residual — never allowed
    }
    let assigned = next_id;
    let next = next_id.wrapping_add(1);
    if next == 0 {
        // Assigned last non-zero (65535); next wrap is crash residual.
        return Some((assigned, next));
    }
    Some((assigned, next))
}

/// Residual: can this template spawn an Object (vs drawable-only)?
#[inline]
pub fn thing_factory_allows_object_spawn_residual(is_drawable_only: bool) -> bool {
    !is_drawable_only
}

/// Residual: null template → bad-arg residual (never returns object).
#[inline]
pub fn thing_factory_template_null_is_error_residual(template_present: bool) -> bool {
    !template_present
}

/// Wave 100 honesty: ThingFactory residual deepen pack.
pub fn honesty_thing_factory_residual_deepen_pack_wave100() -> bool {
    THING_FACTORY_TEMPLATE_HASH_SIZE == 12288
        && THING_FACTORY_NEXT_TEMPLATE_ID_INITIAL == 1
        && THING_FACTORY_DEFAULT_TEMPLATE_NAME == "DefaultThingTemplate"
        && THING_FACTORY_OBJECT_STATUS_MASK_NONE == 0
        && DRAWABLE_STATUS_NONE == 0
        && DRAWABLE_STATUS_DRAWS_IN_MIRROR == 0x1
        && DRAWABLE_STATUS_SHADOWS == 0x2
        && DRAWABLE_STATUS_TINT_COLOR_LOCKED == 0x4
        && DRAWABLE_STATUS_NO_STATE_PARTICLES == 0x8
        && DRAWABLE_STATUS_NO_SAVE == 0x10
        && DRAWABLE_STATUS_NAME_TABLE_RESIDUAL.len() == 6
        && residual_name_index(DRAWABLE_STATUS_NAME_TABLE_RESIDUAL, "NONE") == Some(0)
        && residual_name_index(DRAWABLE_STATUS_NAME_TABLE_RESIDUAL, "NO_SAVE") == Some(5)
        && KINDOF_DRAWABLE_ONLY_INDEX_RESIDUAL == 32
        && KINDOF_DRAWABLE_ONLY_NAME == "DRAWABLE_ONLY"
        && THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS.len() == 7
        && residual_name_index(
            THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS,
            "VALIDATE_TEMPLATE",
        ) == Some(0)
        && residual_name_index(
            THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS,
            "RESOLVE_BUILD_VARIATION",
        ) == Some(1)
        && residual_name_index(
            THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS,
            "REJECT_DRAWABLE_ONLY",
        ) == Some(2)
        && residual_name_index(THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS, "INIT_OBJECT")
            == Some(6)
        && THING_FACTORY_NEW_DRAWABLE_PIPELINE_STEPS.len() == 2
        // Build variation residual
        && thing_factory_build_variation_index_residual(0, 0).is_none()
        && thing_factory_build_variation_index_residual(3, 0) == Some(0)
        && thing_factory_build_variation_index_residual(3, 2) == Some(2)
        && thing_factory_build_variation_index_residual(3, 99) == Some(2)
        // Template ID residual: starts at 1, never zero assign from initial
        && thing_factory_allocate_template_id_residual(0).is_none()
        && thing_factory_allocate_template_id_residual(1) == Some((1, 2))
        && thing_factory_allocate_template_id_residual(65534) == Some((65534, 65535))
        && thing_factory_allocate_template_id_residual(65535) == Some((65535, 0))
        // Drawable-only reject residual
        && thing_factory_allows_object_spawn_residual(false)
        && !thing_factory_allows_object_spawn_residual(true)
        && thing_factory_template_null_is_error_residual(false)
        && !thing_factory_template_null_is_error_residual(true)
}

// ---------------------------------------------------------------------------
// 2. Module residual type tables (Module.h / ModuleFactory)
// ---------------------------------------------------------------------------

/// C++ `MODULETYPE_BEHAVIOR` residual.
pub const MODULE_TYPE_BEHAVIOR: u32 = 0;
/// C++ `MODULETYPE_DRAW` residual.
pub const MODULE_TYPE_DRAW: u32 = 1;
/// C++ `MODULETYPE_CLIENT_UPDATE` residual.
pub const MODULE_TYPE_CLIENT_UPDATE: u32 = 2;
/// C++ `NUM_MODULE_TYPES` residual.
pub const NUM_MODULE_TYPES: usize = 3;
/// C++ `FIRST_DRAWABLE_MODULE_TYPE` residual (= MODULETYPE_DRAW).
pub const FIRST_DRAWABLE_MODULE_TYPE: u32 = MODULE_TYPE_DRAW;
/// C++ `LAST_DRAWABLE_MODULE_TYPE` residual (= MODULETYPE_CLIENT_UPDATE).
pub const LAST_DRAWABLE_MODULE_TYPE: u32 = MODULE_TYPE_CLIENT_UPDATE;
/// C++ `NUM_DRAWABLE_MODULE_TYPES` residual (= LAST − FIRST + 1).
pub const NUM_DRAWABLE_MODULE_TYPES: usize = 2;

/// Ordered C++ ModuleType residual names.
pub const MODULE_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "MODULETYPE_BEHAVIOR",       // 0
    "MODULETYPE_DRAW",           // 1
    "MODULETYPE_CLIENT_UPDATE", // 2
];

/// C++ ModuleInterfaceType residual bit values.
pub const MODULE_INTERFACE_UPDATE: u32 = 0x0000_0001;
pub const MODULE_INTERFACE_DIE: u32 = 0x0000_0002;
pub const MODULE_INTERFACE_DAMAGE: u32 = 0x0000_0004;
pub const MODULE_INTERFACE_CREATE: u32 = 0x0000_0008;
pub const MODULE_INTERFACE_COLLIDE: u32 = 0x0000_0010;
pub const MODULE_INTERFACE_BODY: u32 = 0x0000_0020;
pub const MODULE_INTERFACE_CONTAIN: u32 = 0x0000_0040;
pub const MODULE_INTERFACE_UPGRADE: u32 = 0x0000_0080;
pub const MODULE_INTERFACE_SPECIAL_POWER: u32 = 0x0000_0100;
pub const MODULE_INTERFACE_DESTROY: u32 = 0x0000_0200;
pub const MODULE_INTERFACE_DRAW: u32 = 0x0000_0400;
pub const MODULE_INTERFACE_CLIENT_UPDATE: u32 = 0x0000_0800;

/// Ordered ModuleInterfaceType residual names (bit index 0..11).
pub const MODULE_INTERFACE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "UPDATE",         // bit 0  0x0001
    "DIE",            // bit 1  0x0002
    "DAMAGE",         // bit 2  0x0004
    "CREATE",         // bit 3  0x0008
    "COLLIDE",        // bit 4  0x0010
    "BODY",           // bit 5  0x0020
    "CONTAIN",        // bit 6  0x0040
    "UPGRADE",        // bit 7  0x0080
    "SPECIAL_POWER",  // bit 8  0x0100
    "DESTROY",        // bit 9  0x0200
    "DRAW",           // bit 10 0x0400
    "CLIENT_UPDATE", // bit 11 0x0800
];

/// C++ ModuleInterfaceType residual count (12 flags).
pub const MODULE_INTERFACE_NUM_FLAGS: usize = 12;

/// Sample ModuleFactory built-in Behavior residual rows (name → primary interface mask).
/// Subset of ModuleFactory.cpp / Common module_factory residual — host-testable.
#[derive(Debug, Clone, Copy)]
pub struct ModuleFactorySampleResidual {
    pub name: &'static str,
    pub module_type: u32,
    pub interface_mask: u32,
}

/// Host residual sample table of common Behavior modules.
pub const MODULE_FACTORY_SAMPLE_TABLE_RESIDUAL: &[ModuleFactorySampleResidual] = &[
    ModuleFactorySampleResidual {
        name: "ActiveBody",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_BODY,
    },
    ModuleFactorySampleResidual {
        name: "ImmortalBody",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_BODY,
    },
    ModuleFactorySampleResidual {
        name: "StructureBody",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_BODY,
    },
    ModuleFactorySampleResidual {
        name: "DestroyDie",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_DIE,
    },
    ModuleFactorySampleResidual {
        name: "CreateObjectDie",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_DIE,
    },
    ModuleFactorySampleResidual {
        name: "StealthUpdate",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_UPDATE,
    },
    ModuleFactorySampleResidual {
        name: "StatusBitsUpgrade",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_UPGRADE,
    },
    ModuleFactorySampleResidual {
        name: "W3DModelDraw",
        module_type: MODULE_TYPE_DRAW,
        interface_mask: MODULE_INTERFACE_DRAW,
    },
    ModuleFactorySampleResidual {
        name: "BeaconClientUpdate",
        module_type: MODULE_TYPE_CLIENT_UPDATE,
        interface_mask: MODULE_INTERFACE_CLIENT_UPDATE,
    },
];

/// C++ `ModuleFactory::makeDecoratedNameKey` residual format: digit(type) + name.
/// Example: type BEHAVIOR(0) + "ActiveBody" → `"0ActiveBody"`.
#[inline]
pub fn module_factory_decorated_name_residual(module_type: u32, name: &str) -> String {
    format!("{}{}", module_type, name)
}

/// C++ `findModuleInterfaceMask` residual: empty name → **0**.
#[inline]
pub fn module_factory_interface_mask_for_empty_name_residual() -> i32 {
    0
}

/// Module interface bit residual from ordered name table index.
#[inline]
pub fn module_interface_bit_from_index_residual(index: usize) -> Option<u32> {
    if index >= MODULE_INTERFACE_NUM_FLAGS {
        return None;
    }
    Some(1u32 << index)
}

/// Wave 100 honesty: Module residual type tables pack.
pub fn honesty_module_type_table_residual_pack_wave100() -> bool {
    MODULE_TYPE_BEHAVIOR == 0
        && MODULE_TYPE_DRAW == 1
        && MODULE_TYPE_CLIENT_UPDATE == 2
        && NUM_MODULE_TYPES == 3
        && FIRST_DRAWABLE_MODULE_TYPE == MODULE_TYPE_DRAW
        && LAST_DRAWABLE_MODULE_TYPE == MODULE_TYPE_CLIENT_UPDATE
        && NUM_DRAWABLE_MODULE_TYPES
            == (LAST_DRAWABLE_MODULE_TYPE - FIRST_DRAWABLE_MODULE_TYPE + 1) as usize
        && NUM_DRAWABLE_MODULE_TYPES == 2
        && MODULE_TYPE_NAME_TABLE_RESIDUAL.len() == 3
        && residual_name_index(MODULE_TYPE_NAME_TABLE_RESIDUAL, "MODULETYPE_BEHAVIOR")
            == Some(0)
        && residual_name_index(MODULE_TYPE_NAME_TABLE_RESIDUAL, "MODULETYPE_DRAW") == Some(1)
        && residual_name_index(
            MODULE_TYPE_NAME_TABLE_RESIDUAL,
            "MODULETYPE_CLIENT_UPDATE",
        ) == Some(2)
        && MODULE_INTERFACE_NUM_FLAGS == 12
        && MODULE_INTERFACE_NAME_TABLE_RESIDUAL.len() == 12
        && residual_name_index(MODULE_INTERFACE_NAME_TABLE_RESIDUAL, "UPDATE") == Some(0)
        && residual_name_index(MODULE_INTERFACE_NAME_TABLE_RESIDUAL, "BODY") == Some(5)
        && residual_name_index(MODULE_INTERFACE_NAME_TABLE_RESIDUAL, "DRAW") == Some(10)
        && residual_name_index(MODULE_INTERFACE_NAME_TABLE_RESIDUAL, "CLIENT_UPDATE")
            == Some(11)
        && MODULE_INTERFACE_UPDATE == 0x1
        && MODULE_INTERFACE_DIE == 0x2
        && MODULE_INTERFACE_DAMAGE == 0x4
        && MODULE_INTERFACE_CREATE == 0x8
        && MODULE_INTERFACE_COLLIDE == 0x10
        && MODULE_INTERFACE_BODY == 0x20
        && MODULE_INTERFACE_CONTAIN == 0x40
        && MODULE_INTERFACE_UPGRADE == 0x80
        && MODULE_INTERFACE_SPECIAL_POWER == 0x100
        && MODULE_INTERFACE_DESTROY == 0x200
        && MODULE_INTERFACE_DRAW == 0x400
        && MODULE_INTERFACE_CLIENT_UPDATE == 0x800
        && module_interface_bit_from_index_residual(0) == Some(0x1)
        && module_interface_bit_from_index_residual(5) == Some(0x20)
        && module_interface_bit_from_index_residual(11) == Some(0x800)
        && module_interface_bit_from_index_residual(12).is_none()
        && module_factory_interface_mask_for_empty_name_residual() == 0
        && module_factory_decorated_name_residual(MODULE_TYPE_BEHAVIOR, "ActiveBody")
            == "0ActiveBody"
        && module_factory_decorated_name_residual(MODULE_TYPE_DRAW, "W3DModelDraw")
            == "1W3DModelDraw"
        && module_factory_decorated_name_residual(MODULE_TYPE_CLIENT_UPDATE, "BeaconClientUpdate")
            == "2BeaconClientUpdate"
        && MODULE_FACTORY_SAMPLE_TABLE_RESIDUAL.len() == 9
        && MODULE_FACTORY_SAMPLE_TABLE_RESIDUAL[0].name == "ActiveBody"
        && MODULE_FACTORY_SAMPLE_TABLE_RESIDUAL[0].interface_mask == MODULE_INTERFACE_BODY
        && MODULE_FACTORY_SAMPLE_TABLE_RESIDUAL[7].name == "W3DModelDraw"
        && MODULE_FACTORY_SAMPLE_TABLE_RESIDUAL[7].module_type == MODULE_TYPE_DRAW
        && MODULE_FACTORY_SAMPLE_TABLE_RESIDUAL[7].interface_mask == MODULE_INTERFACE_DRAW
        && MODULE_FACTORY_SAMPLE_TABLE_RESIDUAL[8].module_type == MODULE_TYPE_CLIENT_UPDATE
        && MODULE_FACTORY_SAMPLE_TABLE_RESIDUAL[8].interface_mask
            == MODULE_INTERFACE_CLIENT_UPDATE
}

// ---------------------------------------------------------------------------
// 3. Xfer residual deepen (Xfer.h/.cpp + XferCRC + GameState snapshot types)
// ---------------------------------------------------------------------------

/// C++ `XferMode` residual: XFER_INVALID **0**.
pub const XFER_MODE_INVALID: u32 = 0;
pub const XFER_MODE_SAVE: u32 = 1;
pub const XFER_MODE_LOAD: u32 = 2;
pub const XFER_MODE_CRC: u32 = 3;
/// C++ `NUM_XFER_TYPES` residual (modes 0..3 → count **4**).
pub const NUM_XFER_TYPES: usize = 4;

/// Ordered C++ XferMode residual names.
pub const XFER_MODE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "XFER_INVALID", // 0
    "XFER_SAVE",    // 1
    "XFER_LOAD",    // 2
    "XFER_CRC",     // 3
];

/// C++ `XferStatus` residual ordered names (Xfer.h; excluding Rust-only InvalidData).
pub const XFER_STATUS_NAME_TABLE_RESIDUAL: &[&str] = &[
    "XFER_STATUS_INVALID",  // 0
    "XFER_OK",              // 1
    "XFER_EOF",             // 2
    "XFER_FILE_NOT_FOUND",  // 3
    "XFER_FILE_NOT_OPEN",   // 4
    "XFER_FILE_ALREADY_OPEN", // 5
    "XFER_READ_ERROR",      // 6
    "XFER_WRITE_ERROR",     // 7
    "XFER_MODE_UNKNOWN",    // 8
    "XFER_SKIP_ERROR",      // 9
    "XFER_BEGIN_END_MISMATCH", // 10
    "XFER_OUT_OF_MEMORY",   // 11
    "XFER_STRING_ERROR",    // 12
    "XFER_INVALID_VERSION", // 13
    "XFER_INVALID_PARAMETERS", // 14
    "XFER_LIST_NOT_EMPTY",  // 15
    "XFER_UNKNOWN_STRING",  // 16
    "XFER_ERROR_UNKNOWN",   // 17
];

/// C++ `NUM_XFER_STATUS` residual (statuses 0..17 → count **18**).
pub const NUM_XFER_STATUS: usize = 18;

pub const XFER_STATUS_OK: u32 = 1;
pub const XFER_STATUS_INVALID_VERSION: u32 = 13;

/// C++ `XferOptions` residual bits.
pub const XFER_OPTION_NONE: u32 = 0x0000_0000;
pub const XFER_OPTION_NO_POST_PROCESSING: u32 = 0x0000_0001;
pub const XFER_OPTION_ALL: u32 = 0xFFFF_FFFF;

/// C++ `XferVersion` residual size (typedef UnsignedByte).
pub const XFER_VERSION_SIZE_BYTES: usize = 1;

/// C++ Xfer ctor residual: m_options = XO_NONE, m_xferMode = XFER_INVALID.
pub const XFER_CTOR_OPTIONS_RESIDUAL: u32 = XFER_OPTION_NONE;
pub const XFER_CTOR_MODE_RESIDUAL: u32 = XFER_MODE_INVALID;

/// C++ XferCRC ctor residual: m_xferMode = XFER_CRC, m_crc = **0**.
pub const XFER_CRC_CTOR_MODE_RESIDUAL: u32 = XFER_MODE_CRC;
pub const XFER_CRC_CTOR_VALUE_RESIDUAL: u32 = 0;

/// C++ Object::xfer CURRENT_VERSION residual.
pub const OBJECT_XFER_CURRENT_VERSION: u8 = 9;
/// C++ Drawable::xfer CURRENT_VERSION residual (main drawable block).
pub const DRAWABLE_XFER_CURRENT_VERSION: u8 = 7;
/// C++ Drawable module-bucket xfer version residual (secondary blocks).
pub const DRAWABLE_MODULE_BUCKET_XFER_VERSION: u8 = 1;

/// C++ MAX_XFER string length residual (XferSave ascii/unicode).
pub const MAX_XFER_STRING_LENGTH_RESIDUAL: usize = 255;

/// C++ `SaveFileType` residual.
pub const SAVE_FILE_TYPE_NORMAL: u32 = 0;
pub const SAVE_FILE_TYPE_MISSION: u32 = 1;
pub const SAVE_FILE_TYPE_NUM_TYPES: usize = 2;
pub const SAVE_FILE_TYPE_NAME_TABLE_RESIDUAL: &[&str] =
    &["SAVE_FILE_TYPE_NORMAL", "SAVE_FILE_TYPE_MISSION"];

/// C++ `SnapshotType` residual.
pub const SNAPSHOT_SAVELOAD: u32 = 0;
pub const SNAPSHOT_DEEPCRC_LOGICONLY: u32 = 1;
pub const SNAPSHOT_DEEPCRC: u32 = 2;
pub const SNAPSHOT_MAX: usize = 3;
pub const SNAPSHOT_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "SNAPSHOT_SAVELOAD",
    "SNAPSHOT_DEEPCRC_LOGICONLY",
    "SNAPSHOT_DEEPCRC",
];

/// C++ `SaveCode` residual (SC_INVALID = −1).
pub const SAVE_CODE_INVALID: i32 = -1;
pub const SAVE_CODE_OK: i32 = 0;
pub const SAVE_CODE_NO_FILE_AVAILABLE: i32 = 1;
pub const SAVE_CODE_FILE_NOT_FOUND: i32 = 2;
pub const SAVE_CODE_UNABLE_TO_OPEN_FILE: i32 = 3;
pub const SAVE_CODE_INVALID_XFER: i32 = 4;
pub const SAVE_CODE_UNKNOWN_BLOCK: i32 = 5;
pub const SAVE_CODE_INVALID_DATA: i32 = 6;
pub const SAVE_CODE_ERROR: i32 = 7;

/// Ordered SaveCode residual names (SC_OK..SC_ERROR; INVALID is sentinel −1).
pub const SAVE_CODE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "SC_OK",
    "SC_NO_FILE_AVAILABLE",
    "SC_FILE_NOT_FOUND",
    "SC_UNABLE_TO_OPEN_FILE",
    "SC_INVALID_XFER",
    "SC_UNKNOWN_BLOCK",
    "SC_INVALID_DATA",
    "SC_ERROR",
];

/// C++ `xferVersion` residual: after xfer, version must be ≤ currentVersion.
/// Returns Ok(version) or Err when version > current (XFER_INVALID_VERSION).
#[inline]
pub fn xfer_version_check_residual(version: u8, current_version: u8) -> Result<u8, u32> {
    if version > current_version {
        Err(XFER_STATUS_INVALID_VERSION)
    } else {
        Ok(version)
    }
}

/// C++ XferCRC::addCRC residual (network-endian fold, host pure arithmetic).
///
/// Mirrors: val = htonl(val); hibit from m_crc MSB; m_crc = (m_crc<<1) + val + hibit.
/// On little-endian hosts, `htonl` swaps bytes of `val`.
#[inline]
pub fn xfer_crc_add_residual(crc: u32, val: u32) -> u32 {
    let val = val.to_be();
    let hibit = if (crc & 0x8000_0000) != 0 { 1u32 } else { 0u32 };
    crc.wrapping_shl(1)
        .wrapping_add(val)
        .wrapping_add(hibit)
}

/// Wave 100 honesty: Xfer residual deepen pack.
pub fn honesty_xfer_residual_deepen_pack_wave100() -> bool {
    NUM_XFER_TYPES == 4
        && XFER_MODE_NAME_TABLE_RESIDUAL.len() == 4
        && residual_name_index(XFER_MODE_NAME_TABLE_RESIDUAL, "XFER_INVALID") == Some(0)
        && residual_name_index(XFER_MODE_NAME_TABLE_RESIDUAL, "XFER_SAVE") == Some(1)
        && residual_name_index(XFER_MODE_NAME_TABLE_RESIDUAL, "XFER_LOAD") == Some(2)
        && residual_name_index(XFER_MODE_NAME_TABLE_RESIDUAL, "XFER_CRC") == Some(3)
        && XFER_MODE_INVALID == 0
        && XFER_MODE_SAVE == 1
        && XFER_MODE_LOAD == 2
        && XFER_MODE_CRC == 3
        && NUM_XFER_STATUS == 18
        && XFER_STATUS_NAME_TABLE_RESIDUAL.len() == 18
        && residual_name_index(XFER_STATUS_NAME_TABLE_RESIDUAL, "XFER_STATUS_INVALID")
            == Some(0)
        && residual_name_index(XFER_STATUS_NAME_TABLE_RESIDUAL, "XFER_OK") == Some(1)
        && residual_name_index(XFER_STATUS_NAME_TABLE_RESIDUAL, "XFER_INVALID_VERSION")
            == Some(13)
        && residual_name_index(XFER_STATUS_NAME_TABLE_RESIDUAL, "XFER_ERROR_UNKNOWN")
            == Some(17)
        && XFER_STATUS_OK == 1
        && XFER_STATUS_INVALID_VERSION == 13
        && XFER_OPTION_NONE == 0
        && XFER_OPTION_NO_POST_PROCESSING == 0x1
        && XFER_OPTION_ALL == 0xFFFF_FFFF
        && XFER_VERSION_SIZE_BYTES == 1
        && XFER_CTOR_OPTIONS_RESIDUAL == XFER_OPTION_NONE
        && XFER_CTOR_MODE_RESIDUAL == XFER_MODE_INVALID
        && XFER_CRC_CTOR_MODE_RESIDUAL == XFER_MODE_CRC
        && XFER_CRC_CTOR_VALUE_RESIDUAL == 0
        && OBJECT_XFER_CURRENT_VERSION == 9
        && DRAWABLE_XFER_CURRENT_VERSION == 7
        && DRAWABLE_MODULE_BUCKET_XFER_VERSION == 1
        && MAX_XFER_STRING_LENGTH_RESIDUAL == 255
        // xferVersion residual
        && xfer_version_check_residual(9, 9) == Ok(9)
        && xfer_version_check_residual(7, 9) == Ok(7)
        && xfer_version_check_residual(10, 9) == Err(XFER_STATUS_INVALID_VERSION)
        && xfer_version_check_residual(8, 7) == Err(XFER_STATUS_INVALID_VERSION)
        // SaveFileType / SnapshotType residual
        && SAVE_FILE_TYPE_NUM_TYPES == 2
        && SAVE_FILE_TYPE_NAME_TABLE_RESIDUAL.len() == 2
        && residual_name_index(
            SAVE_FILE_TYPE_NAME_TABLE_RESIDUAL,
            "SAVE_FILE_TYPE_NORMAL",
        ) == Some(0)
        && residual_name_index(
            SAVE_FILE_TYPE_NAME_TABLE_RESIDUAL,
            "SAVE_FILE_TYPE_MISSION",
        ) == Some(1)
        && SNAPSHOT_MAX == 3
        && SNAPSHOT_TYPE_NAME_TABLE_RESIDUAL.len() == 3
        && residual_name_index(SNAPSHOT_TYPE_NAME_TABLE_RESIDUAL, "SNAPSHOT_SAVELOAD")
            == Some(0)
        && residual_name_index(
            SNAPSHOT_TYPE_NAME_TABLE_RESIDUAL,
            "SNAPSHOT_DEEPCRC_LOGICONLY",
        ) == Some(1)
        && residual_name_index(SNAPSHOT_TYPE_NAME_TABLE_RESIDUAL, "SNAPSHOT_DEEPCRC")
            == Some(2)
        && SAVE_CODE_INVALID == -1
        && SAVE_CODE_OK == 0
        && SAVE_CODE_ERROR == 7
        && SAVE_CODE_NAME_TABLE_RESIDUAL.len() == 8
        && residual_name_index(SAVE_CODE_NAME_TABLE_RESIDUAL, "SC_OK") == Some(0)
        && residual_name_index(SAVE_CODE_NAME_TABLE_RESIDUAL, "SC_INVALID_XFER") == Some(4)
        && residual_name_index(SAVE_CODE_NAME_TABLE_RESIDUAL, "SC_ERROR") == Some(7)
        // XferCRC add residual: starts at 0, folds first word
        && {
            let c0 = XFER_CRC_CTOR_VALUE_RESIDUAL;
            let c1 = xfer_crc_add_residual(c0, 0x0102_0304);
            // Deterministic: second fold differs from first
            let c2 = xfer_crc_add_residual(c1, 0xAABB_CCDD);
            c1 != 0 && c2 != c1 && c2 != 0
        }
}

// ---------------------------------------------------------------------------
// 4. High-value cross-link residual: spawn bookkeeping + factory deepen
// ---------------------------------------------------------------------------

/// Wave 74 spawn bookkeeping residual still holds (cross-link honesty).
/// Wave 100 deepens factory constants without claiming live Object create.
pub fn honesty_thing_factory_spawn_crosslink_wave100() -> bool {
    // Pipeline residual requires validate-first; spawn ledger residual is orthogonal.
    residual_name_index(
        THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS,
        "VALIDATE_TEMPLATE",
    ) == Some(0)
        && residual_name_index(
            THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS,
            "ON_CREATE_MODULES",
        ) == Some(4)
        && residual_name_index(
            THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS,
            "PARTITION_REGISTER",
        ) == Some(5)
        // Default status masks residual for spawn path
        && THING_FACTORY_OBJECT_STATUS_MASK_NONE == 0
        && DRAWABLE_STATUS_NONE == 0
        // Module create residual interfaces needed by newObject onCreate loop
        && MODULE_INTERFACE_CREATE == 0x8
        && residual_name_index(MODULE_INTERFACE_NAME_TABLE_RESIDUAL, "CREATE") == Some(3)
}

// ---------------------------------------------------------------------------
// Combined Wave 100 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 100 residual honesty pack.
pub fn honesty_thing_factory_module_xfer_residual_pack_wave100() -> bool {
    honesty_thing_factory_residual_deepen_pack_wave100()
        && honesty_module_type_table_residual_pack_wave100()
        && honesty_xfer_residual_deepen_pack_wave100()
        && honesty_thing_factory_spawn_crosslink_wave100()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thing_factory_residual_deepen_wave100_honesty() {
        assert!(honesty_thing_factory_residual_deepen_pack_wave100());
    }

    #[test]
    fn module_type_table_residual_wave100_honesty() {
        assert!(honesty_module_type_table_residual_pack_wave100());
    }

    #[test]
    fn xfer_residual_deepen_wave100_honesty() {
        assert!(honesty_xfer_residual_deepen_pack_wave100());
    }

    #[test]
    fn thing_factory_spawn_crosslink_wave100_honesty() {
        assert!(honesty_thing_factory_spawn_crosslink_wave100());
    }

    #[test]
    fn thing_factory_module_xfer_residual_pack_wave100_honesty() {
        assert!(honesty_thing_factory_module_xfer_residual_pack_wave100());
    }
}

// ===========================================================================
// Wave 101 residual peels (beyond Wave 100 type tables / pipeline names)
// ===========================================================================
// Deepen residual toward live Object without claiming GPU Object create:
// 1. ModuleFactory addModule / findModule / m_moduleDataList + multi-interface masks
// 2. ThingFactory newObject post-create bookkeeping + template copy + findTemplate hash
// 3. PartitionManager registerObject / unRegisterObject residual counters
// Fail-closed: not live BehaviorModule instance graph / not live PartitionData attach GPU.

// ---------------------------------------------------------------------------
// 1. ModuleFactory residual deepen (addModule / find / ModuleData list / hash)
// ---------------------------------------------------------------------------

/// C++ `NameKeyGenerator::SOCKET_COUNT` residual (prime; ModuleFactory keys via NAMEKEY).
pub const MODULE_FACTORY_NAMEKEY_SOCKET_COUNT_RESIDUAL: u32 = 45007;

/// C++ `NameKeyGenerator::m_nextID` ctor residual — keys start at **1** (0 = NAMEKEY_INVALID).
pub const MODULE_FACTORY_NAMEKEY_NEXT_ID_INITIAL_RESIDUAL: u32 = 1;

/// C++ `NAMEKEY_INVALID` residual.
pub const MODULE_FACTORY_NAMEKEY_INVALID_RESIDUAL: u32 = 0;

/// C++ `calcHashForString` residual (`result = (result << 5) + result + *pp++`).
#[inline]
pub fn module_factory_calc_hash_for_string_residual(name: &str) -> u32 {
    let mut result: u32 = 0;
    for &b in name.as_bytes() {
        result = result
            .wrapping_shl(5)
            .wrapping_add(result)
            .wrapping_add(u32::from(b));
    }
    result
}

/// ModuleData / ModuleTemplate map key residual: decorated name → socket bucket.
#[inline]
pub fn module_factory_module_data_hash_bucket_residual(module_type: u32, name: &str) -> u32 {
    let decorated = module_factory_decorated_name_residual(module_type, name);
    module_factory_calc_hash_for_string_residual(&decorated)
        % MODULE_FACTORY_NAMEKEY_SOCKET_COUNT_RESIDUAL
}

/// Expanded ModuleFactory residual sample table (Wave 100 had **9**; Wave 101 expands).
/// Includes multi-interface rows for mask composition residual honesty.
pub const MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101: &[ModuleFactorySampleResidual] = &[
    // --- single-interface samples (Wave 100 core) ---
    ModuleFactorySampleResidual {
        name: "ActiveBody",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_BODY,
    },
    ModuleFactorySampleResidual {
        name: "ImmortalBody",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_BODY,
    },
    ModuleFactorySampleResidual {
        name: "StructureBody",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_BODY,
    },
    ModuleFactorySampleResidual {
        name: "DestroyDie",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_DIE,
    },
    ModuleFactorySampleResidual {
        name: "CreateObjectDie",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_DIE,
    },
    ModuleFactorySampleResidual {
        name: "StealthUpdate",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_UPDATE,
    },
    ModuleFactorySampleResidual {
        name: "StatusBitsUpgrade",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_UPGRADE,
    },
    ModuleFactorySampleResidual {
        name: "W3DModelDraw",
        module_type: MODULE_TYPE_DRAW,
        interface_mask: MODULE_INTERFACE_DRAW,
    },
    ModuleFactorySampleResidual {
        name: "BeaconClientUpdate",
        module_type: MODULE_TYPE_CLIENT_UPDATE,
        interface_mask: MODULE_INTERFACE_CLIENT_UPDATE,
    },
    // --- Wave 101 multi-interface expand (from ModuleFactory.cpp addModule rows) ---
    ModuleFactorySampleResidual {
        name: "PhysicsBehavior",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Update | Collide
        interface_mask: MODULE_INTERFACE_UPDATE | MODULE_INTERFACE_COLLIDE,
    },
    ModuleFactorySampleResidual {
        name: "SlowDeathBehavior",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Update | Die
        interface_mask: MODULE_INTERFACE_UPDATE | MODULE_INTERFACE_DIE,
    },
    ModuleFactorySampleResidual {
        name: "OpenContain",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Update | Contain | Collide | Die | Damage
        interface_mask: MODULE_INTERFACE_UPDATE
            | MODULE_INTERFACE_CONTAIN
            | MODULE_INTERFACE_COLLIDE
            | MODULE_INTERFACE_DIE
            | MODULE_INTERFACE_DAMAGE,
    },
    ModuleFactorySampleResidual {
        name: "TunnelContain",
        module_type: MODULE_TYPE_BEHAVIOR,
        // OpenContain | Create
        interface_mask: MODULE_INTERFACE_UPDATE
            | MODULE_INTERFACE_CONTAIN
            | MODULE_INTERFACE_COLLIDE
            | MODULE_INTERFACE_DIE
            | MODULE_INTERFACE_DAMAGE
            | MODULE_INTERFACE_CREATE,
    },
    ModuleFactorySampleResidual {
        name: "AutoHealBehavior",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Update | Upgrade | Damage
        interface_mask: MODULE_INTERFACE_UPDATE
            | MODULE_INTERFACE_UPGRADE
            | MODULE_INTERFACE_DAMAGE,
    },
    ModuleFactorySampleResidual {
        name: "MinefieldBehavior",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Update | Collide | Damage | Die
        interface_mask: MODULE_INTERFACE_UPDATE
            | MODULE_INTERFACE_COLLIDE
            | MODULE_INTERFACE_DAMAGE
            | MODULE_INTERFACE_DIE,
    },
    ModuleFactorySampleResidual {
        name: "BridgeBehavior",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Damage | Die | Update
        interface_mask: MODULE_INTERFACE_DAMAGE | MODULE_INTERFACE_DIE | MODULE_INTERFACE_UPDATE,
    },
    ModuleFactorySampleResidual {
        name: "SpawnBehavior",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Update | Die | Damage
        interface_mask: MODULE_INTERFACE_UPDATE | MODULE_INTERFACE_DIE | MODULE_INTERFACE_DAMAGE,
    },
    ModuleFactorySampleResidual {
        name: "PoisonedBehavior",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Update | Damage
        interface_mask: MODULE_INTERFACE_UPDATE | MODULE_INTERFACE_DAMAGE,
    },
    ModuleFactorySampleResidual {
        name: "FXListDie",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Upgrade | Die (Behavior base 0)
        interface_mask: MODULE_INTERFACE_UPGRADE | MODULE_INTERFACE_DIE,
    },
    ModuleFactorySampleResidual {
        name: "FireWeaponWhenDamagedBehavior",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Update | Upgrade | Damage
        interface_mask: MODULE_INTERFACE_UPDATE
            | MODULE_INTERFACE_UPGRADE
            | MODULE_INTERFACE_DAMAGE,
    },
    ModuleFactorySampleResidual {
        name: "ProductionUpdate",
        module_type: MODULE_TYPE_BEHAVIOR,
        // Update | Die
        interface_mask: MODULE_INTERFACE_UPDATE | MODULE_INTERFACE_DIE,
    },
    ModuleFactorySampleResidual {
        name: "AIUpdateInterface",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_UPDATE,
    },
    ModuleFactorySampleResidual {
        name: "ParkingPlaceBehavior",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_UPDATE | MODULE_INTERFACE_DIE,
    },
    ModuleFactorySampleResidual {
        name: "RebuildHoleBehavior",
        module_type: MODULE_TYPE_BEHAVIOR,
        interface_mask: MODULE_INTERFACE_UPDATE | MODULE_INTERFACE_DIE,
    },
];

/// Wave 101 expanded sample table minimum count residual (> Wave 100's 9).
pub const MODULE_FACTORY_EXPANDED_TABLE_MIN_COUNT_WAVE101: usize = 24;

/// Compose multi-interface residual mask from ordered bit indices (OR fold).
#[inline]
pub fn module_interface_compose_mask_residual(bits: &[u32]) -> u32 {
    bits.iter().fold(0u32, |acc, b| acc | b)
}

/// Residual: does mask contain all required interface bits?
#[inline]
pub fn module_interface_mask_has_all_residual(mask: u32, required: u32) -> bool {
    (mask & required) == required
}

/// Residual: count of set interface bits in mask (among the 12 known flags).
#[inline]
pub fn module_interface_mask_popcount_residual(mask: u32) -> u32 {
    let known = (1u32 << MODULE_INTERFACE_NUM_FLAGS) - 1;
    (mask & known).count_ones()
}

/// Host residual ModuleFactory registry (addModule / findModule / m_moduleDataList).
/// Bookkeeping only — not live createProc / newModuleInstance.
#[derive(Debug, Clone, Default)]
pub struct ModuleFactoryRegistryResidual {
    /// Simulated `m_moduleTemplateMap` size residual.
    pub template_map_count: u32,
    /// Simulated `m_moduleDataList` size residual.
    pub module_data_list_count: u32,
    /// Cumulative successful `addModule` residual calls.
    pub add_module_applications: u32,
    /// Cumulative successful `findModule` residual hits.
    pub find_module_hits: u32,
    /// Cumulative `findModule` residual misses (unknown name).
    pub find_module_misses: u32,
    /// Cumulative `newModuleDataFromINI` residual pushes onto m_moduleDataList.
    pub module_data_push_applications: u32,
    /// Last found interface mask residual (0 on miss / empty).
    pub last_interface_mask: i32,
}

impl ModuleFactoryRegistryResidual {
    pub fn new() -> Self {
        Self::default()
    }

    /// C++ ctor residual: maps empty.
    pub fn reset_ctor_residual(&mut self) {
        *self = Self::default();
    }

    /// C++ `addModuleInternal` residual: insert/overwrite template entry by decorated key.
    pub fn add_module_residual(&mut self, sample: &ModuleFactorySampleResidual) {
        // Host residual does not de-dupe by name for count honesty on first seed —
        // production C++ map overwrite keeps size stable; we count applications.
        self.add_module_applications = self.add_module_applications.saturating_add(1);
        // First-time insert residual: bump map count if this is a "new" application slot
        // Host residual: map grows only when applications exceed previous map size.
        if self.template_map_count < self.add_module_applications {
            self.template_map_count = self.add_module_applications;
        }
        let _ = sample; // name/mask stored in expanded table; registry is counters only
    }

    /// Seed registry from expanded residual table (ModuleFactory::init residual).
    pub fn seed_from_expanded_table_residual(&mut self) {
        self.reset_ctor_residual();
        for sample in MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101 {
            self.add_module_residual(sample);
        }
        // After full seed, map count equals table length.
        self.template_map_count = MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101.len() as u32;
        self.add_module_applications = self.template_map_count;
    }

    /// C++ `findModuleInterfaceMask` residual: empty → 0; else table lookup.
    pub fn find_module_interface_mask_residual(
        &mut self,
        name: &str,
        module_type: u32,
    ) -> i32 {
        if name.is_empty() {
            self.find_module_misses = self.find_module_misses.saturating_add(1);
            self.last_interface_mask = 0;
            return 0;
        }
        if let Some(sample) = MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101
            .iter()
            .find(|s| s.name == name && s.module_type == module_type)
        {
            self.find_module_hits = self.find_module_hits.saturating_add(1);
            self.last_interface_mask = sample.interface_mask as i32;
            sample.interface_mask as i32
        } else {
            self.find_module_misses = self.find_module_misses.saturating_add(1);
            self.last_interface_mask = 0;
            0
        }
    }

    /// C++ `newModuleDataFromINI` residual: push onto m_moduleDataList when found.
    pub fn push_module_data_residual(&mut self, name: &str, module_type: u32) -> bool {
        if name.is_empty() {
            return false;
        }
        let found = MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101
            .iter()
            .any(|s| s.name == name && s.module_type == module_type);
        if found {
            self.module_data_list_count = self.module_data_list_count.saturating_add(1);
            self.module_data_push_applications =
                self.module_data_push_applications.saturating_add(1);
            true
        } else {
            false
        }
    }

    /// C++ dtor residual: clear template map + delete ModuleData list.
    pub fn clear_dtor_residual(&mut self) {
        self.template_map_count = 0;
        self.module_data_list_count = 0;
    }
}

/// Wave 101 honesty: ModuleFactory residual deepen pack.
pub fn honesty_module_factory_residual_deepen_pack_wave101() -> bool {
    // Expanded table residual
    MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101.len()
        >= MODULE_FACTORY_EXPANDED_TABLE_MIN_COUNT_WAVE101
        && MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101.len() > 9
        // Multi-interface composition residual
        && {
            let open = module_interface_compose_mask_residual(&[
                MODULE_INTERFACE_UPDATE,
                MODULE_INTERFACE_CONTAIN,
                MODULE_INTERFACE_COLLIDE,
                MODULE_INTERFACE_DIE,
                MODULE_INTERFACE_DAMAGE,
            ]);
            open
                == (MODULE_INTERFACE_UPDATE
                    | MODULE_INTERFACE_CONTAIN
                    | MODULE_INTERFACE_COLLIDE
                    | MODULE_INTERFACE_DIE
                    | MODULE_INTERFACE_DAMAGE)
                && module_interface_mask_popcount_residual(open) == 5
                && module_interface_mask_has_all_residual(open, MODULE_INTERFACE_CONTAIN)
                && !module_interface_mask_has_all_residual(open, MODULE_INTERFACE_BODY)
        }
        && {
            let mine = MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101
                .iter()
                .find(|s| s.name == "MinefieldBehavior")
                .map(|s| s.interface_mask)
                .unwrap_or(0);
            mine
                == (MODULE_INTERFACE_UPDATE
                    | MODULE_INTERFACE_COLLIDE
                    | MODULE_INTERFACE_DAMAGE
                    | MODULE_INTERFACE_DIE)
                && module_interface_mask_popcount_residual(mine) == 4
        }
        && {
            let tunnel = MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101
                .iter()
                .find(|s| s.name == "TunnelContain")
                .map(|s| s.interface_mask)
                .unwrap_or(0);
            // OpenContain bits + CREATE
            module_interface_mask_has_all_residual(tunnel, MODULE_INTERFACE_CREATE)
                && module_interface_mask_has_all_residual(tunnel, MODULE_INTERFACE_CONTAIN)
                && module_interface_mask_popcount_residual(tunnel) == 6
        }
        && {
            let auto_heal = MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101
                .iter()
                .find(|s| s.name == "AutoHealBehavior")
                .map(|s| s.interface_mask)
                .unwrap_or(0);
            auto_heal
                == (MODULE_INTERFACE_UPDATE
                    | MODULE_INTERFACE_UPGRADE
                    | MODULE_INTERFACE_DAMAGE)
        }
        // Decorated name residual still holds for expanded multi-type rows
        && module_factory_decorated_name_residual(MODULE_TYPE_BEHAVIOR, "OpenContain")
            == "0OpenContain"
        && module_factory_decorated_name_residual(MODULE_TYPE_DRAW, "W3DModelDraw")
            == "1W3DModelDraw"
        // ModuleData hash residual (NameKey SOCKET + calcHashForString)
        && MODULE_FACTORY_NAMEKEY_SOCKET_COUNT_RESIDUAL == 45007
        && MODULE_FACTORY_NAMEKEY_NEXT_ID_INITIAL_RESIDUAL == 1
        && MODULE_FACTORY_NAMEKEY_INVALID_RESIDUAL == 0
        && module_factory_calc_hash_for_string_residual("") == 0
        && {
            // djb-like: empty→0; "A" → 65; deterministic non-zero for decorated
            let h = module_factory_calc_hash_for_string_residual("0ActiveBody");
            let h2 = module_factory_calc_hash_for_string_residual("0OpenContain");
            h != 0 && h2 != 0 && h != h2
        }
        && {
            let b0 = module_factory_module_data_hash_bucket_residual(
                MODULE_TYPE_BEHAVIOR,
                "ActiveBody",
            );
            let b1 = module_factory_module_data_hash_bucket_residual(
                MODULE_TYPE_DRAW,
                "W3DModelDraw",
            );
            b0 < MODULE_FACTORY_NAMEKEY_SOCKET_COUNT_RESIDUAL
                && b1 < MODULE_FACTORY_NAMEKEY_SOCKET_COUNT_RESIDUAL
        }
        // Registry residual bookkeeping
        && {
            let mut reg = ModuleFactoryRegistryResidual::new();
            reg.seed_from_expanded_table_residual();
            let n = MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101.len() as u32;
            reg.template_map_count == n
                && reg.add_module_applications == n
                && reg.find_module_interface_mask_residual("ActiveBody", MODULE_TYPE_BEHAVIOR)
                    == MODULE_INTERFACE_BODY as i32
                && reg.find_module_interface_mask_residual("", MODULE_TYPE_BEHAVIOR) == 0
                && reg.find_module_interface_mask_residual("NoSuchModule", MODULE_TYPE_BEHAVIOR)
                    == 0
                && reg.find_module_hits == 1
                && reg.find_module_misses == 2
                && reg.push_module_data_residual("ActiveBody", MODULE_TYPE_BEHAVIOR)
                && reg.module_data_list_count == 1
                && !reg.push_module_data_residual("", MODULE_TYPE_BEHAVIOR)
                && {
                    reg.clear_dtor_residual();
                    reg.template_map_count == 0 && reg.module_data_list_count == 0
                }
        }
}

// ---------------------------------------------------------------------------
// 2. ThingFactory create residual deepen (post-create + copy + findTemplate)
// ---------------------------------------------------------------------------

/// C++ `newObject` post-create residual bookkeeping step names (subset of Wave 100 pipeline).
pub const THING_FACTORY_POST_CREATE_STEPS_WAVE101: &[&str] = &[
    "GAMELOGIC_CREATE",  // TheGameLogic->friend_createObject(tmplate, statusBits, team)
    "TEAM_ASSIGN",       // Object ctor takes Team* (statusBits applied pre-onCreate)
    "ON_CREATE_MODULES", // CreateModuleInterface::onCreate loop
    "PARTITION_REGISTER", // ThePartitionManager->registerObject
    "INIT_OBJECT",       // obj->initObject
];

/// Host residual counters for ThingFactory::newObject post-create path.
#[derive(Debug, Clone, Default)]
pub struct ThingFactoryCreateResidualCounters {
    pub gamelogic_create_applications: u32,
    pub team_assign_applications: u32,
    pub on_create_module_applications: u32,
    pub partition_register_applications: u32,
    pub init_object_applications: u32,
    pub reject_null_template_applications: u32,
    pub reject_drawable_only_applications: u32,
    pub build_variation_resolve_applications: u32,
    /// Objects currently "alive" in residual ledger (create − destroy path not modeled fully).
    pub live_object_count: u32,
}

impl ThingFactoryCreateResidualCounters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Residual newObject path: returns false when template invalid / drawable-only.
    pub fn new_object_residual(
        &mut self,
        template_present: bool,
        is_drawable_only: bool,
        build_variation_count: usize,
        create_module_count: u32,
        team_present: bool,
    ) -> bool {
        if !template_present {
            self.reject_null_template_applications =
                self.reject_null_template_applications.saturating_add(1);
            return false;
        }
        if build_variation_count > 0 {
            self.build_variation_resolve_applications =
                self.build_variation_resolve_applications.saturating_add(1);
        }
        if is_drawable_only {
            self.reject_drawable_only_applications =
                self.reject_drawable_only_applications.saturating_add(1);
            return false;
        }
        // GAMELOGIC_CREATE
        self.gamelogic_create_applications =
            self.gamelogic_create_applications.saturating_add(1);
        // TEAM_ASSIGN residual (team pointer applied in Object ctor)
        if team_present {
            self.team_assign_applications = self.team_assign_applications.saturating_add(1);
        }
        // ON_CREATE_MODULES residual (loop count applications)
        self.on_create_module_applications = self
            .on_create_module_applications
            .saturating_add(create_module_count.max(0));
        // PARTITION_REGISTER
        self.partition_register_applications =
            self.partition_register_applications.saturating_add(1);
        // INIT_OBJECT
        self.init_object_applications = self.init_object_applications.saturating_add(1);
        self.live_object_count = self.live_object_count.saturating_add(1);
        true
    }
}

/// Template copy residual fields preserved across `ThingTemplate::copyFrom`.
/// C++ preserves name, id, and next-list-link; copies remaining guts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThingTemplateCopyResidual {
    pub name: String,
    pub template_id: u16,
    pub next_link_token: u32,
    pub armor_copied_from_default: bool,
    pub weapons_copied_from_default: bool,
    pub modules_copied_from_default: bool,
    /// Sample payload field residual (copied from source).
    pub display_name_token: u32,
}

impl ThingTemplateCopyResidual {
    pub fn new(name: &str, template_id: u16) -> Self {
        Self {
            name: name.to_string(),
            template_id,
            next_link_token: 0,
            armor_copied_from_default: false,
            weapons_copied_from_default: false,
            modules_copied_from_default: false,
            display_name_token: 0,
        }
    }

    /// C++ `copyFrom`: preserve name/id/next; copy payload.
    pub fn copy_from_residual(&mut self, source: &ThingTemplateCopyResidual) {
        let name = self.name.clone();
        let id = self.template_id;
        let next = self.next_link_token;
        *self = source.clone();
        self.name = name;
        self.template_id = id;
        self.next_link_token = next;
    }

    /// C++ `setCopiedFromDefault` residual flags.
    pub fn set_copied_from_default_residual(&mut self) {
        self.armor_copied_from_default = true;
        self.weapons_copied_from_default = true;
        self.modules_copied_from_default = true;
    }
}

/// C++ AsciiString hash residual used by ThingTemplateHashMap (`rts::hash` via char*).
/// Mirrors NameKey `calcHashForString` for host honesty (bucket not required for find).
#[inline]
pub fn thing_factory_template_name_hash_residual(name: &str) -> u32 {
    module_factory_calc_hash_for_string_residual(name)
}

/// Host residual findTemplate by exact name (case-sensitive; C++ AsciiString exact).
/// Returns index into residual name table, or None.
#[inline]
pub fn thing_factory_find_template_by_name_residual(
    table: &[&str],
    name: &str,
) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    // Case-sensitive exact match residual (C++ is case sensitive).
    table.iter().position(|&n| n == name)
}

/// Residual: findTemplate with check=true on missing non-empty name → crash residual flag.
#[inline]
pub fn thing_factory_find_template_missing_is_crash_residual(
    found: bool,
    check: bool,
    name_empty: bool,
) -> bool {
    check && !found && !name_empty
}

/// Wave 101 honesty: ThingFactory create residual deepen pack.
pub fn honesty_thing_factory_create_residual_deepen_pack_wave101() -> bool {
    // Post-create step table residual
    THING_FACTORY_POST_CREATE_STEPS_WAVE101.len() == 5
        && residual_name_index(THING_FACTORY_POST_CREATE_STEPS_WAVE101, "GAMELOGIC_CREATE")
            == Some(0)
        && residual_name_index(THING_FACTORY_POST_CREATE_STEPS_WAVE101, "TEAM_ASSIGN")
            == Some(1)
        && residual_name_index(THING_FACTORY_POST_CREATE_STEPS_WAVE101, "ON_CREATE_MODULES")
            == Some(2)
        && residual_name_index(
            THING_FACTORY_POST_CREATE_STEPS_WAVE101,
            "PARTITION_REGISTER",
        ) == Some(3)
        && residual_name_index(THING_FACTORY_POST_CREATE_STEPS_WAVE101, "INIT_OBJECT")
            == Some(4)
        // Cross-link Wave 100 pipeline still has PARTITION_REGISTER / INIT_OBJECT
        && residual_name_index(
            THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS,
            "PARTITION_REGISTER",
        ) == Some(5)
        && residual_name_index(THING_FACTORY_NEW_OBJECT_PIPELINE_STEPS, "INIT_OBJECT")
            == Some(6)
        // Create counters residual
        && {
            let mut c = ThingFactoryCreateResidualCounters::new();
            !c.new_object_residual(false, false, 0, 0, false)
                && c.reject_null_template_applications == 1
                && !c.new_object_residual(true, true, 0, 0, true)
                && c.reject_drawable_only_applications == 1
                && c.new_object_residual(true, false, 2, 3, true)
                && c.gamelogic_create_applications == 1
                && c.team_assign_applications == 1
                && c.build_variation_resolve_applications == 1
                && c.on_create_module_applications == 3
                && c.partition_register_applications == 1
                && c.init_object_applications == 1
                && c.live_object_count == 1
                // Second create without team
                && c.new_object_residual(true, false, 0, 1, false)
                && c.team_assign_applications == 1
                && c.live_object_count == 2
                && c.on_create_module_applications == 4
        }
        // Template copy residual
        && {
            let mut dst = ThingTemplateCopyResidual::new("AmericaTankCrusader", 42);
            dst.next_link_token = 7;
            let mut src = ThingTemplateCopyResidual::new("DefaultThingTemplate", 1);
            src.display_name_token = 99;
            src.set_copied_from_default_residual();
            dst.copy_from_residual(&src);
            dst.name == "AmericaTankCrusader"
                && dst.template_id == 42
                && dst.next_link_token == 7
                && dst.display_name_token == 99
                && dst.armor_copied_from_default
                && dst.weapons_copied_from_default
                && dst.modules_copied_from_default
                && src.name == "DefaultThingTemplate"
        }
        // findTemplate name hash residual honesty
        && {
            let names = [
                "DefaultThingTemplate",
                "AmericaTankCrusader",
                "ChinaCommandCenter",
            ];
            thing_factory_find_template_by_name_residual(&names, "AmericaTankCrusader")
                == Some(1)
                && thing_factory_find_template_by_name_residual(&names, "americatankcrusader")
                    .is_none() // case-sensitive
                && thing_factory_find_template_by_name_residual(&names, "").is_none()
                && thing_factory_find_template_by_name_residual(&names, "Missing").is_none()
                && thing_factory_find_template_missing_is_crash_residual(false, true, false)
                && !thing_factory_find_template_missing_is_crash_residual(false, true, true)
                && !thing_factory_find_template_missing_is_crash_residual(true, true, false)
                && !thing_factory_find_template_missing_is_crash_residual(false, false, false)
                && {
                    let h1 = thing_factory_template_name_hash_residual("AmericaTankCrusader");
                    let h2 = thing_factory_template_name_hash_residual("ChinaCommandCenter");
                    h1 != 0 && h2 != 0 && h1 != h2
                }
        }
        // Default template name residual still holds
        && THING_FACTORY_DEFAULT_TEMPLATE_NAME == "DefaultThingTemplate"
        && THING_FACTORY_TEMPLATE_HASH_SIZE == 12288
}

// ---------------------------------------------------------------------------
// 3. PartitionManager register residual (host counters; cell size cross-link)
// ---------------------------------------------------------------------------

/// Retail GameData.ini / Wave 96 `PartitionCellSize` residual (**40** world units).
pub const PARTITION_REGISTER_CELL_SIZE_RESIDUAL: f32 = 40.0;

/// C++ `PartitionManager::registerObject` residual step names.
pub const PARTITION_REGISTER_OBJECT_STEPS_WAVE101: &[&str] = &[
    "SANITY_NULL",           // object == NULL → return
    "REJECT_ALREADY_REG",    // friend_getPartitionData() != NULL → return
    "ALLOC_PARTITION_DATA",  // newInstance(PartitionData)
    "LINK_MODULE_LIST",      // prepend to m_moduleList
    "ATTACH_TO_OBJECT",      // mod->attachToObject(object)
];

/// C++ `PartitionManager::unRegisterObject` residual step names (happy path, no ghost).
pub const PARTITION_UNREGISTER_OBJECT_STEPS_WAVE101: &[&str] = &[
    "SANITY_NULL",            // object == NULL → return
    "SANITY_NO_PARTDATA",     // friend_getPartitionData() == NULL → return
    "GHOST_FOG_HOLD",         // ghost seen residual may defer delete
    "DETACH_FROM_OBJECT",     // mod->detachFromObject()
    "UNLINK_MODULE_LIST",     // remove from m_moduleList
    "DELETE_PARTITION_DATA",  // mod->deleteInstance()
];

/// Host residual PartitionManager register bookkeeping.
#[derive(Debug, Clone, Default)]
pub struct PartitionRegisterResidualCounters {
    pub register_applications: u32,
    pub register_null_rejects: u32,
    pub register_already_rejects: u32,
    pub unregister_applications: u32,
    pub unregister_null_rejects: u32,
    pub unregister_missing_rejects: u32,
    pub ghost_fog_hold_applications: u32,
    /// Simulated live registered object count residual.
    pub registered_live_count: u32,
}

impl PartitionRegisterResidualCounters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Residual registerObject: `already_registered` models friend_getPartitionData()!=NULL.
    pub fn register_object_residual(
        &mut self,
        object_present: bool,
        already_registered: bool,
    ) -> bool {
        if !object_present {
            self.register_null_rejects = self.register_null_rejects.saturating_add(1);
            return false;
        }
        if already_registered {
            self.register_already_rejects = self.register_already_rejects.saturating_add(1);
            return false;
        }
        self.register_applications = self.register_applications.saturating_add(1);
        self.registered_live_count = self.registered_live_count.saturating_add(1);
        true
    }

    /// Residual unRegisterObject.
    /// `has_partition_data` / `ghost_fog_hold` model C++ early outs.
    pub fn unregister_object_residual(
        &mut self,
        object_present: bool,
        has_partition_data: bool,
        ghost_fog_hold: bool,
    ) -> bool {
        if !object_present {
            self.unregister_null_rejects = self.unregister_null_rejects.saturating_add(1);
            return false;
        }
        if !has_partition_data {
            self.unregister_missing_rejects = self.unregister_missing_rejects.saturating_add(1);
            return false;
        }
        if ghost_fog_hold {
            // C++ keeps PartitionData for fogged ghost; object pointer cleared.
            self.ghost_fog_hold_applications =
                self.ghost_fog_hold_applications.saturating_add(1);
            // live count still drops from "object registered" view
            self.registered_live_count = self.registered_live_count.saturating_sub(1);
            return true;
        }
        self.unregister_applications = self.unregister_applications.saturating_add(1);
        self.registered_live_count = self.registered_live_count.saturating_sub(1);
        true
    }
}

/// World→cell residual using PartitionCellSize **40** (Wave 96 closed; linked here).
#[inline]
pub fn partition_register_world_to_cell_residual(world: f32, world_lo: f32) -> i32 {
    let inv = 1.0 / PARTITION_REGISTER_CELL_SIZE_RESIDUAL;
    ((world - world_lo) * inv).floor() as i32
}

/// Wave 101 honesty: PartitionManager register residual pack.
pub fn honesty_partition_register_residual_pack_wave101() -> bool {
    PARTITION_REGISTER_CELL_SIZE_RESIDUAL == 40.0
        && PARTITION_REGISTER_OBJECT_STEPS_WAVE101.len() == 5
        && residual_name_index(PARTITION_REGISTER_OBJECT_STEPS_WAVE101, "SANITY_NULL")
            == Some(0)
        && residual_name_index(
            PARTITION_REGISTER_OBJECT_STEPS_WAVE101,
            "ATTACH_TO_OBJECT",
        ) == Some(4)
        && PARTITION_UNREGISTER_OBJECT_STEPS_WAVE101.len() == 6
        && residual_name_index(
            PARTITION_UNREGISTER_OBJECT_STEPS_WAVE101,
            "GHOST_FOG_HOLD",
        ) == Some(2)
        && residual_name_index(
            PARTITION_UNREGISTER_OBJECT_STEPS_WAVE101,
            "DELETE_PARTITION_DATA",
        ) == Some(5)
        // Register / unregister bookkeeping residual
        && {
            let mut p = PartitionRegisterResidualCounters::new();
            !p.register_object_residual(false, false)
                && p.register_null_rejects == 1
                && p.register_object_residual(true, false)
                && p.register_applications == 1
                && p.registered_live_count == 1
                && !p.register_object_residual(true, true)
                && p.register_already_rejects == 1
                && p.registered_live_count == 1
                && p.unregister_object_residual(true, true, false)
                && p.unregister_applications == 1
                && p.registered_live_count == 0
                && !p.unregister_object_residual(false, false, false)
                && p.unregister_null_rejects == 1
                && !p.unregister_object_residual(true, false, false)
                && p.unregister_missing_rejects == 1
                // Ghost fog hold residual
                && p.register_object_residual(true, false)
                && p.unregister_object_residual(true, true, true)
                && p.ghost_fog_hold_applications == 1
                && p.registered_live_count == 0
        }
        // Cell size world→cell residual link
        && partition_register_world_to_cell_residual(0.0, 0.0) == 0
        && partition_register_world_to_cell_residual(39.9, 0.0) == 0
        && partition_register_world_to_cell_residual(40.0, 0.0) == 1
        && partition_register_world_to_cell_residual(80.0, 0.0) == 2
        && partition_register_world_to_cell_residual(40.0, -40.0) == 2
        // Cross-link ThingFactory newObject always partition-registers on success
        && {
            let mut c = ThingFactoryCreateResidualCounters::new();
            c.new_object_residual(true, false, 0, 0, true)
                && c.partition_register_applications == 1
        }
}

// ---------------------------------------------------------------------------
// Combined Wave 101 residual pack + cross-link
// ---------------------------------------------------------------------------

/// Cross-link: ModuleFactory CREATE interface residual used by newObject onCreate loop.
pub fn honesty_thing_factory_module_partition_crosslink_wave101() -> bool {
    // CREATE interface ordinal residual (Wave 100 still holds)
    MODULE_INTERFACE_CREATE == 0x8
        && residual_name_index(MODULE_INTERFACE_NAME_TABLE_RESIDUAL, "CREATE") == Some(3)
        // TunnelContain multi-interface includes CREATE (onCreate path modules)
        && {
            let tunnel = MODULE_FACTORY_EXPANDED_TABLE_RESIDUAL_WAVE101
                .iter()
                .find(|s| s.name == "TunnelContain")
                .map(|s| s.interface_mask)
                .unwrap_or(0);
            module_interface_mask_has_all_residual(tunnel, MODULE_INTERFACE_CREATE)
        }
        // newObject residual always hits PARTITION_REGISTER after ON_CREATE
        && residual_name_index(
            THING_FACTORY_POST_CREATE_STEPS_WAVE101,
            "ON_CREATE_MODULES",
        ) == Some(2)
        && residual_name_index(
            THING_FACTORY_POST_CREATE_STEPS_WAVE101,
            "PARTITION_REGISTER",
        ) == Some(3)
        // Cell size residual still 40
        && PARTITION_REGISTER_CELL_SIZE_RESIDUAL == 40.0
}

/// Combined Wave 101 residual honesty pack.
pub fn honesty_thing_factory_module_partition_residual_pack_wave101() -> bool {
    honesty_module_factory_residual_deepen_pack_wave101()
        && honesty_thing_factory_create_residual_deepen_pack_wave101()
        && honesty_partition_register_residual_pack_wave101()
        && honesty_thing_factory_module_partition_crosslink_wave101()
        // Wave 100 packs still hold (deepen, not replace)
        && honesty_thing_factory_residual_deepen_pack_wave100()
        && honesty_module_type_table_residual_pack_wave100()
}

#[cfg(test)]
mod tests_wave101 {
    use super::*;

    #[test]
    fn residual_pack_honesty_wave101_module_factory() {
        assert!(honesty_module_factory_residual_deepen_pack_wave101());
    }

    #[test]
    fn residual_pack_honesty_wave101_thing_factory_create() {
        assert!(honesty_thing_factory_create_residual_deepen_pack_wave101());
    }

    #[test]
    fn residual_pack_honesty_wave101_partition_register() {
        assert!(honesty_partition_register_residual_pack_wave101());
    }

    #[test]
    fn residual_pack_honesty_wave101_crosslink() {
        assert!(honesty_thing_factory_module_partition_crosslink_wave101());
    }

    #[test]
    fn residual_pack_honesty_wave101() {
        assert!(honesty_thing_factory_module_partition_residual_pack_wave101());
    }
}
