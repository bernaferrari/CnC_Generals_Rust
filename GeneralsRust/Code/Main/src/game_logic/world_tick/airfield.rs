//! Host tick `impl GameLogic` — `airfield`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    /// Update combat for all objects.
    ///
    /// Fail-closed residual: uses secondary when present and selected by
    /// `Object::select_combat_weapon_slot` (prefer secondary vs structures when
    /// secondary damage is better; alternate secondary when primary not ready).
    /// Not full C++ AutoChoose / PreferredAgainst matrices.
    ///
    /// `pub(crate)` so residual/unit tests can exercise the fire path directly.
    /// C++ JetAIUpdate RETURN_TO_BASE residual: rearm empty jet weapons near
    /// a friendly airfield (FSAirfield / name residual). Fail-closed vs full
    /// ParkingPlace reserve / taxi matrix.
    /// C++ JetOrHeliCirclingDeadAirfieldState residual for all empty RTB jets.
    pub(crate) fn tick_out_of_ammo_jet_damage(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft)
                    && o.needs_return_to_base_rearm()
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if self.try_return_to_base_rearm(id) {
                continue;
            }
            if let Some(jet) = self.objects.get_mut(&id) {
                let _ = jet.apply_out_of_ammo_damage_frame();
            }
        }
    }

    /// C++ ParkingPlaceBehavior / JetAIUpdate airfield residual helper.
    pub(crate) fn is_friendly_airfield(obj: &Object, jet_team: Team) -> bool {
        if !obj.is_alive() || obj.team != jet_team || obj.status.under_construction {
            return false;
        }
        obj.is_kind_of(KindOf::FSAirfield)
            || (obj.is_kind_of(KindOf::Structure) && {
                let n = obj.template_name.to_ascii_lowercase();
                n.contains("airfield") || n.contains("airbase") || n.contains("hangar")
            })
    }

    /// Retail airfield parking capacity residual (NumRows 2 × NumCols 2 = 4).
    pub(crate) fn airfield_parking_capacity() -> usize {
        use crate::game_logic::host_dock_contain_exit_heal_residual::{
            PARKING_PLACE_AIRFIELD_NUM_COLS, PARKING_PLACE_AIRFIELD_NUM_ROWS,
        };
        (PARKING_PLACE_AIRFIELD_NUM_ROWS.max(0) as usize)
            * (PARKING_PLACE_AIRFIELD_NUM_COLS.max(0) as usize)
    }

    /// Count jets currently docked at this airfield (contained_by residual).
    pub(crate) fn airfield_parked_count(&self, airfield_id: ObjectId) -> usize {
        self.objects
            .values()
            .filter(|o| {
                o.is_alive()
                    && o.contained_by == Some(airfield_id)
                    && (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft)
            })
            .count()
    }

    /// Ensure runway slot vector sized for airfield (HasRunways residual).
    pub(crate) fn airfield_runway_slots_mut(
        &mut self,
        airfield_id: ObjectId,
    ) -> &mut Vec<Option<ObjectId>> {
        use crate::game_logic::host_dock_contain_exit_heal_residual::{
            airfield_runway_count, PARKING_PLACE_AIRFIELD_HAS_RUNWAYS,
            PARKING_PLACE_AIRFIELD_NUM_COLS,
        };
        let n = airfield_runway_count(
            PARKING_PLACE_AIRFIELD_HAS_RUNWAYS,
            PARKING_PLACE_AIRFIELD_NUM_COLS,
        );
        let slots = self
            .runway_reservations
            .entry(airfield_id)
            .or_insert_with(|| vec![None; n.max(1)]);
        if slots.len() < n {
            slots.resize(n, None);
        }
        slots
    }

    /// C++ ParkingPlaceBehavior::transferRunwayReservationToNext / reserve residual.
    ///
    /// Returns runway index when a free runway is reserved for `jet_id`.
    pub(crate) fn reserve_airfield_runway(
        &mut self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<usize> {
        // Already holding a runway?
        if let Some(slots) = self.runway_reservations.get(&airfield_id) {
            if let Some(idx) = slots.iter().position(|s| *s == Some(jet_id)) {
                return Some(idx);
            }
        }
        let slots = self.airfield_runway_slots_mut(airfield_id);
        if let Some(idx) = slots.iter().position(|s| s.is_none()) {
            slots[idx] = Some(jet_id);
            return Some(idx);
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
        use crate::game_logic::host_dock_contain_exit_heal_residual::{
            PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT, PARKING_PLACE_AIRFIELD_HAS_RUNWAYS,
            PARKING_PLACE_RUNWAY_PREP_SPACING,
        };
        let (af_id, team) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            if !(jet.is_kind_of(KindOf::Aircraft) || jet.object_type == ObjectType::Aircraft) {
                return false;
            }
            let parked = jet.is_parked_at_airfield() || jet.contained_by.is_some();
            if !parked {
                // Already free — still clear any stale reservation.
                self.release_airfield_runway_for_jet(jet_id);
                return true;
            }
            let af = jet.contained_by.or_else(|| {
                // hangar list residual
                None
            });
            (af, jet.team)
        };
        let Some(af_id) = af_id else {
            // No airfield link — fall through to legacy takeoff.
            let _ = self.release_jet_from_airfield_parking(jet_id);
            return true;
        };
        if !PARKING_PLACE_AIRFIELD_HAS_RUNWAYS {
            let _ = self.release_jet_from_airfield_parking(jet_id);
            return true;
        }
        let Some(runway_idx) = self.reserve_airfield_runway(af_id, jet_id) else {
            // All runways busy — remain docked this frame.
            return false;
        };
        // Taxi residual: offset prep position by runway index, then climb.
        if let Some(af) = self.objects.get(&af_id).map(|o| o.get_position()) {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                let mut prep = af;
                // Simplified two-runway layout along X.
                prep.x += (runway_idx as f32 - 0.5) * PARKING_PLACE_RUNWAY_PREP_SPACING;
                prep.y = PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
                // Host-immediate taxi prep snap; log for GameWorld pose last-write.
                jet.set_position(prep);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    crate::game_logic::host_move_log::record(
                        jet_id,
                        Some([prep.x, prep.y, prep.z]),
                    );
                    jet.record_host_movement();
                }
            }
        }
        let _ = team;
        let ok = self.release_jet_from_airfield_parking(jet_id);
        // Keep runway reserved until clear tick releases it.
        if !ok {
            // If release failed, free runway.
            self.release_airfield_runway_for_jet(jet_id);
            return false;
        }
        // Re-assert reservation after release (release shouldn't clear runway).
        let _ = self.reserve_airfield_runway(af_id, jet_id);
        true
    }

    /// Release runway reservations once jets are clear of the airfield.
    pub(crate) fn tick_airfield_runway_clear(&mut self) {
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_CLEAR_DIST;
        let clear_sq = PARKING_PLACE_RUNWAY_CLEAR_DIST * PARKING_PLACE_RUNWAY_CLEAR_DIST;
        let mut to_clear: Vec<(ObjectId, usize)> = Vec::new();
        let airfields: Vec<ObjectId> = self.runway_reservations.keys().copied().collect();
        for af_id in airfields {
            let af_pos = match self.objects.get(&af_id) {
                Some(o) if o.is_alive() => o.get_position(),
                _ => {
                    // Airfield gone — drop all.
                    self.runway_reservations.remove(&af_id);
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
                let clear = match self.objects.get(&jet_id) {
                    None => true,
                    Some(jet) if !jet.is_alive() => true,
                    Some(jet) if jet.contained_by.is_some() => true, // re-docked
                    Some(jet) => {
                        let p = jet.get_position();
                        let dx = p.x - af_pos.x;
                        let dz = p.z - af_pos.z;
                        let d2 = dx * dx + dz * dz;
                        // Clear once airborne and away, or no longer attacking from pad.
                        jet.status.airborne_target && d2 >= clear_sq
                    }
                };
                if clear {
                    to_clear.push((af_id, idx));
                }
            }
        }
        for (af_id, idx) in to_clear {
            if let Some(slots) = self.runway_reservations.get_mut(&af_id) {
                if idx < slots.len() {
                    slots[idx] = None;
                }
            }
        }
    }

    /// C++ JetAIUpdate leave ParkingPlace residual (clear hangar slot + takeoff).
    pub(crate) fn release_jet_from_airfield_parking(&mut self, jet_id: ObjectId) -> bool {
        let (team, af_hint) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            (jet.team, jet.contained_by)
        };
        let af_hint = af_hint.or_else(|| {
            self.objects.iter().find_map(|(id, o)| {
                if Self::is_friendly_airfield(o, team) && o.contained_units().contains(&jet_id) {
                    Some(*id)
                } else {
                    None
                }
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
        let mut freed = false;
        if let Some(af_id) = af_hint {
            if let Some(af) = self.objects.get_mut(&af_id) {
                freed = af.remove_occupant(jet_id);
            }
            // C++ releaseSpace → setHoldDoorOpen(door, false) when slot empty.
            // Fail-closed: clear hold when airfield has no remaining parked jets.
            let still_parked = self.airfield_parked_count(af_id) > 0;
            if let Some(af) = self.objects.get_mut(&af_id) {
                if !still_parked {
                    af.set_production_door_hold_open(false, self.frame);
                }
            }
        }
        // Success if we launched, or cleaned a lingering parking slot, or jet is free.
        took_off || freed || af_hint.is_some()
    }

    pub(crate) fn try_return_to_base_rearm(&mut self, jet_id: ObjectId) -> bool {
        let (needs, jet_team, jet_pos) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            if !jet.needs_return_to_base_rearm() {
                return false;
            }
            (true, jet.team, jet.get_position())
        };
        if !needs {
            return false;
        }
        // C++ JetAIUpdate final dock / rearm proximity residual.
        const REARM_RANGE: f32 = 120.0;
        // C++ seek-airfield path residual (map-wide).
        const RTB_SEEK_RANGE: f32 = 50_000.0;
        let candidates: Vec<_> = self
            .objects
            .values()
            .filter(|obj| obj.id != jet_id && Self::is_friendly_airfield(obj, jet_team))
            .map(
                |obj| crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: obj.id,
                    team: obj.team,
                    position: obj.get_position(),
                    is_alive: obj.is_alive(),
                    is_neutral: obj.team == Team::Neutral,
                    under_construction: obj.status.under_construction,
                    combat_kind: true,
                    effectively_stealthed: false,
                    is_air: false,
                    eject_invulnerable: false,
                },
            )
            .collect();
        // Prefer in-range airfield for immediate dock; else nearest map-wide for RTB path.
        let in_range = crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            jet_id,
            jet_team,
            jet_pos,
            candidates.iter().cloned(),
            |_| REARM_RANGE,
            |_| true,
        );
        let af_id = if let Some((id, _, _)) = in_range {
            id
        } else {
            let Some((id, _, _)) =
                crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
                    jet_id,
                    jet_team,
                    jet_pos,
                    candidates,
                    |_| RTB_SEEK_RANGE,
                    |_| true,
                )
            else {
                return false;
            };
            // C++ JetAIUpdate RETURN_TO_BASE path residual: fly toward airfield.
            let af_pos = self
                .objects
                .get(&id)
                .map(|o| o.get_position())
                .unwrap_or(jet_pos);
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.target = None;
                jet.set_status_attacking(false);
                if !matches!(jet.ai_state, AIState::Moving | AIState::Docked) {
                    jet.set_ai_state(AIState::Moving);
                }
            }
            let _ = self.assign_unit_path(jet_id, af_pos, &[]);
            // Not docked yet — suppress out-of-ammo damage while RTB en route.
            return true;
        };
        let af_pos = self
            .objects
            .get(&af_id)
            .map(|o| o.get_position())
            .unwrap_or(jet_pos);
        // Capacity residual: refuse dock if parking places full (unless already parked here).
        let already = self
            .objects
            .get(&jet_id)
            .map(|j| j.contained_by == Some(af_id))
            .unwrap_or(false);
        if !already {
            let parked = self.airfield_parked_count(af_id);
            if parked >= Self::airfield_parking_capacity() {
                return false;
            }
        }
        // C++ HasRunways landing residual: need a free runway to final-approach dock.
        // Jets already parked here skip the runway gate.
        let runway_idx = if already {
            None
        } else {
            match self.reserve_airfield_runway(af_id, jet_id) {
                Some(i) => Some(i),
                None => return false, // hold off RTB dock until a runway frees
            }
        };
        // C++ JetAIUpdate RETURN_TO_BASE + Weapon ClipReload airfield rearm residual:
        // dock immediately, wait ClipReload frames, then restore ammo.
        {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                self.release_airfield_runway_for_jet(jet_id);
                return false;
            };
            // C++ setProducer + park residual: dock at airfield hangar.
            jet.set_contained_by(Some(af_id));
            jet.set_ai_state(AIState::Docked);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(jet_id, 12);
                // Docked
            }
            jet.set_status_moving(false);
            jet.status.airborne_target = false;
            jet.movement.path.clear();
            jet.movement.current_path_index = 0;
            jet.record_host_movement();
            jet.movement.target_position = None;
            // Snap to airfield pad residual (hangar park).
            // Landing taxi: approach along reserved runway offset then settle.
            use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_PREP_SPACING;
            let mut pad = af_pos;
            if let Some(idx) = runway_idx {
                pad.x += (idx as f32 - 0.5) * PARKING_PLACE_RUNWAY_PREP_SPACING;
            }
            pad.y = af_pos.y;
            // Host-immediate hangar dock snap; log for GameWorld pose last-write.
            jet.set_position(pad);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(jet_id, Some([pad.x, pad.y, pad.z]));
                jet.movement.target_position = Some(pad);
                jet.record_host_movement();
            }
            // Arm ClipReload timer on first dock while empty (8000ms standard / 2000ms King-Black).
            // clip_reload_time == 0 → immediate rearm residual (legacy test / unknown weapon).
            if jet.needs_return_to_base_rearm() && jet.airfield_rearm_ready_frame.is_none() {
                let frames = jet.airfield_rearm_clip_reload_frames();
                if frames > 0 {
                    jet.airfield_rearm_ready_frame = Some(self.frame.saturating_add(frames));
                }
            }
        }
        let rearmed = {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                self.release_airfield_runway_for_jet(jet_id);
                return false;
            };
            if !jet.needs_return_to_base_rearm() {
                jet.airfield_rearm_ready_frame = None;
                true
            } else if let Some(ready) = jet.airfield_rearm_ready_frame {
                if self.frame < ready {
                    // Hangar ClipReload in progress — suppress OOA damage via caller.
                    false
                } else if jet.rearm_return_to_base_weapons() {
                    jet.airfield_rearm_ready_frame = None;
                    true
                } else {
                    false
                }
            } else if jet.rearm_return_to_base_weapons() {
                true
            } else {
                false
            }
        };
        // Dock always succeeded; only fail closed if we could not dock (handled above).
        // While ClipReload pending, still return true so OOA damage is suppressed.
        if !rearmed {
            // Docked + ClipReload in progress: free runway, park, suppress OOA damage.
            self.release_airfield_runway_for_jet(jet_id);
            if let Some(af) = self.objects.get_mut(&af_id) {
                if let Some(building) = af.building_data.as_mut() {
                    if !building.garrisoned_units.contains(&jet_id) {
                        building.garrisoned_units.push(jet_id);
                    }
                } else if !af.occupants.contains(&jet_id) {
                    af.occupants.push(jet_id);
                }
                af.set_production_door_hold_open(true, self.frame);
            }
            return true;
        }
        // Docked: free the landing runway immediately (space now hangar-parked).
        self.release_airfield_runway_for_jet(jet_id);
        // Register parking slot on the airfield (ParkingPlace reserve residual).
        // Structures report transport_capacity 0, so bypass add_occupant gates and
        // write the hangar roster directly (NumRows×NumCols capacity already checked).
        if let Some(af) = self.objects.get_mut(&af_id) {
            if let Some(building) = af.building_data.as_mut() {
                if !building.garrisoned_units.contains(&jet_id) {
                    building.garrisoned_units.push(jet_id);
                }
            } else if !af.occupants.contains(&jet_id) {
                af.occupants.push(jet_id);
            }
            // C++ ParkingPlaceBehavior::reserveSpace → setHoldDoorOpen(door, true).
            af.set_production_door_hold_open(true, self.frame);
        }
        true
    }

    /// C++ ParkingPlaceBehavior heal residual for docked aircraft at airfields.
    ///
    /// Retail AmericaAirfield HealAmountPerSecond **10** → **10/30** HP per frame.
    pub(crate) fn tick_airfield_parking_heal(&mut self) {
        use crate::game_logic::host_countermeasures::aircraft_has_countermeasures_upgrade;
        use crate::game_logic::host_dock_contain_exit_heal_residual::{
            parking_place_heal_per_frame, PARKING_PLACE_AIRFIELD_HEAL_AMOUNT_PER_SEC,
        };
        let heal = parking_place_heal_per_frame(PARKING_PLACE_AIRFIELD_HEAL_AMOUNT_PER_SEC);
        // Docked jets with Countermeasures (ReloadTime=0 / MustReloadAtAirfield residual)
        // reload flares even when already at full HP.
        // Parking heal residual keys off hangar association (contained_by airfield).
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
            let (af_ok, has_cm) = {
                let Some(jet) = self.objects.get(&jid) else {
                    continue;
                };
                let Some(af_id) = jet.contained_by else {
                    continue;
                };
                let af_ok = self
                    .objects
                    .get(&af_id)
                    .map(|af| Self::is_friendly_airfield(af, jet.team))
                    .unwrap_or(false);
                let has_cm = aircraft_has_countermeasures_upgrade(&jet.applied_upgrades);
                (af_ok, has_cm)
            };
            if !af_ok {
                continue;
            }
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
