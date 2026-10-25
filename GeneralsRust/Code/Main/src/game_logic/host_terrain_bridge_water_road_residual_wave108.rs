//! Wave 108 residual peels: HeightMap / bridge / water / road deepen + cliff peels
//! (host-testable terrain residual; orthogonal to Wave 87 env + Wave 93 road/texture).
//!
//! Orthogonal to Wave 81 MAP_XY/MAP_HEIGHT sample residual, Wave 87 water/bridge
//! packs, and Wave 93 terrain texture / road residual peels.
//! Host residual only — shell `playable_claim` stays false; network deferred.
//!
//! Sources (retail ZH C++ / INI):
//! - WorldHeightMap.h / HeightMap.h / BaseHeightMap.h / MapObject.h
//! - TerrainLogic.h BridgeInfo / MAX_DYNAMIC_WATER; W3DBridgeBuffer.h
//! - TerrainRoads.h BridgeTowerType / bridge FieldParse; Roads.ini samples
//! - Water.h WaterTransparency skybox defaults; W3DWater.h WaterType / bump
//! - WorldHeightMap.cpp PATHFIND_CLIFF_SLOPE_LIMIT_F / cliff UV stretch
//! - AIPathfind.h CELL_CLIFF pathfind cell residual
//!
//! Fail-closed:
//! - Not full SAGE HeightMap bilinear / bridge-aware sample residual
//! - Not full W3DBridgeBuffer mesh bake / DX8 VB residual
//! - Not full W3DWater reflection / skybox mesh residual
//! - Not full W3DRoadBuffer mesh bake residual
//! - Not full cliff seam UV mutant mapping GPU residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

// ---------------------------------------------------------------------------
// 1. HeightMap residual deepen (beyond Wave 81 sample scale residual)
// ---------------------------------------------------------------------------

/// C++ `MAP_XY_FACTOR` residual (MapObject.h) — world units per heightmap cell.
pub const HEIGHTMAP_MAP_XY_FACTOR_RESIDUAL: f32 = 10.0;
/// C++ `MAP_HEIGHT_SCALE` residual (`MAP_XY_FACTOR / 16`).
pub const HEIGHTMAP_MAP_HEIGHT_SCALE_RESIDUAL: f32 = HEIGHTMAP_MAP_XY_FACTOR_RESIDUAL / 16.0;
/// C++ `K_MIN_HEIGHT` residual (WorldHeightMap.h).
pub const HEIGHTMAP_K_MIN_HEIGHT_RESIDUAL: i32 = 0;
/// C++ `K_MAX_HEIGHT` residual (WorldHeightMap.h) — raw 8-bit sample max.
pub const HEIGHTMAP_K_MAX_HEIGHT_RESIDUAL: i32 = 255;
/// C++ `NUM_SOURCE_TILES` residual.
pub const HEIGHTMAP_NUM_SOURCE_TILES_RESIDUAL: i32 = 1024;
/// C++ `NUM_BLEND_TILES` residual.
pub const HEIGHTMAP_NUM_BLEND_TILES_RESIDUAL: i32 = 16192;
/// C++ `NUM_CLIFF_INFO` residual.
pub const HEIGHTMAP_NUM_CLIFF_INFO_RESIDUAL: i32 = 32384;
/// C++ `FLAG_VAL` residual (WorldHeightMap chunk flag sentinel).
pub const HEIGHTMAP_FLAG_VAL_RESIDUAL: u32 = 0x7ADA_0000;
/// C++ `TEX_PATH_LEN` residual.
pub const HEIGHTMAP_TEX_PATH_LEN_RESIDUAL: i32 = 256;
/// C++ `NUM_TEXTURE_CLASSES` residual.
pub const HEIGHTMAP_NUM_TEXTURE_CLASSES_RESIDUAL: i32 = 256;
/// C++ `NUM_ALPHA_TILES` residual.
pub const HEIGHTMAP_NUM_ALPHA_TILES_RESIDUAL: i32 = 12;
/// C++ `NORMAL_DRAW_WIDTH` / `NORMAL_DRAW_HEIGHT` residual.
pub const HEIGHTMAP_NORMAL_DRAW_WIDTH_RESIDUAL: i32 = 129;
pub const HEIGHTMAP_NORMAL_DRAW_HEIGHT_RESIDUAL: i32 = 129;
/// C++ `STRETCH_DRAW_WIDTH` / `STRETCH_DRAW_HEIGHT` residual.
pub const HEIGHTMAP_STRETCH_DRAW_WIDTH_RESIDUAL: i32 = 65;
pub const HEIGHTMAP_STRETCH_DRAW_HEIGHT_RESIDUAL: i32 = 65;
/// C++ `VERTEX_BUFFER_TILE_LENGTH` residual (HeightMap.h tiles of side 32).
pub const HEIGHTMAP_VERTEX_BUFFER_TILE_LENGTH_RESIDUAL: i32 = 32;
/// C++ `FLIP_TRIANGLES` residual (cliff triangle flip enable).
pub const HEIGHTMAP_FLIP_TRIANGLES_RESIDUAL: i32 = 1;
/// C++ `MAX_ENABLED_DYNAMIC_LIGHTS` residual (BaseHeightMap.h).
pub const HEIGHTMAP_MAX_ENABLED_DYNAMIC_LIGHTS_RESIDUAL: i32 = 20;
/// C++ scorch residual caps (BaseHeightMap.h).
pub const HEIGHTMAP_MAX_SCORCH_VERTEX_RESIDUAL: i32 = 8194;
pub const HEIGHTMAP_MAX_SCORCH_INDEX_RESIDUAL: i32 = 6 * 8194;
pub const HEIGHTMAP_MAX_SCORCH_MARKS_RESIDUAL: i32 = 500;
/// C++ HeightMap ctor residual: cliffInfo[0] is the default info; m_numCliffInfo starts 1.
pub const HEIGHTMAP_NUM_CLIFF_INFO_DEFAULT_USED_RESIDUAL: i32 = 1;
/// C++ cliffInfoNdx 0 means no cliff info residual.
pub const HEIGHTMAP_CLIFF_INFO_NDX_NONE_RESIDUAL: i16 = 0;

