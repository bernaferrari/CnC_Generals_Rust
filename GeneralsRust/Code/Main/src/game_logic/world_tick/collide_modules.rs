//! Live-host CollideModule::onCollide residual (C++ Object::onCollide).

use crate::game_logic::{AIState, ObjectId};

impl super::super::GameLogic {
    /// C++ `Object::onCollide` — walk contain-enter, crate pickup, and
    /// `FireWeaponCollide::onCollide` (`FireWeaponCollide.cpp:52-67`).
    pub(in super::super) fn dispatch_host_collide_modules(
        &mut self,
        self_id: ObjectId,
        other_id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        self.dispatch_host_collide_modules_with_effects(self_id, other_id, &mut Default::default())
    }

    pub(in super::super) fn dispatch_host_collide_modules_with_effects(
        &mut self,
        self_id: ObjectId,
        other_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if self_id == other_id {
            return Ok(());
        }
        let (entering_other, crate_other) = {
            let Some(us) = self.objects.get(&self_id) else {
                return Ok(());
            };
            let Some(other) = self.objects.get(&other_id) else {
                return Ok(());
            };
            let entering = matches!(us.ai_state(), AIState::Entering)
                && (us.target == Some(other_id) || us.container_id() == Some(other_id))
                && other.can_contain();
            let crate_name = other.template_name.to_ascii_lowercase();
            let is_crate = crate_name.contains("crate");
            (entering, is_crate)
        };
        if entering_other {
            let tunnel_key = self.objects.get(&other_id).and_then(|container| {
                container
                    .is_tunnel_network_style_container()
                    .then(|| container.tunnel_system_key())
            });
            let cave_index = if tunnel_key.is_none() {
                self.objects.get(&other_id).and_then(|container| {
                    container
                        .is_cave_style_container()
                        .then_some(container.cave_index)
                })
            } else {
                None
            };
            let is_garrison = tunnel_key.is_none()
                && cave_index.is_none()
                && self
                    .objects
                    .get(&other_id)
                    .is_some_and(|c| c.is_garrison_contain());
            let is_transport = self.objects.get(&other_id).is_some_and(|container| {
                container.normal_enter_uses_transport_slots()
                    && matches!(
                        container.thing().template.contain_module.kind,
                        crate::game_logic::ContainModuleKind::Transport
                            | crate::game_logic::ContainModuleKind::RailedTransport
                    )
            });
            if is_garrison || is_transport {
                // OpenContain::onCollide orders opposing riders out before its
                // final isValidContainerFor check.
                self.kick_other_controller_occupants_for_enter_with_effects(
                    other_id, self_id, effects,
                );
            }
            let eligible = if let Some(key) = tunnel_key {
                self.objects
                    .get(&self_id)
                    .is_some_and(|rider| !rider.is_kind_of(crate::game_logic::KindOf::Aircraft))
                    && self.tunnel_network.has_capacity(key)
            } else if let Some(index) = cave_index {
                self.objects
                    .get(&self_id)
                    .is_some_and(|rider| !rider.is_kind_of(crate::game_logic::KindOf::Aircraft))
                    && self.cave_system.has_capacity(index)
            } else if is_garrison {
                self.objects
                    .get(&self_id)
                    .zip(self.objects.get(&other_id))
                    .is_some_and(|(rider, container)| {
                        let relationship = self.normal_enter_relationship(rider, container);
                        container.garrison_container_accepts_entry()
                            && !rider.is_kind_of(crate::game_logic::KindOf::NoGarrison)
                            && container
                                .thing()
                                .template
                                .contain_module
                                .leftover_kind_masks_admit(rider.kind_of_cpp_mask())
                            && container.allows_normal_enter_for_relationship(relationship)
                            && container.has_capacity_for(1)
                    })
            } else if is_transport {
                self.objects
                    .get(&self_id)
                    .zip(self.objects.get(&other_id))
                    .is_some_and(|(rider, container)| {
                        // TransportContain validates the first passenger inside
                        // a special zero-slot wrapper, but boards the wrapper.
                        let checked_id = if rider.thing().template.contain_module.kind
                            == crate::game_logic::ContainModuleKind::Parachute
                        {
                            rider.contained_units().first().copied().unwrap_or(self_id)
                        } else {
                            self_id
                        };
                        let Some(checked_rider) = self.objects.get(&checked_id) else {
                            return false;
                        };
                        let Some(slots) = self.effective_transport_slot_count(checked_id) else {
                            return false;
                        };
                        slots > 0
                            && container
                                .thing()
                                .template
                                .contain_module
                                .leftover_kind_masks_admit(checked_rider.kind_of_cpp_mask())
                            && container.allows_normal_enter_for_relationship(
                                self.normal_enter_relationship(checked_rider, container),
                            )
                            && self.normal_enter_controller_matches(checked_rider, container)
                            && self
                                .normal_enter_available_capacity_for(self_id, other_id)
                                .is_some_and(|available| available >= slots)
                    })
            } else {
                true
            };
            let cancelled = eligible
                && (is_garrison || is_transport || tunnel_key.is_some() || cave_index.is_some())
                && self.should_cancel_containment_after_booby_trap_with_effects(
                    other_id, self_id, effects,
                );
            let uncontained = self
                .objects
                .get(&self_id)
                .is_some_and(|rider| rider.contained_by.is_none());
            if eligible && !cancelled && uncontained && is_garrison {
                self.validate_garrison_owner_team_transfer(other_id, self_id)?;
            }
            let mut boarded = eligible
                && !cancelled
                && uncontained
                && self.objects.get_mut(&other_id).is_some_and(|c| {
                    if tunnel_key.is_some() || cave_index.is_some() {
                        c.add_shared_tracker_occupant(self_id)
                    } else {
                        c.add_occupant(self_id)
                    }
                });
            if boarded && let Some(key) = tunnel_key {
                if self.tunnel_network.record_enter(key, self_id, other_id) {
                    self.stamp_contained_by_frame(self_id, self.frame);
                } else {
                    if let Some(container) = self.objects.get_mut(&other_id) {
                        container.remove_occupant_with_partition_owner(
                            self_id,
                            &mut self.ownership_events,
                            &mut self.partition_manager,
                        );
                    }
                    boarded = false;
                }
            }
            let mut cave_capture = None;
            if boarded && let Some(index) = cave_index {
                let team = self.objects.get(&self_id).map(|rider| rider.team);
                let result = team.map(|team| {
                    self.cave_system
                        .record_enter(index, self_id, other_id, team)
                });
                if let Some((true, event)) = result {
                    cave_capture = Some((index, event));
                    self.stamp_contained_by_frame(self_id, self.frame);
                } else {
                    if let Some(container) = self.objects.get_mut(&other_id) {
                        container.remove_occupant_with_partition_owner(
                            self_id,
                            &mut self.ownership_events,
                            &mut self.partition_manager,
                        );
                    }
                    boarded = false;
                }
            }
            if boarded {
                // OpenContain::addToContain records the controlling player
                // immediately after adding the rider to its roster.
                self.stamp_player_who_entered(other_id, self_id);
                if let Some((index, event)) = cave_capture {
                    self.apply_cave_capture_event_with_effects(index, event, other_id, effects)?;
                }
                let state = self
                    .objects
                    .get(&other_id)
                    .map_or(AIState::Docked, |container| {
                        if container.is_kind_of(crate::game_logic::KindOf::Structure)
                            && !container
                                .thing()
                                .template
                                .contain_module
                                .kind
                                .is_heal_contain()
                            && !container.is_tunnel_network_style_container()
                            && !container.is_cave_style_container()
                        {
                            AIState::Garrisoned
                        } else {
                            AIState::Docked
                        }
                    });
                // OpenContain removes a rider from its group and the world only
                // when this particular passenger is enclosed by the container.
                let enclosing = self
                    .objects
                    .get(&other_id)
                    .zip(self.objects.get(&self_id))
                    .is_some_and(|(container, rider)| container.is_enclosing_container_for(rider));
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    self_id,
                    Some(other_id),
                    enclosing,
                );
                if let Some(us) = self.objects.get_mut(&self_id) {
                    us.set_ai_state(
                        state,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                    us.status.moving = false;
                    if tunnel_key.is_some() || cave_index.is_some() {
                        us.set_status_disabled_held(true);
                    }
                }
                self.admit_object_drawable_deselection(self_id);
                if is_garrison {
                    self.record_garrison_residual_enter();
                    self.apply_garrison_contain_on_enter_with_effects(other_id, self_id, effects)?;
                    if let Some(owner) = self
                        .objects
                        .get(&self_id)
                        .and_then(|rider| rider.owner_player_id)
                    {
                        if let Some(player) = self.players.get_mut(&owner) {
                            player.selected_objects.retain(|id| *id != self_id);
                        }
                    }
                    self.admit_local_drawable_deselected(self_id);
                    self.play_container_enter_sound(other_id, effects);
                } else if tunnel_key.is_some() || cave_index.is_some() {
                    self.play_container_enter_sound(other_id, effects);
                }
            }
        }
        if crate_other {
            self.update_money_crate_collides_with_effects(effects);
        }
        self.fire_host_weapon_collide_with_effects(self_id, other_id, effects);
        Ok(())
    }

