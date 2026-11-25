//! Original bridge admission/deletion order with Main-owned objects and slots.
use super::*;
use crate::game_logic::host_bridge_behavior::{MapBridgeSource, bridge_deck_corners};

use gamelogic::common::AsciiString;
use gamelogic::system::map_loader::{
    BridgeData, BridgeEndpoints, BridgeModels, MAX_AUTHORED_BRIDGES, MapData,
    read_mounted_bridge_model,
};
use gamelogic::terrain::{BridgeInfo, TerrainLogic};

impl GameLogic {
    pub(in super::super) fn reserve_owned_bridge(
        &mut self,
        info: &gamelogic::terrain::BridgeInfo,
    ) -> u8 {
        let [from_left, from_right, to_left, to_right] = bridge_deck_corners(info);
        let layer = self
            .pathfinding_system
            .grid
            .reserve_bridge_layer(from_left, from_right, to_left, to_right);
        self.pathfinding_system
            .grid
            .bind_reserved_bridge_object(layer, info.bridge_object_id);
        layer
    }

    pub(super) fn resolve_authored_map_bridges(
        &self,
        spans: Vec<BridgeEndpoints>,
    ) -> Vec<BridgeData> {
        let definitions = self.effective_terrain_roads();
        let assets = self.application_assets();
        let files = &self.world_services.common().application.file_system;
        let mut models = BridgeModels::default();
        spans
            .into_iter()
            .filter_map(|span| {
                let Some(bridge) = definitions.find_bridge(&span.template_name) else {
                    log::warn!(
                        "Map bridge '{}' has no Bridge definition",
                        span.template_name
                    );
                    return None;
                };
                models.deck(
                    span,
                    bridge.bridge_model_name.as_str(),
                    bridge.bridge_scale,
                    |path| {
                        if let Some(assets) = &assets {
                            let mut assets = assets.lock().map_err(|error| error.to_string())?;
                            return assets
                                .archive_file_system_mut()
                                .read_optional_definition_source(path)
                                .map_err(|error| error.to_string())?
                                .ok_or_else(|| format!("missing {path}"));
                        }
                        read_mounted_bridge_model(files, path)
                    },
                )
            })
            .take(MAX_AUTHORED_BRIDGES)
            .collect()
    }

    pub(super) fn stage_authored_map_bridge_geometry(
        &mut self,
        terrain: &mut TerrainLogic,
        data: MapData,
    ) -> Result<Vec<(BridgeInfo, AsciiString)>, String> {
        TerrainLogic::validate_map_geometry(&data)?;
        terrain.reset();
        self.pathfinding_system.grid.clear_bridge_admission();
        terrain.try_load_authored_map_geometry(data)
    }

    #[cfg(test)]
    pub(in super::super) fn admit_map_bridge_geometry(
        &mut self,
        terrain: &mut TerrainLogic,
        data: MapData,
    ) -> Result<(), String> {
        TerrainLogic::validate_map_geometry(&data)?;
        terrain.reset();
        self.pathfinding_system.grid.clear_bridge_admission();
        for (info, template) in terrain.try_load_map_geometry(data)? {
            let layer = self.reserve_owned_bridge(&info);
            terrain.prepend_bridge_on_layer(
                info,
                template,
                gamelogic::path::PathfindLayerEnum::from_u32(layer as u32),
            );
        }
        Ok(())
    }

