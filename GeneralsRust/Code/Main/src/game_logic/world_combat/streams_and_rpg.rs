//! Host combat `impl GameLogic` — `streams_and_rpg`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub fn honesty_technical_rpg_missile_projectile_ok(&self) -> bool {
        self.technical_rpg_missiles_spawned > 0
    }

    /// Spawn Technical cannon GenericTankShell Bezier residual.
    pub fn spawn_technical_cannon_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.spawn_technical_cannon_shell_projectile_with_effects(
            source_id,
            from,
            aim,
            intended,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_technical_cannon_shell_projectile_with_effects(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_technical::{
            TECH_CANNON_SHELL_MAX_HEALTH, TECH_CANNON_SHELL_PROJECTILE,
            technical_cannon_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if self.receiving_module_content.is_some() {
            if !self.ensure_host_spawn_template(TECH_CANNON_SHELL_PROJECTILE) {
                return None;
            }
        } else if !self.templates.contains_key(TECH_CANNON_SHELL_PROJECTILE) {
            if let Some(authored) =
                Self::build_template_from_asset_definition(TECH_CANNON_SHELL_PROJECTILE)
            {
                self.templates
                    .insert(TECH_CANNON_SHELL_PROJECTILE.to_string(), authored);
            } else {
                let mut t = ThingTemplate::new(TECH_CANNON_SHELL_PROJECTILE);
                t.add_kind_of(KindOf::Projectile)
                    .set_health(TECH_CANNON_SHELL_MAX_HEALTH)
                    .set_cost(0, 0);
                t.body_initial_health = Some(TECH_CANNON_SHELL_MAX_HEALTH);
                self.templates
                    .insert(TECH_CANNON_SHELL_PROJECTILE.to_string(), t);
            }
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on TechnicalCannonWeapon vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_technical::technical_cannon_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.technical_cannon_scatter_applied =
                self.technical_cannon_scatter_applied.saturating_add(1);
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
            if crate::game_logic::host_technical::technical_cannon_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_technical::TECH_CANNON_RADIUS {
                        self.technical_cannon_scatter_misses =
                            self.technical_cannon_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(2.0);
        let pid = self.create_object_with_script_effects(
            TECH_CANNON_SHELL_PROJECTILE,
            team,
            start,
            None,
            presentation_effects,
        )?;
        let flight = technical_cannon_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.technical_cannon_shell_projectile = true;
            o.technical_cannon_shell_from = Some([start.x, start.y, start.z]);
            o.technical_cannon_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.technical_cannon_shell_launch_frame = Some(self.frame);
            o.technical_cannon_shell_flight_frames = flight;
            o.technical_cannon_shell_intended = intended.map(|id| id.0);
            o.note_producer(source_id);
        }
        self.technical_cannon_shells_spawned =
            self.technical_cannon_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_technical_cannon_shell_projectiles(&mut self) {
        self.update_technical_cannon_shell_projectiles_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_technical_cannon_shell_projectiles_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_technical::technical_cannon_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if o.technical_cannon_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, from, aim, launch, flight) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .technical_cannon_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .technical_cannon_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.technical_cannon_shell_intended.map(ObjectId),
                    from,
                    aim,
                    o.technical_cannon_shell_launch_frame.unwrap_or(frame),
                    o.technical_cannon_shell_flight_frames.max(1),
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / flight as f32).clamp(0.0, 1.0);
            let pos = technical_cannon_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                o.set_position_with_effects(
                    pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
            }
            if elapsed >= flight {
                impact.push((id, source, intended, aim));
            }
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                {
                    o.internal_change_health(-o.health.current);
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.technical_cannon_shell_projectile = false;
                o.set_position_with_effects(
                    pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
            }
            let _ = self.apply_technical_residual_at_with_effects(
                pos,
                source,
                intended,
                presentation_effects,
            );
            self.mark_object_for_destruction_with_effects(id, team, presentation_effects);
        }
    }

    pub fn honesty_technical_cannon_shell_projectile_ok(&self) -> bool {
        self.technical_cannon_shells_spawned > 0
    }

    pub fn honesty_toxin_stream_projectile_ok(&self) -> bool {
        self.toxin_stream_missiles_spawned > 0
    }

    pub fn honesty_dragon_flame_projectile_ok(&self) -> bool {
        self.dragon_flame_missiles_spawned > 0
    }

    pub fn honesty_humvee_tow_missile_projectile_ok(&self) -> bool {
        self.humvee_tow_missiles_spawned > 0 || self.humvee_tow_scatter_applied > 0
    }

    /// Residual honesty: Humvee ground TOW ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_humvee_tow_scatter_ok(&self) -> bool {
        self.humvee_tow_scatter_applied > 0 || self.humvee_tow_scatter_misses > 0
    }

    pub fn humvee_tow_residual_fires(&self) -> u32 {
        self.humvee_tow_residual_fires
    }

    /// Apply Humvee TOW dual path splash at impact (ground 30/r5 or air 50/r5).
    pub fn apply_humvee_tow_residual_at(
        &mut self,
        impact: glam::Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        air: bool,
    ) -> (u32, bool) {
        self.apply_humvee_tow_residual_at_with_effects(
            impact,
            source,
            intended_target,
            air,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_humvee_tow_residual_at_with_effects(
        &mut self,
        impact: glam::Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        air: bool,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_humvee::{
            HUMVEE_AIR_TOW_RADIUS, HUMVEE_GROUND_TOW_RADIUS, HUMVEE_TOW_DAMAGE_TYPE,
            HUMVEE_TOW_DEATH_TYPE, HUMVEE_TOW_FIRE_AUDIO, humvee_air_tow_damage_at,
            humvee_ground_tow_damage_at,
        };

        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);
        let radius = if air {
            HUMVEE_AIR_TOW_RADIUS
        } else {
            HUMVEE_GROUND_TOW_RADIUS
        };
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let victims: Vec<(ObjectId, f32)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                if source.map(|s| s == *id).unwrap_or(false) {
                    return None;
                }
                // Projectiles / debris skip.
                let d = (obj.get_position() - impact).length();
                if d > radius + 0.001 {
                    return None;
                }
                // Friendly fire residual: RadiusDamageAffects ALLIES ENEMIES NEUTRALS — allow all teams.
                let _ = source_team;
                let _ = intended_target;
                Some((*id, d))
            })
            .collect();
        for (vid, dist) in victims {
            let dmg = if air {
                humvee_air_tow_damage_at(dist)
            } else {
                humvee_ground_tow_damage_at(dist)
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(v) = self.objects.get_mut(&vid) {
                let receiving_damage_id = v.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        HUMVEE_TOW_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        HUMVEE_TOW_DEATH_TYPE,
                    ),
                    None,
                    presentation_effects,
                );
                let v = self
                    .objects
                    .get_mut(&receiving_damage_id)
                    .expect("bounded residual victim remains installed");
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    let team = v.team;
                    self.mark_object_for_destruction_with_effects(
                        vid,
                        Some(team),
                        presentation_effects,
                    );
                }
            }
        }
        let _ = HUMVEE_TOW_FIRE_AUDIO;
        (hits, any_destroyed)
    }

    pub fn honesty_flashbang_grenade_projectile_ok(&self) -> bool {
        self.flashbang_grenades_spawned > 0
    }

    pub fn apply_ranger_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        flashbang_slot: bool,
    ) -> (u32, bool) {
        self.apply_ranger_residual_at_with_effects(
            impact,
            source,
            intended_target,
            flashbang_slot,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_ranger_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        flashbang_slot: bool,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_ranger::{
            FLASHBANG_DAMAGE_TYPE, FLASHBANG_DEATH_TYPE, FLASHBANG_SECONDARY_RADIUS,
            RANGER_FLASHBANG_FIRE_AUDIO, RANGER_RIFLE_DAMAGE, RANGER_RIFLE_DAMAGE_TYPE,
            RANGER_RIFLE_DEATH_TYPE, RANGER_RIFLE_FIRE_AUDIO, flashbang_damage_at,
            is_legal_ranger_target, ranger_flashbang_scatter_aim, ranger_flashbang_scatter_misses,
        };

        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        let mut hits = 0u32;
        let mut any_destroyed = false;

        // C++ RangerFlashBangGrenadeWeapon ScatterRadius residual on instant apply.
        let mut impact = impact;
        let mut intended_scatter_miss = false;
        if flashbang_slot {
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
            let (new_impact, scattered) = ranger_flashbang_scatter_aim(impact, seed);
            if scattered {
                self.flashbang_scatter_applied = self.flashbang_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if intended_target.is_some() && ranger_flashbang_scatter_misses(seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > FLASHBANG_SECONDARY_RADIUS {
                        self.flashbang_scatter_misses =
                            self.flashbang_scatter_misses.saturating_add(1);
                        intended_scatter_miss = true;
                    }
                }
            }
        }

        if flashbang_slot {
            // Dual-radius flashbang residual (Primary 35/10 + Secondary 10/40).
            let victims: Vec<(ObjectId, f32, bool)> = self
                .objects
                .registered()
                .filter_map(|(id, obj)| {
                    if obj.team == source_team {
                        return None;
                    }
                    let combat_kind = obj.is_kind_of(KindOf::Attackable)
                        || obj.is_kind_of(KindOf::Structure)
                        || obj.is_kind_of(KindOf::Infantry)
                        || obj.is_kind_of(KindOf::Vehicle)
                        || obj.is_kind_of(KindOf::Aircraft);
                    if !is_legal_ranger_target(
                        obj.is_alive(),
                        source == Some(*id),
                        obj.status.under_construction,
                        combat_kind,
                    ) {
                        return None;
                    }
                    let pos = obj.get_position();
                    let dx = pos.x - impact.x;
                    let dz = pos.z - impact.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    let is_intended = intended_target == Some(*id);
                    // Scatter miss residual: intended outside secondary is not force-hit.
                    if is_intended && intended_scatter_miss {
                        return None;
                    }
                    // Splash ring only (no force-hit primary when impact scattered away).
                    if dist > FLASHBANG_SECONDARY_RADIUS {
                        return None;
                    }
                    let dmg = flashbang_damage_at(false, dist);
                    if dmg <= 0.0 {
                        return None;
                    }
                    Some((*id, dmg, is_intended))
                })
                .collect();

            for (vid, dmg, _intended) in victims {
                if let Some(obj) = self.objects.get_mut(&vid) {
                    let receiving_damage_target_id = obj.id;
                    let destroyed = self.apply_residual_damage_owned_with_effects(
                        receiving_damage_target_id,
                        dmg,
                        source,
                        crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                            FLASHBANG_DAMAGE_TYPE,
                        ),
                        crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                            FLASHBANG_DEATH_TYPE,
                        ),
                        None,
                        presentation_effects,
                    );
                    hits = hits.saturating_add(1);
                    if destroyed {
                        any_destroyed = true;
                        self.mark_object_for_destruction_with_effects(
                            vid,
                            Some(source_team),
                            presentation_effects,
                        );
                    }
                }
            }

            self.ranger_residual_flashbang_fires =
                self.ranger_residual_flashbang_fires.saturating_add(1);
            self.ranger_residual_units_hit = self.ranger_residual_units_hit.saturating_add(hits);

            self.queue_audio_event(
                AudioEventRequest::new(RANGER_FLASHBANG_FIRE_AUDIO)
                    .with_position(impact)
                    .with_priority(155),
            );
        } else {
            // Rifle residual: intended-only PrimaryDamage 5.
            let damage = source
                .and_then(|sid| self.objects.get(&sid))
                .and_then(|o| o.weapon.as_ref().map(|w| w.damage))
                .unwrap_or(RANGER_RIFLE_DAMAGE);

            let Some(target_id) = intended_target else {
                return (0, false);
            };
            let Some(target) = self.objects.get(&target_id) else {
                return (0, false);
            };
            let combat_kind = target.is_kind_of(KindOf::Attackable)
                || target.is_kind_of(KindOf::Structure)
                || target.is_kind_of(KindOf::Infantry)
                || target.is_kind_of(KindOf::Vehicle)
                || target.is_kind_of(KindOf::Aircraft);
            if !is_legal_ranger_target(
                target.is_alive(),
                source == Some(target_id),
                target.status.under_construction,
                combat_kind,
            ) {
                return (0, false);
            }
            let target_pos = target.get_position();

            if let Some(obj) = self.objects.get_mut(&target_id) {
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    damage,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        RANGER_RIFLE_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        RANGER_RIFLE_DEATH_TYPE,
                    ),
                    None,
                    presentation_effects,
                );
                hits = 1;
                if destroyed {
                    any_destroyed = true;
                    self.mark_object_for_destruction_with_effects(
                        target_id,
                        Some(source_team),
                        presentation_effects,
                    );
                }
            }

            self.ranger_residual_rifle_fires = self.ranger_residual_rifle_fires.saturating_add(1);
            self.ranger_residual_units_hit = self.ranger_residual_units_hit.saturating_add(hits);

            self.queue_audio_event(
                AudioEventRequest::new(RANGER_RIFLE_FIRE_AUDIO)
                    .with_position(target_pos)
                    .with_priority(150),
            );
        }

        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    /// Apply China MiniGunner residual fire: intended-only ground/AA residual.
    pub(in super::super) fn apply_minigunner_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
    ) -> (u32, bool) {
        self.apply_minigunner_residual_at_with_effects(
            impact,
            source,
            intended_target,
            slot,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super) fn apply_minigunner_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_minigunner::{
            MINIGUNNER_AA_FIRE_AUDIO, MINIGUNNER_AIR_DAMAGE, MINIGUNNER_AIR_DAMAGE_TYPE,
            MINIGUNNER_AIR_DEATH_TYPE, MINIGUNNER_FIRE_AUDIO, MINIGUNNER_GROUND_DAMAGE,
            MINIGUNNER_GROUND_DAMAGE_TYPE, MINIGUNNER_GROUND_DEATH_TYPE, has_chain_guns_upgrade,
            is_legal_minigunner_target, minigunner_damage_with_chain_guns,
        };

        let dmg = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| {
                let chain = has_chain_guns_upgrade(&o.applied_upgrades);
                // Prefer live weapon damage (already chain/horde-refreshed).
                if slot == 1 {
                    o.secondary_weapon
                        .as_ref()
                        .map(|w| w.damage)
                        .unwrap_or_else(|| {
                            minigunner_damage_with_chain_guns(MINIGUNNER_AIR_DAMAGE, chain)
                        })
                } else {
                    o.weapon.as_ref().map(|w| w.damage).unwrap_or_else(|| {
                        minigunner_damage_with_chain_guns(MINIGUNNER_GROUND_DAMAGE, chain)
                    })
                }
            })
            .unwrap_or(MINIGUNNER_GROUND_DAMAGE);

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let Some(tid) = intended_target else {
            return (0, false);
        };
        if let Some(obj) = self.objects.get_mut(&tid) {
            let combat_kind = obj.is_kind_of(KindOf::Attackable)
                || obj.is_kind_of(KindOf::Structure)
                || obj.is_kind_of(KindOf::Infantry)
                || obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Aircraft);
            if is_legal_minigunner_target(
                obj.is_alive(),
                source == Some(tid),
                obj.status.under_construction,
                combat_kind,
            ) {
                let (dt_name, death_name) = if slot == 1 {
                    (MINIGUNNER_AIR_DAMAGE_TYPE, MINIGUNNER_AIR_DEATH_TYPE)
                } else {
                    (MINIGUNNER_GROUND_DAMAGE_TYPE, MINIGUNNER_GROUND_DEATH_TYPE)
                };
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        dt_name,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        death_name,
                    ),
                    None,
                    presentation_effects,
                );
                hits = 1;
                if destroyed {
                    any_destroyed = true;
                    self.mark_object_for_destruction_with_effects(
                        tid,
                        source_team,
                        presentation_effects,
                    );
                }
            }
        }

        if slot == 1 {
            self.minigunner_residual_aa_fires = self.minigunner_residual_aa_fires.saturating_add(1);
        } else {
            self.minigunner_residual_ground_fires =
                self.minigunner_residual_ground_fires.saturating_add(1);
        }

        let audio = if slot == 1 {
            MINIGUNNER_AA_FIRE_AUDIO
        } else {
            MINIGUNNER_FIRE_AUDIO
        };
        self.queue_audio_event(
            AudioEventRequest::new(audio)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    /// Advance MiniGunner continuous-fire ramp residual after a successful shot.
    pub(in super::super) fn advance_minigunner_continuous_fire(
        &mut self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        slot: u8,
    ) {
        use crate::game_logic::host_gattling_tank::GattlingFireLevel;
        use crate::game_logic::host_minigunner::{
            MINIGUNNER_RAPID_FIRE_AUDIO, has_chain_guns_upgrade, minigunner_air_weapon,
            minigunner_coast_until_after_shot, minigunner_ground_weapon, minigunner_on_shot_fired,
        };

        let frame = self.frame;
        let morale = self.owner_morale_upgrades(
            self.objects
                .get(&attacker_id)
                .and_then(|obj| obj.owner_player_id),
        );
        let Some(obj) = self.objects.get_mut(&attacker_id) else {
            return;
        };
        let prev_level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        let prev_consec = obj.consecutive_shots_at_target;
        let prev_victim = obj.consecutive_shot_target.map(|id| id.0);
        let new_victim = target_id.map(|id| id.0);
        let coast_until = obj.continuous_fire_coast_until_frame;
        let in_horde = obj.weapon_bonus_horde;
        // C++ evaluateMoraleBonus: nationalism from upgrade; AllowedNationalism
        // vetoes only while in horde (default TRUE).
        let nationalism = super::tanks_and_upgrades::nationalism_bonus_from_upgrade(
            morale.nationalism,
            in_horde,
            super::tanks_and_upgrades::HORDE_DEFAULT_ALLOWED_NATIONALISM,
        );
        obj.weapon_bonus_nationalism = nationalism;
        let chain = has_chain_guns_upgrade(&obj.applied_upgrades);

        let (new_level, consecutive, entered_fast) = minigunner_on_shot_fired(
            prev_level,
            prev_consec,
            prev_victim,
            new_victim,
            frame,
            coast_until,
        );

        obj.continuous_fire_level = new_level.as_u8();
        obj.consecutive_shots_at_target = consecutive;
        // C++ coolDown resets the victim together with the count
        // (FiringTracker.cpp:319-321); consecutive == 0 marks that reset.
        obj.consecutive_shot_target = if consecutive == 0 { None } else { target_id };
        // C++ m_frameToStartCooldown = getPossibleNextShotFrame() + coast
        // (FiringTracker.cpp:106-110): the next-shot frame was stamped with
        // the FIRED shot's (pre-promotion) delay (Weapon.cpp:2645-2647;
        // speedUp runs after firing), so the deadline uses prev_level.
        obj.continuous_fire_coast_until_frame =
            minigunner_coast_until_after_shot(frame, prev_level, in_horde, nationalism);

        // Rebind weapons with ramped + horde reload residual.
        if let Some(w) = obj.weapon.as_mut() {
            let refreshed = minigunner_ground_weapon(new_level, chain, in_horde, nationalism);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = false;
            w.can_target_ground = true;
        }
        if let Some(w) = obj.secondary_weapon.as_mut() {
            let refreshed = minigunner_air_weapon(new_level, chain, in_horde, nationalism);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = true;
            w.can_target_ground = false;
        }

        if new_level == GattlingFireLevel::Mean && prev_level != GattlingFireLevel::Mean {
            self.minigunner_residual_ramp_mean =
                self.minigunner_residual_ramp_mean.saturating_add(1);
        }
        let became_fast = entered_fast
            || (new_level == GattlingFireLevel::Fast && prev_level != GattlingFireLevel::Fast);
        if became_fast {
            self.minigunner_residual_ramp_fast =
                self.minigunner_residual_ramp_fast.saturating_add(1);
            // C++ FiringTracker::speedUp MEAN→FAST: getPerUnitSound("VoiceRapidFire") + setObjectID.
            self.queue_resolved_per_unit_sound(
                attacker_id,
                MINIGUNNER_RAPID_FIRE_AUDIO,
                true,
                false,
                None,
                140,
            );
        }
        let _ = slot; // slot honesty counted in apply_minigunner_residual_at
    }

    /// Refresh RPG Trooper residual from current AP Rockets upgrade tag.
    pub(in super::super) fn refresh_rpg_trooper_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_rpg_trooper::{
            has_ap_rockets_upgrade, is_rpg_trooper_weapon_name, rpg_trooper_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !obj
            .authored_weapon_name_for_slot(0)
            .is_some_and(is_rpg_trooper_weapon_name)
        {
            return;
        }
        let ap = has_ap_rockets_upgrade(&obj.applied_upgrades);
        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut w = rpg_trooper_weapon(ap);
        w.last_fire_time = last_fire;
        // C++ AP Rockets changes this weapon through WeaponBonusUpgrade, which
        // retains the concrete Weapon and its barrel cursor.
        obj.weapon = Some(w);
    }

    /// Apply AP Rockets residual tag to an RPG Trooper (damage × 1.25).
    pub fn apply_rpg_trooper_ap_rockets_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_rpg_trooper::{
            UPGRADE_GLA_AP_ROCKETS, is_rpg_trooper_weapon_name,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !obj
            .authored_weapon_name_for_slot(0)
            .is_some_and(is_rpg_trooper_weapon_name)
        {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_AP_ROCKETS.to_string());
        self.rpg_trooper_residual_ap_upgrades =
            self.rpg_trooper_residual_ap_upgrades.saturating_add(1);
        self.refresh_rpg_trooper_weapon(object_id);
        true
    }

    pub fn honesty_rpg_trooper_missile_projectile_ok(&self) -> bool {
        self.rpg_trooper_missiles_spawned > 0
    }

    pub fn apply_rpg_trooper_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_rpg_trooper_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_rpg_trooper_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_rpg_trooper::{
            RPG_TROOPER_DAMAGE, RPG_TROOPER_DAMAGE_TYPE, RPG_TROOPER_DEATH_TYPE,
            RPG_TROOPER_FIRE_AUDIO, RPG_TROOPER_SPLASH_RADIUS, is_legal_rpg_trooper_splash_target,
            rpg_trooper_scatter_aim, rpg_trooper_scatter_misses_infantry,
            rpg_trooper_splash_damage_at,
        };

        let damage = source
            .and_then(|sid| self.objects.get(&sid))
            .and_then(|o| o.weapon.as_ref().map(|w| w.damage))
            .unwrap_or(RPG_TROOPER_DAMAGE);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ TunnelDefenderRocketWeapon ScatterRadiusVsInfantry residual on instant apply.
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
            let (new_impact, scattered) = rpg_trooper_scatter_aim(impact, true, seed);
            if scattered {
                self.rpg_trooper_scatter_applied =
                    self.rpg_trooper_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if rpg_trooper_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > RPG_TROOPER_SPLASH_RADIUS {
                        self.rpg_trooper_scatter_misses =
                            self.rpg_trooper_scatter_misses.saturating_add(1);
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
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_rpg_trooper_splash_target(
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
                if is_intended && intended_is_infantry && dist > RPG_TROOPER_SPLASH_RADIUS {
                    return None;
                }
                if is_intended || dist <= RPG_TROOPER_SPLASH_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = rpg_trooper_splash_damage_at(is_intended, dist, damage);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        RPG_TROOPER_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        RPG_TROOPER_DEATH_TYPE,
                    ),
                    None,
                    presentation_effects,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.rpg_trooper_residual_fires = self.rpg_trooper_residual_fires.saturating_add(1);
        self.rpg_trooper_residual_units_hit =
            self.rpg_trooper_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(RPG_TROOPER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    pub fn honesty_missile_defender_missile_projectile_ok(&self) -> bool {
        self.missile_defender_missiles_spawned > 0
    }

    pub fn apply_missile_defender_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        laser_slot: bool,
    ) -> (u32, bool) {
        self.apply_missile_defender_residual_at_with_effects(
            impact,
            source,
            intended_target,
            laser_slot,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_missile_defender_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        laser_slot: bool,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_missile_defender::{
            MISSILE_DEFENDER_DAMAGE, MISSILE_DEFENDER_DEATH_TYPE, MISSILE_DEFENDER_FIRE_AUDIO,
            MISSILE_DEFENDER_LASER_DAMAGE_TYPE, MISSILE_DEFENDER_PRIMARY_DAMAGE_TYPE,
            MISSILE_DEFENDER_SPLASH_RADIUS, is_legal_missile_defender_splash_target,
            missile_defender_scatter_aim, missile_defender_scatter_misses_infantry,
            missile_defender_splash_damage_at,
        };

        let damage = source
            .and_then(|sid| self.objects.get(&sid))
            .and_then(|o| {
                if laser_slot {
                    o.secondary_weapon
                        .as_ref()
                        .or(o.weapon.as_ref())
                        .map(|w| w.damage)
                } else {
                    o.weapon.as_ref().map(|w| w.damage)
                }
            })
            .unwrap_or(MISSILE_DEFENDER_DAMAGE);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ MissileDefenderMissileWeapon ScatterRadiusVsInfantry residual (primary only).
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        if intended_is_infantry && !laser_slot {
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
            let (new_impact, scattered) = missile_defender_scatter_aim(impact, true, seed);
            if scattered {
                self.missile_defender_scatter_applied =
                    self.missile_defender_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if missile_defender_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > MISSILE_DEFENDER_SPLASH_RADIUS {
                        self.missile_defender_scatter_misses =
                            self.missile_defender_scatter_misses.saturating_add(1);
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
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_missile_defender_splash_target(
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
                if is_intended
                    && intended_is_infantry
                    && !laser_slot
                    && dist > MISSILE_DEFENDER_SPLASH_RADIUS
                {
                    return None;
                }
                if is_intended || dist <= MISSILE_DEFENDER_SPLASH_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = missile_defender_splash_damage_at(is_intended, dist, damage);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let dt_name = if laser_slot {
                    MISSILE_DEFENDER_LASER_DAMAGE_TYPE
                } else {
                    MISSILE_DEFENDER_PRIMARY_DAMAGE_TYPE
                };
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        dt_name,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        MISSILE_DEFENDER_DEATH_TYPE,
                    ),
                    None,
                    presentation_effects,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.missile_defender_residual_fires =
            self.missile_defender_residual_fires.saturating_add(1);
        self.missile_defender_residual_units_hit = self
            .missile_defender_residual_units_hit
            .saturating_add(hits);
        if laser_slot {
            self.missile_defender_residual_laser_fires =
                self.missile_defender_residual_laser_fires.saturating_add(1);
        }

        self.queue_audio_event(
            AudioEventRequest::new(MISSILE_DEFENDER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    /// Activate Missile Defender laser guided special residual: lock secondary + attack.
    ///
    /// Fail-closed: not full PreparationTime / PersistentPrepTime / LaserBeam object matrix.
    /// SpecialPower ReloadTime = 0 residual (always ready).
    /// Residual last TNT plant frame for a Tank Hunter (ReloadTime gate).
    pub fn tank_hunter_tnt_last_plant_frame(&self, object_id: ObjectId) -> Option<u32> {
        self.tank_hunter_tnt_last_frame.get(&object_id).copied()
    }
}
