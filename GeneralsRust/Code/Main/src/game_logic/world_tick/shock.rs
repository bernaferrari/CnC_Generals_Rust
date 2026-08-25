//! Host tick `impl GameLogic` — `shock`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    pub(crate) fn tick_physics_collisions_all(&mut self) -> u32 {
        self.sync_all_contained_items_mass();
        // Per-frame blocked bookkeeping residual (before new collide pairs).
        // Snapshot ground heights before mut pass (terrain borrow).
        // Only mobile / physics-active bodies need terrain samples + motion step —
        // sampling every structure on Lone Eagle (~900 objs) dominated host frames.
        let ground_heights: Vec<(ObjectId, f32)> = {
            let mut out = Vec::new();
            for (id, o) in self.objects.iter() {
                let wreck = o.status.destroyed || !o.is_alive();
                let wreck_airborne = wreck && o.get_position().y > o.ground_height + 0.05;
                let physics_active = o.can_move()
                    || o.shock_stun_frames > 0
                    || o.bounce_audio_pending > 0
                    || o.pending_ground_collide
                    || o.movement.velocity.length_squared() > 1e-6
                    || o.allow_to_fall
                    || o.is_physics_held()
                    || wreck_airborne;
                if !physics_active {
                    continue;
                }
                if o.is_kind_of(KindOf::Structure) && !o.allow_to_fall && !wreck_airborne {
                    continue;
                }
                let p = o.get_position();
                let terrain_y = self
                    .terrain_height_at(glam::Vec3::new(p.x, 0.0, p.z))
                    .unwrap_or(0.0);
                out.push((*id, self.physics_ground_y_with_deck(o, terrain_y)));
            }
            out
        };
        for o in self.objects.values_mut() {
            o.clear_blocked_frame_state();
            o.tick_move_away_state();
            o.tick_path_queue();
            // C++ DISABLED_HELD skips gravity/friction/Euler; accel still zeros.
            if o.is_physics_held() {
                o.physics_accel = glam::Vec3::ZERO;
                continue;
            }
            // C++ PhysicsBehavior update residual order: friction → integrate accel → motion.
            // applyFrictionalForces every non-HELD update (debris, disabled, wrecks).
            o.apply_frictional_forces();
            o.integrate_physics_accel();
        }
        let mut landed: Vec<ObjectId> = Vec::new();
        for (id, ground_y) in ground_heights {
            if let Some(o) = self.objects.get_mut(&id) {
                let _ = o.tick_physics_motion_step(ground_y);
                if o.pending_ground_collide {
                    landed.push(id);
                }
            }
        }
        for id in landed {
            self.dispatch_physics_ground_collide(id);
        }
        // Rebuild partition cells (C++ registerObject residual each update).
        // Dead wrecks stay registered and still onCollide (C++ PartitionManager).
        self.partition_manager.clear_registered_objects();
        // C++ OpenContain::addOrRemoveObjFromWorld unRegisterObject when
        // isEnclosingContainerFor (Humvee/Chinook/garrison/Troop Crawler).
        // Fire Base / parachute / Overlord portable stay registered.
        let enclosing_hidden = enclosing_hidden_rider_ids(&self.objects);
        // O(1) id → pose/radius for pair resolution (was linear find per neighbor).
        let mut entry_by_id: std::collections::HashMap<u32, (glam::Vec3, f32)> =
            std::collections::HashMap::new();
        let mut mobile_ids: Vec<ObjectId> = Vec::new();
        for (id, o) in self.objects.iter() {
            // C++ unRegisterObject while ride-hidden (hijacker mesh + collide).
            if o.drawable_hidden || o.hijacker_in_vehicle {
                continue;
            }
            if enclosing_hidden.contains(&id.0) {
                continue;
            }
            let pos = o.get_position();
            let fp = super::collide_dispatch::host_object_footprint(o);
            self.partition_manager
                .register_object_geometry(id.0, pos.x, pos.z, fp);
            let r = o.physics_on_collide_radius();
            entry_by_id.insert(id.0, (pos, r));
            // Mobile bodies and physics-active wrecks initiate collide queries.
            // Dead hulks stay registered as obstacles; only moving/falling ones query.
            let wreck_physics = (o.status.destroyed || !o.is_alive())
                && (o.movement.velocity.length_squared() > 1e-6
                    || o.get_position().y > o.ground_height
                    || o.allow_to_fall
                    || o.shock_stun_frames > 0);
            if o.can_move() || wreck_physics {
                mobile_ids.push(*id);
            }
        }
        mobile_ids.sort_by_key(|id| id.0);

        let mut handled = 0u32;
        let mut seen_pairs: std::collections::HashSet<(u32, u32)> =
            std::collections::HashSet::new();
        for a_id in &mobile_ids {
            let Some((_a_pos, a_r)) = entry_by_id.get(&a_id.0).copied() else {
                continue;
            };
            // C++ addPossibleCollisions: every other module in every COI cell.
            let neighbors = self.partition_manager.neighbor_object_ids_of(a_id.0);
            for b_raw in neighbors {
                if b_raw == a_id.0 {
                    continue;
                }
                let lo = a_id.0.min(b_raw);
                let hi = a_id.0.max(b_raw);
                if !seen_pairs.insert((lo, hi)) {
                    continue;
                }
                let Some((_b_pos, b_r)) = entry_by_id.get(&b_raw).copied() else {
                    continue;
                };
                let b_id = ObjectId(b_raw);
                let geom_hit =
                    if let (Some(a), Some(b)) = (self.objects.get(a_id), self.objects.get(&b_id)) {
                        super::collide_dispatch::host_geom_collides(a, b)
                    } else {
                        None
                    };
                // C++ processContactList only onCollide after geomCollidesWithGeom.
                // First-overlap 0-damage crush bookkeeping must not fire from
                // a shared partition cell without contact (hq-zsklj).
                if let Some((loc, normal)) = geom_hit {
                    // C++ processContactList skips OBJECT_STATUS_NO_COLLISIONS
                    // (PartitionManager.cpp:2466-2468).
                    let skip_status = self
                        .objects
                        .get(a_id)
                        .is_some_and(|o| o.status.no_collisions)
                        || self
                            .objects
                            .get(&b_id)
                            .is_some_and(|o| o.status.no_collisions);
                    if skip_status {
                        continue;
                    }
                    super::collide_dispatch::dispatch_collide_modules(*a_id, b_id, loc, normal);

                    // Both onCollide (hq-uerpy): higher-id crusher still crushes.
                    // try_physics_collide dispatches host collide modules once per side.
                    if self.try_physics_collide(*a_id, b_id, a_r) {
                        handled = handled.saturating_add(1);
                    }
                    let reverse_ok = self.objects.get(a_id).is_some_and(|o| !o.status.destroyed)
                        && self.objects.get(&b_id).is_some_and(|o| !o.status.destroyed);
                    if reverse_ok && self.try_physics_collide(b_id, *a_id, b_r) {
                        handled = handled.saturating_add(1);
                    }
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
            let terrain_y = self
                .terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z))
                .unwrap_or(0.0);
            let ground_y = self
                .objects
                .get(&id)
                .map(|o| self.physics_ground_y_with_deck(o, terrain_y))
                .unwrap_or(terrain_y);
            if let Some(o) = self.objects.get_mut(&id) {
                o.cell_is_cliff = cliff;
                o.cell_is_underwater = water;
                o.ground_height = ground_y;
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
        source_pos: glam::Vec3,
        search_radius: f32,
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
        if amount <= 0.0 || radius <= 0.0 || search_radius <= 0.0 {
            return 0;
        }
        let r2 = search_radius * search_radius;
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
            let Some(force) = compute_shock_wave_force(source_pos, pos, amount, radius, taper)
            else {
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
        use crate::game_logic::weapon_bootstrap::{
            WEAPON_AFFECTS_DEFAULT, host_radius_damage_affects_for_weapon_name,
            radius_damage_affects_victim,
        };
        let affects = weapon_name
            .map(host_radius_damage_affects_for_weapon_name)
            .unwrap_or(WEAPON_AFFECTS_DEFAULT);
        let (shooter_template, attacker_owner, attacker_team_instance, attacker_producer) = self
            .objects
            .get(&attacker_id)
            .map(|a| {
                (
                    a.template_name.clone(),
                    a.owner_player_id,
                    a.team_instance_name.clone(),
                    a.producer_id,
                )
            })
            .unwrap_or_default();
        // C++ Weapon.cpp dealDamageInternal iterates partition-world objects
        // only. Enclosing occupants (tunnel/transport) unRegisterObject on
        // enter, so splash at an entrance must not hit the shared pool.
        let enclosing_hidden = enclosing_hidden_rider_ids(&self.objects);
        let tunnel_held: std::collections::HashSet<u32> = self
            .tunnel_network
            .occupant_player_ids()
            .into_iter()
            .flat_map(|pid| self.tunnel_network.contained_for_player(pid))
            .map(|id| id.0)
            .collect();
        let players = &self.players;
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
                if enclosing_hidden.contains(&id.0) || tunnel_held.contains(&id.0) {
                    return None;
                }
                let airborne = obj.is_significantly_above_terrain();
                let same_tmpl = crate::game_logic::weapon_bootstrap::splash_templates_equivalent(
                    &shooter_template,
                    &obj.template_name,
                );
                let relationship = GameLogic::object_relationship_from_owners(
                    players,
                    obj.owner_player_id,
                    &obj.team_instance_name,
                    attacker_owner,
                    &attacker_team_instance,
                );
                if !radius_damage_affects_victim(
                    affects,
                    relationship,
                    attacker_id,
                    *id,
                    attacker_producer,
                    airborne,
                    same_tmpl,
                ) {
                    return None;
                }
                let dist = crate::game_logic::combat::splash_from_bounding_sphere_3d(
                    impact,
                    obj.get_position(),
                    crate::game_logic::combat::victim_splash_sphere_radius(obj),
                );
                if dist > max_r {
                    return None;
                }
                Some((*id, dist))
            })
            .collect();
        let mut hits = 0u32;
        let mut destroy: Vec<ObjectId> = Vec::new();
        for (id, dist) in candidates {
            let dmg = if primary_radius > 0.0 && dist <= primary_radius {
                primary_damage
            } else if secondary_damage > 0.0 && dist <= max_r {
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
            self.award_score_the_kill_experience(attacker_id, id);
            self.mark_object_for_destruction(id, Some(attacker_team));
        }
        let source_pos = self
            .objects
            .get(&attacker_id)
            .map(|a| a.get_position())
            .unwrap_or(impact);
        let _ = self.apply_shock_wave_at_impact(impact, source_pos, max_r, weapon_name, None);
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
        use crate::game_logic::weapon_bootstrap::{
            WEAPON_AFFECTS_DEFAULT, host_primary_damage_radius_for_weapon_name,
            host_radius_damage_affects_for_weapon_name, host_secondary_damage_for_weapon_name,
            host_secondary_damage_radius_for_weapon_name, radius_damage_affects_victim,
        };
        // C++ dealDamageInternal: authored primary/secondary radii, no 1.5x ring.
        let (primary_r, secondary_r, primary_dmg, secondary_dmg) = match weapon_name {
            Some(n) => {
                let radius_mult = self
                    .objects
                    .get(&attacker_id)
                    .map(|a| a.weapon_bonus_radius())
                    .unwrap_or(1.0);
                let pr = host_primary_damage_radius_for_weapon_name(n) * radius_mult;
                let sr = host_secondary_damage_radius_for_weapon_name(n) * radius_mult;
                let sd = host_secondary_damage_for_weapon_name(n);
                let primary = if pr > 0.0 {
                    pr
                } else {
                    splash_radius * radius_mult
                };
                (primary, sr, weapon_damage, sd)
            }
            None => (splash_radius, 0.0, weapon_damage, 0.0),
        };
        let max_r = primary_r.max(secondary_r);
        if max_r <= 0.0 || (primary_dmg <= 0.0 && secondary_dmg <= 0.0) {
            return 0;
        }
        let affects = weapon_name
            .map(host_radius_damage_affects_for_weapon_name)
            .unwrap_or(WEAPON_AFFECTS_DEFAULT);
        let (shooter_template, attacker_owner, attacker_team_instance, attacker_producer) = self
            .objects
            .get(&attacker_id)
            .map(|a| {
                (
                    a.template_name.clone(),
                    a.owner_player_id,
                    a.team_instance_name.clone(),
                    a.producer_id,
                )
            })
            .unwrap_or_default();
        let players = &self.players;
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
                let airborne = obj.is_significantly_above_terrain();
                let same_tmpl = crate::game_logic::weapon_bootstrap::splash_templates_equivalent(
                    &shooter_template,
                    &obj.template_name,
                );
                let relationship = GameLogic::object_relationship_from_owners(
                    players,
                    obj.owner_player_id,
                    &obj.team_instance_name,
                    attacker_owner,
                    &attacker_team_instance,
                );
                if !radius_damage_affects_victim(
                    affects,
                    relationship,
                    attacker_id,
                    *id,
                    attacker_producer,
                    airborne,
                    same_tmpl,
                ) {
                    return None;
                }
                let dist = crate::game_logic::combat::splash_from_bounding_sphere_3d(
                    impact,
                    obj.get_position(),
                    crate::game_logic::combat::victim_splash_sphere_radius(obj),
                );
                if dist > max_r {
                    return None;
                }
                Some((*id, dist))
            })
            .collect();
        let mut hits = 0u32;
        let mut destroy: Vec<ObjectId> = Vec::new();
        for (id, dist) in candidates {
            let dmg = if dist <= primary_r {
                primary_dmg
            } else {
                secondary_dmg
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
            self.award_score_the_kill_experience(attacker_id, id);
            self.mark_object_for_destruction(id, Some(attacker_team));
        }
        let source_pos = self
            .objects
            .get(&attacker_id)
            .map(|a| a.get_position())
            .unwrap_or(impact);
        let _ =
            self.apply_shock_wave_at_impact(impact, source_pos, max_r, weapon_name, Some(skip_id));
        hits
    }

    pub(crate) fn instant_scatter_misses_shot(
        &self,
        attacker_id: ObjectId,
        target_id: ObjectId,
        slot: u8,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            DEFAULT_SCATTER_HIT_RADIUS, host_effective_scatter_radius,
            scatter_misses_intended_target, scatter_seed_for_shot,
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
            || crate::game_logic::weapon_bootstrap::is_contact_effective_range(weapon_range);
        if contact {
            return target_pos;
        }
        let src = self
            .objects
            .get(&attacker_id)
            .map(|o| o.get_position())
            .unwrap_or(target_pos);
        let dest = crate::game_logic::weapon_bootstrap::compute_approach_target_pos(
            src,
            target_pos,
            weapon_range,
        );
        self.adjust_aircraft_attack_approach(attacker_id, dest, target_pos, weapon_range, 0.0)
    }

    /// C++ `isAircraftThatAdjustsDestination` + leftover `adjustTargetDestination`
    /// after range*0.9 so HOVER/WINGS do not stack on one hover cell.
    pub(crate) fn adjust_aircraft_attack_approach(
        &self,
        attacker_id: ObjectId,
        dest: glam::Vec3,
        target_pos: glam::Vec3,
        weapon_range: f32,
        min_range: f32,
    ) -> glam::Vec3 {
        let Some(obj) = self.objects.get(&attacker_id) else {
            return dest;
        };
        if !PathfindingGrid::is_aircraft_that_adjusts_destination(obj) {
            return dest;
        }
        let src_r = obj.thing.template.geometry_info.bounding_circle_radius();
        let unit_radius = obj.selection_radius.max(src_r);
        let tgt_r = self
            .objects
            .values()
            .filter(|o| o.id != attacker_id && o.is_alive())
            .find(|o| {
                let p = o.get_position();
                let dx = p.x - target_pos.x;
                let dz = p.z - target_pos.z;
                dx * dx + dz * dz < 1.0
            })
            .map(|o| o.thing.template.geometry_info.bounding_circle_radius())
            .unwrap_or(0.0);
        let min_range = self
            .objects
            .get(&attacker_id)
            .and_then(|a| {
                a.selected_weapon_slot()
                    .and_then(|s| a.weapon_slot(s))
                    .map(|w| w.min_range)
            })
            .unwrap_or(min_range);
        let airborne = obj.status.airborne_target
            || obj.is_kind_of(KindOf::Aircraft)
            || obj.object_type == crate::game_logic::ObjectType::Aircraft;
        let surfaces = if airborne {
            gamelogic::ai::pathfind_complete::SURFACE_AIR
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        self.pathfinding_system.adjust_target_destination(
            attacker_id.0,
            &self.objects,
            dest,
            target_pos,
            unit_radius,
            surfaces,
            obj.crusher_level > 0,
            src_r,
            tgt_r,
            weapon_range,
            min_range,
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

    /// C++ ExperienceTracker::addExperiencePoints sink + trainable gate.
    pub(crate) fn award_experience(&mut self, recipient_id: ObjectId, amount: f32) {
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        let Some(obj) = self.objects.get(&recipient_id) else {
            return;
        };
        if !obj.is_accepting_experience_points() {
            return;
        }
        // C++ forwards `experienceGain * m_experienceScalar` to the sink, then
        // the sink may scale again. Untrainable objects without an explicit
        // sink do not accept XP (no implicit producer_id).
        let source_scalar = if obj.experience_scalar.is_finite() && obj.experience_scalar > 0.0 {
            obj.experience_scalar
        } else {
            1.0
        };
        let dest = obj
            .experience_sink
            .filter(|sid| *sid != recipient_id && self.objects.contains_key(sid));
        let (dest_id, forwarded) = if let Some(sink_id) = dest {
            (sink_id, amount * source_scalar)
        } else {
            (recipient_id, amount)
        };
        if let Some(dest_obj) = self.objects.get_mut(&dest_id) {
            dest_obj.gain_experience(forwarded);
        }
    }

    /// C++ Object::scoreTheKill unit XP for residual apply / capture paths.
    pub(crate) fn award_score_the_kill_experience(
        &mut self,
        killer_id: ObjectId,
        victim_id: ObjectId,
    ) {
        let Some(victim) = self.objects.get(&victim_id) else {
            return;
        };
        if victim.kill_experience_awarded {
            return;
        }
        if victim.is_alive() && victim.health.current > 0.0 && !victim.status.destroyed {
            return;
        }
        let xp = victim.kill_experience_value();
        let team = victim.team;
        if let Some(v) = self.objects.get_mut(&victim_id) {
            v.kill_experience_awarded = true;
        }
        if !self.kill_awards_unit_experience(killer_id, victim_id, team) {
            return;
        }
        self.award_experience(killer_id, xp);
    }

    /// C++ Object::scoreTheKill + getExperienceValue: only ENEMIES, not own/allies.
    /// Also skips non-playable sides and KINDOF_IGNORED_IN_GUI (Object.cpp:2898-2905).
    fn kill_awards_unit_experience(
        &self,
        killer_id: ObjectId,
        victim_id: ObjectId,
        victim_team: Team,
    ) -> bool {
        use gamelogic::common::Relationship;
        let Some(killer) = self.objects.get(&killer_id) else {
            return false;
        };
        if let Some(victim) = self.objects.get(&victim_id) {
            if !self.score_the_kill_victim_counts(victim) {
                return false;
            }
            if killer.owner_player_id.is_some() && killer.owner_player_id == victim.owner_player_id
            {
                return false;
            }
            match (killer.owner_player_id, victim.owner_player_id) {
                (Some(a), Some(b)) => self.player_relationship(a, b) == Relationship::Enemies,
                _ => {
                    killer.team != victim.team
                        && killer.team != Team::Neutral
                        && victim.team != Team::Neutral
                }
            }
        } else {
            if victim_team == Team::Neutral {
                return false;
            }
            killer.team != victim_team
                && killer.team != Team::Neutral
                && victim_team != Team::Neutral
        }
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
        // C++ scores in ActiveBody; mark_object_for_destruction awards from
        // last_damage_source. Keep this for callers that skip mark.
        let _ = kill_xp;
        self.award_score_the_kill_experience(attacker_id, dead_victim_id);
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

/// C++ `OpenContain::addOrRemoveObjFromWorld` — enclosing riders leave the
/// collide partition on enter and re-register on exit. Live rebuilds each tick.
fn enclosing_hidden_rider_ids(
    objects: &std::collections::HashMap<ObjectId, Object>,
) -> std::collections::HashSet<u32> {
    let pairs: Vec<(ObjectId, ObjectId, String)> = objects
        .iter()
        .filter_map(|(id, o)| {
            o.contained_by
                .map(|cid| (*id, cid, o.template_name.clone()))
        })
        .collect();
    pairs
        .into_iter()
        .filter(|(rid, cid, tmpl)| {
            objects
                .get(cid)
                .is_some_and(|c| enclosing_container_hides_rider(c, *rid, tmpl))
        })
        .map(|(rid, _, _)| rid.0)
        .collect()
}

/// Mirrors `Object::is_enclosing_container_for` without dual HashMap borrows.
fn enclosing_container_hides_rider(
    container: &Object,
    victim_id: ObjectId,
    victim_template: &str,
) -> bool {
    let name = container.template_name.to_ascii_lowercase();
    if container.paradrop_parachute || name.contains("parachute") {
        return false;
    }
    if container.is_overlord_style_container() {
        if container.overlord_portable_occupant == Some(victim_id) {
            return false;
        }
        if container
            .contained_units()
            .first()
            .is_some_and(|&id| id == victim_id)
        {
            return false;
        }
        if crate::game_logic::host_battlemaster::is_portable_structure_template(victim_template) {
            return false;
        }
    }
    if container.is_helix_transport
        && crate::game_logic::host_battlemaster::is_portable_structure_template(victim_template)
    {
        return false;
    }
    if container.is_garrison_contain() {
        return container.is_enclosing_garrison_container();
    }
    true
}
