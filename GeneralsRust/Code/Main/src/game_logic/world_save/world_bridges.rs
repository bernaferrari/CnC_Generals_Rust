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
    ) {
        terrain.reset();
        self.pathfinding_system.grid.clear_bridge_admission();
        // Original addBridge reserves in source order; the linked list prepends.
        for (info, template) in terrain.load_map_geometry(data) {
            let layer = self.reserve_owned_bridge(&info);
            terrain.prepend_bridge_on_layer(
                info,
                template,
                gamelogic::path::PathfindLayerEnum::from_u32(layer as u32),
            );
        }
    }

    /// Native load recreates map geometry rather than serializing layer IDs.
    /// Resolve only this world's restored BridgeBehavior/object records.
    pub(crate) fn restore_owned_bridge_bindings(&mut self) {
        let owner = self.world_services.terrain().clone();
        let mut spans = Vec::new();
        owner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .for_each_bridge(|bridge| {
                spans.push((bridge.get_layer() as u8, bridge.get_bridge_info().clone()));
            });
        let mut used = std::collections::HashSet::new();
        for (layer, info) in spans {
            let host = |p: gamelogic::common::Coord3D| Vec3::new(p.x, p.z, p.y);
            let center = (host(info.from_left) + host(info.to_right)) * 0.5;
            let mut ids: Vec<ObjectId> = self
                .objects
                .values()
                .filter(|object| {
                    object.is_kind_of(KindOf::Bridge)
                        && !used.contains(&object.id)
                        && (object.get_position() - center).length_squared() < 0.01
                })
                .map(|o| o.id)
                .collect();
            // The saved canonical ID is preferred; geometry only rebinds after
            // a native map reconstruction assigned fresh IDs before object xfer.
            ids.sort_by_key(|id| (id.0 != info.bridge_object_id, id.0));
            let Some(id) = ids.first().copied() else {
                self.pathfinding_system
                    .grid
                    .stamp_reserved_bridge_layer(layer, true);
                self.pathfinding_system.grid.deactivate_bridge_layer(layer);
                owner
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .detach_bridge_by_object_id(info.bridge_object_id);
                continue;
            };
            used.insert(id);
            self.pathfinding_system
                .grid
                .bind_reserved_bridge_object(layer, id.0);
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
                    if bridge.get_layer() as u8 == layer {
                        bridge.set_bridge_object_id(id.0);
                        for (i, tower) in towers.iter().enumerate() {
                            if let Some(kind) = gamelogic::common::BridgeTowerType::from_index(i) {
                                bridge.set_tower_object_id(tower.0, kind);
                            }
                        }
                    }
                });
        }
        self.seed_pathfinding_from_terrain();
        self.refresh_owned_bridge_pathfinder_states();
    }

    /// Terrain phase reads canonical bodies/scaffolds; no extra behavior tick.
    pub(crate) fn refresh_owned_bridge_pathfinder_states(&mut self) {
        for (layer, id, old_destroyed, unclassified) in
            self.pathfinding_system.grid.admitted_bridge_states()
        {
            let destroyed = if id == 0 {
                old_destroyed
            } else {
                self.objects.get(&ObjectId(id)).is_none_or(|o| {
                    o.health.current <= 0.0 || o.health.maximum <= 0.0
                        || o.body_damage_state == crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                        || self.bridge_behavior.is_scaffold_present(ObjectId(id))
                })
            };
            if unclassified || destroyed != old_destroyed {
                self.pathfinding_system
                    .grid
                    .stamp_reserved_bridge_layer(layer, destroyed);
            }
        }
    }

    pub(in super::super) fn sync_owned_bridge_body_state(
        &mut self,
        id: ObjectId,
        state: gamelogic::common::BodyDamageType,
    ) {
        let owner = self.world_services.terrain().clone();
        owner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .set_bridge_damage_state_for_object(id.0, state);
    }

    /// CPP TerrainLogic::deleteBridge: unlink, disable exact layer, destroy,
    /// then release the detached bridge. Slots remain reserved until reset.
    pub(crate) fn delete_owned_bridge_at(&mut self, pos: Vec3) -> bool {
        let owner = self.world_services.terrain().clone();
        let removed = owner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .detach_bridge_at(&gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y));
        let Some(bridge) = removed else {
            return false;
        };
        let id = ObjectId(bridge.get_bridge_info().bridge_object_id);
        self.pathfinding_system
            .grid
            .stamp_reserved_bridge_layer(bridge.get_layer() as u8, true);
        self.pathfinding_system
            .grid
            .deactivate_bridge_layer(bridge.get_layer() as u8);
        self.destroy_object(id);
        self.bridge_behavior.unregister_span(id);
        drop(bridge);
        true
    }
}
