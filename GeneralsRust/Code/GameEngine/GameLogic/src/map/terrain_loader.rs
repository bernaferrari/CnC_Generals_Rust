//! Terrain Data Loader
//!
//! Parses terrain-specific data from the binary map and converts it into the
//! structures expected by `TerrainLogic`:
//!   - Heightmap (8-bit elevation grid)
//!   - Texture blend layers (splat-map indices)
//!   - Water areas and global water height
//!   - Road networks (stored as map-object road-point flags)
//!   - Bridge placements
//!
//! This module works on the already-parsed `MapData` produced by
//! `system::map_loader::MapLoader` and does **not** perform I/O itself.

use crate::common::*;
use crate::system::map_loader::{BridgeData, Coord3D as SysCoord3D, ICoord2D, MapData};

// ---------------------------------------------------------------------------
// Terrain tile descriptor
// ---------------------------------------------------------------------------

/// Describes a single terrain tile's visual and gameplay properties.
///
/// In the C++ engine these are populated from the `TerrainType` INI blocks
/// and then indexed by the heightmap / blend layers.  This struct captures
/// the subset relevant to the game-logic side; the client-facing visual
/// details live in the `TerrainVisual` counterpart.
#[derive(Debug, Clone)]
pub struct TerrainTile {
    /// Base texture index (into the terrain texture set).
    pub texture_index: u8,
    /// Blend weight for the second texture layer (0-255).
    pub blend: u8,
    /// Surface type flags (e.g. CLIFF, WATER, ROAD).
    pub surface_flags: u8,
    /// Raw heightmap value at this cell.
    pub height: u8,
}