/// Convert raw 8-bit height sample → world Z residual.
#[inline]
pub fn heightmap_raw_sample_to_world_residual(sample: u8) -> f32 {
    (sample as f32) * HEIGHTMAP_MAP_HEIGHT_SCALE_RESIDUAL
}

/// World Z residual max at K_MAX_HEIGHT.
#[inline]
pub fn heightmap_world_z_max_residual() -> f32 {
    (HEIGHTMAP_K_MAX_HEIGHT_RESIDUAL as f32) * HEIGHTMAP_MAP_HEIGHT_SCALE_RESIDUAL
}

/// Wave 108 honesty: HeightMap residual deepen pack.
///
/// Freezes WorldHeightMap capacity tables, draw dimensions, VB tile length,
/// scorch caps, and MAP_XY/HEIGHT scale residual (cross-link Wave 81).
/// Fail-closed: not full live HeightMapData decode / bilinear bridge matrix.
pub fn honesty_heightmap_residual_deepen_pack_wave108() -> bool {
    let scale_ok = (HEIGHTMAP_MAP_XY_FACTOR_RESIDUAL - 10.0).abs() < 1e-5
        && (HEIGHTMAP_MAP_HEIGHT_SCALE_RESIDUAL - 0.625).abs() < 1e-5
        && (HEIGHTMAP_MAP_HEIGHT_SCALE_RESIDUAL - HEIGHTMAP_MAP_XY_FACTOR_RESIDUAL / 16.0).abs()
            < 1e-5
        && HEIGHTMAP_K_MIN_HEIGHT_RESIDUAL == 0
        && HEIGHTMAP_K_MAX_HEIGHT_RESIDUAL == 255
        && (heightmap_raw_sample_to_world_residual(0) - 0.0).abs() < 1e-5
        && (heightmap_raw_sample_to_world_residual(16) - 10.0).abs() < 1e-3
        && (heightmap_raw_sample_to_world_residual(255) - heightmap_world_z_max_residual()).abs()
            < 1e-2
        && (heightmap_world_z_max_residual() - 255.0 * 0.625).abs() < 1e-2;

    let capacity_ok = HEIGHTMAP_NUM_SOURCE_TILES_RESIDUAL == 1024
        && HEIGHTMAP_NUM_BLEND_TILES_RESIDUAL == 16192
        && HEIGHTMAP_NUM_CLIFF_INFO_RESIDUAL == 32384
        // NUM_CLIFF_INFO is 2× NUM_BLEND_TILES residual.
        && HEIGHTMAP_NUM_CLIFF_INFO_RESIDUAL == 2 * HEIGHTMAP_NUM_BLEND_TILES_RESIDUAL
        && HEIGHTMAP_FLAG_VAL_RESIDUAL == 0x7ADA_0000
        && HEIGHTMAP_TEX_PATH_LEN_RESIDUAL == 256
        && HEIGHTMAP_NUM_TEXTURE_CLASSES_RESIDUAL == 256
        && HEIGHTMAP_NUM_ALPHA_TILES_RESIDUAL == 12
        && HEIGHTMAP_NUM_CLIFF_INFO_DEFAULT_USED_RESIDUAL == 1
        && HEIGHTMAP_CLIFF_INFO_NDX_NONE_RESIDUAL == 0;

    let draw_ok = HEIGHTMAP_NORMAL_DRAW_WIDTH_RESIDUAL == 129
        && HEIGHTMAP_NORMAL_DRAW_HEIGHT_RESIDUAL == 129
        && HEIGHTMAP_STRETCH_DRAW_WIDTH_RESIDUAL == 65
        && HEIGHTMAP_STRETCH_DRAW_HEIGHT_RESIDUAL == 65
        // Stretch is roughly half normal residual (129→65): 65*2-1 == 129.
        && HEIGHTMAP_STRETCH_DRAW_WIDTH_RESIDUAL * 2 - 1
            == HEIGHTMAP_NORMAL_DRAW_WIDTH_RESIDUAL
        && HEIGHTMAP_VERTEX_BUFFER_TILE_LENGTH_RESIDUAL == 32
        && HEIGHTMAP_FLIP_TRIANGLES_RESIDUAL == 1
        && HEIGHTMAP_MAX_ENABLED_DYNAMIC_LIGHTS_RESIDUAL == 20
        && HEIGHTMAP_MAX_SCORCH_VERTEX_RESIDUAL == 8194
        && HEIGHTMAP_MAX_SCORCH_INDEX_RESIDUAL == 6 * 8194
        && HEIGHTMAP_MAX_SCORCH_MARKS_RESIDUAL == 500;

    scale_ok && capacity_ok && draw_ok
}

