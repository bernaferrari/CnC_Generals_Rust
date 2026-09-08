//! WthreeDSnow — device layer for the snow manager (C++ `W3DSnow.h` / `W3DSnow.cpp`).
//!
//! C++ derives `W3DSnowManager : SnowManager` inside GameEngineDevice: the
//! GameClient side owns the INI state (`WeatherSetting`) and the fall clock,
//! the device side owns batching and rendering. The Rust port keeps that
//! split — `game_client::snow` owns the manager state, the per-flake math and
//! the view clip (`flake_positions_y_up_clipped`), and the terrain overlay
//! path uploads the camera-facing quads. This module is the device-side
//! surface: the C++ batching constants and the `W3DSnowManager::update`
//! frame-time stepping.

use std::sync::{Arc, Mutex};

pub use game_client::snow::{
    camera_facing_quad_corners, get_snow_manager, get_weather_setting, SnowManager,
    SnowVisibleBoxXy,
};

/// C++ `SNOW_BUFFER_SIZE` — vertex-buffer capacity in flakes (`W3DSnow.cpp:19`).
pub const SNOW_BUFFER_SIZE: usize = 4096;
/// C++ `SNOW_BATCH_SIZE` — flakes per draw call; `2048 * 6` fits the 16-bit
/// index buffer (`W3DSnow.cpp:20`).
pub const SNOW_BATCH_SIZE: usize = 2048;
/// C++ `m_leafDim` set in `render()` — leaf boxes at or below this edge render
/// without further frustum subdivision (`W3DSnow.cpp:423`).
pub const SNOW_LEAF_DIM: i32 = 45;

/// C++ `W3DSnowManager::update` (`W3DSnow.cpp:144-151`): advance the shared
/// fall clock by the frame time in milliseconds and wrap it on the full fall
/// period (`box_dimensions / velocity`).
pub fn step_snow_time(manager: &mut SnowManager, frame_time_ms: f32) {
    manager.update(frame_time_ms / 1000.0);
}

/// Global-singleton form of [`step_snow_time`] over `TheSnowManager`.
pub fn update_snow(frame_time_ms: f32) {
    let Some(manager) = get_snow_manager() else {
        return;
    };
    if let Ok(mut guard) = manager.lock() {
        step_snow_time(&mut guard, frame_time_ms);
    }
}

/// Shared manager handle type mirroring the C++ `TheSnowManager` pointer.
pub type SnowManagerHandle = Arc<Mutex<SnowManager>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batching_constants_match_cpp() {
        assert_eq!(SNOW_BUFFER_SIZE, 4096);
        assert_eq!(SNOW_BATCH_SIZE, 2048);
        assert_eq!(SNOW_LEAF_DIM, 45);
    }

    #[test]
    fn step_snow_time_advances_the_fall_clock() {
        let mut manager = SnowManager::new();
        step_snow_time(&mut manager, 16.0);
        assert!((manager.time() - 0.016).abs() < 1.0e-6);
        step_snow_time(&mut manager, 16.0);
        assert!((manager.time() - 0.032).abs() < 1.0e-6);
    }
}
