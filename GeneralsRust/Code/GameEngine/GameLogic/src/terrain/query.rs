//! TerrainQuery integration for the logical terrain system.

use super::*;

impl TerrainLogic {
    // Private helper methods

    /// Check if point is inside a bridge polygon using point-in-polygon test
    /// Reference: C++ TerrainLogic.cpp Bridge::isPointOnBridge()
    ///
    /// Uses ray casting algorithm for polygon containment test
    fn is_point_in_bridge(&self, x: f32, y: f32) -> Option<&crate::system::map_loader::BridgeData> {
        let terrain_data = self.terrain_data.as_ref()?;

        for bridge in &terrain_data.bridges {
            if self.point_in_polygon(x, y, &bridge.polygon) {
                return Some(bridge);
            }
        }

        None
    }

    /// Point-in-polygon test using ray casting algorithm
    /// Reference: Standard computational geometry algorithm
    ///
    /// # Arguments
    /// * `x` - X coordinate of point to test
    /// * `y` - Y coordinate of point to test
    /// * `polygon` - Polygon vertices (must be closed, first != last)
    ///
    /// # Returns
    /// true if point is inside polygon, false otherwise
    fn point_in_polygon(
        &self,
        x: f32,
        y: f32,
        polygon: &[crate::system::map_loader::Coord2D],
    ) -> bool {
        if polygon.len() < 3 {
            return false;
        }

        let mut inside = false;
        let n = polygon.len();

        let mut j = n - 1;
        for i in 0..n {
            let xi = polygon[i].x;
            let yi = polygon[i].y;
            let xj = polygon[j].x;
            let yj = polygon[j].y;

            // Ray casting algorithm
            let intersect = ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);

            if intersect {
                inside = !inside;
            }

            j = i;
        }

        inside
    }

    /// Calculate terrain slope at position
    /// Reference: TerrainLogic.cpp lines 190-234 getTerrainSlope()
    ///
    /// Algorithm:
    /// 1. Sample height at 4 neighboring points
    /// 2. Calculate gradient vectors
    /// 3. Compute slope angle from gradient magnitude
    fn calculate_slope(&self, x: Real, y: Real) -> Real {
        const SAMPLE_OFFSET: Real = 1.0; // 1 world unit offset for gradient calculation

        // Sample heights at 4 neighboring points
        // C++ TerrainLogic.cpp lines 195-198
        let _h_center = self.get_ground_height(x, y, None);
        let h_north = self.get_ground_height(x, y + SAMPLE_OFFSET, None);
        let h_south = self.get_ground_height(x, y - SAMPLE_OFFSET, None);
        let h_east = self.get_ground_height(x + SAMPLE_OFFSET, y, None);
        let h_west = self.get_ground_height(x - SAMPLE_OFFSET, y, None);

        // Calculate gradients in X and Y directions
        // C++ TerrainLogic.cpp lines 200-201
        let gradient_x = (h_east - h_west) / (2.0 * SAMPLE_OFFSET);
        let gradient_y = (h_north - h_south) / (2.0 * SAMPLE_OFFSET);

        // Calculate slope magnitude
        // C++ TerrainLogic.cpp lines 203-204
        let gradient_magnitude = (gradient_x * gradient_x + gradient_y * gradient_y).sqrt();

        // Convert to degrees
        // C++ TerrainLogic.cpp line 206
        let slope_radians = gradient_magnitude.atan();
        let slope_degrees = slope_radians.to_degrees();

        slope_degrees
    }

    /// Get surface type at world position
    /// Maps terrain to SurfaceType enum for physics queries
    fn get_surface_type_at(&self, x: f32, y: f32) -> SurfaceType {
        // Check if underwater first
        let mut water_z = 0.0;
        let mut terrain_z = 0.0;
        if self.is_underwater(x, y, Some(&mut water_z), Some(&mut terrain_z)) {
            return SurfaceType::Water;
        }

        // Check if on bridge
        let pos = Coord3D::new(x, y, terrain_z);
        if let Some(_bridge) = self.find_bridge_at(&pos) {
            return SurfaceType::Bridge;
        }

        // Check slope for cliff detection
        let slope = self.calculate_slope(x, y);
        const CLIFF_THRESHOLD: Real = 45.0;
        if slope >= CLIFF_THRESHOLD {
            return SurfaceType::Cliff;
        }

        // Default to ground
        SurfaceType::Ground
    }

    /// Get water depth at position (0.0 if no water)
    /// Reference: TerrainLogic.cpp lines 157-189 getWaterDepth()
    fn get_water_depth_at(&self, x: f32, y: f32) -> f32 {
        let mut water_z = 0.0;
        let mut terrain_z = 0.0;
        if self.is_underwater(x, y, Some(&mut water_z), Some(&mut terrain_z)) {
            water_z - terrain_z
        } else {
            0.0
        }
    }
}