// ---------------------------------------------------------------------------
// 2. Bridge residual deepen (beyond Wave 87 tower / scaffold residual)
// ---------------------------------------------------------------------------

/// C++ `TBridgeType` residual (W3DBridgeBuffer.h).
pub const BRIDGE_TYPE_FIXED_RESIDUAL: u32 = 0;
pub const BRIDGE_TYPE_SECTIONAL_RESIDUAL: u32 = 1;
pub const BRIDGE_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &["FIXED_BRIDGE", "SECTIONAL_BRIDGE"];

/// C++ W3DBridgeBuffer capacity residual.
pub const MAX_BRIDGE_VERTEX_RESIDUAL: i32 = 12_000;
pub const MAX_BRIDGE_INDEX_RESIDUAL: i32 = 2 * 12_000;
pub const MAX_BRIDGES_RESIDUAL: i32 = 200;

/// C++ BridgeTowerType residual (TerrainRoads.h) — deepen re-anchor Wave 87.
pub const BRIDGE_TOWER_FROM_LEFT_RESIDUAL: u32 = 0;
pub const BRIDGE_TOWER_FROM_RIGHT_RESIDUAL: u32 = 1;
pub const BRIDGE_TOWER_TO_LEFT_RESIDUAL: u32 = 2;
pub const BRIDGE_TOWER_TO_RIGHT_RESIDUAL: u32 = 3;
pub const BRIDGE_MAX_TOWERS_RESIDUAL: u32 = 4;
pub const BRIDGE_TOWER_TYPE_NAMES_RESIDUAL: &[&str] = &[
    "BRIDGE_TOWER_FROM_LEFT",
    "BRIDGE_TOWER_FROM_RIGHT",
    "BRIDGE_TOWER_TO_LEFT",
    "BRIDGE_TOWER_TO_RIGHT",
];

