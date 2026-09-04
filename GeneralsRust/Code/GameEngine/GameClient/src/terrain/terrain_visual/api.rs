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
    ensure_radar_terrain_paint_source_registered();
    Ok(())
}

/// Bind C++ `TheTerrainVisual::setRawMapHeight` / `staticLightingChanged` to
/// the live GameClient visual. Safe to call more than once.
pub fn init_terrain_visual_hooks() {
    register_logic_height_hooks();
    register_overlay_rebuild_hooks();
    register_unit_moved_hook();
    ensure_radar_terrain_paint_source_registered();
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
    gamelogic::terrain_water::register_visual_water_hooks(
        gamelogic::terrain_water::VisualWaterHooks {
            enable_water_grid: Some(|enable| {
                if let Ok(mut visual) = get_terrain_visual() {
                    if let Some(visual) = visual.as_mut() {
                        visual.enable_water_grid(enable);
                    }
                }
            }),
            set_height_clamps: Some(|low, high| {
                if let Ok(mut visual) = get_terrain_visual() {
                    if let Some(visual) = visual.as_mut() {
                        visual.set_water_grid_height_clamps(low, high);
                    }
                }
            }),
            set_transform: Some(|angle, x, y, z| {
                if let Ok(mut visual) = get_terrain_visual() {
                    if let Some(visual) = visual.as_mut() {
                        visual.set_water_transform(angle, x, y, z);
                    }
                }
            }),
            set_transform_matrix: Some(|cols| {
                if let Ok(mut visual) = get_terrain_visual() {
                    if let Some(visual) = visual.as_mut() {
                        visual.set_water_transform_matrix(Mat4::from_cols_array(&cols));
                    }
                }
            }),
            set_resolution: Some(|cells_x, cells_y, cell_size| {
                if let Ok(mut visual) = get_terrain_visual() {
                    if let Some(visual) = visual.as_mut() {
                        visual.set_water_grid_resolution(cells_x, cells_y, cell_size);
                    }
                }
            }),
            set_attenuation: Some(|a, b, c, range| {
                if let Ok(mut visual) = get_terrain_visual() {
                    if let Some(visual) = visual.as_mut() {
                        visual.set_water_attenuation_factors(a, b, c, range);
                    }
                }
            }),
            get_water_grid_height: Some(|x, y| {
                get_terrain_visual()
                    .ok()
                    .and_then(|guard| guard.as_ref().and_then(|v| v.get_water_grid_height(x, y)))
            }),
            get_transform_z: Some(|| {
                get_terrain_visual()
                    .ok()
                    .and_then(|guard| guard.as_ref().map(|v| v.water_transform().w_axis.z))
                    .unwrap_or(0.0)
            }),
            set_transform_z: Some(|height| {
                if let Ok(mut visual) = get_terrain_visual() {
                    if let Some(visual) = visual.as_mut() {
                        let mut transform = visual.water_transform();
                        transform.w_axis.z = height;
                        visual.set_water_transform_matrix(transform);
                    }
                }
            }),
        },
    );

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

/// Bind leftover TheRadar land/bridge paint to GameClient TerrainVisual + leftover TerrainLogic.
///
/// C++ `W3DRadar::buildTerrainTexture` (`W3DRadar.cpp:1174`, `:1142-1167`) samples
/// `TheTerrainVisual->getTerrainColorAt` and intact-bridge `TerrainRoads->findBridge()->getRadarColor`.
pub fn ensure_radar_terrain_paint_source_registered() {
    let _ = game_engine::common::system::radar::register_radar_terrain_paint_source(
        std::sync::Arc::new(ClientRadarTerrainPaintSource),
    );
}

struct ClientRadarTerrainPaintSource;

impl game_engine::common::system::radar::RadarTerrainPaintSource for ClientRadarTerrainPaintSource {
    fn terrain_color_at(&self, world_x: f32, world_y: f32) -> Option<[f32; 3]> {
        leftover_radar_terrain_color_at(world_x, world_y)
    }

    fn bridge_at(
        &self,
        world: &game_engine::common::system::radar::Coord3D,
    ) -> Option<game_engine::common::system::radar::RadarBridgeSample> {
        leftover_radar_bridge_at(world)
    }
}

/// `TheTerrainVisual->getTerrainColorAt(world.x, world.y)`.
pub fn leftover_radar_terrain_color_at(world_x: f32, world_y: f32) -> Option<[f32; 3]> {
    let guard = get_terrain_visual().ok()?;
    let visual = guard.as_ref()?;
    // Tiles hydrated with stand-in placeholders (real Art/Terrain TGAs did
    // not resolve) or not hydrated at all report None: C++ samples real tile
    // art only (WorldHeightMap.cpp:2347-2356 null getSourceTile leaves the
    // color unset), so the radar software path shades its fallback base
    // color instead of painting black or hash-placeholder noise.
    if !visual.has_terrain_source_tiles() {
        return None;
    }
    visual.radar_terrain_color_at(world_x, world_y)
}

/// Intact working-bridge radar sample: object body != RUBBLE, Roads.ini color, deck-Z average.
pub fn leftover_radar_bridge_at(
    world: &game_engine::common::system::radar::Coord3D,
) -> Option<game_engine::common::system::radar::RadarBridgeSample> {
    let terrain = gamelogic::terrain::get_terrain_logic().try_read().ok()?;
    let loc = gamelogic::common::Coord3D::new(world.x, world.y, world.z);
    let bridge = terrain.find_bridge_at(&loc)?;
    let info = bridge.get_bridge_info();
    if info.bridge_object_id == gamelogic::common::INVALID_ID {
        return None;
    }
    let obj = gamelogic::helpers::TheGameLogic::find_object_by_id(info.bridge_object_id)?;
    let obj_g = obj.try_read().ok()?;
    let body = obj_g.get_body_module()?;
    let body_g = body.try_lock().ok()?;
    if body_g.get_damage_state() == gamelogic::object::body::BodyDamageType::Rubble {
        return None;
    }
    drop(body_g);
    drop(obj_g);

    let color = game_engine::common::ini::try_get_terrain_roads()
        .and_then(|roads| {
            roads
                .find_bridge(bridge.get_bridge_template_name().as_str())
                .map(|tmpl| {
                    let c = tmpl.radar_color;
                    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]
                })
        })
        .unwrap_or([1.0, 1.0, 1.0]);
    let height = (info.from_left.z + info.from_right.z + info.to_left.z + info.to_right.z) / 4.0;
    Some(game_engine::common::system::radar::RadarBridgeSample { color, height })
}
