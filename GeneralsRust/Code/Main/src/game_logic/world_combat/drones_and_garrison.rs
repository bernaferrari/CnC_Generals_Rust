//! Host combat `impl GameLogic` — `drones_and_garrison`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;


/// C++ GarrisonContain FIREPOINT residual: occupants shoot from points around
/// the container, not the building center. Closest ring point to the victim
/// wins (`calcBestGarrisonPosition`); occupant index breaks ties as station.
fn garrison_occupant_fire_point(
    container_pos: glam::Vec3,
    occupant_index: usize,
    occupant_count: usize,
    target_pos: glam::Vec3,
) -> glam::Vec3 {
    const FIRE_POINT_RADIUS: f32 = 12.0;
    const MAX_POINTS: usize = 8;
    let n = occupant_count.clamp(1, MAX_POINTS);
    let mut best = container_pos;
    let mut best_score = f32::MAX;
    for i in 0..n {
        let ang = (i as f32) * (std::f32::consts::TAU / n as f32);
        let p = glam::Vec3::new(
            container_pos.x + ang.cos() * FIRE_POINT_RADIUS,
            container_pos.y,
            container_pos.z + ang.sin() * FIRE_POINT_RADIUS,
        );
        let assigned = if i == occupant_index % n { -1.0 } else { 0.0 };
        let score = (p - target_pos).length_squared() + assigned;
        if score < best_score {
            best_score = score;
            best = p;
        }
    }
    best
}

impl GameLogic {
    /// Apply NeutronBlast residual at world impact: kill infantry + unman vehicles
    /// in blast radius. Returns (infantry_kills, vehicles_unmanned, vehicle_kills).
    ///
    /// Fail-closed: not full AffectAirborne / ally Relationship matrix.
    /// C++ NeutronCannonShell DumbProjectileBehavior residual (Bezier flight + blast).
    pub fn spawn_neutron_cannon_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_neutron_shell::{
            neutron_shell_flight_frames, NEUTRON_CANNON_SHELL_PROJECTILE, NEUTRON_SHELL_MAX_HEALTH,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(NEUTRON_CANNON_SHELL_PROJECTILE) {
            let mut t = ThingTemplate::new(NEUTRON_CANNON_SHELL_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(NEUTRON_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(NEUTRON_CANNON_SHELL_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on NukeCannonNeutronWeapon vs infantry (**10**).
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_neutron_shell::neutron_shell_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.neutron_shell_scatter_applied =
                self.neutron_shell_scatter_applied.saturating_add(1);
        }
        // Pure-splash apply (HOST_NEUTRON_BLAST_RADIUS 70). Miss counter peels when the
        // scatter aim lands outside primary splash residual (default 10).
        if target_is_infantry {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_neutron_shell::neutron_shell_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_neutron_shell::NEUTRON_BLAST_DEFAULT_RADIUS {
                        self.neutron_shell_scatter_misses =
                            self.neutron_shell_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 2.0;
        let pid = self.create_object(NEUTRON_CANNON_SHELL_PROJECTILE, team, start)?;
        let frames = neutron_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.neutron_cannon_shell_projectile = true;
            o.neutron_shell_from = Some([start.x, start.y, start.z]);
            o.neutron_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.neutron_shell_launch_frame = Some(self.frame);
            o.neutron_shell_flight_frames = frames;
            o.note_producer(source_id);
            o.health.maximum = NEUTRON_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, NEUTRON_SHELL_MAX_HEALTH);
            let dir = aim - start;
            o.set_orientation(dir.z.atan2(dir.x));
        }
        self.neutron_shells_spawned = self.neutron_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_neutron_cannon_shell_projectiles(&mut self) {
        use crate::game_logic::host_neutron_shell::neutron_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.neutron_cannon_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, glam::Vec3, Team)> = Vec::new();
        for id in flying {
            let (source, team, from, aim, launch, total) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .neutron_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .neutron_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                let launch = o.neutron_shell_launch_frame.unwrap_or(frame);
                let total = o.neutron_shell_flight_frames.max(1);
                (o.producer_id, o.team, from, aim, launch, total)
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / total as f32).clamp(0.0, 1.0);
            let pos = neutron_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
                o.movement.velocity = d;
            }
            if elapsed >= total || t >= 0.999 {
                impact.push((id, source, aim, team));
            }
        }
        for (id, source, pos, team) in impact {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.neutron_cannon_shell_projectile = false;
                o.set_position(pos);
            }
            // DetonateCallsKill residual: NeutronBlastBehavior on shell die.
            let caster_team = source
                .and_then(|sid| self.objects.get(&sid).map(|s| s.team))
                .unwrap_or(team);
            let _ = self.apply_neutron_blast_at(pos, caster_team, source, true);
            self.mark_object_for_destruction(id, Some(team));
        }
    }

    pub fn honesty_neutron_shell_projectile_ok(&self) -> bool {
        self.neutron_shells_spawned > 0
    }

