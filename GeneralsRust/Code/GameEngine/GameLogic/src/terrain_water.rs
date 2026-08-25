//! Grid water used by `TerrainLogic`.
//!
//! C++ `TerrainLogic::enableWaterGrid` pushes GameData.INI vertex-water settings
//! onto `TheTerrainVisual`, then `enableWaterGrid` even on disable.
//! `isUnderwater` / `getWaterHandle` sample `TheTerrainVisual->getWaterGridHeight`
//! (world-to-grid + vertex height), not an AABB.

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
}

/// Logic-side replica of the C++ water-grid mesh (transform / resolution / deltas).
#[derive(Clone, Debug)]
pub struct WaterGridState {
    pub enabled: bool,
    pub transform: Mat4,
    pub resolution: (f32, f32, f32),
    pub height_clamps: (f32, f32),
    pub attenuation: (f32, f32, f32, f32),
    pub height_deltas: HashMap<(i32, i32), f32>,
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
        }
    }
}

static WATER_HOOKS: LazyLock<Mutex<VisualWaterHooks>> =
    LazyLock::new(|| Mutex::new(VisualWaterHooks::default()));
static WATER_STATE: LazyLock<Mutex<WaterGridState>> =
    LazyLock::new(|| Mutex::new(WaterGridState::default()));

pub fn register_visual_water_hooks(new_hooks: VisualWaterHooks) {
    if let Ok(mut slot) = WATER_HOOKS.lock() {
        *slot = new_hooks;
    }
}

fn with_hooks<R>(f: impl FnOnce(&VisualWaterHooks) -> R) -> Option<R> {
    WATER_HOOKS.lock().ok().map(|h| f(&h))
}

fn with_state_mut<R>(f: impl FnOnce(&mut WaterGridState) -> R) -> Option<R> {
    WATER_STATE.lock().ok().map(|mut s| f(&mut s))
}

fn with_state<R>(f: impl FnOnce(&WaterGridState) -> R) -> Option<R> {
    WATER_STATE.lock().ok().map(|s| f(&s))
}
pub fn reset_water_grid_state() {
    let _ = with_state_mut(|s| *s = WaterGridState::default());
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
    if let Some(z) =
        with_hooks(|h| h.get_water_grid_height.and_then(|f| f(world_x, world_y))).flatten()
    {
        return Some(z);
    }
    with_state(|s| sample_grid_height(s, world_x, world_y)).flatten()
}

/// C++ `transform.Get_Z_Translation()`.
pub fn get_transform_z() -> f32 {
    if let Some(Some(z)) = with_hooks(|h| h.get_transform_z.map(|f| f())) {
        return z;
    }
    with_state(|s| s.transform.w_axis.z).unwrap_or(0.0)
}

/// C++ `transform.Set_Z_Translation(height); setWaterTransform(&transform)`.
pub fn set_transform_z(height: f32) {
    let _ = with_state_mut(|s| s.transform.w_axis.z = height);
    if let Some(hook) = with_hooks(|h| h.set_transform_z).flatten() {
        hook(height);
        return;
    }
    if let Some(hook) = with_hooks(|h| h.set_transform_matrix).flatten() {
        if let Some(matrix) = with_state(|s| s.transform) {
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
    if !enable {
        visual_enable_water_grid(false);
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

    visual_set_height_clamps(
        global.vertex_water_height_clamp_low[i],
        global.vertex_water_height_clamp_hi[i],
    );
    visual_set_transform(
        global.vertex_water_angle[i],
        global.vertex_water_x_position[i],
        global.vertex_water_y_position[i],
        global.vertex_water_z_position[i],
    );
    visual_set_resolution(
        global.vertex_water_x_grid_cells[i] as f32,
        global.vertex_water_y_grid_cells[i] as f32,
        global.vertex_water_grid_size[i],
    );
    visual_set_attenuation(
        global.vertex_water_attenuation_a[i],
        global.vertex_water_attenuation_b[i],
        global.vertex_water_attenuation_c[i],
        global.vertex_water_attenuation_range[i],
    );
    visual_enable_water_grid(true);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_query_rejects_aabb_and_requires_enable() {
        let _ = with_state_mut(|s| {
            *s = WaterGridState::default();
            s.enabled = false;
            s.transform = Mat4::from_translation(Vec3::new(100.0, 200.0, 12.0));
            s.resolution = (8.0, 8.0, 10.0);
        });
        assert!(get_water_grid_height(100.0, 200.0).is_none());

        let _ = with_state_mut(|s| s.enabled = true);
        assert!((get_water_grid_height(100.0, 200.0).unwrap() - 12.0).abs() < 1e-4);
        // Outside the mesh (not an AABB of the whole map).
        assert!(get_water_grid_height(0.0, 0.0).is_none());
    }
}