impl Default for TerrainTile {
    fn default() -> Self {
        Self {
            texture_index: 0,
            blend: 0,
            surface_flags: 0,
            height: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Water area descriptor
// ---------------------------------------------------------------------------

/// A named water area extracted from polygon triggers or the WorldInfo dict.
#[derive(Debug, Clone)]
pub struct WaterArea {
    /// Polygon trigger name (if any).
    pub name: String,
    /// Whether this water body is a river (vs. standing water).
    pub is_river: bool,
    /// Water height (Z value in world coordinates).
    pub height: f32,
    /// Polygon vertices in world coordinates.
    pub vertices: Vec<SysCoord3D>,
}

// ---------------------------------------------------------------------------
// Road segment descriptor
// ---------------------------------------------------------------------------

/// A single road or bridge segment defined by two endpoint map objects.
///
/// In the original C++ engine these are encoded as `MapObjectFlags`:
///   - `ROAD_POINT1` / `ROAD_POINT2`  for roads
///   - `BRIDGE_POINT1` / `BRIDGE_POINT2` for bridges
/// The map file stores a list of objects with these flags, and segments are
/// formed by pairing consecutive point-1 / point-2 objects with the same name.
#[derive(Debug, Clone)]
pub struct RoadSegment {
    /// Road template name (matches a `TerrainRoadType` in INI).
    pub road_type: String,
    /// Start position in world coordinates.
    pub from: SysCoord3D,
    /// End position in world coordinates.
    pub to: SysCoord3D,
    /// Whether this is actually a bridge.
    pub is_bridge: bool,
}

// ---------------------------------------------------------------------------
// Parsed terrain summary
// ---------------------------------------------------------------------------

/// All terrain data extracted from a loaded map, in a form suitable for
/// feeding into `TerrainLogic`.
#[derive(Debug, Clone)]
pub struct ParsedTerrain {
    /// Heightmap grid (row-major, `width * height` entries).
    pub heightmap: Vec<u8>,
    /// Grid width in cells (excluding border).
    pub width: u32,
    /// Grid height in cells (excluding border).
    pub height: u32,
    /// Border width in cells.
    pub border_size: i32,
    /// Boundary polygon points.
    pub boundaries: Vec<ICoord2D>,
    /// Per-tile terrain descriptors (same grid size as heightmap).
    pub tiles: Vec<TerrainTile>,
    /// Bridge structures.
    pub bridges: Vec<BridgeData>,
    /// Road segments.
    pub roads: Vec<RoadSegment>,
    /// Water areas.
    pub water_areas: Vec<WaterArea>,
    /// Global water table height (standing water outside explicit areas).
    pub global_water_height: Option<f32>,
}

impl Default for ParsedTerrain {
    fn default() -> Self {
        Self {
            heightmap: Vec::new(),
            width: 0,
            height: 0,
            border_size: 0,
            boundaries: Vec::new(),
            tiles: Vec::new(),
            bridges: Vec::new(),
            roads: Vec::new(),
            water_areas: Vec::new(),
            global_water_height: None,
        }
    }
}

// ---------------------------------------------------------------------------
// TerrainLoader
// ---------------------------------------------------------------------------

/// Stateless helper that converts raw `MapData` into `ParsedTerrain`.
pub struct TerrainLoader;

impl TerrainLoader {
    /// Build a `ParsedTerrain` from a loaded `MapData`.
    ///
    /// The returned struct can be handed directly to `TerrainLogic` via
    /// `load_map_data`.
    pub fn parse(data: &MapData) -> ParsedTerrain {
        let tile_count = (data.width as usize)
            .checked_mul(data.height as usize)
            .unwrap_or(0);

        // Build per-tile descriptors from the heightmap + texture_tiles.
        let mut tiles = Vec::with_capacity(tile_count);
        for i in 0..tile_count {
            let height = data.heightmap.get(i).copied().unwrap_or(0);
            let texture_index = data.texture_tiles.get(i).copied().unwrap_or(0);
            tiles.push(TerrainTile {
                texture_index,
                blend: 0, // blend layer not yet decoded from binary format
                surface_flags: Self::classify_surface(height),
                height,
            });
        }

        // Extract water areas from polygon triggers.
        let water_areas = Self::extract_water_areas(data);

        ParsedTerrain {
            heightmap: data.heightmap.clone(),
            width: data.width,
            height: data.height,
            border_size: data.border_size,
            boundaries: data.boundaries.clone(),
            tiles,
            bridges: data.bridges.clone(),
            roads: Vec::new(), // road segments require full object-list parsing
            water_areas,
            global_water_height: data.water_height,
        }
    }

    /// Classify the surface type of a cell based on its heightmap value.
    ///
    /// In the C++ engine the classification is more nuanced (cliff detection,
    /// slope analysis, etc.).  This is a simplified version suitable for the
    /// game-logic side.
    fn classify_surface(height: u8) -> u8 {
        if height == 0 {
            SURFACE_WATER
        } else {
            SURFACE_GROUND
        }
    }

    /// Scan polygon triggers for water areas.
    fn extract_water_areas(data: &MapData) -> Vec<WaterArea> {
        let mut areas = Vec::new();

        for trigger in &data.polygon_triggers {
            if !trigger.is_water_area() {
                continue;
            }

            // Attempt to determine water height from the polygon.
            // In the C++ engine the height is taken from the first point Z
            // component or from a water-setting INI block.
            let height = trigger
                .get_point(0)
                .map(|p| p.z as f32)
                .unwrap_or(data.water_height.unwrap_or(0.0));

            // Collect all points as world-space coordinates.
            let all_vertices: Vec<SysCoord3D> = (0..trigger.get_num_points())
                .filter_map(|i| {
                    trigger.get_point(i).map(|p| {
                        SysCoord3D::new(
                            p.x as f32 * crate::system::map_loader::MAP_XY_FACTOR,
                            p.y as f32 * crate::system::map_loader::MAP_XY_FACTOR,
                            p.z as f32,
                        )
                    })
                })
                .collect();

            areas.push(WaterArea {
                name: trigger.get_trigger_name().to_string(),
                is_river: trigger.is_river(),
                height,
                vertices: all_vertices,
            });
        }

        areas
    }
}

// ---------------------------------------------------------------------------
// Surface-type constants
// ---------------------------------------------------------------------------

/// Surface is water (heightmap value == 0 or below water table).
const SURFACE_WATER: u8 = 0x01;
/// Surface is normal ground.
const SURFACE_GROUND: u8 = 0x00;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_map_data() -> MapData {
        MapData {
            width: 64,
            height: 64,
            heightmap: vec![128u8; 64 * 64],
            water_height: Some(10.0),
            bridges: Vec::new(),
            texture_tiles: vec![1u8; 64 * 64],
            boundaries: vec![
                ICoord2D::new(0, 0),
                ICoord2D::new(63, 0),
                ICoord2D::new(63, 63),
                ICoord2D::new(0, 63),
            ],
            border_size: 3,
            polygon_triggers: Vec::new(),
            waypoints: Vec::new(),
            waypoint_links: Vec::new(),
        }
    }

    #[test]
    fn test_parse_basic_terrain() {
        let data = sample_map_data();
        let terrain = TerrainLoader::parse(&data);

        assert_eq!(terrain.width, 64);
        assert_eq!(terrain.height, 64);
        assert_eq!(terrain.border_size, 3);
        assert_eq!(terrain.heightmap.len(), 64 * 64);
        assert_eq!(terrain.tiles.len(), 64 * 64);
        assert_eq!(terrain.global_water_height, Some(10.0));
    }

    #[test]
    fn test_terrain_tile_properties() {
        let data = sample_map_data();
        let terrain = TerrainLoader::parse(&data);

        let tile = &terrain.tiles[0];
        assert_eq!(tile.height, 128);
        assert_eq!(tile.texture_index, 1);
        assert_eq!(tile.surface_flags, SURFACE_GROUND);
    }

    #[test]
    fn test_classify_water_surface() {
        assert_eq!(TerrainLoader::classify_surface(0), SURFACE_WATER);
        assert_eq!(TerrainLoader::classify_surface(128), SURFACE_GROUND);
    }

    #[test]
    fn test_empty_map_data() {
        let data = MapData::new();
        let terrain = TerrainLoader::parse(&data);

        assert_eq!(terrain.width, 0);
        assert_eq!(terrain.height, 0);
        assert!(terrain.heightmap.is_empty());
        assert!(terrain.tiles.is_empty());
    }

    #[test]
    fn test_water_areas_extraction() {
        let mut data = sample_map_data();

        use crate::polygon_trigger::PolygonTrigger;
        let mut trigger = PolygonTrigger::new(
            1,
            "Lake1".to_string().into(),
            vec![
                crate::common::ICoord3D::new(0, 0, 10),
                crate::common::ICoord3D::new(100, 0, 10),
                crate::common::ICoord3D::new(100, 100, 10),
                crate::common::ICoord3D::new(0, 100, 10),
            ],
        );
        trigger.set_water_area(true);

        data.polygon_triggers.push(trigger);

        let terrain = TerrainLoader::parse(&data);
        assert_eq!(terrain.water_areas.len(), 1);
        assert_eq!(terrain.water_areas[0].name, "Lake1");
        assert_eq!(terrain.water_areas[0].height, 10.0);
        assert!(!terrain.water_areas[0].is_river);
    }

    #[test]
    fn test_parsed_terrain_default() {
        let terrain = ParsedTerrain::default();
        assert!(terrain.heightmap.is_empty());
        assert_eq!(terrain.width, 0);
        assert!(terrain.bridges.is_empty());
        assert!(terrain.water_areas.is_empty());
    }
}
