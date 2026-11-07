//! Live-host `FlightDeckBehavior` (C++ `FlightDeckBehavior.cpp`).
//!
//! Deck geometry comes from the receiving world's authored logical bones.
//!
//! Fail-closed: no FlightDeckBehavior module on the template means no deck.

use crate::game_logic::{AIState, GameLogic, KindOf, ObjectId};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// C++ `MAX_RUNWAYS`.
pub const FLIGHT_DECK_MAX_RUNWAYS: usize = 2;
/// C++ `isInPositionToTakeoff` 2D range-squared gate (`< 10`).
pub const FLIGHT_DECK_TAKEOFF_RANGE_SQR: f32 = 10.0;
const FOREVER: u32 = u32::MAX;

/// C++ `AICommandType` subset stored on the carrier (`m_designatedCommand`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HostFlightDeckCommand {
    #[default]
    NoCommand,
    GuardPosition,
    AttackPosition,
    AttackObject,
    ForceAttackObject,
    AttackMoveToPosition,
    Idle,
}

/// One C++ `FlightDeckInfo` stall.
#[derive(Debug, Clone, Copy)]
pub struct HostFlightDeckSpace {
    pub object_id: Option<ObjectId>,
    pub object_generation: u64,
    pub position: Vec3,
    pub orientation: f32,
    pub runway: usize,
}

/// One C++ `RunwayInfo`.
#[derive(Debug, Clone)]
pub struct HostFlightDeckRunway {
    pub start: Vec3,
    pub end: Vec3,
    pub landing_start: Vec3,
    pub landing_end: Vec3,
    pub taxi: Vec<Vec3>,
    pub creation: Vec<Vec3>,
    pub start_orient: f32,
    pub in_use_by_for_takeoff: Option<ObjectId>,
    pub in_use_by_for_landing: Option<ObjectId>,
}

/// Per-carrier runtime state (C++ `FlightDeckBehavior` members).
#[derive(Debug, Clone)]
pub struct HostFlightDeckState {
    pub spaces: Vec<HostFlightDeckSpace>,
    pub runways: Vec<HostFlightDeckRunway>,
    pub owner_generation: u64,
    pub got_info: bool,
    pub healing: Vec<(ObjectId, u32)>,
    pub next_heal_frame: u32,
    pub next_cleanup_frame: u32,
    pub started_production_frame: u32,
    pub next_allowed_production_frame: u32,
    pub next_launch_wave_frame: [u32; FLIGHT_DECK_MAX_RUNWAYS],
    pub ramp_up_frame: [u32; FLIGHT_DECK_MAX_RUNWAYS],
    pub catapult_system_frame: [u32; FLIGHT_DECK_MAX_RUNWAYS],
    pub lower_ramp_frame: [u32; FLIGHT_DECK_MAX_RUNWAYS],
    pub ramp_up: [bool; FLIGHT_DECK_MAX_RUNWAYS],
    pub designated_target: Option<ObjectId>,
    pub designated_command: HostFlightDeckCommand,
    pub designated_position: Vec3,
    pub pending_replacement: bool,
    pub catapult_fx_count: u32,
}

impl Default for HostFlightDeckState {
    fn default() -> Self {
        Self {
            spaces: Vec::new(),
            runways: Vec::new(),
            owner_generation: 0,
            got_info: false,
            healing: Vec::new(),
            next_heal_frame: FOREVER,
            next_cleanup_frame: 0,
            started_production_frame: FOREVER,
            next_allowed_production_frame: 0,
            next_launch_wave_frame: [0; FLIGHT_DECK_MAX_RUNWAYS],
            ramp_up_frame: [0; FLIGHT_DECK_MAX_RUNWAYS],
            catapult_system_frame: [0; FLIGHT_DECK_MAX_RUNWAYS],
            lower_ramp_frame: [0; FLIGHT_DECK_MAX_RUNWAYS],
            ramp_up: [false; FLIGHT_DECK_MAX_RUNWAYS],
            designated_target: None,
            designated_command: HostFlightDeckCommand::NoCommand,
            designated_position: Vec3::ZERO,
            pending_replacement: false,
            catapult_fx_count: 0,
        }
    }
}

impl GameLogic {
    /// C++ `FlightDeckBehavior::update` — first-build, replacements, taxi, launch.
    pub(crate) fn tick_flight_decks(&mut self) {
        self.tick_flight_decks_with_effects(&mut Default::default())
    }

