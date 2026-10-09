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
        let _ = crate::gameworld_shadow::eager_mark_host_destroy_if_coupled(id);
    }

    /// Complete the supported onDelete slice on the installed Object.
    /// This also applies when onDie began earlier; deferred death effects
    /// keep their existing queue completion and are not replayed here.
    pub(in super::super) fn apply_object_delete_callbacks(&mut self, id: ObjectId) {
        self.detach_destroyed_object_from_contain(id);
        self.delete_direct_containment(id);
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
        let Some(object) = self.objects.get(&id) else {
            return;
        };
        if object.is_cave_style_container() {
            return;
        }
        if object.is_tunnel_network_style_container() {
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
        let Some(object) = self.objects.remove(&id) else {
            return false;
        };
        self.remove_host_dock_approach_queue(id);
        self.host_radar_remove_object(id);
        crate::game_logic::host_destroy_log::record(id);
        let _ = crate::gameworld_shadow::eager_unmap_host_destroy_if_coupled(id);
        self.clear_removed_object_references(id);
        object.is_kind_of(KindOf::Structure)
    }

    pub(in super::super) fn clear_removed_object_references(&mut self, id: ObjectId) {
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
        let mut idle = Vec::new();
        for (other, object) in &mut self.objects {
            if object.guard_target == Some(id) {
                object.guard_target = None;
                if object.ai_state == AIState::GuardingObject {
                    object.set_ai_state(AIState::Idle);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        idle.push(*other);
                    }
                }
            }
        }
        for other in idle {
            crate::game_logic::host_ai_decision_log::record_set_state(other, 0);
        }
    }
}
