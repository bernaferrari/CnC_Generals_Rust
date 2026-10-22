//! C++ ParkingPlaceBehavior death callbacks and reservation cleanup.
use super::*;

impl GameLogic {
    #[cfg(test)]
    pub(crate) fn reserve_produced_aircraft_parking_space_for_test(
        &mut self,
        airfield: ObjectId,
        aircraft: ObjectId,
    ) -> bool {
        self.reserve_produced_aircraft_parking_space(airfield, aircraft)
    }

    /// C++ purgeDead clears only missing/dead reservations, including after sale.
    /// Normal admission rejects SOLD airfields and must not be used for cleanup.
    fn purge_dead_airfield_reservations(&mut self, airfield_id: ObjectId) {
        let objects = &self.objects;
        let live = |id: ObjectId| {
            objects
                .get(&id)
                .is_some_and(|object| !object.status.effectively_dead)
        };
        if let Some(spaces) = self.airfield_parking_spaces.get_mut(&airfield_id) {
            for space in spaces {
                if space.object_id.is_some_and(|id| !live(id)) {
                    *space = AirfieldParkingSpace::default();
                }
            }
        }
        if let Some(runways) = self.runway_reservations.get_mut(&airfield_id) {
            for (index, runway) in runways.iter_mut().enumerate() {
                if runway.is_some_and(|id| !live(id)) {
                    *runway = None;
                    if let Some(was) = self
                        .airfield_runway_was_in_line
                        .get_mut(&airfield_id)
                        .and_then(|rows| rows.get_mut(index))
                    {
                        *was = false;
                    }
                }
            }
        }
        if let Some(next) = self.airfield_runway_next_in_line.get_mut(&airfield_id) {
            for slot in next {
                if slot.is_some_and(|id| !live(id)) {
                    *slot = None;
                }
            }
        }
        let mut healing_changed = false;
        if let Some(healing) = self.airfield_healing.get_mut(&airfield_id) {
            let before = healing.len();
            healing.retain(|info| live(info.getting_healed_id));
            healing_changed = before != healing.len();
        }
        if healing_changed {
            self.reset_airfield_heal_wake(airfield_id);
        }
        self.sync_airfield_hangar_doors(airfield_id);
    }

    /// C++ ParkingPlaceBehavior::killAllParkedUnits: current slots in order,
    /// synchronous Body/Die callbacks, then purge; no physical deletion here.
    pub(in crate::game_logic) fn kill_all_parked_units_with_effects(
        &mut self,
        airfield_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        let Some(capacity) = self
            .objects
            .get(&airfield_id)
            .and_then(|object| object.thing().template.parking_place.as_ref())
            .and_then(|metadata| metadata.capacity())
        else {
            return 0;
        };
        self.airfield_parking_spaces
            .entry(airfield_id)
            .or_insert_with(|| vec![AirfieldParkingSpace::default(); capacity]);
        self.purge_dead_airfield_reservations(airfield_id);
        let mut killed = 0u32;
        for index in 0..capacity {
            // Earlier onDie callbacks can change later reservations or bodies.
            let Some(jet_id) = self
                .airfield_parking_spaces
                .get(&airfield_id)
                .and_then(|spaces| spaces.get(index))
                .and_then(|space| space.object_id)
            else {
                continue;
            };
            let Some(jet) = self
                .objects
                .get(&jet_id)
                .filter(|jet| !jet.status.effectively_dead)
            else {
                continue;
            };
            let takeoff_or_landing =
                jet.jet_ai.takeoff_in_progress || jet.jet_ai.landing_in_progress;
            if jet.is_above_terrain() && !takeoff_or_landing {
                continue;
            }
            let result = self.apply_owned_kill_with_effects(
                jet_id,
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                presentation_effects,
            );
            if result.is_some_and(|result| result.destroyed) {
                killed = killed.saturating_add(1);
            }
        }
        self.purge_dead_airfield_reservations(airfield_id);
        killed
    }
}
