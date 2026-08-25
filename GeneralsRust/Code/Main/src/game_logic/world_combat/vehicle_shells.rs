//! Host combat `impl GameLogic` — `vehicle_shells`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub fn update_scorpion_missile_projectiles(&mut self) {
        use crate::game_logic::host_scorpion::{
            SCORPION_MISSILE_INITIAL_VELOCITY, SCORPION_MISSILE_PROJECTILE_SPEED,
            SCORPION_MISSILE_TURN_DISTANCE,
        };
        let frame = self.frame;
        let launch = SCORPION_MISSILE_INITIAL_VELOCITY / 30.0;
        let cruise = SCORPION_MISSILE_PROJECTILE_SPEED / 30.0;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.scorpion_missile_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3, u8)> =
            Vec::new();
        for id in flying {
            let (source, intended, aim, pos, travelled, fuel_done, slot) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .scorpion_missile_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let intended = o.scorpion_missile_intended.map(ObjectId);
                let fuel_done = o
                    .scorpion_missile_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                (
                    o.producer_id,
                    intended,
                    aim,
                    o.get_position(),
                    o.scorpion_missile_travelled,
                    fuel_done,
                    o.scorpion_missile_slot,
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
            let speed = if travelled < SCORPION_MISSILE_TURN_DISTANCE {
                launch
            } else {
                cruise
            };
            let to_aim = aim - pos;
            let vel = if to_aim.length() > 0.001 {
                to_aim.normalize() * speed
            } else {
                glam::Vec3::new(0.0, -speed, 0.0)
            };
            let step = vel.length().max(speed);
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                o.set_position(pos + vel);
                o.scorpion_missile_travelled += step;
                o.scorpion_missile_aim = Some([aim.x, aim.y, aim.z]);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            let new_pos = pos + vel;
            let near = (aim - new_pos).length() < 6.0;
            if fuel_done || near {
                impact.push((id, source, intended, aim, slot));
            }
        }
        for (id, source, intended, pos, slot) in impact {
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
                o.scorpion_missile_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_scorpion_residual_at(pos, source, intended, slot);
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_scorpion_shell_projectile_ok(&self) -> bool {
        self.scorpion_shells_spawned > 0
    }

    pub fn honesty_scorpion_missile_projectile_ok(&self) -> bool {
        self.scorpion_missiles_spawned > 0
    }

    pub fn apply_scorpion_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
    ) -> (u32, bool) {
        use crate::game_logic::host_scorpion::{
            SCORPION_GUN_DAMAGE_TYPE, SCORPION_GUN_DEATH_TYPE, SCORPION_GUN_FIRE_AUDIO,
            SCORPION_GUN_SPLASH_RADIUS, SCORPION_MISSILE_DAMAGE_TYPE, SCORPION_MISSILE_DEATH_TYPE,
            SCORPION_MISSILE_FIRE_AUDIO, SCORPION_MISSILE_SECONDARY_RADIUS, has_ap_rockets_upgrade,
            is_legal_scorpion_splash_target, is_scorpion_template, salvage_tier_from_upgrades,
            scorpion_gun_splash_damage_at, scorpion_missile_damage_at, scorpion_scatter_aim,
            scorpion_scatter_misses_infantry,
        };

        let (source_team, gun_damage, has_ap, is_missile) = {
            let Some(sid) = source else {
                return (0, false);
            };
            let Some(obj) = self.objects.get(&sid) else {
                return (0, false);
            };
            if !is_scorpion_template(&obj.template_name) {
                return (0, false);
            }
            let tier = salvage_tier_from_upgrades(&obj.applied_upgrades);
            let gun_dmg = obj
                .weapon
                .as_ref()
                .map(|w| w.damage)
                .unwrap_or_else(|| tier.gun_damage());
            let ap = has_ap_rockets_upgrade(&obj.applied_upgrades);
            (obj.team, gun_dmg, ap, slot == 1)
        };

        // C++ ScorpionTankGun / ScorpionMissileWeapon ScatterRadiusVsInfantry residual.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let search_radius = if is_missile {
            SCORPION_MISSILE_SECONDARY_RADIUS
        } else {
            SCORPION_GUN_SPLASH_RADIUS
        };
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
            let (new_impact, scattered) = scorpion_scatter_aim(impact, true, seed);
            if scattered {
                self.scorpion_scatter_applied = self.scorpion_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if scorpion_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > search_radius {
                        self.scorpion_scatter_misses =
                            self.scorpion_scatter_misses.saturating_add(1);
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
                if !is_legal_scorpion_splash_target(
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
                if is_intended && intended_is_infantry && dist > search_radius {
                    return None;
                }
                if is_intended || dist <= search_radius {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = if is_missile {
                scorpion_missile_damage_at(if is_intended { 0.0 } else { dist }, has_ap)
            } else {
                scorpion_gun_splash_damage_at(is_intended, dist, gun_damage)
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let (dt_name, death_name) = if is_missile {
                    (SCORPION_MISSILE_DAMAGE_TYPE, SCORPION_MISSILE_DEATH_TYPE)
                } else {
                    (SCORPION_GUN_DAMAGE_TYPE, SCORPION_GUN_DEATH_TYPE)
                };
                let destroyed =
                    obj.take_damage_from_immediate_residual(dmg, source, dt_name, death_name);
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            if let Some(src) = source {
                self.award_score_the_kill_experience(src, id);
            }
            self.mark_object_for_destruction(id, killer);
        }

        self.scorpion_residual_fires = self.scorpion_residual_fires.saturating_add(1);
        self.scorpion_residual_units_hit = self.scorpion_residual_units_hit.saturating_add(hits);
        if is_missile {
            self.scorpion_residual_missile_fires =
                self.scorpion_residual_missile_fires.saturating_add(1);
        }

        let audio = if is_missile {
            SCORPION_MISSILE_FIRE_AUDIO
        } else {
            SCORPION_GUN_FIRE_AUDIO
        };
        self.queue_audio_event(
            AudioEventRequest::new(audio)
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

    /// Apply Tomahawk residual fire (dual-radius long-range missile).
    /// C++ TomahawkMissile ProjectileObject residual (MissileAI lob + impact splash).
    pub fn spawn_tomahawk_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_height_die::HostHeightDieData;
        use crate::game_logic::host_tomahawk::{
            TOMAHAWK_DISTANCE_BEFORE_TURNING, TOMAHAWK_FUEL_LIFETIME_FRAMES,
            TOMAHAWK_INITIAL_VELOCITY, TOMAHAWK_MISSILE_HEIGHT_DIE_TARGET,
            TOMAHAWK_MISSILE_MAX_HEALTH, TOMAHAWK_MISSILE_PROJECTILE, TOMAHAWK_PREFERRED_HEIGHT,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(TOMAHAWK_MISSILE_PROJECTILE) {
            let mut t = ThingTemplate::new(TOMAHAWK_MISSILE_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(TOMAHAWK_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(TOMAHAWK_MISSILE_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on TomahawkMissileWeapon vs infantry (**20**).
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
            crate::game_logic::host_tomahawk::tomahawk_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.tomahawk_scatter_applied = self.tomahawk_scatter_applied.saturating_add(1);
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
            if crate::game_logic::host_tomahawk::tomahawk_scatter_misses_infantry(true, seed, hit_r)
            {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_tomahawk::TOMAHAWK_SECONDARY_RADIUS {
                        self.tomahawk_scatter_misses =
                            self.tomahawk_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + TOMAHAWK_PREFERRED_HEIGHT * 0.2;
        let pid = self.create_object(TOMAHAWK_MISSILE_PROJECTILE, team, start)?;
        // Launch residual uses InitialVelocity; cruise accelerates toward projectile Speed 200.
        let launch_speed = TOMAHAWK_INITIAL_VELOCITY / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        let mut vel = dir * launch_speed;
        vel.y = vel.y.max(launch_speed * 0.75);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.tomahawk_missile_projectile = true;
            o.tomahawk_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.tomahawk_missile_travelled = 0.0;
            o.tomahawk_missile_fuel_expires_frame =
                Some(self.frame.saturating_add(TOMAHAWK_FUEL_LIFETIME_FRAMES));
            o.note_producer(source_id);
            o.health.maximum = TOMAHAWK_MISSILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, TOMAHAWK_MISSILE_MAX_HEALTH);
            o.movement.velocity = vel;
            o.set_orientation(dir.z.atan2(dir.x));
            o.height_die = Some(HostHeightDieData::with_target(
                TOMAHAWK_MISSILE_HEIGHT_DIE_TARGET,
                true,
                self.frame.saturating_add(2),
            ));
            o.ensure_height_die(self.frame);
        }
        let _ = TOMAHAWK_DISTANCE_BEFORE_TURNING;
        self.tomahawk_missiles_spawned = self.tomahawk_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_tomahawk_missile_projectiles(&mut self) {
        use crate::game_logic::host_tomahawk::{
            TOMAHAWK_DISTANCE_BEFORE_DIVING, TOMAHAWK_DISTANCE_BEFORE_TURNING,
            TOMAHAWK_INITIAL_VELOCITY, TOMAHAWK_PREFERRED_HEIGHT, TOMAHAWK_PROJECTILE_SPEED,
        };
        let frame = self.frame;
        let launch_speed = TOMAHAWK_INITIAL_VELOCITY / 30.0;
        let cruise_speed = TOMAHAWK_PROJECTILE_SPEED / 30.0;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.tomahawk_missile_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, glam::Vec3)> = Vec::new();
        for id in flying {
            let (source, aim, pos, travelled, fuel_done) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .tomahawk_missile_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let fuel_done = o
                    .tomahawk_missile_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                (
                    o.producer_id,
                    aim,
                    o.get_position(),
                    o.tomahawk_missile_travelled,
                    fuel_done,
                )
            };
            let to_aim = aim - pos;
            let horiz = glam::Vec3::new(to_aim.x, 0.0, to_aim.z).length();
            let speed = if travelled < TOMAHAWK_DISTANCE_BEFORE_TURNING {
                launch_speed
            } else {
                cruise_speed
            };
            let vel = if travelled < TOMAHAWK_DISTANCE_BEFORE_TURNING {
                let dir = if to_aim.length() > 0.001 {
                    to_aim.normalize()
                } else {
                    glam::Vec3::Y
                };
                let mut v = dir * speed;
                if pos.y < aim.y + TOMAHAWK_PREFERRED_HEIGHT {
                    v.y = speed * 0.9;
                }
                v
            } else if horiz > TOMAHAWK_DISTANCE_BEFORE_DIVING {
                let loft_aim =
                    glam::Vec3::new(aim.x, aim.y + TOMAHAWK_PREFERRED_HEIGHT * 0.55, aim.z);
                let d = loft_aim - pos;
                if d.length() > 0.001 {
                    d.normalize() * speed
                } else {
                    glam::Vec3::new(0.0, -speed, 0.0)
                }
            } else {
                // TryToFollowTarget terminal dive residual.
                let d = aim - pos;
                if d.length() > 0.001 {
                    d.normalize() * speed
                } else {
                    glam::Vec3::new(0.0, -speed, 0.0)
                }
            };
            let step = vel.length().max(speed);
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                let p = o.get_position();
                o.set_position(p + vel);
                o.tomahawk_missile_travelled += step;
                o.set_orientation(vel.z.atan2(vel.x));
            }
            let new_pos = pos + vel;
            let near = glam::Vec3::new(aim.x - new_pos.x, 0.0, aim.z - new_pos.z).length() < 10.0
                && new_pos.y <= aim.y + 15.0;
            if fuel_done || near {
                // TryToFollowTarget residual: warhead detonates on locked aim point.
                impact.push((id, source, aim));
            }
        }
        for (id, source, pos) in impact {
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
                o.tomahawk_missile_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_tomahawk_residual_at(pos, source, None);
            self.mark_object_for_destruction(id, team);
        }

        // HeightDie residual detonation.
        let height_die_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.tomahawk_missile_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in height_die_ids {
            let (source, pos, die, team) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let die = o.tick_height_die(frame, 0.0);
                (o.producer_id, o.get_position(), die, o.team)
            };
            if die {
                let aim = self
                    .objects
                    .get(&id)
                    .and_then(|o| o.tomahawk_missile_aim)
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(pos);
                if let Some(o) = self.objects.get_mut(&id) {
                    o.tomahawk_missile_projectile = false;
                }
                let _ = self.apply_tomahawk_residual_at(aim, source, None);
                self.mark_object_for_destruction(id, Some(team));
            }
        }
    }

    pub fn honesty_tomahawk_missile_projectile_ok(&self) -> bool {
        self.tomahawk_missiles_spawned > 0
    }

    pub fn apply_tomahawk_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_tomahawk::{
            TOMAHAWK_DAMAGE_TYPE, TOMAHAWK_DEATH_TYPE, TOMAHAWK_FIRE_AUDIO,
            TOMAHAWK_SECONDARY_RADIUS, is_legal_tomahawk_splash_target, is_tomahawk_template,
            tomahawk_damage_at, tomahawk_scatter_aim, tomahawk_scatter_misses_infantry,
        };

        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ TomahawkMissileWeapon ScatterRadiusVsInfantry residual on instant apply.
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
            let (new_impact, scattered) = tomahawk_scatter_aim(impact, true, seed);
            if scattered {
                self.tomahawk_scatter_applied = self.tomahawk_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if tomahawk_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > TOMAHAWK_SECONDARY_RADIUS {
                        self.tomahawk_scatter_misses =
                            self.tomahawk_scatter_misses.saturating_add(1);
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
                if !is_legal_tomahawk_splash_target(
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
                if is_intended && intended_is_infantry && dist > TOMAHAWK_SECONDARY_RADIUS {
                    return None;
                }
                if is_intended || dist <= TOMAHAWK_SECONDARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = tomahawk_damage_at(if is_intended { 0.0 } else { dist });
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    TOMAHAWK_DAMAGE_TYPE,
                    TOMAHAWK_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            if let Some(src) = source {
                self.award_score_the_kill_experience(src, id);
            }
            self.mark_object_for_destruction(id, killer);
        }

        self.tomahawk_residual_fires = self.tomahawk_residual_fires.saturating_add(1);
        self.tomahawk_residual_units_hit = self.tomahawk_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(TOMAHAWK_FIRE_AUDIO)
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
            let _ = is_tomahawk_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Laser Missiles residual tag + rebind Raptor jet missile damage.
    pub fn apply_raptor_laser_missiles_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_raptor::{
            UPGRADE_AMERICA_LASER_MISSILES, has_laser_missiles_upgrade, is_king_raptor_template,
            is_raptor_template, raptor_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_raptor_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_AMERICA_LASER_MISSILES.to_string());
        let king = is_king_raptor_template(&obj.template_name);
        let laser = has_laser_missiles_upgrade(&obj.applied_upgrades);
        let mut w = raptor_weapon(king, laser);
        // Preserve fire clock if already mid-combat.
        if let Some(prev) = obj.weapon.as_ref() {
            w.last_fire_time = prev.last_fire_time;
        }
        // C++ WeaponBonusUpgrade updates the existing Raptor weapon in place;
        // retain its current barrel cursor rather than treating this as a
        // WeaponSet replacement.
        obj.weapon = Some(w);
        self.raptor_residual_laser_missiles_upgrades = self
            .raptor_residual_laser_missiles_upgrades
            .saturating_add(1);
        true
    }

    /// Apply Raptor residual fire (jet missile + primary radius splash).
    /// C++ RaptorJetMissile ProjectileObject residual.
    pub fn spawn_raptor_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_raptor::{
            RAPTOR_MISSILE_FUEL_FRAMES, RAPTOR_MISSILE_IGNITION_DELAY_FRAMES,
            RAPTOR_MISSILE_INITIAL_VELOCITY, RAPTOR_MISSILE_MAX_HEALTH, RAPTOR_PROJECTILE,
            RAPTOR_PROJECTILE_SPEED,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(RAPTOR_PROJECTILE) {
            let mut t = ThingTemplate::new(RAPTOR_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(RAPTOR_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(RAPTOR_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on RaptorJetMissileWeapon vs infantry.
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
            crate::game_logic::host_raptor::raptor_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.raptor_scatter_applied = self.raptor_scatter_applied.saturating_add(1);
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
            if crate::game_logic::host_raptor::raptor_scatter_misses_infantry(true, seed, hit_r) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_raptor::RAPTOR_PRIMARY_RADIUS {
                        self.raptor_scatter_misses = self.raptor_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        // Air launch residual: start slightly below attacker altitude toward aim.
        start.y = start.y.max(aim.y + 20.0);
        let pid = self.create_object(RAPTOR_PROJECTILE, team, start)?;
        let launch = RAPTOR_MISSILE_INITIAL_VELOCITY / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.raptor_missile_projectile = true;
            o.raptor_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.raptor_missile_intended = intended.map(|id| id.0);
            o.raptor_missile_travelled = 0.0;
            o.raptor_missile_fuel_expires_frame =
                Some(self.frame.saturating_add(RAPTOR_MISSILE_FUEL_FRAMES));
            o.raptor_missile_ignition_frame = Some(
                self.frame
                    .saturating_add(RAPTOR_MISSILE_IGNITION_DELAY_FRAMES),
            );
            o.note_producer(source_id);
            o.health.maximum = RAPTOR_MISSILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, RAPTOR_MISSILE_MAX_HEALTH);
            o.movement.velocity = dir * launch;
            o.set_orientation(dir.z.atan2(dir.x));
        }
        let _ = RAPTOR_PROJECTILE_SPEED;
        self.raptor_missiles_spawned = self.raptor_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_raptor_missile_projectiles(&mut self) {
        use crate::game_logic::host_raptor::{
            RAPTOR_MISSILE_INITIAL_VELOCITY, RAPTOR_PROJECTILE_SPEED,
        };
        let frame = self.frame;
        let launch = RAPTOR_MISSILE_INITIAL_VELOCITY / 30.0;
        let cruise = RAPTOR_PROJECTILE_SPEED / 30.0;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.raptor_missile_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, aim, pos, fuel_done, ignited) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .raptor_missile_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let intended = o.raptor_missile_intended.map(ObjectId);
                let fuel_done = o
                    .raptor_missile_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                let ignited = o
                    .raptor_missile_ignition_frame
                    .map(|f| f <= frame)
                    .unwrap_or(true);
                (
                    o.producer_id,
                    intended,
                    aim,
                    o.get_position(),
                    fuel_done,
                    ignited,
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
            // Pre-ignition: coast at InitialVelocity; post-ignition: cruise WeaponSpeed.
            let speed = if ignited { cruise } else { launch };
            let to_aim = aim - pos;
            let dist = to_aim.length();
            // Clamp step so high WeaponSpeed cruise cannot skip past the aim.
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
                o.raptor_missile_travelled += step;
                o.raptor_missile_aim = Some([aim.x, aim.y, aim.z]);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            let near = dist <= speed + 0.001 || (aim - new_pos).length() < 8.0;
            if fuel_done || near {
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
                o.raptor_missile_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_raptor_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_raptor_missile_projectile_ok(&self) -> bool {
        self.raptor_missiles_spawned > 0
    }

    pub fn apply_raptor_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_raptor::{
            RAPTOR_DAMAGE_TYPE, RAPTOR_DEATH_TYPE, RAPTOR_FIRE_AUDIO, RAPTOR_PRIMARY_RADIUS,
            has_laser_missiles_upgrade, is_king_raptor_template, is_legal_raptor_target,
            is_raptor_template, raptor_damage_at, raptor_scatter_aim,
            raptor_scatter_misses_infantry,
        };

        let (source_team, is_king, has_laser) = {
            if let Some(sid) = source {
                if let Some(obj) = self.objects.get(&sid) {
                    (
                        obj.team,
                        is_king_raptor_template(&obj.template_name),
                        has_laser_missiles_upgrade(&obj.applied_upgrades),
                    )
                } else {
                    (Team::Neutral, false, false)
                }
            } else {
                (Team::Neutral, false, false)
            }
        };

        // C++ RaptorJetMissileWeapon ScatterRadiusVsInfantry residual on instant apply.
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
            let (new_impact, scattered) = raptor_scatter_aim(impact, true, seed);
            if scattered {
                self.raptor_scatter_applied = self.raptor_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if raptor_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > RAPTOR_PRIMARY_RADIUS {
                        self.raptor_scatter_misses = self.raptor_scatter_misses.saturating_add(1);
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
                if !is_legal_raptor_target(
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
                if is_intended && intended_is_infantry && dist > RAPTOR_PRIMARY_RADIUS {
                    return None;
                }
                if is_intended || dist <= RAPTOR_PRIMARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = raptor_damage_at(if is_intended { 0.0 } else { dist }, is_king, has_laser);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    RAPTOR_DAMAGE_TYPE,
                    RAPTOR_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            if let Some(src) = source {
                self.award_score_the_kill_experience(src, id);
            }
            self.mark_object_for_destruction(id, killer);
        }

        self.raptor_residual_fires = self.raptor_residual_fires.saturating_add(1);
        self.raptor_residual_units_hit = self.raptor_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(RAPTOR_FIRE_AUDIO)
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
            let _ = is_raptor_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply BlackNapalm residual to a China MiG (PLAYER_UPGRADE fire-field residual).
    pub fn apply_mig_black_napalm_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_mig::{
            UPGRADE_CHINA_BLACK_NAPALM, is_mig_template, is_nuke_mig_template, mig_loadout,
            mig_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_mig_template(&obj.template_name) || is_nuke_mig_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_BLACK_NAPALM.to_string());
        let loadout = mig_loadout(false, &obj.applied_upgrades);
        let mut w = mig_weapon(loadout);
        if let Some(prev) = obj.weapon.as_ref() {
            w.last_fire_time = prev.last_fire_time;
        }
        let _ = obj.replace_weapon_set_slot(0, Some(w));
        self.mig_residual_black_napalm_upgrades =
            self.mig_residual_black_napalm_upgrades.saturating_add(1);
        true
    }

    /// Apply Tactical Nuke MiG residual to a Nuke General MiG.
    pub fn apply_mig_tactical_nuke_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_mig::{
            UPGRADE_CHINA_TACTICAL_NUKE_MIG, is_nuke_mig_template, mig_loadout, mig_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_nuke_mig_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_TACTICAL_NUKE_MIG.to_string());
        let loadout = mig_loadout(true, &obj.applied_upgrades);
        let mut w = mig_weapon(loadout);
        if let Some(prev) = obj.weapon.as_ref() {
            w.last_fire_time = prev.last_fire_time;
        }
        let _ = obj.replace_weapon_set_slot(0, Some(w));
        self.mig_residual_tactical_nuke_upgrades =
            self.mig_residual_tactical_nuke_upgrades.saturating_add(1);
        true
    }

    /// Apply China MiG residual fire (dual-radius missile + fire/radiation field).
    /// C++ NapalmMissile ProjectileObject residual (MiG).
    pub fn spawn_mig_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mig::{
            MIG_MISSILE_FUEL_FRAMES, MIG_MISSILE_IGNITION_DELAY_FRAMES,
            MIG_MISSILE_INITIAL_VELOCITY, MIG_MISSILE_MAX_HEALTH, MIG_PROJECTILE,
            MIG_PROJECTILE_SPEED,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(MIG_PROJECTILE) {
            let mut t = ThingTemplate::new(MIG_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(MIG_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(MIG_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on NapalmMissileWeapon vs infantry (**10**).
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
            crate::game_logic::host_mig::mig_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.mig_scatter_applied = self.mig_scatter_applied.saturating_add(1);
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
            if crate::game_logic::host_mig::mig_scatter_misses_infantry(true, seed, hit_r) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_mig::MIG_PRIMARY_RADIUS {
                        self.mig_scatter_misses = self.mig_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y + 20.0);
        let pid = self.create_object(MIG_PROJECTILE, team, start)?;
        let launch = MIG_MISSILE_INITIAL_VELOCITY / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.mig_missile_projectile = true;
            o.mig_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.mig_missile_intended = intended.map(|id| id.0);
            o.mig_missile_travelled = 0.0;
            o.mig_missile_fuel_expires_frame =
                Some(self.frame.saturating_add(MIG_MISSILE_FUEL_FRAMES));
            o.mig_missile_ignition_frame =
                Some(self.frame.saturating_add(MIG_MISSILE_IGNITION_DELAY_FRAMES));
            o.note_producer(source_id);
            o.health.maximum = MIG_MISSILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, MIG_MISSILE_MAX_HEALTH);
            o.movement.velocity = dir * launch;
            o.set_orientation(dir.z.atan2(dir.x));
        }
        let _ = MIG_PROJECTILE_SPEED;
        self.mig_missiles_spawned = self.mig_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_mig_missile_projectiles(&mut self) {
        use crate::game_logic::host_mig::{MIG_MISSILE_INITIAL_VELOCITY, MIG_PROJECTILE_SPEED};
        let frame = self.frame;
        let launch = MIG_MISSILE_INITIAL_VELOCITY / 30.0;
        let cruise = MIG_PROJECTILE_SPEED / 30.0;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.mig_missile_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, aim, pos, fuel_done, ignited) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .mig_missile_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let intended = o.mig_missile_intended.map(ObjectId);
                let fuel_done = o
                    .mig_missile_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                let ignited = o
                    .mig_missile_ignition_frame
                    .map(|f| f <= frame)
                    .unwrap_or(true);
                (
                    o.producer_id,
                    intended,
                    aim,
                    o.get_position(),
                    fuel_done,
                    ignited,
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
                o.mig_missile_travelled += step;
                o.mig_missile_aim = Some([aim.x, aim.y, aim.z]);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            let near = dist <= speed + 0.001 || (aim - new_pos).length() < 8.0;
            if fuel_done || near {
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
                o.mig_missile_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_mig_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_mig_missile_projectile_ok(&self) -> bool {
        self.mig_missiles_spawned > 0
    }

    pub fn apply_mig_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_mig::{
            MIG_BLACK_DAMAGE_TYPE, MIG_BLACK_DEATH_TYPE, MIG_DAMAGE_TYPE, MIG_DEATH_TYPE,
            MIG_FIRE_AUDIO, MIG_PRIMARY_RADIUS, MigLoadout, is_legal_mig_target, is_mig_template,
            is_nuke_mig_template, mig_damage_at, mig_fire_field_upgraded, mig_loadout,
            mig_scatter_aim, mig_scatter_misses_infantry, mig_secondary_radius,
            mig_spawns_fire_field, mig_spawns_radiation,
        };

        let (source_team, loadout) = {
            if let Some(sid) = source {
                if let Some(obj) = self.objects.get(&sid) {
                    (
                        obj.team,
                        mig_loadout(
                            is_nuke_mig_template(&obj.template_name),
                            &obj.applied_upgrades,
                        ),
                    )
                } else {
                    (Team::Neutral, MigLoadout::Standard)
                }
            } else {
                (Team::Neutral, MigLoadout::Standard)
            }
        };

        // C++ NapalmMissileWeapon ScatterRadiusVsInfantry residual on instant apply.
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
            let (new_impact, scattered) = mig_scatter_aim(impact, true, seed);
            if scattered {
                self.mig_scatter_applied = self.mig_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if mig_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    // Outside primary: not force-hit. Secondary splash may still apply by dist.
                    if dist > MIG_PRIMARY_RADIUS {
                        self.mig_scatter_misses = self.mig_scatter_misses.saturating_add(1);
                        intended_scatter_miss = true;
                    }
                }
            }
        }

        let max_radius = mig_secondary_radius(loadout);
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
                if !is_legal_mig_target(
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
                // (secondary splash may still apply via distance).
                if is_intended && intended_scatter_miss {
                    if dist > max_radius {
                        return None;
                    }
                    // Keep as splash-only (not force primary).
                    return Some((*id, dist, false));
                }
                if is_intended || dist <= max_radius {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = mig_damage_at(if is_intended { 0.0 } else { dist }, loadout);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let (dt_name, death_name) = match loadout {
                    MigLoadout::BlackNapalm => (MIG_BLACK_DAMAGE_TYPE, MIG_BLACK_DEATH_TYPE),
                    MigLoadout::NukeBase | MigLoadout::NukeTactical => {
                        (MIG_BLACK_DAMAGE_TYPE, MIG_BLACK_DEATH_TYPE)
                    }
                    MigLoadout::Standard => (MIG_DAMAGE_TYPE, MIG_DEATH_TYPE),
                };
                let destroyed =
                    obj.take_damage_from_immediate_residual(dmg, source, dt_name, death_name);
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            if let Some(src) = source {
                self.award_score_the_kill_experience(src, id);
            }
            self.mark_object_for_destruction(id, killer);
        }

        // Residual field residual at impact (FireField or SmallRadiation).
        if let Some(sid) = source {
            if mig_spawns_fire_field(loadout) {
                let upgraded = mig_fire_field_upgraded(loadout);
                let _ = self.spawn_inferno_fire_zone(sid, source_team, impact, upgraded);
                self.mig_residual_fire_fields = self.mig_residual_fire_fields.saturating_add(1);
            } else if mig_spawns_radiation(loadout) {
                let _ =
                    self.nuclear_tanks
                        .spawn_radiation_zone(sid, source_team, impact, self.frame);
                self.mig_residual_radiation_fields =
                    self.mig_residual_radiation_fields.saturating_add(1);
            }
        }

        self.mig_residual_fires = self.mig_residual_fires.saturating_add(1);
        self.mig_residual_units_hit = self.mig_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(MIG_FIRE_AUDIO)
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
            let _ = is_mig_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply America Fire Base residual fire (howitzer primary-radius splash).
    /// C++ Fire Base GenericTankShell ScaleWeaponSpeed lob residual.
    pub fn spawn_fire_base_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_fire_base::{
            FIRE_BASE_PROJECTILE, FIRE_BASE_SHELL_MAX_HEALTH, fire_base_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(FIRE_BASE_PROJECTILE) {
            let mut t = ThingTemplate::new(FIRE_BASE_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(FIRE_BASE_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(FIRE_BASE_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on FireBaseHowitzerGun vs infantry.
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
            crate::game_logic::host_fire_base::fire_base_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.fire_base_scatter_applied = self.fire_base_scatter_applied.saturating_add(1);
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
            if crate::game_logic::host_fire_base::fire_base_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_fire_base::FIRE_BASE_PRIMARY_RADIUS {
                        self.fire_base_scatter_misses =
                            self.fire_base_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 6.0;
        let pid = self.create_object(FIRE_BASE_PROJECTILE, team, start)?;
        let frames = fire_base_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.fire_base_shell_projectile = true;
            o.fire_base_shell_from = Some([start.x, start.y, start.z]);
            o.fire_base_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.fire_base_shell_launch_frame = Some(self.frame);
            o.fire_base_shell_flight_frames = frames;
            o.fire_base_shell_intended = intended.map(|id| id.0);
            o.note_producer(source_id);
            o.health.maximum = FIRE_BASE_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, FIRE_BASE_SHELL_MAX_HEALTH);
        }
        self.fire_base_shells_spawned = self.fire_base_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_fire_base_shell_projectiles(&mut self) {
        use crate::game_logic::host_fire_base::fire_base_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.fire_base_shell_projectile && o.is_alive() {
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
                    .fire_base_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .fire_base_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.fire_base_shell_intended.map(ObjectId),
                    from,
                    aim,
                    o.fire_base_shell_launch_frame.unwrap_or(frame),
                    o.fire_base_shell_flight_frames.max(1),
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = fire_base_shell_bezier_point(from, aim, t);
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
                o.fire_base_shell_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_fire_base_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
        }
    }

    /// Residual honesty: Fire Base ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_fire_base_scatter_ok(&self) -> bool {
        self.fire_base_scatter_applied > 0 || self.fire_base_scatter_misses > 0
    }

    pub fn honesty_fire_base_shell_projectile_ok(&self) -> bool {
        self.fire_base_shells_spawned > 0
    }

    pub fn apply_fire_base_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_fire_base::{
            FIRE_BASE_DAMAGE_TYPE, FIRE_BASE_DEATH_TYPE, FIRE_BASE_FIRE_AUDIO,
            FIRE_BASE_PRIMARY_RADIUS, fire_base_damage_at, fire_base_scatter_aim,
            fire_base_scatter_misses_infantry, is_fire_base_template, is_legal_fire_base_target,
        };

        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ FireBaseHowitzerGun ScatterRadiusVsInfantry residual on instant apply path.
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
            let (new_impact, scattered) = fire_base_scatter_aim(impact, true, seed);
            if scattered {
                self.fire_base_scatter_applied = self.fire_base_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if fire_base_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > FIRE_BASE_PRIMARY_RADIUS {
                        self.fire_base_scatter_misses =
                            self.fire_base_scatter_misses.saturating_add(1);
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
                if !is_legal_fire_base_target(
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
                if is_intended && intended_is_infantry && dist > FIRE_BASE_PRIMARY_RADIUS {
                    return None;
                }
                if is_intended || dist <= FIRE_BASE_PRIMARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = fire_base_damage_at(if is_intended { 0.0 } else { dist });
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    FIRE_BASE_DAMAGE_TYPE,
                    FIRE_BASE_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            if let Some(src) = source {
                self.award_score_the_kill_experience(src, id);
            }
            self.mark_object_for_destruction(id, killer);
        }

        self.fire_base_residual_fires = self.fire_base_residual_fires.saturating_add(1);
        self.fire_base_residual_units_hit = self.fire_base_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(FIRE_BASE_FIRE_AUDIO)
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
            let _ = is_fire_base_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }
}
