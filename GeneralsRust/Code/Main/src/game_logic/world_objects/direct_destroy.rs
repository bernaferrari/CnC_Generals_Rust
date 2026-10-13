//! Owner-local direct deletion, distinct from body death and physical removal.
//! C++ GameLogic.cpp3935–3980 / Object.cpp722–742.
use super::super::*;

impl GameLogic {
    pub(crate) fn has_pending_object_removals(&self) -> bool {
        !self.objects_to_destroy.is_empty()
    }

    pub(in super::super) fn destroy_live_object(&mut self, id: ObjectId) {
        let Some(object) = self.objects.get_mut(&id) else {
            return;
        };
        if object.status.destroyed {
            return;
        }
        object.status.destroyed = true;
        object.set_locomotor_goal_none();
        object.movement.path.clear();
        // Queue before OpenContain child deletion. Reentrant deletion sees
        // the DESTROYED latch and appends distinct children to this same list.
        self.objects_to_destroy
            .push_back(DestructionEvent::direct(id));
        self.apply_object_delete_callbacks(id);
    }

    /// Complete the supported onDelete slice on the installed Object.
    /// This also applies when onDie began earlier; deferred death effects
    /// keep their existing queue completion and are not replayed here.
    pub(in super::super) fn apply_object_delete_callbacks(&mut self, id: ObjectId) {
        self.detach_destroyed_object_from_contain(id);
        self.delete_direct_containment(id);
        self.remove_host_radar_provider(id);
        if let Some(object) = self.objects.get_mut(&id) {
            if let Some(bone_fx) = object.bone_fx_damage.as_mut() {
                bone_fx.kill_running_particle_systems();
            }
            object.handle_partition_cell_maintenance();
        }
        if self
            .objects
            .get(&id)
            .is_some_and(|o| o.is_kind_of(KindOf::WalkOnTopOfWall))
        {
            self.pathfinding_system.remove_wall_piece(id);
        }
    }

    /// Supported containment onDelete operations; Cave's shared cave-in is
    /// an onDie operation. The complete authored behavior dispatcher remains
    /// separate from this local OpenContain / TunnelContain migration.
    fn delete_direct_containment(&mut self, id: ObjectId) {
        let redirected = self.redirected_containment_owner(id);
        if redirected != id {
            let children = self
                .host_object(redirected)
                .map(|object| object.contained_units())
                .unwrap_or_default();
            // CPP onDelete removes the child's contents before switching back
            // to the hull's base list; callbacks still see both owners.
            for child in children {
                self.remove_direct_contained_child(redirected, child);
            }
            if let Some(outer) = self.host_object_mut(id) {
                outer.overlord_redirection_active = false;
            }
            let payloads = self
                .host_object(id)
                .map(|object| object.occupants.clone())
                .unwrap_or_default();
            for payload in payloads {
                self.remove_direct_contained_child(id, payload);
            }
            return;
        }
        let Some(object) = self.objects.get(&id) else {
            return;
        };
        if object.is_cave_style_container() {
            return;
        }
        if object.is_tunnel_network_style_container() {
            self.unregister_tunnel_containment(id);
            return;
        }
        if object.is_overlord_style_container() {
            let children = object.occupants.clone();
            for child in children {
                self.destroy_object(child);
            }
            return;
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
        ) {
            let children = object.contained_units();
            for child in children {
                self.destroy_object(child);
            }
        }
    }

    /// CPP OverlordContain::onDelete uses removeAllContained, not destroy.
    /// Complete each removal while both identities remain installed. Do not
    /// issue a movement order: removeAllContained is not exitObjectViaDoor.
    pub(in crate::game_logic) fn remove_direct_contained_child(
        &mut self,
        container_id: ObjectId,
        child_id: ObjectId,
    ) {
        let Some(container) = self.host_object(container_id) else {
            return;
        };
        if !container.contained_units().contains(&child_id)
            || !self
                .host_object(child_id)
                .is_some_and(|child| child.contained_by == Some(container_id))
        {
            return;
        }
        if container.thing().template.contain_module.kind == ContainModuleKind::RiderChange
            && !container.is_alive()
        {
            // CPP RiderChangeContain245 destroys its hidden rider during onRemoving.
            self.destroy_object(child_id);
            return;
        }
        let garrison = container.is_garrison_contain();
        let transport = matches!(
            container.thing().template.contain_module.kind,
            ContainModuleKind::Transport
                | ContainModuleKind::RiderChange
                | ContainModuleKind::RailedTransport
                | ContainModuleKind::InternetHack
        ) || container.is_overlord_style_container();
        let airborne = container.is_above_terrain_for_exit();
        let velocity = container
            .transport_keep_container_velocity_on_exit()
            .then_some(container.movement.velocity);
        let aggressive = container.transport_go_aggressive_on_exit();
        let delay = container.transport_exit_delay_frames();
        if let Some(container) = self.host_object_mut(container_id) {
            container.remove_occupant(child_id);
            if container.overlord_portable_occupant == Some(child_id) {
                container.overlord_portable_occupant = None;
            }
        }
        self.play_container_removing_template_sounds(container_id, child_id);
        if let Some(child) = self.host_object_mut(child_id) {
            child.set_contained_by(None);
            if transport {
                child.status.disabled_held = false;
                if airborne {
                    child.allow_to_fall = true;
                }
                if let Some(velocity) = velocity {
                    child.apply_motive_force(velocity * child.physics_get_mass());
                }
                if aggressive {
                    child.set_ai_attitude(
                        crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive,
                    );
                }
            }
        }
        if garrison {
            self.recalc_garrison_apparent_controller(container_id);
        }
        if transport {
            self.scatter_dying_transport_rider(container_id, child_id);
            self.reset_rider_mood_check_on_exit(container_id, child_id);
            let frame = self.frame;
            if let Some(container) = self.host_object_mut(container_id) {
                container.frame_exit_not_busy = frame.wrapping_add(delay);
                if container.contained_units().is_empty() {
                    let bit = crate::game_logic::host_enum_table_residual::loaded_model_bit();
                    container.model_condition_bits &= !(1u128 << bit);
                    container.record_host_model_condition();
                }
            }
            self.refresh_battle_bus_armed_riders_weapon_set(container_id);
        }
    }