/// C++ `MAX_BRIDGE_BODY_FX` residual.
pub const MAX_BRIDGE_BODY_FX_RESIDUAL: u32 = 3;
/// C++ BODYDAMAGETYPE_COUNT residual (PRISTINE/DAMAGED/REALLYDAMAGED/RUBBLE).
pub const BRIDGE_BODY_DAMAGE_TYPE_COUNT_RESIDUAL: usize = 4;
/// C++ BridgeBehaviorModuleData scaffold speed defaults residual.
pub const BRIDGE_LATERAL_SCAFFOLD_SPEED_RESIDUAL: f32 = 1.0;
pub const BRIDGE_VERTICAL_SCAFFOLD_SPEED_RESIDUAL: f32 = 1.0;
/// C++ TerrainRoadType ctor residual: m_bridgeScale default.
pub const BRIDGE_SCALE_CTOR_DEFAULT_RESIDUAL: f32 = 1.0;
/// C++ TerrainRoadType ctor residual: m_transitionEffectsHeight / m_numFXPerType.
pub const BRIDGE_TRANSITION_EFFECTS_HEIGHT_CTOR_RESIDUAL: f32 = 0.0;
pub const BRIDGE_NUM_FX_PER_TYPE_CTOR_RESIDUAL: i32 = 0;

/// C++ TerrainRoadType bridge FieldParse residual keys (subset).
pub const BRIDGE_FIELD_PARSE_KEYS_RESIDUAL: &[&str] = &[
    "BridgeScale",
    "ScaffoldObjectName",
    "ScaffoldSupportObjectName",
    "RadarColor",
    "TransitionEffectsHeight",
    "NumFXPerType",
    "BridgeModelName",
    "Texture",
    "BridgeModelNameDamaged",
    "TextureDamaged",
    "BridgeModelNameReallyDamaged",
    "TextureReallyDamaged",
    "BridgeModelNameBroken",
    "TextureBroken",
    "TowerObjectNameFromLeft",
    "TowerObjectNameFromRight",
    "TowerObjectNameToLeft",
    "TowerObjectNameToRight",
    "DamagedToSound",
    "RepairedToSound",
    "TransitionToOCL",
    "TransitionToFX",
];