    pub(crate) fn tick_flight_decks_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let mut carriers: Vec<ObjectId> = self.flight_decks.keys().copied().collect();
        for (&id, object) in self.objects.registered() {
            if object.thing().template.flight_deck.is_some() {
                carriers.push(id);
            }
        }
        carriers.sort_by_key(|id| id.0);
        carriers.dedup();
        for carrier_id in carriers {
            self.tick_one_flight_deck_with_effects(carrier_id, effects);
        }
    }

    fn tick_one_flight_deck(&mut self, carrier_id: ObjectId) {
        self.tick_one_flight_deck_with_effects(carrier_id, &mut Default::default())
    }

    fn tick_one_flight_deck_with_effects(
        &mut self,
        carrier_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(carrier) = self.objects.get(&carrier_id) else {
            self.flight_decks.remove(&carrier_id);
            return;
        };
        let carrier_id_birth = carrier.visual_object_generation;
        let epoch = self.visual_world_epoch;
        if carrier_id_birth == 0 { return; }
        if !carrier.is_alive() || carrier.status.destroyed || carrier.status.sold {
            self.kill_flight_deck_parked_units(carrier_id);
            self.flight_decks.remove(&carrier_id);
            return;
        }
        if carrier.status.under_construction {
            return;
        }
        if carrier.thing().template.flight_deck.is_none() {
            self.flight_decks.remove(&carrier_id);
            return;
        }
        if self.flight_decks.get(&carrier_id).is_some_and(|state| state.owner_generation != carrier_id_birth) {
            self.flight_decks.remove(&carrier_id);
        }

        if !self
            .flight_decks
            .get(&carrier_id)
            .is_some_and(|state| state.got_info)
        {
            let create_units = !self.flight_decks.contains_key(&carrier_id);
            self.flight_deck_build_info_with_effects(carrier_id, create_units, effects);
            if !self.flight_decks.get(&carrier_id).is_some_and(|state| state.got_info) {
                return;
            }
            if self.visual_world_epoch != epoch || !self.objects.get(&carrier_id).is_some_and(|object| object.visual_object_generation == carrier_id_birth) {
                return;
            }
        }
        self.flight_deck_purge_dead(carrier_id);
        self.flight_deck_tick_healing(carrier_id, effects);
        if self.visual_world_epoch != epoch || !self.objects.get(&carrier_id).is_some_and(|object| object.visual_object_generation == carrier_id_birth) { return; }
        self.flight_deck_update_parking_assignments(carrier_id);
        self.flight_deck_update_replacements_with_effects(carrier_id, effects);
        if self.visual_world_epoch != epoch || !self.objects.get(&carrier_id).is_some_and(|object| object.visual_object_generation == carrier_id_birth) {
            return;
        }
        self.flight_deck_update_launch_waves(carrier_id, effects);
        if self.visual_world_epoch != epoch || !self.objects.get(&carrier_id).is_some_and(|object| object.visual_object_generation == carrier_id_birth) {
            return;
        }
        self.flight_deck_update_no_attack(carrier_id);
    }

    /// C++ `FlightDeckBehavior::buildInfo`.
    fn flight_deck_build_info(&mut self, carrier_id: ObjectId, create_units: bool) {
        self.flight_deck_build_info_with_effects(carrier_id, create_units, &mut Default::default())
    }

    fn flight_deck_build_info_with_effects(
        &mut self,
        carrier_id: ObjectId,
        create_units: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(carrier_id_birth) = self
            .objects
            .get(&carrier_id)
            .map(|object| object.visual_object_generation)
        else {
            return;
        };
        let epoch = self.visual_world_epoch;
        if carrier_id_birth == 0 { return; }

        let pose_frame = self.frame;
        if self
            .flight_decks
            .get(&carrier_id)
            .is_some_and(|state| state.got_info)
        {
            return;
        }
        let Some((metadata, authored, team, owner)) =
            self.objects.get(&carrier_id).and_then(|carrier| {
                let metadata = carrier.thing().template.flight_deck.clone()?;
                let module = carrier.get_template().receiving_client_template.as_ref()?
                    .behavior_modules().iter()
                    .find(|entry| entry.name.as_str().eq_ignore_ascii_case("FlightDeckBehavior"))?;
                let authored = module.data.as_any().downcast_ref::<
                    gamelogic::object::behavior::flight_deck_behavior::FlightDeckBehaviorModuleData
                >()?.clone();
                Some((metadata, authored, carrier.team, carrier.owner_player_id))
            })
        else {
            return;
        };
        if self
            .objects
            .get(&carrier_id)
            .is_some_and(|c| c.status.under_construction || c.status.sold)
        {
            return;
        }

        let num_rows = usize::try_from(metadata.num_rows.max(0)).unwrap_or(0);
        let num_cols = usize::try_from(metadata.num_cols.max(0))
            .unwrap_or(0)
            .min(FLIGHT_DECK_MAX_RUNWAYS);
        let Some(carrier) = self.objects.get(&carrier_id) else { return };
        let pose = |bone: &str| {
            let poses = self.query_pristine_world_bone_poses(
                carrier, effects.pristine_bones, bone, 0, 1, 0, 0,
            ).ok()?;
            // Object.cpp:5990-6003 uses the owner pose on a real bone miss.
            Some(poses.into_iter().next().unwrap_or_else(|| {
                (carrier.get_position(), carrier.get_orientation())
            }))
        };

        let mut spaces = Vec::with_capacity(num_rows.saturating_mul(num_cols));
        let mut spawn_jobs: Vec<(usize, Vec3, f32)> = Vec::new();
        for row in 0..num_rows {
            for col in 0..num_cols {
                let Some((pos, orientation)) = authored.runway_info[col]
                    .spaces_bone_names.get(row).and_then(|bone| pose(bone)) else { return };
                if create_units && !metadata.payload_template.is_empty() {
                    spawn_jobs.push((spaces.len(), pos, orientation));
                }
                spaces.push(HostFlightDeckSpace {
                    object_id: None,
                    object_generation: 0,
                    position: pos,
                    orientation,
                    runway: col,
                });
            }
        }

        let mut runways = Vec::with_capacity(num_cols);
        for col in 0..num_cols {
            let definition = &authored.runway_info[col];
            let Some((start, _)) = pose(&definition.takeoff_bone_names[0]) else { return };
            let Some((end, _)) = pose(&definition.takeoff_bone_names[1]) else { return };
            let Some((landing_start, _)) = pose(&definition.landing_bone_names[0]) else { return };
            let Some((landing_end, _)) = pose(&definition.landing_bone_names[1]) else { return };
            let Some(taxi) = definition.taxi_bone_names.iter()
                .map(|bone| pose(bone).map(|p| p.0)).collect::<Option<Vec<_>>>() else { return };
            let Some(creation_poses) = definition.creation_bone_names.iter()
                .map(|bone| pose(bone)).collect::<Option<Vec<_>>>() else { return };
            let Some(&(_, start_orient)) = creation_poses.first() else { return };
            let creation = creation_poses.into_iter().map(|p| p.0).collect();
            runways.push(HostFlightDeckRunway {
                start,
                end,
                landing_start,
                landing_end,
                taxi,
                creation,
                start_orient,
                in_use_by_for_takeoff: None,
                in_use_by_for_landing: None,
            });
        }

        let mut state = self.flight_decks.remove(&carrier_id).unwrap_or_default();
        state.owner_generation = carrier_id_birth;
        for (new, old) in spaces.iter_mut().zip(&state.spaces) {
            new.object_id = old.object_id;
            new.object_generation = old.object_generation;
        }
        for (new, old) in runways.iter_mut().zip(&state.runways) {
            new.in_use_by_for_takeoff = old.in_use_by_for_takeoff;
            new.in_use_by_for_landing = old.in_use_by_for_landing;
        }
        state.spaces = spaces;
        state.runways = runways;
        state.got_info = true;

        for (index, pos, orient) in spawn_jobs {
            if self.visual_world_epoch != epoch || !self.objects.get(&carrier_id).is_some_and(|object| object.visual_object_generation == carrier_id_birth) {
                return;
            }
            let Some(jet_id) = self.create_object_for_owner_or_team_with_effects(
                &metadata.payload_template,
                team,
                owner,
                pos,
                None,
                effects,
            ) else {
                continue;
            };
            if self.visual_world_epoch != epoch || !self.objects.get(&carrier_id).is_some_and(|object| object.visual_object_generation == carrier_id_birth) {
                return;
            }
            let Some(jet_birth) = self.objects.get(&jet_id).map(|jet| jet.visual_object_generation).filter(|birth| *birth != 0) else { continue };
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.producer_id = Some(carrier_id);
                jet.airfield_parking_space_index = u32::try_from(index).ok();
                let _ = jet.apply_status_bits_upgrade_masks(&["DECK_HEIGHT_OFFSET"], &[]);
            }
            // FlightDeckBehavior.cpp201-202 installs position before orientation.
            if !self.set_owned_object_position_with_effects(jet_id, pos, pose_frame, effects) {
                continue;
            }
            if self.visual_world_epoch != epoch || !self.objects.get(&carrier_id).is_some_and(|object| object.visual_object_generation == carrier_id_birth) {
                return;
            }
            if !self.objects.get(&jet_id).is_some_and(|jet| jet.visual_object_generation == jet_birth) {
                continue;
            }
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.set_orientation_with_effects(
                    orient,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                );
                jet.set_ai_state(
                    AIState::Idle,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                jet.status.airborne_target = false;
            }
            if let Some(space) = state.spaces.get_mut(index) {
                space.object_id = Some(jet_id);
                space.object_generation = jet_birth;
            }
        }

        if self.visual_world_epoch != epoch || !self.objects.get(&carrier_id).is_some_and(|object| object.visual_object_generation == carrier_id_birth) {
            return;
        }
        self.flight_decks.insert(carrier_id, state);
    }

    fn flight_deck_purge_dead(&mut self, carrier_id: ObjectId) {
        let Some(state) = self.flight_decks.get_mut(&carrier_id) else {
            return;
        };
        for space in &mut state.spaces {
            let keep = space.object_id.is_some_and(|jet_id| {
                self.objects.get(&jet_id).is_some_and(|jet| {
                    jet.visual_object_generation == space.object_generation && space.object_generation != 0
                        && jet.is_alive() && !jet.status.destroyed && !jet.status.effectively_dead
                })
            });
            if !keep {
                space.object_id = None;
                space.object_generation = 0;
            }
        }
        for runway in &mut state.runways {
            if runway
                .in_use_by_for_takeoff
                .is_some_and(|id| !self.objects.get(&id).is_some_and(|o| o.is_alive()))
            {
                runway.in_use_by_for_takeoff = None;
            }
            if runway
                .in_use_by_for_landing
                .is_some_and(|id| !self.objects.get(&id).is_some_and(|o| o.is_alive()))
            {
                runway.in_use_by_for_landing = None;
            }
        }
    }

    /// FlightDeckBehavior::setHealee, called by the Jet/Chinook AI owner.
    pub(crate) fn set_flight_deck_healee(&mut self, carrier_id: ObjectId, jet_id: ObjectId, add: bool) {
        let Some(state) = self.flight_decks.get_mut(&carrier_id) else { return };
        if add {
            if state.healing.iter().any(|(id, _)| *id == jet_id) { return; }
            state.healing.push((jet_id, self.frame));
        } else {
            let before = state.healing.len();
            state.healing.retain(|(id, _)| *id != jet_id);
            if before == state.healing.len() { return; }
        }
        state.next_heal_frame = if state.healing.is_empty() { FOREVER } else { self.frame.saturating_add(6) };
    }

    fn flight_deck_tick_healing(&mut self, carrier_id: ObjectId, effects: &mut crate::game_logic::ScriptPresentationEffects<'_>) {
        let Some(amount) = self.objects.get(&carrier_id).and_then(|c| c.thing().template.flight_deck.as_ref())
            .map(|data| data.heal_amount_per_second * (6.0 / 30.0)) else { return };
        let Some(state) = self.flight_decks.get_mut(&carrier_id) else { return };
        if self.frame < state.next_heal_frame { return; }
        state.next_heal_frame = self.frame.saturating_add(6);
        state.healing.retain(|(id, _)| self.objects.get(id).is_some_and(|o| o.is_alive() && !o.status.effectively_dead));
        let healees: Vec<_> = state.healing.iter().map(|(id, _)| *id).collect();
        let epoch = self.visual_world_epoch;
        let Some(owner_birth) = self.objects.get(&carrier_id).map(|o| o.visual_object_generation) else { return };
        for id in healees {
            let _ = self.apply_owned_healing_with_effects(id, amount, Some(carrier_id), effects);
            if self.visual_world_epoch != epoch || !self.objects.get(&carrier_id).is_some_and(|o| o.visual_object_generation == owner_birth) { return; }
        }
    }


    /// C++ reserveDoorForExit always returns DOOR_1; occupancy is checked by
    /// exitObjectViaDoor after the new object and creation callbacks exist.
    pub(crate) fn flight_deck_spawn_exit(
        &mut self,
        carrier_id: ObjectId,
        jet_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let epoch = self.visual_world_epoch;
        let Some(carrier_birth) = self.host_object(carrier_id).map(|o| o.visual_object_generation) else { return false };
        let Some(jet_birth) = self.host_object(jet_id).map(|o| o.visual_object_generation) else { return false };
        if carrier_birth == 0 || jet_birth == 0 { return false; }
        if self.flight_decks.get(&carrier_id).is_some_and(|s| s.owner_generation != carrier_birth) {
            self.flight_decks.remove(&carrier_id);
        }
        if !self.flight_decks.get(&carrier_id).is_some_and(|s| s.got_info) {
            self.flight_deck_build_info_with_effects(carrier_id, false, effects);
        }
        let Some(state) = self.flight_decks.get_mut(&carrier_id) else { return false };
        if !state.got_info { return false; }
        let Some(index) = state.spaces.iter().position(|space| space.object_id.is_none()) else { return false };
        let stall = state.spaces[index];
        let Some(runway) = state.runways.get(stall.runway) else { return false };
        let Some(&creation) = runway.creation.first() else { return false };
        let start_orient = runway.start_orient;
        state.spaces[index].object_id = Some(jet_id);
        state.spaces[index].object_generation = jet_birth;
        state.pending_replacement = false;
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.airfield_parking_space_index = u32::try_from(index).ok();
        }
        let frame = self.frame;
        if !self.set_owned_object_position_with_effects(jet_id, creation, frame, effects)
            || self.visual_world_epoch != epoch
            || !self.host_object(carrier_id).is_some_and(|o| o.visual_object_generation == carrier_birth)
            || !self.host_object(jet_id).is_some_and(|o| o.visual_object_generation == jet_birth)
        { return false; }
        if let Some((jet, partition)) = self.host_object_and_partition_mut(jet_id) {
            jet.set_orientation_with_partition_owner(start_orient, partition);
        }
        self.update_owned_pathfinder_position(jet_id);
        if self.host_object(jet_id).is_some_and(|jet| jet.has_ai_update_interface()) {
            self.follow_exit_production_path(jet_id, &[stall.position], Some(carrier_id));
        }
        self.visual_world_epoch == epoch
            && self.host_object(carrier_id).is_some_and(|o| o.visual_object_generation == carrier_birth)
            && self.host_object(jet_id).is_some_and(|o| o.visual_object_generation == jet_birth)
    }

    fn flight_deck_exit_object_via_door(
        &mut self,
        carrier_id: ObjectId,
        jet_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.flight_deck_spawn_exit(carrier_id, jet_id, effects);
    }

    /// C++ `update` parking cleanup — promote idle/reloading rear jets.
    fn flight_deck_update_parking_assignments(&mut self, carrier_id: ObjectId) {
        let now = self.frame;
        let Some(metadata) = self
            .objects
            .get(&carrier_id)
            .and_then(|c| c.thing().template.flight_deck.clone())
        else {
            return;
        };
        if metadata.cleanup_frames == 0 {
            return;
        }
        let Some(state) = self.flight_decks.get(&carrier_id) else {
            return;
        };
        if now < state.next_cleanup_frame {
            return;
        }
        let num_cols = usize::try_from(metadata.num_cols.max(0))
            .unwrap_or(0)
            .min(FLIGHT_DECK_MAX_RUNWAYS)
            .max(1);
        let mut next_cleanup = now.saturating_add(metadata.cleanup_frames);
        let mut complete = [false; FLIGHT_DECK_MAX_RUNWAYS];
        let space_count = state.spaces.len();
        let mut promotions: Vec<(usize, usize, ObjectId, Option<ObjectId>, Vec3)> = Vec::new();

        for index in 0..space_count {
            let Some(state) = self.flight_decks.get(&carrier_id) else {
                break;
            };
            let Some(space) = state.spaces.get(index) else {
                break;
            };
            let runway = space.runway;
            if runway >= FLIGHT_DECK_MAX_RUNWAYS || complete[runway] {
                continue;
            }
            let front_id = space.object_id;
            if !self.flight_deck_space_can_give_up(carrier_id, front_id) {
                continue;
            }
            let mut temp = index + num_cols;
            while temp < space_count {
                let Some(state) = self.flight_decks.get(&carrier_id) else {
                    break;
                };
                let Some(rear) = state.spaces.get(temp) else {
                    break;
                };
                if let Some(parked_id) = rear.object_id {
                    if self.flight_deck_jet_can_move_forward(parked_id) {
                        let dest = state.spaces[index].position;
                        promotions.push((index, temp, parked_id, front_id, dest));
                        complete[runway] = true;
                        next_cleanup = now.saturating_add(metadata.human_follow_frames);
                        break;
                    }
                }
                temp = temp.saturating_add(num_cols);
            }
        }

        if let Some(state) = self.flight_decks.get_mut(&carrier_id) {
            state.next_cleanup_frame = next_cleanup;
            for (front, rear, parked_id, old_front, _) in &promotions {
                let front_birth = state.spaces[*front].object_generation;
                let rear_birth = state.spaces[*rear].object_generation;
                if let Some(space) = state.spaces.get_mut(*front) {
                    space.object_id = Some(*parked_id);
                    space.object_generation = rear_birth;
                }
                if let Some(space) = state.spaces.get_mut(*rear) {
                    space.object_id = *old_front;
                    space.object_generation = front_birth;
                }
            }
        }
        let operation_frame = self.frame;
        for (front, _rear, parked_id, _old_front, dest) in promotions {
            if let Some((jet, mut movement_events, audio_events, partition)) =
                self.host_object_movement_audio_partition_mut(parked_id)
            {
                let _ = jet.apply_status_bits_upgrade_masks(&["REASSIGN_PARKING"], &[]);
                jet.airfield_parking_space_index = u32::try_from(front).ok();
                jet.set_destination_with_operation(
                    dest,
                    &mut movement_events,
                    &mut crate::game_logic::ObjectAudio::queue(audio_events),
                    operation_frame,
                    &mut crate::game_logic::ScriptPresentationEffects::default()
                        .reborrow_for_object(partition),
                );
                jet.set_ai_state(
                    AIState::Moving,
                    operation_frame,
                    &mut crate::game_logic::ObjectAudio::queue(audio_events),
                );
            }
        }
    }

    /// C++ empty-stall `queueCreateUnit` residual. Host vehicles often have no
    /// `building_data`; after `ReplacementDelay + DockAnimationDelay` we spawn
    /// the payload and taxi it onto the empty stall (`exitObjectViaDoor`).
    fn flight_deck_update_replacements(&mut self, carrier_id: ObjectId) {
        self.flight_deck_update_replacements_with_effects(carrier_id, &mut Default::default())
    }

    fn flight_deck_update_replacements_with_effects(
        &mut self,
        carrier_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let now = self.frame;
        let Some(metadata) = self
            .objects
            .get(&carrier_id)
            .and_then(|c| c.thing().template.flight_deck.clone())
        else {
            return;
        };
        if metadata.payload_template.is_empty() {
            return;
        }
        let Some(state) = self.flight_decks.get_mut(&carrier_id) else {
            return;
        };
        if state.next_allowed_production_frame <= now {
            state.started_production_frame = FOREVER;
        }
        let empty = state.spaces.iter().any(|space| space.object_id.is_none());
        if !empty {
            state.pending_replacement = false;
            return;
        }
        if now < state.next_allowed_production_frame {
            return;
        }
        if state.pending_replacement {
            return;
        }

        let queued = self
            .objects
            .get(&carrier_id)
            .and_then(|c| c.building_data.as_ref())
            .is_some_and(|b| !b.production_queue.is_empty());
        if queued {
            return;
        }

        if let Some(building) = self
            .objects
            .get_mut(&carrier_id)
            .and_then(|c| c.building_data.as_mut())
        {
            if let Some(template) = self.templates.get(&metadata.payload_template).cloned() {
                let _ = building.add_to_queue_with_quantity_and_terms(
                    metadata.payload_template.clone(),
                    &template,
                    1,
                    0.0,
                    crate::game_logic::Resources::default(),
                );
            }
        }

        let Some(state) = self.flight_decks.get_mut(&carrier_id) else {
            return;
        };
        state.started_production_frame = now;
        state.next_allowed_production_frame = now
            .saturating_add(metadata.replacement_frames)
            .saturating_add(metadata.dock_animation_frames);
        state.pending_replacement = true;

        let has_queue = self
            .objects
            .get(&carrier_id)
            .and_then(|c| c.building_data.as_ref())
            .is_some_and(|b| !b.production_queue.is_empty());
        if has_queue {
            return;
        }
        let (team, owner, creation, orient) = {
            let Some(carrier) = self.objects.get(&carrier_id) else {
                return;
            };
            let Some(state) = self.flight_decks.get(&carrier_id) else {
                return;
            };
            let empty_index = state.spaces.iter().position(|s| s.object_id.is_none());
            let Some(empty_index) = empty_index else {
                return;
            };
            let runway = state.spaces[empty_index].runway;
            let creation = state
                .runways
                .get(runway)
                .and_then(|r| r.creation.first().copied())
                .unwrap_or(state.spaces[empty_index].position);
            let orient = state
                .runways
                .get(runway)
                .map(|r| r.start_orient)
                .unwrap_or(state.spaces[empty_index].orientation);
            (carrier.team, carrier.owner_player_id, creation, orient)
        };
        let Some(jet_id) = self.create_object_for_owner_or_team_with_effects(
            &metadata.payload_template,
            team,
            owner,
            creation,
            None,
            effects,
        ) else {
            if let Some(state) = self.flight_decks.get_mut(&carrier_id) {
                state.pending_replacement = false;
            }
            return;
        };
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.producer_id = Some(carrier_id);
            jet.set_orientation_with_effects(
                orient,
                &mut effects.reborrow_for_object(&mut self.partition_manager),
            );
        }
        self.flight_deck_exit_object_via_door(carrier_id, jet_id, effects);
    }

    /// C++ launch-wave + ramp + catapult (`update` `:1212-1269`).
    fn flight_deck_update_launch_waves(
        &mut self,
        carrier_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let now = self.frame;
        let Some(metadata) = self
            .objects
            .get(&carrier_id)
            .and_then(|c| c.thing().template.flight_deck.clone())
        else {
            return;
        };
        let num_cols = usize::try_from(metadata.num_cols.max(0))
            .unwrap_or(0)
            .min(FLIGHT_DECK_MAX_RUNWAYS);
        if num_cols == 0 {
            return;
        }
        let has_orders = self.flight_deck_has_takeoff_orders(carrier_id);
        for i in 0..num_cols {
            let Some(state) = self.flight_decks.get(&carrier_id) else {
                return;
            };
            let Some(front) = state.spaces.get(i) else {
                continue;
            };
            let Some(jet_id) = front.object_id else {
                self.flight_deck_tick_ramp_and_catapult(carrier_id, i, now, &metadata, effects);
                continue;
            };
            let in_position = self.flight_deck_jet_in_position_to_takeoff(carrier_id, jet_id, i);
            let can_give_up = self.flight_deck_jet_can_give_up(carrier_id, jet_id);
            if !can_give_up && in_position && has_orders {
                let Some(state) = self.flight_decks.get_mut(&carrier_id) else {
                    return;
                };
                if state.next_launch_wave_frame[i] <= now {
                    if !state.ramp_up[i] {
                        state.ramp_up[i] = true;
                        state.ramp_up_frame[i] = now.saturating_add(metadata.launch_ramp_frames);
                        state.lower_ramp_frame[i] = FOREVER;
                        self.flight_deck_set_ramp(carrier_id, i, true);
                    }
                    let Some(state) = self.flight_decks.get_mut(&carrier_id) else {
                        return;
                    };
                    if state.ramp_up[i] && state.ramp_up_frame[i] <= now {
                        self.flight_deck_propagate_order_to_plane(carrier_id, jet_id);
                        if let Some(state) = self.flight_decks.get_mut(&carrier_id) {
                            state.next_launch_wave_frame[i] =
                                now.saturating_add(metadata.launch_wave_frames);
                            state.catapult_system_frame[i] =
                                now.saturating_add(metadata.catapult_fire_frames);
                            state.lower_ramp_frame[i] =
                                now.saturating_add(metadata.lower_ramp_frames);
                            if let Some(runway) = state.runways.get_mut(i) {
                                runway.in_use_by_for_takeoff = Some(jet_id);
                            }
                        }
                    }
                }
            }
            self.flight_deck_tick_ramp_and_catapult(carrier_id, i, now, &metadata, effects);
        }
    }

    fn flight_deck_tick_ramp_and_catapult(
        &mut self,
        carrier_id: ObjectId,
        runway: usize,
        now: u32,
        metadata: &crate::game_logic::FlightDeckMetadata,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(state) = self.flight_decks.get_mut(&carrier_id) else {
            return;
        };
        if runway >= FLIGHT_DECK_MAX_RUNWAYS {
            return;
        }
        if state.catapult_system_frame[runway] <= now {
            state.catapult_system_frame[runway] = FOREVER;
            state.catapult_fx_count = state.catapult_fx_count.saturating_add(1);
            let (start, start_orient) = state
                .runways
                .get(runway)
                .map(|r| (r.start, r.start_orient))
                .unwrap_or((Vec3::ZERO, 0.0));
            let system = metadata
                .catapult_system
                .get(runway)
                .and_then(|name| name.as_deref())
                .unwrap_or("AircraftCarrierCatapultSteam");
            spawn_carrier_catapult_steam(system, start, start_orient, effects);
        }
        if state.ramp_up[runway] && state.lower_ramp_frame[runway] <= now {
            state.ramp_up[runway] = false;
            self.flight_deck_set_ramp(carrier_id, runway, false);
        }
    }

    fn flight_deck_set_ramp(&mut self, carrier_id: ObjectId, runway: usize, opening: bool) {
        use crate::game_logic::host_enum_table_residual::{
            door_2_closing_model_bit, door_2_opening_model_bit, door_3_closing_model_bit,
            door_3_opening_model_bit,
        };
        let (open_bit, close_bit) = if runway == 0 {
            (door_2_opening_model_bit(), door_2_closing_model_bit())
        } else {
            (door_3_opening_model_bit(), door_3_closing_model_bit())
        };
        let Some(carrier) = self.objects.get_mut(&carrier_id) else {
            return;
        };
        carrier.model_condition_bits &= !(1u128 << open_bit);
        carrier.model_condition_bits &= !(1u128 << close_bit);
        if opening {
            carrier.model_condition_bits |= 1u128 << open_bit;
        } else {
            carrier.model_condition_bits |= 1u128 << close_bit;
        }
    }

    fn flight_deck_update_no_attack(&mut self, carrier_id: ObjectId) {
        let has_aircraft = self
            .flight_decks
            .get(&carrier_id)
            .is_some_and(|state| state.spaces.iter().any(|space| space.object_id.is_some()));
        if let Some(carrier) = self.objects.get_mut(&carrier_id) {
            if has_aircraft {
                let _ = carrier.apply_status_bits_upgrade_masks(&[], &["NO_ATTACK"]);
            } else {
                let _ = carrier.apply_status_bits_upgrade_masks(&["NO_ATTACK"], &[]);
            }
        }
    }

    fn kill_flight_deck_parked_units(&mut self, carrier_id: ObjectId) {
        let Some(spaces) = self
            .flight_decks
            .get(&carrier_id)
            .map(|state| state.spaces.clone())
        else {
            return;
        };
        for space in spaces {
            let Some(jet_id) = space.object_id else {
                continue;
            };
            let airborne = self
                .objects
                .get(&jet_id)
                .is_some_and(|jet| jet.status.airborne_target);
            if airborne {
                if let Some(jet) = self.objects.get_mut(&jet_id) {
                    if jet.producer_id == Some(carrier_id) {
                        jet.producer_id = None;
                    }
                }
                continue;
            }
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                if !jet.is_alive() {
                    continue;
                }
                jet.internal_change_health(-jet.health.current);
                jet.status.destroyed = true;
                jet.status.effectively_dead = true;
            }
        }
    }

    /// C++ `FlightDeckBehavior::aiDoCommand` — carrier is a stump; orders go to jets.
    pub(crate) fn flight_deck_ai_do_command(
        &mut self,
        carrier_id: ObjectId,
        command: HostFlightDeckCommand,
        target: Option<ObjectId>,
        position: Option<Vec3>,
    ) -> bool {
        self.flight_deck_ai_do_command_with_effects(
            carrier_id,
            command,
            target,
            position,
            &mut Default::default(),
        )
    }

    pub(crate) fn flight_deck_ai_do_command_with_effects(
        &mut self,
        carrier_id: ObjectId,
        command: HostFlightDeckCommand,
        target: Option<ObjectId>,
        position: Option<Vec3>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        if !self.objects.get(&carrier_id).is_some_and(|obj| {
            obj.thing().template.flight_deck.is_some()
                && obj.is_alive()
                && !obj.status.under_construction
                && !obj.status.sold
        }) {
            return false;
        }
        if !self
            .flight_decks
            .get(&carrier_id)
            .is_some_and(|state| state.got_info)
        {
            let create_units = !self.flight_decks.contains_key(&carrier_id);
            self.flight_deck_build_info_with_effects(carrier_id, create_units, effects);
            if !self.flight_decks.get(&carrier_id).is_some_and(|state| state.got_info) {
                return false;
            }
        }
        let Some(state) = self.flight_decks.get_mut(&carrier_id) else {
            return true;
        };
        let mut movement_events = self.movement_events.writer(self.frame);
        let attack_events = &mut self.attack_events;
        match command {
            HostFlightDeckCommand::GuardPosition
            | HostFlightDeckCommand::AttackPosition
            | HostFlightDeckCommand::AttackMoveToPosition => {
                state.designated_target = None;
                state.designated_position = position.unwrap_or(Vec3::ZERO);
                state.designated_command = command;
            }
            HostFlightDeckCommand::ForceAttackObject | HostFlightDeckCommand::AttackObject => {
                state.designated_target = target;
                state.designated_position = Vec3::ZERO;
                state.designated_command = command;
            }
            HostFlightDeckCommand::Idle => {
                state.designated_target = None;
                state.designated_position = Vec3::ZERO;
                state.designated_command = command;
            }
            HostFlightDeckCommand::NoCommand => {
                state.designated_command = HostFlightDeckCommand::NoCommand;
                return true;
            }
        }
        self.flight_deck_propagate_orders_to_planes(carrier_id);
        true
    }

    fn flight_deck_propagate_orders_to_planes(&mut self, carrier_id: ObjectId) {
        let jets: Vec<ObjectId> = self
            .flight_decks
            .get(&carrier_id)
            .map(|state| {
                state
                    .spaces
                    .iter()
                    .filter_map(|space| space.object_id)
                    .collect()
            })
            .unwrap_or_default();
        for jet_id in jets {
            if self.flight_deck_jet_can_give_up(carrier_id, jet_id) {
                self.flight_deck_propagate_order_to_plane(carrier_id, jet_id);
            }
        }
    }

    fn flight_deck_propagate_order_to_plane(&mut self, carrier_id: ObjectId, jet_id: ObjectId) {
        let Some(state) = self.flight_decks.get(&carrier_id) else {
            return;
        };
        let command = state.designated_command;
        let target = state.designated_target;
        let position = state.designated_position;
        let stall_pos = state
            .spaces
            .iter()
            .find(|space| space.object_id == Some(jet_id))
            .map(|space| space.position);
        let Some(jet) = self.objects.get_mut(&jet_id) else {
            return;
        };
        let mut movement_events = self.movement_events.writer(self.frame);
        let attack_events = &mut self.attack_events;
        match command {
            HostFlightDeckCommand::GuardPosition => {
                jet.status.airborne_target = true;
                jet.set_guard_position(
                    Some(position),
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                jet.set_ai_state(
                    AIState::GuardingArea,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            HostFlightDeckCommand::AttackPosition => {
                jet.status.airborne_target = true;
                jet.target_location = Some(position);
                jet.set_ai_state(
                    AIState::AttackingGround,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            HostFlightDeckCommand::ForceAttackObject | HostFlightDeckCommand::AttackObject => {
                if let Some(target_id) = target {
                    jet.status.airborne_target = true;
                    jet.set_force_attack(command == HostFlightDeckCommand::ForceAttackObject);
                    jet.attack_target_with_operation(
                        target_id,
                        attack_events,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                        self.frame,
                        &mut crate::game_logic::ScriptPresentationEffects::default()
                            .reborrow_for_object(&mut self.partition_manager),
                    );
                }
            }
            HostFlightDeckCommand::AttackMoveToPosition => {
                jet.status.airborne_target = true;
                jet.set_destination_with_operation(
                    position,
                    &mut movement_events,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    self.frame,
                    &mut crate::game_logic::ScriptPresentationEffects::default()
                        .reborrow_for_object(&mut self.partition_manager),
                );
                jet.set_ai_state(
                    AIState::AttackMoving,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                jet.unit_ai_runtime.set_attack_move_goal(position);
            }
            HostFlightDeckCommand::Idle => {
                jet.set_destination_with_operation(
                    stall_pos.unwrap_or(position),
                    &mut movement_events,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    self.frame,
                    &mut crate::game_logic::ScriptPresentationEffects::default()
                        .reborrow_for_object(&mut self.partition_manager),
                );
                jet.set_ai_state(
                    AIState::Entering,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                jet.status.airborne_target = false;
                // Entering must pass the carrier. Do not keep the building
                // the jet was attacking, and do not blank the ignore.
                jet.ignored_obstacle_id = Some(carrier_id);
            }
            HostFlightDeckCommand::NoCommand => {}
        }
    }

    fn flight_deck_has_takeoff_orders(&mut self, carrier_id: ObjectId) -> bool {
        let Some(state) = self.flight_decks.get(&carrier_id) else {
            return false;
        };
        match state.designated_command {
            HostFlightDeckCommand::GuardPosition
            | HostFlightDeckCommand::AttackPosition
            | HostFlightDeckCommand::AttackMoveToPosition => true,
            HostFlightDeckCommand::ForceAttackObject | HostFlightDeckCommand::AttackObject => {
                let alive = state
                    .designated_target
                    .is_some_and(|id| self.objects.get(&id).is_some_and(|o| o.is_alive()));
                if alive {
                    true
                } else if let Some(state) = self.flight_decks.get_mut(&carrier_id) {
                    state.designated_command = HostFlightDeckCommand::NoCommand;
                    state.designated_target = None;
                    false
                } else {
                    false
                }
            }
            HostFlightDeckCommand::Idle | HostFlightDeckCommand::NoCommand => false,
        }
    }

    fn flight_deck_space_can_give_up(
        &self,
        carrier_id: ObjectId,
        jet_id: Option<ObjectId>,
    ) -> bool {
        match jet_id {
            None => true,
            Some(id) => self.flight_deck_jet_can_give_up(carrier_id, id),
        }
    }

    fn flight_deck_jet_can_give_up(&self, carrier_id: ObjectId, jet_id: ObjectId) -> bool {
        let Some(jet) = self.objects.get(&jet_id) else {
            return true;
        };
        if jet.status.airborne_target {
            return true;
        }
        let taxiing =
            jet.ai_state() == AIState::Moving && jet.has_object_status_bit("REASSIGN_PARKING");
        if jet.ai_state() != AIState::Idle && !taxiing {
            let designated_idle = self
                .flight_decks
                .get(&carrier_id)
                .is_some_and(|s| s.designated_command == HostFlightDeckCommand::Idle);
            if jet.ai_state() == AIState::Entering || designated_idle {
                return false;
            }
            if self.flight_decks.get(&carrier_id).is_some_and(|state| {
                state
                    .runways
                    .iter()
                    .any(|r| r.in_use_by_for_takeoff == Some(jet_id))
            }) {
                return false;
            }
            return true;
        }
        false
    }

    fn flight_deck_jet_can_move_forward(&self, jet_id: ObjectId) -> bool {
        let Some(jet) = self.objects.get(&jet_id) else {
            return false;
        };
        !jet.status.airborne_target
            && matches!(
                jet.ai_state(),
                AIState::Idle | AIState::Docked | AIState::SeekingRepair
            )
    }

    fn flight_deck_jet_in_position_to_takeoff(
        &self,
        carrier_id: ObjectId,
        jet_id: ObjectId,
        front_index: usize,
    ) -> bool {
        let Some(state) = self.flight_decks.get(&carrier_id) else {
            return false;
        };
        let Some(space) = state.spaces.get(front_index) else {
            return false;
        };
        if space.object_id != Some(jet_id) {
            return false;
        }
        let Some(jet) = self.objects.get(&jet_id) else {
            return false;
        };
        let delta = jet.get_position() - space.position;
        let dist_sqr = delta.x * delta.x + delta.z * delta.z;
        dist_sqr < FLIGHT_DECK_TAKEOFF_RANGE_SQR
    }

    pub fn snapshot_flight_deck_occupancy(&self) -> Vec<crate::save_load::FlightDeckPersistSnapshot> {
        use crate::save_load::{FlightDeckPersistSnapshot, FlightDeckRunwaySnapshot, FlightDeckSpaceSnapshot};
        let mut rows: Vec<_> = self.flight_decks.iter().map(|(&carrier_id, state)| FlightDeckPersistSnapshot {
            carrier_id,
            got_info: state.got_info || !state.spaces.is_empty() || !state.runways.is_empty(),
            spaces: state.spaces.iter().map(|s| FlightDeckSpaceSnapshot { object_id: s.object_id, runway: s.runway as u32 }).collect(),
            runways: state.runways.iter().map(|r| FlightDeckRunwaySnapshot { in_use_takeoff: r.in_use_by_for_takeoff, in_use_landing: r.in_use_by_for_landing }).collect(),
            designated_target: state.designated_target,
            designated_command: state.designated_command as u8,
            pending_replacement: state.pending_replacement,
            healing: state.healing.clone(),
            next_heal_frame: state.next_heal_frame,
            next_cleanup_frame: state.next_cleanup_frame,
            started_production_frame: state.started_production_frame,
            next_allowed_production_frame: state.next_allowed_production_frame,
            designated_position_bits: state.designated_position.to_array().map(f32::to_bits),
            next_launch_wave_frame: state.next_launch_wave_frame,
            ramp_up_frame: state.ramp_up_frame,
            catapult_system_frame: state.catapult_system_frame,
            lower_ramp_frame: state.lower_ramp_frame,
            ramp_up: state.ramp_up,
        }).collect();
        rows.sort_by_key(|row| row.carrier_id.0);
        rows
    }

    pub fn restore_flight_deck_occupancy(&mut self, rows: &[crate::save_load::FlightDeckPersistSnapshot]) {
        self.flight_decks.clear();
        for row in rows {
            let mut state = HostFlightDeckState::default();
            state.owner_generation = self.objects.get(&row.carrier_id).map_or(0, |o| o.visual_object_generation);
            // The receiving world's pristine assets rebuild these poses before use.
            state.spaces = row.spaces.iter().map(|s| HostFlightDeckSpace {
                object_id: s.object_id, object_generation: s.object_id.and_then(|id| self.objects.get(&id)).map_or(0, |o| o.visual_object_generation),
                runway: s.runway as usize, position: Vec3::ZERO, orientation: 0.0,
            }).collect();
            state.runways = row.runways.iter().map(|r| HostFlightDeckRunway {
                start: Vec3::ZERO, end: Vec3::ZERO, landing_start: Vec3::ZERO, landing_end: Vec3::ZERO,
                taxi: Vec::new(), creation: Vec::new(), start_orient: 0.0,
                in_use_by_for_takeoff: r.in_use_takeoff, in_use_by_for_landing: r.in_use_landing,
            }).collect();
            state.designated_target = row.designated_target;
            state.designated_command = match row.designated_command {
                1 => HostFlightDeckCommand::GuardPosition,
                2 => HostFlightDeckCommand::AttackPosition,
                3 => HostFlightDeckCommand::AttackObject,
                4 => HostFlightDeckCommand::ForceAttackObject,
                5 => HostFlightDeckCommand::AttackMoveToPosition,
                6 => HostFlightDeckCommand::Idle,
                _ => HostFlightDeckCommand::NoCommand,
            };
            state.pending_replacement = row.pending_replacement;
            state.healing = row.healing.clone();
            state.next_heal_frame = row.next_heal_frame;
            state.next_cleanup_frame = row.next_cleanup_frame;
            state.started_production_frame = row.started_production_frame;
            state.next_allowed_production_frame = row.next_allowed_production_frame;
            state.designated_position = Vec3::from_array(row.designated_position_bits.map(f32::from_bits));
            state.next_launch_wave_frame = row.next_launch_wave_frame;
            state.ramp_up_frame = row.ramp_up_frame;
            state.catapult_system_frame = row.catapult_system_frame;
            state.lower_ramp_frame = row.lower_ramp_frame;
            state.ramp_up = row.ramp_up;
            self.flight_decks.insert(row.carrier_id, state);
        }
    }
}


/// C++ `FlightDeckBehavior.cpp:1248-1257`: `TheParticleSystemManager->createParticleSystem`
/// then `setLocalTransform(startTransform)` + `setPosition(start)`.
/// `RunwayNCatapultSystem` is a ParticleSystemTemplate, not an FXList.
fn spawn_carrier_catapult_steam(
    template_name: &str,
    start: Vec3,
    start_orient: f32,
    effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
) {
    if template_name.is_empty() || template_name.eq_ignore_ascii_case("None") {
        return;
    }
    #[cfg(feature = "game_client")]
    {
        let Some(manager) =
            crate::game_logic::combat_particles::ParticleBindingAccess::particle_systems_mut(
                effects,
            )
        else {
            return;
        };
        if manager.admit_effective_definitions().is_err() {
            return;
        }
        let Some(template) = manager.find_template(template_name) else {
            return;
        };
        let Ok(system_id) = manager.create_particle_system(&template, true) else {
            return;
        };
        if let Some(system) = manager.find_particle_system_mut(system_id) {
            // C++ order: setLocalTransform(startTransform), then setPosition(start).
            system.set_local_transform(glam::Mat3::from_rotation_z(start_orient));
            // Host Y-up `(x, height, z)` → C++ Z-up `(x, y_ground, z_height)`.
            system.set_position(Vec3::new(start.x, start.z, start.y));
        }
    }
    #[cfg(not(feature = "game_client"))]
    let _ = (template_name, start, start_orient, effects);
}

#[allow(dead_code)]
fn _kindof_aircraft_hint() -> KindOf {
    KindOf::Aircraft
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{FlightDeckMetadata, KindOf, Team, ThingTemplate};

    fn deck_metadata() -> FlightDeckMetadata {
        FlightDeckMetadata {
            payload_template: "AircraftCarrierRaptor".to_string(),
            num_rows: 2,
            num_cols: 2,
            approach_height: 50.0,
            landing_deck_height_offset: 22.0,
            heal_amount_per_second: 10.0,
            cleanup_frames: 1,
            human_follow_frames: 1,
            replacement_frames: 1,
            dock_animation_frames: 0,
            launch_wave_frames: 1,
            launch_ramp_frames: 0,
            lower_ramp_frames: 1,
            catapult_fire_frames: 0,
            catapult_system: [Some("AircraftCarrierCatapultSteam".into()), None],
        }
    }

    fn authored_deck_assets(offset: f32) -> crate::assets::AssetManager {
        use crate::assets::{W3dHierarchy, W3dHlod, W3dHlodLod, W3dPivot};
        let mut model = crate::assets::W3DModel::new("DeckTest".into());
        let pivot = |name: &str, parent_idx, translation| W3dPivot {
            name: name.into(), parent_idx, translation, euler_angles: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0],
        };
        model.hierarchy = Some(W3dHierarchy { name: "DeckHierarchy".into(), pivots: vec![
            pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]),
            pivot("Stall", 0, [4.0 + offset, 2.0, 3.0]),
            pivot("TakeStart", 0, [5.0 + offset, 2.0, 3.0]),
            pivot("TakeEnd", 0, [9.0 + offset, 2.0, 3.0]),
            pivot("LandStart", 0, [-4.0 + offset, 2.0, 3.0]),
            pivot("LandEnd", 0, [1.0 + offset, 2.0, 3.0]),
            pivot("Create", 0, [2.0 + offset, 3.0, 4.0]),
        ], pivot_fixups: Vec::new() });
        model.hlods = vec![W3dHlod { version: 0, name: "DeckTest".into(), hierarchy_name: "DeckHierarchy".into(),
            lods: vec![W3dHlodLod { max_screen_size: 1.0, subobjects: Vec::new() }],
            aggregates: None, proxies: None, has_unrendered_aggregates: false, has_invalid_trailing_records: false }];
        let mut assets = crate::assets::AssetManager::new().unwrap();
        assets.admit_test_w3d_source("DeckTest", model);
        assets
    }

    fn authored_spawn_deck() -> (GameLogic, ObjectId) {
        let ini = r#"
Object FlightDeckSpawnOwner
 KindOf = STRUCTURE SELECTABLE
 Draw = W3DModelDraw DeckDraw
  DefaultConditionState
   Model = DeckTest
  End
 End
 Body = ActiveBody OwnerBody
  MaxHealth = 500
  InitialHealth = 500
 End
 Behavior = FlightDeckBehavior Deck
  NumRunways = 1
  NumSpacesPerRunway = 1
  Runway1Spaces = Stall
  Runway1Takeoff = TakeStart TakeEnd
  Runway1Landing = LandStart LandEnd
  Runway1Creation = Create
 End
 Behavior = SpawnBehavior Spawn
  SpawnNumber = 1
  SpawnTemplateName = FlightDeckSpawnChild
  OneShot = Yes
 End
End
Object FlightDeckSpawnChild
 KindOf = AIRCRAFT SELECTABLE
 Body = ActiveBody ChildBody
  MaxHealth = 100
  InitialHealth = 100
 End
End
"#;
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("deck-spawn.ini".into(), ini.into())]).unwrap();
        let owner = world.create_object("FlightDeckSpawnOwner", Team::USA, Vec3::new(10.0, 0.0, 20.0)).unwrap();
        (world, owner)
    }

    #[test]
    fn authored_deck_spawn_uses_receiving_w3d_and_survives_save_load() {
        let assets = authored_deck_assets(0.0);
        let peer_assets = authored_deck_assets(100.0);
        let (mut world, owner) = authored_spawn_deck();
        let (mut peer, peer_owner) = authored_spawn_deck();
        assert_eq!(owner, peer_owner);
        world.set_current_frame(1);
        world.update_spawn_behavior_with_effects(&mut crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(&assets), ..Default::default()
        }).unwrap();
        let children = world.host_object(owner).unwrap().spawn_behavior_children();
        assert_eq!(children.len(), 1);
        let child = children[0].0;
        let position = world.host_object(child).unwrap().get_position();
        assert_ne!(position, Vec3::ZERO);
        assert!(peer.host_object(peer_owner).unwrap().spawn_behavior_children().is_empty());
        let snapshot = crate::save_load::SnapshotBuilder::new().create_world_snapshot(&world).unwrap();
        let (mut restored, _) = authored_spawn_deck();
        crate::save_load::SnapshotBuilder::new().restore_from_snapshot(&snapshot, &mut restored).unwrap();
        assert_eq!(restored.host_object(owner).unwrap().spawn_behavior_children()[0].0, child);
        assert_eq!(restored.host_object(child).unwrap().get_position(), position);
        peer.set_current_frame(1);
        peer.update_spawn_behavior_with_effects(&mut crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(&peer_assets), ..Default::default()
        }).unwrap();
        let peer_child = peer.host_object(peer_owner).unwrap().spawn_behavior_children()[0].0;
        assert_eq!(child, peer_child);
        assert_ne!(peer.host_object(peer_child).unwrap().get_position(), position);
    }

    fn seed_carrier(logic: &mut GameLogic) -> ObjectId {
        let mut carrier = ThingTemplate::new("AmericaAircraftCarrier");
        carrier
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(2000.0);
        carrier.flight_deck = Some(deck_metadata());
        logic
            .templates
            .insert("AmericaAircraftCarrier".into(), carrier);

        let mut jet = ThingTemplate::new("AircraftCarrierRaptor");
        jet.add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(160.0);
        logic.templates.insert("AircraftCarrierRaptor".into(), jet);

        logic
            .create_object(
                "AmericaAircraftCarrier",
                Team::USA,
                Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("carrier")
    }

    #[test]
    fn carrier_without_receiving_bones_does_not_fabricate_deck() {
        let mut logic = GameLogic::new();
        let carrier = seed_carrier(&mut logic);
        logic.tick_flight_decks();
        assert!(!logic.flight_decks.get(&carrier).is_some_and(|state| state.got_info));
    }

    #[test]
    fn flight_deck_xfer_timers_rebind_to_receiving_carrier() {
        let mut source = GameLogic::new();
        let carrier = seed_carrier(&mut source);
        let mut state = HostFlightDeckState::default();
        state.owner_generation = source.host_object(carrier).unwrap().visual_object_generation;
        state.next_heal_frame = 17;
        state.next_cleanup_frame = 23;
        state.started_production_frame = 31;
        state.next_allowed_production_frame = 37;
        state.designated_position = Vec3::new(2.0, 3.0, 5.0);
        state.next_launch_wave_frame = [41, 43];
        state.ramp_up_frame = [47, 53];
        state.catapult_system_frame = [59, 61];
        state.lower_ramp_frame = [67, 71];
        state.ramp_up = [true, false];
        source.flight_decks.insert(carrier, state);
        let snapshot = crate::save_load::SnapshotBuilder::new().create_world_snapshot(&source).unwrap();
        let mut receiver = GameLogic::new();
        let same_id = seed_carrier(&mut receiver);
        assert_eq!(carrier, same_id);
        crate::save_load::SnapshotBuilder::new().restore_from_snapshot(&snapshot, &mut receiver).unwrap();
        let restored = receiver.flight_decks.get(&carrier).unwrap();
        assert_eq!(restored.owner_generation, receiver.host_object(carrier).unwrap().visual_object_generation);
        assert_eq!(restored.next_heal_frame, 17);
        assert_eq!(restored.next_cleanup_frame, 23);
        assert_eq!(restored.started_production_frame, 31);
        assert_eq!(restored.next_allowed_production_frame, 37);
        assert_eq!(restored.designated_position, Vec3::new(2.0, 3.0, 5.0));
        assert_eq!(restored.next_launch_wave_frame, [41, 43]);
        assert_eq!(restored.ramp_up_frame, [47, 53]);
        assert_eq!(restored.catapult_system_frame, [59, 61]);
        assert_eq!(restored.lower_ramp_frame, [67, 71]);
        assert_eq!(restored.ramp_up, [true, false]);
    }

    #[test]
    fn duplicate_flight_deck_snapshot_is_rejected_before_world_mutation() {
        let mut source = GameLogic::new();
        let carrier = seed_carrier(&mut source);
        let mut state = HostFlightDeckState::default();
        state.owner_generation = source.host_object(carrier).unwrap().visual_object_generation;
        source.flight_decks.insert(carrier, state);
        let builder = crate::save_load::SnapshotBuilder::new();
        let mut snapshot = builder.create_world_snapshot(&source).unwrap();
        let duplicate = snapshot.airfield_parking.flight_decks[0].clone();
        snapshot.airfield_parking.flight_decks.push(duplicate);
        let mut receiver = GameLogic::new();
        let receiver_id = seed_carrier(&mut receiver);
        let birth = receiver.host_object(receiver_id).unwrap().visual_object_generation;
        assert!(builder.restore_from_snapshot(&snapshot, &mut receiver).is_err());
        assert_eq!(receiver.host_object(receiver_id).unwrap().visual_object_generation, birth);
    }
}