    /// TerrainLogic::addBridgeToLogic constructs the actor before prepending
    /// the span and assigning its pathfinder layer.
    pub(in super::super) fn admit_loaded_bridge_spans(
        &mut self,
        spans: Vec<(BridgeInfo, AsciiString)>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use gamelogic::GameLogicError;

        let owner = self.world_services.terrain().clone();
        let mut missing_template = false;
        for (mut info, template) in spans {
            let [from_left, from_right, to_left, to_right] = bridge_deck_corners(&info);
            let center = (from_left + to_right) * 0.5;
            if !missing_template {
                if let Some(id) = self.create_terrain_bridge_actor(center, effects)? {
                    let properties = owner
                        .read()
                        .map_err(|error| GameLogicError::Threading(error.to_string()))?
                        .authored_bridge_properties(info.bridge_index)
                        .cloned()
                        .unwrap_or_default();
                    let actor = self
                        .host_object(id)
                        .ok_or(GameLogicError::InvalidObject(id.0))?;
                    let birth = std::sync::Arc::downgrade(&actor.visual_draw_identity_token);
                    self.apply_update_obj_values_from_map_properties_with_effects(
                        id,
                        &properties,
                        effects,
                    )
                    .map_err(GameLogicError::Configuration)?;
                    if !self.object_birth_matches(id, &birth) {
                        return Err(GameLogicError::InvalidObject(id.0));
                    }
                    let angle = (info.to_left.y - info.from_left.y)
                        .atan2(info.to_left.x - info.from_left.x);
                    self.objects
                        .get_mut(&id)
                        .ok_or(GameLogicError::InvalidObject(id.0))?
                        .set_orientation_with_partition_owner(angle, &mut self.partition_manager);
                    info.bridge_object_id = id.0;
                    self.bridge_behavior
                        .register_span(id, from_left, from_right, to_left, to_right);
                } else {
                    // The original retains the geometric bridge even without
                    // an actor. It still gives the deck a pathfinder layer.
                    log::warn!("GenericBridge template not found for map bridge actors");
                    missing_template = true;
                }
            }
            let layer = self.reserve_owned_bridge(&info);
            owner
                .write()
                .map_err(|error| GameLogicError::Threading(error.to_string()))?
                .prepend_bridge_on_layer(
                    info,
                    template,
                    gamelogic::path::PathfindLayerEnum::from_u32(layer as u32),
                );
        }
        Ok(())
    }

    /// Native load recreates map geometry rather than serializing layer IDs.
    /// Resolve only this world's restored BridgeBehavior/object records.
    pub(crate) fn restore_owned_bridge_bindings(
        &mut self,
        map_bridges: &std::collections::HashMap<MapBridgeSource, ObjectId>,
    ) {
        let owner = self.world_services.terrain().clone();
        let mut removed = Vec::new();
        owner
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .for_each_bridge_mut(|bridge| {
                let layer = bridge.get_layer() as u8;
                let info = bridge.get_bridge_info();
                let source = MapBridgeSource::of(info);
                let saved = match source {
                    Some(source) => map_bridges.get(&source).copied(),
                    None => {
                        let id = ObjectId(info.bridge_object_id);
                        self.objects
                            .get(&id)
                            .filter(|actor| actor.is_kind_of(KindOf::Bridge))
                            .map(|_| id)
                    }
                };
                let Some((id, span)) =
                    saved.and_then(|id| self.bridge_behavior.span(id).map(|span| (id, span)))
                else {
                    removed.push((layer, source, info.bridge_object_id));
                    return;
                };
                let towers = span.tower_ids;
                if source.is_none() {
                    let [from_left, from_right, to_left, to_right] = bridge_deck_corners(info);
                    self.bridge_behavior
                        .register_span(id, from_left, from_right, to_left, to_right);
                }
                self.pathfinding_system
                    .grid
                    .bind_reserved_bridge_object(layer, id.0);
                bridge.set_bridge_object_id(id.0);
                for (index, tower) in towers.into_iter().enumerate() {
                    if let Some(kind) = gamelogic::common::BridgeTowerType::from_index(index) {
                        bridge.set_tower_object_id(tower.0, kind);
                    }
                }
            });

        for (layer, source, previous_id) in removed {
            {
                let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
                let ground_source = PathfindingGround::for_terrain(
                    &terrain,
                    self.map_loaded,
                    self.terrain.as_ref(),
                    self.pathfinding_height_samples.as_ref(),
                    &self.pathfinding_system.grid,
                );
                self.pathfinding_system.grid.stamp_reserved_bridge_layer(
                    layer,
                    true,
                    &ground_source,
                );
            }
            self.pathfinding_system.grid.deactivate_bridge_layer(layer);
            let mut terrain = owner.write().unwrap_or_else(|error| error.into_inner());
            match source {
                Some(MapBridgeSource::Drawable(index)) => terrain.detach_bridge_by_index(index),
                Some(MapBridgeSource::Landmark(index)) => {
                    terrain.detach_bridge_by_map_object_index(index)
                }
                None => terrain.detach_bridge_by_object_id(previous_id),
            };
        }
        // GameLogic::xfer calls Pathfinder::newMap after restoring the map
        // water and Objects. Classify both terrain and restored footprints.
        if self.map_loaded && self.terrain.is_some() {
            self.sync_structure_path_blocks();
        } else {
            self.refresh_owned_bridge_pathfinder_states();
        }
    }

