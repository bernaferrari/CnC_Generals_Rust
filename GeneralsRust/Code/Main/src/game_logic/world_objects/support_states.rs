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
    let _ = container.replace_weapon_set_slot(0, None);
    let _ = container.replace_weapon_set_slot(1, None);
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
    crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(container, binding);
}

enum LeftoverSaTick {
    Waiting,
    Trigger,
    Finished,
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
        self.hero_abilities.clear_capture_flash(object_id);
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
        self.hero_abilities.clear_capture_flash(object_id);
    }

    /// C++ `onExit` when a player/script order replaces capture.
    /// Do not stop a newly issued move — only drop the stale channel.
    fn abort_capture_channel_on_new_order(&mut self, object_id: ObjectId) {
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.capture_channel = None;
            object.set_status_using_ability(false);
        }
        self.hero_abilities.clear_capture_flash(object_id);
    }

    /// C++ `SpecialAbilityUpdate::triggerAbilityEffect` AwardXPForTriggering
    /// (`SpecialAbilityUpdate.cpp:1248-1253`) plus skill-points fallback
    /// (`:1256-1264`). SkillPointsForTriggering defaults to -1, so retail
    /// uses the same AwardXP integer.
    fn award_ability_trigger_experience(&mut self, object_id: ObjectId, award_xp: i32) {
        if award_xp <= 0 {
            return;
        }
        let (owner_player_id, team) = match self.objects.get(&object_id) {
            Some(object) => (object.owner_player_id, object.team),
            None => return,
        };
        self.award_experience(object_id, award_xp as f32);
        let player_id = owner_player_id.or_else(|| self.player_id_for_team(team));
        if let Some(player) = player_id.and_then(|id| self.get_player_mut(id)) {
            let _ = player.add_skill_points(award_xp);
        }
    }

    /// Retail Object INI `AwardXPForTriggering` for the four capture powers.
    fn award_xp_for_capture_trigger(kind: crate::game_logic::CapturePowerKind) -> i32 {
        use crate::game_logic::CapturePowerKind;
        match kind {
            CapturePowerKind::Ranger | CapturePowerKind::RedGuard => {
                crate::game_logic::host_structure_economy_residual::CAPTURE_AWARD_XP as i32
            }
            CapturePowerKind::Rebel => {
                crate::game_logic::host_gla_rebel::REBEL_CAPTURE_AWARD_XP as i32
            }
            CapturePowerKind::BlackLotus => {
                crate::game_logic::host_hero_abilities::LOTUS_CAPTURE_AWARD_XP as i32
            }
            CapturePowerKind::None => 0,
        }
    }

    fn leftover_sa_kind(
        ability: PendingSpecialAbility,
    ) -> Option<crate::game_logic::host_hero_abilities::LeftoverSaKind> {
        use crate::game_logic::host_hero_abilities::LeftoverSaKind;
        match ability {
            PendingSpecialAbility::StealCashHack { .. } => Some(LeftoverSaKind::StealCash),
            PendingSpecialAbility::DisableVehicleHack { .. } => {
                Some(LeftoverSaKind::DisableVehicle)
            }
            PendingSpecialAbility::PlantTimedDemoCharge { .. } => Some(LeftoverSaKind::PlantTimed),
            PendingSpecialAbility::PlantRemoteDemoCharge { .. } => {
                Some(LeftoverSaKind::PlantRemote)
            }
            _ => None,
        }
    }

    fn leftover_timings_for(
        &self,
        object_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
    ) -> (
        crate::game_logic::host_hero_abilities::LeftoverSaTimings,
        f32,
    ) {
        use crate::game_logic::host_hero_abilities::{leftover_sa_timings, LeftoverSaKind};
        let mut timings = leftover_sa_timings(kind);
        let mut variation = 0.0;
        if matches!(
            kind,
            LeftoverSaKind::PlantTimed | LeftoverSaKind::PlantRemote
        ) {
            let meta = self.objects.get(&object_id).and_then(|object| {
                if matches!(kind, LeftoverSaKind::PlantTimed) {
                    object.thing.template.charge_plant_ability_for_timed()
                } else {
                    object.thing.template.charge_plant_ability_for_remote()
                }
            });
            if let Some(meta) = meta {
                timings.unpack_ms = meta.unpack_time_ms;
                timings.pack_ms = meta.pack_time_ms;
                timings.flee_range = meta.flee_range_after_completion;
                timings.flip_after_unpack = meta.flip_object_after_unpacking;
                variation = meta.pack_unpack_variation_factor;
            } else {
                // C++ skips unpack/flee when the module is absent / UnpackTime is 0.
                timings.unpack_ms = 0;
                timings.pack_ms = 0;
                timings.flee_range = 0.0;
                timings.flip_after_unpack = false;
            }
        }
        (timings, variation)
    }


    fn abort_leftover_sa_channel_on_new_order(&mut self, object_id: ObjectId) {
        self.hero_abilities.take_leftover_channel(object_id);
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.set_status_using_ability(false);
        }
    }

    fn leftover_sa_within_start_range(
        &self,
        position: glam::Vec3,
        selection_radius: f32,
        target_position: glam::Vec3,
        target_radius: f32,
        start_range: f32,
    ) -> bool {
        let edge = crate::game_logic::host_hero_abilities::leftover_bounding_sphere_2d(
            position,
            selection_radius,
            target_position,
            target_radius,
        );
        crate::game_logic::host_hero_abilities::leftover_within_start_ability_range(
            edge,
            start_range,
        )
    }

    fn leftover_probe_booby_at_target(
        &mut self,
        planter_id: ObjectId,
        target_id: ObjectId,
        planter_team: crate::game_logic::Team,
    ) -> bool {
        let trap_position = self.objects.get(&target_id).map(|t| t.get_position());
        let planter_ally = self
            .booby_trap
            .plant(target_id)
            .map(|plant| plant.planter_team == planter_team)
            .unwrap_or(false);
        let target_is_trapped = self.booby_trap.is_booby_trapped(target_id)
            || self
                .objects
                .get(&target_id)
                .map(|t| t.status.booby_trapped)
                .unwrap_or(false);
        if planter_ally || !target_is_trapped {
            return false;
        }
        if let Some(trap_position) = trap_position {
            let _ = self.detonate_booby_trap_at(
                target_id,
                trap_position,
                Some(planter_id),
                true,
                false,
            );
        }
        let target_dead = self
            .objects
            .get(&target_id)
            .map(|t| !t.is_alive() || t.status.destroyed)
            .unwrap_or(true);
        let planter_dead = self
            .objects
            .get(&planter_id)
            .map(|p| !p.is_alive() || p.status.destroyed)
            .unwrap_or(true);
        target_dead || planter_dead
    }

    fn leftover_nearest_own_mine(
        &self,
        dest: glam::Vec3,
        team: crate::game_logic::Team,
        flee_range: f32,
    ) -> Option<glam::Vec3> {
        let mut best: Option<(f32, glam::Vec3)> = None;
        for obj in self.objects.values() {
            if obj.team != team || !obj.is_kind_of(KindOf::Mine) {
                continue;
            }
            let p = obj.get_position();
            let d = crate::game_logic::host_hero_abilities::horizontal_distance(dest, p);
            if d > flee_range {
                continue;
            }
            if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, p));
            }
        }
        best.map(|(_, p)| p)
    }

    fn leftover_start_sa_preparation(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
        prep_ms: u32,
    ) -> bool {
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::host_hero_abilities::{LeftoverSaChannel, LeftoverSaKind, LeftoverSaPhase};
        let power = match kind {
            LeftoverSaKind::StealCash => Some(SpecialPowerType::BlackLotusStealCash),
            LeftoverSaKind::DisableVehicle => Some(SpecialPowerType::BlackLotusDisableVehicle),
            _ => None,
        };
        if let Some(power) = power {
            if !self.consume_special_power_charge_for(object_id, &power) {
                return false;
            }
        }
        if let Some(object) = self.objects.get_mut(&object_id) {
            if !object.is_alive() {
                return false;
            }
            object.set_ai_state(AIState::SpecialAbility);
            object.set_status_using_ability(true);
        } else {
            return false;
        }
        self.hero_abilities.set_leftover_channel(
            object_id,
            LeftoverSaChannel::new(kind, target_id, LeftoverSaPhase::Preparing, prep_ms),
        );
        if matches!(kind, LeftoverSaKind::StealCash | LeftoverSaKind::DisableVehicle) {
            self.queue_audio_event(
                AudioEventRequest::new(
                    crate::game_logic::host_hero_abilities::LOTUS_PREP_SOUND_LOOP,
                )
                .with_object(object_id)
                .with_priority(140),
            );
        }
        true
    }

    fn leftover_flee_after_plant(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        team: crate::game_logic::Team,
        flee_range: f32,
        flip_after_unpack: bool,
    ) {
        let (pos, dir) = match self.objects.get(&object_id) {
            Some(obj) => (obj.get_position(), obj.unit_direction_xz()),
            None => return,
        };
        let dest_guess = crate::game_logic::host_hero_abilities::leftover_flee_position(
            pos,
            dir,
            flee_range,
            flip_after_unpack,
            None,
        );
        let mine = self.leftover_nearest_own_mine(dest_guess, team, flee_range);
        let dest = crate::game_logic::host_hero_abilities::leftover_flee_position(
            pos,
            dir,
            flee_range,
            flip_after_unpack,
            mine,
        );
        self.hero_abilities.take_leftover_channel(object_id);
        self.pending_special_abilities.remove(&object_id);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.set_status_using_ability(false);
        }
        self.path_approach_with_state(object_id, dest, AIState::Moving);
        let _ = target_id;
    }

    fn tick_leftover_special_ability(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
        dt: f32,
    ) -> LeftoverSaTick {
        use crate::game_logic::host_hero_abilities::{
            leftover_should_unstealth_during_unpack, LeftoverSaChannel, LeftoverSaPhase,
        };
        const EPS: f32 = 0.000_1;
        let (timings, variation) = self.leftover_timings_for(object_id, kind);
        let channel = self.hero_abilities.leftover_channel(object_id).copied();
        match channel {
            None => {
                let unpack_ms =
                    crate::game_logic::vary_pack_unpack_duration_ms(timings.unpack_ms, variation);
                if unpack_ms > 0 {
                    self.hero_abilities.set_leftover_channel(
                        object_id,
                        LeftoverSaChannel::new(
                            kind,
                            target_id,
                            LeftoverSaPhase::Unpacking,
                            unpack_ms,
                        ),
                    );
                    return LeftoverSaTick::Waiting;
                }
                if !self.leftover_start_sa_preparation(
                    object_id,
                    target_id,
                    kind,
                    timings.prep_ms,
                ) {
                    self.abort_leftover_sa_channel_on_new_order(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    return LeftoverSaTick::Finished;
                }
                if timings.prep_ms == 0 {
                    LeftoverSaTick::Trigger
                } else {
                    LeftoverSaTick::Waiting
                }
            }
            Some(mut channel) if channel.phase == LeftoverSaPhase::Unpacking => {
                channel.remaining_seconds = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if timings.pre_trigger_unstealth_ms > 0
                    && leftover_should_unstealth_during_unpack(
                        channel.remaining_seconds,
                        timings.pre_trigger_unstealth_ms,
                    )
                    && !channel.unstealthed
                {
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_status_detected(true);
                    }
                    channel.unstealthed = true;
                }
                if channel.remaining_seconds > EPS {
                    self.hero_abilities.set_leftover_channel(object_id, channel);
                    return LeftoverSaTick::Waiting;
                }
                if !self.leftover_start_sa_preparation(
                    object_id,
                    target_id,
                    kind,
                    timings.prep_ms,
                ) {
                    self.abort_leftover_sa_channel_on_new_order(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    return LeftoverSaTick::Finished;
                }
                if timings.prep_ms == 0 {
                    LeftoverSaTick::Trigger
                } else {
                    LeftoverSaTick::Waiting
                }
            }
            Some(channel) if channel.phase == LeftoverSaPhase::Preparing => {
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if remaining > EPS {
                    self.hero_abilities.set_leftover_channel(
                        object_id,
                        LeftoverSaChannel {
                            remaining_seconds: remaining,
                            ..channel
                        },
                    );
                    LeftoverSaTick::Waiting
                } else {
                    LeftoverSaTick::Trigger
                }
            }
            Some(channel) if channel.phase == LeftoverSaPhase::Packing => {
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if remaining > EPS {
                    self.hero_abilities.set_leftover_channel(
                        object_id,
                        LeftoverSaChannel {
                            remaining_seconds: remaining,
                            ..channel
                        },
                    );
                    LeftoverSaTick::Waiting
                } else {
                    self.hero_abilities.take_leftover_channel(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.set_status_using_ability(false);
                        obj.set_target(None);
                        obj.set_ai_state(AIState::Idle);
                    }
                    LeftoverSaTick::Finished
                }
            }
            Some(_) => LeftoverSaTick::Finished,
        }
    }

    fn leftover_begin_packing(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
        pack_ms: u32,
    ) {
        use crate::game_logic::host_hero_abilities::{LeftoverSaChannel, LeftoverSaPhase};
        if pack_ms == 0 {
            self.hero_abilities.take_leftover_channel(object_id);
            self.pending_special_abilities.remove(&object_id);
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.stop_moving();
                obj.set_status_using_ability(false);
                obj.set_target(None);
                obj.set_ai_state(AIState::Idle);
            }
            return;
        }
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.stop_moving();
            obj.set_status_using_ability(false);
            obj.set_ai_state(AIState::SpecialAbility);
        }
        self.hero_abilities.set_leftover_channel(
            object_id,
            LeftoverSaChannel::new(kind, target_id, LeftoverSaPhase::Packing, pack_ms),
        );
    }


    fn apply_leftover_capture_fx(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        remaining_seconds: f32,
        preparation_time_ms: u32,
    ) {
        if !crate::game_logic::host_hero_abilities::LOTUS_CAPTURE_DO_CAPTURE_FX {
            return;
        }
        let phase = self
            .hero_abilities
            .capture_flash_phase
            .entry(object_id)
            .or_insert(0.0);
        if crate::game_logic::host_hero_abilities::leftover_capture_fx_should_flash(
            phase,
            remaining_seconds,
            preparation_time_ms,
        ) {
            if let Some(target) = self.objects.get_mut(&target_id) {
                target.flash_as_selected();
            }
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
        let pack_time_ms = self
            .objects
            .get(&object_id)
            .and_then(|object| object.thing.template.hacker_disable_building.as_ref())
            .map(|meta| {
                crate::game_logic::vary_pack_unpack_duration_ms(
                    pack_time_ms,
                    meta.pack_unpack_variation_factor,
                )
            })
            .unwrap_or(pack_time_ms);
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
                    } else {
                        let unpack_ms = crate::game_logic::vary_pack_unpack_duration_ms(
                            metadata.unpack_time_ms,
                            metadata.pack_unpack_variation_factor,
                        );
                        if let Some(object) = self.objects.get_mut(&object_id) {
                            object.stop_moving();
                            object.hacker_disable_channel =
                                Some(crate::game_logic::HackerDisableChannelState::new(
                                    channel.target_id,
                                    crate::game_logic::HackerDisableChannelPhase::Unpacking,
                                    unpack_ms,
                                ));
                            object.set_status_using_ability(false);
                            object.set_ai_state(AIState::SpecialAbility);
                        }
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
        self.update_leftover_laser_guided_channels(dt);

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
            self.expire_temporary_stealth_grant(object_id);


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
                self.abort_leftover_sa_channel_on_new_order(object_id);
            }
            // C++ SpecialAbilityUpdate::update: any non-AI command source
            // immediately onExit. Leftover capture must not keep
            // IS_USING_ABILITY / capture_channel after a player move.
            if ai_state != AIState::Capturing {
                let has_capture = self
                    .objects
                    .get(&object_id)
                    .is_some_and(|o| o.capture_channel.is_some());
                if has_capture {
                    self.abort_capture_channel_on_new_order(object_id);
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
                        let tunnel_nemesis = {
                            let guard_is_tunnel = self.objects.get(&guard_target_id).is_some_and(
                                |g| {
                                    g.is_tunnel_network_style_container()
                                        || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                                            &g.template_name,
                                        )
                                },
                            );
                            if guard_is_tunnel {
                                self.resolved_tunnel_nemesis(team)
                            } else {
                                None
                            }
                        };
                        if let Some(enemy_id) = tunnel_nemesis {
                            if self.engage_target_decision_aware(object_id, enemy_id) {
                                continue;
                            }
                        }
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

                    let interact = crate::game_logic::host_repair::repair_action_range(
                        repair_target_selection_radius,
                    );
                    if position.distance(repair_target_pos) > interact {
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
                                        endpoint.distance(repair_target_pos) <= interact
                                    })
                            });
                        if can_move && !has_valid_active_approach_path {
                            let approach =
                                crate::game_logic::host_repair::dozer_repair_approach_position(
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
                    // C++ DozerAIUpdate.cpp:694-699 percent heal, no 8.75 HP/s floor.
                    // C++ DozerAIUpdate.cpp:670: ACTIVELY_CONSTRUCTING only at the dock.
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_actively_constructing(true);
                    }
                    let max_hp = self
                        .objects
                        .get(&repair_target_id)
                        .map(|t| t.health.maximum)
                        .unwrap_or(0.0);
                    let heal_per_sec =
                        crate::game_logic::host_repair::dozer_repair_hp_per_sec(max_hp);
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
                            self.dozer_internal_task_complete(object_id, true);
                            let _ = self.dozer_idle_resume_pending_build(object_id);
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
                        self.dozer_internal_task_complete(object_id, true);
                        let _ = self.dozer_idle_resume_pending_build(object_id);
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
                        self.dozer_internal_task_complete(object_id, true);
                        let _ = self.dozer_idle_resume_pending_build(object_id);
                    }
                }
                state @ (AIState::SeekingRepair | AIState::SeekingHealing) => {
                    if health_current >= health_maximum - 0.01 {
                        if let Some(tid) = target_id {
                            if matches!(state, AIState::SeekingRepair) {
                                self.send_to_rally_after_repair_dock(object_id, tid);
                            }
                            self.release_dock_if_holder(tid, object_id);
                        }
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
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
                        if matches!(state, AIState::SeekingRepair) {
                            self.release_dock_if_holder(support_target_id, object_id);
                        }

                        continue;
                    }

                    // Pad/airfield/war-factory: C++ RepairDockUpdate::action
                    // TimeForFullHeal. One activeDocker; rate computed once
                    // from missing HP so Humvee ≠ Overlord.
                    if matches!(state, AIState::SeekingRepair)
                        && !self.try_claim_dock(support_target_id, object_id)
                    {
                        continue;
                    }
                    let mut vehicle_healed = false;
                    let mut heal_pad_healed = false;
                    let repair_rate = if matches!(state, AIState::SeekingRepair) {
                        self.repair_dock_rate_for_docker(
                            support_target_id,
                            object_id,
                            health_maximum,
                            health_current,
                        )
                    } else {
                        0.0
                    };
                    let seeking_repair = matches!(state, AIState::SeekingRepair);
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        let rate = match state {
                            AIState::SeekingRepair => repair_rate,
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
                        } else if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            let ordinal =
                                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                                    &state,
                                );
                            crate::game_logic::host_ai_decision_log::record_set_state(
                                object_id, ordinal,
                            );
                            obj.set_ai_state(state);
                        } else {
                            obj.set_ai_state(state);
                        }
                    }
                    let fully_repaired = self
                        .objects
                        .get(&object_id)
                        .is_some_and(|o| o.health.current >= o.health.maximum - 0.01);
                    if seeking_repair && fully_repaired {
                        self.send_to_rally_after_repair_dock(object_id, support_target_id);
                        self.release_dock_if_holder(support_target_id, object_id);
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
                        .map(|o| {
                            (o.is_kind_of(KindOf::Infantry) || o.is_hero())
                                && !o.is_kind_of(KindOf::NoGarrison)
                        })
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

                    // C++ ChinookAIUpdate::getAiFreeToExit — WAIT_TO_EXIT while flying.
                    if container_is_combat_chinook {
                        let allow = self.objects.get_mut(&container_id).is_some_and(|c| {
                            let p = c.get_position();
                            let moving = c.status.moving;
                            if let Some(ai) = c.chinook_ai.as_mut() {
                                ai.pos = [p.x, p.z, p.y];
                                ai.wanting_enter_or_exit = true;
                                ai.parent_idle = !moving;
                                ai.tick_idle_auto_land();
                                ai.ai_free_to_exit(false)
                                    == crate::game_logic::host_combat_chinook::HostChinookFreeToExit::FreeToExit
                            } else {
                                true
                            }
                        });
                        if !allow {
                            continue;
                        }
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

                    let container_is_heal_contain = self.objects.get(&container_id).is_some_and(
                        |c| c.thing.template.contain_module.kind.is_heal_contain(),
                    );
                    self.tunnel_network
                        .stamp_contained_by_frame(object_id, self.frame);

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.set_status_attacking(false);
                        obj.target_location = None;
                        obj.set_status_force_attack(false);
                        obj.target = Some(container_id);
                        obj.set_contained_by(Some(container_id));
                        if container_is_overlord_bunker {
                            // C++ OverlordContain::onContaining ExperienceSinkForRider
                            // (`OverlordContain.cpp:354-355`, default TRUE). Live
                            // BattleBunker infantry are the rider analog — bunker
                            // kills must level the tank, not the occupant.
                            obj.set_experience_sink(Some(container_id));
                        }
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
                        let __ai_st = if container_is_heal_contain {
                            // C++ HealContain is not garrisonable; Docked avoids garrison fire.
                            AIState::Docked
                        } else if container_is_structure {
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
                    } else if container_is_heal_contain {
                        // C++ HealContain is not a garrison / transport load.
                    } else if container_is_structure {
                        self.record_garrison_residual_enter();
                        self.apply_garrison_contain_on_enter(container_id, object_id);
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
                                let factor = self
                                    .objects
                                    .get(&object_id)
                                    .map(|object| {
                                        object.thing.template.capture_pack_unpack_variation_factor
                                    })
                                    .unwrap_or(0.0);
                                let unpack_time_ms = crate::game_logic::vary_pack_unpack_duration_ms(
                                    unpack_time_ms,
                                    factor,
                                );
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
                                self.apply_leftover_capture_fx(
                                    object_id,
                                    capture_target_id,
                                    remaining,
                                    preparation_time_ms,
                                );
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

                    // C++ `triggerAbilityEffect` awards XP before the capture
                    // switch (`SpecialAbilityUpdate.cpp:1248-1253`), including
                    // garrison-evac triggers that do not defect the building.
                    self.award_ability_trigger_experience(
                        object_id,
                        Self::award_xp_for_capture_trigger(capture_power),
                    );

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
                                // C++ capture uses Object::defect (SpecialAbilityUpdate.cpp:1442).
                                // defect cancelAndRefundAllProduction (Object.cpp:6136-6139)
                                // before setTeam; onCapture (Object.cpp:4509) then keeps the
                                // emptied ProductionUpdate module. Do not transfer the queue.
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
                                    crate::game_logic::vary_pack_unpack_duration_ms(
                                        pack_time_ms,
                                        object.thing.template.capture_pack_unpack_variation_factor,
                                    ),
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


                    // C++ SpecialAbilityDisguiseAsVehicle StartAbilityRange = 1e6
                    // residual: complete without approach walk.
                    let disguise_instant =
                        matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. });
                    let black_lotus_range = matches!(
                        ability,
                        PendingSpecialAbility::StealCashHack { .. }
                            | PendingSpecialAbility::DisableVehicleHack { .. }
                    );
                    let booby_trap_range =
                        matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. });
                    let plant_range = matches!(
                        ability,
                        PendingSpecialAbility::PlantTimedDemoCharge { .. }
                            | PendingSpecialAbility::PlantRemoteDemoCharge { .. }
                    );
                    let leftover_busy = self.hero_abilities.leftover_channel(object_id).is_some();
                    let out_of_start_range = if plant_range {
                        !self.leftover_sa_within_start_range(
                            position,
                            selection_radius,
                            target_position,
                            target_radius,
                            crate::game_logic::host_hero_abilities::PLANT_START_ABILITY_RANGE,
                        )
                    } else if black_lotus_range {
                        position.distance(target_position)
                            > crate::game_logic::host_hero_abilities::BLACK_LOTUS_START_ABILITY_RANGE
                    } else if booby_trap_range {
                        position.distance(target_position)
                            > crate::game_logic::host_booby_trap::BOOBY_START_ABILITY_RANGE
                                + selection_radius
                                + target_radius
                    } else {
                        position.distance(target_position)
                            > selection_radius + target_radius + SPECIAL_ABILITY_RANGE_PADDING
                    };
                    if !leftover_busy
                        && !disguise_instant
                        && can_move
                        && out_of_start_range
                    {
                        self.path_approach_with_state(
                            object_id,
                            target_position,
                            AIState::SpecialAbility,
                        );
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

                    // Disable vehicle hack: unmanned still matches ActionManager.
                    // Already-hacked is legal — C++ triggerAbilityEffect refreshes.
                    if matches!(ability, PendingSpecialAbility::DisableVehicleHack { .. })
                        && target_is_unmanned
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

                    if let Some(kind) = Self::leftover_sa_kind(ability) {
                        match self.tick_leftover_special_ability(
                            object_id,
                            special_target_id,
                            kind,
                            dt,
                        ) {
                            LeftoverSaTick::Waiting | LeftoverSaTick::Finished => continue,
                            LeftoverSaTick::Trigger => {}
                        }
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
                                    t.is_kind_of(KindOf::FSSupplyDropzone),
                                    crate::game_logic::host_saboteur::is_aircraft_carrier_template(
                                        &t.template_name,
                                    ),
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
                                            // C++ SabotagePowerPlantCrateCollide.cpp:112-120
                                            // other->getControllingPlayer(), never first same-faction slot.
                                            let victim_id = self
                                                .objects
                                                .get(&special_target_id)
                                                .and_then(|target| {
                                                    self.player_owner_for_host_object(target)
                                                });
                                            if let Some(player) = victim_id
                                                .and_then(|id| self.players.get_mut(&id))
                                            {
                                                player.power_sabotaged_till_frame = until;
                                            }
                                        }
                                        SaboteurEffectKind::SupplyCenter
                                        | SaboteurEffectKind::SupplyDropzone => {
                                            if matches!(
                                                kind,
                                                SaboteurEffectKind::SupplyDropzone
                                            ) {
                                                // C++ OCLUpdate::resetTimer
                                                // (SabotageSupplyDropzoneCrateCollide.cpp:112-117).
                                                self.supply_drop_zones.reset_timer(
                                                    special_target_id,
                                                    self.frame,
                                                );
                                            }
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
                            // C++ triggerAbilityEffect checkAndDetonateBoobyTrap before plant.
                            if self.leftover_probe_booby_at_target(
                                object_id,
                                special_target_id,
                                team,
                            ) {
                                self.hero_abilities.take_leftover_channel(object_id);
                                self.pending_special_abilities.remove(&object_id);
                                continue;
                            }
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
                            if self.leftover_probe_booby_at_target(
                                object_id,
                                special_target_id,
                                team,
                            ) {
                                self.hero_abilities.take_leftover_channel(object_id);
                                self.pending_special_abilities.remove(&object_id);
                                continue;
                            }
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
                            // C++ `triggerAbilityEffect` AwardXPForTriggering
                            // (`SpecialAbilityUpdate.cpp:1248-1253`). Retail
                            // `SpecialAbilityBlackLotusStealCashHack` = 20.
                            self.award_ability_trigger_experience(
                                object_id,
                                crate::game_logic::host_hero_abilities::LOTUS_STEAL_AWARD_XP as i32,
                            );
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

                    if let Some(kind) = Self::leftover_sa_kind(ability) {
                        let (timings, variation) = self.leftover_timings_for(object_id, kind);
                        if timings.flee_range > 0.0 {
                            self.leftover_flee_after_plant(
                                object_id,
                                special_target_id,
                                team,
                                timings.flee_range,
                                timings.flip_after_unpack,
                            );
                        } else {
                            let pack_ms = crate::game_logic::vary_pack_unpack_duration_ms(
                                timings.pack_ms,
                                variation,
                            );
                            self.leftover_begin_packing(
                                object_id,
                                special_target_id,
                                kind,
                                pack_ms,
                            );
                        }
                    } else {
                        self.pending_special_abilities.remove(&object_id);
                    }

                }
                AIState::Gathering => {
                    // Retail GameData.ini `ValuePerSupplyBox = 75` (ZH override of
                    // C++ GlobalData.cpp default 100). Player::getSupplyBoxValue
                    // (Player.cpp:1928-1930) reads TheGlobalData->m_baseValuePerSupplyBox.
                    const SUPPLY_BOX_VALUE: u32 = crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX
                        as u32;

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
                        // C++ wanting: findBestSupplyWarehouse with scan; fail → Regrouping.
                        let scan = self.collector_warehouse_scan(object_id, owner_player_id);
                        if let Some(next) =
                            self.find_nearest_harvestable_supply_within(team, position, scan)
                        {
                            if let Some(dest) = self.objects.get(&next).map(|s| s.get_position()) {
                                if let Some(obj) = self.objects.get_mut(&object_id) {
                                    obj.set_target(Some(next));
                                }
                                self.path_approach_with_state(object_id, dest, AIState::Gathering);
                                continue;
                            }
                        }
                        self.begin_supply_regroup(object_id, team, owner_player_id, position);
                        continue;
                    }

                    let collector_metadata_early = self
                        .objects
                        .get(&object_id)
                        .and_then(|object| object.thing.template.supply_truck_metadata);
                    if source_is_warehouse && collector_metadata_early.is_some() {
                        let docker_r = self
                            .objects
                            .get(&object_id)
                            .map(|o| o.thing.geometry.radius.max(1.0))
                            .unwrap_or(1.0);
                        if crate::game_logic::host_supply_gather::warehouse_too_far_2d(
                            (position.x, position.z),
                            (source_pos.x, source_pos.z),
                            docker_r,
                        ) {
                            let close = docker_r * 2.0;
                            if can_move && position.distance(source_pos) > close + 1.0 {
                                self.path_approach_with_state(
                                    object_id,
                                    source_pos,
                                    AIState::Gathering,
                                );
                                continue;
                            }
                            let (dx, dz) = crate::game_logic::host_supply_gather::warehouse_twitch_delta(
                                crate::game_logic::host_supply_gather::twitch_seed(
                                    object_id,
                                    self.frame,
                                ),
                                1,
                            );
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                let mut pos = obj.get_position();
                                pos.x += dx;
                                pos.z += dz;
                                obj.set_position(pos);
                            }
                            continue;
                        }
                    } else if can_move && position.distance(source_pos) > INTERACT_RANGE {
                        self.path_approach_with_state(object_id, source_pos, AIState::Gathering);
                        continue;
                    }
                    if source_is_warehouse && !self.try_claim_dock(source_id, object_id) {
                        continue;
                    }



                    // C++ AIDock waits for the authored warehouse action
                    // delay, then SupplyWarehouseDockUpdate transfers one
                    // box. Only an authored SupplyTruckAIUpdate uses this
                    // path; the older generic harvest command remains intact.
                    let collector_metadata = self
                        .objects
                        .get(&object_id)
                        .and_then(|object| object.thing.template.supply_truck_metadata);
                    if let Some(metadata) = collector_metadata {
                        let (state, next_frame) = self
                            .objects
                            .get(&object_id)
                            .map(|object| {
                                (
                                    object.supply_truck_state,
                                    object.supply_truck_next_dock_action_frame,
                                )
                            })
                            .unwrap_or((SupplyTruckState::Idle, 0));
                        if state != SupplyTruckState::DockingWarehouse {
                            if let Some(object) = self.objects.get_mut(&object_id) {
                                object.supply_truck_force_pending = false;
                                object.supply_truck_state = SupplyTruckState::DockingWarehouse;
                                object.supply_truck_next_dock_action_frame =
                                    self.frame.saturating_add(metadata.warehouse_delay_frames);
                            }
                            continue;
                        }
                        if self.frame < next_frame {
                            continue;
                        }
                    }

                    // In range — gather resources.  The host tracks cash
                    // value rather than C++ individual boxes, but a warehouse
                    // still cannot grant more than its authored stock.  This
                    // avoids turning an empty `SupplyWarehouseDockUpdate`
                    // into an infinite source.
                    let gather_amount = collector_metadata
                        .map(|_| SUPPLY_BOX_VALUE)
                        .unwrap_or_else(|| (100.0 * dt) as u32);
                    let max_carry = collector_metadata
                        .map(|metadata| metadata.max_boxes.saturating_mul(SUPPLY_BOX_VALUE))
                        .unwrap_or(1000);
                    let current_carry = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.stored_resources.supplies)
                        .unwrap_or(0);
                    // C++ SupplyTruckAIUpdate::gainOneBox (SupplyTruckAIUpdate.cpp:134-135)
                    // fails when m_numberBoxes >= m_maxBoxesData. Warehouse action
                    // (SupplyWarehouseDockUpdate.cpp:89-111) then ++m_boxesStored
                    // to take the tentative debit back.
                    let already_at_max_boxes = collector_metadata.is_some_and(|metadata| {
                        let (_remaining, _carry, transferred) =
                            crate::game_logic::host_supply_gather::warehouse_action_transfer_one_box(
                                source_supplies / SUPPLY_BOX_VALUE,
                                current_carry / SUPPLY_BOX_VALUE,
                                metadata.max_boxes,
                            );
                        !transferred && current_carry / SUPPLY_BOX_VALUE >= metadata.max_boxes
                    });
                    // Keep the legacy generic-resource path intact.  The
                    // precise stock gate is required specifically for the
                    // newly-authored SupplyWarehouseDockUpdate target.
                    let taken = if source_is_warehouse {
                        gather_amount.min(source_supplies)
                    } else {
                        gather_amount
                    };
                    if source_is_warehouse {
                        let crippled = self.objects.get(&source_id).is_some_and(|s| {
                            matches!(
                                s.body_damage_state,
                                crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged
                                    | crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                            )
                        });
                        if crippled {
                            let airborne = self.objects.get(&object_id).is_some_and(|o| {
                                o.is_kind_of(KindOf::Aircraft) || o.status.airborne_target
                            });
                            let docker_r = self
                                .objects
                                .get(&object_id)
                                .map(|o| o.thing.geometry.radius.max(1.0))
                                .unwrap_or(1.0);
                            let inside = !crate::game_logic::host_supply_gather::warehouse_too_far_2d(
                                (position.x, position.z),
                                (source_pos.x, source_pos.z),
                                docker_r,
                            );
                            match crate::game_logic::host_supply_gather::dock_cripple_victim_action(
                                true, inside, airborne,
                            ) {
                                crate::game_logic::host_supply_gather::DockCrippleVictimAction::KillGround => {
                                    if let Some(obj) = self.objects.get_mut(&object_id) {
                                        Self::mark_object_destroyed_authority_aware(obj, None);
                                    }
                                    self.mark_object_for_destruction(object_id, None);
                                }
                                crate::game_logic::host_supply_gather::DockCrippleVictimAction::IdleAndForceWanting => {
                                    if let Some(obj) = self.objects.get_mut(&object_id) {
                                        obj.supply_truck_force_pending = true;
                                        obj.supply_truck_state = SupplyTruckState::Wanting;
                                    }
                                    self.stop_attack_decision_aware(object_id);
                                    self.set_ai_state_decision_aware(object_id, AIState::Idle);
                                }
                                crate::game_logic::host_supply_gather::DockCrippleVictimAction::None => {}
                            }
                            self.release_dock_if_holder(source_id, object_id);

                            continue;
                        }
                    }
                    if source_is_warehouse && already_at_max_boxes {
                        self.release_dock_if_holder(source_id, object_id);
                        let refinery_dest = self
                            .preferred_or_allied_supply_center(
                                object_id,
                                team,
                                owner_player_id,
                                position,
                            )
                            .and_then(|rid| self.objects.get(&rid).map(|r| r.get_position()));
                        if let Some(dest) = refinery_dest {
                            self.path_approach_with_state(
                                object_id,
                                dest,
                                AIState::ReturningResources,
                            );
                        }
                        continue;
                    }
                    if source_is_warehouse && taken == 0 {
                        self.release_dock_if_holder(source_id, object_id);

                        let scan = self.collector_warehouse_scan(object_id, owner_player_id);
                        if let Some(next) =
                            self.find_nearest_harvestable_supply_within(team, position, scan)
                        {
                            if let Some(dest) = self.objects.get(&next).map(|s| s.get_position()) {
                                if let Some(obj) = self.objects.get_mut(&object_id) {
                                    obj.set_target(Some(next));
                                }
                                self.path_approach_with_state(object_id, dest, AIState::Gathering);
                                continue;
                            }
                        }
                        self.begin_supply_regroup(object_id, team, owner_player_id, position);
                        continue;
                    }
                    let is_full = current_carry + taken >= max_carry;

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_stored_supplies(
                            obj.stored_resources
                                .supplies
                                .saturating_add(taken)
                                .min(max_carry),
                        );
                        if let Some(metadata) = collector_metadata {
                            obj.supply_truck_next_dock_action_frame =
                                self.frame.saturating_add(metadata.warehouse_delay_frames);
                        }
                    }

                    let remaining_after = source_supplies.saturating_sub(taken);
                    // Deplete the supply source.
                    if let Some(source) = self.objects.get_mut(&source_id) {
                        source.set_stored_supplies(remaining_after);
                        if remaining_after == 0 && (!source_is_warehouse || delete_when_empty) {
                            Self::mark_object_destroyed_authority_aware(source, None);
                            self.mark_object_for_destruction(source_id, None);
                        }
                    }
                    if remaining_after == 0 && collector_metadata.is_some() {
                        let scan = self
                            .collector_warehouse_scan(object_id, owner_player_id)
                            .unwrap_or(0.0);
                        let next_dist = self
                            .find_nearest_harvestable_supply_within(team, position, Some(scan).filter(|d| *d > 0.0))
                            .and_then(|nid| self.objects.get(&nid).map(|s| s.get_position().distance(position)));
                        let voice = self
                            .objects
                            .get(&object_id)
                            .map(|o| o.thing.template.supplies_depleted_voice.clone())
                            .unwrap_or_default();
                        if crate::game_logic::host_supply_gather::should_play_supplies_depleted_voice(
                            next_dist, scan, &voice,
                        ) {
                            self.queue_audio_event(
                                crate::game_logic::AudioEventRequest::new(&voice)
                                    .with_object(object_id)
                                    .with_position(position)
                                    .with_priority(160),
                            );
                        }
                    }


                    if source_is_warehouse && (is_full || remaining_after == 0) {
                        self.release_dock_if_holder(source_id, object_id);
                    }

                    if is_full {
                        // Full — `SupplyTruckAIUpdate::m_preferredDock` wins
                        // over ResourceManager's nearest-center search when
                        // AI assigned this collector to a specific depot.
                        let refinery_dest = self
                            .preferred_or_allied_supply_center(
                                object_id,
                                team,
                                owner_player_id,
                                position,
                            )
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
                        .preferred_or_allied_supply_center(
                            object_id,
                            team,
                            owner_player_id,
                            position,
                        )
                        .and_then(|rid| {
                            self.objects
                                .get(&rid)
                                .map(|r| (Some(rid), r.get_position()))
                        })
                        .unwrap_or((None, position));


                    let at_refinery =
                        refinery_id.is_some() && position.distance(refinery_pos) <= INTERACT_RANGE;

                    if at_refinery {
                        if let Some(rid) = refinery_id {
                            if !self.try_claim_dock(rid, object_id) {
                                continue;
                            }
                        }

                        let collector_metadata = self
                            .objects
                            .get(&object_id)
                            .and_then(|object| object.thing.template.supply_truck_metadata);
                        if let Some(metadata) = collector_metadata {
                            let (state, next_frame) = self
                                .objects
                                .get(&object_id)
                                .map(|object| {
                                    (
                                        object.supply_truck_state,
                                        object.supply_truck_next_dock_action_frame,
                                    )
                                })
                                .unwrap_or((SupplyTruckState::Idle, 0));
                            if state != SupplyTruckState::DockingCenter {
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.supply_truck_state = SupplyTruckState::DockingCenter;
                                    object.supply_truck_next_dock_action_frame =
                                        self.frame.saturating_add(metadata.center_delay_frames);
                                }
                                continue;
                            }
                            if self.frame < next_frame {
                                continue;
                            }
                        }
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
                            // C++ SupplyCenterDockUpdate::action credits the
                            // *center* controlling player, so allied drop-offs
                            // pay the dock owner instead of vanishing.
                            let credited_player_id = refinery_id.and_then(|rid| {
                                self.objects
                                    .get(&rid)
                                    .and_then(|center| self.player_owner_for_host_object(center))
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
                                    let center_status = self
                                        .objects
                                        .get(&refinery_id.expect("checked above"))
                                        .map(|c| (c.status.stealthed, c.status.detected));
                                    let local = self
                                        .players
                                        .get(&player_id)
                                        .map(|p| p.is_local)
                                        .unwrap_or(false);
                                    let hide = center_status.is_some_and(|(stealth, detected)| {
                                        crate::game_logic::host_supply_gather::hide_stealth_supply_cash(
                                            stealth, local, detected,
                                        )
                                    });

                                    if !hide && credited > 0 {
                                        let ground_y = self
                                            .terrain_height_at(position)
                                            .unwrap_or(position.y);
                                        let color = self
                                            .players
                                            .get(&player_id)
                                            .map(|p| p.color_rgb)
                                            .unwrap_or((0, 255, 0));
                                        self.oil_derricks.record_floating_text(
                                            crate::game_logic::host_oil_derrick::HostAutoDepositFloatingText {
                                                text: crate::game_logic::host_supply_gather::format_gui_add_cash(credited),
                                                text_key: crate::game_logic::host_supply_gather::SUPPLY_CENTER_ADD_CASH_KEY
                                                    .to_string(),
                                                position: glam::Vec3::new(position.x, ground_y, position.z),
                                                color_rgba: (
                                                    color.0,
                                                    color.1,
                                                    color.2,
                                                    crate::game_logic::host_supply_gather::SUPPLY_CENTER_FLOATING_TEXT_ALPHA,
                                                ),
                                                amount: credited,
                                                spawn_frame: self.frame,
                                                source_id: object_id,
                                                is_capture_bonus: false,
                                            },
                                        );
                                    }
                                }

                            }
                            if let Some(rid) = refinery_id {
                                self.grant_center_temporary_stealth(rid, object_id);
                                self.release_dock_if_holder(rid, object_id);
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
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.supply_truck_state = SupplyTruckState::Wanting;
                                    object.supply_truck_next_dock_action_frame = 0;
                                }
                                self.path_approach_with_state(object_id, dest, AIState::Gathering);
                            } else if let Some(next) = self.find_nearest_harvestable_supply_within(
                                team,
                                position,
                                self.collector_warehouse_scan(object_id, owner_player_id),
                            ) {
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
                                self.begin_supply_regroup(
                                    object_id,
                                    team,
                                    owner_player_id,
                                    position,
                                );
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

        // C++ HealContain::update + TunnelContain::update → TunnelTracker::healObjects.
        self.tick_heal_contain_and_tunnel();
        // C++ TunnelContain::update nemesis + AITNGuard::lookForInnerTarget.
        self.tick_tunnel_network_nemesis();
    }

    /// C++ HealContain::update (HealContain.cpp:68-157) and
    /// TunnelContain::update → TunnelTracker::healObjects (TunnelContain.cpp:441-458).
    fn tick_heal_contain_and_tunnel(&mut self) {
        use crate::game_logic::host_tunnel_network::{
            heal_contain_done, tunnel_tracker_heal_amount, TUNNEL_FULL_HEAL_FRAMES,
        };

        let mut heal_jobs: Vec<(ObjectId, u32, Vec<ObjectId>, glam::Vec3)> = Vec::new();
        for (&id, obj) in &self.objects {
            if !obj.thing.template.contain_module.kind.is_heal_contain() {
                continue;
            }
            if !obj.is_alive() || obj.status.under_construction {
                continue;
            }
            let frames = obj
                .thing
                .template
                .contain_module
                .frames_for_full_heal
                .unwrap_or(0);
            let occupants = obj.contained_units();
            if occupants.is_empty() {
                continue;
            }
            heal_jobs.push((id, frames, occupants, obj.get_position()));
        }
        for (container_id, frames, occupants, origin) in heal_jobs {
            for (i, unit_id) in occupants.into_iter().enumerate() {
                let enter_frame = self
                    .tunnel_network
                    .contained_by_frame(unit_id)
                    .unwrap_or(self.frame);
                let contained_frames = self.frame.saturating_sub(enter_frame);
                let done = heal_contain_done(contained_frames, frames);
                let mut healed = false;
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    let amount = tunnel_tracker_heal_amount(
                        unit.health.maximum,
                        contained_frames,
                        frames,
                    );
                    if amount > 0.0 {
                        unit.heal(amount);
                        healed = true;
                    }
                }
                if healed {
                    self.tunnel_network.record_heal_tick();
                }
                if !done {
                    continue;
                }
                if let Some(container) = self.objects.get_mut(&container_id) {
                    let _ = container.remove_occupant(unit_id);
                }
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    unit.set_contained_by(None);
                    unit.target = None;
                    let angle = (i as f32) * 0.9;
                    let drop =
                        origin + glam::Vec3::new(angle.cos() * 8.0, 0.0, angle.sin() * 8.0);
                    unit.set_position(drop);
                    unit.stop_moving();
                    unit.set_ai_state(AIState::Idle);
                    unit.status.moving = false;
                }
                self.tunnel_network.clear_contained_by_frame(unit_id);
                self.tunnel_network.record_heal_auto_exit();
            }
        }

        // Each living TunnelContain::update heals the shared tracker (C++ per-entrance).
        let mut tunnel_ticks: Vec<(Team, u32)> = Vec::new();
        for obj in self.objects.values() {
            if !obj.is_alive() || obj.status.under_construction {
                continue;
            }
            let is_tunnel = obj.is_tunnel_network_style_container()
                || obj.thing.template.contain_module.kind.is_tunnel_contain();
            if !is_tunnel {
                continue;
            }
            let frames = obj
                .thing
                .template
                .contain_module
                .frames_for_full_heal
                .unwrap_or(TUNNEL_FULL_HEAL_FRAMES);
            tunnel_ticks.push((obj.team, frames));
        }
        for (team, frames) in tunnel_ticks {
            let passengers = self.tunnel_network.contained_for_team(team);
            for unit_id in passengers {
                let enter_frame = self
                    .tunnel_network
                    .contained_by_frame(unit_id)
                    .unwrap_or(self.frame);
                let contained_frames = self.frame.saturating_sub(enter_frame);
                let mut healed = false;
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    let amount = tunnel_tracker_heal_amount(
                        unit.health.maximum,
                        contained_frames,
                        frames,
                    );
                    if amount > 0.0 {
                        unit.heal(amount);
                        healed = true;
                    }
                }
                if healed {
                    self.tunnel_network.record_heal_tick();
                }
            }
        }
    }



    /// C++ `TunnelTracker::getCurNemesis` object-validity half.
    fn resolved_tunnel_nemesis(&mut self, team: Team) -> Option<ObjectId> {
        let Some(id) = self.tunnel_network.get_cur_nemesis_id(team, self.frame) else {
            return None;
        };
        let Some(obj) = self.objects.get(&id) else {
            self.tunnel_network.clear_nemesis(team);
            return None;
        };
        if !obj.is_alive() || obj.status.effectively_dead || obj.is_effectively_stealthed() {
            self.tunnel_network.clear_nemesis(team);
            return None;
        }
        Some(id)
    }

    /// C++ TunnelContain::update nemesis write + AITNGuard sally from the pool.
    fn tick_tunnel_network_nemesis(&mut self) {
        use crate::game_logic::KindOf;
        use gamelogic::common::Relationship;

        let mut writes: Vec<(Team, ObjectId)> = Vec::new();
        for obj in self.objects.values() {
            if !obj.is_alive() || obj.status.under_construction {
                continue;
            }
            let is_tunnel = obj.is_tunnel_network_style_container()
                || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                    &obj.template_name,
                );
            if !is_tunnel {
                continue;
            }
            let Some(src) = obj.last_damage_source else {
                continue;
            };
            let Some(ts) = obj.last_damage_timestamp else {
                continue;
            };
            if ts.saturating_add(30) <= self.frame {
                continue;
            }
            writes.push((obj.team, src));
        }
        for (team, src) in writes {
            let Some((v, s, inf, air, att_team, att_alive, att_owner, tunnel_rel_enemies)) = self
                .objects
                .get(&src)
                .map(|attacker| {
                    (
                        attacker.is_kind_of(KindOf::Vehicle),
                        attacker.is_kind_of(KindOf::Structure),
                        attacker.is_kind_of(KindOf::Infantry),
                        attacker.is_kind_of(KindOf::Aircraft),
                        attacker.team,
                        attacker.is_alive(),
                        attacker.owner_player_id,
                    )
                })
                .and_then(|(v, s, inf, air, att_team, att_alive, att_owner)| {
                    if !att_alive {
                        return None;
                    }
                    let tunnel = self.objects.values().find(|o| {
                        o.team == team
                            && o.is_alive()
                            && (o.is_tunnel_network_style_container()
                                || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                                    &o.template_name,
                                ))
                    })?;
                    let rel = match (tunnel.owner_player_id, att_owner) {
                        (Some(a), Some(b)) => self.player_relationship(a, b),
                        _ => Relationship::Neutral,
                    };
                    let enemies = rel == Relationship::Enemies
                        || (rel == Relationship::Neutral
                            && tunnel.team != att_team
                            && tunnel.team != Team::Neutral
                            && att_team != Team::Neutral);
                    Some((v, s, inf, air, att_team, att_alive, att_owner, enemies))
                })
            else {
                continue;
            };
            let _ = (att_team, att_alive, att_owner);
            if !tunnel_rel_enemies {
                continue;
            }
            self.tunnel_network
                .update_nemesis(team, src, v, s, inf, air, self.frame);
        }

        // AITNGuard::lookForInnerTarget residual: pool units that are
        // GuardTunnelNetwork / tunnel-defender sally to the tracker nemesis.
        let teams: Vec<Team> = self.tunnel_network_teams_with_occupants();
        for team in teams {
            let Some(nemesis) = self.resolved_tunnel_nemesis(team) else {
                continue;
            };
            let Some(nemesis_pos) = self.objects.get(&nemesis).map(|o| o.get_position()) else {
                continue;
            };
            let passengers = self.tunnel_network.contained_for_team(team);
            let mut sally: Vec<(ObjectId, ObjectId)> = Vec::new();
            for uid in passengers {
                let Some(unit) = self.objects.get(&uid) else {
                    continue;
                };
                if !unit.is_alive() || unit.target == Some(nemesis) {
                    continue;
                }
                let guard_is_tunnel = unit.guard_target.is_some_and(|gid| {
                    self.objects.get(&gid).is_some_and(|g| {
                        g.is_tunnel_network_style_container()
                            || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                                &g.template_name,
                            )
                    })
                });
                let is_defender = crate::game_logic::host_rpg_trooper::is_rpg_trooper_template(
                    &unit.template_name,
                );
                if !guard_is_tunnel && !is_defender {
                    continue;
                }
                let exit_tunnel = self
                    .tunnel_network
                    .entry_tunnel_of(uid)
                    .or_else(|| self.first_living_tunnel_for_team(team));
                let Some(exit_tunnel) = exit_tunnel else {
                    continue;
                };
                sally.push((uid, exit_tunnel));
            }
            for (uid, exit_tunnel) in sally {
                let _ = self.tunnel_network.record_exit(team, uid, exit_tunnel);
                let pos = self
                    .objects
                    .get(&exit_tunnel)
                    .map(|o| o.get_position())
                    .unwrap_or(nemesis_pos);
                if let Some(unit) = self.objects.get_mut(&uid) {
                    unit.set_contained_by(None);
                    unit.set_position(pos);
                    if crate::gameworld_shadow::gameworld_movement_authority_live() {
                        crate::game_logic::host_move_log::record(
                            unit.id,
                            Some([pos.x, pos.y, pos.z]),
                        );
                        unit.record_host_movement();
                    }
                }
                let _ = self.engage_target_decision_aware(uid, nemesis);
            }
        }
    }

    fn tunnel_network_teams_with_occupants(&self) -> Vec<Team> {
        [Team::USA, Team::China, Team::GLA]
            .into_iter()
            .filter(|team| self.tunnel_network.contain_count(*team) > 0)
            .collect()
    }

    fn first_living_tunnel_for_team(&self, team: Team) -> Option<ObjectId> {
        self.objects.iter().find_map(|(id, o)| {
            if o.team == team
                && o.is_alive()
                && !o.status.sold
                && (o.is_tunnel_network_style_container()
                    || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                        &o.template_name,
                    ))
            {
                Some(*id)
            } else {
                None
            }
        })
    }





    fn collector_warehouse_scan(&self, object_id: ObjectId, owner_player_id: Option<u32>) -> Option<f32> {
        let authored = self
            .objects
            .get(&object_id)
            .and_then(|object| object.thing.template.supply_truck_metadata)
            .map(|metadata| metadata.warehouse_scan_distance)?;
        let is_computer =
            owner_player_id.is_some_and(|pid| self.ai_manager.ai_players.contains_key(&pid));
        Some(crate::game_logic::host_supply_gather::warehouse_scan_distance(
            authored,
            is_computer,
        ))
    }

    fn begin_supply_regroup(
        &mut self,
        object_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        from: Vec3,
    ) {
        use crate::game_logic::host_supply_gather::{
            REGROUP_FIND_POSITION_RADIUS, REGROUP_SUCCESS_DISTANCE_SQUARED,
        };
        let dest = self.find_supply_regroup_target(team, owner_player_id, from);
        if let Some(dest_pos) = dest {
            let dx = dest_pos.x - from.x;
            let dz = dest_pos.z - from.z;
            if dx * dx + dz * dz > REGROUP_SUCCESS_DISTANCE_SQUARED {
                let offset = REGROUP_FIND_POSITION_RADIUS * 0.15;
                let approach = Vec3::new(dest_pos.x + offset, dest_pos.y, dest_pos.z);
                self.path_approach_with_state(object_id, approach, AIState::Idle);
            }
            if let Some(object) = self.objects.get_mut(&object_id) {
                object.supply_truck_state = SupplyTruckState::Regrouping;
                object.supply_truck_force_pending = true;
                object.supply_truck_next_dock_action_frame = 0;
            }
        } else {
            self.stop_attack_decision_aware(object_id);
            self.set_ai_state_decision_aware(object_id, AIState::Idle);
        }
    }

    fn find_supply_regroup_target(
        &self,
        team: Team,
        owner_player_id: Option<u32>,
        from: Vec3,
    ) -> Option<Vec3> {
        let mut best_cash: Option<(f32, Vec3)> = None;
        let mut best_cc: Option<(f32, Vec3)> = None;
        let mut best_struct: Option<(f32, Vec3)> = None;
        for obj in self.objects.values() {
            if !obj.is_alive() || obj.status.destroyed || obj.team != team {
                continue;
            }
            if owner_player_id.is_some()
                && self.player_owner_for_host_object(obj) != owner_player_id
            {
                continue;
            }
            if !obj.is_constructed() {
                continue;
            }
            let pos = obj.get_position();
            let dx = pos.x - from.x;
            let dz = pos.z - from.z;
            let dist2 = dx * dx + dz * dz;
            let is_cash = obj.is_kind_of(KindOf::SupplyCenter)
                || obj.is_kind_of(KindOf::FSSupplyCenter)
                || obj.thing.template.dock_kind == crate::game_logic::DockKind::SupplyCenter;
            let is_cc = obj.is_kind_of(KindOf::CommandCenter);
            let is_struct = obj.is_kind_of(KindOf::Structure);
            if is_cash && best_cash.is_none_or(|(d, _)| dist2 < d) {
                best_cash = Some((dist2, pos));
            }
            if is_cc && best_cc.is_none_or(|(d, _)| dist2 < d) {
                best_cc = Some((dist2, pos));
            }
            if is_struct && best_struct.is_none_or(|(d, _)| dist2 < d) {
                best_struct = Some((dist2, pos));
            }
        }
        best_cash
            .or(best_cc)
            .or(best_struct)
            .map(|(_, pos)| pos)
    }

    fn expire_temporary_stealth_grant(&mut self, object_id: ObjectId) {
        let Some(object) = self.objects.get(&object_id) else {
            return;
        };
        let expire = object.temporary_stealth_expires_frame;
        // Host residual for C++ getLastCommandSource() == CMD_FROM_PLAYER:
        // a move/attack order after stash grant is the player-visible exploit path.
        let last_command_from_player = matches!(
            object.ai_state,
            AIState::Moving
                | AIState::Attacking
                | AIState::AttackMoving
                | AIState::AttackingGround
        );
        if !Object::temporary_stealth_grant_should_expire(
            expire,
            self.frame,
            last_command_from_player,
        ) {
            return;
        }
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.temporary_stealth_expires_frame = 0;
            if !object.innate_stealth {
                object.break_stealth();
            }
        }
    }

    fn try_claim_dock(&mut self, dock_id: ObjectId, docker_id: ObjectId) -> bool {
        let current = self
            .objects
            .get(&dock_id)
            .and_then(|dock| dock.dock_active_docker);
        let current_alive = current
            .and_then(|id| self.objects.get(&id))
            .is_some_and(|object| object.is_alive());
        let next = crate::game_logic::host_supply_gather::dock_claim_active(
            current,
            current_alive,
            docker_id,
        );
        if let Some(dock) = self.objects.get_mut(&dock_id) {
            dock.dock_active_docker = next;
        }
        let clear = crate::game_logic::host_supply_gather::dock_is_clear_to_act(next, docker_id);
        if clear && next == Some(docker_id) && current != Some(docker_id) {
            self.apply_docking_model_conditions(dock_id, docker_id, true);
        }
        clear
    }

    fn release_dock_if_holder(&mut self, dock_id: ObjectId, docker_id: ObjectId) {
        let was_holder = self
            .objects
            .get(&dock_id)
            .is_some_and(|dock| dock.dock_active_docker == Some(docker_id));
        if let Some(dock) = self.objects.get_mut(&dock_id) {
            if dock.dock_active_docker == Some(docker_id) {
                dock.dock_active_docker = None;
            }
            if dock.repair_dock_last_id == Some(docker_id) {
                dock.repair_dock_last_id = None;
                dock.repair_dock_health_per_sec = 0.0;
            }
        }
        if was_holder {
            self.apply_docking_model_conditions(dock_id, docker_id, false);
        }
    }

    /// C++ `RepairDockUpdate::isRallyPointAfterDockType` + `AIDockMoveToRallyState`.
    fn send_to_rally_after_repair_dock(&mut self, docker_id: ObjectId, dock_id: ObjectId) {
        let Some(dock) = self.objects.get(&dock_id) else {
            return;
        };
        if !dock.is_kind_of(KindOf::RepairPad) {
            return;
        }
        let Some(rally) = dock.building_data.as_ref().and_then(|b| b.rally_point) else {
            return;
        };
        self.path_approach_with_state(docker_id, rally, AIState::Moving);
    }

    /// C++ `onEnterReached` / `onDockReached` / `onExitReached` MODELCONDITION_DOCKING*.
    fn apply_docking_model_conditions(
        &mut self,
        dock_id: ObjectId,
        docker_id: ObjectId,
        entering: bool,
    ) {
        use crate::game_logic::host_enum_table_residual::{
            docking_active_model_bit, docking_beginning_model_bit, docking_ending_model_bit,
            model_condition_bit_name_index,
        };
        let beginning = docking_beginning_model_bit();
        let active = docking_active_model_bit();
        let ending = docking_ending_model_bit();
        let docking = model_condition_bit_name_index("DOCKING").unwrap_or(0) as u32;
        for id in [dock_id, docker_id] {
            if let Some(obj) = self.objects.get_mut(&id) {
                if entering {
                    obj.model_condition_bits &= !(1u128 << ending);
                    obj.model_condition_bits |= 1u128 << beginning;
                    obj.model_condition_bits |= 1u128 << docking;
                    obj.model_condition_bits |= 1u128 << active;
                } else {
                    obj.model_condition_bits &= !(1u128 << beginning);
                    obj.model_condition_bits &= !(1u128 << docking);
                    obj.model_condition_bits &= !(1u128 << active);
                    obj.model_condition_bits |= 1u128 << ending;
                }
                obj.record_host_model_condition();
            }
        }
    }

    fn repair_dock_rate_for_docker(
        &mut self,
        pad_id: ObjectId,
        docker_id: ObjectId,
        max_hp: f32,
        current_hp: f32,
    ) -> f32 {
        let need_recompute = self.objects.get(&pad_id).is_some_and(|pad| {
            pad.repair_dock_last_id != Some(docker_id) || pad.repair_dock_health_per_sec <= 0.0
        });
        if !need_recompute {
            return self
                .objects
                .get(&pad_id)
                .map(|pad| pad.repair_dock_health_per_sec)
                .unwrap_or(0.0);
        }
        let rate = crate::game_logic::host_repair::repair_dock_hp_per_sec_from_missing(
            max_hp, current_hp,
        );
        if let Some(pad) = self.objects.get_mut(&pad_id) {
            pad.repair_dock_last_id = Some(docker_id);
            pad.repair_dock_health_per_sec = rate;
        }
        rate
    }

    fn grant_center_temporary_stealth(&mut self, center_id: ObjectId, docker_id: ObjectId) {
        let Some(center) = self.objects.get(&center_id) else {
            return;
        };
        let grant_frames =
            crate::game_logic::host_supply_gather::grant_temporary_stealth_frames_for_center(
                &center.template_name,
            );
        let center_stealthed = center.status.stealthed;
        let Some(docker) = self.objects.get(&docker_id) else {
            return;
        };
        let docker_is_temp = docker.temporary_stealth_expires_frame > self.frame;
        let docker_can_stealth = docker.innate_stealth || docker.status.stealthed;
        if !crate::game_logic::host_supply_gather::should_grant_temporary_stealth(
            center_stealthed,
            grant_frames,
            docker_is_temp,
            docker_can_stealth,
        ) {
            return;
        }
        if let Some(docker) = self.objects.get_mut(&docker_id) {
            docker.apply_grant_stealth();
            docker.temporary_stealth_expires_frame = self.frame.saturating_add(grant_frames);
        }
    }

    /// C++ ResourceManager + SupplyCenterDock: own or allied constructed center.
    fn preferred_or_allied_supply_center(
        &self,
        collector_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        from_position: glam::Vec3,
    ) -> Option<ObjectId> {
        let preferred = self
            .objects
            .get(&collector_id)
            .and_then(|collector| collector.preferred_dock_id);
        if let Some(center_id) = preferred {
            if self.supply_center_accepts_deposit(center_id, team, owner_player_id) {
                return Some(center_id);
            }
        }
        let mut best: Option<(f32, ObjectId)> = None;
        for (&id, obj) in &self.objects {
            if !self.supply_center_accepts_deposit(id, team, owner_player_id) {
                continue;
            }
            let d = from_position.distance(obj.get_position());
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, id));
            }
        }
        best.map(|(_, id)| id)
    }

    fn supply_center_accepts_deposit(
        &self,
        center_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
    ) -> bool {
        let Some(center) = self.objects.get(&center_id) else {
            return false;
        };
        if !center.is_alive() || !center.is_constructed() {
            return false;
        }
        let is_center = center.thing.template.dock_kind
            == crate::game_logic::DockKind::SupplyCenter
            || center.is_kind_of(KindOf::SupplyCenter)
            || center.thing.template.has_supply_center_create;
        if !is_center {
            return false;
        }
        let center_owner = self.player_owner_for_host_object(center);
        if center_owner == owner_player_id {
            return true;
        }
        if let (Some(a), Some(b)) = (owner_player_id, center_owner) {
            return self.player_relationship(a, b) == gamelogic::common::Relationship::Allies;
        }
        center.team == team
    }



}
