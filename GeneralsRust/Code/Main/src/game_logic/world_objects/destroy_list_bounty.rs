//! Host objects `impl GameLogic` — `destroy_list_bounty`.
//! process_destroy_list and cash bounty. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;


/// C++ `OpenContain::getDamagePercentageToUnits` — INI `DamagePercentToUnits`
/// via `parsePercentToReal` (`100%` → `1.0`). Live `building_data` is never
/// populated, so resolve authored retail percents by container kind.
fn damage_percent_to_units_from_ini(obj: &Object) -> f32 {
    let authored = obj
        .building_data
        .as_ref()
        .map(|bd| bd.damage_percent_to_units)
        .unwrap_or(0.0);
    if authored > 0.0 {
        return authored;
    }
    if obj.is_humvee_style_container() {
        return crate::game_logic::host_humvee::HUMVEE_DAMAGE_PERCENT_TO_UNITS / 100.0;
    }
    if obj.is_battle_bus_style_container() {
        return crate::game_logic::host_battle_bus::BATTLE_BUS_DAMAGE_PERCENT_TO_UNITS / 100.0;
    }
    if obj.is_combat_chinook_style_container() {
        return crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_DAMAGE_PERCENT_TO_UNITS
            / 100.0;
    }
    if obj.is_technical_style_container() {
        return crate::game_logic::host_technical::TECHNICAL_DAMAGE_PERCENT_TO_UNITS / 100.0;
    }
    if obj.is_troop_crawler_style_container() {
        return crate::game_logic::host_troop_crawler::TROOP_CRAWLER_DAMAGE_PERCENT_TO_UNITS / 100.0;
    }
    if obj.is_listening_outpost_style_container() {
        return crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_DAMAGE_PERCENT_TO_UNITS
            / 100.0;
    }
    if obj.is_overlord_style_container() || obj.is_helix_transport {
        return crate::game_logic::host_overlord_addons::OVERLORD_CONTAIN_DAMAGE_PERCENT_TO_UNITS;
    }
    if crate::game_logic::host_heal::is_ambulance_healer(&obj.template_name) {
        return crate::game_logic::host_heal::AMBULANCE_TRANSPORT_DAMAGE_PERCENT_TO_UNITS;
    }
    if obj
        .template_name
        .to_ascii_lowercase()
        .contains("firebase")
    {
        // Retail AmericaFireBase DamagePercentToUnits 100% (parsePercentToReal).
        return 1.0;
    }
    // C++ OpenContainModuleData default DamagePercentToUnits.
    0.0
}

impl GameLogic {
    /// Wave 912: true when destroy queue or destroy-ready residual has work.
    #[inline]
    pub fn has_pending_destroy_work(&self) -> bool {
        if !self.objects_to_destroy.is_empty() {
            return true;
        }
        crate::gameworld_shadow::gameworld_damage_authority_live()
            && crate::game_logic::host_destroy_ready_log::has_pending()
    }

    /// Wave 912: process destroy list only when residual work is pending.
    #[inline]
    pub fn process_destroy_list_if_needed(&mut self) {
        if self.has_pending_destroy_work() {
            self.process_destroy_list();
        }
    }

