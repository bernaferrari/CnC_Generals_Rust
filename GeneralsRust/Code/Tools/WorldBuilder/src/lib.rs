//! WorldBuilder library surface for C++-matching core transforms.

pub mod save_map;
pub mod scorch_tool;

pub use save_map::{
    BlendTileDataEdit, BlendedTileEdit, BuildListItemEdit, CliffInfoEdit, GlobalLightEdit,
    GlobalLightingEdit, HeightMapEdit, MapDocument, MapObjectEdit, PolygonTriggerEdit, SaveMap,
    SaveMapError, ScriptEdit, ScriptGroupEdit, ScriptListEdit, SideInfoEdit, TextureClassEdit,
    BLEND_TILE_FLAG_VAL, K_BLEND_TILE_VERSION_8, K_HEIGHT_MAP_VERSION_4, K_LIGHTING_VERSION_3,
    K_OBJECTS_VERSION_3, K_SIDES_DATA_VERSION_3, K_TRIGGERS_VERSION_4, K_WORLDDICT_VERSION_1,
    LIGHTING_TOD_SLOTS, MAX_GLOBAL_LIGHTS,
};
pub use scorch_tool::{
    mouse_down_scorch, pick_scorch, snap_doc_point, DEFAULT_SCORCHMARK_RADIUS,
    NEUTRAL_TEAM_INTERNAL_STR, SCORCH_1,
};
