use super::super::super::*;
use super::firepoints::*;

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
        if !attacker.is_alive() || attacker.weapon.is_none() {
            return;
        }
        let Some(weapon) = attacker.weapon.as_ref() else {
            return;
        };

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
        if outer
            .and_then(|id| self.objects.get(&id))
            .is_some_and(|host| {
                host.contained_by.is_some()
                    || host.status.disabled_held
                    || host.status.disabled_subdued
                    || !host.is_alive()
            })
        {
            return;
        }
        let nested = container.contained_by.is_some() && outer.is_none();
        let bunker_slots = container.overlord_bunker_slot_capacity();
        let bunker_may =
            crate::game_logic::host_passengers_fire_upgrade::overlord_bunker_passengers_may_fire(
                bunker_slots,
                nested,
            );
        // C++ OpenContain::isPassengerAllowedToFire residual + Overlord bunker peel.
        if !container.passengers_allowed_to_fire && !bunker_may {
            return;
        }
        if nested {
            return;
        }
        // C++ TransportContain::isPassengerAllowedToFire (TransportContain.cpp:576-578):
        // leftover helper — only infantry fire out. Vehicles ride silent
        // (Combat Chinook AllowInsideKindOf = INFANTRY VEHICLE).
        if !gamelogic::object::contain::transport_contain_passenger_kind_allowed_to_fire(
            attacker.is_kind_of(KindOf::Infantry),
        ) {
            return;
        }

        let is_battle_bus = container.is_battle_bus_style_container();
        let is_combat_chinook = container.is_combat_chinook_style_container();
        let is_listening_outpost = container.is_listening_outpost_style_container();
        let team = attacker.team;
        // HelixContain::onContaining sets WEAPONBONUSCONDITION_GARRISONED.
        // Occupants stay Docked, so Object::weapon_bonus_fields never applies
        // the Garrisoned AIState 133% path used by bunker occupants.
        let range = if container.is_helix_transport {
            weapon.range * GARRISONED_WEAPON_RANGE_MULT
        } else {
            weapon.range
        };
        let under = crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
        let range = (range - under).max(0.0);
        let passenger_index = container
            .contained_units()
            .iter()
            .position(|&id| id == passenger_id)
            .unwrap_or(0);
        let fire_pos = transport_passenger_fire_origin(
            outer
                .and_then(|id| self.objects.get(&id))
                .unwrap_or(container),
            passenger_index,
        );
        if bunker_may {
            if let Some(c) = self.objects.get_mut(&cid) {
                if !c.passengers_allowed_to_fire {
                    c.passengers_allowed_to_fire = true;
                }
            }
        }

        // Pure residual acquire query (fire decision choice phase).
        let candidates: Vec<_> = self
            .objects
            .registered()
            .filter(|(id, _)| **id != passenger_id && **id != cid)
            .map(|(id, obj)| {
                let combat_kind = crate::game_logic::host_residual_acquire::residual_combat_kind(
                    obj.is_kind_of(KindOf::Attackable),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                );
                crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: *id,
                    team: obj.team,
                    position: obj.get_position(),
                    is_alive: obj.is_alive(),
                    is_neutral: obj.team == Team::Neutral,
                    under_construction: obj.status.under_construction,
                    combat_kind,
                    effectively_stealthed: obj.is_effectively_stealthed(),
                    is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                    eject_invulnerable: obj.is_eject_invulnerable(),
                }
            })
            .collect();
        let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            passenger_id,
            team,
            fire_pos,
            candidates,
            |_| range,
            |c| c.is_alive && c.team != team && !c.is_neutral && c.combat_kind,
        );

        let Some((target_id, _, _)) = best else {
            return;
        };

        if !self.fire_owned_passive_weapon_with_effects(
            passenger_id,
            target_id,
            0,
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
        let now = self.frame;
        for pid in occupants {
            let stealth_garrison = self
                .objects
                .get(&pid)
                .is_some_and(|o| o.is_kind_of(KindOf::StealthGarrison));
            if !stealth_garrison {
                continue;
            }
            let delay = self
                .objects
                .get(&pid)
                .map(|o| o.stealth_delay_frames)
                .unwrap_or(0)
                .max(60);
            if let Some(occ) = self.objects.get_mut(&pid) {
                occ.mark_detected(now.saturating_add(delay));
            }
        }
    }
}
