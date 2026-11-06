// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

// Re-export the main implementation
pub use TerrainVisualImpl as TerrainVisualSystem;

type PublishedTerrainVisual = std::sync::Arc<parking_lot::Mutex<Option<TerrainVisualImpl>>>;

/// The live client's terrain visual (C++ `TheTerrainVisual`), published on
/// this game instance's CommonServices rather than a process static.
pub(crate) fn publish_terrain_visual(visual: PublishedTerrainVisual) {
    if let Some(services) = game_engine::common::common_services::try_common_services() {
        *services
            .system
            .terrain_visual_any
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(visual);
    }
}

pub(crate) fn unpublish_terrain_visual(visual: &PublishedTerrainVisual) {
    let Some(services) = game_engine::common::common_services::try_common_services() else {
        return;
    };
    let mut slot = services
        .system
        .terrain_visual_any
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let same = slot.as_ref().is_some_and(|published| {
        std::sync::Arc::as_ptr(published) as *const () == std::sync::Arc::as_ptr(visual) as *const ()
    });
    if same {
        *slot = None;
    }
}

fn published_terrain_visual() -> Option<PublishedTerrainVisual> {
    let services = game_engine::common::common_services::try_common_services()?;
    let any = services
        .system
        .terrain_visual_any
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()?;
    any.downcast::<parking_lot::Mutex<Option<TerrainVisualImpl>>>().ok()
}

/// Mutate the live/published terrain visual (preferred over `get_terrain_visual`).
pub fn with_terrain_visual_mut<R>(f: impl FnOnce(&mut Option<TerrainVisualImpl>) -> R) -> Option<R> {
    let mut f = Some(f);
    if let Some(result) = crate::core::game_client::with_live_game_client_mut(|client| {
        let mut guard = client.terrain_visual_gpu.lock();
        f.take().expect("terrain visual")(&mut guard)
    }) {
        return Some(result);
    }
    let arc = published_terrain_visual()?;
    let mut guard = arc.lock();
    Some(f.take().expect("terrain visual")(&mut guard))
}

/// Initialize terrain visual on the live GameClient (C++ GameClient constructs TheTerrainVisual).
pub fn init_terrain_visual() -> TerrainResult<()> {
    let inited = crate::core::game_client::with_live_game_client_mut(|client| {
        let mut guard = client.terrain_visual_gpu.lock();
        *guard = Some(TerrainVisualImpl::new());
        publish_terrain_visual(std::sync::Arc::clone(&client.terrain_visual_gpu));
    });
    if inited.is_none() {
        // Headless/tests without a live client: publish a dedicated Arc.
        let arc = std::sync::Arc::new(parking_lot::Mutex::new(Some(TerrainVisualImpl::new())));
        publish_terrain_visual(arc);
    }
    register_logic_height_hooks();
    register_overlay_rebuild_hooks();
    ensure_radar_terrain_paint_source_registered();
    Ok(())
}

/// C++ `GameClient::~GameClient` deletes `TheTerrainVisual` after Drawables.
/// Called only by the published owner during GameClient teardown.
pub(crate) fn shutdown_terrain_visual() {
    // C++ deletes TheTerrainVisual but the process pointer cell remains for the
    // next GameClient; keep the published Arc and clear its Option.
    let _ = with_terrain_visual_mut(|visual| {
        *visual = None;
    });
}

/// Compatibility export for older direct `THE_TERRAIN_VISUAL.lock()` call sites.
/// Prefer [`with_terrain_visual_mut`].
#[deprecated(note = "use with_terrain_visual_mut / GameClient.terrain_visual_gpu")]
pub fn the_terrain_visual_published(
) -> Option<std::sync::Arc<parking_lot::Mutex<Option<TerrainVisualImpl>>>> {
    published_terrain_visual()
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
    gamelogic::helpers::register_terrain_visual_raw_height_hook(Some(|x, y, height| {
        if let Ok(mut visual) = get_terrain_visual() {
            if let Some(visual) = visual.as_mut() {
                visual.set_raw_map_height(x, y, height);
            }
        }
    }));
    gamelogic::helpers::register_terrain_visual_lighting_changed_hook(Some(|| {
        if let Ok(mut visual) = get_terrain_visual() {
            if let Some(visual) = visual.as_mut() {
                visual.static_lighting_changed();
            }
        }
    }));
    gamelogic::helpers::register_terrain_visual_add_prop_hook(Some(
        |drawable_id, position, angle, scale, model_name| {
            if let Ok(mut visual) = get_terrain_visual() {
                if let Some(visual) = visual.as_mut() {
                    let _ = visual.add_prop(drawable_id, position, angle, scale, model_name);
                }
            }
        },
    ));
    if let Some(logic_visual) = gamelogic::helpers::TheTerrainVisual::get() {
        for (drawable_id, position, angle, scale, model_name) in logic_visual.take_pending_props() {
            if let Ok(mut visual) = get_terrain_visual() {
                if let Some(visual) = visual.as_mut() {
                    let _ = visual.add_prop(drawable_id, position, angle, scale, &model_name);
                }
            }
        }
    }
    gamelogic::terrain_water::register_visual_water_hooks(
        gamelogic::terrain_water::VisualWaterHooks {
            publish_grid: Some(|state| {
                if let Ok(mut visual) = get_terrain_visual() {
                    if let Some(visual) = visual.as_mut() {
                        visual.admit_water_mesh(state);
                    }
                }
            }),
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

/// Guard that keeps the published Arc alive while the inner mutex is locked.
pub struct TerrainVisualGuard {
    guard: parking_lot::ArcMutexGuard<parking_lot::RawMutex, Option<TerrainVisualImpl>>,
}

impl std::ops::Deref for TerrainVisualGuard {
    type Target = Option<TerrainVisualImpl>;
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl std::ops::DerefMut for TerrainVisualGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

/// Lock the published/live terrain visual. Prefer [`with_terrain_visual_mut`].
pub fn get_terrain_visual() -> Result<TerrainVisualGuard, TerrainError> {
    let arc = published_terrain_visual().ok_or_else(|| {
        TerrainError::InitializationError(
            "no published terrain visual (need live GameClient or init_terrain_visual)".into(),
        )
    })?;
    let guard = arc.lock_arc();
    Ok(TerrainVisualGuard { guard })
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
    let terrain_owner_handle = gamelogic::terrain::get_terrain_logic();
    let terrain = terrain_owner_handle.try_read().ok()?;
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
