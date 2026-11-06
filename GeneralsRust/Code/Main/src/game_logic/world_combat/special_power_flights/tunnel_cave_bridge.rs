//! Tunnel network, cave system and bridge scaffold/rubble flows.

use super::*;

impl GameLogic {
    /// Residual Tunnel Network honesty: enter count.
    pub fn tunnel_network_residual_enters(&self) -> u32 {
        self.tunnel_network.enters
    }

    /// Residual Tunnel Network honesty: exit count.
    pub fn tunnel_network_residual_exits(&self) -> u32 {
        self.tunnel_network.exits
    }

    /// Residual Tunnel Network honesty: cross-tunnel exit count (enter A, exit B).
    pub fn tunnel_network_residual_cross_exits(&self) -> u32 {
        self.tunnel_network.cross_exits
    }

    /// Residual honesty: tunnel enter → exit path.
    pub fn honesty_tunnel_network_enter_exit_ok(&self) -> bool {
        self.tunnel_network.honesty_enter_exit_ok()
    }

    /// Residual honesty: cross-tunnel exit (enter A, exit B) exercised.
    pub fn honesty_tunnel_network_cross_exit_ok(&self) -> bool {
        self.tunnel_network.honesty_cross_exit_ok()
    }

    /// Residual honesty: any tunnel network residual path.
    pub fn honesty_tunnel_network_ok(&self) -> bool {
        self.tunnel_network.honesty_any_ok()
    }

    /// Residual honesty: TunnelNetworkGun auto-fire residual exercised.
    pub fn honesty_tunnel_network_gun_ok(&self) -> bool {
        self.tunnel_network.honesty_gun_fire_ok()
    }

    /// Residual honesty counter: TunnelNetworkGun residual shots.
    pub fn tunnel_network_residual_gun_fires(&self) -> u32 {
        self.tunnel_network.gun_fires
    }

    /// Residual honesty counter: TunnelNetworkGun residual units hit.
    pub fn tunnel_network_residual_gun_units_hit(&self) -> u32 {
        self.tunnel_network.gun_units_hit
    }

    /// Shared tunnel pool accessors for command residual.
    pub fn tunnel_network_residual(
        &self,
    ) -> &crate::game_logic::host_tunnel_network::HostTunnelNetworkRegistry {
        &self.tunnel_network
    }

    /// Exit one unit from the player's tunnel network at `exit_tunnel`.
    /// Removes local occupant bookkeeping from the entry tunnel if different.
    /// Returns true when the unit was in the shared pool and was released.
    pub fn exit_tunnel_network_unit(&mut self, unit_id: ObjectId, exit_tunnel: ObjectId) -> bool {
        let Some(player_id) = self.tunnel_network.player_holding_unit(unit_id) else {
            return false;
        };
        let entry = self
            .tunnel_network
            .record_exit(player_id, unit_id, exit_tunnel);
        // Remove from entry tunnel local list (and exit tunnel if mirrored).
        if let Some(entry_id) = entry {
            if let Some(container) = self.objects.get_mut(&entry_id) {
                container.remove_occupant_with_partition_owner(
                    unit_id,
                    &mut self.ownership_events,
                    &mut self.partition_manager,
                );
            }
        }
        if entry != Some(exit_tunnel) {
            if let Some(container) = self.objects.get_mut(&exit_tunnel) {
                container.remove_occupant_with_partition_owner(
                    unit_id,
                    &mut self.ownership_events,
                    &mut self.partition_manager,
                );
            }
        }
        // C++ TunnelContain::onRemoving setSafeOcclusionFrame(frame + OcclusionDelay).
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.set_status_disabled_held(false);
            unit.stamp_safe_occlusion_frame(self.frame);
        }

