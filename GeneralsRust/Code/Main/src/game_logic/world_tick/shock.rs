//! Host tick `impl GameLogic` — `shock`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    pub(crate) fn tick_physics_collisions_all(&mut self) -> u32 {
        // Per-frame blocked bookkeeping residual (before new collide pairs).
        // Snapshot ground heights before mut pass (terrain borrow).
        // Only mobile / physics-active bodies need terrain samples + motion step —
        // sampling every structure on Lone Eagle (~900 objs) dominated host frames.
        let ground_heights: Vec<(ObjectId, f32)> = {
            let mut out = Vec::new();
            for (id, o) in self.objects.iter() {
                if o.status.destroyed || !o.is_alive() {
                    continue;
                }
                // Structures/immobile skip full physics motion residual.
                if !(o.can_move()
                    || o.shock_stun_frames > 0
                    || o.bounce_audio_pending > 0
                    || o.movement.velocity.length_squared() > 1e-6)
                {
                    continue;
                }
                let p = o.get_position();
                let g = self
                    .terrain_height_at(glam::Vec3::new(p.x, 0.0, p.z))
                    .unwrap_or(0.0);
                out.push((*id, g));
            }
            out
        };
        for o in self.objects.values_mut() {
            if o.status.destroyed || !o.is_alive() {
                continue;
            }
            o.clear_blocked_frame_state();
            o.tick_move_away_state();
            o.tick_path_queue();
            // C++ PhysicsBehavior update residual order: friction → integrate accel → motion.
            if o.can_move() {
                o.apply_frictional_forces();
            }
            // Immobile structures still clear accel residual cheaply.
            o.integrate_physics_accel();
        }
        for (id, ground_y) in ground_heights {
            if let Some(o) = self.objects.get_mut(&id) {
                let _ = o.tick_physics_motion_step(ground_y);
            }
        }
        // Rebuild partition cells (C++ registerObject residual each update).
        // Keep FOW reveal residual; only re-register live objects.
        self.partition_manager.clear_registered_objects();
        // O(1) id → pose/radius for pair resolution (was linear find per neighbor).
        let mut entry_by_id: std::collections::HashMap<u32, (glam::Vec3, f32)> =
            std::collections::HashMap::new();
        let mut mobile_ids: Vec<ObjectId> = Vec::new();
        for (id, o) in self.objects.iter() {
            if !o.is_alive() || o.status.destroyed {
                continue;
            }
            let pos = o.get_position();
            let r = o.selection_radius.max(1.0);
            self.partition_manager
                .register_object_footprint(id.0, pos.x, pos.z, r);
            entry_by_id.insert(id.0, (pos, r));
            // Only mobile bodies initiate collide queries (structures stay as
            // partition obstacles via neighbor lookup).
            if o.can_move() {
                mobile_ids.push(*id);
            }
        }
        mobile_ids.sort_by_key(|id| id.0);

        let mut handled = 0u32;
        let mut seen_pairs: std::collections::HashSet<(u32, u32)> =
            std::collections::HashSet::new();
        for a_id in &mobile_ids {
            let Some((a_pos, a_r)) = entry_by_id.get(&a_id.0).copied() else {
                continue;
            };
            let neighbors = self.partition_manager.neighbor_object_ids(a_pos.x, a_pos.z);
            for b_raw in neighbors {
                if b_raw == a_id.0 {
                    continue;
                }
                let lo = a_id.0.min(b_raw);
                let hi = a_id.0.max(b_raw);
                if !seen_pairs.insert((lo, hi)) {
                    continue;
                }
                let Some((b_pos, b_r)) = entry_by_id.get(&b_raw).copied() else {
                    continue;
                };
                let dx = a_pos.x - b_pos.x;
                let dz = a_pos.z - b_pos.z;
                let sum = a_r + b_r;
                if dx * dx + dz * dz > sum * sum {
                    continue;
                }
                let b_id = ObjectId(b_raw);
                if let (Some(a), Some(b)) = (self.objects.get(a_id), self.objects.get(&b_id)) {
                    if let Some((loc, normal)) = super::collide_dispatch::host_geom_collides(a, b) {
                        super::collide_dispatch::dispatch_collide_modules(*a_id, b_id, loc, normal);
                        self.dispatch_host_collide_modules(*a_id, b_id);
                    }
                }
                if self.try_physics_collide(*a_id, b_id, a_r) {
                    handled = handled.saturating_add(1);
                } else if self.try_physics_collide(b_id, *a_id, b_r) {
                    handled = handled.saturating_add(1);
                }
            }
        }
        // C++ previousOverlap = currentOverlap end of physics update residual.
        for id in mobile_ids {
            if let Some(o) = self.objects.get_mut(&id) {
                o.advance_physics_overlap_frame();
            }
        }
        // Stun/bounce residual on non-mobile still advances overlap bookkeeping.
        let extra: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && !o.status.destroyed
                    && !o.can_move()
                    && (o.shock_stun_frames > 0 || o.bounce_audio_pending > 0)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in extra {
            if let Some(o) = self.objects.get_mut(&id) {
                o.advance_physics_overlap_frame();
            }
        }
        handled
    }

    pub(crate) fn tick_shock_stun_all(&mut self) {
        // Include bounce_audio_pending so land audio drains even after stun ends.
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.shock_stun_frames > 0 || o.bounce_audio_pending > 0)
            .map(|(id, _)| *id)
            .collect();
        let mut bounce_audio: Vec<(ObjectId, String, glam::Vec3, f32)> = Vec::new();
        for id in ids {
            let pos = self
                .objects
                .get(&id)
                .map(|o| o.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            let (cliff, water) = self.sample_stun_surface_at(pos);
            if let Some(o) = self.objects.get_mut(&id) {
                o.cell_is_cliff = cliff;
                o.cell_is_underwater = water;
                if o.shock_stun_frames > 0 {
                    // Wave 764: under coupled shadow, GW sole-decrements frames;
                    // host keeps tumble/bounce physics only.
                    if crate::gameworld_shadow::gameworld_shadow_enabled()
                        && crate::gameworld_shadow::shadow_coupled_tick_active()
                    {
                        o.tick_shock_stun_physics_only();
                    } else {
                        o.tick_shock_stun();
                    }
                }
                while let Some((name, vol)) = o.take_bounce_audio_pending() {
                    let p = o.get_position();
                    bounce_audio.push((id, name, p, vol));
                }
            }
        }
        // C++ TheAudio->addAudioEvent bounce residual.
        for (id, name, pos, vol) in bounce_audio {
            let pri = (64.0 + vol * 136.0).clamp(0.0, 255.0) as u8;
            self.queue_audio_event(
                AudioEventRequest::new(&name)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(pri),
            );
        }
    }

    pub(crate) fn apply_shock_wave_at_impact(
        &mut self,
        impact: glam::Vec3,
        weapon_name: Option<&str>,
        skip_id: Option<ObjectId>,
    ) -> u32 {
        use crate::game_logic::weapon_bootstrap::{
            compute_shock_wave_force, host_shock_wave_amount_for_weapon_name,
            host_shock_wave_radius_for_weapon_name, host_shock_wave_taper_for_weapon_name,
        };
        let Some(name) = weapon_name else {
            return 0;
        };
        let amount = host_shock_wave_amount_for_weapon_name(name);
        let radius = host_shock_wave_radius_for_weapon_name(name);
        let taper = host_shock_wave_taper_for_weapon_name(name);
        if amount <= 0.0 || radius <= 0.0 {
            return 0;
        }
        let r2 = radius * radius;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if skip_id == Some(*id) {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                let p = obj.get_position();
                let dx = p.x - impact.x;
                let dz = p.z - impact.z;
                if dx * dx + dz * dz > r2 {
                    return None;
                }
                Some(*id)
            })
            .collect();
        let mut n = 0u32;
        for id in ids {
            let pos = match self.objects.get(&id) {
                Some(o) => o.get_position(),
                None => continue,
            };
            let Some(force) = compute_shock_wave_force(impact, pos, amount, radius, taper) else {
                continue;
            };
            if let Some(o) = self.objects.get_mut(&id) {
                if o.apply_shock_wave_impulse(force) {
                    n = n.saturating_add(1);
                }
            }
        }
        n
    }

    pub(crate) fn apply_instant_hit_splash_at(
        &mut self,
        impact: glam::Vec3,
        primary_damage: f32,
        secondary_damage: f32,
        primary_radius: f32,
        secondary_radius: f32,
        attacker_id: ObjectId,
        attacker_team: Team,
        intended_id: ObjectId,
        weapon_name: Option<&str>,
    ) -> u32 {
        let max_r = primary_radius.max(secondary_radius);
        if max_r <= 0.0 || (primary_damage <= 0.0 && secondary_damage <= 0.0) {
            return 0;
        }
        use crate::game_logic::host_ai_path_combat_residual_wave105::{
            WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS,
        };
        use crate::game_logic::weapon_bootstrap::{
            host_radius_damage_affects_for_weapon_name, radius_damage_affects_victim,
        };
        let affects = weapon_name
            .map(host_radius_damage_affects_for_weapon_name)
            .unwrap_or(WEAPON_AFFECTS_ENEMIES | WEAPON_AFFECTS_NEUTRALS);
        let shooter_template = self
            .objects
            .get(&attacker_id)
            .map(|a| a.template_name.clone())
            .unwrap_or_default();
        let primary_sq = primary_radius * primary_radius;
        let secondary_sq = max_r * max_r;
        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if *id == intended_id {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                if obj.is_eject_invulnerable() {
                    return None;
                }
                let airborne = obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target;
                let same_tmpl = !shooter_template.is_empty()
                    && obj.template_name.eq_ignore_ascii_case(&shooter_template);
                if !radius_damage_affects_victim(
                    affects,
                    attacker_team,
                    attacker_id,
                    *id,
                    obj.team,
                    airborne,
                    same_tmpl,
                ) {
                    return None;
                }
                let p = obj.get_position();
                let dx = p.x - impact.x;
                let dz = p.z - impact.z;
                let d2 = dx * dx + dz * dz;
                if d2 > secondary_sq {
                    return None;
                }
                Some((*id, d2))
            })
            .collect();
        let mut hits = 0u32;
        let mut destroy: Vec<ObjectId> = Vec::new();
        for (id, d2) in candidates {
            let dmg = if primary_radius > 0.0 && d2 <= primary_sq {
                primary_damage
            } else if secondary_damage > 0.0 {
                secondary_damage
            } else {
                0.0
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let damage_type = weapon_name
                    .map(crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name)
                    .unwrap_or(crate::game_logic::combat::DamageType::Bullet);
                let death_type = crate::game_logic::host_armor_residual::resolve_host_death_type(
                    weapon_name,
                    damage_type,
                );
                let dead = obj.take_damage_from_typed_death(
                    dmg,
                    Some(attacker_id),
                    damage_type,
                    death_type,
                );
                hits = hits.saturating_add(1);
                if dead {
                    destroy.push(id);
                }
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, Some(attacker_team));
        }
        let _ = self.apply_shock_wave_at_impact(impact, weapon_name, None);
        hits
    }

    pub(crate) fn apply_scatter_miss_splash_at(
        &mut self,
        impact: glam::Vec3,
        weapon_damage: f32,
        splash_radius: f32,
        attacker_id: ObjectId,
        attacker_team: Team,
        skip_id: ObjectId,
        weapon_name: Option<&str>,
    ) -> u32 {
        if weapon_damage <= 0.0 || splash_radius <= 0.0 {
            return 0;
        }
        use crate::game_logic::weapon_bootstrap::{
            host_radius_damage_affects_for_weapon_name, radius_damage_affects_victim,
        };
        // Default residual when name unknown: ENEMIES|NEUTRALS.
        let affects = weapon_name
            .map(host_radius_damage_affects_for_weapon_name)
            .unwrap_or_else(|| {
                use crate::game_logic::host_ai_path_combat_residual_wave105::{
                    WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS,
                };
                WEAPON_AFFECTS_ENEMIES | WEAPON_AFFECTS_NEUTRALS
            });
        let shooter_template = self
            .objects
            .get(&attacker_id)
            .map(|a| a.template_name.clone())
            .unwrap_or_default();
        let primary_r = splash_radius;
        let secondary_r = splash_radius * 1.5; // outer residual taper ring
        let primary_sq = primary_r * primary_r;
        let secondary_sq = secondary_r * secondary_r;
        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if *id == skip_id {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                if obj.is_eject_invulnerable() {
                    return None;
                }
                let airborne = obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target;
                let same_tmpl = !shooter_template.is_empty()
                    && obj.template_name.eq_ignore_ascii_case(&shooter_template);
                if !radius_damage_affects_victim(
                    affects,
                    attacker_team,
                    attacker_id,
                    *id,
                    obj.team,
                    airborne,
                    same_tmpl,
                ) {
                    return None;
                }
                let p = obj.get_position();
                let dx = p.x - impact.x;
                let dz = p.z - impact.z;
                let d2 = dx * dx + dz * dz;
                if d2 > secondary_sq {
                    return None;
                }
                Some((*id, d2.sqrt()))
            })
            .collect();
        let mut hits = 0u32;
        let mut destroy: Vec<ObjectId> = Vec::new();
        for (id, dist) in candidates {
            let dmg = if dist * dist <= primary_sq {
                weapon_damage
            } else {
                weapon_damage * 0.5
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let damage_type = weapon_name
                    .map(crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name)
                    .unwrap_or(crate::game_logic::combat::DamageType::Bullet);
                let death_type = crate::game_logic::host_armor_residual::resolve_host_death_type(
                    weapon_name,
                    damage_type,
                );
                let dead = obj.take_damage_from_typed_death(
                    dmg,
                    Some(attacker_id),
                    damage_type,
                    death_type,
                );
                hits = hits.saturating_add(1);
                if dead {
                    destroy.push(id);
                }
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, Some(attacker_team));
        }
        let _ = self.apply_shock_wave_at_impact(impact, weapon_name, Some(skip_id));
        hits
    }

    pub(crate) fn instant_scatter_misses_shot(
        &self,
        attacker_id: ObjectId,
        target_id: ObjectId,
        slot: u8,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_effective_scatter_radius, scatter_misses_intended_target, scatter_seed_for_shot,
            DEFAULT_SCATTER_HIT_RADIUS,
        };
        let (wname, tgt_inf, hit_r) = {
            let attacker = match self.objects.get(&attacker_id) {
                Some(a) => a,
                None => return false,
            };
            let target = match self.objects.get(&target_id) {
                Some(t) => t,
                None => return false,
            };
            let wname = if slot == 1 {
                attacker
                    .thing
                    .template
                    .secondary_weapon_name
                    .as_deref()
                    .or(attacker.thing.template.primary_weapon_name.as_deref())
            } else {
                attacker.thing.template.primary_weapon_name.as_deref()
            };
            let hit_r = if target.selection_radius > 0.0 {
                target.selection_radius
            } else {
                DEFAULT_SCATTER_HIT_RADIUS
            };
            (
                wname.map(|s| s.to_string()),
                target.is_kind_of(KindOf::Infantry),
                hit_r,
            )
        };
        let Some(name) = wname else {
            return false;
        };
        let scatter = host_effective_scatter_radius(&name, tgt_inf);
        if scatter <= 0.0 {
            return false;
        }
        let seed = scatter_seed_for_shot(attacker_id.0, target_id.0, self.frame);
        scatter_misses_intended_target(scatter, seed, hit_r)
    }

    pub(crate) fn try_min_range_backup(
        &mut self,
        attacker_id: ObjectId,
        target_pos: glam::Vec3,
        min_range: f32,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            compute_min_range_backup_pos, is_inside_minimum_attack_range,
        };
        let Some(attacker) = self.objects.get(&attacker_id) else {
            return false;
        };
        if !attacker.can_move() || !attacker.is_alive() {
            return false;
        }
        let src = attacker.get_position();
        let dx = src.x - target_pos.x;
        let dz = src.z - target_pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if !is_inside_minimum_attack_range(dist, min_range) {
            return false;
        }
        let dest = compute_min_range_backup_pos(src, target_pos, min_range);
        // Direct backup residual (fail-closed vs full reverse-pathfind matrix).
        if let Some(a) = self.objects.get_mut(&attacker_id) {
            a.movement.path.clear();
            a.movement.current_path_index = 0;
            a.record_host_movement();
            a.movement.target_position = Some(dest);
            a.set_ai_state(AIState::Attacking);
            a.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(attacker_id, 2);
                // Attacking
            }
            a.set_status_moving(true);
            crate::game_logic::host_move_log::record(attacker_id, Some([dest.x, dest.y, dest.z]));
            return true;
        }
        false
    }

    pub(crate) fn approach_pos_for_attack(
        &self,
        attacker_id: ObjectId,
        target_pos: glam::Vec3,
        weapon_range: f32,
        weapon_name: Option<&str>,
    ) -> glam::Vec3 {
        let contact = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_is_contact_weapon_name)
            .unwrap_or(false)
            || crate::game_logic::weapon_bootstrap::is_contact_weapon_range(weapon_range);
        if contact {
            return target_pos;
        }
        let src = self
            .objects
            .get(&attacker_id)
            .map(|o| o.get_position())
            .unwrap_or(target_pos);
        crate::game_logic::weapon_bootstrap::compute_approach_target_pos(
            src,
            target_pos,
            weapon_range,
        )
    }

    pub(crate) fn try_continue_attack_after_kill(
        &mut self,
        attacker_id: ObjectId,
        dead_victim_id: ObjectId,
        original_victim_pos: glam::Vec3,
        continue_range: f32,
        victim_team: Team,
    ) -> bool {
        if continue_range <= 0.0 {
            return false;
        }
        // Pure residual acquire: nearest same-team victim near kill position (XZ).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if id == attacker_id || id == dead_victim_id || !obj.is_alive() {
                    return None;
                }
                if obj.team != victim_team {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: obj.team == Team::Neutral,
                        under_construction: obj.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: obj.is_effectively_stealthed(),
                        is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                        eject_invulnerable: obj.is_eject_invulnerable(),
                    },
                )
            })
            .collect();
        let Some((next_id, _, _)) =
            crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                Some(attacker_id),
                (original_victim_pos.x, original_victim_pos.z),
                candidates,
                continue_range,
                |_| true,
            )
        else {
            return false;
        };
        // Continue-attack residual: under AI decision authority, log AttackTarget
        // (+ Attacking state) for GameWorld apply/writeback; host stays clean.
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(attacker_id, next_id);
            crate::game_logic::host_ai_decision_log::record_set_state(attacker_id, 2); // Attacking
            return true;
        }
        if let Some(attacker) = self.objects.get_mut(&attacker_id) {
            attacker.target = Some(next_id);
            attacker.set_ai_state(AIState::Attacking);
            attacker.set_status_attacking(true);
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub fn try_continue_attack_after_kill_for_test(
        &mut self,
        attacker_id: ObjectId,
        dead_victim_id: ObjectId,
        original_victim_pos: glam::Vec3,
        continue_range: f32,
        victim_team: Team,
    ) -> bool {
        self.try_continue_attack_after_kill(
            attacker_id,
            dead_victim_id,
            original_victim_pos,
            continue_range,
            victim_team,
        )
    }

    /// After a combat kill: grant XP, ContinueAttackRange retarget, else stop.
    pub(crate) fn continue_or_stop_after_kill(
        &mut self,
        attacker_id: ObjectId,
        dead_victim_id: ObjectId,
        original_victim_pos: glam::Vec3,
        victim_team: Team,
        weapon_name: Option<&str>,
        kill_xp: f32,
    ) {
        if let Some(attacker) = self.objects.get_mut(&attacker_id) {
            if kill_xp > 0.0 {
                attacker.gain_experience(kill_xp);
            }
        }
        let cont = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_continue_attack_range_for_weapon_name)
            .unwrap_or(0.0);
        if self.try_continue_attack_after_kill(
            attacker_id,
            dead_victim_id,
            original_victim_pos,
            cont,
            victim_team,
        ) {
            return;
        }
        self.stop_attack_decision_aware(attacker_id);
    }
}