/// Implement TerrainQuery trait for PhysicsEngine integration
/// Reference: TerrainLogic.cpp matching C++ interface
impl TerrainQuery for TerrainLogic {
    /// Get ground height at position
    /// Reference: TerrainLogic.cpp lines 44-156 getGroundHeight()
    fn get_ground_height(&self, x: Real, y: Real) -> Real {
        self.get_ground_height(x, y, None)
    }

    /// Get water depth at position
    /// Reference: TerrainLogic.cpp lines 157-189 getWaterDepth()
    fn get_water_depth(&self, x: Real, y: Real) -> Real {
        self.get_water_depth_at(x, y)
    }

    /// Get terrain slope angle at position (in degrees)
    /// Reference: TerrainLogic.cpp lines 190-234 getTerrainSlope()
    fn get_terrain_slope(&self, x: Real, y: Real) -> Real {
        self.calculate_slope(x, y)
    }

    /// Check if position is on a bridge
    /// Reference: TerrainLogic.cpp lines 235-278 isOnBridge()
    ///
    /// This implementation uses the loaded bridge data from the map file
    /// and performs point-in-polygon tests to determine if a position
    /// is on any bridge surface.
    fn is_on_bridge(&self, pos: &Coord3D) -> (Bool, Real) {
        // First try the old bridge list (for compatibility)
        if let Some(bridge) = self.find_bridge_at(pos) {
            let height = bridge.get_bridge_height(pos, None);
            return (true, height);
        }

        // Then check loaded bridge data from map file
        if let Some(bridge_data) = self.is_point_in_bridge(pos.x, pos.y) {
            let height = bridge_data.get_height_at(pos.x, pos.y);
            return (true, height);
        }

        (false, 0.0)
    }

    /// Check if position is a cliff (steep slope)
    /// Reference: TerrainLogic.cpp lines 279-298 isCliff()
    fn is_cliff(&self, pos: &Coord3D) -> Bool {
        const CLIFF_THRESHOLD: Real = 45.0;
        let slope = self.calculate_slope(pos.x, pos.y);
        slope >= CLIFF_THRESHOLD
    }

    /// Get surface type at position
    /// Reference: TerrainLogic.cpp lines 299-324 getSurfaceType()
    fn get_surface_type(&self, x: Real, y: Real) -> SurfaceType {
        self.get_surface_type_at(x, y)
    }
}

/// Wrapper to make Arc<RwLock<TerrainLogic>> implement TerrainQuery
/// This allows the global terrain instance to be used by the physics engine
#[derive(Clone)]
pub struct TerrainQueryWrapper(Arc<RwLock<TerrainLogic>>);

impl TerrainQueryWrapper {
    pub fn new(terrain: Arc<RwLock<TerrainLogic>>) -> Self {
        Self(terrain)
    }
}

impl TerrainQuery for TerrainQueryWrapper {
    fn get_ground_height(&self, x: Real, y: Real) -> Real {
        if let Ok(terrain) = self.0.read() {
            terrain.get_ground_height(x, y, None)
        } else {
            0.0
        }
    }

    fn get_water_depth(&self, x: Real, y: Real) -> Real {
        if let Ok(terrain) = self.0.read() {
            terrain.get_water_depth_at(x, y)
        } else {
            0.0
        }
    }

    fn get_terrain_slope(&self, x: Real, y: Real) -> Real {
        if let Ok(terrain) = self.0.read() {
            terrain.calculate_slope(x, y)
        } else {
            0.0
        }
    }

    fn is_on_bridge(&self, pos: &Coord3D) -> (Bool, Real) {
        if let Ok(terrain) = self.0.read() {
            // First try the old bridge list (for compatibility)
            if let Some(bridge) = terrain.find_bridge_at(pos) {
                let height = bridge.get_bridge_height(pos, None);
                return (true, height);
            }

            // Then check loaded bridge data from map file
            if let Some(bridge_data) = terrain.is_point_in_bridge(pos.x, pos.y) {
                let height = bridge_data.get_height_at(pos.x, pos.y);
                return (true, height);
            }
        }
        (false, 0.0)
    }

    fn is_cliff(&self, pos: &Coord3D) -> Bool {
        const CLIFF_THRESHOLD: Real = 45.0;
        if let Ok(terrain) = self.0.read() {
            let slope = terrain.calculate_slope(pos.x, pos.y);
            return slope >= CLIFF_THRESHOLD;
        }
        false
    }

    fn get_surface_type(&self, x: Real, y: Real) -> SurfaceType {
        if let Ok(terrain) = self.0.read() {
            terrain.get_surface_type_at(x, y)
        } else {
            SurfaceType::Ground
        }
    }
}
