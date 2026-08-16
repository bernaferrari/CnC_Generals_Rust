// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

// Re-export the main implementation
pub use TerrainVisualImpl as TerrainVisualSystem;

// Global singleton instance (matching C++ pattern)
lazy_static::lazy_static! {
    pub static ref THE_TERRAIN_VISUAL: std::sync::Mutex<Option<TerrainVisualImpl>> = std::sync::Mutex::new(None);
}

/// Initialize the global terrain visual instance
pub fn init_terrain_visual() -> TerrainResult<()> {
    let mut global_instance = THE_TERRAIN_VISUAL.lock().unwrap_or_else(|e| e.into_inner());
    *global_instance = Some(TerrainVisualImpl::new());
    register_logic_height_hooks();
    Ok(())
}

/// Bind C++ `TheTerrainVisual::setRawMapHeight` / `staticLightingChanged` to
/// the live GameClient visual. Safe to call more than once.
pub fn init_terrain_visual_hooks() {
    register_logic_height_hooks();
}

fn register_logic_height_hooks() {
    gamelogic::helpers::register_terrain_visual_raw_height_hook(Some(
        |x, y, height| {
            if let Ok(mut visual) = get_terrain_visual() {
                if let Some(visual) = visual.as_mut() {
                    visual.set_raw_map_height(x, y, height);
                }
            }
        },
    ));
    gamelogic::helpers::register_terrain_visual_lighting_changed_hook(Some(|| {
        if let Ok(mut visual) = get_terrain_visual() {
            if let Some(visual) = visual.as_mut() {
                visual.static_lighting_changed();
            }
        }
    }));
}


/// Get reference to global terrain visual instance
pub fn get_terrain_visual(
) -> Result<std::sync::MutexGuard<'static, Option<TerrainVisualImpl>>, TerrainError> {
    THE_TERRAIN_VISUAL.lock().map_err(|_| {
        TerrainError::InitializationError("Failed to lock terrain visual mutex".to_string())
    })
}
