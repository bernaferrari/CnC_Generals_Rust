//! Terrain road/bridge definitions (wrapper over Common ini_road).

use game_engine::common::ini::ini_road as common_roads;

pub use common_roads::{
    BodyDamageType, BridgeTowerType, RGBColor, TerrainRoadCollection, TerrainRoadType,
    MAX_BRIDGE_BODY_FX,
};

/// Access the global terrain road collection (mirrors TheTerrainRoads).
pub fn terrain_roads() -> std::sync::RwLockReadGuard<'static, TerrainRoadCollection> {
    common_roads::get_terrain_roads()
}

/// Mutable access to the global terrain road collection.
pub fn terrain_roads_mut() -> std::sync::RwLockWriteGuard<'static, TerrainRoadCollection> {
    common_roads::get_terrain_roads_mut()
}
