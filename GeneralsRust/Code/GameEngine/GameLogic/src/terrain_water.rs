//! Grid water used by `TerrainLogic`.
//!
//! C++ `TerrainLogic::enableWaterGrid` pushes GameData.INI vertex-water settings
//! onto `TheTerrainVisual`, then `enableWaterGrid` even on disable.
//! `isUnderwater` / `getWaterHandle` preserve the original world-to-grid and
//! vertex-height queries on the owning logical grid. Visual hooks only receive
//! completed values; they cannot select or overwrite simulation water.

use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Live GameClient `TerrainVisualImpl` hooks (registered from GameClient).
#[derive(Clone, Copy, Default)]
pub struct VisualWaterHooks {
    pub enable_water_grid: Option<fn(bool)>,
    pub set_height_clamps: Option<fn(f32, f32)>,
    pub set_transform: Option<fn(f32, f32, f32, f32)>,
    pub set_transform_matrix: Option<fn([f32; 16])>,
    pub set_resolution: Option<fn(f32, f32, f32)>,
    pub set_attenuation: Option<fn(f32, f32, f32, f32)>,
    pub get_water_grid_height: Option<fn(f32, f32) -> Option<f32>>,
    pub get_transform_z: Option<fn() -> f32>,
    pub set_transform_z: Option<fn(f32)>,
    /// Immutable logical grid output; the receiver never feeds simulation queries.
    pub publish_grid: Option<fn(&WaterGridState)>,
}

/// Canonical logical water-grid state, owned by one TerrainLogic instance.
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

static WATER_HOOKS: LazyLock<Mutex<VisualWaterHooks>> =
    LazyLock::new(|| Mutex::new(VisualWaterHooks::default()));

pub fn register_visual_water_hooks(new_hooks: VisualWaterHooks) {
    if let Ok(mut slot) = WATER_HOOKS.lock() {
        *slot = new_hooks;
    }
}

fn with_hooks<R>(f: impl FnOnce(&VisualWaterHooks) -> R) -> Option<R> {
    WATER_HOOKS.lock().ok().map(|h| f(&h))
}

// Retained engine adapters resolve the driving service; constructors never publish it.
fn with_state_mut<R>(f: impl FnOnce(&mut WaterGridState) -> R) -> Option<R> {
    let terrain = crate::terrain::get_terrain_logic();
    let mut terrain = terrain.write().ok()?;
    Some(f(terrain.water_grid_state_mut()))
}
fn with_state<R>(f: impl FnOnce(&WaterGridState) -> R) -> Option<R> {
    let terrain = crate::terrain::get_terrain_logic();
    let terrain = terrain.read().ok()?;
    Some(f(terrain.water_grid_state()))
}
pub fn reset_water_grid_state() {
    let _ = with_state_mut(|state| *state = WaterGridState::default());
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
    pub fn bounds(&self) -> crate::common::Region3D {
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
        crate::common::Region3D::new(
            crate::common::Coord3D::new(lo.x, lo.y, lo.z),
            crate::common::Coord3D::new(hi.x, hi.y, hi.z),
        )
    }
    /// Publish an immutable completed logical value to the native visual adapter.
    /// Visual queries never write or choose authoritative simulation water.
    pub fn publish_visual(&self) {
        let hooks = with_hooks(|hooks| *hooks).unwrap_or_default();
        if let Some(hook) = hooks.set_height_clamps {
            hook(self.height_clamps.0, self.height_clamps.1);
        }
        if let Some(hook) = hooks.set_transform_matrix {
            hook(self.transform.to_cols_array());
        }
        if let Some(hook) = hooks.set_resolution {
            hook(self.resolution.0, self.resolution.1, self.resolution.2);
        }
        if let Some(hook) = hooks.set_attenuation {
            hook(
                self.attenuation.0,
                self.attenuation.1,
                self.attenuation.2,
                self.attenuation.3,
            );
        }
        if let Some(hook) = hooks.enable_water_grid {
            hook(self.enabled);
        }
        if let Some(hook) = hooks.publish_grid {
            hook(self);
        }
    }
}

/// C++ `TheTerrainVisual->enableWaterGrid`.
pub fn visual_enable_water_grid(enable: bool) {
    let _ = with_state_mut(|s| s.enabled = enable);
    if let Some(hook) = with_hooks(|h| h.enable_water_grid).flatten() {
        hook(enable);
    }
}

/// C++ `TheTerrainVisual->setWaterGridHeightClamps`.
pub fn visual_set_height_clamps(low: f32, high: f32) {
    let _ = with_state_mut(|s| s.height_clamps = (low, high));
    if let Some(hook) = with_hooks(|h| h.set_height_clamps).flatten() {
        hook(low, high);
    }
}

/// C++ `TheTerrainVisual->setWaterTransform(NULL, angle, x, y, z)`.
pub fn visual_set_transform(angle: f32, x: f32, y: f32, z: f32) {
    let _ = with_state_mut(|s| {
        s.transform = Mat4::from_translation(Vec3::new(x, y, z)) * Mat4::from_rotation_z(angle);
    });
    if let Some(hook) = with_hooks(|h| h.set_transform).flatten() {
        hook(angle, x, y, z);
    }
}