        true
    }

    /// List all units in a player's shared tunnel pool (for Evacuate residual).
    pub fn tunnel_network_contained_for_player(&self, player_id: u32) -> Vec<ObjectId> {
        self.tunnel_network.contained_for_player(player_id)
    }

    /// C++ CaveContain.cpp:254-336 changeTeamOnAllConnectedCaves.
    pub(in crate::game_logic) fn apply_cave_capture_event_with_effects(
        &mut self,
        index: i32,
        event: crate::game_logic::host_cave_system::CaveCaptureEvent,
        entrance: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::host_cave_system::CaveCaptureEvent;
        match event {
            CaveCaptureEvent::None => {}
            CaveCaptureEvent::FirstOccupant(_) => {
                let owner = self
                    .cave_system
                    .contained_for_index(index)
                    .first()
                    .and_then(|id| self.objects.get(id))
                    .and_then(|rider| rider.owner_player_id);
                let Some(owner) = owner else {
                    return Ok(());
                };
                let player = self.players.get(&owner).ok_or_else(|| {
                    gamelogic::GameLogicError::Configuration(
                        "Cave rider controller is missing from the receiving world".into(),
                    )
                })?;
                if player.default_team_id.is_none() {
                    return Ok(());
                }
                let destination = self.host_default_team_id(owner).ok_or_else(|| {
                    gamelogic::GameLogicError::Configuration(
                        "Cave rider controller has no receiving default Team".into(),
                    )
                })?;
                for cid in self.cave_system.cave_ids_for_index(index) {
                    if let Some(cave) = self.objects.get_mut(&cid) {
                        let Some(building) = cave.building_data.as_mut() else {
                            continue;
                        };
                        building.original_team = cave.team_instance_id.map(|_| cave.team);
                        building.original_garrison_player_id =
                            cave.team_instance_id.and(cave.owner_player_id);
                        building.original_team_instance_id = cave.team_instance_id;
                        building.original_team_instance_name = cave
                            .team_instance_id
                            .map(|_| cave.team_instance_name.clone());
                    }
                    self.defect_host_object_with_effects(cid, destination, 0, effects)?;
                }
            }
            CaveCaptureEvent::LastEmpty => {
                // C++ onRemoving uses the exiting cave's saved Team for all
                // connected caves, even when they began on different Teams.
                let original = self
                    .objects
                    .get(&entrance)
                    .and_then(|cave| cave.building_data.as_ref()?.original_team_instance_id);
                for cid in self.cave_system.cave_ids_for_index(index) {
                    if let Some(cave) = self.objects.get_mut(&cid) {
                        if let Some(building) = cave.building_data.as_mut() {
                            building.original_team = None;
                            building.original_garrison_player_id = None;
                            building.original_team_instance_id = None;
                            building.original_team_instance_name = None;
                        }
                    }
                    if let Some(original) = original {
                        self.defect_host_object_with_effects(cid, original, 0, effects)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// C++ ScriptActions.cpp:5063 SET_CAVE_INDEX / CaveContain::tryToSetCaveIndex.
    pub fn try_set_cave_index(&mut self, cave_id: ObjectId, new_index: i32) -> bool {
        if !self
            .objects
            .get(&cave_id)
            .is_some_and(|o| o.is_cave_style_container())
        {
            return false;
        }
        if !self.cave_system.try_set_cave_index(cave_id, new_index) {
            return false;
        }
        if let Some(obj) = self.objects.get_mut(&cave_id) {
            obj.cave_index = new_index;
        }
        true
    }

    /// C++ ScriptActions::doSetCaveIndex named-object lookup.
    pub fn set_named_cave_index(&mut self, cave_name: &str, new_index: i32) -> bool {
        let Some(id) = self.find_object_id_by_name(cave_name) else {
            return false;
        };
        self.try_set_cave_index(id, new_index)
    }

    pub fn exit_cave_unit(&mut self, unit_id: ObjectId, exit_cave: ObjectId) -> bool {
        match self.exit_cave_unit_with_effects(unit_id, exit_cave, &mut Default::default()) {
            Ok(exited) => exited,
            Err(error) => {
                log::error!("Cave owner restoration failed: {error}");
                false
            }
        }
    }

    pub(crate) fn exit_cave_unit_with_effects(
        &mut self,
        unit_id: ObjectId,
        exit_cave: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(index) = self.cave_system.index_holding_unit(unit_id) else {
            return Ok(false);
        };
        let (entry, ev) = self.cave_system.record_exit(index, unit_id, exit_cave);
        if let Some(entry_id) = entry {
            if let Some(container) = self.objects.get_mut(&entry_id) {
                container.remove_occupant_with_partition_owner(
                    unit_id,
                    &mut self.ownership_events,
                    &mut self.partition_manager,
                );
            }
        }
        if entry != Some(exit_cave) {
            if let Some(container) = self.objects.get_mut(&exit_cave) {
                container.remove_occupant_with_partition_owner(
                    unit_id,
                    &mut self.ownership_events,
                    &mut self.partition_manager,
                );
            }
        }
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.set_status_disabled_held(false);
        }
        self.apply_cave_capture_event_with_effects(index, ev, exit_cave, effects)?;
        Ok(true)
    }

    pub fn cave_system_residual(&self) -> &crate::game_logic::host_cave_system::HostCaveSystem {
        &self.cave_system
    }

    pub(crate) fn resolve_bridge_span_for_repair(&self, target_id: ObjectId) -> Option<ObjectId> {
        if let Some(sid) = self.bridge_behavior.span_id_for(target_id) {
            return Some(sid);
        }
        if self.bridge_behavior.span(target_id).is_some() {
            return Some(target_id);
        }
        let pos = self.objects.get(&target_id)?.get_position();
        self.objects
            .registered_values()
            .filter(|o| {
                crate::game_logic::host_bridge_behavior::is_bridge_span_template(&o.template_name)
                    || o.is_kind_of(crate::game_logic::KindOf::Bridge)
            })
            .min_by(|a, b| {
                a.get_position()
                    .distance(pos)
                    .partial_cmp(&b.get_position().distance(pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|o| o.id)
    }

    pub(crate) fn spawn_bridge_scaffolding(&mut self, span_id: ObjectId) {
        self.spawn_bridge_scaffolding_with_effects(
            span_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_bridge_scaffolding_with_effects(
        &mut self,
        span_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if !self.bridge_behavior.create_scaffolding(span_id) {
            return;
        }
        let team = self
            .objects
            .get(&span_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let center = self
            .bridge_behavior
            .span(span_id)
            .map(|s| (s.from_left + s.from_right + s.to_left + s.to_right) * 0.25)
            .unwrap_or(glam::Vec3::ZERO);
        let tiles = self.bridge_behavior.tiled_scaffold_sites(span_id);
        let mut ids = Vec::new();
        let mut anims = Vec::new();
        for tile in tiles {
            let template = if tile.is_support {
                crate::game_logic::host_bridge_behavior::BRIDGE_SCAFFOLD_SUPPORT_TEMPLATE
            } else {
                crate::game_logic::host_bridge_behavior::BRIDGE_SCAFFOLD_TEMPLATE
            };
            if let Some(sid) = self.create_object_with_script_effects(
                template,
                team,
                tile.create_pos,
                None,
                presentation_effects,
            ) {
                if let Some(obj) = self.objects.get_mut(&sid) {
                    obj.set_orientation_with_partition_owner(
                        tile.angle,
                        &mut self.partition_manager,
                    );
                }
                ids.push(sid);
                anims.push(
                    crate::game_logic::host_bridge_behavior::HostScaffoldAnim::from_tile(
                        sid, &tile, center,
                    ),
                );
            }
        }
        if let Some(span) = self.bridge_behavior.span_mut(span_id) {
            span.scaffold_ids.extend(ids);
        }
        self.bridge_behavior.bind_scaffold_anims(span_id, anims);
        if let Some(span) = self.bridge_behavior.span(span_id) {
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
                self.pathfinding_system.grid.stamp_bridge_deck(
                    span.from_left,
                    span.from_right,
                    span.to_left,
                    span.to_right,
                    true,
                    &ground_source,
                )
            });
        }
    }

    /// C++ `WorkerAIUpdate.cpp:830` / `DozerAIUpdate::removeBridgeScaffolding`.
    pub(crate) fn remove_bridge_scaffolding(&mut self, span_id: ObjectId) {
        let _ = self.bridge_behavior.remove_scaffolding(span_id);
        let rubble = self.objects.get(&span_id).is_some_and(|o| {
            o.body_damage_state
                == crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                || o.health.current <= 0.0
        });
        if !rubble {
            if let Some(span) = self.bridge_behavior.span(span_id) {
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
                    self.pathfinding_system.grid.stamp_bridge_deck(
                        span.from_left,
                        span.from_right,
                        span.to_left,
                        span.to_right,
                        false,
                        &ground_source,
                    )
                });
            }
        }
    }

    pub(in crate::game_logic) fn leftover_bridge_body_state(
        state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    ) -> gamelogic::common::BodyDamageType {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        match state {
            HostBodyDamageType::Pristine => gamelogic::common::BodyDamageType::Pristine,
            HostBodyDamageType::Damaged => gamelogic::common::BodyDamageType::Damaged,
            HostBodyDamageType::ReallyDamaged => gamelogic::common::BodyDamageType::ReallyDamaged,
            HostBodyDamageType::Rubble => gamelogic::common::BodyDamageType::Rubble,
        }
    }

    pub(super) fn play_bridge_die_fx_at(&mut self, span_id: ObjectId) {
        self.play_bridge_die_fx_at_with_effects(
            span_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(super) fn play_bridge_die_fx_at_with_effects(
        &mut self,
        span_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let pos = self
            .bridge_behavior
            .span(span_id)
            .map(|s| s.random_surface_position(self.frame))
            .or_else(|| self.objects.get(&span_id).map(|o| o.get_position()))
            .unwrap_or(glam::Vec3::ZERO);
        let _ = self.dispatch_fx_list_at_host_position_with_effects(
            crate::game_logic::host_bridge_behavior::BRIDGE_DIE_FX_NAME,
            pos,
            presentation_effects,
        );
        if let Some(obj) = self.objects.get_mut(&span_id) {
            if obj.pending_death_fx.is_none() {
                obj.pending_death_fx =
                    Some(crate::game_logic::host_bridge_behavior::BRIDGE_DIE_FX_NAME.to_string());
            }
        }
        self.fire_owned_fx_list_die_with_effects(span_id, presentation_effects);
    }

    pub(super) fn play_bridge_die_ocl_at_with_effects(
        &mut self,
        span_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let pos = self
            .bridge_behavior
            .span(span_id)
            .map(|s| s.random_surface_position(self.frame.wrapping_add(31)))
            .or_else(|| self.objects.get(&span_id).map(|o| o.get_position()))
            .unwrap_or(glam::Vec3::ZERO);
        self.execute_bridge_ocl_at_with_effects(
            crate::game_logic::host_bridge_behavior::BRIDGE_DIE_OCL_NAME,
            span_id,
            pos,
            presentation_effects,
        );
    }

    /// Create a bridge cue through this GameLogic's admitted OCL catalog and
    /// object templates. Snapshot the source facts before creation can mutate
    /// this world's object store; the bridge itself remains installed.
    pub(super) fn execute_bridge_ocl_at_with_effects(
        &mut self,
        ocl: &str,
        span_id: ObjectId,
        position: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some((team, veterancy, orientation, velocity)) =
            self.objects.get(&span_id).map(|bridge| {
                (
                    bridge.team,
                    bridge.experience.level,
                    bridge.get_orientation(),
                    bridge.movement.velocity,
                )
            })
        else {
            return;
        };
        let _ = self.execute_parsed_weapon_ocl_at_with_effects(
            ocl,
            Some(span_id),
            team,
            veterancy,
            orientation,
            velocity,
            position,
            presentation_effects,
        );
    }

    pub(in crate::game_logic) fn play_bridge_body_transition(
        &mut self,
        span_id: ObjectId,
        old_state: u8,
        new_state: u8,
    ) {
        self.play_bridge_body_transition_with_effects(
            span_id,
            old_state,
            new_state,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn play_bridge_body_transition_with_effects(
        &mut self,
        span_id: ObjectId,
        old_state: u8,
        new_state: u8,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let cue = self
            .bridge_behavior
            .body_transition_cues(old_state, new_state);
        if let Some(sound) = cue.sound.as_deref() {
            let pos = self
                .objects
                .get(&span_id)
                .map(|o| o.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            self.queue_audio_event(
                AudioEventRequest::new(sound)
                    .with_object(span_id)
                    .with_position(pos)
                    .with_priority(160),
            );
        }
        for (i, fx) in cue.fx.iter().enumerate() {
            let pos = self
                .bridge_behavior
                .span(span_id)
                .map(|s| s.random_surface_position(self.frame.wrapping_add(i as u32 * 7 + 1)))
                .unwrap_or(glam::Vec3::ZERO);
            let _ = self.dispatch_fx_list_at_host_position_with_effects(fx, pos, effects);
        }
        for (i, ocl) in cue.ocl.iter().enumerate() {
            let pos = self
                .bridge_behavior
                .span(span_id)
                .map(|s| s.random_surface_position(self.frame.wrapping_add(i as u32 * 11 + 3)))
                .unwrap_or(glam::Vec3::ZERO);
            self.execute_bridge_ocl_at_with_effects(ocl, span_id, pos, effects);
        }
    }

    pub(in super::super::super) fn sync_host_bridge_rubble_and_scaffolds(
        &mut self,
    ) -> gamelogic::GameLogicResult<()> {
        self.sync_host_bridge_rubble_and_scaffolds_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super::super) fn sync_host_bridge_rubble_and_scaffolds_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // Each move belongs to the scaffold birth observed before any of the
        // destruction or pose callbacks below can reuse another scaffold ID.
        let moved: Vec<_> = self
            .bridge_behavior
            .tick_scaffolds()
            .into_iter()
            .filter_map(|(id, position, finished)| {
                self.objects.get(&id).map(|object| {
                    (
                        id,
                        position,
                        finished,
                        std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                    )
                })
            })
            .collect();
        for (sid, pos, finished_sink, birth) in moved {
            let is_original = |world: &Self| {
                world.objects.get(&sid).is_some_and(|object| {
                    birth.ptr_eq(&std::sync::Arc::downgrade(
                        &object.visual_draw_identity_token,
                    ))
                })
            };
            if !is_original(self) {
                continue;
            }
            if finished_sink {
                self.destroy_object_with_effects(sid, presentation_effects)?;
            }
            if is_original(self) {
                let _ = self.set_owned_object_position_with_effects(
                    sid,
                    pos,
                    self.frame,
                    presentation_effects,
                );
            }
        }
        let span_ids: Vec<ObjectId> = self
            .objects
            .registered_values()
            .filter(|o| {
                crate::game_logic::host_bridge_behavior::is_bridge_span_template(&o.template_name)
                    || o.is_kind_of(crate::game_logic::KindOf::Bridge)
            })
            .map(|o| o.id)
            .collect();
        for id in span_ids {
            let Some((hp, max, pos, radius, body_state)) = self.objects.get(&id).map(|o| {
                (
                    o.health.current,
                    o.health.maximum,
                    o.get_position(),
                    o.selection_radius.max(20.0),
                    o.body_damage_state,
                )
            }) else {
                continue;
            };
            if self.bridge_behavior.span(id).is_none() {
                self.bridge_behavior.register_span(
                    id,
                    glam::Vec3::new(pos.x - radius, 0.0, pos.z - radius),
                    glam::Vec3::new(pos.x + radius, 0.0, pos.z - radius),
                    glam::Vec3::new(pos.x - radius, 0.0, pos.z + radius),
                    glam::Vec3::new(pos.x + radius, 0.0, pos.z + radius),
                );
            }
            let old_state = self
                .bridge_behavior
                .span(id)
                .map(|s| s.last_body_state)
                .unwrap_or(0);
            if self
                .bridge_behavior
                .note_body_state(id, body_state.ordinal())
            {
                self.sync_owned_bridge_body_state(id, Self::leftover_bridge_body_state(body_state));
                self.play_bridge_body_transition_with_effects(
                    id,
                    old_state,
                    body_state.ordinal(),
                    presentation_effects,
                );
                if matches!(
                    body_state,
                    crate::game_logic::host_enum_table_residual::HostBodyDamageType::Damaged
                        | crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged
                        | crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine
                ) {
                    self.host_radar_queue_terrain_refresh();
                }
            }
            let rubble = max <= 0.0
                || hp <= 0.0
                || matches!(
                    body_state,
                    crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                );
            if rubble {
                let positions: Vec<(ObjectId, glam::Vec3)> = self
                    .objects
                    .registered()
                    .filter(|(oid, o)| {
                        **oid != id
                            && o.is_alive()
                            && !crate::game_logic::host_bridge_behavior::is_bridge_or_tower_template(
                                &o.template_name,
                            )
                    })
                    .map(|(oid, o)| (*oid, o.get_position()))
                    .collect();
                let occupants = self.bridge_behavior.occupants_on_deck(id, &positions);
                self.sync_owned_bridge_body_state(id, gamelogic::common::BodyDamageType::Rubble);

                if self.bridge_behavior.on_enter_rubble(id, &occupants) {
                    self.bridge_behavior.mark_death(id, self.frame);
                    if let Some(layer) = self.pathfinding_system.grid.bridge_layer_for_object(id.0)
                    {
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
                    }
                    for uid in occupants {
                        if let Some(unit) = self.objects.get_mut(&uid) {
                            let receiving_damage_target_id = unit.id;
                            let _ = self.apply_residual_damage_owned_with_effects(
                                receiving_damage_target_id,
                                crate::game_logic::host_bridge_behavior::BRIDGE_SPLAT_DAMAGE,
                                Some(id),
                                crate::game_logic::combat::DamageType::Falling,
                                crate::game_logic::host_usa_pilot::HostDeathType::Splatted,
                                None,
                                presentation_effects,
                            );
                        }
                    }
                    self.host_radar_queue_terrain_refresh();
                }
                let due = self.bridge_behavior.take_due_die_fx(id, self.frame);
                for _ in 0..due {
                    self.play_bridge_die_fx_at_with_effects(id, presentation_effects);
                }
                let due_ocl = self.bridge_behavior.take_due_die_ocl(id, self.frame);
                for _ in 0..due_ocl {
                    self.play_bridge_die_ocl_at_with_effects(id, presentation_effects);
                }
            } else {
                self.bridge_behavior.on_leave_rubble(id);
                if !self.bridge_behavior.is_scaffold_present(id) {
                    if let Some(layer) = self.pathfinding_system.grid.bridge_layer_for_object(id.0)
                    {
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
                                false,
                                &ground_source,
                            )
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