    /// C++ `FireWeaponCollide::onCollide` — skip ground (other is always an
    /// object here), then `loadAmmoNow` + `fireWeapon` every frame using the
    /// INI `CollideWeapon` template (damage type / amount), not invented Flame.
    fn fire_host_weapon_collide(&mut self, self_id: ObjectId, other_id: ObjectId) {
        self.fire_host_weapon_collide_with_effects(self_id, other_id, &mut Default::default())
    }

    fn fire_host_weapon_collide_with_effects(
        &mut self,
        self_id: ObjectId,
        other_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use super::collide_dispatch::{
            host_fire_weapon_collide_damage, host_fire_weapon_collide_spec,
            host_should_fire_weapon_collide,
        };
        let Some(us) = self.objects.get(&self_id) else {
            return;
        };
        if !us.is_alive() {
            return;
        }
        let Some(spec) = host_fire_weapon_collide_spec(us) else {
            return;
        };
        // C++ never sets m_everFired — match, not a gap.
        if !host_should_fire_weapon_collide(us.object_status_mask_for_queries(), &spec, false) {
            return;
        }
        let damage = host_fire_weapon_collide_damage(&spec.weapon_name);
        if damage <= 0.0 {
            return;
        }
        let damage_type = crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
            &spec.weapon_name,
        );
        let death_type = crate::game_logic::host_armor_residual::resolve_host_death_type(
            Some(&spec.weapon_name),
            damage_type,
        );
        if let Some(other) = self.objects.get_mut(&other_id) {
            if !other.is_alive() {
                return;
            }
            let _ = {
                let owned_victim_id = other.id;
                let owned_damage_type = damage_type;
                self.apply_residual_damage_owned_with_effects(
                    owned_victim_id,
                    damage,
                    Some(self_id),
                    owned_damage_type,
                    death_type,
                    None,
                    effects,
                )
            };
        }
    }
}