/// C++ `TheTerrainVisual->setWaterTransform(&matrix)`.
pub fn visual_set_transform_matrix(matrix: Mat4) {
    let cols = matrix.to_cols_array();
    let _ = with_state_mut(|s| s.transform = matrix);
    if let Some(hook) = with_hooks(|h| h.set_transform_matrix).flatten() {
        hook(cols);
    }
}

/// C++ `TheTerrainVisual->setWaterGridResolution`.
pub fn visual_set_resolution(cells_x: f32, cells_y: f32, cell_size: f32) {
    let _ = with_state_mut(|s| {
        let cell_size = cell_size.max(f32::EPSILON);
        let old_x = s.resolution.0;
        s.resolution.2 = cell_size;
        // C++ W3DWater.cpp only reallocates when `m_gridCellsX` changes.
        if old_x != cells_x {
            s.resolution.0 = cells_x;
            s.resolution.1 = cells_y;
            s.height_deltas.clear();
            s.mesh_motion.clear();
        }
    });
    if let Some(hook) = with_hooks(|h| h.set_resolution).flatten() {
        hook(cells_x, cells_y, cell_size);
    }
}

/// C++ `TheTerrainVisual->setWaterAttenuationFactors`.
pub fn visual_set_attenuation(a: f32, b: f32, c: f32, range: f32) {
    let _ = with_state_mut(|s| s.attenuation = (a, b, c, range));
    if let Some(hook) = with_hooks(|h| h.set_attenuation).flatten() {
        hook(a, b, c, range);
    }
}

/// C++ `TheTerrainVisual->getWaterGridHeight`.
///
/// Returns `Some(z)` only when the point is inside the mesh (world-to-grid).
pub fn get_water_grid_height(world_x: f32, world_y: f32) -> Option<f32> {
    with_state(|state| state.height_at(world_x, world_y)).flatten()
}

pub fn get_transform_z() -> f32 {
    with_state(|state| state.transform.w_axis.z).unwrap_or(0.0)
}

pub fn set_transform_z(height: f32) {
    let matrix = with_state_mut(|state| {
        state.set_height(height);
        state.transform
    });
    if let Some(matrix) = matrix {
        if let Some(hook) = with_hooks(|hooks| hooks.set_transform_matrix).flatten() {
            hook(matrix.to_cols_array());
        }
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

/// Apply GameData.INI vertex-water settings and notify the visual.
///
/// Returns `false` when enable was requested but no matching map entry exists
/// (C++ returns before `TheTerrainVisual->enableWaterGrid`).
pub fn enable_water_grid(enable: bool) -> bool {
    with_state_mut(|state| state.configure_for_map(enable)).unwrap_or(false)
}

impl WaterGridState {
    pub fn configure_for_map(&mut self, enable: bool) -> bool {
        if !enable {
            self.enabled = false;
            self.publish_visual();
            return true;
        }

        let Some(global) = game_engine::common::ini::get_global_data() else {
            return false;
        };
        let global = global.read();
        let map_name = global.map_name.trim();
        if map_name.is_empty() {
            return false;
        }

        let map_leaf = map_name.rsplit(['\\', '/']).next().unwrap_or(map_name);
        let mut water_setting_index: Option<usize> = None;
        for (i, configured) in global.vertex_water_available_maps.iter().enumerate() {
            let configured = configured.trim();
            if configured.is_empty() {
                continue;
            }
            if configured.eq_ignore_ascii_case(map_name) {
                water_setting_index = Some(i);
                break;
            }
            let configured_leaf = configured.rsplit(['\\', '/']).next().unwrap_or(configured);
            if configured_leaf.eq_ignore_ascii_case(map_leaf) {
                water_setting_index = Some(i);
                break;
            }
        }

        let Some(i) = water_setting_index else {
            log::error!(
                "!!!!!! Deformable water won't work because there was no group of vertex water data defined in GameData.INI for this map name '{}' !!!!!! (C. Day)",
                map_name
            );
            return false;
        };

        self.height_clamps = (
            global.vertex_water_height_clamp_low[i],
            global.vertex_water_height_clamp_hi[i],
        );
        self.transform = Mat4::from_translation(Vec3::new(
            global.vertex_water_x_position[i],
            global.vertex_water_y_position[i],
            global.vertex_water_z_position[i],
        )) * Mat4::from_rotation_z(global.vertex_water_angle[i]);
        self.set_resolution(
            global.vertex_water_x_grid_cells[i] as f32,
            global.vertex_water_y_grid_cells[i] as f32,
            global.vertex_water_grid_size[i],
        );
        self.attenuation = (
            global.vertex_water_attenuation_a[i],
            global.vertex_water_attenuation_b[i],
            global.vertex_water_attenuation_c[i],
            global.vertex_water_attenuation_range[i],
        );
        self.enabled = true;
        self.publish_visual();
        true
    }
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
