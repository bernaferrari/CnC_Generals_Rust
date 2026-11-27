//! Native water delivery and GameData.INI adaptation.
use glam::Vec3;

use generals_sim::terrain::{WaterGridSettings, WaterGridState, water_grid_setting_index};

/// Live GameClient `TerrainVisualImpl` hooks (registered from GameClient).
#[derive(Clone, Copy, Default)]
pub struct VisualWaterHooks {
    pub enable_water_grid: Option<fn(bool)>,
    pub set_height_clamps: Option<fn(f32, f32)>,
    pub set_transform_matrix: Option<fn([f32; 16])>,
    pub set_resolution: Option<fn(f32, f32, f32)>,
    pub set_attenuation: Option<fn(f32, f32, f32, f32)>,
    /// Immutable logical grid output; the receiver never feeds simulation queries.
    pub publish_grid: Option<fn(&WaterGridState)>,
}

pub fn register_visual_water_hooks(new_hooks: VisualWaterHooks) {
    let services = crate::system::engine_stores::active_services();
    *services
        .client_visuals()
        .water_hooks
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = new_hooks;
}

fn visual_water_hooks() -> VisualWaterHooks {
    let services = crate::system::engine_stores::active_services();
    *services
        .client_visuals()
        .water_hooks
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// Publish completed logical water to the standalone native visual adapter.
pub fn publish_water_grid(state: &WaterGridState) {
    let hooks = visual_water_hooks();
    if let Some(hook) = hooks.set_height_clamps {
        hook(state.height_clamps.0, state.height_clamps.1);
    }
    if let Some(hook) = hooks.set_transform_matrix {
        hook(state.transform.to_cols_array());
    }
    if let Some(hook) = hooks.set_resolution {
        hook(state.resolution.0, state.resolution.1, state.resolution.2);
    }
    if let Some(hook) = hooks.set_attenuation {
        hook(
            state.attenuation.0,
            state.attenuation.1,
            state.attenuation.2,
            state.attenuation.3,
        );
    }
    if let Some(hook) = hooks.enable_water_grid {
        hook(state.enabled);
    }
    if let Some(hook) = hooks.publish_grid {
        hook(state);
    }
}

/// Apply a receiving map's explicit definitions. Visual delivery is separate.
pub(super) fn configure_map_water(
    state: &mut WaterGridState,
    enable: bool,
    map_name: &str,
    definitions: Option<&game_engine::common::ini::ini_game_data::GlobalData>,
) -> bool {
    if !enable {
        state.enabled = false;
        return true;
    }
    let Some(global) = definitions else {
        return false;
    };
    let map_name = map_name.trim();
    if map_name.is_empty() {
        return false;
    }
    let Some(i) = water_grid_setting_index(map_name, &global.vertex_water_available_maps) else {
        log::error!(
            "!!!!!! Deformable water won't work because there was no group of vertex water data defined in GameData.INI for this map name '{}' !!!!!! (C. Day)",
            map_name
        );
        return false;
    };

    state.apply_settings(WaterGridSettings {
        height_clamps: (
            global.vertex_water_height_clamp_low[i],
            global.vertex_water_height_clamp_hi[i],
        ),
        position: Vec3::new(
            global.vertex_water_x_position[i],
            global.vertex_water_y_position[i],
            global.vertex_water_z_position[i],
        ),
        angle: global.vertex_water_angle[i],
        resolution: (
            global.vertex_water_x_grid_cells[i] as f32,
            global.vertex_water_y_grid_cells[i] as f32,
            global.vertex_water_grid_size[i],
        ),
        attenuation: (
            global.vertex_water_attenuation_a[i],
            global.vertex_water_attenuation_b[i],
            global.vertex_water_attenuation_c[i],
            global.vertex_water_attenuation_range[i],
        ),
    });
    true
}

/// Temporary standalone native delivery boundary. Disable has no configuration
/// side effects, matching TerrainLogic.cpp enableWaterGrid(false).
pub fn publish_map_water_legacy_native(state: &WaterGridState) {
    if state.enabled {
        publish_water_grid(state);
    } else if let Some(hook) = visual_water_hooks().enable_water_grid {
        hook(false);
    }
}
