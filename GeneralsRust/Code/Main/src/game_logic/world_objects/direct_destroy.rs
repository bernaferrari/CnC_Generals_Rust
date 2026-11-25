//! Owner-local direct deletion, distinct from body death and physical removal.
//! C++ GameLogic.cpp3935–3980 / Object.cpp722–742.
use super::super::*;

#[cfg(test)]
mod tests;

impl GameLogic {
    pub(crate) fn has_pending_object_removals(&self) -> bool {
        !self.objects_to_destroy.is_empty() || self.objects.failed_retirement_reason().is_some()
    }

    pub(in super::super) fn destroy_live_object(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        self.destroy_live_object_with_effects(id, &mut Default::default())
    }

    pub(in super::super) fn destroy_live_object_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(object) = self.objects.get_mut(&id) else {
            return Ok(());
        };
        if object.status.destroyed {
            return Ok(());
        }
        object.status.destroyed = true;
        object.set_locomotor_goal_none();
        object.movement.clear_path();
        let wall_piece = object.is_kind_of(KindOf::WalkOnTopOfWall);
        // Queue before OpenContain child deletion. Reentrant deletion sees
        // the DESTROYED latch and appends distinct children to this same list.
        self.objects_to_destroy
            .push_back(DestructionEvent::direct(id));
        self.apply_object_delete_callbacks_with_effects(id, effects)?;
        if wall_piece {
            self.pathfinding_system.unregister_wall_piece(id);
        }
        Ok(())
    }

    /// Complete the supported onDelete slice on the installed Object.
    /// This also applies when onDie began earlier; deferred death effects
    /// keep their existing queue completion and are not replayed here.
    pub(in super::super) fn apply_object_delete_callbacks(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_object_delete_callbacks_with_effects(id, &mut Default::default())
    }

    pub(in super::super) fn apply_object_delete_callbacks_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.object_lifecycle_callback_depth += 1;
        let result = self.apply_object_delete_callbacks_body_with_effects(id, effects);
        self.object_lifecycle_callback_depth -= 1;
        result
    }

    fn apply_object_delete_callbacks_body_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
        let definition = object.get_template().receiving_object_definition.clone();

        // Object::onDestroy removes this rider before entering its behavior list.
        self.detach_destroyed_object_from_contain(id, effects)?;
        if !self.object_birth_matches(id, &birth) {
            return Ok(());
        }
        if let Some(definition) = definition {
            let mut cost_modifiers = None;
            let mut radar_index = 0;
            for (index, module) in definition.behavior_modules.iter().enumerate() {
                if !self.object_birth_matches(id, &birth) {
                    return Ok(());
                }
                let is = |class: &str| module.class_name.eq_ignore_ascii_case(class);
                if is("CostModifierUpgrade") {
                    let slots = cost_modifiers
                        .get_or_insert_with(|| self.authored_cost_modifier_modules(id).into_iter());
                    if let Some(cost) = slots.next() {
                        self.delete_cost_modifier_slot(id, &cost);
                    }
                } else if is("PowerPlantUpgrade") || is("OverchargeBehavior") {
                    self.delete_plant_power_bonus_slot(id, index);
                } else if is("RadarUpgrade") {
                    self.delete_radar_upgrade_slot(id, radar_index);
                    radar_index += 1;
                } else if is("PropagandaTowerBehavior") {
                    self.remove_propaganda_tower_influence(id);
                } else if is("SpawnBehavior") {
                    self.delete_required_spawn_children(id, effects)?;
                } else if [
                    "TransportContain",
                    "RiderChangeContain",
                    "RailedTransportContain",
                    "GarrisonContain",
                    "InternetHackContain",
                    "HealContain",
                    "TunnelContain",
                    "CaveContain",
                    "HelixContain",
                    "OverlordContain",
                    "ParachuteContain",
                ]
                .into_iter()
                .any(is)
                {
                    self.delete_direct_containment_with_effects(id, effects)?;
                }
            }
            if !self.object_birth_matches(id, &birth) {
                return Ok(());
            }
            if radar_index == 0 {
                self.remove_host_radar_provider(id);
            } else {
                self.host_radar.providers.remove(&id);
            }
        } else {
            self.remove_propaganda_tower_influence(id);
            self.delete_direct_containment_with_effects(id, effects)?;
            self.delete_required_spawn_children(id, effects)?;
            if !self.object_birth_matches(id, &birth) {
                return Ok(());
            }
            self.remove_host_radar_provider(id);
        }
        if let Some(object) = self.objects.get_mut(&id) {
            object.handle_partition_cell_maintenance_with_partition_owner(
                &mut self.partition_manager,
            );
        }
        Ok(())
    }

    fn delete_required_spawn_children(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // SpawnBehavior::onDelete destroys live required children directly,
        // unlike onDie which kills them through their death modules.
        let spawn_children = self.objects.get(&id).and_then(|parent| {
            Self::spawn_behavior_requires_spawner(parent).then(|| {
                (
                    parent.visual_object_generation,
                    parent.spawn_behavior_children(),
                )
            })
        });
        if let Some((parent_birth, children)) = spawn_children {
            let epoch = self.visual_world_epoch;
            for (child_id, birth) in children {
                if self.visual_world_epoch != epoch
                    || !self.objects.get(&id).is_some_and(|parent| {
                        parent.visual_object_generation == parent_birth
                            && parent.contains_spawn_behavior_child(child_id, &birth)
                    })
                {
                    continue;
                }
                if self.objects.get(&child_id).is_some_and(|child| {
                    birth != 0
                        && child.visual_object_generation == birth
                        && !child.status.effectively_dead
                }) {
                    self.destroy_object_with_effects(child_id, effects)?;
                }
            }
        }
        Ok(())
    }

    /// Containment onDelete. Cave's shared cave-in belongs to onDie.
    fn delete_direct_containment(&mut self, id: ObjectId) -> gamelogic::GameLogicResult<()> {
        self.delete_direct_containment_with_effects(id, &mut Default::default())
    }

    fn delete_direct_containment_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let redirected = self.redirected_containment_owner(id);
        if redirected != id {
            let children = self
                .teardown_object(redirected)
                .map(|object| object.contained_units())
                .unwrap_or_default();
            // CPP onDelete removes the child's contents before switching back
            // to the hull's base list; callbacks still see both owners.
            for child in children {
                self.remove_direct_contained_child_with_effects(redirected, child, effects)?;
            }
            if let Some(outer) = self.teardown_object_mut(id) {
                outer.overlord_redirection_active = false;
            }
            let payloads = self
                .teardown_object(id)
                .map(|object| object.occupants.clone())
                .unwrap_or_default();
            for payload in payloads {
                self.remove_direct_contained_child_with_effects(id, payload, effects)?;
            }
            return Ok(());
        }
        if let Some(portable) = self.objects.get(&id)
            .filter(|object| object.is_helix_transport)
            .and_then(|object| object.overlord_portable_occupant)
        {
            self.destroy_object_with_effects(portable, effects)?;
        }
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        if object.is_cave_style_container() {
            return Ok(());
        }
        if object.is_tunnel_network_style_container() {
            self.unregister_tunnel_containment_with_effects(id, effects)?;
            return Ok(());
        }
        if object.is_overlord_style_container() {
            let children = object.occupants.clone();
            for child in children {
                self.destroy_object_with_effects(child, effects)?;
            }
            return Ok(());
        }
        let kind = object.thing().template.contain_module.kind;
        if matches!(
            kind,
            ContainModuleKind::Transport
                | ContainModuleKind::RiderChange
                | ContainModuleKind::RailedTransport
                | ContainModuleKind::Garrison
                | ContainModuleKind::InternetHack
                | ContainModuleKind::Heal
                | ContainModuleKind::Parachute
        ) {
            let children = object.contained_units();
            for child in children {
                self.destroy_object_with_effects(child, effects)?;
            }
        }
        Ok(())
    }

    /// CPP OverlordContain::onDelete uses removeAllContained, not destroy.
    /// Complete each removal while both identities remain installed. Do not
    /// issue a movement order: removeAllContained is not exitObjectViaDoor.
    pub(in crate::game_logic) fn remove_direct_contained_child(
        &mut self,
        container_id: ObjectId,
        child_id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        self.remove_direct_contained_child_with_effects(
            container_id,
            child_id,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn remove_direct_contained_child_with_effects(
        &mut self,
        container_id: ObjectId,
        child_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(container) = self.teardown_object(container_id) else {
            return Ok(());
        };
        let Some(child) = self.teardown_object(child_id) else { return Ok(()) };
        if !container.contained_unit_ids().contains(&child_id)
            || child.contained_by != Some(container_id)
        {
            return Ok(());
        }
        let kind = container.get_template().contain_module.kind;
        if kind == ContainModuleKind::RailedTransport {
            self.railed_remove_contained_with_effects(container_id, child_id, effects)
                .map_err(gamelogic::GameLogicError::Configuration)?;
            return Ok(());
        }
        if kind == ContainModuleKind::Parachute {
            return self.remove_parachute_rider_owned(container_id, child_id, effects);
        }
        if kind == ContainModuleKind::RiderChange {
            if container.status.effectively_dead {
                // RiderChangeContain::onRemoving destroys a dead bike's hidden rider.
                self.destroy_object_with_effects(child_id, effects)?;
            } else {
                self.rider_change_remove_occupant_with_effects(container_id, child_id, effects);
            }
            return Ok(());
        }
        let container_birth = std::sync::Arc::downgrade(&container.visual_draw_identity_token);
        let child_birth = std::sync::Arc::downgrade(&child.visual_draw_identity_token);
        let garrison = container.is_garrison_contain();
        let helix = container.is_helix_transport;
        let transport = matches!(
            kind,
            ContainModuleKind::Transport
                | ContainModuleKind::InternetHack
        ) || container.is_overlord_style_container();
        let airborne = container.is_above_terrain_for_exit();
        let velocity = container
            .transport_keep_container_velocity_on_exit()
            .then_some(container.movement.velocity);
        let aggressive = container.transport_go_aggressive_on_exit();
        let delay = container.transport_exit_delay_frames();
        if !self.remove_owned_occupant_with_bonus_callback(container_id, child_id, effects)
            || !self.object_birth_matches(container_id, &container_birth)
            || !self.object_birth_matches(child_id, &child_birth)
        {
            return Ok(());
        }
        if let Some(container) = self.objects.get_mut(&container_id) {
            if container.overlord_portable_occupant == Some(child_id) {
                container.overlord_portable_occupant = None;
            }
        }
        if !garrison && !helix {
            self.play_container_removing_template_sounds(container_id, child_id, effects);
        }
        if self.objects.contains_key(&child_id) {
            Self::set_owned_containment_in_parts(
                &mut self.objects,
                &mut self.partition_manager,
                &mut self.ai_groups,
                &mut self.pathfinding_system,
                child_id,
                None,
                true,
            );
            let child = self
                .objects
                .get_mut(&child_id)
                .expect("installed detached child");
            if transport && !helix {
                if airborne {
                    child.allow_to_fall = true;
                }
                if let Some(velocity) = velocity {
                    child.apply_motive_force(velocity * child.physics_get_mass(), self.frame);
                }
                if aggressive {
                    child.set_ai_attitude(
                        crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive,
                    );
                }
            }
        }
        if garrison {
            self.recalc_garrison_apparent_controller_with_effects(container_id, effects)?;
        }
        if transport && !helix {
            self.scatter_dying_transport_rider_with_effects(container_id, child_id, effects);
            self.reset_rider_mood_check_on_exit(container_id, child_id);
            let frame = self.frame;
            if let Some(container) = self.teardown_object_mut(container_id) {
                container.frame_exit_not_busy = frame.wrapping_add(delay);
                if container.contained_units().is_empty() {
                    let bit = crate::game_logic::host_enum_table_residual::loaded_model_bit();
                    container.model_condition_bits &= !(1u128 << bit);
                }
            }
            self.refresh_battle_bus_armed_riders_weapon_set(container_id);
        }
        Ok(())
    }

    /// C++ TunnelContain::onDie/onDelete share m_isCurrentlyRegistered.
    /// Keep the dying entrance installed while remapping or deleting children;
    /// the exact registered ID is consumed before any reentrant callback.
    pub(in super::super) fn unregister_tunnel_containment(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        self.unregister_tunnel_containment_with_effects(id, &mut Default::default())
    }

    pub(in super::super) fn unregister_tunnel_containment_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        if !object.is_tunnel_network_style_container() {
            return Ok(());
        }
        let player = object.tunnel_system_key();
        // CPP TunnelContain347: only a currently registered entrance
        // calls onTunnelDestroyed. Construction/reset may leave it inert.
        if !self.tunnel_network.tunnel_ids_for(player).contains(&id) {
            return Ok(());
        }
        let remaining: Vec<_> = self
            .tunnel_network
            .tunnel_ids_for(player)
            .iter()
            .copied()
            .filter(|other| {
                *other != id && self.objects.get(other).is_some_and(|o| !o.status.destroyed)
            })
            .collect();
        let outcome = self
            .tunnel_network
            .on_tunnel_destroyed(player, id, &remaining);
        if outcome.cave_in {
            for child in outcome.cave_in_units {
                if let Some(object) = self.objects.get_mut(&child) {
                    let containment_actor_id = object.id;
                    Self::set_owned_containment_in_parts(
                        &mut self.objects,
                        &mut self.partition_manager,
                        &mut self.ai_groups,
                        &mut self.pathfinding_system,
                        containment_actor_id,
                        None,
                        true,
                    );
                    let object = self
                        .objects
                        .get_mut(&containment_actor_id)
                        .expect("containment operation preserves this birth");
                }
                self.destroy_object_with_effects(child, effects)?;
            }
        } else if let Some(entrance) = outcome.remapped_to {
            for child in self.tunnel_network.contained_for_player(player) {
                if let Some(object) = self.objects.get_mut(&child) {
                    if object.contained_by == Some(id) {
                        let containment_actor_id = object.id;
                        Self::set_owned_containment_in_parts(
                            &mut self.objects,
                            &mut self.partition_manager,
                            &mut self.ai_groups,
                            &mut self.pathfinding_system,
                            containment_actor_id,
                            Some(entrance),
                            true,
                        );
                        let object = self
                            .objects
                            .get_mut(&containment_actor_id)
                            .expect("containment operation preserves this birth");
                        self.tunnel_network
                            .stamp_contained_by_frame(child, self.frame);
                    }
                }
            }
        }
        Ok(())
    }

    /// Object::onDestroy removes the exact occupant while both identities are
    /// still installed. Physical deletion must not be needed to free its slot.
    fn detach_destroyed_object_from_contain(
        &mut self, id: ObjectId, effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let containing = self.objects.get(&id).and_then(|o| o.contained_by);
        if let Some(cave) = containing {
            if self.cave_system.index_holding_unit(id).is_some_and(|index| {
                self.objects.get(&cave).is_some_and(|object| {
                    object.is_cave_style_container() && object.cave_index == index
                })
            }) {
                self.exit_cave_unit_with_effects(id, cave, effects)?;
                return Ok(());
            }
        }
        if let Some(cid) = containing {
            if self.objects.get(&cid).is_some_and(|container| {
                container.thing().template.contain_module.kind == ContainModuleKind::RailedTransport
            }) {
                self.railed_remove_contained_with_effects(cid, id, effects)
                    .map_err(gamelogic::GameLogicError::Configuration)?;
                return Ok(());
            }
            if self.objects.get(&cid).is_some_and(|container| {
                container.get_template().contain_module.kind == ContainModuleKind::RiderChange
                    && !container.status.effectively_dead
            }) {
                self.rider_change_remove_occupant_with_effects(cid, id, effects);
                return Ok(());
            }
            let garrison = self
                .objects
                .get(&cid)
                .is_some_and(|c| c.is_garrison_contain());
            if let Some(container) = self.objects.get_mut(&cid) {
                if container.overlord_portable_occupant == Some(id) {
                    container.overlord_portable_occupant = None;
                    container.overlord_redirection_active = false;
                }
            }
            self.remove_owned_occupant_with_bonus_callback(cid, id, effects);
            if let Some(object) = self.objects.get_mut(&id) {
                let containment_actor_id = object.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    &mut self.pathfinding_system,
                    containment_actor_id,
                    None,
                    true,
                );
                let object = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
            }
            if garrison {
                self.recalc_garrison_apparent_controller_with_effects(cid, effects)?;
            }
        }
        if let Some(player) = self.tunnel_network.player_holding_unit(id) {
            let _ = self
                .tunnel_network
                .record_exit(player, id, containing.unwrap_or(id));
        }
        // Cave-in clears contained_by before destruction and retains the
        // tracker roster until every direct-destruction callback has run.
        Ok(())
    }

    /// Physical-only queue completion: no onDie, body damage, bounty or death FX.
    pub(in super::super) fn finish_direct_object_removal(&mut self, id: ObjectId) -> bool {
        self.finish_direct_object_removal_checked(id, &mut Default::default())
            .expect("native physical Drawable retirement must complete")
    }

    pub(in super::super) fn finish_direct_object_removal_checked(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if let Some(error) = self.object_retirement_error() {
            return Err(error);
        }
        self.pending_special_abilities.remove(&id);
        self.pending_special_abilities
            .retain(|_, ability| ability.target_id() != id);
        self.finish_building_sound(id);
        let Some(object) = self.take_object_for_physical_retirement(id, effects)? else {
            return Ok(false);
        };
        self.remove_host_dock_approach_queue(id, object.dock_active_docker);
        #[cfg(test)]
        crate::game_logic::host_destroy_log::record(id);

        self.clear_removed_object_references(id);
        Ok(object.is_kind_of(KindOf::Structure))
    }

    pub(in super::super) fn clear_removed_object_references(&mut self, id: ObjectId) {
        self.beacon_client.remove(id);
        self.admit_local_drawable_deselected(id);
        for player in self.players.values_mut() {
            player.selected_objects.retain(|&selected| selected != id);
        }
        let targets: Vec<_> = self
            .objects
            .registered()
            .filter_map(|(other, object)| (object.target == Some(id)).then_some(*other))
            .collect();
        for other in targets {
            self.stop_attack_decision_aware(other);
        }
        for (_other, object) in self.objects.registered_mut() {
            if object.active_guard_target() == Some(id) {
                object.set_guard_target_data(None);
                if object.ai_state() == AIState::GuardingObject {
                    object.set_ai_state(
                        AIState::Idle,
                        self.frame,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                    {}
                }
            }
        }
    }
}
