//! Host objects `impl GameLogic` — `support_states`.
//! update_support_states. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

#[derive(Clone)]
struct RiderChangeEnterPlan {
    rider: crate::game_logic::RiderChangeRiderMetadata,
    /// Resolved before the first eject mutation.  A missing/changed host
    /// locomotor cannot therefore strand both riders after an old rider was
    /// removed from the bike.
    active_locomotor_name: String,
    /// The complete, surface-safe SET_NORMAL projection.  Do not reduce this
    /// to the old three-field `Movement` bridge: C++ chooseLocomotorSet also
    /// changes braking, damage movement, surface capability, and physics
    /// options.
    active_locomotor: crate::game_logic::locomotor_bootstrap::HostUniformLocomotorSetBinding,
    previous_rider: Option<ObjectId>,
    container_position: glam::Vec3,
    /// C++ `wasSelected` is an in-game/local selection fact.  Keep the
    /// owning local player so the object bit and the player's selection list
    /// move together after a successful board.
    incoming_selection_player: Option<u32>,
    incoming_was_selected: bool,
    incoming_veterancy: VeterancyLevel,
}

/// Clear exactly the RiderChange state currently represented by the active
/// host.  C++ additionally clears `DOOR_1_CLOSING` while removing a rider;
/// keep that flag out of the next rider's model state even if an interrupted
/// generic transport animation left it behind.
fn clear_rider_change_runtime_state(
    container: &mut Object,
    authored_rider: Option<&crate::game_logic::RiderChangeRiderMetadata>,
) {
    let authored_model_mask = authored_rider
        .map(|rider| rider.model_condition_mask)
        .unwrap_or(0);
    let authored_status_mask = authored_rider
        .map(|rider| rider.object_status_mask)
        .unwrap_or(0);
    let door_closing_mask =
        crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
            "DOOR_1_CLOSING",
        )
        .filter(|bit| *bit < 128)
        .map(|bit| 1u128 << bit)
        .unwrap_or(0);

    container.model_condition_bits &=
        !(container.rider_change_model_condition_mask | authored_model_mask | door_closing_mask);
    container.object_status_bits &=
        !(container.rider_change_object_status_mask | authored_status_mask);
    container.rider_change_active_slot = None;
    container.rider_change_model_condition_mask = 0;
    container.rider_change_object_status_mask = 0;
    container.rider_change_weapon_set = None;
    container.rider_change_locomotor_set = None;
    container.rider_change_locomotor_name = None;
    // C++ clears the active WeaponSet before the replacement's set is chosen.
    // The bounded Combat Cycle bridge is the live representation of that set.
    container.combat_cycle_rider = 0;
    container.weapon = None;
    container.secondary_weapon = None;
    container.record_host_weapon_stats();
    container.record_host_model_condition();
}

/// Apply every Locomotor.ini field for which the live host Object has an
/// authoritative representation.  This is deliberately one helper so an
/// atomic RiderChange replacement cannot accidentally update its nominal
/// speed while leaving the old rider's braking, damage, or surface state.
fn apply_rider_change_locomotor_binding(
    container: &mut Object,
    binding: &crate::game_logic::locomotor_bootstrap::HostLocomotorBinding,
) {
    container.movement.max_speed = binding.movement.max_speed;
    container.movement.max_speed_damaged = binding.max_speed_damaged;
    container.movement.acceleration = binding.movement.acceleration;
    container.movement.acceleration_damaged = binding.acceleration_damaged;
    container.movement.turn_rate = binding.movement.turn_rate;
    container.movement.turn_rate_damaged = binding.turn_rate_damaged;
    container.braking = binding.braking;
    container.min_speed = binding.min_speed;
    container.min_turn_speed = binding.min_turn_speed;
    container.loco_behavior_z = binding.behavior_z;
    container.loco_appearance = binding.appearance;
    container.loco_extra_2d_friction = binding.extra_2d_friction;
    container.loco_apply_2d_friction_airborne = binding.apply_2d_friction_when_airborne;
    container.can_move_backward = binding.can_move_backward;
    container.downhill_only = binding.downhill_only;
    container.max_lift = binding.max_lift;
    container.max_lift_damaged = binding.max_lift_damaged;
    container.speed_limit_z = binding.speed_limit_z;
    container.loco_preferred_height = binding.preferred_height;
    container.loco_preferred_height_damping = binding.preferred_height_damping;
    container.circling_radius = binding.circling_radius;
    container.turn_pivot_offset = binding.turn_pivot_offset;
    container.stick_to_ground = binding.stick_to_ground;
    container.locomotor_surfaces = binding.locomotor_surfaces;
    container.set_locomotor_physics_options();
    container.record_host_locomotor();
    container.record_host_movement();
}