    pub fn process_destroy_list(&mut self) {
        // Wave 621: under damage authority, GameWorld health writeback records
        // lethal IDs; host marks them here before draining the destroy queue.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            for ev in crate::game_logic::host_destroy_ready_log::drain() {
                if self.objects_to_destroy.iter().any(|e| e.id == ev.object) {
                    continue;
                }
                let lethal = self
                    .objects
                    .get(&ev.object)
                    .map(|o| o.status.destroyed || o.health.current <= 0.0)
                    .unwrap_or(false);
                if lethal {
                    self.mark_object_for_destruction(ev.object, None);
                }
            }
        }
        let mut destroyed_structure = false;
        while let Some(event) = self.objects_to_destroy.pop_front() {
            self.pending_special_abilities.remove(&event.id);
            self.pending_special_abilities
                .retain(|_, ability| ability.target_id() != event.id);

            self.cancel_all_production(event.id);
            // Damage authority / an old snapshot can enqueue a death without
            // passing through mark_object_for_destruction.  Keep the one
            // typed EjectPilotDie onDie path live before removing the object.
            self.maybe_apply_eject_pilot_die(event.id);

            // C++ Object::onDie RECONSTRUCTING residual (lost rebuild → hole).
            let handled_recon = self.handle_reconstructing_death(event.id);
            // C++ RebuildHoleExposeDie residual (GLA structures → hole).
            // Skip if this was a reconstructing building (hole already exists).
            if !handled_recon {
                let _ = self.maybe_spawn_rebuild_hole(event.id);
            }

            // Snapshot CreateCrateDie residual fields before remove.
            let (crate_data, death_pos_pre, death_team_pre, last_src) =
                if let Some(o) = self.objects.get(&event.id) {
                    (
                        o.thing.template.create_crate_data.clone(),
                        o.get_position(),
                        o.team,
                        o.last_damage_source,
                    )
                } else {
                    (Vec::new(), glam::Vec3::ZERO, Team::Neutral, None)
                };
            if !crate_data.is_empty() {
                let _ = self.try_create_crates_on_die(
                    event.id,
                    death_pos_pre,
                    death_team_pre,
                    &crate_data,
                    last_src,
                );
            }

            // C++ FireWeaponWhenDeadBehavior::onDie residual.
            self.apply_fire_weapon_when_dead(event.id);

            if let Some(obj) = self.objects.remove(&event.id) {
                self.host_radar_remove_object(event.id);
                crate::game_logic::host_destroy_log::record(event.id);
                // Wave 681: mid-frame GameWorld Destroy while coupled shadow tick is live.
                // End-of-tick host_destroy_log drain remains idempotent for unmapped IDs.
                let _ = crate::gameworld_shadow::eager_unmap_host_destroy_if_coupled(event.id);
                // Combat particle residual: death → registry entry (explosion + smoke).
                // PresentationFrame / client can observe systems after the kill.
                let death_pos = obj.get_position();
                let is_structure = obj.is_kind_of(KindOf::Structure);
                if is_structure {
                    destroyed_structure = true;
                }
                let victim_team = obj.team;
                // C++ Object::onDie EVA residual (local, non-self-inflicted).
                let is_infantry = obj.is_kind_of(KindOf::Infantry);
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                // KINDOF_MP_COUNT_FOR_VICTORY residual class (main base buildings).
                let is_mp_count = is_structure
                    && (obj.is_kind_of(KindOf::CommandCenter)
                        || obj.is_kind_of(KindOf::FSPower)
                        || obj.is_kind_of(KindOf::PowerPlant)
                        || obj.is_kind_of(KindOf::FSBarracks)
                        || obj.is_kind_of(KindOf::FSWarFactory)
                        || obj.is_kind_of(KindOf::FSAirfield)
                        || obj.is_kind_of(KindOf::FSSuperweapon)
                        || obj.is_kind_of(KindOf::FSStrategyCenter)
                        || obj.is_kind_of(KindOf::FSTechnology)
                        || obj.is_kind_of(KindOf::SupplyCenter)
                        || obj.is_kind_of(KindOf::FSSupplyCenter));
                self.try_eva_on_local_object_death(
                    event.id,
                    victim_team,
                    is_structure,
                    is_infantry,
                    is_vehicle,
                    is_mp_count,
                    death_pos,
                    event.killer,
                );
                let frame = self.frame;
                let death_type = obj.status.death_type;
                let _ = self.combat_particles.spawn_death_fx_for_type(
                    death_pos,
                    frame,
                    event.id,
                    is_structure,
                    victim_team,
                    death_type,
                );

                // Audio residual (hq-7zxm slice): unit/structure death → AudioEventRequest.
                // DeathType residual selects die cue family (not full voice bank).
                let death_event = crate::game_logic::combat_particles::CombatParticleRegistry::death_audio_event_name(
                    is_structure,
                    death_type,
                );
                self.queue_audio_event(
                    AudioEventRequest::new(death_event)
                        .with_object(event.id)
                        .with_position(death_pos)
                        .with_priority(200),
                );

                let eject_origin = obj.get_position();

                // C++ OpenContain::onDie: processDamageToContained(getDamagePercentageToUnits()).
                let damage_pct = damage_percent_to_units_from_ini(&obj);

                // C++ ParachuteContain::onDie: airborne chute → FreeFallDamage riders.
                let is_america_parachute = obj.template_name.eq_ignore_ascii_case(
                    crate::game_logic::host_car_bomb::HIJACKER_PARACHUTE_NAME,
                );
                let chute_airborne = is_america_parachute
                    && crate::game_logic::host_usa_pilot::should_apply_parachute_free_fall_damage(
                        obj.is_parachuting() || is_america_parachute,
                        eject_origin.y,
                    );

                // `RiderChangeContain::onRemoving` destroys its hidden rider
                // when the bike is effectively dead; it must not fall through
                // OpenContain's ordinary eject-to-world behavior.  Queue the
                // contained body for the existing destruction authority after
                // clearing its containment link, so no snapshot frame can
                // retain an orphan rider inside a removed bike.
                let rider_change_payload = obj.thing.template.contain_module.kind
                    == crate::game_logic::ContainModuleKind::RiderChange;
                // C++ TunnelContain::onDie (TunnelContain.cpp:326) overrides
                // OpenContain::onDie — no occupant eject; shared pool stays.
                let is_tunnel = obj.is_tunnel_network_style_container()
                    || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                        &obj.template_name,
                    );

                if rider_change_payload {
                    for contained_id in obj.contained_units() {
                        if let Some(unit) = self.objects.get_mut(&contained_id) {
                            unit.set_contained_by(None);
                            unit.set_target(None);
                            unit.stop_moving();
                            unit.set_status_moving(false);
                            unit.set_status_attacking(false);
                            unit.status.destroyed = true;
                        }
                        if let Some(team) = self.tunnel_network.team_holding_unit(contained_id) {
                            let _ = self.tunnel_network.record_exit(team, contained_id, event.id);
                        }
                        self.mark_object_for_destruction(contained_id, event.killer);
                    }
                } else if chute_airborne {
                    let riders = obj.contained_units();
                    for rid in riders {
                        if let Some(team) = self.tunnel_network.team_holding_unit(rid) {
                            let _ = self.tunnel_network.record_exit(team, rid, event.id);
                        }
                        let _ = self.apply_rider_free_fall_damage(rid, eject_origin);
                    }
                    self.car_bomb.record_airborne_parachute_free_fall();
                } else if is_tunnel {
                    // C++ TunnelTracker::onTunnelDestroyed (TunnelTracker.cpp:187).
                    let remaining: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|o| {
                            o.is_alive()
                                && o.team == obj.team
                                && !o.status.sold
                                && (o.is_tunnel_network_style_container()
                                    || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                                        &o.template_name,
                                    ))
                        })
                        .map(|o| o.id)
                        .collect();
                    let outcome =
                        self.tunnel_network
                            .on_tunnel_destroyed(obj.team, event.id, &remaining);
                    if outcome.cave_in {
                        for uid in outcome.cave_in_units {
                            if let Some(unit) = self.objects.get_mut(&uid) {
                                unit.set_contained_by(None);
                                unit.set_target(None);
                                unit.stop_moving();
                                unit.set_status_moving(false);
                                unit.set_status_attacking(false);
                                unit.status.destroyed = true;
                                unit.health.current = 0.0;
                            }
                            self.mark_object_for_destruction(uid, event.killer);
                        }
                    } else if let Some(valid) = outcome.remapped_to {
                        let pool = self.tunnel_network.contained_for_team(obj.team);
                        for uid in pool {
                            if let Some(unit) = self.objects.get_mut(&uid) {
                                if unit.contained_by == Some(event.id) {
                                    unit.set_contained_by(Some(valid));
                                }
                            }
                        }
                    }
                } else {
                    for (i, contained_id) in obj.contained_units().into_iter().enumerate() {
                        if let Some(unit) = self.objects.get_mut(&contained_id) {
                            // C++ OpenContain::processDamageToContained:
                            // UNRESISTABLE, BURNED default, source = container,
                            // then kill() if percent == 1.0 and still alive.
                            if damage_pct > 0.0 {
                                let dmg = unit.max_health * damage_pct;
                                let destroyed = unit.take_damage_from_typed_death(
                                    dmg,
                                    Some(event.id),
                                    crate::game_logic::combat::DamageType::Unresistable,
                                    crate::game_logic::host_usa_pilot::HostDeathType::Burned,
                                );
                                let flame_proof_kill = !destroyed
                                    && !unit.status.destroyed
                                    && (damage_pct - 1.0).abs() < f32::EPSILON;
                                if flame_proof_kill {
                                    let _ = unit.take_damage_from_immediate(
                                        crate::game_logic::host_partition_collision_physics_residual::PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL,
                                        Some(event.id),
                                    );
                                    unit.status.destroyed = true;
                                }
                                if destroyed || flame_proof_kill || unit.status.destroyed {
                                    unit.status.destroyed = true;
                                    if let Some(team) =
                                        self.tunnel_network.team_holding_unit(contained_id)
                                    {
                                        let _ = self.tunnel_network.record_exit(
                                            team,
                                            contained_id,
                                            event.id,
                                        );
                                    }
                                    self.mark_object_for_destruction(contained_id, event.killer);
                                    continue;
                                }
                            }

                            let angle = (contained_id.0 as f32 + i as f32 * 1.11).sin().atan2(1.0)
                                + i as f32 * 0.73;
                            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 8.0;
                            unit.stop_moving();
                            unit.set_position(eject_origin + offset);
                            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                                let p = eject_origin + offset;
                                crate::game_logic::host_move_log::record(
                                    unit.id,
                                    Some([p.x, p.y, p.z]),
                                );
                                unit.record_host_movement();
                            }
                            unit.set_target(None);
                            unit.set_contained_by(None);
                            unit.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_stop_attack(
                                    contained_id,
                                );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    contained_id,
                                    0,
                                );
                            }
                            unit.set_status_moving(false);
                            unit.set_status_attacking(false);
                        }
                    }
                }

                // GLA Toxin Tractor death residual: ToxinShellWeapon → SmallPoisonField.
                // Fail-closed: not full FireWeaponWhenDead anthrax matrix / FX list.
                {
                    use crate::game_logic::host_toxin_tractor::{
                        anthrax_tier_from_flags, is_chem_general_template,
                        is_toxin_tractor_template, UPGRADE_GLA_ANTHRAX_BETA,
                        UPGRADE_GLA_ANTHRAX_GAMMA, UPGRADE_GLA_ANTHRAX_GAMMA_ALT,
                    };
                    if !obj.fire_weapon_when_dead_fired
                        && is_toxin_tractor_template(&obj.template_name)
                    {
                        let has_gamma = obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA)
                            || obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA_ALT)
                            || obj.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxGamma");
                        let has_beta = obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxBeta");
                        let anthrax = anthrax_tier_from_flags(
                            has_gamma,
                            has_beta,
                            is_chem_general_template(&obj.template_name),
                        );
                        let death_pos = obj.get_position();
                        let team = obj.team;
                        let _ = self
                            .toxin_tractor
                            .spawn_death_field(event.id, team, death_pos, self.frame, anthrax);
                        self.queue_audio_event(
                            AudioEventRequest::new(
                                crate::game_logic::host_toxin_tractor::TOXIN_POISON_AUDIO,
                            )
                            .with_position(death_pos)
                            .with_priority(140),
                        );
                    }
                }

                // GLA Bomb Truck FireWeaponWhenDead residual: HE/Bio detonation matrix.
                // Fail-closed: not full exclusive module / SubObjectsUpgrade payload visuals.
                // Note: object already removed from map — use `obj` snapshot for upgrades/pos.
                {
                    use crate::game_logic::host_bomb_truck_detonate::{
                        is_bomb_truck_template, BombTruckDetonationProfile, UPGRADE_BOMB_TRUCK_BIO,
                        UPGRADE_BOMB_TRUCK_HE, UPGRADE_GLA_ANTHRAX_BETA,
                    };
                    if !obj.fire_weapon_when_dead_fired
                        && is_bomb_truck_template(&obj.template_name)
                    {
                        let he = obj.has_upgrade_tag(UPGRADE_BOMB_TRUCK_HE)
                            || obj.has_upgrade_tag("Upgrade_GLABombTruckHighExplosiveBomb");
                        let bio = obj.has_upgrade_tag(UPGRADE_BOMB_TRUCK_BIO)
                            || obj.has_upgrade_tag("Upgrade_GLABombTruckBioBomb");
                        let anthrax = obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxBeta")
                            || obj.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxGamma")
                            || obj.has_upgrade_tag(
                                crate::game_logic::host_toxin_tractor::UPGRADE_GLA_ANTHRAX_GAMMA,
                            )
                            || obj.has_upgrade_tag(
                                crate::game_logic::host_toxin_tractor::UPGRADE_GLA_ANTHRAX_GAMMA_ALT,
                            );
                        let profile = BombTruckDetonationProfile::from_upgrades(he, bio, anthrax);
                        let _ = self.apply_bomb_truck_death_detonation_at(
                            event.id, obj.team, death_pos, profile,
                        );
                    }
                }

                // China Nuclear Tanks FireWeaponWhenDead residual: dual-radius + radiation.
                // Fail-closed: not full exclusive module / Nuclear*Locomotor visual matrix.
                {
                    use crate::game_logic::host_nuclear_tanks::{
                        has_nuclear_tanks_upgrade, is_nuclear_tanks_eligible,
                        is_nuke_general_nuclear_tanks,
                    };
                    if !obj.fire_weapon_when_dead_fired
                        && is_nuclear_tanks_eligible(&obj.template_name)
                        && has_nuclear_tanks_upgrade(&obj.applied_upgrades)
                    {
                        let nuke_gen = is_nuke_general_nuclear_tanks(&obj.template_name);
                        let _ = self.apply_nuclear_tanks_death_detonation_at(
                            event.id, obj.team, death_pos, nuke_gen,
                        );
                    }
                }

                // Demo SuicideBomb FireWeaponWhenDead residual: Demo_DestroyedWeapon blast.
                // Skip intentional SUICIDED path (PlusFire already applied via TertiarySuicide).
                // Skip terrorists (already handled by host_terrorist SUICIDED residual).
                {
                    use crate::game_logic::host_demo_suicide_bomb::{
                        has_demo_suicide_bomb_upgrade, is_demo_suicide_bomb_eligible_template,
                    };
                    use crate::game_logic::host_terrorist::is_terrorist_template;
                    if !obj.fire_weapon_when_dead_fired
                        && !obj.demo_suicided_detonating
                        && is_demo_suicide_bomb_eligible_template(&obj.template_name)
                        && has_demo_suicide_bomb_upgrade(&obj.applied_upgrades)
                        && !is_terrorist_template(&obj.template_name)
                    {
                        let _ =
                            self.apply_demo_suicide_bomb_death_at(event.id, obj.team, death_pos);
                    }
                }

                // GLA Rebel BoobyTrap residual: structure death detonates trap.
                // C++ Object::checkAndDetonateBoobyTrap(NULL) on die path.
                if obj.status.booby_trapped || self.booby_trap.is_booby_trapped(event.id) {
                    let _ = self.detonate_booby_trap_at(event.id, death_pos, None, false, true);
                }

                log::debug!(
                    "Destroyed object {} ({})",
                    event.id,
                    obj.get_template().name
                );
                self.record_destruction(&obj, event.killer);

                // Remove from player selections
                for (_, player) in self.players.iter_mut() {
                    player.selected_objects.retain(|&x| x != event.id);
                }

                // C++ parity: clear stale target references from all other objects.
                // When an object is destroyed, anything targeting it should stop.
                let destroyed_id = event.id;
                let clear_ids: Vec<ObjectId> = self
                    .objects
                    .iter()
                    .filter(|(_, o)| o.target == Some(destroyed_id))
                    .map(|(id, _)| *id)
                    .collect();
                for cid in clear_ids {
                    self.stop_attack_decision_aware(cid);
                }
                let mut guard_idle: Vec<ObjectId> = Vec::new();
                for (oid, other_obj) in self.objects.iter_mut() {
                    if other_obj.guard_target == Some(destroyed_id) {
                        other_obj.guard_target = None;
                        if other_obj.ai_state == AIState::GuardingObject {
                            other_obj.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                guard_idle.push(*oid);
                            }
                        }
                    }
                }
                for gid in guard_idle {
                    crate::game_logic::host_ai_decision_log::record_set_state(gid, 0);
                }
            }
        }

        if destroyed_structure {
            // Rebuild static path/LOS mask without the destroyed footprint.
            self.sync_structure_path_blocks();
        }
    }

    pub(in super::super) fn record_destruction(
        &mut self,
        destroyed_object: &Object,
        killer: Option<Team>,
    ) {
        let destroyed_is_structure = destroyed_object.is_kind_of(KindOf::Structure);
        let victim_team = destroyed_object.team;
        let victim_id = destroyed_object.id;
        let victim_pos = destroyed_object.get_position();
        // C++ Object::scoreTheKill / Player::doBountyForKill:
        // no bounty for under-construction, non-enemy, or same-controller kills.
        let under_construction = destroyed_object.status.under_construction;
        let build_cost = destroyed_object.thing.template.build_cost.supplies;
        let victim_owner_player_id = self.player_owner_for_host_object(destroyed_object);

        let mut bounty_awarded = 0_u32;
        let mut bounty_killer_id = ObjectId(0);
        let mut bounty_float_pos = victim_pos;
        let mut used_last_damage_source = false;
        if let Some(team) = killer {
            // `killer` is still a legacy team event, but BodyModule gives us
            // the actual attacking object.  Carry that object's player owner
            // through scoring instead of selecting the first same-faction
            // player slot.
            let mut killer_owner_player_id = None;
            // Prefer C++ BodyModule last_damage_source residual for killer ObjectId.
            if let Some(src) = destroyed_object.last_damage_source {
                if let Some(src_obj) = self.objects.get(&src) {
                    if src_obj.team == team {
                        bounty_killer_id = src;
                        bounty_float_pos = src_obj.get_position();
                        killer_owner_player_id = self.player_owner_for_host_object(src_obj);
                        used_last_damage_source = true;
                    }
                } else {
                    // Killer already removed this frame — still record ObjectId residual.
                    bounty_killer_id = src;
                    used_last_damage_source = true;
                }
            }
            // Fallback residual: nearest living unit on killer team near victim
            // (destruction event carries team; last_damage_source may be unset).
            if !used_last_damage_source {
                if let Some((kid, kpos)) = self
                    .objects
                    .iter()
                    .filter(|(_, o)| o.team == team && o.is_alive())
                    .map(|(id, o)| (*id, o.get_position()))
                    .min_by(|a, b| {
                        let da = (a.1.x - victim_pos.x).hypot(a.1.z - victim_pos.z);
                        let db = (b.1.x - victim_pos.x).hypot(b.1.z - victim_pos.z);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                {
                    bounty_killer_id = kid;
                    bounty_float_pos = kpos;
                }
            }
            let enemy_kill = match (killer_owner_player_id, victim_owner_player_id) {
                (Some(killer_player_id), Some(victim_player_id)) => {
                    self.player_relationship(killer_player_id, victim_player_id)
                        == gamelogic::common::Relationship::Enemies
                }
                // A genuinely unowned victim has no player relationship.
                // Preserve the legacy faction gate for map/old-save objects
                // without assigning it a player.
                _ => team != victim_team && team != Team::Neutral && victim_team != Team::Neutral,
            };
            if let Some(player_id) = killer_owner_player_id {
                if let Some(player) = self.players.get_mut(&player_id) {
                    if destroyed_is_structure {
                        player.record_structure_destroyed();
                    } else {
                        player.record_unit_destroyed();
                    }

                    // Cash bounty residual: award ceil(cost * percent) on enemy kill.
                    if enemy_kill && !under_construction && player.cash_bounty_percent > 0.0 {
                        bounty_awarded = player.do_bounty_for_kill(build_cost);
                    }

                    // C++ Player::addSkillPointsForKill (scoreTheKill).
                    // Skill value is victim template SkillPointValue / ExperienceValue.
                    if enemy_kill && !under_construction {
                        let skill = destroyed_object.kill_skill_point_value();
                        if skill != 0 {
                            let _leveled = player.add_skill_points_for_kill(skill);
                        }
                    }
                }
            }
        }
        if bounty_awarded > 0 {
            self.cash_bounty.record_bounty_award(bounty_awarded);
            if used_last_damage_source {
                self.cash_bounty.record_last_damage_source_kill();
            }
            // C++ doBountyForKill floating text: yellow, killer pos + Z10.
            self.cash_bounty.record_floating_text(
                crate::game_logic::host_cash_bounty::HostCashBountyFloatingText::new(
                    bounty_killer_id,
                    victim_id,
                    bounty_float_pos,
                    bounty_awarded,
                    self.frame,
                ),
            );
        }

        if let Some(player_id) = victim_owner_player_id {
            if let Some(player) = self.players.get_mut(&player_id) {
                if destroyed_is_structure {
                    player.record_structure_lost();
                } else {
                    player.record_unit_lost();
                }
            }
        }
    }

    /// Set cash bounty percent on a player (residual / tests).
    /// Raises percent only (C++ CashBountyPower set if higher).
    pub fn set_player_cash_bounty(&mut self, player_id: u32, percent: f32) -> bool {
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };
        player.set_cash_bounty(percent);
        self.cash_bounty
            .record_bounty_set(player.cash_bounty_percent);
        true
    }

    /// Force-set cash bounty percent (tests / load restore).
    pub fn force_set_player_cash_bounty(&mut self, player_id: u32, percent: f32) -> bool {
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };
        player.force_set_cash_bounty(percent);
        self.cash_bounty
            .record_bounty_set(player.cash_bounty_percent);
        true
    }

    /// Residual honesty: cash bounty was configured and at least one award paid.
    /// Fail-closed: not full palace module / floating-text parity.
    pub fn honesty_cash_bounty_ok(&self) -> bool {
        self.cash_bounty.honesty_ok()
    }

    /// Residual honesty: at least one bounty cash award on kill.
    pub fn honesty_cash_bounty_award_ok(&self) -> bool {
        self.cash_bounty.honesty_bounty_award_ok()
    }

    /// Residual cash bounty floating cash text honesty.
    pub fn honesty_cash_bounty_floating_text_ok(&self) -> bool {
        self.cash_bounty.honesty_floating_text_ok()
    }

    /// Total residual cash credited via kill bounty (observability).
    pub fn cash_bounty_earned_total(&self) -> u32 {
        self.cash_bounty.bounty_earned_total
    }

    /// Host cash bounty registry (tests / honesty).
    pub fn cash_bounty_registry(
        &self,
    ) -> &crate::game_logic::host_cash_bounty::HostCashBountyRegistry {
        &self.cash_bounty
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    fn setup_two_tunnels_and_rider() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::GLA, "GLA", true));
        let mut tn = ThingTemplate::new("GLATunnelNetwork");
        tn.add_kind_of(KindOf::Structure).set_health(1000.0);
        tn.build_cost.supplies = 800;
        logic.templates.insert("GLATunnelNetwork".into(), tn);
        let mut rebel = ThingTemplate::new("GLARebel");
        rebel.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic.templates.insert("GLARebel".into(), rebel);

        let t1 = logic
            .create_object(
                "GLATunnelNetwork",
                Team::GLA,
                glam::Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("t1");
        let t2 = logic
            .create_object(
                "GLATunnelNetwork",
                Team::GLA,
                glam::Vec3::new(80.0, 0.0, 0.0),
            )
            .expect("t2");
        let uid = logic
            .create_object("GLARebel", Team::GLA, glam::Vec3::new(1.0, 0.0, 0.0))
            .expect("rider");
        if let Some(o) = logic.host_object_mut(t1) {
            o.set_status_under_construction(false);
            o.construction_percent = 1.0;
            let _ = o.add_occupant(uid);
        }
        if let Some(o) = logic.host_object_mut(t2) {
            o.set_status_under_construction(false);
            o.construction_percent = 1.0;
        }
        if let Some(u) = logic.host_object_mut(uid) {
            u.set_contained_by(Some(t1));
        }
        assert!(logic.tunnel_network.record_enter(Team::GLA, uid, t1));
        (logic, t1, t2, uid)
    }

    #[test]
    fn tunnel_on_die_keeps_shared_pool_when_another_entrance_lives() {
        // C++ TunnelContain.cpp:326 onDie — no OpenContain eject.
        let (mut logic, t1, t2, uid) = setup_two_tunnels_and_rider();
        let origin = logic
            .host_object(uid)
            .map(|o| o.get_position())
            .expect("pos");
        logic.mark_object_for_destruction(t1, None);
        logic.process_destroy_list();
        assert!(
            logic.tunnel_network.is_in_network(Team::GLA, uid),
            "occupant must stay in the shared pool"
        );
        let u = logic.host_object(uid).expect("rider lives");
        assert!(u.is_alive());
        assert!(!u.status.destroyed);
        assert_eq!(
            u.contained_by,
            Some(t2),
            "ContainedBy remaps to remaining tunnel"
        );
        let p = u.get_position();
        assert!(
            (p.x - origin.x).abs() < 0.01 && (p.z - origin.z).abs() < 0.01,
            "must not spill at rubble"
        );
        assert_eq!(logic.tunnel_network.contain_count(Team::GLA), 1);
    }

    #[test]
    fn last_tunnel_die_cave_in_kills_pool() {
        // C++ TunnelTracker.cpp:192-197 last tunnel destroyObject all contained.
        let (mut logic, t1, t2, uid) = setup_two_tunnels_and_rider();
        logic.mark_object_for_destruction(t1, None);
        logic.process_destroy_list();
        assert!(logic.tunnel_network.is_in_network(Team::GLA, uid));
        logic.mark_object_for_destruction(t2, None);
        logic.process_destroy_list();
        assert_eq!(logic.tunnel_network.contain_count(Team::GLA), 0);
        assert!(logic.tunnel_network.honesty_cave_in_ok());
        let dead = logic.host_object(uid);
        assert!(
            dead.is_none() || dead.is_some_and(|o| o.status.destroyed || !o.is_alive()),
            "cave-in must kill remaining pool"
        );
    }

    #[test]
    fn bunker_buster_record_exit_removes_tunnel_pool() {
        // C++ TunnelContain.cpp:95 harmAndForceExitAllContained + record_exit.
        let (mut logic, t1, _t2, uid) = setup_two_tunnels_and_rider();
        let (kills, _, _) = logic.apply_bunker_buster_to_target(t1, Team::USA, 100.0, None);
        assert!(kills >= 1);
        assert!(
            !logic.tunnel_network.is_in_network(Team::GLA, uid),
            "bunker-buster must record_exit the shared pool occupant"
        );
    }
}