    /// Terrain phase reads canonical bodies/scaffolds; no extra behavior tick.
    pub(crate) fn refresh_owned_bridge_pathfinder_states(&mut self) {
        let owner = self.world_services.terrain();
        let loan = owner.read().unwrap_or_else(|error| error.into_inner());
        let ground = PathfindingGround::for_terrain(
            &loan,
            self.map_loaded,
            self.terrain.as_ref(),
            self.pathfinding_height_samples.as_ref(),
            &self.pathfinding_system.grid,
        );
        refresh_grid_bridge_states(
            &mut self.pathfinding_system.grid,
            self.objects.read().with_players(&self.players).with_team_factory(&self.team_factory),
            &self.bridge_behavior,
            &ground,
        );
    }

    /// An existing terrain loan (map/water reclassification) is passed through
    /// rather than reacquired while an OS writer could be waiting.
    pub(crate) fn refresh_owned_bridge_pathfinder_states_with_ground(
        &mut self,
        ground: &PathfindingGround<'_>,
    ) {
        refresh_grid_bridge_states(
            &mut self.pathfinding_system.grid,
            self.objects.read().with_players(&self.players).with_team_factory(&self.team_factory),
            &self.bridge_behavior,
            ground,
        );
    }

    pub(in super::super) fn sync_owned_bridge_body_state(
        &mut self,
        id: ObjectId,
        state: gamelogic::common::BodyDamageType,
    ) {
        let owner = self.world_services.terrain().clone();
        let mut terrain = owner.write().unwrap_or_else(|e| e.into_inner());
        terrain.set_bridge_damage_state_for_object(id.0, state);
    }

    /// CPP TerrainLogic::deleteBridge: unlink, disable exact layer, destroy,
    /// then release the detached bridge. Slots remain reserved until reset.
    pub(crate) fn delete_owned_bridge_at(&mut self, pos: Vec3) -> bool {
        self.delete_owned_bridge_at_with_effects(pos, &mut Default::default())
    }

    pub(crate) fn delete_owned_bridge_at_with_effects(
        &mut self,
        pos: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let owner = self.world_services.terrain().clone();
        let removed = owner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .detach_bridge_at(&gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y));
        let Some(bridge) = removed else {
            return false;
        };
        let id = ObjectId(bridge.get_bridge_info().bridge_object_id);
        ({
            let ground_owner = self.world_services.terrain();
            let ground_loan = ground_owner
                .read()
                .unwrap_or_else(|error| error.into_inner());
            let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                &ground_loan,
                self.map_loaded,
                self.terrain.as_ref(),
                self.pathfinding_height_samples.as_ref(),
                &self.pathfinding_system.grid,
            );
            self.pathfinding_system.grid.stamp_reserved_bridge_layer(
                bridge.get_layer() as u8,
                true,
                &ground_source,
            )
        });
        self.pathfinding_system
            .grid
            .deactivate_bridge_layer(bridge.get_layer() as u8);
        self.destroy_object_with_effects(id, effects);
        self.bridge_behavior.unregister_span(id);
        drop(bridge);
        true
    }
}

fn refresh_grid_bridge_states(
    grid: &mut PathfindingGrid,
    objects: crate::game_logic::HostObjectRead<'_>,
    bridges: &crate::game_logic::host_bridge_behavior::HostBridgeBehaviorRegistry,
    ground: &PathfindingGround<'_>,
) {
    for (layer, id, old_destroyed, unclassified) in grid.admitted_bridge_states() {
        let destroyed = if id == 0 {
            old_destroyed
        } else {
            objects.lookup(&ObjectId(id)).is_none_or(|object| {
                object.health.current <= 0.0
                    || object.health.maximum <= 0.0
                    || object.body_damage_state
                        == crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                    || bridges.is_scaffold_present(ObjectId(id))
            })
        };
        if unclassified || destroyed != old_destroyed {
            grid.stamp_reserved_bridge_layer(layer, destroyed, ground);
        }
    }
}
