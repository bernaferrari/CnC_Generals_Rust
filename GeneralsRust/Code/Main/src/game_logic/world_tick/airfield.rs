//! Host tick `impl GameLogic` — `airfield`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
struct HostAirfieldPPInfo {
    parking_space: glam::Vec3,
    parking_orientation: f32,
    runway_prep: glam::Vec3,
    runway_start: glam::Vec3,
    runway_end: glam::Vec3,
    runway_approach: glam::Vec3,
    hangar_internal: glam::Vec3,
    hangar_internal_orient: f32,
    runway_takeoff_dist: f32,
}
impl GameLogic {
    /// Update combat for all objects.
    ///
    /// Fail-closed residual: uses secondary when present and selected by
    /// `Object::select_combat_weapon_slot` (prefer secondary vs structures when
    /// secondary damage is better; alternate secondary when primary not ready).
    /// Not full C++ AutoChoose / PreferredAgainst matrices.
    ///
    /// `pub(crate)` so residual/unit tests can exercise the fire path directly.
    /// C++ JetAIUpdate RETURN_TO_BASE residual: rearm empty jet weapons at
    /// a source-authored ParkingPlaceBehavior airfield.  Fail-closed versus
    /// unsupported parking/taxi matrices.
    /// C++ JetOrHeliCirclingDeadAirfieldState residual for all empty RTB jets.
    pub(crate) fn tick_out_of_ammo_jet_damage(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft)
                    && (o.needs_return_to_base_rearm() || o.return_to_base_requested)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if self.try_return_to_base_rearm(id) {
                continue;
            }
            if let Some(jet) = self.objects.get_mut(&id) {
                if jet.needs_return_to_base_rearm() {
                    let _ = jet.apply_out_of_ammo_damage_frame();
                }
            }
        }
    }

    #[inline]
    fn is_aircraft(object: &Object) -> bool {
        object.is_kind_of(KindOf::Aircraft) || object.object_type == ObjectType::Aircraft
    }

    /// C++ `KINDOF_PRODUCED_AT_HELIPAD`. Host KindOf bank may omit the bit, so
    /// also honor the established helicopter-template detector.
    pub(crate) fn template_is_produced_at_helipad(
        template: &crate::game_logic::ThingTemplate,
    ) -> bool {
        crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
            &template.name,
        )
    }

    pub(crate) fn object_is_produced_at_helipad(object: &Object) -> bool {
        Self::template_is_produced_at_helipad(&object.thing.template)
            || crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
                &object.template_name,
            )
    }

    /// C++ `ParkingPlaceInfo` bone layout residual (host has no W3D logical bones).
    fn airfield_space_row_col(
        index: usize,
        metadata: &crate::game_logic::ParkingPlaceMetadata,
    ) -> Option<(usize, usize)> {
        let cols = usize::try_from(metadata.num_cols).ok().filter(|&c| c > 0)?;
        Some((index / cols, index % cols))
    }

    fn airfield_logical_bone_pose(
        origin: glam::Vec3,
        forward: glam::Vec3,
        right: glam::Vec3,
        col: usize,
        row: usize,
        num_cols: usize,
        num_rows: usize,
        deck: f32,
        along: f32,
        across_scale: f32,
    ) -> (glam::Vec3, f32) {
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_PREP_SPACING;
        let col_center = col as f32 - (num_cols.saturating_sub(1) as f32 * 0.5);
        let row_center = row as f32 - (num_rows.saturating_sub(1) as f32 * 0.5);
        let pos = origin
            + right * (col_center * PARKING_PLACE_RUNWAY_PREP_SPACING)
            + forward * (along + row_center * PARKING_PLACE_RUNWAY_PREP_SPACING * across_scale);
        (
            glam::Vec3::new(pos.x, origin.y + deck, pos.z),
            forward.x.atan2(forward.z),
        )
    }

    /// C++ `calcPPInfo` from stall/runway bones + `wasInLine` start override.
    fn calc_airfield_pp_info(
        &self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<HostAirfieldPPInfo> {
        use crate::game_logic::host_dock_contain_exit_heal_residual::{
            PARKING_PLACE_RUNWAY_APPROACH_DIST, PARKING_PLACE_RUNWAY_PREP_SPACING,
        };
        let airfield = self.objects.get(&airfield_id)?;
        let metadata = airfield.thing.template.parking_place.as_ref()?;
        let spaces = self.airfield_parking_spaces.get(&airfield_id)?;
        let index = spaces.iter().position(|space| space.object_id == Some(jet_id))?;
        let (row, col) = Self::airfield_space_row_col(index, metadata)?;
        let num_cols = usize::try_from(metadata.num_cols).ok().filter(|&c| c > 0)?;
        let num_rows = usize::try_from(metadata.num_rows).ok().filter(|&r| r > 0)?;
        let origin = airfield.get_position();
        let mut forward = airfield.thing.get_direction_vector();
        forward.y = 0.0;
        if forward.length_squared() < 1.0e-6 {
            forward = glam::Vec3::new(0.0, 0.0, -1.0);
        } else {
            forward = forward.normalize();
        }
        let right = glam::Vec3::new(forward.z, 0.0, -forward.x);
        let deck = metadata.landing_deck_height_offset;
        let (hangar, hangar_orient) = Self::airfield_logical_bone_pose(
            origin, forward, right, col, row, num_cols, num_rows, deck,
            -PARKING_PLACE_RUNWAY_PREP_SPACING, 0.25,
        );
        let (parking, parking_orient) = Self::airfield_logical_bone_pose(
            origin, forward, right, col, row, num_cols, num_rows, deck, 0.0, 0.25,
        );
        let (prep, _) = Self::airfield_logical_bone_pose(
            origin, forward, right, col, row, num_cols, num_rows, deck,
            PARKING_PLACE_RUNWAY_PREP_SPACING * 0.5, 0.0,
        );
        let (runway_start, _) = Self::airfield_logical_bone_pose(
            origin, forward, right, col, 0, num_cols, 1, deck, 0.0, 0.0,
        );
        let (runway_end, _) = Self::airfield_logical_bone_pose(
            origin, forward, right, col, 0, num_cols, 1, deck,
            PARKING_PLACE_RUNWAY_PREP_SPACING * 2.0, 0.0,
        );
        let park_in_hangars = metadata.park_in_hangars;
        let mut info = HostAirfieldPPInfo {
            parking_space: if park_in_hangars { hangar } else { parking },
            parking_orientation: if park_in_hangars {
                hangar_orient
            } else {
                parking_orient
            },
            runway_prep: prep,
            runway_start,
            runway_end,
            runway_approach: runway_end
                + (runway_end - runway_start) * PARKING_PLACE_RUNWAY_APPROACH_DIST,
            hangar_internal: hangar,
            hangar_internal_orient: hangar_orient,
            runway_takeoff_dist: runway_start.distance(runway_end),
        };
        info.runway_approach.y =
            runway_end.y + metadata.approach_height + metadata.landing_deck_height_offset;
        if self
            .airfield_runway_was_in_line
            .get(&airfield_id)
            .and_then(|flags| flags.get(col))
            .copied()
            .unwrap_or(false)
            && self
                .runway_reservations
                .get(&airfield_id)
                .and_then(|slots| slots.get(col))
                .copied()
                .flatten()
                == Some(jet_id)
        {
            info.runway_start = info.runway_prep;
        }
        Some(info)
    }

    fn heli_park01_pose(&self, airfield_id: ObjectId) -> Option<glam::Vec3> {
        let airfield = self.objects.get(&airfield_id)?;
        let deck = airfield
            .thing
            .template
            .parking_place
            .as_ref()
            .map(|metadata| metadata.landing_deck_height_offset)
            .unwrap_or(0.0);
        let mut pos = airfield.get_position();
        pos.y += deck;
        Some(pos)
    }

    fn ensure_airfield_runway_queues(&mut self, airfield_id: ObjectId) -> Option<usize> {
        let runway_count = self
            .objects
            .get(&airfield_id)
            .filter(|object| Self::has_usable_airfield_parking_behavior(object))
            .and_then(|object| object.thing.template.parking_place.as_ref())
            .and_then(|metadata| metadata.runway_count())?;
        if runway_count == 0 {
            return None;
        }
        {
            let slots = self
                .runway_reservations
                .entry(airfield_id)
                .or_insert_with(|| vec![None; runway_count]);
            if slots.len() != runway_count {
                slots.resize(runway_count, None);
            }
        }
        {
            let next = self
                .airfield_runway_next_in_line
                .entry(airfield_id)
                .or_insert_with(|| vec![None; runway_count]);
            if next.len() != runway_count {
                next.resize(runway_count, None);
            }
        }
        {
            let was = self
                .airfield_runway_was_in_line
                .entry(airfield_id)
                .or_insert_with(|| vec![false; runway_count]);
            if was.len() != runway_count {
                was.resize(runway_count, false);
            }
        }
        Some(runway_count)
    }

    fn airfield_parking_runway_column(
        &self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<usize> {
        let spaces = self.airfield_parking_spaces.get(&airfield_id)?;
        let index = spaces
            .iter()
            .position(|space| space.object_id == Some(jet_id))?;
        let cols = self
            .objects
            .get(&airfield_id)
            .and_then(|object| object.thing.template.parking_place.as_ref())
            .and_then(|metadata| usize::try_from(metadata.num_cols).ok())
            .filter(|&cols| cols > 0)?;
        Some(index % cols)
    }

    fn jet_holds_airfield_runway(&self, airfield_id: ObjectId, jet_id: ObjectId) -> bool {
        self.runway_reservations
            .get(&airfield_id)
            .is_some_and(|slots| slots.iter().any(|slot| *slot == Some(jet_id)))
    }

    fn jet_is_above_terrain(jet: &Object) -> bool {
        jet.status.airborne_target || jet.get_position().y > 5.0
    }

    fn sync_airfield_hangar_doors(&mut self, airfield_id: ObjectId) {
        let hold = self
            .airfield_parking_spaces
            .get(&airfield_id)
            .is_some_and(|spaces| {
                spaces
                    .iter()
                    .any(|space| space.object_id.is_some() || space.reserved_for_exit)
            });
        let frame = self.frame;
        if let Some(airfield) = self.objects.get_mut(&airfield_id) {
            airfield.set_production_door_hold_open(hold, frame);
        }
    }

    fn apply_pending_helipad_exits(&mut self) {
        let pending: Vec<(ObjectId, ObjectId)> = self
            .airfield_pending_helipad_exits
            .drain()
            .collect();
        for (jet_id, airfield_id) in pending {
            let Some(heli) = self.heli_park01_pose(airfield_id) else {
                continue;
            };
            let rally = self
                .objects
                .get(&airfield_id)
                .and_then(|airfield| airfield.building_data.as_ref())
                .and_then(|building| building.rally_point);
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                if !jet.is_alive() || !Self::object_is_produced_at_helipad(jet) {
                    continue;
                }
                jet.set_position(heli);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    crate::game_logic::host_move_log::record(jet_id, Some([heli.x, heli.y, heli.z]));
                    jet.record_host_movement();
                }
            }
            let dest = rally.unwrap_or(heli);
            let _ = self.assign_unit_path(jet_id, dest, &[]);
        }
    }

    /// C++ `ParkingPlaceBehavior::killAllParkedUnits`.
    fn kill_all_parked_units(&mut self, airfield_id: ObjectId) {
        let Some(spaces) = self.airfield_parking_spaces.get(&airfield_id).cloned() else {
            return;
        };
        let mut kill = Vec::new();
        for space in &spaces {
            let Some(jet_id) = space.object_id else {
                continue;
            };
            let Some(jet) = self.objects.get(&jet_id) else {
                continue;
            };
            if !jet.is_alive() {
                continue;
            }
            let takeoff_or_landing = self.jet_holds_airfield_runway(airfield_id, jet_id);
            if Self::jet_is_above_terrain(jet) && !takeoff_or_landing {
                continue;
            }
            kill.push(jet_id);
        }
        for jet_id in kill {
            self.destroy_object(jet_id);
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            self.release_airfield_runway_for_jet(jet_id);
        }
    }

    /// C++ `ParkingPlaceBehavior::defectAllParkedUnits`.
    fn defect_all_parked_units(&mut self, airfield_id: ObjectId) {
        use crate::game_logic::host_defection_helper::DEFAULT_DEFECTION_PROTECTION_FRAMES;
        let Some(spaces) = self.airfield_parking_spaces.get(&airfield_id).cloned() else {
            return;
        };
        let Some((new_team, new_owner)) = self.objects.get(&airfield_id).map(|airfield| {
            (airfield.team, airfield.owner_player_id)
        }) else {
            return;
        };
        let now = self.frame;
        let mut release = Vec::new();
        for space in &spaces {
            let Some(jet_id) = space.object_id else {
                continue;
            };
            let Some(jet) = self.objects.get(&jet_id) else {
                continue;
            };
            if !jet.is_alive() {
                continue;
            }
            let takeoff_or_landing = self.jet_holds_airfield_runway(airfield_id, jet_id);
            if Self::jet_is_above_terrain(jet) && !takeoff_or_landing {
                let owner_differs = jet.team != new_team || jet.owner_player_id != new_owner;
                if owner_differs {
                    release.push(jet_id);
                }
                continue;
            }
            if jet.team == new_team && jet.owner_player_id == new_owner {
                continue;
            }
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.defect(new_team, now, DEFAULT_DEFECTION_PROTECTION_FRAMES);
                jet.owner_player_id = new_owner;
            }
        }
        for jet_id in release {
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                if jet.producer_id == Some(airfield_id) {
                    jet.producer_id = None;
                }
            }
        }
    }

    fn tick_airfield_parking_lifecycle(&mut self) {
        self.apply_pending_helipad_exits();
        let mut airfields: Vec<ObjectId> = self.airfield_parking_spaces.keys().copied().collect();
        for (&id, object) in self.objects.iter() {
            if object.thing.template.parking_place.is_some() {
                airfields.push(id);
            }
        }
        airfields.sort_by_key(|id| id.0);
        airfields.dedup();
        for airfield_id in airfields {
            let dead_or_sold = self
                .objects
                .get(&airfield_id)
                .map(|object| {
                    !object.is_alive()
                        || object.status.destroyed
                        || object.status.sold
                        || object.status.effectively_dead
                })
                .unwrap_or(true);
            if dead_or_sold {
                self.kill_all_parked_units(airfield_id);
            } else {
                self.defect_all_parked_units(airfield_id);
            }
        }
    }

    /// Exact C++ `getPP` eligibility for the compact host path.  `FSAirfield`
    /// by itself is deliberately insufficient: C++ finds a real
    /// `ParkingPlaceBehaviorInterface`, and the host must have its authored
    /// module data before it can reserve a space.
    #[inline]
    fn has_usable_airfield_parking_behavior(object: &Object) -> bool {
        object.is_alive()
            && !object.status.destroyed
            && !object.status.under_construction
            && !object.status.sold
            && object.is_kind_of(KindOf::FSAirfield)
            && object
                .thing
                .template
                .parking_place
                .as_ref()
                .and_then(|metadata| metadata.capacity())
                .is_some_and(|capacity| capacity > 0)
    }

    /// C++ `ActionManager::canEnterObject` uses a controller-exact check for
    /// a jet entering its own airfield.  JetAI's later `findSuitableAirfield`
    /// search may use an explicitly allied airfield, but never a merely
    /// same-faction or unproven-owner object.
    fn airfield_has_exact_controller_for_jet(
        &self,
        jet_id: ObjectId,
        airfield_id: ObjectId,
    ) -> bool {
        let (Some(jet), Some(airfield)) =
            (self.objects.get(&jet_id), self.objects.get(&airfield_id))
        else {
            return false;
        };
        match (jet.owner_player_id, airfield.owner_player_id) {
            (Some(jet_owner), Some(airfield_owner)) => {
                jet_owner == airfield_owner
                    && self.player_owner_for_host_object(jet) == Some(jet_owner)
                    && self.player_owner_for_host_object(airfield) == Some(airfield_owner)
            }
            // Producer preference deliberately does not infer an exact owner
            // from a faction.  A pre-owner-save aircraft can use the explicit
            // relationship fallback below, never an arbitrary producer id.
            _ => false,
        }
    }

    /// JetAI's fallback `findSuitableAirfield` predicate.  This is separate
    /// from the producer-first controller check above: `ALLOW_ALLIES` is an
    /// explicit player relationship, not a matching USA/China/GLA faction.
    pub(crate) fn is_friendly_airfield(&self, jet_id: ObjectId, airfield_id: ObjectId) -> bool {
        let (Some(jet), Some(airfield)) =
            (self.objects.get(&jet_id), self.objects.get(&airfield_id))
        else {
            return false;
        };
        Self::has_usable_airfield_parking_behavior(airfield)
            && self.normal_enter_relationship(jet, airfield)
                == gamelogic::common::Relationship::Allies
    }

    /// Authored C++ `ParkingPlaceBehavior` capacity for exactly one object.
    /// No template-name or retail `2 × 2` fallback is permitted here.
    pub(crate) fn airfield_parking_capacity(&self, airfield_id: ObjectId) -> Option<usize> {
        self.objects
            .get(&airfield_id)
            .filter(|object| Self::has_usable_airfield_parking_behavior(object))
            .and_then(|object| object.thing.template.parking_place.as_ref())
            .and_then(|metadata| metadata.capacity())
    }

    /// Rebuild/clean the host's exact parking reservation table from its
    /// persistent per-jet slot index.  Old snapshots that predate that field
    /// can only recover a reservation when the jet is physically marked as
    /// contained by this exact airfield; arbitrary garrison rosters are never
    /// treated as ParkingPlace state.
    fn normalize_airfield_parking_spaces(&mut self, airfield_id: ObjectId) -> bool {
        let Some(capacity) = self.airfield_parking_capacity(airfield_id) else {
            self.airfield_parking_spaces.remove(&airfield_id);
            return false;
        };

        let mut spaces = self
            .airfield_parking_spaces
            .remove(&airfield_id)
            .filter(|spaces| spaces.len() == capacity)
            .unwrap_or_else(|| vec![AirfieldParkingSpace::default(); capacity]);

        // Clear an entry once its owner no longer records this exact parking
        // slot.  C++ `releaseSpace` is explicit; producer identity alone is
        // not enough because a jet keeps its producer after takeoff.
        for (index, space) in spaces.iter_mut().enumerate() {
            let keep = space.object_id.is_some_and(|jet_id| {
                self.objects.get(&jet_id).is_some_and(|jet| {
                    jet.is_alive()
                        && Self::is_aircraft(jet)
                        && jet.producer_id == Some(airfield_id)
                        && jet.airfield_parking_space_index == u32::try_from(index).ok()
                })
            });
            if !keep {
                *space = AirfieldParkingSpace::default();
            }
        }

        // Restore live reservations from the persistent pair written by
        // `reserveSpace`: this covers both parked jets and en-route jets
        // after a save/load, where the in-memory table itself is rebuilt.
        // A malformed duplicate index is resolved deterministically and the
        // later claimant is left unreserved rather than silently sharing a
        // C++ ParkingPlace slot.
        let mut persisted_reservations: Vec<(ObjectId, usize)> = self
            .objects
            .iter()
            .filter_map(|(&jet_id, jet)| {
                (jet.is_alive() && Self::is_aircraft(jet) && jet.producer_id == Some(airfield_id))
                    .then(|| {
                        jet.airfield_parking_space_index
                            .and_then(|index| usize::try_from(index).ok())
                            .filter(|&index| index < capacity)
                            .map(|index| (jet_id, index))
                    })
                    .flatten()
            })
            .collect();
        persisted_reservations.sort_by_key(|(jet_id, _)| jet_id.0);
        let mut conflicting_claimants = Vec::new();
        for (jet_id, index) in persisted_reservations {
            if spaces[index].object_id.is_none() || spaces[index].object_id == Some(jet_id) {
                spaces[index].object_id = Some(jet_id);
            } else {
                conflicting_claimants.push(jet_id);
            }
        }
        for jet_id in conflicting_claimants {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.airfield_parking_space_index = None;
            }
        }

        // Older save/runtime records did not retain the slot index.  Their
        // exact `contained_by` link is an active parked state, so recover the
        // first free source-authored space deterministically by ObjectId.
        let mut legacy_parked: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(&jet_id, jet)| {
                (jet.is_alive()
                    && Self::is_aircraft(jet)
                    && jet.contained_by == Some(airfield_id)
                    && jet.airfield_parking_space_index.is_none())
                .then_some(jet_id)
            })
            .collect();
        legacy_parked.sort_by_key(|id| id.0);
        let mut recovered = Vec::new();
        for jet_id in legacy_parked {
            let Some(index) = spaces
                .iter()
                .position(|space| space.object_id.is_none() && !space.reserved_for_exit)
            else {
                break;
            };
            spaces[index].object_id = Some(jet_id);
            recovered.push((jet_id, index));
        }
        for (jet_id, index) in recovered {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.producer_id = Some(airfield_id);
                jet.airfield_parking_space_index = u32::try_from(index).ok();
            }
        }

        self.airfield_parking_spaces.insert(airfield_id, spaces);
        true
    }

    /// C++ `ParkingPlaceBehavior::hasReservedSpace` mirrored against the
    /// actual authored-space reservation, never a building garrison list.
    fn airfield_has_reserved_space(&self, airfield_id: ObjectId, jet_id: ObjectId) -> bool {
        self.airfield_parking_spaces
            .get(&airfield_id)
            .is_some_and(|spaces| spaces.iter().any(|space| space.object_id == Some(jet_id)))
    }

    /// C++ `ParkingPlaceBehavior::reserveSpace`.  The selected slot is stored
    /// on both sides of the relation: the airfield table and the aircraft's
    /// persistent `producer_id` + parking-space index.
    fn reserve_airfield_parking_space(
        &mut self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<usize> {
        if !self.normalize_airfield_parking_spaces(airfield_id) {
            return None;
        }
        let existing = self
            .airfield_parking_spaces
            .get(&airfield_id)
            .and_then(|spaces| {
                spaces
                    .iter()
                    .position(|space| space.object_id == Some(jet_id))
            });
        let index = match existing {
            Some(index) => index,
            None => {
                let spaces = self.airfield_parking_spaces.get_mut(&airfield_id)?;
                let index = spaces
                    .iter()
                    .position(|space| space.object_id.is_none() && !space.reserved_for_exit)?;
                spaces[index].object_id = Some(jet_id);
                index
            }
        };
        let slot_index = u32::try_from(index).ok()?;
        let jet = self.objects.get_mut(&jet_id)?;
        if !Self::is_aircraft(jet) || !jet.is_alive() {
            return None;
        }
        jet.producer_id = Some(airfield_id);
        jet.airfield_parking_space_index = Some(slot_index);
        self.sync_airfield_hangar_doors(airfield_id);
        Some(index)
    }

    /// Authored `NumRows × NumCols` stall count (C++ ParkingPlaceInfo length).
    fn authored_parking_capacity(metadata: &crate::game_logic::ParkingPlaceMetadata) -> usize {
        let rows = usize::try_from(metadata.num_rows).unwrap_or(0);
        let cols = usize::try_from(metadata.num_cols).unwrap_or(0);
        rows.saturating_mul(cols)
    }

    /// Free stalls that C++ `reserveSpace` / `DOOR_NONE_AVAILABLE` would accept.
    pub(in super::super) fn count_free_airfield_parking_slots(
        spaces: Option<&[AirfieldParkingSpace]>,
        capacity: usize,
    ) -> usize {
        match spaces {
            Some(spaces) if !spaces.is_empty() => spaces
                .iter()
                .filter(|space| space.object_id.is_none() && !space.reserved_for_exit)
                .count(),
            _ => capacity,
        }
    }

    /// C++ `ParkingPlaceBehavior::exitObjectViaDoor` hangar/parking bone pose.
    pub(in super::super) fn place_produced_jet_at_parking_pose(
        &mut self,
        producer_id: ObjectId,
        jet_id: ObjectId,
    ) -> bool {
        let Some(info) = self.calc_airfield_pp_info(producer_id, jet_id) else {
            return false;
        };
        let Some(jet) = self.objects.get_mut(&jet_id) else {
            return false;
        };
        jet.set_position(info.parking_space);
        jet.set_orientation(info.parking_orientation);
        jet.set_contained_by(Some(producer_id));
        jet.set_ai_state(AIState::Docked);
        jet.set_status_moving(false);
        jet.status.airborne_target = false;
        jet.movement.path.clear();
        jet.movement.current_path_index = 0;
        jet.movement.target_position = None;
        if crate::gameworld_shadow::gameworld_movement_authority_live() {
            crate::game_logic::host_move_log::record(
                jet_id,
                Some([
                    info.parking_space.x,
                    info.parking_space.y,
                    info.parking_space.z,
                ]),
            );
            jet.record_host_movement();
        }
        self.sync_airfield_hangar_doors(producer_id);
        true
    }


    /// C++ `ParkingPlaceBehavior::exitObjectViaDoor` for a completed factory
    /// aircraft: its producer link is only a landing authority when the
    /// output was assigned one real authored parking space under the exact
    /// same controller.  The new jet remains outside generic containment and
    /// follows the ordinary production exit path while retaining the slot.
    pub(in super::super) fn reserve_produced_aircraft_parking_space(
        &mut self,
        producer_id: ObjectId,
        jet_id: ObjectId,
    ) -> bool {
        if !self
            .objects
            .get(&producer_id)
            .is_some_and(Self::has_usable_airfield_parking_behavior)
        {
            return false;
        }
        let produced_at_helipad = self
            .objects
            .get(&jet_id)
            .is_some_and(Self::object_is_produced_at_helipad);
        if produced_at_helipad {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.producer_id = Some(producer_id);
                jet.airfield_parking_space_index = None;
            }
            self.airfield_pending_helipad_exits
                .insert(jet_id, producer_id);
            return true;
        }
        self.airfield_has_exact_controller_for_jet(jet_id, producer_id)
            && self
                .reserve_airfield_parking_space(producer_id, jet_id)
                .is_some()
    }

    /// C++ `ParkingPlaceBehavior::releaseSpace` for every reservation held by
    /// this aircraft.  Producer identity intentionally survives the release,
    /// matching JetAI's producer-first future landing preference.
    fn release_airfield_parking_space_for_jet(&mut self, jet_id: ObjectId) -> Option<ObjectId> {
        let airfields: Vec<ObjectId> = self.airfield_parking_spaces.keys().copied().collect();
        let mut released_from = None;
        for airfield_id in airfields {
            let Some(spaces) = self.airfield_parking_spaces.get_mut(&airfield_id) else {
                continue;
            };
            let mut released_here = false;
            for space in spaces.iter_mut() {
                if space.object_id == Some(jet_id) {
                    *space = AirfieldParkingSpace::default();
                    released_here = true;
                }
            }
            if released_here {
                released_from = Some(airfield_id);
            }
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.airfield_parking_space_index = None;
        }
        if let Some(airfield_id) = released_from {
            self.sync_airfield_hangar_doors(airfield_id);
        }
        released_from
    }

    /// Count currently parked jets from the actual ParkingPlace reservation
    /// records.  En-route reservations remain occupied but are intentionally
    /// excluded from this parked-only view.
    pub(crate) fn airfield_parked_count(&self, airfield_id: ObjectId) -> usize {
        self.airfield_parking_spaces
            .get(&airfield_id)
            .map(|spaces| {
                spaces
                    .iter()
                    .filter_map(|space| space.object_id)
                    .filter(|jet_id| {
                        self.objects.get(jet_id).is_some_and(|jet| {
                            jet.is_alive()
                                && Self::is_aircraft(jet)
                                && jet.contained_by == Some(airfield_id)
                        })
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    /// Ensure runway slots are sized from the same authored ParkingPlace
    /// metadata.  `HasRunways = No` yields no synthetic runway.
    pub(crate) fn airfield_runway_slots_mut(
        &mut self,
        airfield_id: ObjectId,
    ) -> Option<&mut Vec<Option<ObjectId>>> {
        let runway_count = self
            .objects
            .get(&airfield_id)
            .filter(|object| Self::has_usable_airfield_parking_behavior(object))
            .and_then(|object| object.thing.template.parking_place.as_ref())
            .and_then(|metadata| metadata.runway_count())?;
        if runway_count == 0 {
            return None;
        }
        let slots = self
            .runway_reservations
            .entry(airfield_id)
            .or_insert_with(|| vec![None; runway_count]);
        if slots.len() != runway_count {
            slots.resize(runway_count, None);
            slots.truncate(runway_count);
        }
        Some(slots)
    }

    /// C++ ParkingPlaceBehavior::transferRunwayReservationToNext / reserve residual.
    ///
    /// Returns runway index when a free runway is reserved for `jet_id`.
    pub(crate) fn reserve_airfield_runway(
        &mut self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<usize> {
        self.reserve_airfield_runway_ex(airfield_id, jet_id, false)
    }

    /// C++ `ParkingPlaceBehavior::reserveRunway(id, forLanding)`.
    fn reserve_airfield_runway_ex(
        &mut self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
        for_landing: bool,
    ) -> Option<usize> {
        let _ = self.ensure_airfield_runway_queues(airfield_id)?;
        if let Some(slots) = self.runway_reservations.get(&airfield_id) {
            if let Some(idx) = slots.iter().position(|s| *s == Some(jet_id)) {
                return Some(idx);
            }
        }
        let column = self
            .airfield_parking_runway_column(airfield_id, jet_id)
            .or_else(|| {
                self.runway_reservations
                    .get(&airfield_id)
                    .and_then(|slots| slots.iter().position(|s| s.is_none()))
            })?;
        let in_use = self
            .runway_reservations
            .get(&airfield_id)
            .and_then(|slots| slots.get(column))
            .copied()
            .flatten();
        if in_use == Some(jet_id) {
            return Some(column);
        }
        if in_use.is_none() {
            if let Some(slots) = self.runway_reservations.get_mut(&airfield_id) {
                if let Some(slot) = slots.get_mut(column) {
                    *slot = Some(jet_id);
                }
            }
            let was_in_line = self
                .airfield_runway_next_in_line
                .get(&airfield_id)
                .and_then(|next| next.get(column))
                .copied()
                .flatten()
                == Some(jet_id);
            if let Some(next) = self.airfield_runway_next_in_line.get_mut(&airfield_id) {
                if let Some(slot) = next.get_mut(column) {
                    if *slot == Some(jet_id) {
                        *slot = None;
                    }
                }
            }
            if let Some(was) = self.airfield_runway_was_in_line.get_mut(&airfield_id) {
                if let Some(flag) = was.get_mut(column) {
                    *flag = was_in_line;
                }
            }
            return Some(column);
        }
        if !for_landing {
            let next_free = self
                .airfield_runway_next_in_line
                .get(&airfield_id)
                .and_then(|next| next.get(column))
                .copied()
                .flatten()
                .is_none();
            if next_free {
                if let Some(next) = self.airfield_runway_next_in_line.get_mut(&airfield_id) {
                    if let Some(slot) = next.get_mut(column) {
                        *slot = Some(jet_id);
                    }
                }
            }
        }
        None
    }

    /// Release any runway held by this jet (all airfields).
    pub(crate) fn release_airfield_runway_for_jet(&mut self, jet_id: ObjectId) {
        for slots in self.runway_reservations.values_mut() {
            for s in slots.iter_mut() {
                if *s == Some(jet_id) {
                    *s = None;
                }
            }
        }
        for next in self.airfield_runway_next_in_line.values_mut() {
            for s in next.iter_mut() {
                if *s == Some(jet_id) {
                    *s = None;
                }
            }
        }
        for (airfield_id, slots) in self.runway_reservations.iter() {
            if let Some(was) = self.airfield_runway_was_in_line.get_mut(airfield_id) {
                for (idx, holder) in slots.iter().enumerate() {
                    if holder.is_none() {
                        if let Some(flag) = was.get_mut(idx) {
                            *flag = false;
                        }
                    }
                }
            }
        }
    }

    /// Count reserved runways at airfield.
    pub(crate) fn airfield_runway_reserved_count(&self, airfield_id: ObjectId) -> usize {
        self.runway_reservations
            .get(&airfield_id)
            .map(|s| s.iter().filter(|x| x.is_some()).count())
            .unwrap_or(0)
    }

    /// C++ JetAIUpdate runway takeoff residual: reserve runway, taxi to prep, then climb.
    ///
    /// Returns false when HasRunways and all runways are busy (jet stays docked).
    pub(crate) fn try_runway_takeoff_from_airfield(&mut self, jet_id: ObjectId) -> bool {
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_PREP_SPACING;
        let (af_id, metadata, airfield_position) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            if !Self::is_aircraft(jet) {
                return false;
            }
            let parked = jet.is_parked_at_airfield() || jet.contained_by.is_some();
            if !parked {
                // Already free — still clear any stale reservation.
                self.release_airfield_runway_for_jet(jet_id);
                return true;
            }
            let Some(af_id) = jet.contained_by else {
                return false;
            };
            let Some(airfield) = self
                .objects
                .get(&af_id)
                .filter(|airfield| Self::has_usable_airfield_parking_behavior(airfield))
            else {
                return false;
            };
            let Some(metadata) = airfield.thing.template.parking_place.clone() else {
                return false;
            };
            (af_id, metadata, airfield.get_position())
        };
        if !metadata.has_runways || metadata.runway_count().unwrap_or(0) == 0 {
            let released = self.release_jet_from_airfield_parking(jet_id);
            if released {
                if let Some(jet) = self.objects.get_mut(&jet_id) {
                    let mut position = jet.get_position();
                    position.y = position.y.max(
                        airfield_position.y
                            + metadata.landing_deck_height_offset
                            + metadata.approach_height,
                    );
                    jet.set_position(position);
                }
            }
            return released;
        }
        let Some(runway_idx) = self.reserve_airfield_runway(af_id, jet_id) else {
            // All runways busy — remain docked this frame.
            return false;
        };
        let ppinfo = self.calc_airfield_pp_info(af_id, jet_id);
        let taxi = if let Some(info) = ppinfo {
            vec![
                info.runway_prep,
                info.runway_start,
                {
                    let mut end = info.runway_end;
                    end.y = end.y.max(info.runway_approach.y);
                    end
                },
            ]
        } else {
            let runway_count = metadata.runway_count().unwrap_or(1);
            let mut prep = airfield_position;
            prep.x += (runway_idx as f32 - (runway_count.saturating_sub(1) as f32 * 0.5))
                * PARKING_PLACE_RUNWAY_PREP_SPACING;
            prep.y += metadata.landing_deck_height_offset + metadata.approach_height;
            vec![prep]
        };
        let dest = *taxi.last().unwrap_or(&airfield_position);
        let via: Vec<glam::Vec3> = taxi.iter().copied().take(taxi.len().saturating_sub(1)).collect();
        let ok = self.release_jet_from_airfield_parking(jet_id);
        if !ok {
            self.release_airfield_runway_for_jet(jet_id);
            return false;
        }
        let _ = self.reserve_airfield_runway(af_id, jet_id);
        if !self.assign_unit_path(jet_id, dest, &via) {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.movement.path = taxi.clone();
                jet.movement.current_path_index = 0;
                jet.movement.target_position = Some(dest);
                jet.set_ai_state(AIState::Moving);
                jet.set_status_moving(true);
            }
        }
        true

    }

    /// Release runway reservations once jets are clear of the airfield.
    pub(crate) fn tick_airfield_runway_clear(&mut self) {
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_CLEAR_DIST;
        let clear_sq = PARKING_PLACE_RUNWAY_CLEAR_DIST * PARKING_PLACE_RUNWAY_CLEAR_DIST;
        let mut to_clear: Vec<(ObjectId, usize, bool)> = Vec::new();
        let airfields: Vec<ObjectId> = self.runway_reservations.keys().copied().collect();
        for af_id in airfields {
            let af_pos = match self.objects.get(&af_id) {
                Some(o) if Self::has_usable_airfield_parking_behavior(o) => o.get_position(),
                _ => {
                    self.runway_reservations.remove(&af_id);
                    self.airfield_runway_next_in_line.remove(&af_id);
                    self.airfield_runway_was_in_line.remove(&af_id);
                    continue;
                }
            };
            let Some(slots) = self.runway_reservations.get(&af_id) else {
                continue;
            };
            for (idx, holder) in slots.iter().enumerate() {
                let Some(jet_id) = *holder else {
                    continue;
                };
                let (clear, dead) = match self.objects.get(&jet_id) {
                    None => (true, true),
                    Some(jet) if !jet.is_alive() => (true, true),
                    Some(jet) if jet.contained_by.is_some() => (true, false),
                    Some(jet) => {
                        let p = jet.get_position();
                        let dx = p.x - af_pos.x;
                        let dz = p.z - af_pos.z;
                        let d2 = dx * dx + dz * dz;
                        (jet.status.airborne_target && d2 >= clear_sq, false)
                    }
                };
                if clear {
                    to_clear.push((af_id, idx, dead));
                }
            }
        }
        for (af_id, idx, dead) in to_clear {
            let next = self
                .airfield_runway_next_in_line
                .get(&af_id)
                .and_then(|next| next.get(idx))
                .copied()
                .flatten()
                .filter(|_| !dead);
            if let Some(slots) = self.runway_reservations.get_mut(&af_id) {
                if idx < slots.len() {
                    slots[idx] = next;
                }
            }
            if let Some(queue) = self.airfield_runway_next_in_line.get_mut(&af_id) {
                if idx < queue.len() {
                    queue[idx] = None;
                }
            }
            if let Some(was) = self.airfield_runway_was_in_line.get_mut(&af_id) {
                if idx < was.len() {
                    was[idx] = next.is_some();
                }
            }
        }
    }

    /// C++ JetAIUpdate leave ParkingPlace residual (clear hangar slot + takeoff).
    pub(crate) fn release_jet_from_airfield_parking(&mut self, jet_id: ObjectId) -> bool {
        let af_hint = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            jet.contained_by.or(jet.producer_id)
        };
        let af_hint = af_hint.or_else(|| {
            self.airfield_parking_spaces
                .iter()
                .find_map(|(&airfield_id, spaces)| {
                    spaces
                        .iter()
                        .any(|space| space.object_id == Some(jet_id))
                        .then_some(airfield_id)
                })
        });
        let took_off = {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                return false;
            };
            let was_parked = jet.is_parked_at_airfield() || jet.contained_by.is_some();
            let af = jet.takeoff_from_airfield_parking();
            jet.set_contained_by(None);
            was_parked || af.is_some()
        };
        let released_from = self.release_airfield_parking_space_for_jet(jet_id);
        if let Some(af_id) = released_from.or(af_hint) {
            self.sync_airfield_hangar_doors(af_id);
        }
        // Success if we launched or released an actual ParkingPlace space.
        took_off || released_from.is_some()
    }

    /// C++ `JetOrHeliReturnForLandingState::onEnter`: first use the producer
    /// parking place only when it still has the jet's exact controller.  If
    /// that concrete producer is absent/stale, scan only explicitly allied
    /// ParkingPlaceBehavior airfields and reserve a real authored space.
    fn select_and_reserve_airfield_for_return(&mut self, jet_id: ObjectId) -> Option<ObjectId> {
        let (producer_id, jet_position) = {
            let jet = self.objects.get(&jet_id)?;
            (jet.producer_id, jet.get_position())
        };

        if let Some(producer_id) = producer_id {
            let producer_is_usable = self
                .objects
                .get(&producer_id)
                .is_some_and(Self::has_usable_airfield_parking_behavior);
            if producer_is_usable && self.airfield_has_exact_controller_for_jet(jet_id, producer_id)
            {
                // A valid producer remains authoritative.  C++ does not
                // silently select another airfield merely because its own
                // ParkingPlaceBehavior cannot reserve a space this frame.
                return self
                    .reserve_airfield_parking_space(producer_id, jet_id)
                    .map(|_| producer_id);
            }

            // A previous `findSuitableAirfield` fallback writes that field
            // into producer_id too.  It is not upgraded into a same-owner
            // producer, but its already-reserved real ParkingPlace may remain
            // live while the explicit alliance remains valid.  Without this
            // branch every update would release/reacquire the same allied
            // space and could lose it to a queue between frames.
            if producer_is_usable
                && self.is_friendly_airfield(jet_id, producer_id)
                && self.airfield_has_reserved_space(producer_id, jet_id)
            {
                return Some(producer_id);
            }

            // C++ clears an unusable producer before `findSuitableAirfield`.
            // Release the old reservation first so a captured/dead airfield
            // cannot retain an unavailable aircraft slot.
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                if jet.producer_id == Some(producer_id) {
                    jet.producer_id = None;
                }
            }
        }

        let mut candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(&airfield_id, airfield)| {
                (airfield_id != jet_id
                    && Self::has_usable_airfield_parking_behavior(airfield)
                    && self.is_friendly_airfield(jet_id, airfield_id))
                .then(|| {
                    (
                        airfield_id,
                        airfield.get_position().distance_squared(jet_position),
                    )
                })
            })
            .collect();
        candidates.sort_by(|(left_id, left_distance), (right_id, right_distance)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left_id.0.cmp(&right_id.0))
        });
        candidates.into_iter().find_map(|(airfield_id, _)| {
            self.reserve_airfield_parking_space(airfield_id, jet_id)
                .map(|_| airfield_id)
        })
    }

    /// Physical ReturnToBase command authority.  The command executor passes
    /// a frozen player id, then this method revalidates that exact ownership
    /// before it writes a request or reserves an airfield slot.
    pub(crate) fn request_return_to_base(&mut self, jet_id: ObjectId, player_id: u32) -> bool {
        let can_request = self.objects.get(&jet_id).is_some_and(|jet| {
            jet.is_alive()
                && Self::is_aircraft(jet)
                && jet.owner_player_id == Some(player_id)
                && self.player_owner_for_host_object(jet) == Some(player_id)
        });
        if !can_request {
            return false;
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.return_to_base_requested = true;
        }
        if self.try_return_to_base_rearm(jet_id) {
            true
        } else {
            // A physical command that cannot reserve a C++-valid parking
            // place is not accepted optimistically.
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.return_to_base_requested = false;
            }
            false
        }
    }

    /// C++ JetAIUpdate return/land/rearm residual.  Every tick revalidates
    /// the selected producer or allied airfield before it can keep a parking
    /// reservation, so capture/destruction cannot route a jet into an enemy
    /// or same-faction other-player hangar.
    pub(crate) fn try_return_to_base_rearm(&mut self, jet_id: ObjectId) -> bool {
        let (needs_rearm, requested, jet_position) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            if !jet.is_alive() || !Self::is_aircraft(jet) {
                return false;
            }
            (
                jet.needs_return_to_base_rearm(),
                jet.return_to_base_requested,
                jet.get_position(),
            )
        };
        if !needs_rearm && !requested {
            return false;
        }

        let Some(airfield_id) = self.select_and_reserve_airfield_for_return(jet_id) else {
            return false;
        };
        let Some((metadata, airfield_position)) =
            self.objects.get(&airfield_id).and_then(|airfield| {
                Self::has_usable_airfield_parking_behavior(airfield)
                    .then(|| {
                        airfield
                            .thing
                            .template
                            .parking_place
                            .clone()
                            .map(|metadata| (metadata, airfield.get_position()))
                    })
                    .flatten()
            })
        else {
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            return false;
        };
        if !(self.airfield_has_exact_controller_for_jet(jet_id, airfield_id)
            || self.is_friendly_airfield(jet_id, airfield_id))
            || !self.airfield_has_reserved_space(airfield_id, jet_id)
        {
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            return false;
        }

        const REARM_RANGE: f32 = 120.0;
        let dx = jet_position.x - airfield_position.x;
        let dz = jet_position.z - airfield_position.z;
        if dx * dx + dz * dz > REARM_RANGE * REARM_RANGE {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.target = None;
                jet.set_status_attacking(false);
                jet.set_ai_state(AIState::Moving);
            }
            if self.assign_unit_path(jet_id, airfield_position, &[]) {
                // Keep the authenticated `ParkingPlaceBehavior` reservation
                // while JetAI is returning; capacity is not a garrison count.
                return true;
            }
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            return false;
        }

        let already_docked = self
            .objects
            .get(&jet_id)
            .is_some_and(|jet| jet.contained_by == Some(airfield_id));
        let runway_index = if already_docked || !metadata.has_runways {
            None
        } else {
            match self.reserve_airfield_runway_ex(airfield_id, jet_id, true) {
                Some(index) => Some(index),
                None => return self.airfield_has_reserved_space(airfield_id, jet_id),
            }
        };

        let ppinfo = self.calc_airfield_pp_info(airfield_id, jet_id);
        let pad = if let Some(info) = ppinfo {
            info.parking_space
        } else {
            use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_PREP_SPACING;
            let mut pad = airfield_position;
            if let Some(index) = runway_index {
                let runway_count = metadata.runway_count().unwrap_or(1);
                pad.x += (index as f32 - (runway_count.saturating_sub(1) as f32 * 0.5))
                    * PARKING_PLACE_RUNWAY_PREP_SPACING;
            }
            pad.y += metadata.landing_deck_height_offset;
            pad
        };
        let approach = ppinfo.map(|info| info.runway_approach).unwrap_or(pad);
        let dxp = jet_position.x - pad.x;
        let dyp = jet_position.y - pad.y;
        let dzp = jet_position.z - pad.z;
        const PAD_SNAP: f32 = 12.0;
        if dxp * dxp + dyp * dyp + dzp * dzp > PAD_SNAP * PAD_SNAP {
            let via = if (jet_position - approach).length_squared() > PAD_SNAP * PAD_SNAP {
                vec![approach]
            } else {
                Vec::new()
            };
            if self.assign_unit_path(jet_id, pad, &via) {
                return true;
            }
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.target = None;
                jet.set_status_attacking(false);
                jet.set_ai_state(AIState::Moving);
                jet.movement.path = via.into_iter().chain(std::iter::once(pad)).collect();
                jet.movement.current_path_index = 0;
                jet.movement.target_position = Some(pad);
                jet.set_status_moving(true);
            }
            return true;
        }
        {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                self.release_airfield_runway_for_jet(jet_id);
                return false;
            };
            jet.set_contained_by(Some(airfield_id));
            jet.set_ai_state(AIState::Docked);
            jet.set_status_moving(false);
            jet.status.airborne_target = false;
            jet.target = None;
            jet.movement.path.clear();
            jet.movement.current_path_index = 0;
            jet.movement.target_position = None;
            jet.set_position(pad);
            jet.return_to_base_requested = false;
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(jet_id, 12);
                crate::game_logic::host_move_log::record(jet_id, Some([pad.x, pad.y, pad.z]));
                jet.record_host_movement();
            }
            if jet.needs_return_to_base_rearm() && jet.airfield_rearm_ready_frame.is_none() {
                let frames = jet.airfield_rearm_clip_reload_frames();
                if frames > 0 {
                    jet.airfield_rearm_ready_frame = Some(self.frame.saturating_add(frames));
                }
            }
        }


        self.release_airfield_runway_for_jet(jet_id);
        self.sync_airfield_hangar_doors(airfield_id);

        if let Some(jet) = self.objects.get_mut(&jet_id) {
            if !jet.needs_return_to_base_rearm() {
                jet.airfield_rearm_ready_frame = None;
            } else if let Some(ready_frame) = jet.airfield_rearm_ready_frame {
                if self.frame >= ready_frame && jet.rearm_return_to_base_weapons() {
                    jet.airfield_rearm_ready_frame = None;
                }
            } else {
                let _ = jet.rearm_return_to_base_weapons();
            }
        }
        // A parked clip-reload in progress is still a successful RTB route.
        true
    }

    /// C++ ParkingPlaceBehavior heal residual for docked aircraft at airfields.
    pub(crate) fn tick_airfield_parking_heal(&mut self) {
        self.tick_airfield_parking_lifecycle();
        use crate::game_logic::host_countermeasures::aircraft_has_countermeasures_upgrade;
        // Docked jets with Countermeasures (ReloadTime=0 / MustReloadAtAirfield residual)
        // reload flares even when already at full HP.
        // Parking heal keys off the actual authored ParkingPlace reservation,
        // not a structure's generic garrison/occupant list.
        // Under AI_DECISION_AUTHORITY host AIState::Docked is writeback-owned; contained_by
        // is still host-local after RTB rearm.
        let jet_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft)
                    && o.contained_by.is_some()
                    && (o.ai_state == AIState::Docked
                        || crate::gameworld_shadow::gameworld_ai_decision_authority_live())
                    && (o.health.current + 1e-3 < o.health.maximum
                        || o.needs_return_to_base_rearm()
                        || aircraft_has_countermeasures_upgrade(&o.applied_upgrades))
            })
            .map(|(id, _)| *id)
            .collect();
        for jid in jet_ids {
            let (airfield_id, has_cm) = {
                let Some(jet) = self.objects.get(&jid) else {
                    continue;
                };
                let Some(af_id) = jet.contained_by else {
                    continue;
                };
                let has_cm = aircraft_has_countermeasures_upgrade(&jet.applied_upgrades);
                (af_id, has_cm)
            };
            if !self.normalize_airfield_parking_spaces(airfield_id)
                || !self.airfield_has_reserved_space(airfield_id, jid)
            {
                continue;
            }
            let heal = self
                .objects
                .get(&airfield_id)
                .and_then(|airfield| airfield.thing.template.parking_place.as_ref())
                .map(|metadata| metadata.heal_amount_per_second / LOGIC_FRAMES_PER_SECOND)
                .unwrap_or(0.0);
            if has_cm {
                // C++ JetAIUpdate → CountermeasuresBehaviorInterface::reloadCountermeasures
                // when landing / docked at airfield (ReloadTime=0 residual).
                self.countermeasures.reload_at_airfield(jid);
            }
            if let Some(jet) = self.objects.get_mut(&jid) {
                // Also top-up RTB ammo while parked (continuous rearm residual).
                if jet.needs_return_to_base_rearm() {
                    let _ = jet.rearm_return_to_base_weapons();
                }
                if heal > 0.0 && jet.health.current + 1e-3 < jet.health.maximum {
                    jet.heal(heal);
                }
            }
        }
    }
}
