//! Host combat `impl GameLogic` — `air_and_mig`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Apply Stealth Fighter residual fire (intended + primary splash) + bunker-buster structure path.
    /// C++ StealthJetMissile ProjectileObject residual (KillSelfDelay 2000ms).
    pub fn spawn_stealth_jet_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_stealth_fighter::{
            STEALTH_FIGHTER_PROJECTILE_SPEED, STEALTH_JET_MISSILE_KILL_SELF_DELAY_FRAMES,
            STEALTH_JET_MISSILE_MAX_HEALTH, STEALTH_JET_MISSILE_PROJECTILE,
            STEALTH_MISSILE_FUEL_FRAMES, STEALTH_MISSILE_IGNITION_DELAY_FRAMES,
            STEALTH_MISSILE_INITIAL_VELOCITY,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(STEALTH_JET_MISSILE_PROJECTILE) {
            let mut t = ThingTemplate::new(STEALTH_JET_MISSILE_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(STEALTH_JET_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(STEALTH_JET_MISSILE_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on StealthJetMissileWeapon vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_stealth_fighter::stealth_jet_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.stealth_jet_scatter_applied = self.stealth_jet_scatter_applied.saturating_add(1);
        }
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
            if crate::game_logic::host_stealth_fighter::stealth_jet_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist
                        > crate::game_logic::host_stealth_fighter::STEALTH_FIGHTER_PRIMARY_RADIUS
                    {
                        self.stealth_jet_scatter_misses =
                            self.stealth_jet_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y + 20.0);
        let pid = self.create_object(STEALTH_JET_MISSILE_PROJECTILE, team, start)?;
        let launch = STEALTH_MISSILE_INITIAL_VELOCITY / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.stealth_jet_missile_projectile = true;
            o.stealth_jet_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.stealth_jet_missile_intended = intended.map(|id| id.0);
            o.stealth_jet_missile_travelled = 0.0;
            o.stealth_jet_missile_fuel_expires_frame =
                Some(self.frame.saturating_add(STEALTH_MISSILE_FUEL_FRAMES));
            o.stealth_jet_missile_ignition_frame = Some(
                self.frame
                    .saturating_add(STEALTH_MISSILE_IGNITION_DELAY_FRAMES),
            );
            o.stealth_jet_missile_expires_frame = Some(
                self.frame.saturating_add(STEALTH_MISSILE_FUEL_FRAMES).max(
                    self.frame
                        .saturating_add(STEALTH_JET_MISSILE_KILL_SELF_DELAY_FRAMES),
                ),
            );
            o.note_producer(source_id);
            o.health.maximum = STEALTH_JET_MISSILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, STEALTH_JET_MISSILE_MAX_HEALTH);
            o.movement.velocity = dir * launch;
            o.set_orientation(dir.z.atan2(dir.x));
        }
        let _ = STEALTH_FIGHTER_PROJECTILE_SPEED;
        self.stealth_jet_missiles_spawned = self.stealth_jet_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_stealth_jet_missile_projectiles(&mut self) {
        use crate::game_logic::host_bunker_buster::{
            BUNKER_BUSTER_CRASH_THROUGH_FX, should_play_crash_through_fx,
        };
        use crate::game_logic::host_stealth_fighter::{
            STEALTH_FIGHTER_PROJECTILE_SPEED, STEALTH_JET_MISSILE_KILL_SELF_DELAY_FRAMES,
            STEALTH_MISSILE_INITIAL_VELOCITY,
        };
        let frame = self.frame;
        let launch = STEALTH_MISSILE_INITIAL_VELOCITY / 30.0;
        let cruise = STEALTH_FIGHTER_PROJECTILE_SPEED / 30.0;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.stealth_jet_missile_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        let mut crash_fx: Vec<ObjectId> = Vec::new();
        for id in flying {
            // C++ MissileAIUpdate::detonate → KILL_SELF + MISSILE_KILLING_SELF.
            // BunkerBusterBehavior::update plays CrashThroughBunkerFX on the missile
            // every CrashThroughBunkerFXFrequency frames; bustTheBunker/DetonationFX
            // run when DetonateCallsKill kill() fires after KillSelfDelay.
            if self
                .objects
                .get(&id)
                .map(|o| o.is_missile_killing_self())
                .unwrap_or(false)
            {
                if should_play_crash_through_fx(frame, true) {
                    crash_fx.push(id);
                }
                let (source, intended, pos, done) = {
                    let Some(o) = self.objects.get(&id) else {
                        continue;
                    };
                    let intended = o.stealth_jet_missile_intended.map(ObjectId);
                    let pos = o
                        .stealth_jet_missile_aim
                        .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                        .unwrap_or_else(|| o.get_position());
                    let done = o
                        .stealth_jet_missile_expires_frame
                        .map(|f| f <= frame)
                        .unwrap_or(true);
                    (o.producer_id, intended, pos, done)
                };
                if done {
                    impact.push((id, source, intended, pos));
                }
                continue;
            }
            let (source, intended, aim, pos, fuel_done, ignited, kill_self) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .stealth_jet_missile_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let intended = o.stealth_jet_missile_intended.map(ObjectId);
                let fuel_done = o
                    .stealth_jet_missile_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                let ignited = o
                    .stealth_jet_missile_ignition_frame
                    .map(|f| f <= frame)
                    .unwrap_or(true);
                let kill_self = o
                    .stealth_jet_missile_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                (
                    o.producer_id,
                    intended,
                    aim,
                    o.get_position(),
                    fuel_done,
                    ignited,
                    kill_self,
                )
            };
            let aim = intended
                .and_then(|tid| {
                    self.objects
                        .get(&tid)
                        .filter(|t| t.is_alive())
                        .map(|t| t.get_position())
                })
                .unwrap_or(aim);
            let speed = if ignited { cruise } else { launch };
            let to_aim = aim - pos;
            let dist = to_aim.length();
            let step_speed = if dist > 0.001 { speed.min(dist) } else { speed };
            let vel = if dist > 0.001 {
                to_aim.normalize() * step_speed
            } else {
                glam::Vec3::new(0.0, -step_speed, 0.0)
            };
            let step = vel.length().max(step_speed);
            let new_pos = pos + vel;
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                o.set_position(new_pos);
                o.stealth_jet_missile_travelled += step;
                o.stealth_jet_missile_aim = Some([aim.x, aim.y, aim.z]);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            let near = dist <= speed + 0.001 || (aim - new_pos).length() < 8.0;
            if fuel_done || kill_self || near {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.set_status_missile_killing_self(true);
                    o.set_status_no_collisions(true);
                    o.stealth_jet_missile_expires_frame =
                        Some(frame.saturating_add(STEALTH_JET_MISSILE_KILL_SELF_DELAY_FRAMES));
                    o.set_position(aim);
                    o.stealth_jet_missile_aim = Some([aim.x, aim.y, aim.z]);
                }
                if should_play_crash_through_fx(frame, true) {
                    crash_fx.push(id);
                }
            }
        }
        for id in crash_fx {
            let _ = self.dispatch_fx_list_at_host_object(BUNKER_BUSTER_CRASH_THROUGH_FX, id, None);
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
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
                o.stealth_jet_missile_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_stealth_fighter_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_stealth_jet_missile_projectile_ok(&self) -> bool {
        self.stealth_jet_missiles_spawned > 0
    }

    pub fn apply_stealth_fighter_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_bunker_buster::{
            UPGRADE_AMERICA_BUNKER_BUSTERS, is_bunker_buster_carrier, should_apply_bunker_buster,
        };
        use crate::game_logic::host_stealth_fighter::{
            STEALTH_FIGHTER_DAMAGE, STEALTH_FIGHTER_DAMAGE_TYPE, STEALTH_FIGHTER_DEATH_TYPE,
            STEALTH_FIGHTER_FIRE_AUDIO, STEALTH_FIGHTER_PRIMARY_RADIUS,
            is_legal_stealth_fighter_target, is_stealth_fighter_template,
            stealth_fighter_damage_at, stealth_jet_scatter_aim,
            stealth_jet_scatter_misses_infantry,
        };

        let (source_team, has_bunker_buster, is_carrier) = {
            if let Some(sid) = source {
                if let Some(obj) = self.objects.get(&sid) {
                    (
                        obj.team,
                        obj.has_upgrade_tag(UPGRADE_AMERICA_BUNKER_BUSTERS)
                            || obj.has_upgrade_tag("Upgrade_AmericaBunkerBusters"),
                        is_bunker_buster_carrier(&obj.template_name),
                    )
                } else {
                    (Team::Neutral, false, false)
                }
            } else {
                (Team::Neutral, false, false)
            }
        };

        let intended_is_structure = intended_target
            .and_then(|tid| self.objects.get(&tid))
            .map(|t| t.is_kind_of(KindOf::Structure))
            .unwrap_or(false);
        let bunker_buster_hit =
            should_apply_bunker_buster(has_bunker_buster, is_carrier, intended_is_structure);

        // C++ StealthJetMissileWeapon ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let mut intended_scatter_miss = false;
        if intended_is_infantry && !bunker_buster_hit {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source.map(|s| s.0).unwrap_or(0),
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = stealth_jet_scatter_aim(impact, true, seed);
            if scattered {
                self.stealth_jet_scatter_applied =
                    self.stealth_jet_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if stealth_jet_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > STEALTH_FIGHTER_PRIMARY_RADIUS {
                        self.stealth_jet_scatter_misses =
                            self.stealth_jet_scatter_misses.saturating_add(1);
                        intended_scatter_miss = true;
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        // Bunker-buster residual owns structure impact (garrison kill + bunker mult).
        // Skip splash so garrisoned occupants are not pre-killed outside contain bookkeeping.
        if bunker_buster_hit {
            if let Some(tid) = intended_target {
                let (kills, _structure_dmg, destroyed) = self.apply_bunker_buster_to_target(
                    tid,
                    source_team,
                    STEALTH_FIGHTER_DAMAGE,
                    source,
                );
                hits = hits.saturating_add(1).saturating_add(kills);
                if destroyed {
                    any_destroyed = true;
                }
            }
        } else {
            let candidates: Vec<(ObjectId, f32, bool)> = self
                .objects
                .iter()
                .filter_map(|(id, obj)| {
                    if source == Some(*id) {
                        return None;
                    }
                    let combat_kind = obj.is_kind_of(KindOf::Attackable)
                        || obj.is_kind_of(KindOf::Structure)
                        || obj.is_kind_of(KindOf::Infantry)
                        || obj.is_kind_of(KindOf::Vehicle)
                        || obj.is_kind_of(KindOf::Aircraft);
                    if !is_legal_stealth_fighter_target(
                        obj.is_alive(),
                        false,
                        obj.status.under_construction,
                        combat_kind,
                    ) {
                        return None;
                    }
                    let pos = obj.get_position();
                    let dist = {
                        let dx = impact_xz.0 - pos.x;
                        let dz = impact_xz.1 - pos.z;
                        (dx * dx + dz * dz).sqrt()
                    };
                    let is_intended = intended_target == Some(*id);
                    // Scatter miss residual: intended infantry outside splash is not force-hit.
                    if is_intended && intended_scatter_miss {
                        return None;
                    }
                    if is_intended || dist <= STEALTH_FIGHTER_PRIMARY_RADIUS {
                        Some((*id, dist, is_intended))
                    } else {
                        None
                    }
                })
                .collect();

            for (id, dist, is_intended) in candidates {
                let dmg = stealth_fighter_damage_at(if is_intended { 0.0 } else { dist });
                if dmg <= 0.0 {
                    continue;
                }
                if let Some(obj) = self.objects.get_mut(&id) {
                    let destroyed = obj.take_damage_from_immediate_residual(
                        dmg,
                        source,
                        STEALTH_FIGHTER_DAMAGE_TYPE,
                        STEALTH_FIGHTER_DEATH_TYPE,
                    );
                    hits = hits.saturating_add(1);
                    if destroyed {
                        any_destroyed = true;
                        destroy_ids.push((id, Some(source_team)));
                    }
                }
            }

            for (id, killer) in destroy_ids {
                self.mark_object_for_destruction(id, killer);
            }
        }

        self.stealth_fighter_residual_fires = self.stealth_fighter_residual_fires.saturating_add(1);
        self.stealth_fighter_residual_units_hit =
            self.stealth_fighter_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(STEALTH_FIGHTER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_stealth_fighter_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Comanche primary 20mm residual (intended-only).
    pub(in super::super) fn apply_comanche_cannon_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_comanche_rocket_pods::{
            COMANCHE_CANNON_DAMAGE, COMANCHE_CANNON_DAMAGE_TYPE, COMANCHE_CANNON_DEATH_TYPE,
            COMANCHE_CANNON_FIRE_AUDIO, is_comanche_template, is_legal_comanche_target,
        };

        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);
        let dmg = source
            .and_then(|sid| self.objects.get(&sid))
            .and_then(|o| o.weapon.as_ref().map(|w| w.damage))
            .unwrap_or(COMANCHE_CANNON_DAMAGE);

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        if let Some(tid) = intended_target {
            let legal = self
                .objects
                .get(&tid)
                .map(|obj| {
                    let combat_kind = obj.is_kind_of(KindOf::Attackable)
                        || obj.is_kind_of(KindOf::Structure)
                        || obj.is_kind_of(KindOf::Infantry)
                        || obj.is_kind_of(KindOf::Vehicle)
                        || obj.is_kind_of(KindOf::Aircraft);
                    is_legal_comanche_target(
                        obj.is_alive(),
                        source == Some(tid),
                        obj.status.under_construction,
                        combat_kind,
                    )
                })
                .unwrap_or(false);
            if legal {
                if let Some(obj) = self.objects.get_mut(&tid) {
                    let destroyed = obj.take_damage_from_immediate_residual(
                        dmg,
                        source,
                        COMANCHE_CANNON_DAMAGE_TYPE,
                        COMANCHE_CANNON_DEATH_TYPE,
                    );
                    hits = 1;
                    if destroyed {
                        any_destroyed = true;
                        destroy_ids.push((tid, Some(source_team)));
                    }
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.comanche_cannon_residual_fires = self.comanche_cannon_residual_fires.saturating_add(1);
        self.comanche_cannon_residual_units_hit =
            self.comanche_cannon_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(COMANCHE_CANNON_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(130),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_comanche_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Helix PRIMARY minigun residual (intended-only, dmg 6).
    pub(in super::super) fn apply_helix_minigun_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_helix_minigun::{
            HELIX_MINIGUN_DAMAGE, HELIX_MINIGUN_DAMAGE_TYPE, HELIX_MINIGUN_DEATH_TYPE,
            HELIX_MINIGUN_FIRE_AUDIO, is_legal_helix_minigun_target,
        };
        use crate::game_logic::host_overlord_addons::is_helix_template;

        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);
        let dmg = source
            .and_then(|sid| self.objects.get(&sid))
            .and_then(|o| o.weapon.as_ref().map(|w| w.damage))
            .unwrap_or(HELIX_MINIGUN_DAMAGE);

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        if let Some(tid) = intended_target {
            let legal = self
                .objects
                .get(&tid)
                .map(|obj| {
                    let combat_kind = obj.is_kind_of(KindOf::Attackable)
                        || obj.is_kind_of(KindOf::Structure)
                        || obj.is_kind_of(KindOf::Infantry)
                        || obj.is_kind_of(KindOf::Vehicle)
                        || obj.is_kind_of(KindOf::Aircraft);
                    is_legal_helix_minigun_target(
                        obj.is_alive(),
                        source == Some(tid),
                        obj.status.under_construction,
                        combat_kind,
                    )
                })
                .unwrap_or(false);
            if legal {
                if let Some(obj) = self.objects.get_mut(&tid) {
                    let destroyed = obj.take_damage_from_immediate_residual(
                        dmg,
                        source,
                        HELIX_MINIGUN_DAMAGE_TYPE,
                        HELIX_MINIGUN_DEATH_TYPE,
                    );
                    hits = 1;
                    if destroyed {
                        any_destroyed = true;
                        destroy_ids.push((tid, Some(source_team)));
                    }
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.helix_minigun_residual_fires = self.helix_minigun_residual_fires.saturating_add(1);
        self.helix_minigun_residual_units_hit =
            self.helix_minigun_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(HELIX_MINIGUN_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(130),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_helix_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Comanche anti-tank dual-radius residual (primary 50/5 + secondary 30/25).
    pub(in super::super) fn apply_comanche_antitank_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_comanche_rocket_pods::{
            COMANCHE_AT_DAMAGE_TYPE, COMANCHE_AT_DEATH_TYPE, COMANCHE_AT_FIRE_AUDIO,
            COMANCHE_AT_PRIMARY_RADIUS, COMANCHE_AT_SECONDARY_RADIUS, comanche_antitank_damage_at,
            comanche_antitank_scatter_aim, comanche_antitank_scatter_misses_infantry,
            is_comanche_template, is_legal_comanche_target,
        };

        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ ComancheAntiTankMissileWeapon ScatterRadiusVsInfantry residual.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let mut intended_scatter_miss = false;
        if intended_is_infantry {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source.map(|s| s.0).unwrap_or(0),
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = comanche_antitank_scatter_aim(impact, true, seed);
            if scattered {
                self.comanche_at_scatter_applied =
                    self.comanche_at_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if comanche_antitank_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > COMANCHE_AT_PRIMARY_RADIUS {
                        self.comanche_at_scatter_misses =
                            self.comanche_at_scatter_misses.saturating_add(1);
                        intended_scatter_miss = true;
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_comanche_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                // Scatter miss residual: intended infantry outside primary is not force-hit
                // (secondary splash-by-distance may still apply).
                if is_intended && intended_scatter_miss {
                    if dist > COMANCHE_AT_SECONDARY_RADIUS {
                        return None;
                    }
                    return Some((*id, dist, false));
                }
                if is_intended || dist <= COMANCHE_AT_SECONDARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = comanche_antitank_damage_at(if is_intended { 0.0 } else { dist });
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    COMANCHE_AT_DAMAGE_TYPE,
                    COMANCHE_AT_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.comanche_antitank_residual_fires =
            self.comanche_antitank_residual_fires.saturating_add(1);
        self.comanche_antitank_residual_units_hit = self
            .comanche_antitank_residual_units_hit
            .saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(COMANCHE_AT_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_comanche_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Battle Drone residual fire (intended-only machine gun).
    pub(in super::super) fn apply_battle_drone_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_slave_drones::{
            BATTLE_DRONE_FIRE_AUDIO, BATTLE_DRONE_GUN_DAMAGE, BATTLE_DRONE_GUN_DAMAGE_TYPE,
            BATTLE_DRONE_GUN_DEATH_TYPE, is_battle_drone_template,
        };

        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        if let Some(tid) = intended_target {
            if let Some(obj) = self.objects.get_mut(&tid) {
                if obj.is_alive() && !obj.status.under_construction {
                    let destroyed = obj.take_damage_from_immediate_residual(
                        BATTLE_DRONE_GUN_DAMAGE,
                        source,
                        BATTLE_DRONE_GUN_DAMAGE_TYPE,
                        BATTLE_DRONE_GUN_DEATH_TYPE,
                    );
                    hits = 1;
                    if destroyed {
                        any_destroyed = true;
                        destroy_ids.push((tid, Some(source_team)));
                    }
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.battle_drone_residual_fires = self.battle_drone_residual_fires.saturating_add(1);
        self.battle_drone_residual_units_hit =
            self.battle_drone_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(BATTLE_DRONE_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(120),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_battle_drone_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Tick residual Battle Drone master repair (weld SM, 12-unit closeEnough).
    ///
    /// C++ `SlavedUpdate::doRepairLogic` (`SlavedUpdate.cpp:413-495`): heal only
    /// when `distSqr < 12*12` and `m_repairing` after the first weld spark.
    /// Welding enter queues leftover MiscAudio RepairSparks at the weld pose
    /// (`SlavedUpdate.cpp:601-627` / leftover `spawn_welding_fx`).
    /// Fail-closed: not full arm pack/unpack weld FX / RepairMinAltitude matrix.
    pub fn update_battle_drone_repair_residual(&mut self, dt: f32) {
        use crate::game_logic::host_slave_drones::{
            BATTLE_DRONE_REPAIR_SPARKS_AUDIO, BATTLE_DRONE_REPAIR_WELDING_FX_BONE,
            BATTLE_DRONE_REPAIR_WELDING_SYS, battle_drone_repair_amount_for_frame,
            battle_drone_should_idle_repair_master, battle_drone_weld_close_enough,
            battle_drone_weld_pose, is_battle_drone_template,
        };

        // C++ doRepairLogic heals TheGameLogic->findObjectByID(m_slaver) only.
        let drones: Vec<(ObjectId, ObjectId, Vec3)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if !o.is_alive() || !is_battle_drone_template(&o.template_name) {
                    return None;
                }
                let slaver = o.producer_id?;
                Some((*id, slaver, o.get_position()))
            })
            .collect();
        if drones.is_empty() {
            self.battle_drone_weld_states.clear();
            return;
        }
        let live: std::collections::HashSet<ObjectId> =
            drones.iter().map(|(id, _, _)| *id).collect();
        self.battle_drone_weld_states
            .retain(|id, _| live.contains(id));

        let heal = battle_drone_repair_amount_for_frame(dt);

        for (drone_id, slaver_id, dpos) in drones {
            let Some(master) = self.objects.get(&slaver_id) else {
                self.battle_drone_weld_states.remove(&drone_id);
                continue;
            };
            if !master.is_alive() {
                if let Some(st) = self.battle_drone_weld_states.get_mut(&drone_id) {
                    st.end_repair();
                }
                continue;
            }
            let mpos = master.get_position();
            let max_hp = master.health.maximum.max(1.0);
            let mpct = (master.health.current / max_hp) * 100.0;
            let dx = dpos.x - mpos.x;
            let dz = dpos.z - mpos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            // C++ :229-236 idle weld continues until master is full (< 100).
            if !battle_drone_should_idle_repair_master(true, mpct, true, dist) {
                if let Some(st) = self.battle_drone_weld_states.get_mut(&drone_id) {
                    st.end_repair();
                }
                continue;
            }
            let close = battle_drone_weld_close_enough(dist);
            let (heal_now, play_sparks) = {
                let weld = self.battle_drone_weld_states.entry(drone_id).or_default();
                let heal_now = weld.tick(close);
                (heal_now, weld.weld_fx_this_tick)
            };
            if play_sparks && !BATTLE_DRONE_REPAIR_WELDING_SYS.is_empty() {
                let bone = self.objects.get(&drone_id).and_then(|drone| {
                    gamelogic::object::draw::lookup_pristine_bone_translation(
                        drone.thing.template.get_model_name(),
                        drone.thing.template.asset_scale,
                        BATTLE_DRONE_REPAIR_WELDING_FX_BONE,
                    )
                    .map(|c| glam::Vec3::new(c.x, c.z, c.y))
                });
                let pose = battle_drone_weld_pose(dpos, bone);
                let event = crate::game_logic::host_economy_log::resolve_misc_audio_event(
                    BATTLE_DRONE_REPAIR_SPARKS_AUDIO,
                );
                self.queue_audio_event(
                    AudioEventRequest::new(&event)
                        .with_object(drone_id)
                        .with_position(pose),
                );
            }
            if !heal_now || heal <= 0.0 {
                continue;
            }
            if let Some(master) = self.objects.get_mut(&slaver_id) {
                let before = master.health.current;
                let max_hp = master.health.maximum;
                let new_hp = (before + heal).min(max_hp);
                Self::write_object_health_authority_aware(master, new_hp);
                let gained = master.health.current - before;
                if gained > 0.0 {
                    crate::game_logic::host_heal_log::record(slaver_id, master.health.current);
                    self.battle_drone_residual_repairs =
                        self.battle_drone_residual_repairs.saturating_add(1);
                    self.battle_drone_residual_repair_amount += gained;
                }
            }
        }
    }

    /// Apply Uranium Shells residual tag + rebind Overlord / Emperor main gun.
    pub fn apply_overlord_gun_uranium_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_overlord_gun::{
            UPGRADE_CHINA_URANIUM_SHELLS, has_uranium_shells_upgrade, is_overlord_gun_chassis,
            overlord_gun_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_overlord_gun_chassis(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_URANIUM_SHELLS.to_string());
        let uranium = has_uranium_shells_upgrade(&obj.applied_upgrades);
        let mut w = overlord_gun_weapon(uranium);
        if let Some(old) = obj.weapon.as_ref() {
            w.last_fire_time = old.last_fire_time;
        }
        obj.weapon = Some(w);
        self.overlord_gun_residual_uranium_upgrades = self
            .overlord_gun_residual_uranium_upgrades
            .saturating_add(1);
        true
    }

    /// Apply Overlord / Emperor residual fire (dual-radius shell).
    /// C++ OverlordTankShell DumbProjectile residual.
    pub fn spawn_overlord_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_overlord_gun::{
            OVERLORD_PROJECTILE, OVERLORD_SHELL_MAX_HEALTH, overlord_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(OVERLORD_PROJECTILE) {
            let mut t = ThingTemplate::new(OVERLORD_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(OVERLORD_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(OVERLORD_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on OverlordTankGun vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_overlord_gun::overlord_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.overlord_scatter_applied = self.overlord_scatter_applied.saturating_add(1);
        }
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
            if crate::game_logic::host_overlord_gun::overlord_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_overlord_gun::OVERLORD_SECONDARY_RADIUS {
                        self.overlord_scatter_misses =
                            self.overlord_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object(OVERLORD_PROJECTILE, team, start)?;
        let frames = overlord_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.overlord_shell_projectile = true;
            o.overlord_shell_from = Some([start.x, start.y, start.z]);
            o.overlord_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.overlord_shell_launch_frame = Some(self.frame);
            o.overlord_shell_flight_frames = frames;
            o.overlord_shell_intended = intended.map(|id| id.0);
            o.note_producer(source_id);
            o.health.maximum = OVERLORD_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, OVERLORD_SHELL_MAX_HEALTH);
        }
        self.overlord_shells_spawned = self.overlord_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_overlord_shell_projectiles(&mut self) {
        use crate::game_logic::host_overlord_gun::overlord_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.overlord_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, from, aim, launch, frames) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .overlord_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .overlord_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.overlord_shell_intended.map(ObjectId),
                    from,
                    aim,
                    o.overlord_shell_launch_frame.unwrap_or(frame),
                    o.overlord_shell_flight_frames.max(1),
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = overlord_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
            }
            if elapsed >= frames {
                impact.push((id, source, intended, aim));
            }
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
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
                o.overlord_shell_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_overlord_gun_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_overlord_shell_projectile_ok(&self) -> bool {
        self.overlord_shells_spawned > 0
    }

    pub fn apply_overlord_gun_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_overlord_gun::{
            OVERLORD_DAMAGE_TYPE, OVERLORD_DEATH_TYPE, OVERLORD_FIRE_AUDIO,
            OVERLORD_SECONDARY_RADIUS, has_uranium_shells_upgrade,
            is_legal_overlord_gun_splash_target, is_overlord_gun_chassis, overlord_damage_at,
            overlord_scatter_aim, overlord_scatter_misses_infantry,
        };

        let (source_team, has_uranium) = {
            let Some(sid) = source else {
                return (0, false);
            };
            let Some(obj) = self.objects.get(&sid) else {
                return (0, false);
            };
            if !is_overlord_gun_chassis(&obj.template_name) {
                return (0, false);
            }
            (obj.team, has_uranium_shells_upgrade(&obj.applied_upgrades))
        };

        // C++ OverlordTankGun ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        if intended_is_infantry {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source.map(|s| s.0).unwrap_or(0),
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = overlord_scatter_aim(impact, true, seed);
            if scattered {
                self.overlord_scatter_applied = self.overlord_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if overlord_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > OVERLORD_SECONDARY_RADIUS {
                        self.overlord_scatter_misses =
                            self.overlord_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_overlord_gun_splash_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                // Scatter miss residual: intended infantry outside splash is not force-hit.
                if is_intended && intended_is_infantry && dist > OVERLORD_SECONDARY_RADIUS {
                    return None;
                }
                if is_intended || dist <= OVERLORD_SECONDARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = overlord_damage_at(if is_intended { 0.0 } else { dist }, has_uranium);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    OVERLORD_DAMAGE_TYPE,
                    OVERLORD_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.overlord_gun_residual_fires = self.overlord_gun_residual_fires.saturating_add(1);
        self.overlord_gun_residual_units_hit =
            self.overlord_gun_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(OVERLORD_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// Apply AP Bullets residual tag + rebind Jarmen Kell sniper.
    pub fn apply_jarmen_kell_ap_bullets_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_jarmen_kell::{
            UPGRADE_GLA_AP_BULLETS, has_ap_bullets_upgrade, is_jarmen_kell_template,
            jarmen_kell_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_jarmen_kell_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_AP_BULLETS.to_string());
        let ap = has_ap_bullets_upgrade(&obj.applied_upgrades);
        let mut w = jarmen_kell_weapon(ap);
        if let Some(old) = obj.weapon.as_ref() {
            w.last_fire_time = old.last_fire_time;
        }
        obj.weapon = Some(w);
        self.jarmen_kell_residual_ap_upgrades =
            self.jarmen_kell_residual_ap_upgrades.saturating_add(1);
        true
    }

    /// Apply Jarmen Kell residual fire (intended-only sniper).
    pub(in super::super) fn apply_jarmen_kell_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_jarmen_kell::{
            JARMEN_KELL_DAMAGE_TYPE, JARMEN_KELL_DEATH_TYPE, JARMEN_KELL_FIRE_AUDIO,
            has_ap_bullets_upgrade, is_jarmen_kell_template, is_legal_jarmen_kell_target,
            jarmen_kell_damage_with_ap,
        };

        let (source_team, damage) = {
            let Some(sid) = source else {
                return (0, false);
            };
            let Some(obj) = self.objects.get(&sid) else {
                return (0, false);
            };
            if !is_jarmen_kell_template(&obj.template_name) {
                return (0, false);
            }
            let ap = has_ap_bullets_upgrade(&obj.applied_upgrades);
            let dmg = obj
                .weapon
                .as_ref()
                .map(|w| w.damage)
                .unwrap_or_else(|| jarmen_kell_damage_with_ap(ap));
            (obj.team, dmg)
        };

        let Some(tid) = intended_target else {
            return (0, false);
        };

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        if let Some(obj) = self.objects.get_mut(&tid) {
            if source != Some(tid) {
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if is_legal_jarmen_kell_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    let destroyed = obj.take_damage_from_immediate_residual(
                        damage,
                        source,
                        JARMEN_KELL_DAMAGE_TYPE,
                        JARMEN_KELL_DEATH_TYPE,
                    );
                    hits = 1;
                    if destroyed {
                        any_destroyed = true;
                        destroy_ids.push((tid, Some(source_team)));
                    }
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.jarmen_kell_residual_fires = self.jarmen_kell_residual_fires.saturating_add(1);
        self.jarmen_kell_residual_units_hit =
            self.jarmen_kell_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(JARMEN_KELL_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// Refresh Battlemaster weapon residual from current uranium / horde / nationalism flags.
    pub(crate) fn refresh_battlemaster_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_battlemaster::{
            battlemaster_weapon, has_fanaticism_upgrade, has_nationalism_upgrade,
            has_uranium_shells_upgrade, is_battlemaster_template, leftover_horde_fanaticism_bonus,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !is_battlemaster_template(&obj.template_name) {
            return;
        }
        let uranium = has_uranium_shells_upgrade(&obj.applied_upgrades);
        let nationalism = has_nationalism_upgrade(&obj.applied_upgrades);
        let in_horde = obj.weapon_bonus_horde;
        // C++ evaluateMoraleBonus: nationalism from upgrade; AllowedNationalism
        // vetoes only while in horde (default TRUE). Fanaticism nests inside.
        let nationalism_active = super::tanks_and_upgrades::nationalism_bonus_from_upgrade(
            nationalism,
            in_horde,
            super::tanks_and_upgrades::HORDE_DEFAULT_ALLOWED_NATIONALISM,
        );
        obj.weapon_bonus_nationalism = nationalism_active;
        obj.weapon_bonus_fanaticism = leftover_horde_fanaticism_bonus(
            nationalism_active,
            has_fanaticism_upgrade(&obj.applied_upgrades),
        );
        obj.record_host_weapon_bonus();
        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut w = battlemaster_weapon(uranium, in_horde, nationalism_active);
        w.last_fire_time = last_fire;
        obj.weapon = Some(w);
    }

    /// Apply Uranium Shells residual (PLAYER_UPGRADE DAMAGE 125%) to a Battlemaster.
    pub fn apply_battlemaster_uranium_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_battlemaster::{
            UPGRADE_CHINA_URANIUM_SHELLS, is_battlemaster_template,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_battlemaster_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_URANIUM_SHELLS.to_string());
        self.battlemaster_residual_uranium_upgrades = self
            .battlemaster_residual_uranium_upgrades
            .saturating_add(1);
        self.refresh_battlemaster_weapon(object_id);
        true
    }

    /// Apply Nationalism residual tag (ROF stacks with horde when active).
    pub fn apply_battlemaster_nationalism_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_battlemaster::{UPGRADE_NATIONALISM, is_battlemaster_template};
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_battlemaster_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades.insert(UPGRADE_NATIONALISM.to_string());
        self.battlemaster_residual_nationalism_upgrades = self
            .battlemaster_residual_nationalism_upgrades
            .saturating_add(1);
        self.refresh_battlemaster_weapon(object_id);
        true
    }

    /// Recompute China vehicle HordeUpdate residual (Battlemaster + other ExactMatch
    /// HordeUpdate vehicles). Radius 75 / Count 5 / RubOff 20; terrain-decal fade.
    pub fn update_battlemaster_horde_status(&mut self) {
        use crate::game_logic::host_battlemaster::{
            BATTLE_MASTER_HORDE_COUNT, BATTLE_MASTER_HORDE_RADIUS,
            BATTLE_MASTER_HORDE_RUB_OFF_RADIUS, BATTLE_MASTER_HORDE_UPDATE_FRAMES,
            LeftoverHordeScanUnit, counts_toward_battlemaster_horde,
            evaluate_leftover_horde_blob_scan, is_battlemaster_template,
            is_china_vehicle_horde_unit, leftover_horde_bounding_sphere_radius,
            leftover_horde_draw_icon_ui, leftover_horde_take_wake, same_vehicle_horde_family,
        };

        let snapshot: Vec<(ObjectId, Team, Option<u32>, LeftoverHordeScanUnit, String)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if !o.is_alive() || !is_china_vehicle_horde_unit(&o.template_name) {
                    return None;
                }
                let p = o.get_position();
                let geom = &o.thing.template.geometry_info;
                Some((
                    *id,
                    o.team,
                    o.owner_player_id,
                    LeftoverHordeScanUnit {
                        x: p.x,
                        y: p.y,
                        z: p.z,
                        sphere_radius: leftover_horde_bounding_sphere_radius(
                            geom.authored,
                            geom.bounding_sphere_radius(),
                            o.selection_radius,
                        ),
                        alive: o.is_alive(),
                    },
                    o.template_name.clone(),
                ))
            })
            .collect();

        let units: Vec<LeftoverHordeScanUnit> = snapshot.iter().map(|u| u.3).collect();
        let membership = evaluate_leftover_horde_blob_scan(
            &units,
            BATTLE_MASTER_HORDE_COUNT,
            BATTLE_MASTER_HORDE_RADIUS,
            BATTLE_MASTER_HORDE_RUB_OFF_RADIUS,
            |i, j, dist| {
                counts_toward_battlemaster_horde(
                    snapshot[i].3.alive,
                    snapshot[j].3.alive,
                    self.horde_allies_only(
                        snapshot[i].2,
                        snapshot[i].1,
                        snapshot[j].2,
                        snapshot[j].1,
                    ),
                    same_vehicle_horde_family(&snapshot[i].4, &snapshot[j].4),
                    dist,
                    BATTLE_MASTER_HORDE_RADIUS,
                )
            },
        );

        let mut grants = 0u32;
        let mut to_refresh: Vec<ObjectId> = Vec::new();
        let draw_icon = leftover_horde_draw_icon_ui();
        let frame = self.frame;

        for (idx, (id, _team, _owner, _scan, name)) in snapshot.iter().enumerate() {
            let scanned = membership[idx].in_horde;
            if let Some(obj) = self.objects.get_mut(id) {
                let (due, init, last, next) = leftover_horde_take_wake(
                    obj.horde_wake_initialized,
                    false,
                    frame,
                    obj.last_horde_refresh_frame,
                    obj.horde_next_wake_frame,
                    BATTLE_MASTER_HORDE_UPDATE_FRAMES,
                );
                obj.horde_wake_initialized = init;
                obj.last_horde_refresh_frame = last;
                obj.horde_next_wake_frame = next;
                let was = obj.weapon_bonus_horde;
                let now_horde = if due { scanned } else { was };
                if due {
                    obj.weapon_bonus_horde = now_horde;
                    if now_horde && !was {
                        grants = grants.saturating_add(1);
                    }
                    if is_battlemaster_template(name) && (now_horde != was || now_horde) {
                        to_refresh.push(*id);
                    }
                }
                // C++ HordeUpdate::update calls evaluateMoraleBonus after the
                // membership scan (when due) and then stamps the decal from
                // NATIONALISM/FANATICISM. Dragon/Inferno/Gattling/Overlord
                // share this path — they are not Battlemaster-only.
                if due {
                    super::tanks_and_upgrades::apply_evaluate_morale_bonus(obj);
                }
                obj.apply_horde_terrain_decal(was, now_horde, draw_icon);
            }
        }

        self.battlemaster_residual_horde_grants = self
            .battlemaster_residual_horde_grants
            .saturating_add(grants);

        for id in to_refresh {
            self.refresh_battlemaster_weapon(id);
        }
    }

    /// Apply Battlemaster residual fire (primary on intended + small splash radius).
    ///
    /// Damage uses current weapon residual (base 60 or uranium 75).
    /// C++ GenericTankShell DumbProjectile residual (Crusader/Paladin gun).
    pub fn spawn_usa_tank_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        weapon_speed: f32,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_usa_tanks::{
            USA_SHELL_MAX_HEALTH, USA_TANK_GUN_PROJECTILE, usa_tank_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(USA_TANK_GUN_PROJECTILE) {
            let mut t = ThingTemplate::new(USA_TANK_GUN_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(USA_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(USA_TANK_GUN_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on Crusader/PaladinTankGun vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_usa_tanks::usa_tank_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.usa_tank_scatter_applied = self.usa_tank_scatter_applied.saturating_add(1);
        }
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
            if crate::game_logic::host_usa_tanks::usa_tank_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_usa_tanks::USA_TANK_GUN_PRIMARY_RADIUS {
                        self.usa_tank_scatter_misses =
                            self.usa_tank_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object(USA_TANK_GUN_PROJECTILE, team, start)?;
        let frames = usa_tank_shell_flight_frames(start, aim, weapon_speed).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.usa_tank_shell_projectile = true;
            o.usa_tank_shell_from = Some([start.x, start.y, start.z]);
            o.usa_tank_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.usa_tank_shell_launch_frame = Some(self.frame);
            o.usa_tank_shell_flight_frames = frames;
            o.usa_tank_shell_weapon_speed = weapon_speed;
            o.usa_tank_shell_intended = intended.map(|id| id.0);
            o.note_producer(source_id);
            o.health.maximum = USA_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, USA_SHELL_MAX_HEALTH);
        }
        self.usa_tank_shells_spawned = self.usa_tank_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_usa_tank_shell_projectiles(&mut self) {
        use crate::game_logic::host_usa_tanks::usa_tank_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.usa_tank_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, from, aim, launch, frames) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .usa_tank_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .usa_tank_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.usa_tank_shell_intended.map(ObjectId),
                    from,
                    aim,
                    o.usa_tank_shell_launch_frame.unwrap_or(frame),
                    o.usa_tank_shell_flight_frames.max(1),
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = usa_tank_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
            }
            if elapsed >= frames {
                impact.push((id, source, intended, aim));
            }
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
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
                o.usa_tank_shell_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_usa_tank_gun_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_usa_tank_shell_projectile_ok(&self) -> bool {
        self.usa_tank_shells_spawned > 0 || self.usa_tank_scatter_applied > 0
    }

    /// Residual honesty: USA tank ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_usa_tank_scatter_ok(&self) -> bool {
        self.usa_tank_scatter_applied > 0 || self.usa_tank_scatter_misses > 0
    }

    /// Apply USA Crusader/Paladin tank gun splash residual at impact.
    pub fn apply_usa_tank_gun_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_usa_tanks::{
            CRUSADER_FIRE_AUDIO, PALADIN_FIRE_AUDIO, USA_TANK_GUN_DAMAGE, USA_TANK_GUN_DAMAGE_TYPE,
            USA_TANK_GUN_DEATH_TYPE, USA_TANK_GUN_PRIMARY_RADIUS, is_crusader_template,
            is_legal_usa_tank_splash_target, is_paladin_template, usa_tank_gun_splash_damage_at,
            usa_tank_scatter_aim, usa_tank_scatter_misses_infantry,
        };

        let (source_team, damage, is_paladin) = {
            let Some(sid) = source else {
                return (0, false);
            };
            let Some(obj) = self.objects.get(&sid) else {
                return (0, false);
            };
            let is_c = is_crusader_template(&obj.template_name);
            let is_p = is_paladin_template(&obj.template_name);
            if !is_c && !is_p {
                return (0, false);
            }
            let dmg = obj
                .weapon
                .as_ref()
                .map(|w| w.damage)
                .unwrap_or(USA_TANK_GUN_DAMAGE);
            (obj.team, dmg, is_p)
        };

        // C++ Crusader/PaladinTankGun ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let mut intended_scatter_miss = false;
        if intended_is_infantry {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source.map(|s| s.0).unwrap_or(0),
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = usa_tank_scatter_aim(impact, true, seed);
            if scattered {
                self.usa_tank_scatter_applied = self.usa_tank_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if usa_tank_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > USA_TANK_GUN_PRIMARY_RADIUS {
                        self.usa_tank_scatter_misses =
                            self.usa_tank_scatter_misses.saturating_add(1);
                        intended_scatter_miss = true;
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Structure);
                if !is_legal_usa_tank_splash_target(
                    obj.is_alive(),
                    combat_kind,
                    obj.is_kind_of(KindOf::Projectile),
                    false,
                ) {
                    return None;
                }
                let p = obj.get_position();
                let dx = p.x - impact_xz.0;
                let dz = p.z - impact_xz.1;
                let dist = (dx * dx + dz * dz).sqrt();
                // Scatter miss residual: intended infantry outside splash is not force-hit.
                if Some(*id) == intended_target && intended_scatter_miss {
                    return None;
                }
                if dist > USA_TANK_GUN_PRIMARY_RADIUS && Some(*id) != intended_target {
                    return None;
                }
                Some((*id, dist))
            })
            .collect();

        for (id, dist) in candidates {
            let dmg = if Some(id) == intended_target {
                damage
            } else {
                usa_tank_gun_splash_damage_at(dist, damage)
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    USA_TANK_GUN_DAMAGE_TYPE,
                    USA_TANK_GUN_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }
        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }
        self.usa_tank_residual_units_hit = self.usa_tank_residual_units_hit.saturating_add(hits);
        let _audio = if is_paladin {
            PALADIN_FIRE_AUDIO
        } else {
            CRUSADER_FIRE_AUDIO
        };
        let _ = _audio;
        (hits, any_destroyed)
    }
}