impl GameLogic {
    /// End an interrupted C++ capture SpecialAbilityUpdate.  An order may be
    /// cancelled while approaching, unpacking, or preparing; in all cases the
    /// source must not retain a stale channel or `IS_USING_ABILITY` bit.
    fn abort_capture_channel(&mut self, object_id: ObjectId) {
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.stop_moving();
            object.capture_channel = None;
            object.set_status_using_ability(false);
            object.set_target(None);
        }
    }

    /// Complete C++ capture packing.  Ownership changes at the end of
    /// preparation, but the source remains busy until PackTime has elapsed.
    fn finish_capture_channel(&mut self, object_id: ObjectId) {
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.stop_moving();
            object.capture_channel = None;
            object.set_status_using_ability(false);
            object.set_target(None);
        }
    }

    /// Validate every mutable participant in the RiderChange replacement
    /// before taking the first item out of the old containment list.  The
    /// simulation is single-threaded, so once this returns `Some` the
    /// following transaction has no fallible lookup or capacity operation.
    fn rider_change_enter_plan(
        &self,
        rider_id: ObjectId,
        container_id: ObjectId,
    ) -> Option<RiderChangeEnterPlan> {
        if !self.can_unit_enter_normal_target(rider_id, container_id) {
            return None;
        }
        let rider = self.objects.get(&rider_id)?;
        let container = self.objects.get(&container_id)?;
        if !container.supports_authored_rider_change_normal_enter() {
            // A parsed roster is the sole admission/effect capability.  Do
            // not use a Combat Bike basename or legacy transport flag here:
            // those are old bootstrap state, not RiderChange authority.
            return None;
        }
        // RiderChangeContain is a one-rider mobile module.  The transaction
        // below writes the mobile occupant list directly after preflight; a
        // malformed structure carrying the same Behavior would otherwise
        // have a different authoritative list (`BuildingData`) and could
        // lose the rider.  Reject it before the first mutation.
        if container.building_data.is_some() {
            return None;
        }
        let rider_metadata = container
            .authored_rider_change_rider_for_template(&rider.template_name)?
            .clone();
        // Parse-time metadata can outlive a LocomotorStore reload or an old
        // snapshot.  Re-resolve the complete exact authored SET_NORMAL row
        // before any eject mutation; no generic template/default locomotor
        // (nor a single arbitrarily chosen surface member) is a valid
        // substitute for this RiderN locomotor set.
        let active_locomotor_name = rider_metadata.active_locomotor_name.clone()?;
        if !rider_metadata
            .locomotor_set
            .eq_ignore_ascii_case("SET_NORMAL")
            || rider_metadata.active_locomotor_names.is_empty()
            || rider_metadata.active_locomotor_surfaces == 0
        {
            return None;
        }
        let active_locomotor =
            crate::game_logic::locomotor_bootstrap::resolve_uniform_host_locomotor_set(
                &rider_metadata.active_locomotor_names,
            )?;
        if !active_locomotor
            .representative_name
            .eq_ignore_ascii_case(&active_locomotor_name)
            || active_locomotor.locomotor_surfaces != rider_metadata.active_locomotor_surfaces
        {
            // Do not let stale parsed metadata turn a changed/ambiguous
            // Locomotor store into a partial replacement transaction.
            return None;
        }
        let occupants = container.contained_units();
        if occupants.len() > 1 {
            // The source C++ module has one active rider.  A malformed/stale
            // host roster cannot be safely reduced by guessing which unit to
            // evict, so reject before touching either side.
            return None;
        }
        let incoming_selection_player = rider
            .owner_player_id
            .filter(|player_id| rider.selected && self.is_local_player(*player_id));
        if occupants.first().copied() == Some(rider_id) {
            // A stale duplicate Enter is harmless only when the relationship
            // is already internally consistent; the caller repairs the rider
            // state below without adding it a second time.
            return (rider.contained_by == Some(container_id)).then_some(RiderChangeEnterPlan {
                rider: rider_metadata,
                active_locomotor_name,
                active_locomotor,
                previous_rider: Some(rider_id),
                container_position: container.get_position(),
                incoming_selection_player,
                incoming_was_selected: incoming_selection_player.is_some(),
                incoming_veterancy: rider.experience.level,
            });
        }
        if rider.contained_by.is_some() {
            // `execute_enter` removes an old container before pathing.  If a
            // stale link survives that stage, do not let this transaction make
            // a unit belong to two containers.
            return None;
        }
        let previous_rider = occupants.first().copied();
        if let Some(previous_rider_id) = previous_rider {
            let previous = self.objects.get(&previous_rider_id)?;
            if previous_rider_id == rider_id
                || !previous.is_alive()
                || previous.contained_by != Some(container_id)
                || container
                    .authored_rider_change_rider_for_template(&previous.template_name)
                    .is_none()
            {
                return None;
            }
        }
        if incoming_selection_player.is_some() && !container.selected && !container.is_selectable()
        {
            // Selection transfer is a modeled C++ side effect.  Do not drop a
            // selected rider into an unselectable custom container and pretend
            // the transfer succeeded.
            return None;
        }
        Some(RiderChangeEnterPlan {
            rider: rider_metadata,
            active_locomotor_name,
            active_locomotor,
            previous_rider,
            container_position: container.get_position(),
            incoming_selection_player,
            incoming_was_selected: incoming_selection_player.is_some(),
            incoming_veterancy: rider.experience.level,
        })
    }

    /// Complete the C++ `RiderChangeContain::onContaining` replacement at the
    /// arrival boundary.  This intentionally bypasses generic
    /// `Object::add_occupant`: C++ ignores capacity, ejects the prior rider,
    /// and boards the new rider as one ordered transaction.
    pub(in super::super) fn rider_change_enter_at_arrival(
        &mut self,
        rider_id: ObjectId,
        container_id: ObjectId,
    ) -> bool {
        let Some(plan) = self.rider_change_enter_plan(rider_id, container_id) else {
            return false;
        };

        // Idempotent duplicate: retain the valid existing relation but repair
        // the arrival state without disturbing veterancy or rider effects.
        if plan.previous_rider == Some(rider_id) {
            if let Some(rider) = self.objects.get_mut(&rider_id) {
                rider.stop_moving();
                rider.set_target(Some(container_id));
                rider.set_contained_by(Some(container_id));
                rider.set_position(plan.container_position);
                rider.set_ai_state(AIState::Docked);
                rider.set_status_moving(false);
                rider.set_status_attacking(false);
            }
            return true;
        }

        // Phase 1: eject the old rider.  All IDs/list membership were checked
        // above, so the remove cannot fail after any state has changed.
        if let Some(previous_rider_id) = plan.previous_rider {
            let (previous_metadata, bike_veterancy, previous_has_controller) = {
                let container = self
                    .objects
                    .get(&container_id)
                    .expect("RiderChange preflight container disappeared");
                let previous = self
                    .objects
                    .get(&previous_rider_id)
                    .expect("RiderChange preflight rider disappeared");
                (
                    container
                        .authored_rider_change_rider_for_template(&previous.template_name)
                        .expect("RiderChange preflight roster disappeared")
                        .clone(),
                    container.experience.level,
                    previous.owner_player_id.is_some(),
                )
            };
            let removed = self
                .objects
                .get_mut(&container_id)
                .expect("RiderChange preflight container disappeared")
                .remove_occupant(previous_rider_id);
            debug_assert!(removed, "RiderChange preflight must make eject infallible");
            if !removed {
                return false;
            }
            if let Some(previous) = self.objects.get_mut(&previous_rider_id) {
                previous.stop_moving();
                // C++ delegates the exact exit-door/path placement to
                // TransportContain::aiEvacuateInstantly.  That authored
                // exit-path matrix is not represented by this bounded host;
                // keep the ejected rider at the container origin instead of
                // fabricating a pseudo-random offset.
                previous.set_position(plan.container_position);
                previous.set_contained_by(None);
                previous.set_target(None);
                previous.set_ai_state(AIState::Idle);
                previous.set_status_moving(false);
                previous.set_status_attacking(false);
                previous.deselect();
                if previous_has_controller {
                    previous.set_rider_change_veterancy_level(bike_veterancy);
                }
            }
            if let Some(container) = self.objects.get_mut(&container_id) {
                clear_rider_change_runtime_state(container, Some(&previous_metadata));
                if previous_has_controller {
                    container.set_rider_change_veterancy_level(VeterancyLevel::Rookie);
                }
            }
        } else if let Some(container) = self.objects.get_mut(&container_id) {
            // An initial-payload Combat Cycle can have a visible legacy rider
            // without a tracked Object.  Clear only its existing host bridge
            // before applying the explicitly parsed replacement.
            clear_rider_change_runtime_state(container, None);
        }

        // Phase 2: apply the exact parsed RiderN state and publish the sole
        // contained body.  There are no capacity checks or fallible template
        // lookups after the preflight.
        {
            let container = self
                .objects
                .get_mut(&container_id)
                .expect("RiderChange preflight container disappeared");
            container.model_condition_bits |= plan.rider.model_condition_mask;
            container.object_status_bits |= plan.rider.object_status_mask;
            container.rider_change_active_slot = Some(plan.rider.slot);
            container.rider_change_model_condition_mask = plan.rider.model_condition_mask;
            container.rider_change_object_status_mask = plan.rider.object_status_mask;
            container.rider_change_weapon_set = Some(plan.rider.weapon_set.clone());
            container.rider_change_locomotor_set = Some(plan.rider.locomotor_set.clone());
            container.rider_change_locomotor_name = Some(plan.active_locomotor_name.clone());
            container.set_command_set_override(Some(plan.rider.command_set.clone()));
            if container.status.stealthed {
                container.set_status_detected(true);
            }
            container.occupants.push(rider_id);
            container.record_host_model_condition();
            apply_rider_change_locomotor_binding(container, &plan.active_locomotor.binding);
        }
        {
            let rider = self
                .objects
                .get_mut(&rider_id)
                .expect("RiderChange preflight incoming rider disappeared");
            rider.stop_moving();
            rider.set_status_attacking(false);
            rider.target_location = None;
            rider.set_status_force_attack(false);
            rider.set_target(Some(container_id));
            rider.set_contained_by(Some(container_id));
            rider.set_position(plan.container_position);
            rider.set_ai_state(AIState::Docked);
            rider.set_status_moving(false);
            rider.set_rider_change_veterancy_level(VeterancyLevel::Rookie);
            rider.deselect();
        }

        // This is a visual/weapon mirror only.  It receives the parsed RiderN
        // ordinal, never the incoming rider's template spelling; the generic
        // `refresh_combat_cycle_rider_weapon` is intentionally not called.
        let switched = self.apply_combat_cycle_rider(
            container_id,
            crate::game_logic::host_combat_cycle::CombatCycleRider::from_u8(plan.rider.slot),
        );
        debug_assert!(
            switched,
            "RiderChange preflight must make weapon bridge infallible"
        );
        if let Some(container) = self.objects.get_mut(&container_id) {
            container.set_rider_change_veterancy_level(plan.incoming_veterancy);
            if plan.incoming_was_selected && !container.selected {
                container.select();
            }
        }
        if let Some(player_id) = plan.incoming_selection_player {
            // C++ sends MSG_CREATE_SELECTED_GROUP(false, bike) before
            // TransportContain automatically deselects the rider.  Mirror
            // both the per-object bits above and the authoritative local
            // selection roster, preserving every unrelated selected unit.
            if let Some(player) = self.players.get_mut(&player_id) {
                player.selected_objects.retain(|id| *id != rider_id);
                if !player.selected_objects.contains(&container_id) {
                    player.selected_objects.push(container_id);
                }
            }
        }
        self.record_combat_cycle_residual_load();
        true
    }

    /// C++ `RiderChangeContain::onRemoving` for an ordinary player Exit (or
    /// moving a rider to a different container).  This is deliberately
    /// separate from replacement above: only ordinary removal starts the
    /// delayed scuttle and transfers selected state back to the rider.
    pub(in super::super) fn rider_change_remove_occupant(
        &mut self,
        container_id: ObjectId,
        rider_id: ObjectId,
    ) -> bool {
        let Some((
            rider_metadata,
            position,
            bike_veterancy,
            rider_has_controller,
            was_moving,
            transfer_selection,
        )) = self.objects.get(&container_id).and_then(|container| {
            if !container.supports_authored_rider_change_normal_enter()
                || container.contained_units() != vec![rider_id]
            {
                return None;
            }
            let rider = self.objects.get(&rider_id)?;
            if rider.contained_by != Some(container_id) || !rider.is_alive() {
                return None;
            }
            let rider_metadata = container
                .authored_rider_change_rider_for_template(&rider.template_name)?
                .clone();
            let transfer_selection = container.selected
                && container
                    .owner_player_id
                    .is_some_and(|player_id| self.is_local_player(player_id));
            Some((
                rider_metadata,
                container.get_position(),
                container.experience.level,
                rider.owner_player_id.is_some(),
                container.status.moving,
                transfer_selection,
            ))
        })
        else {
            return false;
        };

        let removed = self
            .objects
            .get_mut(&container_id)
            .expect("RiderChange preflight container disappeared")
            .remove_occupant(rider_id);
        debug_assert!(removed, "RiderChange preflight must make exit infallible");
        if !removed {
            return false;
        }

        if let Some(rider) = self.objects.get_mut(&rider_id) {
            rider.stop_moving();
            rider.set_position(position);
            rider.set_contained_by(None);
            rider.set_target(None);
            rider.set_ai_state(AIState::Idle);
            rider.set_status_moving(false);
            rider.set_status_attacking(false);
            if rider_has_controller {
                rider.set_rider_change_veterancy_level(bike_veterancy);
            }
        }
        if let Some(container) = self.objects.get_mut(&container_id) {
            clear_rider_change_runtime_state(container, Some(&rider_metadata));
            if rider_has_controller {
                container.set_rider_change_veterancy_level(VeterancyLevel::Rookie);
            }
            container.rider_change_scuttled_on_frame = self.frame.max(1);
            container.set_status_unselectable(true);
            container.model_condition_bits |= container
                .thing
                .template
                .contain_module
                .rider_change_scuttle_status_mask;
            if !was_moving {
                let _ = container.apply_status_bits_upgrade_masks(&["IMMOBILE"], &[]);
            }
            container.record_host_model_condition();
            if transfer_selection {
                container.deselect();
            }
        }
        if transfer_selection {
            if let Some(rider) = self.objects.get_mut(&rider_id) {
                rider.select();
            }
            if let Some(player_id) = self
                .objects
                .get(&container_id)
                .and_then(|container| container.owner_player_id)
            {
                if let Some(player) = self.players.get_mut(&player_id) {
                    player.selected_objects.retain(|id| *id != container_id);
                    if !player.selected_objects.contains(&rider_id) {
                        player.selected_objects.push(rider_id);
                    }
                }
            }
        }
        true
    }

    /// C++ `GarrisonContain::removeAllContained(TRUE)` on a capture trigger.
    /// It exposes and ejects occupants; it does not kill them and it does not
    /// defect the garrisonable structure in that same ability use.
    fn evacuate_garrison_for_capture(&mut self, structure_id: ObjectId) {
        let Some((position, occupants)) = self
            .objects
            .get(&structure_id)
            .map(|structure| (structure.get_position(), structure.contained_units()))
        else {
            return;
        };

        if let Some(structure) = self.objects.get_mut(&structure_id) {
            for occupant_id in &occupants {
                let _ = structure.remove_occupant(*occupant_id);
            }
        }

        for occupant_id in occupants {
            let expose_stealth = self
                .objects
                .get(&occupant_id)
                .is_some_and(|occupant| occupant.status.stealthed);
            let _ = self.unit_command_exit_drop(occupant_id, position);
            if expose_stealth {
                if let Some(occupant) = self.objects.get_mut(&occupant_id) {
                    occupant.set_status_detected(true);
                }
            }
        }
    }

    /// C++ `SpecialAbilityUpdate::startPreparation` for a capture power.
    /// Crucially, the real SpecialPower timer starts here—not when the player
    /// clicks a distant target. `consume_special_power_charge_for` supplies
    /// the existing per-ability timer and paused/disabled readiness gates.
    fn start_capture_preparation(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        power: crate::game_logic::CapturePowerKind,
        preparation_time_ms: u32,
    ) -> bool {
        let Some(power_type) = power.special_power_type() else {
            return false;
        };
        let Some(source_team) = self.objects.get(&object_id).map(|object| object.team) else {
            return false;
        };

        // C++ checks/detonates a hostile trap when preparation begins, before
        // it marks the SpecialPower as triggered or begins its progress bar.
        let trap_position = self
            .objects
            .get(&target_id)
            .map(|target| target.get_position());
        let planter_is_ally = self
            .booby_trap
            .plant(target_id)
            .map(|plant| plant.planter_team == source_team)
            .unwrap_or(false);
        if let Some(trap_position) = trap_position {
            let target_is_trapped = self.booby_trap.is_booby_trapped(target_id)
                || self
                    .objects
                    .get(&target_id)
                    .map(|target| target.status.booby_trapped)
                    .unwrap_or(false);
            if !planter_is_ally && target_is_trapped {
                let _ = self.detonate_booby_trap_at(
                    target_id,
                    trap_position,
                    Some(object_id),
                    true,
                    false,
                );
            }
        }

        if !self.can_unit_capture_building(object_id, target_id, false)
            || !self.consume_special_power_charge_for(object_id, &power_type)
        {
            return false;
        }

        // C++ starts victim warning/infiltration exactly as the preparation
        // timer begins, giving the defender time to react before ownership
        // changes at trigger time.
        self.try_eva_building_being_stolen(target_id);
        self.try_infiltration_event(target_id);

        let Some(object) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !object.is_alive() {
            return false;
        }
        // `consume_special_power_charge` preserves an older generic Idle
        // side-effect; restore the capture machine after the authoritative
        // timer was accepted.
        object.set_ai_state(AIState::Capturing);
        object.capture_channel = Some(crate::game_logic::CaptureChannelState::new(
            crate::game_logic::CaptureChannelPhase::Preparing,
            preparation_time_ms,
        ));
        object.set_status_using_ability(true);
        true
    }

    /// Clear a completed Hacker Disable Building channel.  Unlike a generic
    /// `PendingSpecialAbility`, HDB has an authored PackTime, so this is only
    /// called after the packing timer has completed (or when the source itself
    /// is no longer able to pack).  Keep the order, channel and visible
    /// `IS_USING_ABILITY` state in one place so a later command cannot inherit
    /// an old physical channel.
    fn finish_hacker_disable_building_channel(&mut self, object_id: ObjectId) {
        self.pending_special_abilities.remove(&object_id);
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.stop_moving();
            object.hacker_disable_channel = None;
            object.set_status_using_ability(false);
            object.set_target(None);
            object.set_ai_state(AIState::Idle);
        }
    }

    /// Begin the parsed HDB `PackTime` after an interrupted or completed
    /// channel.  C++ packs on target death, alliance/relation loss, range
    /// abort, and after a non-persistent trigger; it does not instantly turn
    /// the hacker idle in any of those cases.
    fn begin_hacker_disable_building_packing(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        pack_time_ms: u32,
    ) {
        let mut finish_now = false;
        if let Some(object) = self.objects.get_mut(&object_id) {
            if !object.is_alive() {
                finish_now = true;
            } else {
                object.stop_moving();
                object.hacker_disable_channel =
                    Some(crate::game_logic::HackerDisableChannelState::new(
                        target_id,
                        crate::game_logic::HackerDisableChannelPhase::Packing,
                        pack_time_ms,
                    ));
                object.set_status_using_ability(false);
                object.set_ai_state(AIState::SpecialAbility);
            }
        } else {
            finish_now = true;
        }
        if finish_now || pack_time_ms == 0 {
            self.finish_hacker_disable_building_channel(object_id);
        }
    }

    /// Resolve the live ownership relationship for a channel already in
    /// flight.  Exact player ownership is authoritative; the old team path is
    /// retained only for wholly ownerless synthetic worlds so same-faction
    /// different-player objects never become accidental HDB targets.
    fn hacker_disable_building_channel_has_enemy_relation(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        use gamelogic::common::Relationship;

        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let relation = match (source.owner_player_id, target.owner_player_id) {
            (Some(source_owner), Some(target_owner))
                if self.player_owner_for_host_object(source) == Some(source_owner)
                    && self.player_owner_for_host_object(target) == Some(target_owner) =>
            {
                self.player_relationship(source_owner, target_owner)
            }
            (None, None) if self.uses_legacy_team_ownership_fallback() => {
                if source.team == target.team {
                    Relationship::Allies
                } else if source.team == Team::Neutral || target.team == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            }
            _ => Relationship::Neutral,
        };
        relation == Relationship::Enemies
    }

    /// C++ `SpecialAbilityUpdate::isWithinStartAbilityRange` uses a 2D
    /// bounding-sphere envelope and shaves one quarter of a pathfinding cell
    /// from the approach threshold.  It is deliberately not the old fixed
    /// 150-unit HDB residual.
    fn hacker_disable_building_within_start_range(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        metadata: &crate::game_logic::HackerDisableBuildingMetadata,
    ) -> bool {
        const PATHFIND_CELL_SIZE_F: f32 = 10.0;

        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let source_position = source.get_position();
        let target_position = target.get_position();
        let dx = source_position.x - target_position.x;
        let dz = source_position.z - target_position.z;
        let edge_distance = ((dx * dx + dz * dz).sqrt()
            - source.selection_radius.max(0.0)
            - target.selection_radius.max(0.0))
        .max(0.0);
        let start_range = (metadata.start_ability_range - PATHFIND_CELL_SIZE_F * 0.25).max(0.0);
        if edge_distance > start_range {
            return false;
        }
        if !metadata.approach_requires_los {
            return true;
        }
        let source_eye = glam::Vec3::new(
            source_position.x,
            source_position.y + source.selection_radius.max(5.0) * 0.5,
            source_position.z,
        );
        let target_eye = glam::Vec3::new(
            target_position.x,
            target_position.y + target.selection_radius.max(5.0) * 0.5,
            target_position.z,
        );
        self.is_clear_line_of_sight_terrain(source_eye, target_eye)
    }

    /// C++ `SpecialAbilityUpdate::isWithinAbilityAbortRange` has no start
    /// range undersize.  The source module default is effectively infinite,
    /// so only a finite authored abort range can interrupt preparation.
    fn hacker_disable_building_within_abort_range(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        metadata: &crate::game_logic::HackerDisableBuildingMetadata,
    ) -> bool {
        const DEFAULT_ABILITY_RANGE: f32 = 10_000_000.0;

        if metadata.ability_abort_range >= DEFAULT_ABILITY_RANGE {
            return true;
        }
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let source_position = source.get_position();
        let target_position = target.get_position();
        let dx = source_position.x - target_position.x;
        let dz = source_position.z - target_position.z;
        let edge_distance = ((dx * dx + dz * dz).sqrt()
            - source.selection_radius.max(0.0)
            - target.selection_radius.max(0.0))
        .max(0.0);
        edge_distance <= metadata.ability_abort_range
    }

    /// Enter HDB preparation and begin the exact parsed SpecialPower reload.
    /// The executor freezes click-time readiness, but this repeats the C++
    /// start-preparation authority after physical approach/unpack so a changed
    /// target or consumed shared timer cannot produce a false success.
    fn start_hacker_disable_building_preparation(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        metadata: &crate::game_logic::HackerDisableBuildingMetadata,
    ) -> bool {
        if !self.can_unit_hacker_disable_building(object_id, target_id, false)
            || !self.consume_hacker_disable_building_charge(object_id)
        {
            return false;
        }
        let Some(object) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !object.is_alive() {
            return false;
        }
        object.stop_moving();
        object.hacker_disable_channel = Some(crate::game_logic::HackerDisableChannelState::new(
            target_id,
            crate::game_logic::HackerDisableChannelPhase::Preparing,
            metadata.preparation_time_ms,
        ));
        object.set_status_using_ability(true);
        object.set_ai_state(AIState::SpecialAbility);
        true
    }

    /// Trigger the HDB effect at the authored preparation boundary.  A target
    /// that is already DISABLED_HACKED remains legal; C++ refreshes its
    /// `EffectDuration` and continues the persistent channel.
    fn trigger_hacker_disable_building(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        metadata: &crate::game_logic::HackerDisableBuildingMetadata,
    ) {
        let duration_frames =
            ((metadata.effect_duration_ms as u64 * 30 + 999) / 1_000).min(u32::MAX as u64) as u32;
        let until = self.frame.saturating_add(duration_frames);
        if let Some(target) = self.objects.get_mut(&target_id) {
            target.apply_disabled_hacked(until);
        } else {
            self.begin_hacker_disable_building_packing(object_id, target_id, metadata.pack_time_ms);
            return;
        }
        self.hacker_disable_building_count = self.hacker_disable_building_count.saturating_add(1);

        // HDB is an intrinsic persistent SpecialAbilityUpdate. An omitted
        // PersistentPrepTime is the C++ zero default, not a signal to turn it
        // into a one-shot ability; retain it as a zero-duration preparation
        // which may trigger on the following logic update without an in-tick
        // infinite loop.
        // `PersistenceRequiresRecharge` is the only persistent path that
        // starts/gates another reload. Ordinary HDB uses its authored
        // PersistentPrepTime continuously while the target remains legal.
        if metadata.persistence_requires_recharge
            && !self.consume_hacker_disable_building_charge(object_id)
        {
            self.begin_hacker_disable_building_packing(object_id, target_id, metadata.pack_time_ms);
            return;
        }
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.hacker_disable_channel =
                Some(crate::game_logic::HackerDisableChannelState::new(
                    target_id,
                    crate::game_logic::HackerDisableChannelPhase::Preparing,
                    metadata.persistent_prep_time_ms,
                ));
            object.set_status_using_ability(true);
            object.set_ai_state(AIState::SpecialAbility);
        }
    }

    /// Dedicated C++ `SpecialAbilityUpdate` HDB state machine.  This lives
    /// ahead of generic PendingSpecialAbility handling so the old fixed range,
    /// instant effect, and "already hacked" rejection cannot accidentally
    /// re-enter the player-facing command path.
    fn update_hacker_disable_building_channel(
        &mut self,
        object_id: ObjectId,
        pending_target_id: ObjectId,
        dt: f32,
    ) {
        const COMPLETE_EPSILON: f32 = 0.000_1;

        let Some((metadata, channel)) = self.objects.get(&object_id).and_then(|object| {
            object
                .thing
                .template
                .hacker_disable_building
                .clone()
                .map(|metadata| (metadata, object.hacker_disable_channel))
        }) else {
            self.finish_hacker_disable_building_channel(object_id);
            return;
        };
        let Some(channel) = channel else {
            self.finish_hacker_disable_building_channel(object_id);
            return;
        };
        if channel.target_id != pending_target_id {
            self.begin_hacker_disable_building_packing(
                object_id,
                channel.target_id,
                metadata.pack_time_ms,
            );
            return;
        }

        // Packing remains meaningful after its target dies, defects, or
        // leaves visibility.  It is the only phase that intentionally does
        // not ask the live target authority again.
        if channel.phase == crate::game_logic::HackerDisableChannelPhase::Packing {
            let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
            if remaining > COMPLETE_EPSILON {
                if let Some(object) = self.objects.get_mut(&object_id) {
                    object.hacker_disable_channel =
                        Some(crate::game_logic::HackerDisableChannelState {
                            target_id: channel.target_id,
                            phase: crate::game_logic::HackerDisableChannelPhase::Packing,
                            remaining_seconds: remaining,
                        });
                }
            } else {
                self.finish_hacker_disable_building_channel(object_id);
            }
            return;
        }

        let source_valid = self.objects.get(&object_id).is_some_and(|source| {
            source.is_alive()
                && !source.is_disabled()
                && metadata.update_module_starts_attack
                && !metadata.scripted_special_power_only
        });
        if !source_valid {
            self.finish_hacker_disable_building_channel(object_id);
            return;
        }
        let target_alive = self
            .objects
            .get(&channel.target_id)
            .is_some_and(|target| target.is_alive());
        if !target_alive
            || !self
                .hacker_disable_building_channel_has_enemy_relation(object_id, channel.target_id)
        {
            self.begin_hacker_disable_building_packing(
                object_id,
                channel.target_id,
                metadata.pack_time_ms,
            );
            return;
        }

        match channel.phase {
            crate::game_logic::HackerDisableChannelPhase::Approaching => {
                // Revalidate typed click authority after physical movement but
                // never re-demand the cooldown that is intentionally spent
                // only once preparation begins.
                if !self.can_unit_hacker_disable_building(object_id, channel.target_id, false) {
                    self.begin_hacker_disable_building_packing(
                        object_id,
                        channel.target_id,
                        metadata.pack_time_ms,
                    );
                    return;
                }
                if self.hacker_disable_building_within_start_range(
                    object_id,
                    channel.target_id,
                    &metadata,
                ) {
                    if metadata.unpack_time_ms == 0 {
                        if self.start_hacker_disable_building_preparation(
                            object_id,
                            channel.target_id,
                            &metadata,
                        ) && metadata.preparation_time_ms == 0
                        {
                            self.trigger_hacker_disable_building(
                                object_id,
                                channel.target_id,
                                &metadata,
                            );
                        } else if self
                            .objects
                            .get(&object_id)
                            .is_some_and(|object| object.hacker_disable_channel.is_none())
                        {
                            self.begin_hacker_disable_building_packing(
                                object_id,
                                channel.target_id,
                                metadata.pack_time_ms,
                            );
                        }
                    } else if let Some(object) = self.objects.get_mut(&object_id) {
                        object.stop_moving();
                        object.hacker_disable_channel =
                            Some(crate::game_logic::HackerDisableChannelState::new(
                                channel.target_id,
                                crate::game_logic::HackerDisableChannelPhase::Unpacking,
                                metadata.unpack_time_ms,
                            ));
                        object.set_status_using_ability(false);
                        object.set_ai_state(AIState::SpecialAbility);
                    }
                } else if self.objects.get(&object_id).is_some_and(Object::can_move) {
                    let target_position = self
                        .objects
                        .get(&channel.target_id)
                        .map(Object::get_position)
                        .unwrap_or_default();
                    self.path_approach_with_state(
                        object_id,
                        target_position,
                        AIState::SpecialAbility,
                    );
                } else {
                    self.begin_hacker_disable_building_packing(
                        object_id,
                        channel.target_id,
                        metadata.pack_time_ms,
                    );
                }
            }
            crate::game_logic::HackerDisableChannelPhase::Unpacking => {
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if remaining > COMPLETE_EPSILON {
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.hacker_disable_channel =
                            Some(crate::game_logic::HackerDisableChannelState {
                                target_id: channel.target_id,
                                phase: crate::game_logic::HackerDisableChannelPhase::Unpacking,
                                remaining_seconds: remaining,
                            });
                    }
                } else if self.start_hacker_disable_building_preparation(
                    object_id,
                    channel.target_id,
                    &metadata,
                ) && metadata.preparation_time_ms == 0
                {
                    self.trigger_hacker_disable_building(object_id, channel.target_id, &metadata);
                } else if self
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.hacker_disable_channel.is_none())
                {
                    self.begin_hacker_disable_building_packing(
                        object_id,
                        channel.target_id,
                        metadata.pack_time_ms,
                    );
                }
            }
            crate::game_logic::HackerDisableChannelPhase::Preparing => {
                let target_is_pure_stealth = self
                    .objects
                    .get(&channel.target_id)
                    .is_some_and(Object::is_effectively_stealthed);
                if target_is_pure_stealth
                    || !self.hacker_disable_building_within_abort_range(
                        object_id,
                        channel.target_id,
                        &metadata,
                    )
                {
                    self.begin_hacker_disable_building_packing(
                        object_id,
                        channel.target_id,
                        metadata.pack_time_ms,
                    );
                    return;
                }
                // Source C++ waits at the persistent preparation boundary
                // only when this exact module opted into recharge gating.
                if metadata.persistence_requires_recharge
                    && !self.is_hacker_disable_building_ready(object_id)
                {
                    return;
                }
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if remaining > COMPLETE_EPSILON {
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.hacker_disable_channel =
                            Some(crate::game_logic::HackerDisableChannelState {
                                target_id: channel.target_id,
                                phase: crate::game_logic::HackerDisableChannelPhase::Preparing,
                                remaining_seconds: remaining,
                            });
                    }
                } else {
                    self.trigger_hacker_disable_building(object_id, channel.target_id, &metadata);
                }
            }
            crate::game_logic::HackerDisableChannelPhase::Packing => {
                unreachable!("packing is handled before HDB participant validation")
            }
        }
    }

    pub(in super::super) fn update_support_states(&mut self, object_ids: &[ObjectId], dt: f32) {
        const GUARD_MIN_RADIUS: f32 = 80.0;
        const INTERACT_RANGE: f32 = crate::game_logic::host_repair::HOST_REPAIR_INTERACT_RANGE;
        const CAPTURE_RANGE_PADDING: f32 = 4.0;
        const SPECIAL_ABILITY_RANGE_PADDING: f32 = 4.0;
        // Authored capture durations are integral milliseconds, but the host
        // channel stores the running remainder as `f32` seconds.  A sequence
        // such as 20.0 - 19.9 - 0.1 can otherwise leave one floating-point
        // ulp and defer the C++ frame-boundary trigger by another logic tick.
        // This is far below one authored millisecond (and one 30 Hz logic
        // frame), so it only removes representation residue—not gameplay
        // time.
        const CAPTURE_CHANNEL_COMPLETE_EPSILON: f32 = 0.000_1;
        // Host residual flat HP/sec (not C++ percent-of-max / TimeForFullHeal matrix).
        const REPAIR_RATE: f32 = crate::game_logic::host_repair::HOST_REPAIR_RATE_HP_PER_SEC;
        const HEAL_RATE: f32 = crate::game_logic::host_repair::HOST_HEAL_RATE_HP_PER_SEC;

        for &object_id in object_ids {
            let snapshot = match self.objects.get(&object_id) {
                Some(obj) => (
                    obj.ai_state.clone(),
                    obj.team,
                    obj.owner_player_id,
                    obj.get_position(),
                    obj.target,
                    obj.guard_position,
                    obj.guard_target,
                    obj.guard_radius,
                    obj.guard_mode,
                    obj.can_move(),
                    obj.can_attack(),
                    obj.health.current,
                    obj.health.maximum,
                    obj.selection_radius,
                    obj.is_alive(),
                ),
                None => continue,
            };

            let (
                ai_state,
                team,
                owner_player_id,
                position,
                target_id,
                guard_position,
                guard_target,
                guard_radius,
                guard_mode,
                can_move,
                can_attack,
                health_current,
                health_maximum,
                selection_radius,
                is_alive,
            ) = snapshot;

            if !is_alive {
                continue;
            }

            if ai_state != AIState::SpecialAbility {
                self.pending_special_abilities.remove(&object_id);
                // An explicit replacement order must cancel an in-flight HDB
                // channel without overwriting that new order's target/state.
                // The normal packed completion path below remains responsible
                // for putting a completed channel back to Idle.
                if let Some(object) = self.objects.get_mut(&object_id) {
                    if object.hacker_disable_channel.is_some() {
                        object.hacker_disable_channel = None;
                        object.set_status_using_ability(false);
                    }
                }
            }

            match ai_state {
                AIState::GuardingArea => {
                    let anchor = guard_position.unwrap_or(position);
                    let radius = guard_radius.max(GUARD_MIN_RADIUS);
                    // C++ GuardMode residual (AIGuard.cpp):
                    // Normal — pursue outside (wider acquire).
                    // WithoutPursuit — no outer chase; engage only inside radius.
                    // FlyingUnitsOnly — PartitionFilterIsFlying on acquire.
                    let acquire_radius = match guard_mode {
                        crate::game_logic::GuardMode::Normal => radius * 1.5,
                        _ => radius,
                    };

                    if can_attack {
                        let flying_only =
                            matches!(guard_mode, crate::game_logic::GuardMode::FlyingUnitsOnly);
                        let without_pursuit =
                            matches!(guard_mode, crate::game_logic::GuardMode::WithoutPursuit);
                        // Prefer nearest legal enemy around the guard anchor.
                        let mut best: Option<(ObjectId, f32)> = None;
                        for (cand_id, cand) in self.objects.iter() {
                            if !cand.is_alive() || !cand.is_targetable_by_enemy_of(team) {
                                continue;
                            }
                            // AIGuard.cpp performs this before it assigns a
                            // goal object.  Visibility alone is not enough:
                            // a ground-only guard must keep scanning past an
                            // aircraft and an anti-air guard past a tank.
                            if !matches!(
                                self.get_able_to_attack_specific_object(
                                    object_id,
                                    *cand_id,
                                    AbleToAttackType::NewTarget,
                                    false,
                                ),
                                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                            ) {
                                continue;
                            }
                            if flying_only
                                && !(cand.is_kind_of(KindOf::Aircraft)
                                    || cand.object_type == ObjectType::Aircraft)
                            {
                                continue;
                            }
                            let d = anchor.distance(cand.get_position());
                            if d > acquire_radius {
                                continue;
                            }
                            if without_pursuit && d > radius {
                                continue;
                            }
                            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                                best = Some((*cand_id, d));
                            }
                        }
                        if let Some((enemy_id, _)) = best {
                            // WithoutPursuit: if we already left the bubble, return home first.
                            if without_pursuit && position.distance(anchor) > radius {
                                if can_move {
                                    self.path_approach_with_state(
                                        object_id,
                                        anchor,
                                        AIState::GuardingArea,
                                    );
                                }
                            } else if self.engage_target_decision_aware(object_id, enemy_id) {
                                continue;
                            }
                        }
                    }

                    if can_move && position.distance(anchor) > radius * 0.6 {
                        self.path_approach_with_state(object_id, anchor, AIState::GuardingArea);
                    }
                }
                AIState::GuardingObject => {
                    let guard_target_id = match guard_target {
                        Some(id) => id,
                        None => {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    };

                    let Some(guard_anchor) = self
                        .objects
                        .get(&guard_target_id)
                        .filter(|o| o.is_alive())
                        .map(|o| o.get_position())
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_guard_target(None);
                        }
                        self.clear_target_decision_aware(object_id);
                        continue;
                    };

                    let radius = guard_radius.max(GUARD_MIN_RADIUS);
                    if can_attack {
                        if let Some((enemy_id, _)) =
                            crate::ai_decisions::AIDecisionSystem::find_nearest_enemy_for_attacker(
                                self,
                                object_id,
                                guard_anchor,
                                team,
                                radius,
                            )
                        {
                            if self.engage_target_decision_aware(object_id, enemy_id) {
                                continue;
                            }
                        }
                    }

                    if can_move && position.distance(guard_anchor) > radius * 0.6 {
                        self.path_approach_with_state(
                            object_id,
                            guard_anchor,
                            AIState::GuardingObject,
                        );
                    }
                }
                AIState::Repairing => {
                    let Some(repair_target_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let actor_can_repair = self
                        .objects
                        .get(&object_id)
                        .map(|obj| obj.can_repair() && obj.contained_by.is_none())
                        .unwrap_or(false);
                    if !actor_can_repair {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    let Some((
                        repair_target_pos,
                        repair_target_selection_radius,
                        repair_target_alive,
                        repair_target_is_structure,
                        repair_target_under_construction,
                    )) = self.objects.get(&repair_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Structure),
                            target.status.under_construction,
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !repair_target_alive
                        || !repair_target_is_structure
                        || repair_target_under_construction
                        || !self.repair_relationship_is_not_enemy(object_id, repair_target_id)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if position.distance(repair_target_pos) > INTERACT_RANGE {
                        // Do not replace a live A* route every support tick.
                        // Re-path only if its endpoint is no longer a viable
                        // interaction point, or the mover has stopped; that
                        // preserves obstacle recovery without restarting the
                        // route before movement can consume its next node.
                        let has_valid_active_approach_path =
                            self.objects.get(&object_id).is_some_and(|obj| {
                                obj.status.moving
                                    && obj.movement.current_path_index < obj.movement.path.len()
                                    && obj.movement.path.last().is_some_and(|endpoint| {
                                        endpoint.distance(repair_target_pos) <= INTERACT_RANGE
                                    })
                            });
                        if can_move && !has_valid_active_approach_path {
                            let approach =
                                crate::game_logic::host_repair::support_approach_position(
                                    position,
                                    repair_target_pos,
                                    repair_target_selection_radius,
                                );
                            self.path_approach_with_state(object_id, approach, AIState::Repairing);
                        }
                        // Never heal remotely. This also keeps a valid route
                        // in flight instead of falling through to the repair
                        // effect while still out of range.
                        continue;
                    }

                    // Dozer structure-repair residual: heal HP over time while in range.
                    // C++ DozerAIUpdate DOZER_TASK_REPAIR + MODELCONDITION_ACTIVELY_CONSTRUCTING.
                    // RepairHealthPercentPerSecond residual (2% max HP / sec).
                    // Fail-closed: multi-dozer both allowed (not full sole-benefactor reject).
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_actively_constructing(true);
                    }
                    let max_hp = self
                        .objects
                        .get(&repair_target_id)
                        .map(|t| t.health.maximum)
                        .unwrap_or(0.0);
                    let heal_per_sec =
                        crate::game_logic::host_repair::dozer_repair_hp_per_sec(max_hp)
                            .max(REPAIR_RATE * 0.25);
                    let heal_amount = heal_per_sec * dt;
                    // C++ attemptHealingFromSoleBenefactor(health, dozer, 2) residual.
                    let now = self.frame;
                    let sole = if let Some(target) = self.objects.get_mut(&repair_target_id) {
                        let healed = target.attempt_healing_from_sole_benefactor(
                            heal_amount,
                            object_id,
                            2,
                            now,
                        );
                        let full = target.health.current >= target.health.maximum - 0.01;
                        let pos = target.get_position();
                        Some((full, healed, pos))
                    } else {
                        None
                    };
                    let (target_full, healed, repair_pos) = match sole {
                        Some(v) => v,
                        None => {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                if crate::gameworld_shadow::gameworld_ai_decision_authority_enabled(
                                ) {
                                    crate::game_logic::host_ai_decision_log::record_stop_attack(
                                        object_id,
                                    );
                                    crate::game_logic::host_ai_decision_log::record_set_state(
                                        object_id, 0,
                                    );
                                } else {
                                    obj.set_target(None);
                                    obj.set_ai_state(AIState::Idle);
                                }
                                obj.set_actively_constructing(false);
                            }
                            continue;
                        }
                    };
                    if !healed && !target_full {
                        // Another dozer owns sole-benefactor claim — cancel this dozer task.
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_stop_attack(
                                    object_id,
                                );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, 0,
                                );
                            }
                            obj.set_actively_constructing(false);
                        }
                        self.sole_benefactor_repair_rejects =
                            self.sole_benefactor_repair_rejects.saturating_add(1);
                        continue;
                    }
                    if healed {
                        self.record_structure_repair_residual_heal();
                    }
                    if target_full {
                        // C++ DOZER:RepairComplete residual.
                        let msg = localization::localize("DOZER:RepairComplete", "Repair complete");
                        self.queue_radar_message_at(
                            msg,
                            repair_pos,
                            radar_notifications::RadarKind::Generic,
                        );
                        self.repair_complete_events = self.repair_complete_events.saturating_add(1);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_stop_attack(
                                    object_id,
                                );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, 0,
                                );
                            }
                            obj.set_actively_constructing(false);
                        }
                    }
                }
                state @ (AIState::SeekingRepair | AIState::SeekingHealing) => {
                    if health_current >= health_maximum - 0.01 {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let Some(support_target_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let Some((
                        support_target_pos,
                        support_target_selection_radius,
                        support_target_alive,
                        support_target_under_construction,
                        support_target_sold,
                        support_target_contained,
                        support_target_is_repair_pad,
                        support_target_is_heal_pad,
                        support_target_is_airfield,
                    )) = self.objects.get(&support_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.is_alive(),
                            target.status.under_construction,
                            target.status.sold,
                            target.contained_by.is_some(),
                            target.is_kind_of(KindOf::RepairPad),
                            target.is_kind_of(KindOf::HealPad),
                            target.is_kind_of(KindOf::FSAirfield),
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !support_target_alive
                        || support_target_under_construction
                        || support_target_sold
                        || support_target_contained
                        // C++ `canGetRepairedAt` / `canGetHealedAt` uses the
                        // controlling players' relationship, not a faction
                        // comparison. Repeat that authority check after the
                        // order has begun: capture, diplomacy, or a stale
                        // owner record cannot keep servicing an enemy unit.
                        || !self.service_relationship_is_allies(object_id, support_target_id)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    let source_can_use_support = self
                        .objects
                        .get(&object_id)
                        .map(|obj| {
                            if obj.contained_by.is_some() {
                                return false;
                            }
                            let is_aircraft = obj.is_kind_of(KindOf::Aircraft);
                            let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                            // C++ ActionManager::canGetRepairedAt accepts an
                            // aircraft only while it is above terrain.  Keep
                            // this mutable-state revalidation identical to
                            // command acceptance so landing cannot turn a
                            // pre-existing service order into a free repair.
                            let is_above_terrain = obj.status.airborne_target
                                || (obj.ground_height_from_terrain
                                    && obj.get_position().y > obj.ground_height + 0.01);
                            match state {
                                AIState::SeekingRepair => {
                                    is_vehicle
                                        && if is_aircraft {
                                            support_target_is_airfield && is_above_terrain
                                        } else {
                                            support_target_is_repair_pad
                                        }
                                }
                                AIState::SeekingHealing => {
                                    obj.is_kind_of(KindOf::Infantry) && support_target_is_heal_pad
                                }
                                _ => false,
                            }
                        })
                        .unwrap_or(false);
                    if !source_can_use_support {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    if position.distance(support_target_pos) > INTERACT_RANGE {
                        // Keep a moving, target-valid A* path rather than
                        // restarting it every frame. Re-path only after the
                        // endpoint ceased to be a viable interaction point or
                        // the mover stopped, retaining obstacle recovery.
                        let has_valid_active_approach_path =
                            self.objects.get(&object_id).is_some_and(|obj| {
                                obj.status.moving
                                    && obj.movement.current_path_index < obj.movement.path.len()
                                    && obj.movement.path.last().is_some_and(|endpoint| {
                                        endpoint.distance(support_target_pos) <= INTERACT_RANGE
                                    })
                            });
                        if can_move && !has_valid_active_approach_path {
                            let approach =
                                crate::game_logic::host_repair::support_approach_position(
                                    position,
                                    support_target_pos,
                                    support_target_selection_radius,
                                );
                            self.path_approach_with_state(object_id, approach, state.clone());
                        }
                        // An out-of-range source is never permitted to apply
                        // the repair/heal effect, including after a failed
                        // route allocation.
                        continue;
                    }

                    // Pad/airfield/war-factory residual: heal self over time while docked in range.
                    // C++ RepairDockUpdate::action TimeForFullHeal residual (flat host rate).
                    // HealPad SeekingHealing residual records heal honesty separately.
                    let mut vehicle_healed = false;
                    let mut heal_pad_healed = false;
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        let rate = match state {
                            AIState::SeekingRepair => REPAIR_RATE,
                            AIState::SeekingHealing => HEAL_RATE,
                            _ => 0.0,
                        };
                        let before = obj.health.current;
                        obj.heal(rate * dt);
                        let healed = obj.health.current > before + 0.0001;
                        if healed && matches!(state, AIState::SeekingRepair) {
                            vehicle_healed = true;
                        }
                        if healed && matches!(state, AIState::SeekingHealing) {
                            heal_pad_healed = true;
                        }
                        if obj.health.current >= obj.health.maximum - 0.01 {
                            obj.set_target(None);
                        } else {
                            // Host-immediate residual: keep SeekingRepair/Healing
                            // authoritative on host; log for GameWorld last-write.
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                let ordinal =
                                    crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                                        &state,
                                    );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, ordinal,
                                );
                            }
                            obj.set_ai_state(state);
                        }
                    }
                    if vehicle_healed {
                        self.record_vehicle_repair_residual_heal();
                    }
                    if heal_pad_healed {
                        self.record_heal_pad_residual_heal();
                    }
                }
                state @ (AIState::Entering | AIState::Docking) => {
                    let Some(container_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    };

                    // USA Pilot residual: Enter unmanned vehicle → recrew (not transport load).
                    // Retail VeterancyCrateCollide IsPilot path residual.  The
                    // same parsed authority predicate is repeated here at
                    // arrival so an Enter accepted before an owner/flight
                    // transition cannot re-crew a changed target.
                    {
                        let pilot_snapshot = self.objects.get(&object_id).map(|o| {
                            (
                                o.team,
                                o.owner_player_id,
                                o.experience.level,
                                o.get_position(),
                                o.selection_radius,
                                o.can_move(),
                            )
                        });
                        let vehicle_snapshot = self
                            .objects
                            .get(&container_id)
                            .map(|v| (v.get_position(), v.selection_radius));
                        if let (
                            Some((
                                pilot_team,
                                pilot_owner_player_id,
                                pilot_level,
                                pilot_pos,
                                pilot_radius,
                                pilot_can_move,
                            )),
                            Some((vehicle_pos, vehicle_radius)),
                        ) = (pilot_snapshot, vehicle_snapshot)
                        {
                            if self.can_execute_pilot_recrew(object_id, container_id) {
                                let enter_range = pilot_radius + vehicle_radius + 4.0;
                                if pilot_can_move && pilot_pos.distance(vehicle_pos) > enter_range {
                                    self.path_approach_with_state(
                                        object_id,
                                        vehicle_pos,
                                        AIState::Entering,
                                    );
                                    continue;
                                }
                                let transferred = self
                                    .objects
                                    .get_mut(&container_id)
                                    .map(|v| {
                                        v.apply_pilot_recrew(
                                            pilot_team,
                                            pilot_owner_player_id,
                                            pilot_level,
                                        )
                                    })
                                    .unwrap_or(false);
                                self.usa_pilot.record_recrew(transferred);
                                self.queue_audio_event(
                                    AudioEventRequest::new(
                                        crate::game_logic::host_usa_pilot::PILOT_RECREW_AUDIO,
                                    )
                                    .with_object(container_id)
                                    .with_position(vehicle_pos)
                                    .with_priority(170),
                                );
                                let msg =
                                    localization::localize("hud.pilot.recrew", "Vehicle recrewed");
                                self.queue_radar_message_for_team(pilot_team, msg);
                                self.mark_destroyed_authority_aware(object_id, None);
                                self.mark_object_for_destruction(object_id, Some(pilot_team));
                                continue;
                            }
                        }
                    }

                    // Normal `MSG_ENTER` was already accepted by the command
                    // executor, but the target can change while the unit walks
                    // toward it.  Revalidate through the same centralized
                    // ContainModule/owner/capacity authority at arrival.  Dock
                    // deliberately stays on its separate state machine below.
                    let normal_enter = state == AIState::Entering;
                    if normal_enter && !self.can_unit_enter_normal_target(object_id, container_id) {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }
                    let normal_enter_has_space = normal_enter
                        && self
                            .normal_enter_available_capacity_for(object_id, container_id)
                            .is_some_and(|available| available > 0);

                    let Some((
                        container_pos,
                        container_radius,
                        container_team,
                        container_is_structure,
                        container_is_faction_structure,
                        container_is_overlord_bunker,
                        container_is_battle_bus,
                        container_is_technical,
                        container_is_combat_cycle,
                        container_is_combat_chinook,
                        container_is_listening_outpost,
                        container_is_troop_crawler,
                        container_is_tunnel_network,
                        container_is_alive,
                        container_under_construction,
                        container_can_contain,
                        container_has_space,
                        container_has_unit,
                        container_occupant_count,
                    )) = self.objects.get(&container_id).map(|container| {
                        (
                            container.get_position(),
                            container.selection_radius,
                            container.team,
                            container.is_kind_of(KindOf::Structure),
                            container.is_faction_structure(),
                            container.is_overlord_style_container()
                                && container.overlord_bunker_slot_capacity() > 0,
                            container.is_battle_bus_style_container(),
                            container.is_technical_style_container(),
                            container.is_combat_cycle_style_container(),
                            container.is_combat_chinook_style_container(),
                            container.is_listening_outpost_style_container(),
                            container.is_troop_crawler_style_container(),
                            container.is_tunnel_network_style_container(),
                            container.is_alive(),
                            container.status.under_construction,
                            container.can_contain(),
                            if normal_enter {
                                normal_enter_has_space
                            } else {
                                container.has_capacity_for(1)
                            },
                            container.contained_units().contains(&object_id),
                            container.contained_units().len(),
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    };

                    // Dock's legacy per-role restrictions stay isolated from
                    // normal Enter.  Normal Enter was just checked by the
                    // typed `ContainModule` authority above (including
                    // RiderChange fail-closed), so it must not fall back to a
                    // host-specialized name/flag rule here.
                    let unit_can_garrison_structure = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.is_kind_of(KindOf::Infantry) || o.is_hero())
                        .unwrap_or(false);
                    let unit_is_aircraft = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.is_kind_of(KindOf::Aircraft))
                        .unwrap_or(false);
                    if !normal_enter && container_is_tunnel_network {
                        // TunnelContain residual: reject aircraft only.
                        if unit_is_aircraft {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                            continue;
                        }
                    } else if !normal_enter
                        && (container_is_structure
                            || container_is_overlord_bunker
                            || container_is_battle_bus
                            || container_is_technical
                            || container_is_combat_cycle
                            || container_is_listening_outpost
                            || container_is_troop_crawler)
                        && !unit_can_garrison_structure
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }
                    // Combat Chinook ForbidInsideKindOf = AIRCRAFT HUGE_VEHICLE residual.
                    if !normal_enter && container_is_combat_chinook && unit_is_aircraft {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Tunnel network residual: units already in the shared pool may
                    // transfer to another allied tunnel without walking (can_move false).
                    let already_in_tunnel_network = container_is_tunnel_network
                        && self.tunnel_network.team_holding_unit(object_id).is_some();

                    if (!can_move && !already_in_tunnel_network)
                        || !container_is_alive
                        || container_under_construction
                        || !container_can_contain
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if !normal_enter
                        && container_team != team
                        && container_team != Team::Neutral
                        && (container_is_faction_structure || container_occupant_count > 0)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let enter_range = selection_radius + container_radius + 4.0;
                    // Cross-tunnel residual transfer: skip walk when already in pool.
                    if !already_in_tunnel_network
                        && can_move
                        && position.distance(container_pos) > enter_range
                    {
                        self.path_approach_with_state(object_id, container_pos, state);
                        continue;
                    }

                    // RiderChangeContain is an atomic replacement, never a
                    // generic one-slot `add_occupant`.  Keep every parsed
                    // RiderChange target on this authoritative branch so an
                    // unsupported/custom roster cannot fall through to the
                    // legacy Combat Cycle template-name refresh below.
                    let is_rider_change_target = normal_enter
                        && self.objects.get(&container_id).is_some_and(|container| {
                            container.thing.template.contain_module.kind
                                == crate::game_logic::ContainModuleKind::RiderChange
                        });
                    if is_rider_change_target {
                        if !self.rider_change_enter_at_arrival(object_id, container_id) {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        continue;
                    }

                    // Tunnel shared capacity (MaxTunnelCapacity=10) overrides local space.
                    let tunnel_has_space = if container_is_tunnel_network {
                        self.tunnel_network.is_in_network(team, object_id)
                            || self.tunnel_network.has_capacity(team)
                    } else {
                        true
                    };
                    let can_enter = container_has_unit
                        || (container_has_space && tunnel_has_space)
                        || already_in_tunnel_network;
                    if !can_enter {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let entered = if container_has_unit {
                        true
                    } else {
                        self.objects
                            .get_mut(&container_id)
                            .map(|container| container.add_occupant(object_id))
                            .unwrap_or(false)
                    };
                    if !entered {
                        continue;
                    }

                    // Shared pool bookkeeping for tunnel residual.
                    if container_is_tunnel_network {
                        if !self
                            .tunnel_network
                            .record_enter(team, object_id, container_id)
                        {
                            // Capacity race: undo local occupant add.
                            if let Some(container) = self.objects.get_mut(&container_id) {
                                container.remove_occupant(object_id);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.set_status_attacking(false);
                        obj.target_location = None;
                        obj.set_status_force_attack(false);
                        obj.target = Some(container_id);
                        obj.set_contained_by(Some(container_id));
                        obj.set_position(container_pos);
                        crate::game_logic::host_ground_height_log::record(
                            obj.id,
                            container_pos.y,
                            false,
                        );
                        if crate::gameworld_shadow::gameworld_movement_authority_live() {
                            crate::game_logic::host_move_log::record(
                                obj.id,
                                Some([container_pos.x, container_pos.y, container_pos.z]),
                            );
                            obj.record_host_movement();
                        }
                        let __ai_st = if container_is_structure {
                            AIState::Garrisoned
                        } else {
                            AIState::Docked
                        };
                        // Host-immediate garrison/dock residual under decision auth.
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            let ordinal =
                                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                                    &__ai_st,
                                );
                            crate::game_logic::host_ai_decision_log::record_set_state(
                                object_id, ordinal,
                            );
                        }
                        obj.set_ai_state(__ai_st);
                        obj.set_status_moving(false);
                    }
                    if container_is_tunnel_network {
                        // Enter counter already incremented in record_enter.
                    } else if container_is_structure {
                        self.record_garrison_residual_enter();
                    } else if container_is_overlord_bunker {
                        // China Overlord BattleBunker residual load (redirected bunker slots).
                        self.record_overlord_bunker_residual_enter();
                    } else if container_is_battle_bus {
                        // GLA Battle Bus residual load (Slots=8 infantry transport).
                        self.record_battle_bus_residual_load();
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    } else if container_is_technical {
                        // GLA Technical residual load (Slots=5 infantry; no passenger fire).
                        self.record_technical_residual_load();
                    } else if container_is_combat_cycle {
                        // GLA Combat Cycle residual load (Slots=1) + rider weapon switch.
                        self.record_combat_cycle_residual_load();
                        self.refresh_combat_cycle_rider_weapon(container_id);
                    } else if container_is_combat_chinook {
                        // AirF Combat Chinook residual load (Slots=8 + passenger fire).
                        self.record_combat_chinook_residual_load();
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    } else if container_is_listening_outpost {
                        // China Listening Outpost residual load (Slots=2 + passenger fire).
                        self.record_listening_outpost_residual_load();
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    } else if container_is_troop_crawler {
                        // China Troop Crawler residual load (Slots=8; exit-to-fight).
                        self.record_troop_crawler_residual_load();
                    } else {
                        // Vehicle transport residual load (Humvee / generic transport).
                        self.record_transport_residual_load();
                        // Humvee-style PassengersAllowedToFire still refreshes weapon set
                        // when ArmedRidersUpgradeMyWeaponSet is set.
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    }
                }
                AIState::Capturing => {
                    let Some(capture_target_id) = target_id else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };

                    let (
                        capture_power,
                        capture_start_range,
                        capture_unpack_time_ms,
                        capture_preparation_time_ms,
                        capture_pack_time_ms,
                        capture_channel,
                    ) = self
                        .objects
                        .get(&object_id)
                        .map(|obj| {
                            (
                                obj.thing.template.capture_power,
                                obj.thing.template.capture_start_ability_range,
                                obj.thing.template.capture_unpack_time_ms,
                                obj.thing.template.capture_preparation_time_ms,
                                obj.thing.template.capture_pack_time_ms,
                                obj.capture_channel,
                            )
                        })
                        .unwrap_or((
                            crate::game_logic::CapturePowerKind::None,
                            None,
                            None,
                            None,
                            None,
                            None,
                        ));
                    let Some(power_type) = capture_power.special_power_type() else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };

                    // Once C++ `startPacking` has run, the target may already
                    // have defected or vanished.  Packing is deliberately
                    // independent of capture legality and completes before
                    // this source becomes idle again.
                    if let Some(channel) = capture_channel {
                        if channel.phase == crate::game_logic::CaptureChannelPhase::Packing {
                            let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                            if remaining > CAPTURE_CHANNEL_COMPLETE_EPSILON {
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.capture_channel =
                                        Some(crate::game_logic::CaptureChannelState {
                                            phase: crate::game_logic::CaptureChannelPhase::Packing,
                                            remaining_seconds: remaining,
                                        });
                                }
                            } else {
                                self.finish_capture_channel(object_id);
                            }
                            continue;
                        }
                    }

                    let Some((target_position, target_radius, target_team)) =
                        self.objects.get(&capture_target_id).map(|target| {
                            (target.get_position(), target.selection_radius, target.team)
                        })
                    else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };

                    // All timing fields come from the same exact
                    // SpecialAbilityUpdate module.  A partial/unsupported
                    // parse must not invent a zero-duration capture ability.
                    let Some(authored_range) = capture_start_range else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };
                    let Some(unpack_time_ms) = capture_unpack_time_ms else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };
                    let Some(preparation_time_ms) = capture_preparation_time_ms else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };
                    let Some(pack_time_ms) = capture_pack_time_ms else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };

                    // C++ checks the target at issue/start-preparation and
                    // continues to abort if an ally/dead/immune/garrisoned
                    // target is no longer legal.  Do not re-demand readiness:
                    // the timer begins only below in start_capture_preparation.
                    if !self.can_unit_capture_building(object_id, capture_target_id, false) {
                        self.abort_capture_channel(object_id);
                        continue;
                    }

                    // C++ SpecialAbilityUpdate owns `StartAbilityRange`.
                    // The host's point-position movement retains selection
                    // radii as the collision approach envelope; a missing
                    // exact module is fail-closed rather than using a hero or
                    // infantry template-name fallback.
                    let capture_range = authored_range + selection_radius + target_radius;
                    if capture_channel.is_none()
                        && can_move
                        && position.distance(target_position) > capture_range
                    {
                        if self.assign_unit_path(object_id, target_position, &[]) {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                if crate::gameworld_shadow::gameworld_ai_decision_authority_enabled(
                                ) {
                                    crate::game_logic::host_ai_decision_log::record_set_state(
                                        object_id, 19,
                                    ); // Capturing
                                } else {
                                    obj.set_ai_state(AIState::Capturing);
                                }
                            }
                        } else if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(target_position);
                            obj.set_ai_state(AIState::Capturing);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, 19,
                                ); // Capturing
                            }
                        }
                        continue;
                    }

                    // C++ initializes `STATE_UNPACKED` immediately when
                    // UnpackTime is zero; otherwise the first logical tick
                    // after entering range consumes the unpack timer.
                    let mut preparation_complete = false;
                    match capture_channel {
                        None => {
                            if unpack_time_ms > 0 {
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.capture_channel =
                                        Some(crate::game_logic::CaptureChannelState::new(
                                            crate::game_logic::CaptureChannelPhase::Unpacking,
                                            unpack_time_ms,
                                        ));
                                }
                                continue;
                            }
                            if !self.start_capture_preparation(
                                object_id,
                                capture_target_id,
                                capture_power,
                                preparation_time_ms,
                            ) {
                                self.abort_capture_channel(object_id);
                                continue;
                            }
                            preparation_complete = preparation_time_ms == 0;
                        }
                        Some(channel)
                            if channel.phase
                                == crate::game_logic::CaptureChannelPhase::Unpacking =>
                        {
                            let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                            if remaining > CAPTURE_CHANNEL_COMPLETE_EPSILON {
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.capture_channel =
                                        Some(crate::game_logic::CaptureChannelState {
                                            phase:
                                                crate::game_logic::CaptureChannelPhase::Unpacking,
                                            remaining_seconds: remaining,
                                        });
                                }
                                continue;
                            }
                            if !self.start_capture_preparation(
                                object_id,
                                capture_target_id,
                                capture_power,
                                preparation_time_ms,
                            ) {
                                self.abort_capture_channel(object_id);
                                continue;
                            }
                            preparation_complete = preparation_time_ms == 0;
                        }
                        Some(channel)
                            if channel.phase
                                == crate::game_logic::CaptureChannelPhase::Preparing =>
                        {
                            let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                            if remaining > CAPTURE_CHANNEL_COMPLETE_EPSILON {
                                // C++ `continuePreparation` restarts infantry
                                // capture ReloadTime every preparation frame.
                                // Black Lotus instead resets its zero timer at
                                // the successful trigger below.
                                if capture_power != crate::game_logic::CapturePowerKind::BlackLotus
                                {
                                    if let Some(object) = self.objects.get_mut(&object_id) {
                                        object.start_power_recharge(&power_type);
                                    }
                                }
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.capture_channel =
                                        Some(crate::game_logic::CaptureChannelState {
                                            phase:
                                                crate::game_logic::CaptureChannelPhase::Preparing,
                                            remaining_seconds: remaining,
                                        });
                                }
                                continue;
                            }
                            preparation_complete = true;
                        }
                        Some(_) => unreachable!("Packing is handled before capture legality"),
                    }

                    if !preparation_complete {
                        continue;
                    }

                    // C++ checks an enemy trap again at trigger; one planted
                    // during the visible preparation bar can still interrupt
                    // the actual defection.
                    let planter_ally = self
                        .booby_trap
                        .plant(capture_target_id)
                        .map(|plant| plant.planter_team == team)
                        .unwrap_or(false);
                    let target_is_trapped = self.booby_trap.is_booby_trapped(capture_target_id)
                        || self
                            .objects
                            .get(&capture_target_id)
                            .map(|target| target.status.booby_trapped)
                            .unwrap_or(false);
                    if !planter_ally && target_is_trapped {
                        let _ = self.detonate_booby_trap_at(
                            capture_target_id,
                            target_position,
                            Some(object_id),
                            true,
                            false,
                        );
                    }

                    let did_capture =
                        if self.can_unit_capture_building(object_id, capture_target_id, false) {
                            let target_is_garrisonable =
                                self.objects.get(&capture_target_id).is_some_and(|target| {
                                    target.thing.template.garrison_contain_max.is_some()
                                });
                            if target_is_garrisonable {
                                // C++ `removeAllContained(TRUE); break;`: clearing
                                // a garrison is a successful ability trigger but
                                // never defects that structure on the same use.
                                self.evacuate_garrison_for_capture(capture_target_id);
                                false
                            } else {
                                self.cancel_all_production(capture_target_id);
                                let transferred = match owner_player_id {
                                    Some(player_id) => {
                                        self.transfer_object_to_player(capture_target_id, player_id)
                                    }
                                    None => {
                                        if let Some(target) =
                                            self.objects.get_mut(&capture_target_id)
                                        {
                                            target.set_team(team);
                                            true
                                        } else {
                                            false
                                        }
                                    }
                                };
                                if transferred {
                                    self.objects
                                        .get_mut(&capture_target_id)
                                        .map(|target| {
                                            target.health.heal(target.max_health);
                                            // C++ defect(..., 1) one-frame flash residual.
                                            target.flash_as_selected();
                                            true
                                        })
                                        .unwrap_or(false)
                                } else {
                                    false
                                }
                            }
                        } else {
                            false
                        };

                    // `endPreparation` clears this status before C++ starts
                    // PackTime. The source remains in the capture machine so
                    // a completed capture is not reported as immediately idle.
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        if object.is_alive() {
                            object.stop_moving();
                            object.set_status_using_ability(false);
                            object.capture_channel =
                                Some(crate::game_logic::CaptureChannelState::new(
                                    crate::game_logic::CaptureChannelPhase::Packing,
                                    pack_time_ms,
                                ));
                            object.set_ai_state(AIState::Capturing);
                        }
                    }
                    if pack_time_ms == 0 {
                        self.finish_capture_channel(object_id);
                    }

                    if did_capture {
                        // C++ Object::onCapture residual (kick/idle/AI-sell/deselect).
                        self.on_capture_object_residual(capture_target_id, target_team, team);
                        // C++ getAcademyStats()->recordBuildingCapture() residual.
                        let player = match owner_player_id {
                            Some(player_id) => self.get_player_mut(player_id),
                            None => self.get_player_mut_by_team(team),
                        };
                        if let Some(p) = player {
                            p.record_building_capture();
                        }
                        if capture_power == crate::game_logic::CapturePowerKind::BlackLotus {
                            self.hero_abilities.record_building_capture();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_hero_abilities::CAPTURE_BUILDING_AUDIO,
                                )
                                .with_object(object_id)
                                .with_position(position)
                                .with_priority(160),
                            );
                        }
                        if capture_power == crate::game_logic::CapturePowerKind::BlackLotus {
                            if let Some(object) = self.objects.get_mut(&object_id) {
                                // C++ triggerAbilityEffect restarts only the
                                // Black Lotus capture timer here; infantry
                                // capture repeatedly reset it during prep.
                                object.start_power_recharge(&power_type);
                            }
                        }
                        // C++ EVA_BuildingStolen when victim was local before defect.
                        // (team already flipped — use BeingStolen honesty or explicit
                        // pre-flip: fire BuildingStolen if victim team had local player
                        // that is no longer owner.)
                        // BeingStolen already gated on pre-flip local control; Stolen
                        // should also only fire for former local owner.
                        // Re-check: after flip, former local team lost the building —
                        // if any local player is on previous target_team.
                        let former_local = self
                            .players
                            .values()
                            .any(|p| p.is_local && p.is_alive && p.team == target_team);
                        if former_local {
                            let _ = gamelogic::helpers::TheEva::set_should_play(
                                gamelogic::helpers::EvaEvent::BuildingStolen,
                            );
                            crate::game_logic::host_eva_log::record_event(
                                gamelogic::helpers::EvaEvent::BuildingStolen,
                            );
                            self.hero_abilities.record_eva_building_stolen();
                        }
                        let msg =
                            localization::localize("hud.capture.complete", "Building captured");
                        self.queue_radar_message_for_team(team, msg);
                    }
                }
                AIState::SpecialAbility => {
                    let Some(ability) = self.pending_special_abilities.get(&object_id).copied()
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.hacker_disable_channel = None;
                            obj.set_status_using_ability(false);
                            obj.set_target(None);
                        }
                        continue;
                    };
                    let special_target_id = ability.target_id();

                    // HDB is an authored, persistent SpecialAbilityUpdate
                    // channel.  Keep it wholly outside the legacy generic
                    // special branch: that branch uses a fixed range and used
                    // to apply the disable instantly (and reject an already
                    // disabled target), none of which matches C++.
                    if matches!(ability, PendingSpecialAbility::HackerDisableBuilding { .. }) {
                        self.update_hacker_disable_building_channel(
                            object_id,
                            special_target_id,
                            dt,
                        );
                        continue;
                    }

                    let Some((
                        target_position,
                        target_radius,
                        target_team,
                        target_alive,
                        target_is_vehicle,
                        target_is_structure,
                        target_is_airborne,
                        target_is_carbomb,
                        target_is_hijacked,
                        target_is_hacked,
                        target_is_unmanned,
                    )) = self.objects.get(&special_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.team,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Vehicle),
                            target.is_kind_of(KindOf::Structure),
                            target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                            target.status.is_carbomb,
                            target.status.hijacked,
                            target.status.disabled_hacked,
                            target.status.disabled_unmanned,
                        )
                    })
                    else {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    // CarBomb allows neutral; DisguiseAsVehicle allows any living
                    // vehicle (ally/enemy/neutral) — C++ ActionManager residual.
                    let requires_enemy_target = !matches!(
                        ability,
                        PendingSpecialAbility::CarBomb { .. }
                            | PendingSpecialAbility::DisguiseAsVehicle { .. }
                    );
                    if !target_alive
                        || (requires_enemy_target
                            && (target_team == team || target_team == Team::Neutral))
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if matches!(
                        ability,
                        PendingSpecialAbility::SnipeVehicle { .. }
                            | PendingSpecialAbility::Hijack { .. }
                            | PendingSpecialAbility::CarBomb { .. }
                            | PendingSpecialAbility::DisableVehicleHack { .. }
                            | PendingSpecialAbility::DisguiseAsVehicle { .. }
                    ) && (!target_is_vehicle || target_is_airborne)
                    {
                        self.pending_special_abilities.remove(&object_id);
                        self.clear_target_decision_aware(object_id);
                        continue;
                    }

                    // Disguise: reject bomb-truck / train name residual targets,
                    // unless the target is already disguised (C++ disguiseAsObject
                    // copies that appearance — true template may still be bomb truck).
                    if matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. }) {
                        use crate::game_logic::host_bomb_truck_disguise::{
                            is_bomb_truck_template, is_legal_disguise_target_template,
                        };
                        let (target_tpl, target_disguised) = self
                            .objects
                            .get(&special_target_id)
                            .map(|t| (t.template_name.clone(), t.status.disguised))
                            .unwrap_or_default();
                        let reject_bomb = is_bomb_truck_template(&target_tpl) && !target_disguised;
                        if reject_bomb || !is_legal_disguise_target_template(&target_tpl) {
                            // is_legal rejects bomb trucks by name; allow when disguised.
                            if !(target_disguised && is_bomb_truck_template(&target_tpl)) {
                                self.pending_special_abilities.remove(&object_id);
                                if let Some(obj) = self.objects.get_mut(&object_id) {
                                    obj.set_target(None);
                                }
                                continue;
                            }
                        }
                    }

                    // ConvertToCarBomb: cannot re-convert an existing car bomb.
                    if matches!(ability, PendingSpecialAbility::CarBomb { .. }) && target_is_carbomb
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Hijack: cannot re-hijack an already hijacked vehicle.
                    if matches!(ability, PendingSpecialAbility::Hijack { .. }) && target_is_hijacked
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Disable vehicle hack: skip already-hacked or unmanned vehicles.
                    if matches!(ability, PendingSpecialAbility::DisableVehicleHack { .. })
                        && (target_is_hacked || target_is_unmanned)
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if matches!(ability, PendingSpecialAbility::Sabotage { .. })
                        && !target_is_structure
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Burton plant charge (timed or remote): structure or ground vehicle.
                    if matches!(
                        ability,
                        PendingSpecialAbility::PlantTimedDemoCharge { .. }
                            | PendingSpecialAbility::PlantRemoteDemoCharge { .. }
                    ) && !(target_is_structure || (target_is_vehicle && !target_is_airborne))
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Black Lotus cash hack: enemy cash-generator structures only.
                    if matches!(ability, PendingSpecialAbility::StealCashHack { .. }) {
                        let is_cash_gen = self
                            .objects
                            .get(&special_target_id)
                            .map(|t| {
                                crate::game_logic::host_hero_abilities::is_cash_hack_target(
                                    &t.template_name,
                                    t.is_kind_of(KindOf::SupplyCenter),
                                    t.is_kind_of(KindOf::FSSupplyCenter),
                                    t.is_kind_of(KindOf::FSBlackMarket),
                                    t.is_kind_of(KindOf::FSSupplyDropzone),
                                )
                            })
                            .unwrap_or(false);
                        if !target_is_structure || !is_cash_gen {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // GLA Rebel BoobyTrap: structures only (enemy/neutral residual).
                    if matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. }) {
                        if !target_is_structure {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // C++ SpecialAbilityDisguiseAsVehicle StartAbilityRange = 1e6
                    // residual: complete without approach walk.
                    let disguise_instant =
                        matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. });
                    // Black Lotus residual specials: StartAbilityRange 150.
                    let black_lotus_range = matches!(
                        ability,
                        PendingSpecialAbility::StealCashHack { .. }
                            | PendingSpecialAbility::DisableVehicleHack { .. }
                    );
                    let booby_trap_range =
                        matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. });
                    let interact_range = if black_lotus_range {
                        crate::game_logic::host_hero_abilities::BLACK_LOTUS_START_ABILITY_RANGE
                    } else if booby_trap_range {
                        crate::game_logic::host_booby_trap::BOOBY_START_ABILITY_RANGE
                            + selection_radius
                            + target_radius
                    } else {
                        selection_radius + target_radius + SPECIAL_ABILITY_RANGE_PADDING
                    };
                    if !disguise_instant
                        && can_move
                        && position.distance(target_position) > interact_range
                    {
                        self.path_approach_with_state(
                            object_id,
                            target_position,
                            AIState::SpecialAbility,
                        );
                        continue;
                    }

                    match ability {
                        PendingSpecialAbility::Hijack { .. } => {
                            // C++ ConvertToHijackedVehicleCrateCollide residual:
                            // walk → transfer team + OBJECT_STATUS_HIJACKED; hijacker
                            // consumed (fail-closed vs hide-in-vehicle HijackerUpdate).
                            // Endow MAX veterancy + cancel dozer tasks via apply_hijacked_from.
                            // C++ order: tryInfiltrationEvent → EVA_VehicleStolen → setTeam.
                            self.try_infiltration_event(special_target_id);
                            self.try_eva_vehicle_stolen(special_target_id);
                            let donor_snap = self.objects.get(&object_id).cloned();
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_hijacked_from(donor_snap.as_ref());
                            }
                            match owner_player_id {
                                Some(player_id) => {
                                    let _ = self
                                        .transfer_object_to_player(special_target_id, player_id);
                                }
                                None => {
                                    if let Some(target) = self.objects.get_mut(&special_target_id) {
                                        target.set_team(team);
                                    }
                                }
                            }
                            // C++ transferObjectName residual.
                            let _ = self.transfer_script_object_name(object_id, special_target_id);
                            self.car_bomb.record_hijack();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_car_bomb::HIJACK_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
                            let msg =
                                localization::localize("hud.hijack.complete", "Vehicle hijacked");
                            self.queue_radar_message_for_team(team, msg);
                            // C++: if target has EjectPilotDie → hide hijacker in vehicle;
                            // else destroy hijacker immediately.
                            // Wave 753: ride-hide only when the hijacker is infantry
                            // (HijackerUpdate module). Non-infantry steal path destroys
                            // the attacker immediately (test/tank harness + C++ shape).
                            // C++: if target has EjectPilotDie and hijacker is infantry
                            // (HijackerUpdate) → hide in vehicle; else consume attacker.
                            // Wave 753: ride-hide only for infantry; non-infantry steal
                            // destroys immediately. SlowDeath must not clear destroyed —
                            // hijacker consume is same-frame (begin_slow_death clears the
                            // destroyed flag for delayed peels).
                            let hijacker_is_infantry = self
                                .objects
                                .get(&object_id)
                                .map(|h| {
                                    h.is_kind_of(KindOf::Infantry)
                                        || h.object_type == ObjectType::Infantry
                                })
                                .unwrap_or(false);
                            if hijacker_is_infantry
                                && self.vehicle_supports_hijacker_ride(special_target_id)
                            {
                                if let Some(h) = self.objects.get_mut(&object_id) {
                                    h.begin_hijacker_in_vehicle(special_target_id);
                                }
                            } else {
                                self.mark_destroyed_authority_aware(object_id, None);
                                // Suppress SlowDeath/jet/heli peels so consume sticks.
                                if let Some(o) = self.objects.get_mut(&object_id) {
                                    o.slow_death = None;
                                    o.jet_slow_death = None;
                                    o.helicopter_slow_death = None;
                                    o.status.effectively_dead = true;
                                    o.status.destroyed = true;
                                }
                                self.mark_object_for_destruction(object_id, Some(team));
                                // mark_object may re-enter SlowDeath and clear destroyed;
                                // re-assert consume residual for hijack steal.
                                if let Some(o) = self.objects.get_mut(&object_id) {
                                    o.slow_death = None;
                                    o.jet_slow_death = None;
                                    o.helicopter_slow_death = None;
                                    o.status.effectively_dead = true;
                                    o.status.destroyed = true;
                                    if !crate::gameworld_shadow::gameworld_damage_authority_live()
                                        && o.health.current > 0.0
                                    {
                                        o.health.current = 0.0;
                                    }
                                }
                            }
                        }
                        PendingSpecialAbility::Sabotage { .. } => {
                            // C++ Sabotage*CrateCollide residual: type-specific structure
                            // sabotage; saboteur consumed on success (mobile crate).
                            use crate::game_logic::host_saboteur::{
                                classify_sabotage_target, is_saboteur_template, SaboteurEffectKind,
                                SABOTEUR_CASH_STEAL_AUDIO, SABOTEUR_RESET_TIMER_AUDIO,
                                SABOTEUR_STEAL_CASH_AMOUNT, SABOTEUR_SUCCESS_AUDIO,
                            };
                            let saboteur_ok = self
                                .objects
                                .get(&object_id)
                                .map(|o| is_saboteur_template(&o.template_name))
                                .unwrap_or(false);
                            let effect = self.objects.get(&special_target_id).and_then(|t| {
                                classify_sabotage_target(
                                    &t.template_name,
                                    t.is_kind_of(KindOf::FSPower),
                                    t.is_kind_of(KindOf::PowerPlant),
                                    t.is_kind_of(KindOf::FSSupplyCenter),
                                    t.is_kind_of(KindOf::SupplyCenter),
                                    t.is_kind_of(KindOf::FSBarracks),
                                    t.is_kind_of(KindOf::FSWarFactory),
                                    t.is_kind_of(KindOf::FSAirfield),
                                    t.is_kind_of(KindOf::FSSuperweapon),
                                    t.is_kind_of(KindOf::FSStrategyCenter),
                                    t.is_kind_of(KindOf::CommandCenter),
                                    t.is_kind_of(KindOf::FSInternetCenter),
                                    t.is_kind_of(KindOf::FSFake),
                                )
                            });
                            if saboteur_ok {
                                if let Some(kind) = effect {
                                    let mut cash_stolen = 0u32;
                                    match kind {
                                        SaboteurEffectKind::PowerPlant => {
                                            let until = self.frame.saturating_add(
                                                crate::game_logic::host_saboteur::SABOTEUR_POWER_DURATION_FRAMES,
                                            );
                                            if let Some(player) =
                                                self.get_player_mut_by_team(target_team)
                                            {
                                                player.power_sabotaged_till_frame = until;
                                            }
                                        }
                                        SaboteurEffectKind::SupplyCenter => {
                                            cash_stolen = self.steal_cash_from_team(
                                                target_team,
                                                team,
                                                SABOTEUR_STEAL_CASH_AMOUNT,
                                            );
                                        }
                                        SaboteurEffectKind::MilitaryFactory => {
                                            if let Some(until) =
                                                kind.disabled_hacked_until(self.frame)
                                            {
                                                if let Some(target) =
                                                    self.objects.get_mut(&special_target_id)
                                                {
                                                    target.apply_disabled_hacked(until);
                                                }
                                            }
                                        }
                                        SaboteurEffectKind::InternetCenter => {
                                            // C++ SabotageInternetCenterCrateCollide residual:
                                            // 1) disable SpyVisionUpdate on ALL team internet centers
                                            // 2) DISABLED_HACKED on the sabotaged center
                                            // 3) DISABLED_HACKED on contained hackers
                                            let until = kind
                                                .disabled_hacked_until(self.frame)
                                                .unwrap_or_else(|| {
                                                    self.frame.saturating_add(
                                                        crate::game_logic::host_saboteur::SABOTEUR_INTERNET_DURATION_FRAMES,
                                                    )
                                                });
                                            let (centers, hackers) = self
                                                .apply_internet_center_sabotage_residual(
                                                    special_target_id,
                                                    target_team,
                                                    until,
                                                );
                                            self.saboteur.record_internet_spy_vision_disable(
                                                centers, hackers,
                                            );
                                        }
                                        SaboteurEffectKind::SuperweaponOrCommand => {
                                            // C++ SabotageSuperweaponCrateCollide: reset ALL
                                            // SpecialPowerModule interfaces via startPowerRecharge.
                                            // Host residual: object-level special power + strike
                                            // registry timers for this structure.
                                            let reset_ok = self
                                                .apply_superweapon_sabotage_recharge(
                                                    special_target_id,
                                                );
                                            if reset_ok {
                                                self.saboteur.record_superweapon_power_reset();
                                            }
                                        }
                                        SaboteurEffectKind::FakeBuilding => {
                                            // C++ SabotageFakeBuildingCrateCollide:
                                            // DAMAGE_UNRESISTABLE / DEATH_DETONATED for max health.
                                            let destroyed = self
                                                .objects
                                                .get_mut(&special_target_id)
                                                .map(|target| {
                                                    let max_hp = target
                                                        .health
                                                        .maximum
                                                        .max(target.max_health)
                                                        .max(1.0);
                                                    target.take_damage_from_typed_death(
                                                        max_hp,
                                                        Some(object_id),
                                                        crate::game_logic::combat::DamageType::Unresistable,
                                                        crate::game_logic::host_usa_pilot::HostDeathType::Detonated,
                                                    )
                                                })
                                                .unwrap_or(false);
                                            if destroyed {
                                                self.mark_object_for_destruction(
                                                    special_target_id,
                                                    Some(team),
                                                );
                                                self.saboteur.record_fake_detonated();
                                            }
                                        }
                                    }
                                    self.saboteur.record(kind, cash_stolen);
                                    // C++ TheRadar->tryInfiltrationEvent(other) residual
                                    // (victim local player warning).
                                    self.try_infiltration_event(special_target_id);
                                    // C++ TheEva->setShouldPlay residual when victim local.
                                    // Supply center: CashStolen if cash taken, else BuildingSabotaged.
                                    if kind.steals_cash() && cash_stolen > 0 {
                                        // C++ controller ScoreKeeper::addMoneyEarned residual.
                                        if let Some(p) = self.get_player_mut_by_team(team) {
                                            p.add_money_earned(cash_stolen);
                                        }
                                        self.try_eva_cash_stolen(special_target_id);
                                        // C++ GUI:AddCash / GUI:LoseCash floating text residual.
                                        self.spawn_sabotage_cash_floating_texts(
                                            object_id,
                                            special_target_id,
                                            cash_stolen,
                                        );
                                    } else {
                                        self.try_eva_building_sabotaged(special_target_id);
                                    }
                                    // C++ doSabotageFeedbackFX residual (type audio + flash).
                                    self.do_sabotage_feedback_fx(special_target_id, kind);
                                    let msg = localization::localize(
                                        "hud.saboteur.complete",
                                        "Building sabotaged",
                                    );
                                    self.queue_radar_message_for_team(team, msg);
                                    // C++ CrateCollide: destroy saboteur (mobile crate).
                                    self.mark_destroyed_authority_aware(object_id, None);
                                    self.mark_object_for_destruction(object_id, Some(team));
                                    self.saboteur.record_consumed();
                                } else if let Some(obj) = self.objects.get_mut(&object_id) {
                                    // Fail-closed: non-matching structure — cancel residual.
                                    obj.stop_moving();
                                    obj.set_target(None);
                                }
                            } else if let Some(obj) = self.objects.get_mut(&object_id) {
                                // Fail-closed: non-saboteur cannot complete residual.
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::SnipeVehicle { .. } => {
                            // C++ DAMAGE_KILLPILOT residual: no HP damage; vehicle becomes
                            // unmanned + Neutral so it can be recrewed/captured.
                            // C++ car-bomb dead-man: IS_CARBOMB detonates instead.
                            let is_bomb = self
                                .objects
                                .get(&special_target_id)
                                .map(|t| t.is_car_bomb())
                                .unwrap_or(false);
                            if is_bomb {
                                let _ = self.maybe_detonate_carbomb_on_unmanned(special_target_id);
                            } else if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_kill_pilot_unmanned();
                                target.set_team(Team::Neutral);
                            }
                            self.hero_abilities.record_snipe();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_hero_abilities::SNIPE_VEHICLE_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
                            let msg = localization::localize(
                                "hud.snipe.vehicle_unmanned",
                                "Vehicle unmanned",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::PlantTimedDemoCharge { .. } => {
                            // Burton / Tank Hunter TNT residual: plant sticky timed charge at target.
                            let is_tank_hunter = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    crate::game_logic::host_tank_hunter::is_tank_hunter_template(
                                        &o.template_name,
                                    )
                                })
                                .unwrap_or(false);
                            // Tank Hunter TNT reload residual (7500ms / 225 frames).
                            let tnt_ready = if is_tank_hunter {
                                crate::game_logic::host_tank_hunter::tnt_ready(
                                    self.frame,
                                    self.tank_hunter_tnt_last_frame.get(&object_id).copied(),
                                )
                            } else {
                                true
                            };
                            let charge_id = if tnt_ready {
                                self.place_timed_demo_charge(
                                    team,
                                    target_position,
                                    Some(object_id),
                                    Some(special_target_id),
                                    None,
                                )
                            } else {
                                None
                            };
                            if charge_id.is_some() {
                                self.hero_abilities.record_timed_charge_plant();
                                if is_tank_hunter {
                                    self.tank_hunter_residual_tnt_plants =
                                        self.tank_hunter_residual_tnt_plants.saturating_add(1);
                                    self.tank_hunter_tnt_last_frame
                                        .insert(object_id, self.frame);
                                    self.queue_audio_event(
                                        AudioEventRequest::new(
                                            crate::game_logic::host_tank_hunter::TNT_INITIATE_AUDIO,
                                        )
                                        .with_object(object_id)
                                        .with_position(target_position)
                                        .with_priority(160),
                                    );
                                }
                                let msg = localization::localize(
                                    "hud.demo_charge.planted",
                                    "Demo charge planted",
                                );
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::PlantRemoteDemoCharge { .. } => {
                            // Burton residual: plant sticky remote charge (no auto-timer).
                            let charge_id = self.place_remote_demo_charge(
                                team,
                                target_position,
                                Some(object_id),
                                Some(special_target_id),
                            );
                            if charge_id.is_some() {
                                self.hero_abilities.record_remote_charge_plant();
                                let msg = localization::localize(
                                    "hud.remote_demo_charge.planted",
                                    "Remote demo charge planted",
                                );
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::StealCashHack { .. } => {
                            // Black Lotus residual: steal cash from enemy economy.
                            // C++ SPECIAL_BLACKLOTUS_STEAL_CASH_HACK:
                            // withdraw/deposit, scorekeeper money earned, EVA_CashStolen
                            // when victim local, GUI:AddCash/LoseCash floating texts.
                            let amount =
                                crate::game_logic::host_hero_abilities::STEAL_CASH_DEFAULT_AMOUNT;
                            let stolen = self.steal_cash_from_team(target_team, team, amount);
                            if stolen > 0 {
                                self.hero_abilities.record_cash_steal(stolen);
                                // C++ controller->getScoreKeeper()->addMoneyEarned(cash)
                                if let Some(p) = self.get_player_mut_by_team(team) {
                                    p.add_money_earned(stolen);
                                }
                                self.try_eva_cash_stolen(special_target_id);
                                self.spawn_sabotage_cash_floating_texts(
                                    object_id,
                                    special_target_id,
                                    stolen,
                                );
                                self.queue_audio_event(
                                    AudioEventRequest::new(
                                        crate::game_logic::host_hero_abilities::STEAL_CASH_AUDIO,
                                    )
                                    .with_object(object_id)
                                    .with_position(position)
                                    .with_priority(160),
                                );
                                let msg =
                                    localization::localize("hud.cash_hack.complete", "Cash stolen");
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::CarBomb { .. } => {
                            // C++ ConvertToCarBombCrateCollide residual:
                            // vehicle defects to converter team, gains IS_CARBOMB +
                            // SuicideCarBomb weapon residual. Converter is consumed.
                            // Detonation happens later when the car bomb attacks.
                            // Booby-trap residual: cancel if mine detonates and either dies.
                            let booby = self
                                .objects
                                .get(&special_target_id)
                                .map(|t| t.status.booby_trapped)
                                .unwrap_or(false);
                            if booby {
                                // Detonate trap residual damage on both.
                                if let Some(t) = self.objects.get_mut(&special_target_id) {
                                    let _ = t.take_damage_from(
                                        t.health.maximum.max(1.0),
                                        Some(object_id),
                                    );
                                }
                                if let Some(b) = self.objects.get_mut(&object_id) {
                                    let _ = b.take_damage_from(
                                        b.health.maximum.max(1.0),
                                        Some(special_target_id),
                                    );
                                }
                                let t_dead = self
                                    .objects
                                    .get(&special_target_id)
                                    .map(|t| !t.is_alive() || t.status.destroyed)
                                    .unwrap_or(true);
                                let b_dead = self
                                    .objects
                                    .get(&object_id)
                                    .map(|b| !b.is_alive() || b.status.destroyed)
                                    .unwrap_or(true);
                                if t_dead || b_dead {
                                    if t_dead {
                                        self.mark_object_for_destruction(
                                            special_target_id,
                                            Some(team),
                                        );
                                    }
                                    if b_dead {
                                        self.mark_object_for_destruction(object_id, Some(team));
                                    }
                                    continue;
                                }
                            }
                            // Snapshot donor residual (vision/vet) before consume.
                            let donor_snap = self.objects.get(&object_id).cloned();
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_convert_to_car_bomb_from(donor_snap.as_ref());
                            }
                            match owner_player_id {
                                Some(player_id) => {
                                    let _ = self
                                        .transfer_object_to_player(special_target_id, player_id);
                                }
                                None => {
                                    if let Some(target) = self.objects.get_mut(&special_target_id) {
                                        target.set_team(team);
                                    }
                                }
                            }
                            // C++ transferObjectName residual (script named object).
                            let _ = self.transfer_script_object_name(object_id, special_target_id);
                            self.car_bomb.record_conversion();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_car_bomb::CAR_BOMB_CONVERT_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
                            let msg = localization::localize(
                                "hud.carbomb.converted",
                                "Vehicle converted to car bomb",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            self.mark_destroyed_authority_aware(object_id, None);
                            self.mark_object_for_destruction(object_id, Some(team));
                        }
                        PendingSpecialAbility::DisableVehicleHack { .. } => {
                            // C++ SpecialAbilityUpdate BLACKLOTUS_DISABLE_VEHICLE_HACK:
                            // setDisabledUntil(DISABLED_HACKED, now + EffectDuration).
                            let until = self.frame.saturating_add(
                                crate::game_logic::host_hero_abilities::DISABLE_VEHICLE_HACK_DURATION_FRAMES,
                            );
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_disabled_hacked(until);
                            }
                            self.hero_abilities.record_vehicle_disable();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_hero_abilities::DISABLE_VEHICLE_HACK_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
                            let msg = localization::localize(
                                "hud.vehicle_hack.disabled",
                                "Vehicle disabled",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::HackerDisableBuilding { .. } => {
                            unreachable!("HDB is intercepted by its typed persistent channel")
                        }
                        PendingSpecialAbility::DisguiseAsVehicle { .. } => {
                            // C++ StealthUpdate::disguiseAsObject residual:
                            // if target already disguised, copy *its* disguise
                            // template + player; else copy target template + team.
                            // set OBJECT_STATUS_DISGUISED + STEALTHED.
                            let (tpl, as_team, copied_disguise) = self
                                .objects
                                .get(&special_target_id)
                                .map(|t| {
                                    if t.status.disguised {
                                        if let (Some(dt), Some(dteam)) =
                                            (t.disguise_as_template.as_ref(), t.disguise_as_team)
                                        {
                                            return (dt.clone(), dteam, true);
                                        }
                                    }
                                    (t.template_name.clone(), t.team, false)
                                })
                                .unwrap_or_else(|| {
                                    ("UnknownVehicle".to_string(), target_team, false)
                                });
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.apply_disguise(&tpl, as_team);
                                obj.stop_moving();
                                if crate::gameworld_shadow::gameworld_ai_decision_authority_enabled(
                                ) {
                                    crate::game_logic::host_ai_decision_log::record_stop_attack(
                                        object_id,
                                    );
                                    crate::game_logic::host_ai_decision_log::record_set_state(
                                        object_id, 0,
                                    );
                                } else {
                                    obj.set_target(None);
                                    obj.set_ai_state(AIState::Idle);
                                }
                            }
                            self.bomb_truck_disguise.record_disguise(object_id, &tpl);
                            self.bomb_truck_disguise.record_transition_start();
                            if copied_disguise {
                                self.bomb_truck_disguise.record_disguise_copy();
                            }
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_AUDIO,
                                )
                                .with_object(object_id)
                                .with_position(position)
                                .with_priority(160),
                            );
                            let msg = localization::localize(
                                "hud.bombtruck.disguised",
                                "Bomb truck disguised",
                            );
                            self.queue_radar_message_for_team(team, msg);
                        }
                        PendingSpecialAbility::PlantBoobyTrap { .. } => {
                            // C++ SpecialAbilityBoobyTrap residual: mark structure BOOBY_TRAPPED.
                            use crate::game_logic::host_booby_trap::{
                                has_booby_trap_upgrade, is_booby_trap_planter_template,
                                BOOBY_TRAP_INSTALL_AUDIO,
                            };
                            let (can_plant, ready) = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    let planter_ok =
                                        is_booby_trap_planter_template(&o.template_name)
                                            && has_booby_trap_upgrade(&o.applied_upgrades);
                                    let ready = self.booby_trap.plant_ready(object_id, self.frame);
                                    (planter_ok, ready)
                                })
                                .unwrap_or((false, false));
                            if can_plant
                                && ready
                                && self.booby_trap.can_place_special_object(object_id)
                            {
                                let geom = self
                                    .objects
                                    .get(&special_target_id)
                                    .map(|t| t.selection_radius.max(8.0))
                                    .unwrap_or(8.0);
                                let prev = self.booby_trap.install(
                                    special_target_id,
                                    object_id,
                                    team,
                                    self.frame,
                                    geom,
                                    None,
                                );
                                if let Some(prev_plant) = prev {
                                    if let Some(cid) = prev_plant.charge_object_id {
                                        self.destroy_booby_trap_special_object(cid);
                                    }
                                }
                                if let Some(cid) = self.spawn_booby_trap_special_object(
                                    object_id,
                                    team,
                                    special_target_id,
                                ) {
                                    self.booby_trap.set_charge_object(special_target_id, cid);
                                }
                                if let Some(target) = self.objects.get_mut(&special_target_id) {
                                    target.set_status_booby_trapped(true);
                                }
                                self.queue_audio_event(
                                    AudioEventRequest::new(BOOBY_TRAP_INSTALL_AUDIO)
                                        .with_object(special_target_id)
                                        .with_position(target_position)
                                        .with_priority(160),
                                );
                                let msg = localization::localize(
                                    "hud.booby_trap.planted",
                                    "Booby trap planted",
                                );
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                    }

                    self.pending_special_abilities.remove(&object_id);
                }
                AIState::Gathering => {
                    // Accumulate resources when close to the supply source.
                    const GATHER_RATE: f32 = 100.0;
                    const MAX_CARRY: u32 = 1000;

                    let Some(source_id) = target_id else {
                        self.set_ai_state_decision_aware(object_id, AIState::Idle);
                        continue;
                    };

                    // Extract source state before any mutations.
                    let (
                        source_alive,
                        source_pos,
                        source_supplies,
                        source_is_warehouse,
                        delete_when_empty,
                    ) = self
                        .objects
                        .get(&source_id)
                        .map(|s| {
                            (
                                s.is_alive(),
                                s.get_position(),
                                s.stored_resources.supplies,
                                s.thing.template.dock_kind
                                    == crate::game_logic::DockKind::SupplyWarehouse,
                                s.thing.template.dock_delete_when_empty,
                            )
                        })
                        .unwrap_or((false, position, 0, false, false));

                    if !source_alive {
                        // C++ supply truck residual: find another warehouse when pile empties.
                        if let Some(next) = self.find_nearest_harvestable_supply(team, position) {
                            if let Some(dest) = self.objects.get(&next).map(|s| s.get_position()) {
                                if let Some(obj) = self.objects.get_mut(&object_id) {
                                    obj.set_target(Some(next));
                                }
                                self.path_approach_with_state(object_id, dest, AIState::Gathering);
                                continue;
                            }
                        }
                        self.stop_attack_decision_aware(object_id);
                        self.set_ai_state_decision_aware(object_id, AIState::Idle);
                        continue;
                    }

                    if can_move && position.distance(source_pos) > INTERACT_RANGE {
                        self.path_approach_with_state(object_id, source_pos, AIState::Gathering);
                        continue;
                    }

                    // In range — gather resources.  The host tracks cash
                    // value rather than C++ individual boxes, but a warehouse
                    // still cannot grant more than its authored stock.  This
                    // avoids turning an empty `SupplyWarehouseDockUpdate`
                    // into an infinite source.
                    let gather_amount = (GATHER_RATE * dt) as u32;
                    // Keep the legacy generic-resource path intact.  The
                    // precise stock gate is required specifically for the
                    // newly-authored SupplyWarehouseDockUpdate target.
                    let taken = if source_is_warehouse {
                        gather_amount.min(source_supplies)
                    } else {
                        gather_amount
                    };
                    if source_is_warehouse && taken == 0 {
                        // C++ returns FALSE from SupplyWarehouseDockUpdate
                        // when its boxes are exhausted; it does not credit the
                        // docker.  Stop this explicit dock order rather than
                        // inventing more supply or treating it as Enter.
                        self.stop_attack_decision_aware(object_id);
                        self.set_ai_state_decision_aware(object_id, AIState::Idle);
                        continue;
                    }
                    let is_full = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.stored_resources.supplies)
                        .unwrap_or(0)
                        + taken
                        >= MAX_CARRY;

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_stored_supplies(
                            obj.stored_resources
                                .supplies
                                .saturating_add(taken)
                                .min(MAX_CARRY),
                        );
                    }

                    // Deplete the supply source.
                    if let Some(source) = self.objects.get_mut(&source_id) {
                        source.set_stored_supplies(
                            source.stored_resources.supplies.saturating_sub(taken),
                        );
                        if source.stored_resources.supplies == 0
                            && (!source_is_warehouse || delete_when_empty)
                        {
                            Self::mark_object_destroyed_authority_aware(source, None);
                            self.mark_object_for_destruction(source_id, None);
                        }
                    }

                    if is_full {
                        // Full — `SupplyTruckAIUpdate::m_preferredDock` wins
                        // over ResourceManager's nearest-center search when
                        // AI assigned this collector to a specific depot.
                        let refinery_dest = self
                            .preferred_supply_center_or_nearest(object_id, team, position)
                            .and_then(|rid| self.objects.get(&rid).map(|r| r.get_position()));
                        if let Some(dest) = refinery_dest {
                            self.path_approach_with_state(
                                object_id,
                                dest,
                                AIState::ReturningResources,
                            );
                        }
                    }
                }
                AIState::ReturningResources => {
                    // Deposit resources when close to a supply center.
                    let (refinery_id, refinery_pos) = self
                        .preferred_supply_center_or_nearest(object_id, team, position)
                        .and_then(|rid| {
                            self.objects
                                .get(&rid)
                                .map(|r| (Some(rid), r.get_position()))
                        })
                        .unwrap_or((None, position));

                    let at_refinery =
                        refinery_id.is_some() && position.distance(refinery_pos) <= INTERACT_RANGE;

                    if at_refinery {
                        // Deposit.
                        // C++ SupplyCenterDockUpdate::action: base box value +
                        // supplyTruckAI->getUpgradedSupplyBoost() when player has
                        // Upgrade_AmericaSupplyLines (Chinook residual).
                        let deposit_amount = self
                            .objects
                            .get(&object_id)
                            .map(|o| o.stored_resources.supplies)
                            .unwrap_or(0);

                        if deposit_amount > 0 {
                            // Snapshot carrier for residual boost identity (worker shoes).
                            let (
                                carrier_is_gla_worker,
                                carrier_has_worker_shoes,
                            ) = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    let is_w = crate::game_logic::host_gla_worker::is_gla_worker_template(
                                        &o.template_name,
                                    );
                                    let shoes = o.has_upgrade_tag(
                                        crate::game_logic::host_gla_worker::UPGRADE_GLA_WORKER_SHOES,
                                    ) || self.players.values().any(|p| {
                                        p.team == team
                                            && p.has_unlocked_upgrade(
                                                crate::game_logic::host_gla_worker::UPGRADE_GLA_WORKER_SHOES,
                                            )
                                    });
                                    (is_w, shoes)
                                })
                                .unwrap_or((false, false));

                            // Clear carried resources.
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_stored_supplies(0);
                            }
                            // Player-level Supply Lines residual boost (flat per drop-off).
                            let has_supply_lines = self
                                .players
                                .values()
                                .any(|p| {
                                    p.team == team
                                        && p.has_unlocked_upgrade(
                                            crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES,
                                        )
                                });
                            let supply_lines_boost =
                                crate::game_logic::host_upgrades::residual_supply_lines_drop_off_boost(
                                    has_supply_lines,
                                );
                            // GLA WorkerShoes residual: +8 per drop-off when unlocked.
                            let worker_shoes_boost =
                                crate::game_logic::host_gla_worker::residual_worker_shoes_drop_off_boost(
                                    carrier_is_gla_worker,
                                    carrier_has_worker_shoes,
                                );
                            let boost = supply_lines_boost.saturating_add(worker_shoes_boost);
                            let credited = deposit_amount.saturating_add(boost);
                            // Credit the player (carried supplies + optional economy boost).
                            // Capture the concrete owner before the mutable
                            // credit so the typed event below is tied to this
                            // real ReturningResources deposit, not a later
                            // resource-total observation or passive income.
                            let credited_player_id =
                                self.players.iter().find_map(|(&player_id, player)| {
                                    (player.team == team).then_some(player_id)
                                });
                            if let Some(player_id) = credited_player_id {
                                let credited_player =
                                    if let Some(player) = self.get_player_mut(player_id) {
                                        player.credit_supplies(credited);
                                        true
                                    } else {
                                        false
                                    };
                                if credited_player {
                                    self.record_supply_dropoff_event(
                                        crate::game_logic::SupplyDropoffEvent {
                                            carrier_id: object_id,
                                            player_id,
                                            carried_amount: deposit_amount,
                                        },
                                    );
                                }
                            }
                            if supply_lines_boost > 0 {
                                self.supply_lines_bonus_cash_total = self
                                    .supply_lines_bonus_cash_total
                                    .saturating_add(supply_lines_boost);
                            }
                            if worker_shoes_boost > 0 {
                                self.gla_worker
                                    .record_shoes_drop_off_boost(worker_shoes_boost);
                            }
                            // Head back to gather more from the original source.
                            let source_dest = target_id.and_then(|sid| {
                                self.objects
                                    .get(&sid)
                                    .filter(|s| s.is_alive())
                                    .map(|s| s.get_position())
                            });
                            if let Some(dest) = source_dest {
                                self.path_approach_with_state(object_id, dest, AIState::Gathering);
                            } else if let Some(next) =
                                self.find_nearest_harvestable_supply(team, position)
                            {
                                if let Some(dest) =
                                    self.objects.get(&next).map(|s| s.get_position())
                                {
                                    if let Some(obj) = self.objects.get_mut(&object_id) {
                                        obj.set_target(Some(next));
                                    }
                                    self.path_approach_with_state(
                                        object_id,
                                        dest,
                                        AIState::Gathering,
                                    );
                                }
                            } else {
                                self.stop_attack_decision_aware(object_id);
                                self.set_ai_state_decision_aware(object_id, AIState::Idle);
                            }
                        }
                    } else if can_move {
                        // Still heading to refinery.
                        self.path_approach_with_state(
                            object_id,
                            refinery_pos,
                            AIState::ReturningResources,
                        );
                    }
                }
                AIState::Docked | AIState::Garrisoned => {
                    // Aircraft parking: leave hangar when given a move/attack residual.
                    let wants_sortie = self
                        .objects
                        .get(&object_id)
                        .map(|o| {
                            (o.is_kind_of(KindOf::Aircraft)
                                || o.object_type == ObjectType::Aircraft)
                                && (o.movement.target_position.is_some()
                                    || o.target.is_some()
                                    || o.target_location.is_some())
                        })
                        .unwrap_or(false);
                    if wants_sortie {
                        self.release_jet_from_airfield_parking(object_id);
                        continue;
                    }
                    // Prefer contained_by (authoritative residual link) over target.
                    let container_id = self
                        .objects
                        .get(&object_id)
                        .and_then(|o| o.container_id())
                        .or(target_id);
                    let Some(container_id) = container_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_contained_by(None);
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let Some((container_pos, container_alive, container_has_unit)) =
                        self.objects.get(&container_id).map(|container| {
                            (
                                container.get_position(),
                                container.is_alive(),
                                container.contained_units().contains(&object_id),
                            )
                        })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_contained_by(None);
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !container_alive || !container_has_unit {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_contained_by(None);
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_contained_by(Some(container_id));
                        obj.set_position(container_pos);
                        crate::game_logic::host_ground_height_log::record(
                            obj.id,
                            container_pos.y,
                            false,
                        );
                        if crate::gameworld_shadow::gameworld_movement_authority_live() {
                            crate::game_logic::host_move_log::record(
                                obj.id,
                                Some([container_pos.x, container_pos.y, container_pos.z]),
                            );
                            obj.record_host_movement();
                        }
                        obj.stop_moving();
                        obj.set_status_moving(false);
                    }
                }
                _ => {}
            }
        }

        // C++ RiderChangeContain::update: after an ordinary rider exit, the
        // bike remains as an unselectable toppled shell for ScuttleDelay, then
        // dies with DEATH_TOPPLED.  Replacement keeps m_containing=true and
        // therefore never reaches this list.
        let scuttles_due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(&id, object)| {
                let delay = object
                    .thing
                    .template
                    .contain_module
                    .rider_change_scuttle_delay_frames?;
                let started = object.rider_change_scuttled_on_frame;
                (object.thing.template.contain_module.kind
                    == crate::game_logic::ContainModuleKind::RiderChange
                    && started != 0
                    && !object.status.destroyed
                    && self.frame >= started.saturating_add(delay))
                .then_some(id)
            })
            .collect();
        for object_id in scuttles_due {
            if let Some(object) = self.objects.get_mut(&object_id) {
                object.status.death_type =
                    crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
            }
            self.mark_destroyed_authority_aware(object_id, None);
            self.mark_object_for_destruction(object_id, None);
        }
    }
}
