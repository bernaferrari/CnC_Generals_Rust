//! Authored map data shared by simulation, terrain, and presentation.

use std::borrow::Cow;
use std::io;

use crate::common::system::compression::{decompress_data, is_data_compressed};
use crate::common::system::geometry::{Coord3D as ExtentPoint, Region3D};
use crate::map_object::MAP_XY_FACTOR;
use crate::polygon_trigger::PolygonTrigger;
use glam::{IVec2, Vec3};

mod bridges;
mod object_chunk;
pub use object_chunk::MapObjectChunk;
pub use bridges::{
    BridgeData, BridgeEndpoints, BridgeMarkers, BridgeModels, MAX_AUTHORED_BRIDGES,
    read_mounted_bridge_model,
};

/// Height map data structure
/// Matches C++ heightmap from MapUtil.cpp
#[derive(Debug)]
pub struct HeightMap {
    pub width: i32,
    pub height: i32,
    pub border_size: i32,
    pub boundaries: Vec<IVec2>,
    pub data: Vec<u8>,
}

impl HeightMap {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            border_size: 0,
            boundaries: Vec::new(),
            data: Vec::new(),
        }
    }

    /// Apply the version-one heightmap sampling step.
    pub fn downsample_version_one(&mut self) {
        let (width, height, data) =
            generals_map::downsample_version_one(self.width, self.height, &self.data);
        self.width = width;
        self.height = height;
        self.data = data;
    }

    /// Get playable map dimensions (excluding border)
    pub fn get_playable_dimensions(&self) -> (i32, i32) {
        let dx = self.width - 2 * self.border_size;
        let dy = self.height - 2 * self.border_size;
        (dx, dy)
    }

    /// Calculate map extent in world coordinates
    /// Matches C++ getExtent() from MapUtil.cpp
    pub fn get_extent(&self) -> Region3D {
        let (dx, dy) = self.get_playable_dimensions();

        Region3D::new(
            ExtentPoint::ZERO,
            ExtentPoint::new(dx as f32 * MAP_XY_FACTOR, dy as f32 * MAP_XY_FACTOR, 0.0),
        )
    }
}

/// Complete map data loaded from .map files
/// Matches data needed by TerrainLogic
#[derive(Debug, Clone)]
pub struct MapData {
    /// Map width in cells
    pub width: u32,
    /// Map height in cells
    pub height: u32,
    /// Raw height values (8-bit heightmap)
    pub heightmap: Vec<u8>,
    /// Global water table height (if any)
    pub water_height: Option<f32>,
    /// Bridge structures extracted from map objects
    pub bridges: Vec<BridgeData>,
    /// Surface type per tile (for terrain texture mapping)
    pub texture_tiles: Vec<u8>,
    /// Map boundaries
    pub boundaries: Vec<IVec2>,
    /// Border size
    pub border_size: i32,
    /// Polygon trigger areas for scripts and water
    pub polygon_triggers: Vec<PolygonTrigger>,
    /// Waypoints extracted from map objects
    pub waypoints: Vec<MapWaypoint>,
    /// Waypoint link pairs (id1, id2)
    pub waypoint_links: Vec<(u32, u32)>,
}

impl MapData {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            heightmap: Vec::new(),
            water_height: None,
            bridges: Vec::new(),
            texture_tiles: Vec::new(),
            boundaries: Vec::new(),
            border_size: 0,
            polygon_triggers: Vec::new(),
            waypoints: Vec::new(),
            waypoint_links: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapWaypoint {
    pub id: u32,
    pub name: String,
    pub location: Vec3,
    pub path_label1: String,
    pub path_label2: String,
    pub path_label3: String,
    pub bi_directional: bool,
}

/// Open a raw or compressed map's chunky payload.
pub fn uncompressed_map_bytes(bytes: &[u8]) -> io::Result<Cow<'_, [u8]>> {
    if is_data_compressed(bytes) {
        return decompress_data(bytes).map(Cow::Owned);
    }
    Ok(Cow::Borrowed(bytes))
}
