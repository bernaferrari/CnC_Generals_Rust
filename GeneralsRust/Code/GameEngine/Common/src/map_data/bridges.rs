//! Bridge marker pairing, authored deck dimensions, and model lookup.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::common::dict::Dict;
use crate::common::system::geometry::Coord2D;
use crate::common::system::{file::FileAccess, file_system::FileSystem};
use crate::map_object::{FLAG_BRIDGE_POINT1, FLAG_BRIDGE_POINT2, MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
use glam::Vec3;
use ww3d_core::model_geometry::rigid_section_lateral_bounds;

// W3DBridgeBuffer.h, maximum number of admitted bridges.
pub const MAX_AUTHORED_BRIDGES: usize = 200;

/// Bridge data extracted from map files
/// Corresponds to BridgeInfo from C++ TerrainLogic.h
#[derive(Debug, Clone)]
pub struct BridgeData {
    /// The 4 corners of the rectangle that the bridge covers
    pub polygon: Vec<Coord2D>,
    /// Bridge deck height (interpolated between from.z and to.z)
    pub height: f32,
    /// Bridge endpoints
    pub from: Vec3,
    pub to: Vec3,
    /// Bridge width
    pub width: f32,
    /// Bridge template name
    pub template_name: String,
    /// Object properties from the first marker in W3DBridgeBuffer::loadBridges.
    pub properties: Dict,
}

/// The two adjacent authored map markers for one bridge.
#[derive(Debug)]
pub struct BridgeEndpoints {
    pub template_name: String,
    pub from: Vec3,
    pub to: Vec3,
    pub properties: Dict,
}

/// W3DBridgeBuffer::loadBridges walks consecutive WorldHeightMap objects.
#[derive(Default)]
pub struct BridgeMarkers {
    start: Option<(String, Vec3, Dict)>,
}

impl BridgeMarkers {
    pub fn observe(
        &mut self,
        flags: u32,
        name: &str,
        position: Vec3,
        properties: &Dict,
    ) -> Option<BridgeEndpoints> {
        // WorldHeightMap::ParseObjectData skips these objects before linking.
        const MIN_OBJECT_Z: f32 = -100.0 * MAP_XY_FACTOR;
        const MAX_OBJECT_Z: f32 = 2550.0 * MAP_HEIGHT_SCALE;
        if position.z < MIN_OBJECT_Z || position.z > MAX_OBJECT_Z {
            return None;
        }

        if let Some((template_name, from, properties)) = self.start.take()
            && flags & FLAG_BRIDGE_POINT2 != 0
        {
            return Some(BridgeEndpoints {
                template_name,
                from,
                to: position,
                properties,
            });
        }
        if flags & FLAG_BRIDGE_POINT1 != 0 {
            self.start = Some((name.to_owned(), position, properties.clone()));
        }
        None
    }
}

impl BridgeData {
    /// Create bridge data from endpoints and width
    /// Reference: C++ TerrainLogic.cpp:196-304 Bridge::Bridge() constructor
    ///
    /// # Arguments
    /// * `from` - Starting point of bridge
    /// * `to` - Ending point of bridge
    /// * `width` - Width of the bridge
    /// * `template_name` - Bridge template name
    ///
    /// # Returns
    /// BridgeData structure with calculated polygon
    pub fn new(from: Vec3, to: Vec3, width: f32, template_name: String) -> Self {
        let half_width = width / 2.0;
        Self::from_edges(
            BridgeEndpoints {
                from,
                to,
                template_name,
                properties: Dict::new(),
            },
            -half_width,
            half_width,
            1.0,
            width,
        )
    }

    // W3DBridgeBuffer.cpp, W3DBridge::getBridgeInfo.
    fn from_model_bounds(span: BridgeEndpoints, min: f32, max: f32, scale: f32) -> Self {
        Self::from_edges(span, min, max, scale, (max - min) * scale)
    }

    fn from_edges(
        BridgeEndpoints {
            from,
            to,
            template_name,
            properties,
        }: BridgeEndpoints,
        min: f32,
        max: f32,
        scale: f32,
        width: f32,
    ) -> Self {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let length_squared = dx * dx + dy * dy;
        // Vector3::Normalize in C++ leaves the zero vector alone.
        let inverse_length = if length_squared != 0.0 {
            1.0 / length_squared.sqrt()
        } else {
            1.0
        };
        let (perp_x, perp_y) = (-dy * inverse_length, dx * inverse_length);
        let edge = |point: Vec3, offset: f32| {
            Coord2D::new(
                point.x + perp_x * offset * scale,
                point.y + perp_y * offset * scale,
            )
        };

        Self {
            polygon: vec![
                edge(from, max),
                edge(from, min),
                edge(to, min),
                edge(to, max),
            ],
            height: (from.z + to.z) / 2.0,
            from,
            to,
            width,
            template_name,
            properties,
        }
    }

    /// Get interpolated height at position along bridge
    /// Reference: C++ TerrainLogic.cpp Bridge::getBridgeHeight()
    pub fn get_height_at(&self, x: f32, y: f32) -> f32 {
        // Calculate position along bridge (0.0 to 1.0)
        let bridge_dx = self.to.x - self.from.x;
        let bridge_dy = self.to.y - self.from.y;
        let bridge_length_sq = bridge_dx * bridge_dx + bridge_dy * bridge_dy;

        if bridge_length_sq == 0.0 {
            return self.from.z;
        }

        let pos_dx = x - self.from.x;
        let pos_dy = y - self.from.y;
        let dot = pos_dx * bridge_dx + pos_dy * bridge_dy;
        let t = (dot / bridge_length_sq).clamp(0.0, 1.0);

        // Linear interpolation between from.z and to.z
        self.from.z + (self.to.z - self.from.z) * t
    }
}

/// Per-map model bounds. Spans using the same model share one asset read.
#[derive(Default)]
pub struct BridgeModels {
    bounds: HashMap<String, Option<(f32, f32)>>,
}

impl BridgeModels {
    pub fn deck(
        &mut self,
        span: BridgeEndpoints,
        model: &str,
        scale: f32,
        read_model: impl FnOnce(&str) -> Result<Vec<u8>, String>,
    ) -> Option<BridgeData> {
        let model = model.to_ascii_lowercase();
        let &(min, max) = self
            .bounds
            .entry(model)
            .or_insert_with_key(|model| match load_section_bounds(model, read_model) {
                Ok(bounds) => Some(bounds),
                Err(error) => {
                    log::warn!(
                        "Map bridge '{}' could not load model '{model}': {error}",
                        span.template_name
                    );
                    None
                }
            })
            .as_ref()?;
        Some(BridgeData::from_model_bounds(span, min, max, scale))
    }
}

fn load_section_bounds(
    model: &str,
    read_model: impl FnOnce(&str) -> Result<Vec<u8>, String>,
) -> Result<(f32, f32), String> {
    let stem = model
        .split('.')
        .next()
        .filter(|stem| !stem.is_empty())
        .ok_or("missing BridgeModelName")?;
    let path = format!("art/w3d/{stem}.w3d");
    let bytes = read_model(&path)?;
    rigid_section_lateral_bounds(&bytes, model, "BRIDGE_LEFT").map_err(|error| error.to_string())
}

/// Open the file through a receiving application's mounts.
pub fn read_mounted_bridge_model(files: &Mutex<FileSystem>, path: &str) -> Result<Vec<u8>, String> {
    let mut file = files
        .lock()
        .map_err(|error| error.to_string())?
        .open_mounted_file(path, FileAccess::READ.combine(FileAccess::BINARY))
        .ok_or_else(|| format!("missing {path}"))?;
    file.read_entire_and_close()
        .map_err(|error| error.to_string())
}