    /// Apply NeutronBlast residual: kill infantry, unman vehicles, and
    /// `killAllContained` on every in-radius container (including structures).
    /// Returns (infantry_kills, vehicles_unmanned, vehicle_kills).
    pub fn apply_neutron_blast_at(
        &mut self,
        impact: glam::Vec3,
        caster_team: Team,
        caster_id: Option<ObjectId>,
        affect_allies: bool,
    ) -> (u32, u32, u32) {

        use crate::game_logic::host_neutron_shell::{
            in_neutron_blast_radius_2d, is_legal_neutron_blast_target, neutron_effect_for_target,
            NeutronEffect, HOST_NEUTRON_BLAST_RADIUS, NEUTRON_SHELL_AUDIO,
        };

        let center = (impact.x, impact.z);
        let radius = HOST_NEUTRON_BLAST_RADIUS;
        let mut infantry_kills = 0u32;
        let mut vehicles_unmanned = 0u32;
        let mut vehicle_kills = 0u32;
        let mut passengers_killed = 0u32;
        let mut destroy_ids: Vec<ObjectId> = Vec::new();
        let mut bomb_detonate_ids: Vec<ObjectId> = Vec::new();
        let mut unmanned_ids: Vec<ObjectId> = Vec::new();


        let candidates: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if caster_id == Some(*id) {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                if !in_neutron_blast_radius_2d(center, (pos.x, pos.z), radius) {
                    return None;
                }
                let same_team = obj.team == caster_team;
                if !is_legal_neutron_blast_target(
                    obj.is_alive(),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Drone),
                    obj.status.airborne_target,
                    false, // AffectAirborne = No for NeutronCannonShell residual
                    same_team,
                    affect_allies,
                ) {
                    return None;
                }
                Some(*id)
            })
            .collect();

        // C++ neutronBlastToObject: if contain → killAllContained, even on
        // structures / transports / drones that are otherwise not unmanned.
        let contain_pairs: Vec<(ObjectId, Vec<ObjectId>)> = candidates
            .iter()
            .filter_map(|id| {
                let occupants = self.objects.get(id).map(|o| o.contained_units())?;
                if occupants.is_empty() {
                    None
                } else {
                    Some((*id, occupants))
                }
            })
            .collect();

        for id in candidates {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let effect = neutron_effect_for_target(
                obj.is_kind_of(KindOf::Infantry),
                obj.is_kind_of(KindOf::Vehicle),
                obj.is_kind_of(KindOf::Drone),
                &obj.template_name,
            );
            match effect {
                NeutronEffect::KillInfantry => {
                    // Residual: kill infantry (take full health damage).
                    let _ = obj.take_damage_from(obj.health.current.max(1.0) * 10.0, caster_id);
                    if !obj.is_alive() || obj.health.current <= 0.0 {
                        infantry_kills = infantry_kills.saturating_add(1);
                        destroy_ids.push(id);
                    } else {
                        // Force kill residual.
                        let _ = obj.take_damage_from(999_999.0, caster_id);
                        infantry_kills = infantry_kills.saturating_add(1);
                        destroy_ids.push(id);
                    }
                }
                NeutronEffect::UnmanVehicle => {
                    if obj.is_car_bomb() {
                        // Dead-man trigger residual — detonate after this borrow ends.
                        bomb_detonate_ids.push(id);
                    } else {
                        obj.apply_kill_pilot_unmanned();
                        // C++ NeutonBlastBehavior.cpp:124-127 neutronBlastToObject:
                        //   getAI()->aiIdle(CMD_FROM_AI);
                        //   TheGameLogic->deselectObject(obj, PLAYERMASK_ALL, TRUE);
                        obj.set_ai_state(AIState::Idle);
                        obj.deselect();
                        // C++ NeutronBlastBehavior: setTeam(neutral) residual.
                        obj.team = Team::Neutral;
                        vehicles_unmanned = vehicles_unmanned.saturating_add(1);
                        unmanned_ids.push(id);
                    }
                }
                NeutronEffect::KillVehicle => {
                    let _ = obj.take_damage_from(obj.health.current.max(1.0) * 10.0, caster_id);
                    vehicle_kills = vehicle_kills.saturating_add(1);
                    destroy_ids.push(id);
                }
                NeutronEffect::None => {}
            }
        }

        for (container_id, occupants) in contain_pairs {
            if let Some(container) = self.objects.get_mut(&container_id) {
                for &occ_id in &occupants {
                    container.remove_occupant(occ_id);
                }
            }
            for occ_id in occupants {
                if destroy_ids.contains(&occ_id) {
                    continue;
                }
                let Some(occ) = self.objects.get_mut(&occ_id) else {
                    continue;
                };
                if !occ.is_alive() {
                    continue;
                }
                occ.set_contained_by(None);
                occ.set_ai_state(AIState::Idle);
                let _ = occ.take_damage_from(occ.health.current.max(1.0) * 10.0, caster_id);
                if occ.is_alive() && occ.health.current > 0.0 && !occ.status.destroyed {
                    let _ = occ.take_damage_from(999_999.0, caster_id);
                }
                passengers_killed = passengers_killed.saturating_add(1);
                infantry_kills = infantry_kills.saturating_add(1);
                destroy_ids.push(occ_id);
            }
        }


        // C++ deselectObject(PLAYERMASK_ALL): drop unmanned husks from every
        // selection roster so they cannot keep player/AI orders.
        if !unmanned_ids.is_empty() {
            self.selected_objects
                .retain(|sid| !unmanned_ids.contains(sid));
            for player in self.players.values_mut() {
                player
                    .selected_objects
                    .retain(|sid| !unmanned_ids.contains(sid));
            }
        }


        for id in destroy_ids {
            self.mark_object_for_destruction(id, Some(caster_team));
        }
        for id in bomb_detonate_ids {
            let _ = self.maybe_detonate_carbomb_on_unmanned(id);
        }

        self.neutron_shell_residual_blasts = self.neutron_shell_residual_blasts.saturating_add(1);
        self.neutron_shell_residual_infantry_kills = self
            .neutron_shell_residual_infantry_kills
            .saturating_add(infantry_kills);
        self.neutron_shell_residual_vehicles_unmanned = self
            .neutron_shell_residual_vehicles_unmanned
            .saturating_add(vehicles_unmanned);
        let _ = passengers_killed;


        self.queue_audio_event(
            AudioEventRequest::new(NEUTRON_SHELL_AUDIO)
                .with_position(impact)
                .with_priority(155),
        );
        let _ = self.combat_particles.spawn_weapon_fire_fx(
            impact,
            Some(impact),
            self.frame,
            caster_id.unwrap_or(ObjectId(0)),
            None,
        );

        (infantry_kills, vehicles_unmanned, vehicle_kills)
    }

    /// Residual fire-from-transport: docked passengers auto-engage nearest
    /// enemy in weapon range from the **container position** when the container
    /// has `passengers_allowed_to_fire` (Battle Bus / Combat Chinook / Humvee residual)
    /// or an installed Overlord BattleBunker (`OverlordContain.cpp:553`).
    /// Fail-closed: not C++ transport weapon bone positions / multi-slot matrix.
    pub(in super::super) fn try_transport_passenger_residual_fire(
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
        // C++ OverlordContain::isPassengerAllowedToFire — nested contain voids fire.
        let nested = container.contained_by.is_some();
        let bunker_slots = container.overlord_bunker_slot_capacity();
        let bunker_may = crate::game_logic::host_passengers_fire_upgrade::overlord_bunker_passengers_may_fire(
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
        if bunker_slots > 0 && !attacker.is_kind_of(KindOf::Infantry) {
            return;
        }
        let is_battle_bus = container.is_battle_bus_style_container();
        let is_combat_chinook = container.is_combat_chinook_style_container();
        let is_listening_outpost = container.is_listening_outpost_style_container();
        let team = attacker.team;
        let range = weapon.range;
        let damage = weapon.damage;
        let fire_pos = container.get_position();
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
        let (destroyed, kill_xp) = self.residual_auto_fire_apply_damage(
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
            self.award_experience(passenger_id, kill_xp);
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

    /// Residual fire-from-garrison: each occupant fires **their current weapon**
    /// from a FIREPOINT offset (C++ GarrisonContain `getCurrentWeapon` +
    /// `calcBestGarrisonPosition`), not a single PRIMARY from building center.
    /// Fail-closed: not C++ bone FIREPOINT01-nn artwork / enclosing shuffle.
    pub(in super::super) fn try_garrison_residual_fire(&mut self, garrisoned_id: ObjectId) {
        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

        let Some(attacker) = self.objects.get(&garrisoned_id) else {
            return;
        };
        if !attacker.is_alive() {
            return;
        }
        let container_id = attacker.container_id();
        if container_id
            .and_then(|cid| self.objects.get(&cid))
            .is_some_and(|container| container.status.disabled_subdued)
        {
            // C++ GarrisonContain::isPassengerAllowedToFire: DISABLED_SUBDUED
            // (flashbang / neutron) silences window fire.
            return;
        }
        let has_any_weapon = attacker.weapon_slot(0).is_some()
            || attacker.weapon_slot(1).is_some()
            || attacker.weapon_slot(2).is_some();
        if !has_any_weapon {
            return;
        }

        let team = attacker.team;
        let container_pos = container_id
            .and_then(|cid| self.objects.get(&cid).map(|c| c.get_position()))
            .unwrap_or_else(|| attacker.get_position());
        let occupants = container_id
            .and_then(|cid| self.objects.get(&cid).map(|c| c.contained_units()))
            .unwrap_or_default();
        let occupant_index = occupants
            .iter()
            .position(|&id| id == garrisoned_id)
            .unwrap_or(0);
        let occupant_count = occupants.len().max(1);

        // Pure residual acquire query (fire decision choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter(|(id, _)| **id != garrisoned_id && Some(**id) != container_id)
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

        // C++ GarrisonContain: occupant getCurrentWeapon + best FIREPOINT vs victim.
        let mut best: Option<(ObjectId, f32, u8, glam::Vec3, f32)> = None;
        for cand in &candidates {
            if !(cand.is_alive && cand.team != team && !cand.is_neutral && cand.combat_kind) {
                continue;
            }
            let fire_pos = garrison_occupant_fire_point(
                container_pos,
                occupant_index,
                occupant_count,
                cand.position,
            );
            let Some(target_obj) = self.objects.get(&cand.id) else {
                continue;
            };
            let Some(attacker) = self.objects.get(&garrisoned_id) else {
                return;
            };
            let slot = attacker
                .select_combat_weapon_slot(target_obj, current_time)
                .or_else(|| {
                    let s = attacker.active_weapon_slot;
                    attacker
                        .weapon_slot(s)
                        .filter(|w| Object::weapon_ready(w, current_time))
                        .map(|_| s)
                });
            let Some(slot) = slot else {
                continue;
            };
            let Some(weapon) = attacker.weapon_slot(slot) else {
                continue;
            };
            if !Object::weapon_ready(weapon, current_time) {
                continue;
            }
            let dist = fire_pos.distance(cand.position);
            if dist > weapon.range {
                continue;
            }
            if best.as_ref().map(|(_, d, _, _, _)| dist < *d).unwrap_or(true) {
                best = Some((cand.id, dist, slot, fire_pos, weapon.damage));
            }
        }

        let Some((target_id, _, slot, fire_pos, damage)) = best else {
            return;
        };

        let weapon_snap = self
            .objects
            .get(&garrisoned_id)
            .and_then(|a| a.weapon_slot(slot).cloned());
        let (destroyed, kill_xp) = self.residual_auto_fire_apply_damage(
            garrisoned_id,
            target_id,
            damage,
            fire_pos,
            weapon_snap.as_ref(),
            slot,
        );

        if let Some(attacker) = self.objects.get_mut(&garrisoned_id) {
            let _ = attacker.capture_pending_weapon_visual_dispatch(
                slot,
                self.frame,
                Some(target_id),
                None,
            );
            if let Some(w) = attacker.weapon_slot_mut(slot) {
                // Clip/ammo residual parity with fire_at path (not last_fire-only stamp).
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
            // AI attack authority: residual fire-intent for GameWorld last-writer.
            if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                let (dmg, rng) = attacker
                    .weapon_slot(slot)
                    .map(|w| (w.damage, w.range))
                    .unwrap_or((0.0, 0.0));
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let next_count = attacker.fire_intent_count.saturating_add(1);
                crate::game_logic::host_fire_intent_log::record(
                    attacker.id,
                    target_id.0,
                    slot,
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
                crate::game_logic::host_ai_decision_log::record_attack(garrisoned_id, target_id);
                crate::game_logic::host_ai_decision_log::record_set_state(garrisoned_id, 2);
            }
            // Kill XP awarded after this borrow via award_experience.
        }
        // Occupant discharges their own current slot from the FIREPOINT offset.
        let _ = self.record_accepted_weapon_discharge(garrisoned_id, slot);

        if destroyed {
            self.award_experience(garrisoned_id, kill_xp);
            self.mark_object_for_destruction(target_id, Some(team));
        }
        self.garrison_residual_fires = self.garrison_residual_fires.saturating_add(1);
        self.ensure_garrison_gun_effect(container_id, occupant_index, fire_pos);
    }

    /// C++ GarrisonContain::onContaining setTeam + academy + stealth hide.
    pub(in super::super) fn apply_garrison_contain_on_enter(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if !container.is_garrison_contain() {
            return;
        }
        self.recalc_garrison_apparent_controller(container_id);
        let occupant_owner = self
            .objects
            .get(&occupant_id)
            .and_then(|o| o.owner_player_id);
        let occupant_team = self.objects.get(&occupant_id).map(|o| o.team);
        if let Some(pid) = occupant_owner {
            if let Some(player) = self.players.get_mut(&pid) {
                player.record_building_garrisoned();
                return;
            }
        }
        if let Some(team) = occupant_team {
            if let Some(player) = self.players.values_mut().find(|p| p.team == team) {
                player.record_building_garrisoned();
            }
        }
    }

    /// C++ GarrisonContain::recalcApparentControllingPlayer.
    pub(in super::super) fn recalc_garrison_apparent_controller(&mut self, container_id: ObjectId) {
        let occupants = self
            .objects
            .get(&container_id)
            .map(|c| c.contained_units())
            .unwrap_or_default();
        if occupants.is_empty() {
            if let Some(container) = self.objects.get_mut(&container_id) {
                container.restore_garrison_original_team_if_empty();
            }
            return;
        }
        let first = occupants
            .first()
            .and_then(|id| self.objects.get(id))
            .map(|o| (o.team, o.owner_player_id, o.status.detected));
        let Some((first_team, first_owner, first_detected)) = first else {
            return;
        };
        let all_stealth = occupants.iter().all(|id| {
            self.objects
                .get(id)
                .is_some_and(|o| o.status.stealthed && !o.status.detected)
        });
        let hide = !first_detected && all_stealth;
        if let Some(container) = self.objects.get_mut(&container_id) {
            if let Some(bd) = container.building_data.as_mut() {
                if bd.original_team.is_none() {
                    bd.original_team = Some(container.team);
                }
                bd.hide_garrisoned_state = hide;
            }
            container.set_team_and_owner(first_team, first_owner);
        }
    }

    /// C++ putObjectAtGarrisonPoint + updateEffects GarrisonGun / FIRING_A.
    fn ensure_garrison_gun_effect(
        &mut self,
        container_id: Option<ObjectId>,
        point_index: usize,
        pos: glam::Vec3,
    ) {
        const MUZZLE_FLASH_LIFETIME: u32 = 30 / 7;
        let Some(cid) = container_id else {
            return;
        };
        self.expire_garrison_gun_muzzle_flashes(cid, MUZZLE_FLASH_LIFETIME);
        let existing = self
            .objects
            .get(&cid)
            .and_then(|c| c.building_data.as_ref())
            .and_then(|b| b.garrison_guns.get(point_index))
            .and_then(|g| g.drawable_id);
        let gun_id = existing.or_else(|| {
            if !self.templates.contains_key("GarrisonGun") {
                return None;
            }
            let team = self
                .objects
                .get(&cid)
                .map(|c| c.team)
                .unwrap_or(Team::Neutral);
            self.create_object("GarrisonGun", team, pos)
        });
        if let Some(gid) = gun_id {
            if let Some(gun) = self.objects.get_mut(&gid) {
                gun.set_position(pos);
                gun.model_condition_bits |=
                    1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FIRING_A;
            }
        }
        if let Some(container) = self.objects.get_mut(&cid) {
            if let Some(bd) = container.building_data.as_mut() {
                if bd.garrison_guns.len() <= point_index {
                    bd.garrison_guns
                        .resize(point_index + 1, crate::game_logic::GarrisonGunEffect::default());
                }
                let gun = &mut bd.garrison_guns[point_index];
                gun.drawable_id = gun_id;
                gun.last_effect_frame = self.frame;
                gun.firing = true;
            }
        }
    }

    fn expire_garrison_gun_muzzle_flashes(&mut self, container_id: ObjectId, lifetime: u32) {
        let frame = self.frame;
        let mut expire_ids = Vec::new();
        if let Some(container) = self.objects.get_mut(&container_id) {
            if let Some(bd) = container.building_data.as_mut() {
                for gun in &mut bd.garrison_guns {
                    if gun.firing && frame.saturating_sub(gun.last_effect_frame) > lifetime {
                        gun.firing = false;
                        if let Some(id) = gun.drawable_id {
                            expire_ids.push(id);
                        }
                    }
                }
            }
        }
        for id in expire_ids {
            if let Some(gun) = self.objects.get_mut(&id) {
                gun.model_condition_bits &=
                    !(1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FIRING_A);
            }
        }
    }

    /// Residual honesty: enter → garrisoned → exit path was exercised.
    pub fn honesty_garrison_enter_exit_ok(&self) -> bool {
        self.garrison_residual_enters > 0 && self.garrison_residual_exits > 0
    }

    /// Residual honesty: at least one fire-from-garrison residual shot.
    pub fn honesty_garrison_fire_ok(&self) -> bool {
        self.garrison_residual_fires > 0
    }

    /// Residual honesty: load → docked → unload path was exercised.
    pub fn honesty_transport_load_unload_ok(&self) -> bool {
        self.transport_residual_loads > 0 && self.transport_residual_unloads > 0
    }

    /// Residual honesty: Overlord BattleBunker enter → docked → exit path.
    /// Fail-closed: not full OverlordContain redirect / portable-structure spawn.
    pub fn honesty_overlord_bunker_enter_exit_ok(&self) -> bool {
        self.overlord_bunker_residual_enters > 0 && self.overlord_bunker_residual_exits > 0
    }

    // -----------------------------------------------------------------------
    // Mine / demo-trap / timed demo-charge residual
    // Fail-closed: not full MinefieldBehavior / DemoTrapUpdate / StickyBombUpdate.
    // -----------------------------------------------------------------------

    /// Residual honesty: at least one mine/trap/charge was placed.
    pub fn mine_residual_places(&self) -> u32 {
        self.mine_residual_places
    }

    /// Residual honesty: proximity-triggered detonations.
    pub fn mine_residual_proximity_detonations(&self) -> u32 {
        self.mine_residual_proximity_detonations
    }

    /// Residual honesty: timed-charge detonations.
    pub fn mine_residual_timed_detonations(&self) -> u32 {
        self.mine_residual_timed_detonations
    }

    /// Residual honesty: manual detonations (demo trap command residual).
    pub fn mine_residual_manual_detonations(&self) -> u32 {
        self.mine_residual_manual_detonations
    }

    /// Residual honesty: dozer/worker safe mine clears (disarm without detonation).
    pub fn mine_residual_clears(&self) -> u32 {
        self.mine_residual_clears
    }

    /// Residual honesty: place → enemy trigger → damage path exercised.
    pub fn honesty_mine_place_trigger_ok(&self) -> bool {
        self.mine_residual_places > 0 && self.mine_residual_proximity_detonations > 0
    }

    /// Residual honesty: place timed charge → detonation path exercised.
    pub fn honesty_timed_demo_charge_ok(&self) -> bool {
        self.mine_residual_places > 0 && self.mine_residual_timed_detonations > 0
    }

    /// Residual honesty: place enemy mine → dozer clear → mine gone, dozer lives.
    pub fn honesty_mine_clear_ok(&self) -> bool {
        self.mine_residual_places > 0 && self.mine_residual_clears > 0
    }

    /// Residual dozer structure-repair command accepts.
    pub fn repair_residual_structure_commands(&self) -> u32 {
        self.repair_residual_structure_commands
    }

    /// Residual structure HP heal ticks applied by dozer Repairing state.
    pub fn repair_residual_structure_heals(&self) -> u32 {
        self.repair_residual_structure_heals
    }

    /// Residual vehicle/aircraft SeekingRepair heal ticks at pad/war-factory/airfield.
    pub fn repair_residual_vehicle_heals(&self) -> u32 {
        self.repair_residual_vehicle_heals
    }

    /// Record a successful dozer structure Repair command acceptance.
    pub fn record_structure_repair_residual_command(&mut self) {
        self.repair_residual_structure_commands =
            self.repair_residual_structure_commands.saturating_add(1);
    }

    /// Record a structure HP heal tick from dozer Repairing residual.
    pub fn record_structure_repair_residual_heal(&mut self) {
        self.repair_residual_structure_heals =
            self.repair_residual_structure_heals.saturating_add(1);
    }

    /// Record a vehicle/aircraft pad heal tick from SeekingRepair residual.
    pub fn record_vehicle_repair_residual_heal(&mut self) {
        self.repair_residual_vehicle_heals = self.repair_residual_vehicle_heals.saturating_add(1);
    }

    /// Residual structure repair honesty: command issued and at least one HP heal tick.
    /// Fail-closed: not full C++ percent-heal / sole-benefactor / scaffolding parity.
    pub fn honesty_structure_repair_ok(&self) -> bool {
        self.repair_residual_structure_commands > 0 && self.repair_residual_structure_heals > 0
    }

    /// Residual vehicle pad repair honesty: at least one SeekingRepair heal tick.
    /// Fail-closed: not full RepairDockUpdate TimeForFullHeal / dock bones parity.
    pub fn honesty_vehicle_repair_ok(&self) -> bool {
        self.repair_residual_vehicle_heals > 0
    }

    /// Combined host repair residual path honesty (structure or vehicle pad).
    pub fn honesty_repair_ok(&self) -> bool {
        self.honesty_structure_repair_ok() || self.honesty_vehicle_repair_ok()
    }

    /// Residual ambulance AutoHeal infantry HP ticks applied.
    pub fn heal_residual_ambulance_heals(&self) -> u32 {
        self.heal_residual_ambulance_heals
    }

    /// Residual HealPad SeekingHealing HP ticks applied.
    pub fn heal_residual_heal_pad_heals(&self) -> u32 {
        self.heal_residual_heal_pad_heals
    }

    /// Record an ambulance radius AutoHeal infantry HP tick.
    pub fn record_ambulance_residual_heal(&mut self) {
        self.heal_residual_ambulance_heals = self.heal_residual_ambulance_heals.saturating_add(1);
    }

    /// Record a HealPad SeekingHealing HP tick.
    pub fn record_heal_pad_residual_heal(&mut self) {
        self.heal_residual_heal_pad_heals = self.heal_residual_heal_pad_heals.saturating_add(1);
    }

    /// Residual ambulance infantry heal honesty: at least one radius AutoHeal tick.
    /// Fail-closed: not full sole-benefactor / vehicle AutoHeal ModuleTag_23 parity.
    pub fn honesty_ambulance_heal_ok(&self) -> bool {
        self.heal_residual_ambulance_heals > 0
    }

    /// Residual HealPad infantry heal honesty: at least one SeekingHealing tick.
    pub fn honesty_heal_pad_ok(&self) -> bool {
        self.heal_residual_heal_pad_heals > 0
    }

    /// Combined host infantry heal residual honesty (ambulance radius or HealPad).
    pub fn honesty_heal_ok(&self) -> bool {
        self.honesty_ambulance_heal_ok() || self.honesty_heal_pad_ok()
    }

    /// Host propaganda tower residual heal honesty ticks.
    pub fn propaganda_residual_heals(&self) -> u32 {
        self.propaganda_residual_heals
    }

    /// Host propaganda tower residual buff honesty ticks.
    pub fn propaganda_residual_buffs(&self) -> u32 {
        self.propaganda_residual_buffs
    }

    pub(in super::super) fn record_propaganda_residual_heal(&mut self) {
        self.propaganda_residual_heals = self.propaganda_residual_heals.saturating_add(1);
    }

    pub(in super::super) fn record_propaganda_residual_buff(&mut self) {
        self.propaganda_residual_buffs = self.propaganda_residual_buffs.saturating_add(1);
    }

    /// Residual honesty: speaker/propaganda tower healed at least one unit.
    pub fn honesty_propaganda_heal_ok(&self) -> bool {
        self.propaganda_residual_heals > 0
    }

    /// Residual honesty: speaker/propaganda tower granted ENTHUSIASTIC/SUBLIMINAL buff.
    pub fn honesty_propaganda_buff_ok(&self) -> bool {
        self.propaganda_residual_buffs > 0
    }

    /// Combined host propaganda tower residual honesty (heal or buff).
    pub fn honesty_propaganda_ok(&self) -> bool {
        self.honesty_propaganda_heal_ok() || self.honesty_propaganda_buff_ok()
    }

    /// Host ECM tank residual jam honesty ticks (weapons_jammed grants).
    pub fn ecm_residual_jams(&self) -> u32 {
        self.ecm_residual_jams
    }

    pub(in super::super) fn record_ecm_residual_jam(&mut self) {
        self.ecm_residual_jams = self.ecm_residual_jams.saturating_add(1);
    }

    /// Residual honesty: ECM tank / jammer jammed enemy weapons at least once.
    pub fn honesty_ecm_jam_ok(&self) -> bool {
        self.ecm_residual_jams > 0
            || self.ecm_missiles_jammed > 0
            || self.ecm_laser_beams_spawned > 0
    }

    /// Residual honesty: ECMDisableStream laser spawned at least once.
    pub fn honesty_ecm_laser_ok(&self) -> bool {
        self.ecm_laser_beams_spawned > 0
    }

    /// Host Microwave Tank residual registry (disable structure honesty).
    pub fn microwave_residual(&self) -> &crate::game_logic::host_microwave::HostMicrowaveRegistry {
        &self.microwaves
    }

    /// Residual honesty: Microwave tank disabled an enemy structure at least once.
    pub fn honesty_microwave_disable_ok(&self) -> bool {
        self.microwaves.honesty_disable_ok()
    }

    /// Residual honesty: MicrowaveDisableStream laser spawned at least once.
    pub fn honesty_microwave_laser_ok(&self) -> bool {
        self.microwaves.honesty_laser_ok()
    }

    /// Residual honesty: emitter MICROWAVE field damaged at least once.
    pub fn honesty_microwave_emitter_ok(&self) -> bool {
        self.microwaves.honesty_emitter_ok()
    }

    /// Combined host path honesty for Microwave residual (disable).
    /// Garrison clear honesty is tracked separately via `honesty_kill_garrisoned_ok`.
    pub fn honesty_microwave_ok(&self) -> bool {
        self.microwaves.honesty_disable_ok()
            || self.microwaves.honesty_laser_ok()
            || self.microwaves.honesty_emitter_ok()
    }

    /// Host EMP Pulse residual registry (activate + honesty).
    pub fn emp_pulses(&self) -> &crate::game_logic::host_emp_pulse::HostEmpPulseRegistry {
        &self.emp_pulses
    }

    /// Residual honesty: EmpPulse activated at least once.
    pub fn honesty_emp_pulse_activate_ok(&self) -> bool {
        self.emp_pulses.honesty_activate_ok()
    }

    /// Residual honesty: EmpPulse applied DISABLED_EMP at least once.
    pub fn honesty_emp_pulse_disable_ok(&self) -> bool {
        self.emp_pulses.honesty_disable_ok()
    }

    /// Combined host path honesty for EmpPulse residual.
    pub fn honesty_emp_pulse_ok(&self) -> bool {
        self.emp_pulses.honesty_host_path_ok()
    }

    /// Residual honesty: Baikonur launch door and/or detonation recorded.

    pub fn honesty_defector_ok(&self) -> bool {
        self.defector_special.honesty_ok()
    }

    /// C++ DefectorSpecialPower::doSpecialPowerAtObject residual.
    pub fn activate_defector(&mut self, caster_id: ObjectId, victim_id: ObjectId) -> bool {
        use crate::game_logic::host_defector_special_power::DEFECTOR_DETECTION_FRAMES;
        if caster_id == victim_id {
            return false;
        }
        let Some(caster) = self.objects.get(&caster_id) else {
            return false;
        };
        if caster.is_disabled() {
            return false;
        }
        let caster_team = caster.team;
        let Some(victim) = self.objects.get_mut(&victim_id) else {
            return false;
        };
        if !victim.is_alive() {
            return false;
        }
        if victim.team == caster_team {
            return false;
        }
        let frames = DEFECTOR_DETECTION_FRAMES;
        let now = self.frame;
        victim.defect(caster_team, now, frames);
        self.defector_special.record(victim_id.0 as u32, frames);
        // Reveal residual: optional FOW around victim (FatCursorRadius).
        true
    }

    /// C++ SpecialPowerModule ctor path: StartsPaused → pauseCountdown(TRUE).

    /// C++ SupplyWarehouseCreate::onCreate residual.

    /// C++ SpecialPowerCompletionDie::onDie → notifyOfCompletedSpecialPower residual.
    pub(crate) fn maybe_notify_special_power_completion(&mut self, id: ObjectId) {
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let Some(ref data) = obj.special_power_completion else {
            return;
        };
        if !data.creator_set {
            return;
        }
        let power = data.special_power_name.clone();
        let creator = data.creator_id;
        let team = obj.team;
        let player_id = self
            .players
            .values()
            .find(|p| p.team == team)
            .map(|p| p.id)
            .unwrap_or(0);
        crate::game_logic::script_events::push_event(
            crate::game_logic::script_events::ScriptEvent::CompletedSpecialPower {
                player_id,
                special_power_name: power.clone(),
                creator_id: creator,
            },
        );
        self.special_power_completion_log
            .record_notify(&power, creator);
    }

    /// C++ PowerPlantUpdate::extendRods(true) residual — start rod animation timer.
    pub fn begin_power_plant_rods_extend(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        use crate::game_logic::host_special_power_completion_die::rods_extend_frames_for_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if obj.power_plant_rods_extended && obj.power_plant_rods_done_frame == 0 {
            // Already fully extended.
            return false;
        }
        if obj.power_plant_rods_done_frame > 0 {
            // Already extending.
            return false;
        }
        // Parsed `PowerPlantUpdate::RodsExtendTime` owns this value whenever
        // this object crossed the Object INI metadata boundary.  Keep the
        // older residual helper only for hand-authored templates used by
        // unrelated existing PowerPlantUpgrade paths.
        let frames = obj
            .thing
            .template
            .power_plant_update
            .map(|metadata| metadata.rods_extend_time_frames)
            .unwrap_or_else(|| rods_extend_frames_for_template(&obj.template_name));
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
            obj.model_condition_bits |= 1u128 << bit;
        }
        // Clear upgraded while animating.
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
            obj.model_condition_bits &= !(1u128 << bit);
        }
        obj.power_plant_rods_done_frame = self.frame.saturating_add(frames.max(1));
        obj.power_plant_rods_extended = true;
        self.special_power_completion_log.record_rods_start();
        true
    }

    /// C++ `PowerPlantUpdate::extendRods(false)` — retract immediately and
    /// clear both animation conditions.  Overcharge calls this only when the
    /// parsed object actually exposes the PowerPlantUpdate interface.
    pub fn retract_power_plant_rods(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
            obj.model_condition_bits &= !(1u128 << bit);
        }
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
            obj.model_condition_bits &= !(1u128 << bit);
        }
        obj.power_plant_rods_extended = false;
        obj.power_plant_rods_done_frame = 0;
        true
    }

    /// C++ PowerPlantUpdate::update residual — finish rod extend.
    pub fn update_power_plant_rods(&mut self) {
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        let now = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.power_plant_rods_done_frame > 0 && o.power_plant_rods_done_frame <= now
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
                    obj.model_condition_bits &= !(1u128 << bit);
                }
                if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
                    obj.model_condition_bits |= 1u128 << bit;
                }
                obj.power_plant_rods_done_frame = 0;
                self.special_power_completion_log.record_rods_complete();
            }
        }
    }

    pub(in super::super) fn init_supply_warehouse_create(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_structure_economy_residual::starting_supplies_for_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        // Retail `SupplyWarehouseDockUpdate::StartingBoxes` is authoritative.
        // Retain the legacy bootstrap table only for hand-authored templates
        // that have no parsed Behavior metadata; it never grants Dock ability.
        let supplies = if obj.thing.template.dock_kind
            == crate::game_logic::DockKind::SupplyWarehouse
        {
            obj.thing.template.dock_starting_boxes.map(|boxes| {
                boxes.saturating_mul(
                    crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX as u32,
                )
            })
        } else {
            starting_supplies_for_template(&obj.template_name)
        };
        let has_warehouse_create = obj.thing.template.has_supply_warehouse_create
            || obj.thing.template.dock_kind == crate::game_logic::DockKind::SupplyWarehouse;
        let had_supplies = supplies.is_some();
        if let Some(supplies) = supplies {
            // Only seed if empty (map may already set amount).
            if obj.stored_resources.supplies == 0 {
                obj.set_stored_supplies(supplies);
            }
        }
        if !has_warehouse_create && !had_supplies {
            return;
        }
        drop(obj);
        if has_warehouse_create || had_supplies {
            self.supply_create_warehouse_registers =
                self.supply_create_warehouse_registers.saturating_add(1);
        }
        if has_warehouse_create {
            for player in self.players.values_mut() {
                player.add_supply_warehouse(object_id);
            }
            if let Ok(list) = gamelogic::player::ThePlayerList().read() {
                for player_arc in list.iter() {
                    let Ok(mut player_guard) = player_arc.write() else {
                        continue;
                    };
                    let Some(manager) = player_guard.get_resource_manager_mut() else {
                        continue;
                    };
                    manager.add_supply_warehouse(object_id.0);
                }
            }
        }
    }

    /// C++ SupplyCenterCreate::onBuildComplete residual.
    pub(in super::super) fn on_supply_center_build_complete(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_upgrades::is_supply_center_template;
        let is_supply_center = self.objects.get(&object_id).is_some_and(|obj| {
            obj.thing.template.has_supply_center_create
                || obj.is_kind_of(KindOf::SupplyCenter)
                || obj.is_kind_of(KindOf::FSSupplyCenter)
                || is_supply_center_template(&obj.template_name)
        });
        if is_supply_center {
            self.supply_create_center_registers =
                self.supply_create_center_registers.saturating_add(1);
            // C++ walks every player ResourceGatheringManager::addSupplyCenter.
            for player in self.players.values_mut() {
                player.add_supply_center(object_id);
            }
            if let Ok(list) = gamelogic::player::ThePlayerList().read() {
                for player_arc in list.iter() {
                    let Ok(mut player_guard) = player_arc.write() else {
                        continue;
                    };
                    let Some(manager) = player_guard.get_resource_manager_mut() else {
                        continue;
                    };
                    manager.add_supply_center(object_id.0);
                }
            }
            // C++ SupplyCenter/Stash SpawnBehavior ModuleTag_12 only becomes
            // eligible after UNDER_CONSTRUCTION clears.  It creates the free
            // starter collector outside the paid ProductionUpdate queue.
            let _ = self.spawn_supply_center_one_shot_collector(object_id);
        }
    }

    /// C++ GenerateMinefieldBehavior::upgradeImplementation residual.
    pub(in super::super) fn place_structure_minefield_for_upgrade(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
    ) -> u32 {
        use crate::game_logic::host_mines::{
            is_china_mines_upgrade, structure_minefield_positions, CHINA_STANDARD_MINE_TEMPLATE,
            CHINA_STRUCTURE_MINE_RING_COUNT, CHINA_STRUCTURE_MINE_RING_RADIUS,
        };
        if !is_china_mines_upgrade(upgrade) {
            return 0;
        }
        let Some(obj) = self.objects.get(&object_id) else {
            return 0;
        };
        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
            return 0;
        }
        let team = obj.team;
        let pos = obj.get_position();
        // Ensure mine template exists.
        if !self.templates.contains_key(CHINA_STANDARD_MINE_TEMPLATE) {
            let mut m = ThingTemplate::new(CHINA_STANDARD_MINE_TEMPLATE);
            m.set_health(1.0);
            m.add_kind_of(KindOf::Projectile);
            self.templates
                .insert(CHINA_STANDARD_MINE_TEMPLATE.to_string(), m);
        }
        let spots = structure_minefield_positions(
            pos,
            CHINA_STRUCTURE_MINE_RING_COUNT,
            CHINA_STRUCTURE_MINE_RING_RADIUS,
        );
        let mut placed = 0u32;
        for spot in spots {
            if let Some(mid) = self.create_object(CHINA_STANDARD_MINE_TEMPLATE, team, spot) {
                if let Some(mine) = self.objects.get_mut(&mid) {
                    // Residual: mark as mine kind if available.
                    let _ = mine;
                }
                placed = placed.saturating_add(1);
            }
        }
        self.structure_minefield_placements =
            self.structure_minefield_placements.saturating_add(placed);
        placed
    }

    pub(in super::super) fn init_starts_paused_special_powers(&mut self, object_id: ObjectId) {
        use crate::command_system::SpecialPowerType as P;
        use crate::game_logic::host_upgrade_module_residuals::power_starts_paused;
        // C++ SpecialPowerModule starts the authored ReloadTime on creation
        // before applying StartsPaused.  HDB is not covered by the old
        // handwritten special-power table, so retain the paired Object INI
        // metadata here rather than falling through to a Hacker name.
        let hacker_disable = self
            .objects
            .get(&object_id)
            .and_then(|obj| obj.thing.template.hacker_disable_building.clone());
        if let Some(metadata) = hacker_disable {
            let owner_id = self
                .objects
                .get(&object_id)
                .and_then(|obj| self.player_owner_for_host_object(obj));
            if metadata.shared_n_sync {
                // SharedNSync belongs to the exact controller, never the
                // first player selected by faction.  Missing/stale ownership
                // remains unavailable through the typed readiness gate.
                if let Some(owner_id) = owner_id {
                    if let Some(player) = self.get_player_mut(owner_id) {
                        player.reset_shared_special_power_timer(
                            &P::HackerDisableBuilding,
                            metadata.reload_time_frames as f32 / 30.0,
                        );
                    }
                }
            } else if let Some(object) = self.objects.get_mut(&object_id) {
                if !object.status.under_construction {
                    object.start_power_recharge_with_frames(
                        &P::HackerDisableBuilding,
                        metadata.reload_time_frames,
                    );
                }
            }
            if metadata.starts_paused {
                if let Some(object) = self.objects.get_mut(&object_id) {
                    object.pause_special_power_countdown(&P::HackerDisableBuilding, true);
                }
            }
        }
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if obj.thing.template.capture_starts_paused {
            if let Some(power) = obj.thing.template.capture_power.special_power_type() {
                obj.pause_special_power_countdown(&power, true);
            }
        }
        let name = obj.template_name.to_ascii_lowercase();
        let candidates = [P::RadarScan, P::HelixNapalmBomb];
        for power in candidates {
            if !power_starts_paused(&power) {
                continue;
            }
            let relevant = match power {
                P::RadarScan => name.contains("radarvan") || name.contains("radar_van"),
                P::HelixNapalmBomb => name.contains("helix"),
                _ => false,
            };
            if relevant {
                obj.pause_special_power_countdown(&power, true);
            }
        }
    }

    /// C++ `ThingTemplate::calcCostToBuild` player modifier path.
    ///
    /// The exact `PlayerTemplate::ProductionCostChange` comes first, then the
    /// independently stacked KindOf upgrade modifiers.  The old host helper
    /// returned early for an unclassified template, which incorrectly skipped
    /// a General's exact-name discount even though C++ applies it before the
    /// KindOf query.
    pub fn modified_build_cost_supplies(
        &self,
        player_id: u32,
        template_name: &str,
        base_supplies: u32,
    ) -> u32 {
        use crate::game_logic::host_upgrade_module_residuals::{
            apply_production_cost_factor, kindof_cost_tokens,
        };
        let Some(player) = self.players.get(&player_id) else {
            return base_supplies;
        };
        let (is_vehicle, is_infantry, is_aircraft, is_structure) = self
            .templates
            .get(template_name)
            .map(|t| {
                (
                    t.is_kind_of(crate::game_logic::KindOf::Vehicle),
                    t.is_kind_of(crate::game_logic::KindOf::Infantry),
                    t.is_kind_of(crate::game_logic::KindOf::Aircraft),
                    t.is_kind_of(crate::game_logic::KindOf::Structure),
                )
            })
            .unwrap_or((false, false, false, false));
        let tokens = kindof_cost_tokens(is_vehicle, is_infantry, is_aircraft, is_structure);
        let kindof_factor = player.production_cost_factor(&tokens);
        let template_factor = self.player_template_production_cost_factor(player_id, template_name);
        apply_production_cost_factor(base_supplies, template_factor * kindof_factor)
    }

    /// C++ `ThingTemplate::calcTimeToBuild` authored pre-power frame count.
    ///
    /// Retail first converts `getBuildTime() * 30` to `Int`, then applies the
    /// selected PlayerTemplate's `ProductionTimeChange` and converts to `Int`
    /// again.  Keep this integer form available to both queued production and
    /// dozer construction, before either path applies the low-power penalty.
    pub(crate) fn cpp_build_time_frames_from_factor(base_seconds: f32, factor: f32) -> u32 {
        const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;

        if !base_seconds.is_finite() || !factor.is_finite() {
            // Invalid INI values must not manufacture a near-instant unit.
            return u32::MAX;
        }

        let base_frames = (base_seconds * LOGIC_FRAMES_PER_SECOND)
            .trunc()
            .clamp(0.0, u32::MAX as f32) as u32;
        ((base_frames as f32) * factor.max(0.0))
            .trunc()
            .clamp(0.0, u32::MAX as f32) as u32
    }

    /// Encode C++ `ThingTemplate::calcTimeToBuild`'s authored pre-power frame
    /// count in Main's legacy seconds carrier.
    ///
    /// Retail first converts `getBuildTime() * 30` to `Int`, then applies the
    /// selected PlayerTemplate's `ProductionTimeChange` and converts to `Int`
    /// again.  Main's queue stores seconds but its existing completion code
    /// recovers an integer frame count before applying the low-power penalty.
    /// Preserve that ordering by encoding the already-truncated pre-power
    /// frame count just above its lower frame boundary.  A direct
    /// `frames as f32 / 30.0` can round below that boundary (for example frame
    /// 63), causing the downstream `.trunc()` to lose a frame.
    pub(crate) fn cpp_build_time_seconds_from_factor(base_seconds: f32, factor: f32) -> f32 {
        const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;
        const FRAME_ENCODING_FRACTION: f32 = 0.25;
        let authored_frames = Self::cpp_build_time_frames_from_factor(base_seconds, factor);
        if authored_frames == 0 {
            return 0.0;
        }

        (authored_frames as f32 + FRAME_ENCODING_FRACTION) / LOGIC_FRAMES_PER_SECOND
    }

    /// C++ `ThingTemplate::calcTimeToBuild` authored PlayerTemplate stage.
    ///
    /// This returns a seconds carrier for `ProductionItem`; its existing
    /// logic-frame completion path then applies the C++ low-energy penalty.
    pub(crate) fn modified_build_time_seconds(
        &self,
        player_id: u32,
        template_name: &str,
        base_seconds: f32,
    ) -> f32 {
        let factor = self.player_template_production_time_factor(player_id, template_name);
        Self::cpp_build_time_seconds_from_factor(base_seconds, factor)
    }

    pub fn honesty_baikonur_ok(&self) -> bool {
        self.baikonur_launches.honesty_host_path_ok()
    }

    pub fn baikonur_launches(
        &self,
    ) -> &crate::game_logic::host_baikonur_launch::HostBaikonurLaunchRegistry {
        &self.baikonur_launches
    }

    /// C++ BaikonurLaunchPower::doSpecialPower residual — DOOR_1_OPENING on tower.
    pub fn activate_baikonur_launch_door(&mut self, source_id: ObjectId) -> bool {
        use crate::game_logic::host_enum_table_residual::door_1_opening_model_bit;
        let Some(obj) = self.objects.get_mut(&source_id) else {
            return false;
        };
        if obj.is_disabled() {
            return false;
        }
        let bit = door_1_opening_model_bit();
        obj.model_condition_bits |= 1u128 << bit;
        obj.refresh_model_condition_bits();
        self.baikonur_launches.record_launch_door();
        true
    }

    /// C++ BaikonurLaunchPower::doSpecialPowerAtLocation residual —
    /// spawn BaikonurRocketDetonation + NeutronMissileSlowDeath multi-blast.
    pub fn activate_baikonur_detonation(
        &mut self,
        source_id: ObjectId,
        location: glam::Vec3,
    ) -> bool {
        use crate::game_logic::host_baikonur_launch::{
            BAIKONUR_DETONATION_OBJECT, BAIKONUR_NUKE_FX,
        };
        let Some(src) = self.objects.get(&source_id) else {
            return false;
        };
        if src.is_disabled() {
            return false;
        }
        let team = src.team;
        // Ensure detonation template exists residual.
        if !self.templates.contains_key(BAIKONUR_DETONATION_OBJECT) {
            let mut t = crate::game_logic::ThingTemplate::new(BAIKONUR_DETONATION_OBJECT);
            t.set_health(1.0);
            t.add_kind_of(crate::game_logic::KindOf::Immobile);
            self.templates
                .insert(BAIKONUR_DETONATION_OBJECT.to_string(), t);
        }
        let det_id = match self.create_object(BAIKONUR_DETONATION_OBJECT, team, location) {
            Some(id) => id,
            None => return false,
        };
        // Arm Neutron multi-blast residual at detonation (same as nuke impact).
        let _ = self
            .special_power_strikes
            .spawn_neutron_slow_death_field(det_id, team, location, self.frame, 0);
        // Presentation FX residual name on detonation object.
        if let Some(d) = self.objects.get_mut(&det_id) {
            d.pending_death_fx = Some(BAIKONUR_NUKE_FX.to_string());
            // Lifetime 0 residual — mark for quick completion after blasts.
            d.ensure_lifetime_update(self.frame);
        }
        self.baikonur_launches
            .record_detonation(location.x, location.z);
        // Queue audio residual.
        self.queue_audio_event(
            crate::game_logic::AudioEventRequest::new("BaikonurRocketDetonation")
                .with_object(det_id)
                .with_position(location)
                .with_priority(200),
        );
        true
    }

    /// Activate EmpPulse residual: temporarily disable vehicles/structures in radius.
    ///
    /// Matches retail SuperweaponEMPPulse → EMPPulseEffectSpheroid EMPUpdate:
    /// - Radius residual 200 (RadiusCursorRadius / default EffectRadius)
    /// - DisabledDuration 30000 ms → 900 logic frames (DISABLED_EMP)
    /// - Vehicles + faction structures disabled; airborne aircraft killed residual
    ///
    /// Fail-closed: not full OCL bomb / spheroid drawable / spark particle path.
    /// Returns true when the residual activation was recorded (even if 0 targets).
    pub fn activate_emp_pulse(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        // C++ SUPERWEAPON_EMPPulse DeliverPayload residual: cargo plane + bomb first.
        if let Some(cid) = caster_id {
            if self
                .spawn_emp_pulse_flight(cid, location, player_id)
                .is_some()
            {
                return true;
            }
        }
        self.apply_emp_pulse_at(player_id, location, caster_id)
    }

    /// Apply EMP disable field residual at location (bomb impact / fail-open path).
    pub fn apply_emp_pulse_at(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_emp_pulse::{
            in_emp_pulse_radius_2d, is_emp_hardened_name, is_legal_emp_disable_target,
            should_emp_kill_airborne, HostEmpPulse, EMP_PULSE_ACTIVATE_AUDIO,
            EMP_PULSE_DISABLED_DURATION_FRAMES, HOST_EMP_PULSE_RADIUS,
        };

        let frame = self.frame;
        let until = frame.saturating_add(EMP_PULSE_DISABLED_DURATION_FRAMES);
        let center = (location.x, location.z);

        // Snapshot candidates (avoid borrow conflicts while mutating).
        let candidates: Vec<(
            ObjectId,
            bool, // is_vehicle
            bool, // is_faction_structure
            bool, // is_aircraft
            bool, // is_airborne
            bool, // is_spawns_are_weapons (name residual)
            bool, // under_construction
            bool, // emp_hardened
            f32,
            f32,
        )> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                // Residual: never EMP the caster object itself.
                if caster_id == Some(*id) {
                    return None;
                }
                let pos = obj.get_position();
                if !in_emp_pulse_radius_2d(center, (pos.x, pos.z), HOST_EMP_PULSE_RADIUS) {
                    return None;
                }
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                let is_structure = obj.is_kind_of(KindOf::Structure);
                let is_faction_structure = is_structure && obj.is_faction_structure();
                let is_aircraft = obj.is_kind_of(KindOf::Aircraft);
                let is_airborne = obj.status.airborne_target
                    || is_aircraft && {
                        // Host residual: treat Aircraft KindOf above ground as airborne
                        // when airborne_target is set; ground aircraft use status only.
                        obj.status.airborne_target
                    };
                let is_spawns = obj
                    .template_name
                    .to_ascii_lowercase()
                    .contains("spawnsaretheweapons")
                    || obj.template_name.to_ascii_lowercase().contains("stinger");
                let under_construction =
                    obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                let emp_hardened = is_emp_hardened_name(&obj.template_name);
                Some((
                    *id,
                    is_vehicle,
                    is_faction_structure,
                    is_aircraft,
                    is_airborne,
                    is_spawns,
                    under_construction,
                    emp_hardened,
                    pos.x,
                    pos.z,
                ))
            })
            .collect();

        let mut disables: u32 = 0;
        let mut airborne_kills: u32 = 0;
        let mut destroy_ids: Vec<ObjectId> = Vec::new();

        for (
            id,
            is_vehicle,
            is_faction_structure,
            is_aircraft,
            is_airborne,
            is_spawns,
            under_construction,
            emp_hardened,
            _tx,
            _tz,
        ) in candidates
        {
            if should_emp_kill_airborne(is_aircraft, is_airborne, emp_hardened) {
                destroy_ids.push(id);
                airborne_kills = airborne_kills.saturating_add(1);
                continue;
            }

            if !is_legal_emp_disable_target(
                is_vehicle,
                is_faction_structure,
                is_spawns,
                true,
                under_construction,
                emp_hardened,
            ) {
                continue;
            }

            // Ground aircraft / vehicles / faction structures: DISABLED_EMP.
            let Some(target) = self.objects.get_mut(&id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            let was_emp = target.status.disabled_emp;
            target.apply_disabled_emp(until);
            if !was_emp {
                disables = disables.saturating_add(1);
            } else {
                // Refresh still counts as a residual disable grant for honesty.
                disables = disables.saturating_add(1);
            }
        }

        for id in destroy_ids {
            // Source team residual: use caster team if available for kill credit.
            let killer_team = caster_id
                .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
                .unwrap_or(Team::Neutral);
            self.mark_object_for_destruction(id, Some(killer_team));
        }

        let pulse_id = self.emp_pulses.alloc_id();
        self.emp_pulses.record_activation(HostEmpPulse {
            id: pulse_id,
            player_id,
            location,
            radius: HOST_EMP_PULSE_RADIUS,
            activate_frame: frame,
            disable_until_frame: until,
            caster_id,
            disables,
            airborne_kills,
        });

        self.queue_audio_event(
            AudioEventRequest::new(EMP_PULSE_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(180),
        );
        // Direct-path residual also spawns EMPPulseEffectSpheroid (OCL on detonation).
        if let Some(pid) = caster_id {
            let _ = self.spawn_emp_pulse_spheroid(location, pid);
        } else {
            // No caster: spawn under Neutral producer residual via temp object skip —
            // still spawn at location with first friendly unit if any.
            if let Some((pid, _)) = self.objects.iter().next() {
                let _ = self.spawn_emp_pulse_spheroid(location, *pid);
            }
        }

        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            location,
            frame,
            caster_id,
            None,
        );

        true
    }

    /// Host China Frenzy ("Rage") residual registry (activate + honesty).
    pub fn frenzies(&self) -> &crate::game_logic::host_frenzy::HostFrenzyRegistry {
        &self.frenzies
    }

    /// Residual honesty: Frenzy activated at least once.
    pub fn honesty_frenzy_activate_ok(&self) -> bool {
        self.frenzies.honesty_activate_ok()
    }

    /// Residual honesty: Frenzy applied attack buff at least once.
    pub fn honesty_frenzy_buff_ok(&self) -> bool {
        self.frenzies.honesty_buff_ok()
    }

    /// Combined host path honesty for Frenzy / Rage residual.
    pub fn honesty_frenzy_ok(&self) -> bool {
        self.frenzies.honesty_host_path_ok()
    }

    /// Host USA Strategy Center battle-plan residual registry (select + honesty).
    pub fn battle_plans(&self) -> &crate::game_logic::host_strategy_center::HostBattlePlanRegistry {
        &self.battle_plans
    }

    /// Residual honesty: Strategy Center battle plan selected at least once.
    pub fn honesty_battle_plan_select_ok(&self) -> bool {
        self.battle_plans.honesty_select_ok()
    }

    /// Residual honesty: battle plan applied army residual buff at least once.
    pub fn honesty_battle_plan_buff_ok(&self) -> bool {
        self.battle_plans.honesty_buff_ok()
    }

    /// Residual honesty: BattlePlanChangeParalyze residual applied at least once.
    pub fn honesty_battle_plan_paralyze_ok(&self) -> bool {
        self.battle_plans.honesty_paralyze_ok()
    }

    /// Combined host path honesty for Strategy Center battle-plan residual.
    pub fn honesty_battle_plan_ok(&self) -> bool {
        self.battle_plans.honesty_host_path_ok()
    }

    /// Residual honesty: Bombardment turret StrategyCenterGun fired.
    pub fn honesty_battle_plan_turret_fire_ok(&self) -> bool {
        self.battle_plans.honesty_turret_fire_ok()
    }

    /// Residual honesty: StealthDetectorUpdate enabled (SearchAndDestroy residual).
    pub fn honesty_battle_plan_stealth_detector_ok(&self) -> bool {
        self.battle_plans.honesty_stealth_detector_ok()
    }

    /// Residual honesty: pack/unpack door residual started.
    pub fn honesty_battle_plan_door_ok(&self) -> bool {
        self.battle_plans.honesty_door_residual_ok()
    }

    /// Residual honesty: door residual reached ACTIVE / WAITING_TO_CLOSE.
    pub fn honesty_battle_plan_door_active_ok(&self) -> bool {
        self.battle_plans.honesty_door_active_ok()
    }

    /// Residual honesty: delayed setBattlePlan applied after unpack ACTIVE.
    pub fn honesty_battle_plan_delayed_active_ok(&self) -> bool {
        self.battle_plans.honesty_delayed_active_apply_ok()
    }

    /// Residual honesty: setBattlePlan(NONE) pack-clear residual fired.
    pub fn honesty_battle_plan_pack_clear_ok(&self) -> bool {
        self.battle_plans.honesty_pack_clear_ok()
    }

    /// Residual honesty: Bombardment turret recenter residual before pack.
    pub fn honesty_battle_plan_turret_recenter_ok(&self) -> bool {
        self.battle_plans.honesty_turret_recenter_ok()
    }

    /// Residual honesty: Strategy Center turret pitch/yaw left natural (aim residual).
    pub fn honesty_strategy_center_turret_aim_ok(&self) -> bool {
        self.objects.values().any(|o| {
            crate::game_logic::host_strategy_center::is_strategy_center_template(&o.template_name)
                && !crate::game_logic::host_strategy_center::turret_angles_are_natural(
                    o.turret_angle_deg,
                    o.turret_pitch_deg,
                )
        })
    }

    /// Residual honesty: TurretAI idle-scan residual started (Bombardment ACTIVE).
    pub fn honesty_strategy_center_turret_idle_scan_ok(&self) -> bool {
        self.battle_plans.honesty_turret_idle_scan_ok()
    }

    /// Residual honesty: TurretAI HoldTurret residual started (after idle-scan).
    pub fn honesty_strategy_center_turret_hold_ok(&self) -> bool {
        self.battle_plans.honesty_turret_hold_ok()
    }

    /// Residual honesty: TurretAI idle-recenter residual completed (after Hold).
    pub fn honesty_strategy_center_turret_idle_recenter_ok(&self) -> bool {
        self.battle_plans.honesty_turret_idle_recenter_ok()
    }

    /// Tick TurretAI idle mood-target residual for Bombardment ACTIVE Strategy Centers.
    ///
    /// C++ `TurretAI::friend_checkForIdleMoodTarget` residual:
    /// - When idle, acquire nearest legal enemy in StrategyCenterGun range band
    /// - Aim pitch/yaw at target (FirePitch **45**), flag `m_targetWasSetByIdleMood`
    /// - While held: re-aim each frame; clear when dead / OOR / illegal (team/air/UC)
    /// - Mood matrix Sleep → IgnoreAll (no acquire); Passive → WaitForAttack
    ///   (only last_damage_source residual); Normal/Alert/Aggressive → free
    /// - Fire residual ownership: bombardment fire clears mood flag if it engages
    ///   a different target (see `try_strategy_center_bombardment_turret_fire`)
    pub(in super::super) fn tick_strategy_center_turret_mood_target(&mut self) {
        use crate::game_logic::host_strategy_center::{
            is_strategy_center_template, strategy_center_gun_in_range,
            strategy_center_mood_target_eligible_with_attitude,
            strategy_center_mood_target_enemy_legal_with_vision,
            strategy_center_mood_target_in_vision,
            strategy_center_mood_target_should_clear_with_vision,
            strategy_center_mood_vision_range, strategy_center_turret_aim_at, HostAiAttitude,
            HostBattlePlan, HostBattlePlanTransition,
        };

        // Bombardment ACTIVE centers.
        let centers: Vec<ObjectId> = self
            .battle_plans
            .door_states()
            .iter()
            .filter(|s| {
                s.status == HostBattlePlanTransition::Active
                    && s.door_plan == Some(HostBattlePlan::Bombardment)
                    && !s.centering_turret
            })
            .map(|s| s.center_id)
            .collect();

        let mut acquires = 0u32;
        let mut clears = 0u32;
        for cid in centers {
            let Some(obj) = self.objects.get(&cid) else {
                continue;
            };
            if !obj.is_alive() || !is_strategy_center_template(&obj.template_name) {
                continue;
            }
            if obj.weapon.is_none() {
                continue;
            }
            let team = obj.team;
            let fire_pos = obj.get_position();
            let has_mood = obj.turret_mood_target;
            let attitude = HostAiAttitude::from_i8(obj.ai_attitude);
            let last_dmg = obj.last_damage_source;
            // Partition / AI vision residual: VisionRange **400**.
            // Bombardment ACTIVE path: S&D sight scalar does not apply (plans
            // are mutually exclusive). Host residual still uses the vision
            // filter helper so reduced-vision / S&D matrix stays host-testable.
            let vision_range = strategy_center_mood_vision_range(false);
            // "Busy" for acquire: only non-mood attacking (pack recenter / explicit
            // non-mood attack). Mood-set Attacking is the hold state, not busy.
            let busy_non_mood = !has_mood
                && (obj.status.attacking
                    || matches!(
                        obj.ai_state,
                        AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                    ));

            // Hold / clear / re-aim mood target residual.
            if has_mood {
                let tgt = obj.target;
                let mut clear = tgt.is_none();
                let mut aim_xz: Option<(f32, f32)> = None;
                if let Some(tid) = tgt {
                    if let Some(t) = self.objects.get(&tid) {
                        let tp = t.get_position();
                        let dx = tp.x - fire_pos.x;
                        let dz = tp.z - fire_pos.z;
                        let dist = (dx * dx + dz * dz).sqrt();
                        let is_air = t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target;
                        let legal = strategy_center_mood_target_enemy_legal_with_vision(
                            t.is_alive(),
                            t.team == team,
                            t.team == Team::Neutral,
                            t.status.under_construction,
                            is_air,
                            dist,
                            vision_range,
                        );
                        let in_range = strategy_center_gun_in_range(dist);
                        let in_vision = strategy_center_mood_target_in_vision(dist, vision_range);
                        clear = strategy_center_mood_target_should_clear_with_vision(
                            true,
                            t.is_alive(),
                            in_range,
                            in_vision,
                        ) || !legal;
                        if !clear {
                            aim_xz = Some((tp.x, tp.z));
                        }
                    } else {
                        clear = true;
                    }
                }
                if clear {
                    if let Some(o) = self.objects.get_mut(&cid) {
                        o.turret_mood_target = false;
                        o.set_status_attacking(false);
                        o.target = None;
                        if matches!(o.ai_state, AIState::Attacking) {
                            o.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_set_state(cid, 0);
                            }
                        }
                    }
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_stop_attack(cid);
                    }
                    clears = clears.saturating_add(1);
                } else if let Some((tx, tz)) = aim_xz {
                    // C++ AIM continuous aim residual while mood target held.
                    let (aim_a, aim_p) =
                        strategy_center_turret_aim_at(fire_pos.x, fire_pos.z, tx, tz);
                    if let Some(o) = self.objects.get_mut(&cid) {
                        o.turret_angle_deg = aim_a;
                        o.record_host_turret();
                        o.turret_pitch_deg = aim_p;
                        o.record_host_turret();
                        o.set_ai_state(AIState::Attacking);
                        o.set_status_attacking(true);
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            crate::game_logic::host_ai_decision_log::record_set_state(cid, 2);
                        }
                    }
                }
                continue; // no re-acquire this frame while mood flag set
            }

            // Passive WaitForAttack: only retaliate vs last_damage_source residual.
            let passive_last = last_dmg.is_some();
            if !strategy_center_mood_target_eligible_with_attitude(
                true,
                true,
                busy_non_mood,
                has_mood,
                attitude,
                passive_last,
            ) {
                continue;
            }

            // Find residual mood target: Passive uses last damage source only
            // (C++ getNextMoodTarget Passive branch); else nearest legal enemy.
            // Partition vision residual gates acquire distance.
            let mut best: Option<(ObjectId, f32, f32, f32)> = None; // id, dist, x, z
            if attitude.idle_mood_wait_for_attack() {
                if let Some(tid) = last_dmg {
                    if tid != cid {
                        if let Some(other) = self.objects.get(&tid) {
                            let op = other.get_position();
                            let dx = op.x - fire_pos.x;
                            let dz = op.z - fire_pos.z;
                            let dist = (dx * dx + dz * dz).sqrt();
                            let is_air =
                                other.is_kind_of(KindOf::Aircraft) || other.status.airborne_target;
                            if strategy_center_mood_target_enemy_legal_with_vision(
                                other.is_alive(),
                                other.team == team,
                                other.team == Team::Neutral,
                                other.status.under_construction,
                                is_air,
                                dist,
                                vision_range,
                            ) {
                                best = Some((tid, dist, op.x, op.z));
                            }
                        }
                    }
                }
            } else {
                // Pure residual acquire: nearest legal enemy in mood vision (XZ).
                let candidates: Vec<_> = self
                    .objects
                    .iter()
                    .map(|(&oid, other)| {
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id: oid,
                            team: other.team,
                            position: other.get_position(),
                            is_alive: other.is_alive(),
                            is_neutral: other.team == Team::Neutral,
                            under_construction: other.status.under_construction,
                            combat_kind: true,
                            effectively_stealthed: other.is_effectively_stealthed(),
                            is_air: other.is_kind_of(KindOf::Aircraft)
                                || other.status.airborne_target,
                            eject_invulnerable: other.is_eject_invulnerable(),
                        }
                    })
                    .collect();
                best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    Some(cid),
                    (fire_pos.x, fire_pos.z),
                    candidates,
                    vision_range,
                    |c| {
                        let dist = {
                            let dx = c.position.x - fire_pos.x;
                            let dz = c.position.z - fire_pos.z;
                            (dx * dx + dz * dz).sqrt()
                        };
                        strategy_center_mood_target_enemy_legal_with_vision(
                            c.is_alive,
                            c.team == team,
                            c.is_neutral,
                            c.under_construction,
                            c.is_air,
                            dist,
                            vision_range,
                        )
                    },
                )
                .map(|(id, dist, _)| {
                    let p = self
                        .objects
                        .get(&id)
                        .map(|o| o.get_position())
                        .unwrap_or(fire_pos);
                    (id, dist, p.x, p.z)
                });
            }
            if let Some((tid, _, tx, tz)) = best {
                let (aim_a, aim_p) = strategy_center_turret_aim_at(fire_pos.x, fire_pos.z, tx, tz);
                if let Some(o) = self.objects.get_mut(&cid) {
                    o.set_target(Some(tid));
                    o.turret_mood_target = true;
                    o.turret_angle_deg = aim_a;
                    o.record_host_turret();
                    o.turret_pitch_deg = aim_p;
                    o.record_host_turret();
                    // Mood acquire cancels idle-scan residual.
                    o.turret_idle_scanning = false;
                    o.record_host_turret();
                    o.turret_holding = false;
                    o.record_host_turret();
                    o.turret_hold_until_frame = 0;
                    o.turret_idle_recentering = false;
                    o.set_ai_state(AIState::Attacking);
                    o.set_status_attacking(true);
                }
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_attack(cid, tid);
                    crate::game_logic::host_ai_decision_log::record_set_state(cid, 2);
                }
                acquires = acquires.saturating_add(1);
            }
        }
        for _ in 0..acquires {
            self.battle_plans.record_turret_mood_target_acquire();
        }
        for _ in 0..clears {
            self.battle_plans.record_turret_mood_target_clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{AIState, GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    /// C++ NeutonBlastBehavior.cpp:124-127 — unmanned vehicles are aiIdle'd
    /// and deselectObject'd so they do not stay selected or keep orders.
    #[test]
    fn neutron_unman_deselects_and_idles_ai() {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));

        let mut tank = ThingTemplate::new("NeutronUnmanTank");
        tank.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(250.0);
        logic
            .templates
            .insert("NeutronUnmanTank".to_string(), tank);

        let tank_id = logic
            .create_object(
                "NeutronUnmanTank",
                Team::USA,
                Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("tank");
        logic.select_objects(0, vec![tank_id]);
        {
            let obj = logic.host_object_mut(tank_id).expect("tank mut");
            obj.set_ai_state(AIState::Moving);
            obj.target_location = Some(Vec3::new(80.0, 0.0, 0.0));
            obj.set_status_moving(true);
            assert!(obj.selected || obj.status.selected);
            assert!(matches!(obj.ai_state, AIState::Moving));
        }
        assert!(
            logic
                .players
                .get(&0)
                .unwrap()
                .selected_objects
                .contains(&tank_id)
        );

        let (kills, unmanned, vehicle_kills) =
            logic.apply_neutron_blast_at(Vec3::ZERO, Team::China, None, true);
        assert_eq!(kills, 0);
        assert_eq!(unmanned, 1);
        assert_eq!(vehicle_kills, 0);

        let obj = logic.host_object(tank_id).expect("husk");
        assert!(obj.is_unmanned(), "neutron must unman the vehicle");
        assert_eq!(obj.team, Team::Neutral);
        assert!(!obj.selected, "C++ deselectObject must clear object.selected");
        assert!(
            !obj.status.selected,
            "C++ deselectObject must clear status.selected"
        );
        assert!(
            matches!(obj.ai_state, AIState::Idle),
            "C++ aiIdle(CMD_FROM_AI) must idle the husk"
        );
        assert!(
            obj.target_location.is_none(),
            "idle unman must drop pending move orders"
        );
        assert!(
            !logic
                .players
                .get(&0)
                .unwrap()
                .selected_objects
                .contains(&tank_id),
            "PLAYERMASK_ALL deselect must drop the husk from the player roster"
        );
        assert!(!logic.selected_objects.contains(&tank_id));
    }

    /// C++ OverlordContain.cpp:553 — BattleBunker infantry fire from the tank.
    #[test]
    fn overlord_bunker_infantry_residual_fire_without_helix_flag() {
        let mut logic = GameLogic::new();
        let mut overlord = ThingTemplate::new("ChinaTankOverlord");
        overlord
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1100.0);
        logic
            .templates
            .insert("ChinaTankOverlord".to_string(), overlord);
        let mut red = ThingTemplate::new("ChinaRedguard");
        red.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(120.0);
        logic.templates.insert("ChinaRedguard".to_string(), red);
        let mut enemy = ThingTemplate::new("UsaRanger");
        enemy
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0);
        logic.templates.insert("UsaRanger".to_string(), enemy);

        let tank = logic
            .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("overlord");
        {
            let o = logic.host_object_mut(tank).unwrap();
            o.install_overlord_battle_bunker(5);
            o.passengers_allowed_to_fire = false;
        }
        let rider = logic
            .create_object("ChinaRedguard", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("rider");
        {
            let o = logic.host_object_mut(tank).unwrap();
            assert!(o.add_occupant(rider), "bunker must accept infantry");
        }
        {
            let r = logic.host_object_mut(rider).unwrap();
            r.contained_by = Some(tank);
            r.set_ai_state(AIState::Docked);
            if r.weapon.is_none() {
                r.weapon = Some(crate::game_logic::Weapon::default());
            }
            if let Some(w) = r.weapon.as_mut() {
                w.last_fire_time = -10.0;
                w.reload_time = 0.1;
                w.range = 150.0;
                w.damage = 10.0;
            }
        }
        let victim = logic
            .create_object("UsaRanger", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("victim");
        let hp_before = logic.host_object(victim).unwrap().health.current;
        logic.set_current_frame(30);
        logic.try_transport_passenger_residual_fire(rider);
        let hp_after = logic.host_object(victim).unwrap().health.current;
        assert!(
            hp_after < hp_before - 0.01,
            "bunker infantry must fire (before={hp_before} after={hp_after})"
        );
        assert!(
            logic.host_object(tank).unwrap().passengers_allowed_to_fire,
            "live bunker fire sets passengers_allowed_to_fire"
        );
    }
}

