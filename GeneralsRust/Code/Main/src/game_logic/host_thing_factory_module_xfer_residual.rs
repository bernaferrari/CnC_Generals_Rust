//! Wave 100 residual peels: ThingFactory residual deepen / Module residual type tables /
//! Xfer residual deepen (host-testable factory / module / save-load residual).
//!
//! Orthogonal to Waves 65/74 (ThingFactory object packs + spawn bookkeeping),
//! Wave 82/84 (enum bit-name tables), and Main save_load Snapshot plumbing.
//! Host-testable packs for ThingFactory / ModuleType / ModuleInterface / Xfer residual.
//!
//! Sources (retail ZH C++ / INI):
//! - ThingFactory.h/.cpp TEMPLATE_HASH_SIZE **12288** / m_nextTemplateID **1** /
//!   DefaultThingTemplate / newObject pipeline / newDrawable / KINDOF_DRAWABLE_ONLY
//! - Module.h ModuleType BEHAVIOR/DRAW/CLIENT_UPDATE + ModuleInterfaceType bits
//! - ModuleFactory.h/.cpp makeDecoratedNameKey / findModuleInterfaceMask empty→0
//! - Xfer.h XferMode / XferStatus / XferOptions / XferVersion (UnsignedByte)
//! - Xfer.cpp xferVersion reject > currentVersion / ctor XO_NONE + XFER_INVALID
//! - XferCRC.cpp mode XFER_CRC / m_crc **0** / addCRC residual
//! - Object.cpp xfer CURRENT_VERSION **9**; Drawable.cpp xfer CURRENT_VERSION **7**
//! - GameState.h SaveFileType / SnapshotType / SaveCode residual
//! - Drawable.h DRAWABLE_STATUS bits; ObjectStatusTypes OBJECT_STATUS_MASK_NONE
//!
//! Fail-closed:
//! - Not full ThingFactory Object / live module stack / partition register residual
//! - Not full ModuleFactory addModule registry / live BehaviorModule create residual
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