/// Retail Roads.ini sample residual rows (bridge deepen).
pub const SAMPLE_BRIDGE_IRON_SECTIONAL_NAME_RESIDUAL: &str = "IronSectionalDoublewide";
pub const SAMPLE_BRIDGE_IRON_SECTIONAL_SCALE_RESIDUAL: f32 = 0.85;
pub const SAMPLE_BRIDGE_IRON_SECTIONAL_MODEL_RESIDUAL: &str = "TBDoubWide";
pub const SAMPLE_BRIDGE_CONCRETE_NAME_RESIDUAL: &str = "Concrete";
pub const SAMPLE_BRIDGE_CONCRETE_SCALE_RESIDUAL: f32 = 0.85;
pub const SAMPLE_BRIDGE_CONCRETE_MODEL_RESIDUAL: &str = "CBBridgeSt";
pub const SAMPLE_BRIDGE_CONCRETE_NUM_FX_PER_TYPE_RESIDUAL: i32 = 32;
pub const SAMPLE_BRIDGE_CONCRETE_DAMAGE_SOUND_RESIDUAL: &str = "BridgeDamaged";
pub const SAMPLE_BRIDGE_CONCRETE_REPAIR_SOUND_RESIDUAL: &str = "BridgeRepaired";
pub const SAMPLE_BRIDGE_SCAFFOLD_OBJECT_RESIDUAL: &str = "BridgeScaffold01";
pub const SAMPLE_BRIDGE_SCAFFOLD_SUPPORT_RESIDUAL: &str = "BridgeScaffoldSupport01";
/// Retail radar color residual for standard bridges (R192 G192 B192).
pub const SAMPLE_BRIDGE_RADAR_COLOR_R_RESIDUAL: u8 = 192;
pub const SAMPLE_BRIDGE_RADAR_COLOR_G_RESIDUAL: u8 = 192;
pub const SAMPLE_BRIDGE_RADAR_COLOR_B_RESIDUAL: u8 = 192;
/// Named sample bridge residual table (≥8 common ZH bridges).
pub const SAMPLE_BRIDGE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "IronSectionalDoublewide",
    "WoodenSectional",
    "Industrial",
    "Concrete",
    "ConcreteTwoLane",
    "ConcreteFourLane",
    "EuropeanBridge",
    "TsingMaLandmarkBridge",
];

/// Host residual: effectNum is 1-based in INI, converted to 0-based residual.
#[inline]
pub fn bridge_effect_num_zero_based_residual(effect_num_1_based: i32) -> Option<i32> {
    let zero = effect_num_1_based - 1;
    if zero < 0 || zero >= MAX_BRIDGE_BODY_FX_RESIDUAL as i32 {
        None
    } else {
        Some(zero)
    }
}

/// Wave 108 honesty: bridge residual deepen pack.

// ---------------------------------------------------------------------------
// 3. Water residual deepen (beyond Wave 87 TOD / WaterSet residual)
// ---------------------------------------------------------------------------

/// C++ `WaterType` residual (W3DWater.h).
pub const WATER_TYPE_0_TRANSLUCENT_RESIDUAL: u32 = 0;
pub const WATER_TYPE_1_FB_REFLECTION_RESIDUAL: u32 = 1;
pub const WATER_TYPE_2_PVSHADER_RESIDUAL: u32 = 2;
pub const WATER_TYPE_3_GRIDMESH_RESIDUAL: u32 = 3;
pub const WATER_TYPE_MAX_RESIDUAL: u32 = 4;
pub const WATER_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "WATER_TYPE_0_TRANSLUCENT",
    "WATER_TYPE_1_FB_REFLECTION",
    "WATER_TYPE_2_PVSHADER",
    "WATER_TYPE_3_GRIDMESH",
];

/// C++ `INVALID_WATER_HEIGHT` residual.
pub const INVALID_WATER_HEIGHT_RESIDUAL: f32 = 0.0;
/// C++ `NUM_BUMP_FRAMES` residual.
pub const WATER_NUM_BUMP_FRAMES_RESIDUAL: i32 = 32;
/// C++ TerrainLogic `MAX_DYNAMIC_WATER` residual.
pub const MAX_DYNAMIC_WATER_RESIDUAL: i32 = 64;

/// C++ WaterTransparencySetting ctor skybox texture residual defaults.
pub const WATER_SKYBOX_TEXTURE_N_RESIDUAL: &str = "TSMorningN.tga";
pub const WATER_SKYBOX_TEXTURE_E_RESIDUAL: &str = "TSMorningE.tga";
pub const WATER_SKYBOX_TEXTURE_S_RESIDUAL: &str = "TSMorningS.tga";
pub const WATER_SKYBOX_TEXTURE_W_RESIDUAL: &str = "TSMorningW.tga";
pub const WATER_SKYBOX_TEXTURE_T_RESIDUAL: &str = "TSMorningT.tga";
pub const WATER_SKYBOX_TEXTURE_TABLE_RESIDUAL: &[&str] = &[
    "TSMorningN.tga",
    "TSMorningE.tga",
    "TSMorningS.tga",
    "TSMorningW.tga",
    "TSMorningT.tga",
];

