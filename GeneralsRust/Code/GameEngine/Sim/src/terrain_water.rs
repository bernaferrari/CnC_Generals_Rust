//! Water mesh sampling and authored configuration for the receiving world.
use glam::{Mat4, Vec3};
use std::collections::HashMap;

/// Mutable water mesh and authored settings for the receiving simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct WaterGridState {
    pub enabled: bool,
    pub transform: Mat4,
    pub resolution: (f32, f32, f32),
    pub height_clamps: (f32, f32),
    pub attenuation: (f32, f32, f32, f32),
    pub height_deltas: HashMap<(i32, i32), f32>,
    pub mesh_motion: HashMap<(i32, i32), WaterGridMeshMotion>,
}

/// Original WaterMeshData fields accompanying each logical height delta.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaterGridMeshMotion {
    pub velocity: f32,
    pub status: u8,
    pub preferred_height: u8,
}

impl Default for WaterGridState {
    fn default() -> Self {
        Self {
            enabled: false,
            transform: Mat4::IDENTITY,
            resolution: (0.0, 0.0, 1.0),
            height_clamps: (0.0, 0.0),
            attenuation: (0.0, 0.0, 0.0, 0.0),
            height_deltas: HashMap::new(),
            mesh_motion: HashMap::new(),
        }
    }
}

/// The GameData.INI settings for a single map.
pub struct WaterGridSettings {
    pub height_clamps: (f32, f32),
    pub position: Vec3,
    pub angle: f32,
    pub resolution: (f32, f32, f32),
    pub attenuation: (f32, f32, f32, f32),
}

/// The first configured full-path or filename match, including a save-directory
/// remap of the original campaign map.
pub fn water_grid_setting_index(map_name: &str, configured_maps: &[String]) -> Option<usize> {
    let map_name = map_name.trim();
    if map_name.is_empty() {
        return None;
    }
    let map_leaf = map_name.rsplit(['\\', '/']).next().unwrap_or(map_name);
    configured_maps.iter().position(|configured| {
        let configured = configured.trim();
        if configured.is_empty() {
            return false;
        }
        if configured.eq_ignore_ascii_case(map_name) {
            return true;
        }
        let configured_leaf = configured.rsplit(['\\', '/']).next().unwrap_or(configured);
        configured_leaf.eq_ignore_ascii_case(map_leaf)
    })
}

impl WaterGridState {
    pub fn height_at(&self, x: f32, y: f32) -> Option<f32> {
        sample_grid_height(self, x, y)
    }
    pub fn set_resolution(&mut self, x: f32, y: f32, size: f32) {
        self.resolution.2 = size.max(f32::EPSILON);
        // W3DWater.cpp only reallocates when X changes.
        if self.resolution.0 != x {
            self.resolution.0 = x;
            self.resolution.1 = y;
            self.height_deltas.clear();
            self.mesh_motion.clear();
        }
    }
    pub fn set_height(&mut self, height: f32) {
        self.transform.w_axis.z = height;
    }
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let (x, y, size) = self.resolution;
        // TerrainLogic.cpp builds integer corners before transforming them.
        let width = (x * size) as i32 as f32;
        let height = (y * size) as i32 as f32;
        let corners = [
            Vec3::ZERO,
            Vec3::new(width, 0.0, 0.0),
            Vec3::new(0.0, height, 0.0),
            Vec3::new(width, height, 0.0),
        ];
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for corner in corners {
            let point = self.transform.transform_point3(corner);
            lo = lo.min(point);
            hi = hi.max(point);
        }
        (lo, hi)
    }
    /// C++ TerrainLogic::enableWaterGrid applies the authored settings in this order.
    pub fn apply_settings(&mut self, settings: WaterGridSettings) {
        self.height_clamps = settings.height_clamps;
        self.transform =
            Mat4::from_translation(settings.position) * Mat4::from_rotation_z(settings.angle);
        self.set_resolution(
            settings.resolution.0,
            settings.resolution.1,
            settings.resolution.2,
        );
        self.attenuation = settings.attenuation;
        self.enabled = true;
    }
}

fn sample_grid_height(grid: &WaterGridState, world_x: f32, world_y: f32) -> Option<f32> {
    if !grid.enabled {
        return None;
    }
    let (grid_x, grid_y) = world_to_grid(grid, world_x, world_y)?;
    let ix = grid_x as i32;
    let iy = grid_y as i32;
    let base = grid.transform.w_axis.z;
    Some(base + grid.height_deltas.get(&(ix, iy)).copied().unwrap_or(0.0))
}

fn world_to_grid(grid: &WaterGridState, world_x: f32, world_y: f32) -> Option<(f32, f32)> {
    let (grid_cells_x, grid_cells_y, cell_size) = grid.resolution;
    if grid_cells_x < 1.0 || grid_cells_y < 1.0 || cell_size <= 0.0 {
        return None;
    }
    let local = grid
        .transform
        .inverse()
        .transform_point3(Vec3::new(world_x, world_y, 0.0));
    let grid_x = local.x / cell_size;
    let grid_y = local.y / cell_size;
    if grid_x < 0.0 || grid_y < 0.0 || grid_x > grid_cells_x - 1.0 || grid_y > grid_cells_y - 1.0 {
        return None;
    }
    Some((grid_x, grid_y))
}

/// The highest polygon at the query point, with the grid winning a tie.
pub enum WaterSurface {
    Grid(f32),
    Polygon { trigger_id: i32, height: f32 },
}

pub fn select_water_surface(
    matching_polygons: impl Iterator<Item = (i32, f32)>,
    grid_height: impl FnOnce() -> Option<f32>,
) -> Option<WaterSurface> {
    let mut best_trigger_id = None;
    let mut best_water_z = 0.0;
    for (trigger_id, height) in matching_polygons {
        if height >= best_water_z {
            best_water_z = height;
            best_trigger_id = Some(trigger_id);
        }
    }
    // TerrainLogic::getWaterHandle lets the grid win a tie.
    if let Some(height) = grid_height()
        && height >= best_water_z
    {
        return Some(WaterSurface::Grid(height));
    }
    best_trigger_id.map(|trigger_id| WaterSurface::Polygon {
        trigger_id,
        height: best_water_z,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_query_rejects_aabb_and_requires_enable() {
        let mut grid = WaterGridState::default();
        grid.transform = Mat4::from_translation(Vec3::new(100.0, 200.0, 12.0));
        grid.resolution = (8.0, 8.0, 10.0);
        assert!(grid.height_at(100.0, 200.0).is_none());
        grid.enabled = true;
        assert!((grid.height_at(100.0, 200.0).unwrap() - 12.0).abs() < 1e-4);
        assert!(grid.height_at(0.0, 0.0).is_none());
    }
}
