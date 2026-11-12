//! Original bridge admission/deletion order with Main-owned objects and slots.
use super::*;

impl GameLogic {
    pub(in super::super) fn reserve_owned_bridge(
        &mut self,
        info: &gamelogic::terrain::BridgeInfo,
    ) -> u8 {
        let host = |p: gamelogic::common::Coord3D| Vec3::new(p.x, p.z, p.y);
        let layer = self.pathfinding_system.grid.reserve_bridge_layer(
            host(info.from_left),
            host(info.from_right),
            host(info.to_left),
            host(info.to_right),
        );
        self.pathfinding_system
            .grid
            .bind_reserved_bridge_object(layer, info.bridge_object_id);
        layer
    }

    pub(in super::super) fn admit_map_bridge_geometry(
        &mut self,
        terrain: &mut gamelogic::terrain::TerrainLogic,
        data: gamelogic::system::map_loader::MapData,
    ) -> Result<(), String> {
        gamelogic::terrain::TerrainLogic::validate_map_geometry(&data)?;
        terrain.reset();
        self.pathfinding_system.grid.clear_bridge_admission();
        // Original addBridge reserves in source order; the linked list prepends.
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

    /// Native load recreates map geometry rather than serializing layer IDs.
    /// Resolve only this world's restored BridgeBehavior/object records.
    pub(crate) fn restore_owned_bridge_bindings(
        &mut self,
        map_bridges: Option<&[(i32, Option<usize>, u32)]>,
    ) {
        self.restore_owned_bridge_bindings_with_effects(map_bridges, &mut Default::default())
    }

    pub(crate) fn restore_owned_bridge_bindings_with_effects(
        &mut self,
        map_bridges: Option<&[(i32, Option<usize>, u32)]>,
        _effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let owner = self.world_services.terrain().clone();
        let mut spans = Vec::new();
        owner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .for_each_bridge(|bridge| {
                spans.push((bridge.get_layer() as u8, bridge.get_bridge_info().clone()));
            });
        for (layer, info) in spans {
            let host = |p: gamelogic::common::Coord3D| Vec3::new(p.x, p.z, p.y);
            let saved_id = if info.bridge_index >= 0 || info.source_map_object_index.is_some() {
                map_bridges.and_then(|bridges| {
                    bridges.iter().find(|(index, source, _)| {
                        if info.bridge_index >= 0 {
                            *index == info.bridge_index
                        } else {
                            *index < 0 && *source == info.source_map_object_index
                        }
                    }).map(|(_, _, id)| *id)
                })
            } else {
                Some(info.bridge_object_id)
            };
            let id = saved_id.map(ObjectId).filter(|id| {
                self.objects.get(id).is_some_and(|object| object.is_kind_of(KindOf::Bridge))
                    && self.bridge_behavior.span(*id).is_some()
            });
            let Some(id) = id else {
                ({
                    let ground_owner = self.world_services.terrain();
                    let ground_loan = ground_owner
                        .read()
                        .unwrap_or_else(|error| error.into_inner());
                    let ground_source =
                        crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                            &ground_loan,
                            self.map_loaded,
                            self.terrain.as_ref(),
                            self.pathfinding_height_samples.as_ref(),
                            &self.pathfinding_system.grid,
                        );
                    self.pathfinding_system.grid.stamp_reserved_bridge_layer(
                        layer,
                        true,
                        &ground_source,
                    )
                });
                self.pathfinding_system.grid.deactivate_bridge_layer(layer);
                let mut terrain = owner.write().unwrap_or_else(|e| e.into_inner());
                if info.bridge_index >= 0 {
                    terrain.detach_bridge_by_index(info.bridge_index);
                } else if let Some(index) = info.source_map_object_index {
                    terrain.detach_bridge_by_map_object_index(index);
                } else {
                    terrain.detach_bridge_by_object_id(info.bridge_object_id);
                }
                continue;
            };

            if layer != gamelogic::path::PathfindLayerEnum::Ground as u8 {
                self.pathfinding_system.grid.bind_reserved_bridge_object(layer, id.0);
            }
            self.bridge_behavior.register_span(
                id,
                host(info.from_left),
                host(info.from_right),
                host(info.to_left),
                host(info.to_right),
            );
            let towers = self
                .bridge_behavior
                .span(id)
                .map(|s| s.tower_ids)
                .unwrap_or([ObjectId(0); 4]);
            owner
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .for_each_bridge_mut(|bridge| {
                    if bridge.get_layer() as u8 == layer
                        && if info.bridge_index >= 0 {
                            bridge.get_bridge_info().bridge_index == info.bridge_index
                        } else if info.source_map_object_index.is_some() {
                            bridge.get_bridge_info().source_map_object_index == info.source_map_object_index
                        } else {
                            bridge.get_bridge_info().bridge_object_id == info.bridge_object_id
                        }
                    {
                        bridge.set_bridge_object_id(id.0);
                        for (i, tower) in towers.iter().enumerate() {
                            if let Some(kind) = gamelogic::common::BridgeTowerType::from_index(i) {
                                bridge.set_tower_object_id(tower.0, kind);
                            }
                        }
                    }
                });
        }
        let terrain = owner.read().unwrap_or_else(|e| e.into_inner());
        self.reclassify_pathfinding_from_terrain(&terrain);
        drop(terrain);
        self.refresh_owned_bridge_pathfinder_states();
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
