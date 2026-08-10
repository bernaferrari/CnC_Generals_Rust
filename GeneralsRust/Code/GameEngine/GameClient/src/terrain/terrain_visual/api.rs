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
    Ok(())
}

/// Get reference to global terrain visual instance
pub fn get_terrain_visual(
) -> Result<std::sync::MutexGuard<'static, Option<TerrainVisualImpl>>, TerrainError> {
    THE_TERRAIN_VISUAL.lock().map_err(|_| {
        TerrainError::InitializationError("Failed to lock terrain visual mutex".to_string())
    })
}
