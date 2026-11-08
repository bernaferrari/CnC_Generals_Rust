use super::super::super::*;
use super::firepoints::*;

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;

impl GameLogic {
    /// Residual fire-from-transport: docked passengers auto-engage nearest
    /// enemy in weapon range. HelixContain riders fire from hull origin +8 Y
    /// (`HelixContain::redeployOccupants`). Other transports use sequential
    /// `FIREPOINT` bones (`OpenContain::putObjAtNextFirePoint`); hull if none.
    pub(in crate::game_logic) fn try_transport_passenger_residual_fire(
        &mut self,
        passenger_id: ObjectId,
    ) {
        self.try_transport_passenger_residual_fire_with_effects(
            passenger_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn try_transport_passenger_residual_fire_with_effects(
        &mut self,
        passenger_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

        let Some(attacker) = self.objects.get(&passenger_id) else {
            return;
        };
        if !attacker.is_alive() {
            return;
        }

        let container_id = attacker.container_id();
        let Some(cid) = container_id else {
            return;
        };
        let Some(container) = self.objects.get(&cid) else {
            return;
        };
        // C++ ActiveBody::onSubdualChange → OpenContain::orderAllPassengersToIdle
        // (flashbang / ECM-jammed Humvee/Chinook). Garrison sibling already
        // gates DISABLED_SUBDUED (hq-8ikxi); transport residual fire did not.
        if container.status.disabled_subdued {
            return;
        }
        // C++ OverlordContain::isPassengerAllowedToFire — nested contain voids fire.
        let outer = container
            .contained_by
            .filter(|outer| self.redirected_containment_owner(*outer) == cid);
        if !self.passenger_allowed_to_fire(passenger_id) {
            return;
        }
        let helix_rider = container.is_helix_transport
            && container.overlord_portable_occupant == Some(passenger_id);

        let is_battle_bus = container.is_battle_bus_style_container();
        let is_combat_chinook = container.is_combat_chinook_style_container();
        let is_listening_outpost = container.is_listening_outpost_style_container();
        let team = attacker.team;
        let portable_rider = attacker.is_kind_of(KindOf::PortableStructure);
        let passenger_index = container
            .contained_units()
            .iter()
            .position(|&id| id == passenger_id)
            .unwrap_or(0);
        let fire_pos = if helix_rider {
            // The separate portable structure follows the hull itself; only
            // the TransportContain passenger roster is redeployed at +8.
            attacker.get_position()
        } else {
            transport_passenger_fire_origin(
                outer
                    .and_then(|id| self.objects.get(&id))
                    .unwrap_or(container),
                passenger_index,
            )
        };
        if portable_rider {
            let mut best = None;
            for (&id, target) in self.objects.registered() {
                if id == passenger_id || id == cid || !target.is_alive()
                    || target.team == team || target.team == Team::Neutral
                    || target.is_effectively_stealthed()
                    || !(target.is_kind_of(KindOf::Attackable)
                        || target.is_kind_of(KindOf::Structure)
                        || target.is_kind_of(KindOf::Infantry)
                        || target.is_kind_of(KindOf::Vehicle)
                        || target.is_kind_of(KindOf::Aircraft))
                {
                    continue;
                }
                let Some(slot) = attacker.select_combat_weapon_slot_with_solo_set(
                    target, current_time, &self.solo_weapon_bonus_set,
                ) else {
                    continue;
                };
                let Some(weapon) = attacker.weapon_slot(slot) else {
                    continue;
                };
                let range = (attacker.effective_weapon_range_for_slot_with_solo_set(
                    slot, weapon.range, &self.solo_weapon_bonus_set,
                ) - crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25).max(0.0);
                let distance = fire_pos.distance(target.get_position());
                if distance <= range && best.is_none_or(|(_, d, _)| distance < d) {
                    best = Some((id, distance, slot));
                }
            }
            if let Some((target, _, slot)) = best {
                self.fire_owned_passive_weapon_with_effects(
                    passenger_id, target, slot, fire_pos, presentation_effects,
                );
            }
            return;
        }

        let mut best = None;
        for (&id, target) in self.objects.registered() {
            if id == passenger_id || id == cid || !target.is_alive()
                || target.team == team || target.team == Team::Neutral
                || target.is_effectively_stealthed()
                || !crate::game_logic::host_residual_acquire::residual_combat_kind(
                    target.is_kind_of(KindOf::Attackable),
                    target.is_kind_of(KindOf::Structure),
                    target.is_kind_of(KindOf::Infantry),
                    target.is_kind_of(KindOf::Vehicle),
                    target.is_kind_of(KindOf::Aircraft),
                )
            {
                continue;
            }
            let Some(slot) = attacker.select_combat_weapon_slot_with_solo_set(
                target, current_time, &self.solo_weapon_bonus_set,
            ) else {
                continue;
            };
            let Some(weapon) = attacker.weapon_slot(slot) else {
                continue;
            };
            let range_bonus = self.receiving_weapon_bonus_fields_for_slot(attacker, slot).1;
            let range = (weapon.range * range_bonus
                - crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25).max(0.0);
            let distance = fire_pos.distance(target.get_position());
            if range > 0.0 && distance <= range && best.is_none_or(|(_, d, _)| distance < d) {
                best = Some((id, distance, slot));
            }
        }

        let Some((target_id, _, slot)) = best else {
            return;
        };

        if !self.fire_owned_passive_weapon_with_effects(
            passenger_id,
            target_id,
            slot,
            fire_pos,
            presentation_effects,
        ) {
            return;
        }

        if is_battle_bus {
            self.battle_bus.record_passenger_fire();
        } else if is_combat_chinook {
            self.combat_chinook.record_passenger_fire();
        } else if is_listening_outpost {
            self.listening_outpost.record_passenger_fire();
        }
    }

    /// C++ `OpenContain::markAllPassengersDetected` (`OpenContain.cpp:1322-1343`).
    /// Reveal `KINDOF_STEALTH_GARRISON` riders immediately before an evac dump
    /// so they do not walk out still cloaked.
    pub(crate) fn mark_all_passengers_detected(&mut self, container_id: ObjectId) {
        let occupants = self
            .objects
            .get(&container_id)
            .map(|c| c.contained_units())
            .unwrap_or_default();
        for pid in occupants {
            if self
                .objects
                .get(&pid)
                .is_some_and(|o| o.is_kind_of(KindOf::StealthGarrison))
            {
                self.mark_stealth_detected(pid, 0);
            }
        }
    }
}
