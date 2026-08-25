//! Live-host `FlightDeckBehavior` (C++ `FlightDeckBehavior.cpp`).
//!
//! Player-visible carrier path: a finished AmericaAircraftCarrier payload-spawns
//! its deck jets, auto-rebuilds lost stalls, launches on attack/guard, and taxis
//! rear jets forward with ramp / catapult timing. Host has no W3D logical bones,
//! so stall / runway poses use the authored-count airfield layout
//! (row-major `R1S1, R2S1, …`).
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
    pub got_info: bool,
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
            got_info: false,
            next_cleanup_frame: 0,
            started_production_frame: FOREVER,
            next_allowed_production_frame: 0,
            next_launch_wave_frame: [0; FLIGHT_DECK_MAX_RUNWAYS],
            ramp_up_frame: [0; FLIGHT_DECK_MAX_RUNWAYS],
            catapult_system_frame: [FOREVER; FLIGHT_DECK_MAX_RUNWAYS],
            lower_ramp_frame: [FOREVER; FLIGHT_DECK_MAX_RUNWAYS],
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
        let mut carriers: Vec<ObjectId> = self.flight_decks.keys().copied().collect();
        for (&id, object) in self.objects.iter() {
            if object.thing.template.flight_deck.is_some() {
                carriers.push(id);
            }
        }
        carriers.sort_by_key(|id| id.0);
        carriers.dedup();
        for carrier_id in carriers {
            self.tick_one_flight_deck(carrier_id);
        }
    }

    fn tick_one_flight_deck(&mut self, carrier_id: ObjectId) {
        let Some(carrier) = self.objects.get(&carrier_id) else {
            self.flight_decks.remove(&carrier_id);
            return;
        };
        if !carrier.is_alive() || carrier.status.destroyed || carrier.status.sold {
            self.kill_flight_deck_parked_units(carrier_id);
            self.flight_decks.remove(&carrier_id);
            return;
        }
        if carrier.status.under_construction {
            return;
        }
        if carrier.thing.template.flight_deck.is_none() {
            self.flight_decks.remove(&carrier_id);
            return;
        }

        if !self
            .flight_decks
            .get(&carrier_id)
            .is_some_and(|state| state.got_info)
        {
            self.flight_deck_build_info(carrier_id, true);
        }
        self.flight_deck_purge_dead(carrier_id);
        self.flight_deck_adopt_unassigned_replacements(carrier_id);
        self.flight_deck_update_parking_assignments(carrier_id);
        self.flight_deck_update_replacements(carrier_id);
        self.flight_deck_update_launch_waves(carrier_id);
        self.flight_deck_update_no_attack(carrier_id);
    }

    /// C++ `FlightDeckBehavior::buildInfo`.
    fn flight_deck_build_info(&mut self, carrier_id: ObjectId, create_units: bool) {
        if self
            .flight_decks
            .get(&carrier_id)
            .is_some_and(|state| state.got_info)
        {
            return;
        }
        let Some((metadata, origin, forward, right, team, owner)) =
            self.objects.get(&carrier_id).and_then(|carrier| {
                let metadata = carrier.thing.template.flight_deck.clone()?;
                let mut forward = carrier.thing.get_direction_vector();
                forward.y = 0.0;
                if forward.length_squared() < 1.0e-6 {
                    forward = Vec3::new(0.0, 0.0, -1.0);
                } else {
                    forward = forward.normalize();
                }
                let right = Vec3::new(forward.z, 0.0, -forward.x);
                Some((
                    metadata,
                    carrier.get_position(),
                    forward,
                    right,
                    carrier.team,
                    carrier.owner_player_id,
                ))
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
        let deck = metadata.landing_deck_height_offset;
        let spacing = crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_PREP_SPACING;
        let orientation = forward.x.atan2(forward.z);

        let mut spaces = Vec::with_capacity(num_rows.saturating_mul(num_cols));
        let mut spawn_jobs: Vec<(usize, Vec3, f32)> = Vec::new();
        for row in 0..num_rows {
            for col in 0..num_cols {
                let pos = flight_deck_stall_pose(
                    origin, forward, right, col, row, num_cols, deck, spacing,
                );
                if create_units && !metadata.payload_template.is_empty() {
                    spawn_jobs.push((spaces.len(), pos, orientation));
                }
                spaces.push(HostFlightDeckSpace {
                    object_id: None,
                    position: pos,
                    orientation,
                    runway: col,
                });
            }
        }

        let mut runways = Vec::with_capacity(num_cols);
        for col in 0..num_cols {
            let start = flight_deck_runway_pose(
                origin,
                forward,
                right,
                col,
                num_cols,
                deck,
                spacing,
                num_rows as f32 * spacing + spacing * 0.5,
            );
            let end = flight_deck_runway_pose(
                origin,
                forward,
                right,
                col,
                num_cols,
                deck,
                spacing,
                num_rows as f32 * spacing + spacing * 2.5,
            );
            let landing_end = end;
            let landing_start = flight_deck_runway_pose(
                origin, forward, right, col, num_cols, deck, spacing, -spacing,
            );
            let creation = flight_deck_runway_pose(
                origin,
                forward,
                right,
                col,
                num_cols,
                deck,
                spacing,
                -spacing * 1.5,
            );
            runways.push(HostFlightDeckRunway {
                start,
                end,
                landing_start,
                landing_end,
                taxi: vec![creation, start],
                creation: vec![creation],
                start_orient: orientation,
                in_use_by_for_takeoff: None,
                in_use_by_for_landing: None,
            });
        }

        let mut state = HostFlightDeckState {
            spaces,
            runways,
            got_info: true,
            ..HostFlightDeckState::default()
        };

        for (index, pos, orient) in spawn_jobs {
            let Some(jet_id) =
                self.create_object_for_owner_or_team(&metadata.payload_template, team, owner, pos)
            else {
                continue;
            };
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.producer_id = Some(carrier_id);
                jet.airfield_parking_space_index = u32::try_from(index).ok();
                jet.set_orientation(orient);
                jet.set_position(pos);
                jet.set_ai_state(AIState::Idle);
                jet.status.airborne_target = false;
                if metadata.landing_deck_height_offset != 0.0 {
                    let _ = jet.apply_status_bits_upgrade_masks(&["DECK_HEIGHT_OFFSET"], &[]);
                }
            }
            if let Some(space) = state.spaces.get_mut(index) {
                space.object_id = Some(jet_id);
            }
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
                    jet.is_alive() && !jet.status.destroyed && !jet.status.effectively_dead
                })
            });
            if !keep {
                space.object_id = None;
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

    /// C++ `exitObjectViaDoor`: adopt a just-produced payload jet into an empty stall.
    fn flight_deck_adopt_unassigned_replacements(&mut self, carrier_id: ObjectId) {
        let payload = self
            .objects
            .get(&carrier_id)
            .and_then(|c| c.thing.template.flight_deck.as_ref())
            .map(|m| m.payload_template.clone())
            .filter(|name| !name.is_empty());
        let Some(payload) = payload else {
            return;
        };
        let assigned: Vec<ObjectId> = self
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
        let mut candidates: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                (obj.is_alive()
                    && obj.producer_id == Some(carrier_id)
                    && obj.template_name.eq_ignore_ascii_case(&payload)
                    && !assigned.contains(&id))
                .then_some(id)
            })
            .collect();
        candidates.sort_by_key(|id| id.0);
        for jet_id in candidates {
            self.flight_deck_exit_object_via_door(carrier_id, jet_id);
        }
    }

    fn flight_deck_exit_object_via_door(&mut self, carrier_id: ObjectId, jet_id: ObjectId) {
        let Some(state) = self.flight_decks.get_mut(&carrier_id) else {
            return;
        };
        let Some(index) = state
            .spaces
            .iter()
            .position(|space| space.object_id.is_none())
        else {
            return;
        };
        state.spaces[index].object_id = Some(jet_id);
        state.pending_replacement = false;
        let stall = state.spaces[index];
        let runway = stall.runway;
        let creation = state
            .runways
            .get(runway)
            .and_then(|r| r.creation.first().copied())
            .unwrap_or(stall.position);
        let start_orient = state
            .runways
            .get(runway)
            .map(|r| r.start_orient)
            .unwrap_or(stall.orientation);
        let deck_offset = self
            .objects
            .get(&carrier_id)
            .and_then(|c| c.thing.template.flight_deck.as_ref())
            .map(|m| m.landing_deck_height_offset)
            .unwrap_or(0.0);
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.producer_id = Some(carrier_id);
            jet.airfield_parking_space_index = u32::try_from(index).ok();
            jet.set_position(creation);
            jet.set_orientation(start_orient);
            jet.set_destination(stall.position);
            jet.set_ai_state(AIState::Moving);
            jet.status.airborne_target = false;
            if deck_offset != 0.0 {
                let _ = jet.apply_status_bits_upgrade_masks(&["DECK_HEIGHT_OFFSET"], &[]);
            }
        }
    }

    /// C++ `update` parking cleanup — promote idle/reloading rear jets.
    fn flight_deck_update_parking_assignments(&mut self, carrier_id: ObjectId) {
        let now = self.frame;
        let Some(metadata) = self
            .objects
            .get(&carrier_id)
            .and_then(|c| c.thing.template.flight_deck.clone())
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
                if let Some(space) = state.spaces.get_mut(*front) {
                    space.object_id = Some(*parked_id);
                }
                if let Some(space) = state.spaces.get_mut(*rear) {
                    space.object_id = *old_front;
                }
            }
        }
        for (front, _rear, parked_id, _old_front, dest) in promotions {
            if let Some(jet) = self.objects.get_mut(&parked_id) {
                let _ = jet.apply_status_bits_upgrade_masks(&["REASSIGN_PARKING"], &[]);
                jet.airfield_parking_space_index = u32::try_from(front).ok();
                jet.set_destination(dest);
                jet.set_ai_state(AIState::Moving);
            }
        }
    }

    /// C++ empty-stall `queueCreateUnit` residual. Host vehicles often have no
    /// `building_data`; after `ReplacementDelay + DockAnimationDelay` we spawn
    /// the payload and taxi it onto the empty stall (`exitObjectViaDoor`).
    fn flight_deck_update_replacements(&mut self, carrier_id: ObjectId) {
        let now = self.frame;
        let Some(metadata) = self
            .objects
            .get(&carrier_id)
            .and_then(|c| c.thing.template.flight_deck.clone())
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
        let Some(jet_id) =
            self.create_object_for_owner_or_team(&metadata.payload_template, team, owner, creation)
        else {
            if let Some(state) = self.flight_decks.get_mut(&carrier_id) {
                state.pending_replacement = false;
            }
            return;
        };
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.producer_id = Some(carrier_id);
            jet.set_orientation(orient);
        }
        self.flight_deck_exit_object_via_door(carrier_id, jet_id);
    }

    /// C++ launch-wave + ramp + catapult (`update` `:1212-1269`).
    fn flight_deck_update_launch_waves(&mut self, carrier_id: ObjectId) {
        let now = self.frame;
        let Some(metadata) = self
            .objects
            .get(&carrier_id)
            .and_then(|c| c.thing.template.flight_deck.clone())
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
                self.flight_deck_tick_ramp_and_catapult(carrier_id, i, now, &metadata);
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
            self.flight_deck_tick_ramp_and_catapult(carrier_id, i, now, &metadata);
        }
    }

    fn flight_deck_tick_ramp_and_catapult(
        &mut self,
        carrier_id: ObjectId,
        runway: usize,
        now: u32,
        metadata: &crate::game_logic::FlightDeckMetadata,
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
            spawn_carrier_catapult_steam(system, start, start_orient);
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
        carrier.record_host_model_condition();
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
                jet.health.current = 0.0;
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
        if !self.objects.get(&carrier_id).is_some_and(|obj| {
            obj.thing.template.flight_deck.is_some()
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
            self.flight_deck_build_info(carrier_id, true);
        }
        let Some(state) = self.flight_decks.get_mut(&carrier_id) else {
            return true;
        };
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
        match command {
            HostFlightDeckCommand::GuardPosition => {
                jet.status.airborne_target = true;
                jet.set_guard_position(Some(position));
                jet.set_ai_state(AIState::GuardingArea);
            }
            HostFlightDeckCommand::AttackPosition => {
                jet.status.airborne_target = true;
                jet.target_location = Some(position);
                jet.set_ai_state(AIState::AttackingGround);
            }
            HostFlightDeckCommand::ForceAttackObject | HostFlightDeckCommand::AttackObject => {
                if let Some(target_id) = target {
                    jet.status.airborne_target = true;
                    jet.set_force_attack(command == HostFlightDeckCommand::ForceAttackObject);
                    jet.attack_target(target_id);
                }
            }
            HostFlightDeckCommand::AttackMoveToPosition => {
                jet.status.airborne_target = true;
                jet.is_attack_path = true;
                jet.set_destination(position);
                jet.set_ai_state(AIState::AttackMoving);
            }
            HostFlightDeckCommand::Idle => {
                jet.set_destination(stall_pos.unwrap_or(position));
                jet.set_ai_state(AIState::Entering);
                jet.status.airborne_target = false;
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
            jet.ai_state == AIState::Moving && jet.has_object_status_bit("REASSIGN_PARKING");
        if jet.ai_state != AIState::Idle && !taxiing {
            let designated_idle = self
                .flight_decks
                .get(&carrier_id)
                .is_some_and(|s| s.designated_command == HostFlightDeckCommand::Idle);
            if jet.ai_state == AIState::Entering || designated_idle {
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
                jet.ai_state,
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

    /// C++ FlightDeckBehavior stall occupancy for WorldSnapshot.
    pub fn snapshot_flight_deck_occupancy(
        &self,
    ) -> Vec<(
        ObjectId,
        bool,
        Vec<(Option<ObjectId>, u32)>,
        Vec<(Option<ObjectId>, Option<ObjectId>)>,
        Option<ObjectId>,
        u8,
        bool,
    )> {
        let mut rows: Vec<_> = self
            .flight_decks
            .iter()
            .map(|(&carrier_id, state)| {
                (
                    carrier_id,
                    state.got_info,
                    state
                        .spaces
                        .iter()
                        .map(|space| (space.object_id, space.runway as u32))
                        .collect(),
                    state
                        .runways
                        .iter()
                        .map(|runway| (runway.in_use_by_for_takeoff, runway.in_use_by_for_landing))
                        .collect(),
                    state.designated_target,
                    state.designated_command as u8,
                    state.pending_replacement,
                )
            })
            .collect();
        rows.sort_by_key(|(id, _, _, _, _, _, _)| id.0);
        rows
    }

    pub fn restore_flight_deck_occupancy(
        &mut self,
        rows: Vec<(
            ObjectId,
            bool,
            Vec<(Option<ObjectId>, u32)>,
            Vec<(Option<ObjectId>, Option<ObjectId>)>,
            Option<ObjectId>,
            u8,
            bool,
        )>,
    ) {
        self.flight_decks.clear();
        for (
            carrier_id,
            got_info,
            spaces,
            runways,
            designated_target,
            designated_command,
            pending_replacement,
        ) in rows
        {
            let mut state = HostFlightDeckState::default();
            state.got_info = got_info;
            state.spaces = spaces
                .into_iter()
                .map(|(object_id, runway)| HostFlightDeckSpace {
                    object_id,
                    position: Vec3::ZERO,
                    orientation: 0.0,
                    runway: runway as usize,
                })
                .collect();
            state.runways = runways
                .into_iter()
                .map(
                    |(in_use_by_for_takeoff, in_use_by_for_landing)| HostFlightDeckRunway {
                        start: Vec3::ZERO,
                        end: Vec3::ZERO,
                        landing_start: Vec3::ZERO,
                        landing_end: Vec3::ZERO,
                        taxi: Vec::new(),
                        creation: Vec::new(),
                        start_orient: 0.0,
                        in_use_by_for_takeoff,
                        in_use_by_for_landing,
                    },
                )
                .collect();
            state.designated_target = designated_target;
            state.designated_command = match designated_command {
                1 => HostFlightDeckCommand::GuardPosition,
                2 => HostFlightDeckCommand::AttackPosition,
                3 => HostFlightDeckCommand::AttackObject,
                4 => HostFlightDeckCommand::ForceAttackObject,
                5 => HostFlightDeckCommand::AttackMoveToPosition,
                6 => HostFlightDeckCommand::Idle,
                _ => HostFlightDeckCommand::NoCommand,
            };
            state.pending_replacement = pending_replacement;
            self.flight_decks.insert(carrier_id, state);
        }
    }
}

fn flight_deck_stall_pose(
    origin: Vec3,
    forward: Vec3,
    right: Vec3,
    col: usize,
    row: usize,
    num_cols: usize,
    deck: f32,
    spacing: f32,
) -> Vec3 {
    let col_center = col as f32 - (num_cols.saturating_sub(1) as f32 * 0.5);
    let pos = origin + right * (col_center * spacing) + forward * (row as f32 * spacing);
    Vec3::new(pos.x, origin.y + deck, pos.z)
}

fn flight_deck_runway_pose(
    origin: Vec3,
    forward: Vec3,
    right: Vec3,
    col: usize,
    num_cols: usize,
    deck: f32,
    spacing: f32,
    along: f32,
) -> Vec3 {
    let col_center = col as f32 - (num_cols.saturating_sub(1) as f32 * 0.5);
    let pos = origin + right * (col_center * spacing) + forward * along;
    Vec3::new(pos.x, origin.y + deck, pos.z)
}

/// C++ `FlightDeckBehavior.cpp:1248-1257`: `TheParticleSystemManager->createParticleSystem`
/// then `setLocalTransform(startTransform)` + `setPosition(start)`.
/// `RunwayNCatapultSystem` is a ParticleSystemTemplate, not an FXList.
fn spawn_carrier_catapult_steam(template: &str, start: Vec3, start_orient: f32) {
    if template.is_empty() || template.eq_ignore_ascii_case("None") {
        return;
    }
    let Some(manager) = gamelogic::helpers::TheParticleSystemManager::get() else {
        return;
    };
    let Some(system_id) = manager.create_particle_system(Some(template)) else {
        return;
    };
    // Host Y-up `(x, height, z)` → leftover/C++ Z-up `(x, y_ground, z_height)`.
    let leftover_pos = gamelogic::common::Coord3D::new(start.x, start.z, start.y);
    let leftover_transform = gamelogic::common::Matrix3D::from_rotation_z(start_orient);
    manager.set_particle_system_transform(system_id, &leftover_transform);
    manager.set_particle_system_position(system_id, &leftover_pos);
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
    fn finished_carrier_payload_spawns_deck_jets() {
        let mut logic = GameLogic::new();
        let carrier = seed_carrier(&mut logic);
        logic.tick_flight_decks();
        let state = logic.flight_decks.get(&carrier).expect("deck state");
        assert!(state.got_info);
        assert_eq!(state.spaces.len(), 4);
        let parked = state
            .spaces
            .iter()
            .filter(|space| space.object_id.is_some())
            .count();
        assert_eq!(
            parked, 4,
            "finished carrier must spawn PayloadTemplate jets"
        );
    }

    #[test]
    fn lost_deck_jet_auto_rebuilds() {
        let mut logic = GameLogic::new();
        let carrier = seed_carrier(&mut logic);
        logic.tick_flight_decks();
        let lost = logic
            .flight_decks
            .get(&carrier)
            .and_then(|state| state.spaces[3].object_id)
            .expect("rear jet");
        if let Some(jet) = logic.objects.get_mut(&lost) {
            jet.health.current = 0.0;
            jet.status.destroyed = true;
            jet.status.effectively_dead = true;
        }
        logic.frame = 2;
        logic.tick_flight_decks();
        let state = logic.flight_decks.get(&carrier).expect("deck");
        assert!(
            state.spaces.iter().all(|space| space.object_id.is_some()),
            "empty stall must rebuild payload jet"
        );
        assert_ne!(state.spaces[3].object_id, Some(lost));
    }

    #[test]
    fn attack_on_carrier_launches_front_jets() {
        let mut logic = GameLogic::new();
        let carrier = seed_carrier(&mut logic);
        let mut enemy = ThingTemplate::new("TestTank");
        enemy
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0);
        logic.templates.insert("TestTank".into(), enemy);
        let target = logic
            .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
            .expect("enemy");
        logic.tick_flight_decks();
        assert!(logic.flight_deck_ai_do_command(
            carrier,
            HostFlightDeckCommand::AttackObject,
            Some(target),
            None,
        ));
        logic.frame = 1;
        logic.tick_flight_decks();
        let front = logic.flight_decks.get(&carrier).unwrap().spaces[0]
            .object_id
            .expect("front jet");
        let jet = logic.objects.get(&front).expect("jet");
        assert!(
            jet.status.airborne_target || jet.ai_state == AIState::Attacking,
            "front-row jet must take the carrier attack order after ramp"
        );
        assert!(
            logic
                .flight_decks
                .get(&carrier)
                .is_some_and(|s| s.catapult_fx_count > 0
                    || s.ramp_up[0]
                    || s.catapult_system_frame[0] == u32::MAX),
            "launch must arm ramp/catapult timers"
        );
    }

    #[test]
    fn rear_idle_jet_taxis_into_vacant_front_stall() {
        let mut logic = GameLogic::new();
        let carrier = seed_carrier(&mut logic);
        logic.tick_flight_decks();
        let front = logic.flight_decks.get(&carrier).unwrap().spaces[0]
            .object_id
            .expect("front");
        if let Some(jet) = logic.objects.get_mut(&front) {
            jet.status.airborne_target = true;
            jet.set_ai_state(AIState::Attacking);
        }
        logic.frame = 2;
        logic.tick_flight_decks();
        let state = logic.flight_decks.get(&carrier).expect("deck");
        assert_ne!(
            state.spaces[0].object_id,
            Some(front),
            "rear idle jet must promote into the vacant front stall"
        );
        let promoted = state.spaces[0].object_id.expect("promoted");
        let jet = logic.objects.get(&promoted).expect("promoted jet");
        assert!(
            jet.has_object_status_bit("REASSIGN_PARKING") || jet.ai_state == AIState::Moving,
            "promoted jet taxis with REASSIGN_PARKING"
        );
    }

    /// C++ FlightDeckBehavior.cpp:1248-1257 createParticleSystem + setLocalTransform/setPosition.
    #[test]
    fn catapult_steam_uses_particle_system_not_fx_list() {
        let src = include_str!("host_flight_deck.rs");
        let start = src
            .find("fn spawn_carrier_catapult_steam")
            .expect("catapult spawn");
        let body = &src[start..start + 900];
        assert!(
            body.contains("TheParticleSystemManager::get()"),
            "catapult steam must create a leftover ParticleSystem"
        );
        assert!(
            body.contains("create_particle_system(Some(template))"),
            "RunwayNCatapultSystem is a ParticleSystemTemplate"
        );
        assert!(
            body.contains("set_particle_system_transform")
                && body.contains("set_particle_system_position"),
            "C++ setLocalTransform(startTransform) + setPosition(start)"
        );
        assert!(
            body.contains("Coord3D::new(start.x, start.z, start.y)"),
            "host Y-up must swizzle to leftover Z-up"
        );
        let fire = src
            .find("spawn_carrier_catapult_steam(system, start, start_orient)")
            .expect("fire site");
        let fire_win = &src[fire.saturating_sub(400)..fire + 80];
        assert!(
            !fire_win.contains("dispatch_fx_list_at_pos"),
            "AircraftCarrierCatapultSteam is not an FXList: {fire_win}"
        );
    }
}
