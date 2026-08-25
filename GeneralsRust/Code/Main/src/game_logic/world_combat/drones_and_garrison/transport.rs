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
        if !Object::weapon_ready(weapon, current_time) {
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
        let nested = container.contained_by.is_some();
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
        let damage = weapon.damage;
        let passenger_index = container
            .contained_units()
            .iter()
            .position(|&id| id == passenger_id)
            .unwrap_or(0);
        let fire_pos = transport_passenger_fire_origin(container, passenger_index);
        if bunker_may {
            if let Some(c) = self.objects.get_mut(&cid) {
                if !c.passengers_allowed_to_fire {
                    c.passengers_allowed_to_fire = true;
                    c.record_host_stealth_flags();
                }
            }
        }

        // Pure residual acquire query (fire decision choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
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

        let weapon_snap = self
            .objects
            .get(&passenger_id)
            .and_then(|a| a.weapon.clone());
        let (destroyed, _) = self.residual_auto_fire_apply_damage(
            passenger_id,
            target_id,
            damage,
            fire_pos,
            weapon_snap.as_ref(),
            0,
        );

        if let Some(attacker) = self.objects.get_mut(&passenger_id) {
            let _ = attacker.capture_pending_weapon_visual_dispatch(
                0,
                self.frame,
                Some(target_id),
                None,
            );
            if let Some(w) = attacker.weapon.as_mut() {
                // Clip/ammo residual parity with fire_at path (not last_fire-only stamp).
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
            // AI attack authority: residual fire-intent for GameWorld last-writer.
            if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                let (dmg, rng) = attacker
                    .weapon
                    .as_ref()
                    .map(|w| (w.damage, w.range))
                    .unwrap_or((0.0, 0.0));
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let next_count = attacker.fire_intent_count.saturating_add(1);
                crate::game_logic::host_fire_intent_log::record(
                    attacker.id,
                    target_id.0,
                    0,
                    dmg,
                    rng,
                    current_time,
                    frame,
                    next_count,
                );
                attacker.fire_intent_count = next_count;
            }
            attacker.set_target(Some(target_id));
            attacker.set_ai_state(AIState::Attacking);
            attacker.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(passenger_id, target_id);
                crate::game_logic::host_ai_decision_log::record_set_state(passenger_id, 2);
            }
            // Kill XP awarded after this borrow via award_experience.
        }
        // Contained fire changes where the shot originates, not which
        // concrete passenger WeaponSet slot discharged.
        let _ = self.record_accepted_weapon_discharge(passenger_id, 0);

        if destroyed {
            self.award_score_the_kill_experience(passenger_id, target_id);
            self.mark_object_for_destruction(target_id, Some(team));
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