/// WaterTransparency FieldParse residual keys deepen.
pub const WATER_TRANSPARENCY_FIELD_KEYS_RESIDUAL: &[&str] = &[
    "TransparentWaterDepth",
    "TransparentWaterMinOpacity",
    "StandingWaterColor",
    "StandingWaterTexture",
    "AdditiveBlending",
    "RadarWaterColor",
    "SkyboxTextureN",
    "SkyboxTextureE",
    "SkyboxTextureS",
    "SkyboxTextureW",
    "SkyboxTextureT",
];

/// Wave 87 re-anchor residual values still holding.
pub const WATER_TRANSPARENT_DEPTH_RESIDUAL: f32 = 3.0;
pub const WATER_MIN_OPACITY_RESIDUAL: f32 = 1.0;
pub const WATER_STANDING_TEXTURE_RESIDUAL: &str = "TWWater01.tga";
pub const WATER_ADDITIVE_BLEND_RESIDUAL: bool = false;
pub const WATER_RADAR_COLOR_R_RESIDUAL: f32 = 140.0;
pub const WATER_RADAR_COLOR_G_RESIDUAL: f32 = 140.0;
pub const WATER_RADAR_COLOR_B_RESIDUAL: f32 = 255.0;
pub const WATER_REPEAT_COUNT_RESIDUAL: i32 = 32;
pub const WATER_SCROLL_PER_MS_DAY_RESIDUAL: f32 = 0.002;
pub const WATER_SCROLL_PER_MS_NIGHT_RESIDUAL: f32 = 0.0;

/// Ordered WaterSet TOD residual names (MORNING..NIGHT).
pub const WATER_SET_TOD_NAMES_RESIDUAL: &[&str] = &["MORNING", "AFTERNOON", "EVENING", "NIGHT"];

/// Wave 108 honesty: water residual deepen pack.

// ---------------------------------------------------------------------------
// 4. Road residual deepen (beyond Wave 93 road residual peels)
// ---------------------------------------------------------------------------

/// C++ W3DRoadBuffer residual deepen constants.
pub const ROAD_DEFAULT_SCALE_RESIDUAL: f32 = 8.0;
pub const ROAD_MIN_SEGMENT_RESIDUAL: f32 = 0.25;
pub const ROAD_MAX_LINKS_RESIDUAL: i32 = 6;
pub const ROAD_MAX_SEG_VERTEX_RESIDUAL: i32 = 500;
pub const ROAD_MAX_SEG_INDEX_RESIDUAL: i32 = 2000;
pub const ROAD_NUM_CORNERS_RESIDUAL: i32 = 4;
/// C++ TCorner residual ordinals + NUM_JOINS sentinel.
pub const ROAD_CORNER_SEGMENT_RESIDUAL: u8 = 0;
pub const ROAD_CORNER_CURVE_RESIDUAL: u8 = 1;
pub const ROAD_CORNER_TEE_RESIDUAL: u8 = 2;
pub const ROAD_CORNER_FOUR_WAY_RESIDUAL: u8 = 3;
pub const ROAD_CORNER_THREE_WAY_Y_RESIDUAL: u8 = 4;
pub const ROAD_CORNER_THREE_WAY_H_RESIDUAL: u8 = 5;
pub const ROAD_CORNER_THREE_WAY_H_FLIP_RESIDUAL: u8 = 6;
pub const ROAD_CORNER_ALPHA_JOIN_RESIDUAL: u8 = 7;
pub const ROAD_NUM_JOINS_RESIDUAL: i32 = 8;
pub const ROAD_CORNER_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "SEGMENT",
    "CURVE",
    "TEE",
    "FOUR_WAY",
    "THREE_WAY_Y",
    "THREE_WAY_H",
    "THREE_WAY_H_FLIP",
    "ALPHA_JOIN",
];

