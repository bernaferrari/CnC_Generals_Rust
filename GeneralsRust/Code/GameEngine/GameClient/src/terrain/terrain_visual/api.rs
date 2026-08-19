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
    register_overlay_rebuild_hooks();
    Ok(())
}

/// Bind C++ `TheTerrainVisual::setRawMapHeight` / `staticLightingChanged` to
/// the live GameClient visual. Safe to call more than once.
pub fn init_terrain_visual_hooks() {
    register_logic_height_hooks();
    register_overlay_rebuild_hooks();
    register_unit_moved_hook();
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
    gamelogic::helpers::register_terrain_visual_add_prop_hook(Some(
        |drawable_id, position, angle, scale, model_name| {
            let _ = drawable_id;
            if let Ok(mut visual) = get_terrain_visual() {
                if let Some(visual) = visual.as_mut() {
                    let _ = visual.add_prop(position, angle, scale, model_name);
                }
            }
        },
    ));
    if let Some(logic_visual) = gamelogic::helpers::TheTerrainVisual::get() {
        for (_id, position, angle, scale, model_name) in logic_visual.take_pending_props() {
            if let Ok(mut visual) = get_terrain_visual() {
                if let Some(visual) = visual.as_mut() {
                    let _ = visual.add_prop(position, angle, scale, &model_name);
                }
            }
        }
    }
}

fn rebuild_shoreline_hook() {
    if let Ok(mut visual) = get_terrain_visual() {
        if let Some(visual) = visual.as_mut() {
            visual.rebuild_shoreline();
        }
    }
}

fn rebuild_tank_tracks_hook() {
    if let Ok(mut visual) = get_terrain_visual() {
        if let Some(visual) = visual.as_mut() {
            visual.rebuild_tank_tracks();
        }
    }
}

fn register_overlay_rebuild_hooks() {
    game_engine::common::game_lod::register_rebuild_shoreline(rebuild_shoreline_hook);
    game_engine::common::game_lod::register_rebuild_tank_tracks(rebuild_tank_tracks_hook);
}

fn register_unit_moved_hook() {
    // GameLogic Object slice should call `notify_terrain_unit_moved`.
}

/// C++ `W3DGameClient::notifyTerrainObjectMoved` entry for the live GPU impl.
pub fn notify_terrain_unit_moved(unit: crate::terrain::TreeCollisionUnit, frame: u32) {
    if let Ok(mut visual) = get_terrain_visual() {
        if let Some(visual) = visual.as_mut() {
            visual.unit_moved(unit, frame);
        }
    }
}

pub fn rebuild_shoreline() {
    rebuild_shoreline_hook();
}

pub fn rebuild_tank_tracks() {
    rebuild_tank_tracks_hook();
}


/// Get reference to global terrain visual instance
pub fn get_terrain_visual(
) -> Result<std::sync::MutexGuard<'static, Option<TerrainVisualImpl>>, TerrainError> {
    THE_TERRAIN_VISUAL.lock().map_err(|_| {
        TerrainError::InitializationError("Failed to lock terrain visual mutex".to_string())
    })
}