    /// C++ TunnelContain::onDie/onDelete share m_isCurrentlyRegistered.
    /// Keep the dying entrance installed while remapping or deleting children;
    /// the exact registered ID is consumed before any reentrant callback.
    pub(in super::super) fn unregister_tunnel_containment(&mut self, id: ObjectId) {
        let Some(object) = self.objects.get(&id) else {
            return;
        };
        if !object.is_tunnel_network_style_container() {
            return;
        }
        let player = object.tunnel_system_key();
        // CPP TunnelContain347: only a currently registered entrance
        // calls onTunnelDestroyed. Construction/reset may leave it inert.
        if !self.tunnel_network.tunnel_ids_for(player).contains(&id) {
            return;
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
                    object.set_contained_by(None);
                }
                self.destroy_object(child);
            }
        } else if let Some(entrance) = outcome.remapped_to {
            for child in self.tunnel_network.contained_for_player(player) {
                if let Some(object) = self.objects.get_mut(&child) {
                    if object.contained_by == Some(id) {
                        object.set_contained_by(Some(entrance));
                        self.tunnel_network
                            .stamp_contained_by_frame(child, self.frame);
                    }
                }
            }
        }
    }

    /// Object::onDestroy removes the exact occupant while both identities are
    /// still installed. Physical deletion must not be needed to free its slot.
    fn detach_destroyed_object_from_contain(&mut self, id: ObjectId) {
        let containing = self.objects.get(&id).and_then(|o| o.contained_by);
        if let Some(cid) = containing {
            let garrison = self
                .objects
                .get(&cid)
                .is_some_and(|c| c.is_garrison_contain());
            if let Some(container) = self.objects.get_mut(&cid) {
                if container.overlord_portable_occupant == Some(id) {
                    container.overlord_portable_occupant = None;
                    container.overlord_redirection_active = false;
                }
                container.remove_occupant(id);
            }
            if let Some(object) = self.objects.get_mut(&id) {
                object.set_contained_by(None);
            }
            if garrison {
                self.recalc_garrison_apparent_controller(cid);
            }
        }
        if let Some(player) = self.tunnel_network.player_holding_unit(id) {
            let _ = self
                .tunnel_network
                .record_exit(player, id, containing.unwrap_or(id));
        }
        if self.cave_system.index_holding_unit(id).is_some() {
            let _ = self.exit_cave_unit(id, containing.unwrap_or(id));
        }
    }

    /// Physical-only queue completion: no onDie, body damage, bounty or death FX.
    pub(in super::super) fn finish_direct_object_removal(&mut self, id: ObjectId) -> bool {
        self.pending_special_abilities.remove(&id);
        self.pending_special_abilities
            .retain(|_, ability| ability.target_id() != id);
        #[cfg(feature = "game_client")]
        if let Some(draw) = self
            .objects
            .get(&id)
            .and_then(|o| o.jet_ai.lockon_drawable_id)
        {
            gamelogic::helpers::TheGameClient.destroy_drawable(draw);
        }
        self.finish_building_sound(id);
        if let Some(object) = self.objects.get(&id) {
            if !object.name.is_empty() {
                self.named_unit_history.insert(object.name.clone());
            }
        }
        self.detach_host_fog_ghost(id);
        let Some(object) = self.objects.remove(&id) else {
            return false;
        };
        self.remove_host_dock_approach_queue(id);
        self.remove_host_radar_provider(id);
        self.host_radar_remove_object(id);
        crate::game_logic::host_destroy_log::record(id);

        self.clear_removed_object_references(id);
        object.is_kind_of(KindOf::Structure)
    }

    pub(in super::super) fn clear_removed_object_references(&mut self, id: ObjectId) {
        self.admit_local_drawable_deselected(id);
        for player in self.players.values_mut() {
            player.selected_objects.retain(|&selected| selected != id);
        }
        let targets: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(other, object)| (object.target == Some(id)).then_some(*other))
            .collect();
        for other in targets {
            self.stop_attack_decision_aware(other);
        }
        for (_other, object) in &mut self.objects {
            if object.guard_target == Some(id) {
                object.guard_target = None;
                if object.ai_state == AIState::GuardingObject {
                    object.set_ai_state(AIState::Idle);
                    {}
                }
            }
        }
    }
}