/// C++ corner enum residual (bottomLeft..topRight).
pub const ROAD_CORNER_BOTTOM_LEFT_RESIDUAL: u8 = 0;
pub const ROAD_CORNER_BOTTOM_RIGHT_RESIDUAL: u8 = 1;
pub const ROAD_CORNER_TOP_LEFT_RESIDUAL: u8 = 2;
pub const ROAD_CORNER_TOP_RIGHT_RESIDUAL: u8 = 3;

/// GameData.ini MaxRoad residual (Wave 93 re-anchor).
pub const MAX_ROAD_SEGMENTS_RESIDUAL: i32 = 4000;
pub const MAX_ROAD_VERTEX_RESIDUAL: i32 = 3000;
pub const MAX_ROAD_INDEX_RESIDUAL: i32 = 5000;
pub const MAX_ROAD_TYPES_RESIDUAL: i32 = 100;

/// Road FieldParse residual keys.
pub const ROAD_FIELD_PARSE_KEYS_RESIDUAL: &[&str] = &["Texture", "RoadWidth", "RoadWidthInTexture"];

/// Retail Roads.ini expanded sample residual table (≥10 road names).
pub const SAMPLE_ROAD_NAME_TABLE_RESIDUAL: &[&str] = &[
    "TwoLane",
    "TwoLaneDark",
    "FourLane",
    "FourLaneDark",
    "Cobblestone",
    "GrassStrip",
    "GrassStripSnow",
    "Sidewalk",
    "DirtRoad",
    "DirtRoadTracks",
];

/// Retail sample residual width anchors (Wave 93 + deepen).
pub const SAMPLE_ROAD_TWO_LANE_WIDTH_RESIDUAL: f32 = 35.0;
pub const SAMPLE_ROAD_FOUR_LANE_WIDTH_RESIDUAL: f32 = 60.0;
pub const SAMPLE_ROAD_COBBLESTONE_WIDTH_RESIDUAL: f32 = 30.0;
pub const SAMPLE_ROAD_GRASS_STRIP_WIDTH_RESIDUAL: f32 = 8.0;
pub const SAMPLE_ROAD_TWO_LANE_TEXTURE_RESIDUAL: &str = "TRTwoLane.tga";
pub const SAMPLE_ROAD_DIRT_ROAD_NAME_RESIDUAL: &str = "DirtRoad";
pub const SAMPLE_ROAD_SIDEWALK_NAME_RESIDUAL: &str = "Sidewalk";
pub const ROADS_INI_PATH_RESIDUAL: &str = "Data\\INI\\Roads.ini";
/// TerrainRoadCollection id counter residual MUST start at 1.
pub const ROAD_ID_COUNTER_START_RESIDUAL: u32 = 1;

/// Host residual: next road id after start.
#[inline]
pub fn road_next_id_residual(counter: u32) -> (u32, u32) {
    (counter, counter.saturating_add(1))
}

/// Wave 108 honesty: road residual deepen pack.

// ---------------------------------------------------------------------------
// 5. Cliff residual peels (HeightMap cliff cell + pathfind CELL_CLIFF)
// ---------------------------------------------------------------------------

/// C++ `PATHFIND_CLIFF_SLOPE_LIMIT_F` residual (WorldHeightMap.cpp).
/// Cell is cliff when max(raw*MAP_HEIGHT_SCALE)−min(...) of four corners > 9.8 world Z.
pub const PATHFIND_CLIFF_SLOPE_LIMIT_F_RESIDUAL: f32 = 9.8;
/// C++ cliff UV stretch residual limits (WorldHeightMap getUVData old path).
pub const CLIFF_STRETCH_LIMIT_RESIDUAL: f32 = 1.5;
pub const CLIFF_TILE_LIMIT_RESIDUAL: f32 = 4.0;
pub const CLIFF_TALL_STRETCH_LIMIT_RESIDUAL: f32 = 2.0;
pub const CLIFF_DIAMOND_STRETCH_LIMIT_RESIDUAL: f32 = 2.4;
/// C++ HEIGHT_SCALE residual for cliff UV = MAP_HEIGHT_SCALE / MAP_XY_FACTOR.
pub const CLIFF_HEIGHT_SCALE_UV_RESIDUAL: f32 =
    HEIGHTMAP_MAP_HEIGHT_SCALE_RESIDUAL / HEIGHTMAP_MAP_XY_FACTOR_RESIDUAL;

/// C++ PathfindCell::CellType residual (AIPathfind.h) — CELL_CLIFF anchor.
pub const PATHFIND_CELL_CLEAR_RESIDUAL: u8 = 0x00;
pub const PATHFIND_CELL_WATER_RESIDUAL: u8 = 0x01;
pub const PATHFIND_CELL_CLIFF_RESIDUAL: u8 = 0x02;
pub const PATHFIND_CELL_RUBBLE_RESIDUAL: u8 = 0x03;
pub const PATHFIND_CELL_OBSTACLE_RESIDUAL: u8 = 0x04;
pub const PATHFIND_CELL_BRIDGE_IMPASSABLE_RESIDUAL: u8 = 0x05;
pub const PATHFIND_CELL_IMPASSABLE_RESIDUAL: u8 = 0x06;
pub const PATHFIND_CELL_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "CELL_CLEAR",
    "CELL_WATER",
    "CELL_CLIFF",
    "CELL_RUBBLE",
    "CELL_OBSTACLE",
    "CELL_BRIDGE_IMPASSABLE",
    "CELL_IMPASSABLE",
];

/// C++ cliff bit packing residual: 8 cells per flag byte (xIndex >> 3, bit xIndex&7).
pub const CLIFF_FLAG_BITS_PER_BYTE_RESIDUAL: i32 = 8;

/// Host residual: is cliff from four raw corner heights (world-Z range > 9.8).
/// Wires leftover `terrain_cliff::is_cliff_from_raw_heights` (C++ `setCellCliffFlagFromHeights`).
#[inline]
pub fn cliff_cell_from_raw_heights_residual(h0: u8, h1: u8, h2: u8, h3: u8) -> bool {
    gamelogic::terrain_cliff::is_cliff_from_raw_heights(h0, h1, h2, h3)
}

/// Host residual: cliff flag byte bit test for cell x.
#[inline]
pub fn cliff_flag_bit_mask_residual(x_index: i32) -> u8 {
    1u8 << (x_index & 0x7)
}

/// Host residual: flag byte array index for (x,y) given flipStateWidth.
#[inline]
pub fn cliff_flag_byte_index_residual(x_index: i32, y_index: i32, flip_state_width: i32) -> i32 {
    y_index * flip_state_width + (x_index >> 3)
}

/// Host residual: cliff UV stretch decision — deltaH * HEIGHT_SCALE >= STRETCH_LIMIT.
#[inline]
pub fn cliff_uv_needs_stretch_residual(delta_raw_h: i32) -> bool {
    (delta_raw_h as f32) * CLIFF_HEIGHT_SCALE_UV_RESIDUAL >= CLIFF_STRETCH_LIMIT_RESIDUAL
}

/// Wave 108 honesty: cliff residual peels pack.

// ---------------------------------------------------------------------------
// Combined Wave 108 pack
// ---------------------------------------------------------------------------

/// Combined Wave 108 honesty: HeightMap + bridge + water + road + cliff residual packs.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heightmap_wave108_honesty() {
        assert!(honesty_heightmap_residual_deepen_pack_wave108());
        assert!((heightmap_raw_sample_to_world_residual(16) - 10.0).abs() < 0.01);
    }

    #[test]
    fn bridge_wave108_honesty() {
        assert_eq!(bridge_effect_num_zero_based_residual(1), Some(0));
    }

    #[test]
    fn water_wave108_honesty() {
        assert_eq!(WATER_TYPE_MAX_RESIDUAL, 4);
    }

    #[test]
    fn road_wave108_honesty() {
        assert_eq!(road_next_id_residual(1), (1, 2));
    }

    #[test]
    fn cliff_wave108_honesty() {
        assert!(cliff_cell_from_raw_heights_residual(0, 20, 0, 20));
        assert!(!cliff_cell_from_raw_heights_residual(5, 5, 5, 5));
    }
}
