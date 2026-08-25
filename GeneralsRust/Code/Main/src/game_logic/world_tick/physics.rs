//! Host tick `impl GameLogic` — `physics`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    /// C++ Weapon ContinueAttackRange residual (AIStates attack transfer).
    ///
    /// When a kill lands and the firing weapon has ContinueAttackRange > 0,
    /// retarget the nearest living same-team-as-victim object within that
    /// radius of the original victim position (mine-clear chain residual).

    /// C++ Weapon computeApproachTarget residual for host attack moves.

    /// C++ MinimumAttackRange residual: back away when too close to fire.
    ///
    /// Returns true when a backup move was issued.

    /// C++ ScatterRadius residual for instant update_combat hits.
    ///
    /// Returns true when the shot misses the intended target after scatter offset.

    /// Resolve ScatterRadius for an instant shot: (misses_intended, impact_pos, splash_r).
    pub(crate) fn resolve_instant_scatter_shot(
        &self,
        attacker_id: ObjectId,
        target_id: ObjectId,
        slot: u8,
        target_pos: glam::Vec3,
        table_offset: Option<glam::Vec2>,
    ) -> (bool, glam::Vec3, f32) {
        use crate::game_logic::weapon_bootstrap::{
            DEFAULT_SCATTER_HIT_RADIUS, host_effective_scatter_radius,
            host_primary_damage_radius_for_weapon_name,
            host_secondary_damage_radius_for_weapon_name, scatter_impact_offset,
            scatter_misses_intended_target, scatter_seed_for_shot,
        };
        let (wname, tgt_inf, hit_r, weapon_splash, radius_mult) = {
            let attacker = match self.objects.get(&attacker_id) {
                Some(a) => a,
                None => return (false, target_pos, 0.0),
            };
            let target = match self.objects.get(&target_id) {
                Some(t) => t,
                None => return (false, target_pos, 0.0),
            };
            let wname = attacker.weapon_name_for_slot(slot).map(str::to_owned);
            let hit_r = if target.selection_radius > 0.0 {
                target.selection_radius
            } else {
                DEFAULT_SCATTER_HIT_RADIUS
            };
            let splash = attacker
                .weapon_slot(slot)
                .map(|w| w.splash_radius.max(0.0))
                .unwrap_or(0.0);
            (
                wname,
                target.is_kind_of(KindOf::Infantry),
                hit_r,
                splash,
                attacker.weapon_bonus_radius(),
            )
        };
        let splash_r = {
            let primary_r = wname
                .as_deref()
                .map(host_primary_damage_radius_for_weapon_name)
                .unwrap_or(0.0);
            let secondary_r = wname
                .as_deref()
                .map(host_secondary_damage_radius_for_weapon_name)
                .unwrap_or(0.0);
            weapon_splash.max(primary_r).max(secondary_r) * radius_mult
        };
        if let Some(offset) = table_offset {
            let mut impact = target_pos;
            impact.x += offset.x;
            impact.z += offset.y;
            if let Some(target) = self.objects.get(&target_id) {
                if target.ground_height_from_terrain {
                    impact.y = target.ground_height;
                }
            }
            // C++ nulls victimObj — treat as a position shot / intended miss.
            return (true, impact, splash_r);
        }
        let Some(name) = wname else {
            return (false, target_pos, splash_r);
        };
        let scatter = host_effective_scatter_radius(&name, tgt_inf);
        if scatter <= 0.0 {
            return (false, target_pos, splash_r);
        }
        let seed = scatter_seed_for_shot(attacker_id.0, target_id.0, self.frame);
        let offset = scatter_impact_offset(seed, scatter);
        let impact = target_pos + offset;
        let misses = scatter_misses_intended_target(scatter, seed, hit_r);
        (misses, impact, splash_r)
    }

    /// Apply residual area damage at a scatter impact point (missed intended target).
    ///
    /// Deals full `weapon_damage` within primary splash radius and half in the
    /// outer ring to secondary radius. Respects team vs RadiusDamageAffects ENEMIES
    /// residual (host default: enemies + neutrals).

    /// C++ dealDamageInternal dual-radius residual after a direct hit.
    ///
    /// Intended target is already damaged; splash hits others in primary/secondary
    /// rings using PrimaryDamage / SecondaryDamage peels and RadiusDamageAffects.

    /// C++ shockwave residual around an impact (hit or miss splash).

    /// Sample TerrainLogic-ish cliff/water residual at world position for stun destruction.

    /// C++ TerrainLogic::setWaterHeight damage residual.
    ///
    /// When water rises, every object currently underwater takes `damage_amount`
    /// as DAMAGE_WATER (DEATH_NORMAL). C++ has no aircraft/boat skip.
    /// Returns number of objects damaged.
    pub fn apply_water_rise_damage(&mut self, damage_amount: f32) -> u32 {
        if !(damage_amount > 0.0) {
            return 0;
        }
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        let mut hit = 0u32;
        let mut destroy: Vec<ObjectId> = Vec::new();
        for id in ids {
            let pos = match self.objects.get(&id) {
                Some(o) => o.get_position(),
                None => continue,
            };
            let (_cliff, water) = self.sample_stun_surface_at(pos);
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            obj.cell_is_underwater = water;
            if !water || !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            if obj.is_kind_of(KindOf::Projectile) {
                continue;
            }
            let killed = obj.take_damage_from_typed(
                damage_amount,
                None,
                crate::game_logic::combat::DamageType::Water,
            );
            hit = hit.saturating_add(1);
            if killed || obj.status.destroyed || obj.health.current <= 0.0 {
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
        hit
    }

    /// Refresh underwater/cliff cells. C++ never damages on dry→wet walk-in;
    /// DAMAGE_WATER only runs from leftover setWaterHeight rise (`apply_water_rise_damage`).
    pub fn refresh_surface_cells_and_water_edge_damage(&mut self, _edge_damage: f32) -> u32 {
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            let pos = match self.objects.get(&id) {
                Some(o) => o.get_position(),
                None => continue,
            };
            let (_cliff, water) = self.sample_stun_surface_at(pos);
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            obj.cell_is_cliff = _cliff;
            obj.cell_is_underwater = water;
        }
        0
    }

    pub(crate) fn sample_stun_surface_at(&self, pos: glam::Vec3) -> (bool, bool) {
        if let Some(t) = self.terrain.as_ref() {
            return (t.is_cliff_at_world(pos), t.is_underwater_at_world(pos));
        }
        // Fall back to gamelogic TerrainLogic singleton when Main terrain is unset.
        if let Ok(tl) = gamelogic::terrain::get_terrain_logic().read() {
            // Host XZ ground plane == C++ XY for terrain queries.
            let cliff = tl.is_cliff_cell(pos.x, pos.z);
            let water = tl.is_underwater(pos.x, pos.z, None, None);
            return (cliff, water);
        }
        (false, false)
    }

    /// Advance Physics stun residual on all shocked units.
    ///
    /// Refreshes cell_is_cliff / cell_is_underwater from terrain before each tick
    /// so testStunnedUnitForDestruction sees live surface residual.

    /// C++ PhysicsBehavior onCollide vehicle crash weapon residual.
    ///
    /// Fires temp crash weapon name residual (splash damage at vehicle) and
    /// destroys the vehicle when falling into a structure.

    /// C++ PhysicsBehavior onCollide immobile bounce residual (stiffness / parachute).
    ///
    /// Returns true if bounce was applied. Vehicle crash path is separate
    /// (`apply_vehicle_crash_into_immobile`).

    /// C++ PhysicsBehavior::checkForOverlapCollision residual between two objects.
    ///
    /// `same_team` treats relationship as allies (no crush).

    /// C++ PhysicsBehavior::onCollide residual orchestration (host).
    ///
    /// Order: ignore-collisions gate → mutual parachute skip → overlap crush
    /// → immobile bounce/crash. Sets last_collidee when a real collide runs.
    /// Returns true if the pair was handled (skip generic bounce force).
    pub fn try_physics_collide(&mut self, a_id: ObjectId, b_id: ObjectId, us_radius: f32) -> bool {
        // Snapshot flags without holding borrows across mutates.
        let (
            a_ignore_b,
            b_ignore_a,
            a_para,
            b_para,
            b_immobile,
            a_infantry,
            b_unmanned,
            a_ignore_obs,
            b_ignore_obs,
            a_contained_by,
            b_contained_by,
        ) = {
            let Some(a) = self.objects.get(&a_id) else {
                return false;
            };
            let Some(b) = self.objects.get(&b_id) else {
                return false;
            };
            (
                a.is_ignoring_collisions_with(b_id),
                b.is_ignoring_collisions_with(a_id),
                a.is_parachuting(),
                b.is_parachuting(),
                // C++ PhysicsUpdate.cpp:1221-1222 / leftover physics_collide.rs:144:
                // otherImmobile = isKindOf(KINDOF_IMMOBILE) only. Dead, EMP,
                // deployed, docked, or garrisoned mobiles stay processCollision.
                b.is_kind_of(crate::game_logic::KindOf::Immobile),
                a.is_kind_of(crate::game_logic::KindOf::Infantry),
                b.status.disabled_unmanned,
                a.ignored_obstacle_id == Some(b_id),
                b.ignored_obstacle_id == Some(a_id),
                a.contained_by,
                b.contained_by,
            )
        };
        if a_ignore_b || b_ignore_a {
            return true; // ignore = handled (no bounce)
        }
        // C++ both parachuting: never collide.
        if a_para && b_para {
            return true;
        }
        // C++ Object::onCollide walks only this object's collide modules.
        // Reverse side is the pair loop's second try_physics_collide.
        self.dispatch_host_collide_modules(a_id, b_id);
        // C++ PhysicsUpdate.cpp:1167-1172 — container/occupant never bounce/crush.
        if a_contained_by == Some(b_id) || b_contained_by == Some(a_id) {
            return true;
        }
        // C++ PhysicsUpdate.cpp:1182-1213: recrew only inside
        // ai->getIgnoredObstacleID() == other->getID(); else bounce/crush.
        if a_ignore_obs {
            if a_infantry && b_unmanned && self.try_infantry_unmanned_reclaim(a_id, b_id) {
                if let Some(a) = self.objects.get_mut(&a_id) {
                    a.last_collidee = Some(b_id);
                }
            }
            return true;
        }
        if b_ignore_obs {
            let b_inf = self
                .objects
                .get(&b_id)
                .map(|o| o.is_kind_of(crate::game_logic::KindOf::Infantry))
                .unwrap_or(false);
            let a_unm = self
                .objects
                .get(&a_id)
                .map(|o| o.status.disabled_unmanned)
                .unwrap_or(false);
            if b_inf && a_unm && self.try_infantry_unmanned_reclaim(b_id, a_id) {
                if let Some(a) = self.objects.get_mut(&a_id) {
                    a.last_collidee = Some(b_id);
                }
            }
            return true;
        }

        let is_ally = self.crush_relationship_is_allies(a_id, b_id);
        // C++ ToppleUpdate::onCollide residual: crusher_level > 1 topples trees/props.
        if self.try_topple_on_collide(a_id, b_id) || self.try_topple_on_collide(b_id, a_id) {
            if let Some(a) = self.objects.get_mut(&a_id) {
                a.last_collidee = Some(b_id);
            }
            return true;
        }
        // Overlap crush (may handle the pair).
        if self.apply_overlap_crush_check(a_id, b_id, is_ally) {
            if let Some(a) = self.objects.get_mut(&a_id) {
                a.last_collidee = Some(b_id);
            }
            return true;
        }
        // Immobile bounce path.
        // C++ PhysicsUpdate.cpp:1255-1264: living AI calls processCollision;
        // AIUpdate.cpp:1423-1425 returns FALSE when other has no AI (buildings).
        // Stiffness rebound is dead/parachute only.
        if b_immobile {
            let (has_ai, dead_or_para, allow) = {
                let Some(a) = self.objects.get(&a_id) else {
                    return false;
                };
                let dead = !a.is_alive();
                let para = a.is_parachuting();
                let projectile = a.is_kind_of(crate::game_logic::KindOf::Projectile)
                    || a.object_type == crate::game_logic::ObjectType::Projectile;
                (
                    a.is_mobile() || projectile,
                    dead || para,
                    a.allow_collide_force,
                )
            };
            if has_ai && !dead_or_para {
                let frame = self.frame;
                let b_snap = match self.objects.get(&b_id) {
                    Some(b) => b.clone(),
                    None => return false,
                };
                let do_force = match self.objects.get_mut(&a_id) {
                    Some(a) => a.ai_process_collision(&b_snap, frame, is_ally),
                    None => return false,
                };
                if !do_force {
                    return true; // processCollision refused bounce
                }
            }
            if !allow {
                return true; // handled as no-force
            }
            let handled = self.apply_immobile_collide_bounce(a_id, b_id, us_radius);
            if handled {
                if let Some(a) = self.objects.get_mut(&a_id) {
                    a.last_collidee = Some(b_id);
                }
            }
            return handled;
        }
        // Mobile-mobile: AI processCollision residual (usually no bounce force).
        let frame = self.frame;
        let b_snap = match self.objects.get(&b_id) {
            Some(b) => b.clone(),
            None => return false,
        };
        let allow_force = match self.objects.get_mut(&a_id) {
            Some(a) => a.ai_process_collision(&b_snap, frame, is_ally),
            None => return false,
        };
        let req_away = {
            let Some(a) = self.objects.get_mut(&a_id) else {
                return false;
            };
            a.last_collidee = Some(b_id);
            if a.is_blocked {
                a.apply_blocked_speed_cap();
            }
            a.request_other_move_away.take()
        };
        if let Some(other_id) = req_away {
            let a_path = self
                .objects
                .get(&a_id)
                .map(|a| a.movement.path.clone())
                .unwrap_or_default();
            let a_radius = self
                .objects
                .get(&a_id)
                .map(|a| a.selection_radius)
                .unwrap_or(0.0);
            if a_path.len() >= 2 {
                if let Some(other) = self.objects.get(&other_id) {
                    let from = other.get_position();
                    let surfaces = if other.locomotor_surfaces != 0 {
                        other.locomotor_surfaces
                    } else {
                        gamelogic::ai::pathfind_complete::SURFACE_GROUND
                    };
                    let is_crusher = other.crusher_level > 0;
                    let unit_radius = other.selection_radius;
                    let seeker_player = other.owner_player_id.or(Some(other.team as u32));
                    let crusher_level = other.crusher_level;
                    let can_tunnel = other.can_path_through_units;
                    let mut yield_path = self.pathfinding_system.get_move_away_from_path(
                        from,
                        &a_path,
                        None,
                        surfaces,
                        is_crusher,
                        unit_radius,
                        a_radius,
                        seeker_player,
                        crusher_level,
                        false,
                    );
                    if yield_path.is_none() && !can_tunnel {
                        yield_path = self.pathfinding_system.get_move_away_from_path(
                            from,
                            &a_path,
                            None,
                            surfaces,
                            is_crusher,
                            unit_radius,
                            a_radius,
                            seeker_player,
                            crusher_level,
                            true,
                        );
                    }
                    if let Some(path) = yield_path {
                        if let Some(other) = self.objects.get_mut(&other_id) {
                            other.apply_move_away_path(a_id, &path);
                            if other.ignore_collisions_until_frame > 0
                                && other.ignore_collisions_until_frame < 100_000
                            {
                                other.ignore_collisions_until_frame = frame.saturating_add(60);
                            }
                        }
                    }
                }
            }
        }
        if !allow_force {
            return true; // AI handled / no force
        }
        // C++ PhysicsUpdate.cpp:1278-1398: airborne = 3D sphere radii + delta;
        // ground = 2D circle; early-out if dist exceeds radius sum; overlap cap 5.
        if let Some(a) = self.objects.get_mut(&a_id) {
            if a.allow_collide_force {
                let us = a.get_position();
                let them = b_snap.get_position();
                let airborne = a.is_above_terrain();
                let us_r = if airborne {
                    a.physics_collide_sphere_radius()
                } else {
                    a.physics_collide_circle_radius()
                };
                let them_r = if airborne {
                    b_snap.physics_collide_sphere_radius()
                } else {
                    b_snap.physics_collide_circle_radius()
                };
                let dx = them.x - us.x;
                let dy = if airborne { them.y - us.y } else { 0.0 };
                let dz = them.z - us.z;
                let dist_sqr = dx * dx + dy * dy + dz * dz;
                let radius_sum = us_r + them_r;
                if dist_sqr <= radius_sum * radius_sum {
                    let dist = dist_sqr.sqrt();
                    let overlap = radius_sum - dist;
                    if overlap > 0.0 {
                        a.apply_overlap_collide_force(them, overlap);
                    }
                }
            }
        }
        true
    }

    /// C++ ToppleUpdate::onCollide residual — `crusher` may topple `prop`.
    pub(in super::super) fn try_topple_on_collide(
        &mut self,
        crusher_id: ObjectId,
        prop_id: ObjectId,
    ) -> bool {
        let (level, cpos, speed) = {
            let Some(c) = self.objects.get(&crusher_id) else {
                return false;
            };
            if c.status.destroyed || !c.is_alive() {
                return false;
            }
            let pos = c.get_position();
            let sp = (c.movement.velocity.x * c.movement.velocity.x
                + c.movement.velocity.z * c.movement.velocity.z)
                .sqrt();
            (c.crusher_level, pos, sp)
        };
        if !crate::game_logic::host_topple::crusher_can_topple(level) {
            return false;
        }
        let (kill_now, handled, stump, pos, ori) = {
            let Some(prop) = self.objects.get_mut(&prop_id) else {
                return false;
            };
            let kill_now = prop.try_topple_from_crusher(level, cpos.x, cpos.z, speed.max(1.0));
            let handled = prop
                .topple_data
                .as_ref()
                .map(|t| {
                    !matches!(
                        t.state,
                        crate::game_logic::host_topple::HostToppleState::Upright
                    )
                })
                .unwrap_or(false)
                || kill_now;
            let stump = prop
                .topple_data
                .as_mut()
                .and_then(|t| t.take_pending_stump_name().map(|n| (n, t.burned_at_topple)));
            let (pos, ori) = (prop.get_position(), prop.get_orientation());
            (kill_now, handled, stump, pos, ori)
        };
        if let Some((name, burned)) = stump {
            self.spawn_topple_stump(&name, pos, ori, burned);
        }
        if kill_now {
            self.mark_object_for_destruction(prop_id, None);
        }
        handled
    }

    /// C++ ToppleUpdate::applyTopplingForce stump spawn at the tree pose.
    pub(in super::super) fn spawn_topple_stump(
        &mut self,
        stump_name: &str,
        pos: glam::Vec3,
        orientation: f32,
        burned: bool,
    ) {
        if stump_name.is_empty() {
            return;
        }
        if !self.templates.contains_key(stump_name) {
            let mut t = crate::game_logic::ThingTemplate::new(stump_name);
            t.set_health(1.0);
            self.templates.insert(stump_name.to_string(), t);
        }
        let Some(id) = self.create_object(stump_name, crate::game_logic::Team::Neutral, pos) else {
            return;
        };
        if let Some(stump) = self.objects.get_mut(&id) {
            stump.set_orientation(orientation);
            if burned {
                use crate::game_logic::host_enum_table_residual::burned_model_bit;
                stump.model_condition_bits |= 1u128 << burned_model_bit();
                let _ = stump.apply_status_bits_upgrade_masks(&["BURNED"], &[]);
            }
        }
    }

    /// C++ `Object::getRelationship(other) == ALLIES` for crush gates
    /// (Object.cpp:1096 — crusher's view of the victim, not faction Team).
    fn crush_relationship_is_allies(&self, crusher_id: ObjectId, crushee_id: ObjectId) -> bool {
        use gamelogic::common::Relationship;
        let Some(crusher) = self.objects.get(&crusher_id) else {
            return false;
        };
        let Some(crushee) = self.objects.get(&crushee_id) else {
            return false;
        };
        self.object_relationship(crusher, crushee) == Relationship::Allies
    }

    pub fn apply_overlap_crush_check(
        &mut self,
        crusher_id: ObjectId,
        crushee_id: ObjectId,
        is_ally: bool,
    ) -> bool {
        // Split borrow: take crushee out, mutate both, put back.
        let Some(mut crushee) = self.objects.remove(&crushee_id) else {
            return false;
        };
        let result = if let Some(crusher) = self.objects.get_mut(&crusher_id) {
            crusher.check_for_overlap_collision(&mut crushee, is_ally)
        } else {
            false
        };
        self.objects.insert(crushee_id, crushee);
        result
    }

    pub fn apply_immobile_collide_bounce(
        &mut self,
        mover_id: ObjectId,
        immobile_id: ObjectId,
        us_radius: f32,
    ) -> bool {
        use crate::game_logic::host_partition_collision_physics_residual::PHYSICS_STRUCTURE_STIFFNESS_DEFAULT_RESIDUAL;
        let (mover_para, imm_center, imm_ok) = {
            let Some(m) = self.objects.get(&mover_id) else {
                return false;
            };
            let Some(imm) = self.objects.get(&immobile_id) else {
                return false;
            };
            // C++ PhysicsUpdate.cpp:1222 / leftover: KINDOF_IMMOBILE only.
            let imm_ok = imm.is_kind_of(crate::game_logic::KindOf::Immobile);
            (m.is_parachuting(), imm.get_position(), imm_ok)
        };
        if !imm_ok {
            return false;
        }
        if mover_para {
            // C++ PhysicsUpdate.cpp:1322-1332 / leftover physics_collide.rs:199-221:
            // walk getContainedBy to the outermost container, jam THAT object
            // (usually the chute) by 0.1*usRadius, scrubVelocity2D(0).
            // ParachuteContain is not enclosing — bouncing the rider would pull
            // them out of the harness.
            let bounce_id = {
                let mut bounce_id = mover_id;
                let mut hops = 0u8;
                while hops < 8 {
                    let Some(next) = self.objects.get(&bounce_id).and_then(|o| o.contained_by)
                    else {
                        break;
                    };
                    if next == bounce_id {
                        break;
                    }
                    bounce_id = next;
                    hops += 1;
                }
                bounce_id
            };
            if let Some(bounce) = self.objects.get_mut(&bounce_id) {
                bounce.apply_parachute_building_bounce_out(imm_center, us_radius);
            }
            return true;
        }
        // C++ PhysicsUpdate.cpp:1353-1365 — fall-into-structure destroyObject
        // (weapon only if vehicle) before stiffness bounce.
        if self
            .apply_vehicle_crash_into_immobile(mover_id, immobile_id)
            .is_some()
            && self
                .objects
                .get(&mover_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true)
        {
            return true;
        }
        if let Some(m) = self.objects.get_mut(&mover_id) {
            // Live destroyed ≈ C++ effectivelyDead hulks still in the world;
            // they still stiffness-bounce. C++ isDestroyed() is remove-from-world.
            let _ = m.apply_structure_stiffness_bounce(
                imm_center,
                PHYSICS_STRUCTURE_STIFFNESS_DEFAULT_RESIDUAL,
                crate::game_logic::Object::SHOCK_MASS,
            );
            return true;
        }
        false
    }

    pub fn apply_vehicle_crash_into_immobile(
        &mut self,
        vehicle_id: ObjectId,
        other_id: ObjectId,
    ) -> Option<&'static str> {
        use crate::game_logic::host_partition_collision_physics_residual::{
            VehicleCrashImmobileOutcome, vehicle_crash_destroys_vehicle, vehicle_crash_weapon_name,
        };
        let outcome = {
            let Some(v) = self.objects.get(&vehicle_id) else {
                return None;
            };
            let Some(o) = self.objects.get(&other_id) else {
                return None;
            };
            v.evaluate_vehicle_crash_into(o)
        };
        if matches!(outcome, VehicleCrashImmobileOutcome::None) {
            return None;
        }
        let weapon = vehicle_crash_weapon_name(outcome);
        let pos = self
            .objects
            .get(&vehicle_id)
            .map(|o| o.get_position())
            .unwrap_or(glam::Vec3::ZERO);
        // C++ TheWeaponStore->createAndFireTempWeapon(template, obj, pos)
        // then destroyObject for structures (PhysicsUpdate.cpp:1361-1364).
        // Non-vehicles skip the weapon but still destroyObject.
        if let Some(weapon) = weapon {
            let spec = crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadEphemeralWeaponSpec {
                module_source_index: 0,
                weapon_template_name: weapon.to_string(),
                weapon_slot:
                    crate::game_logic::host_temporary_weapon_behavior::TemporaryWeaponSlot::Primary,
            };
            let _ = self.create_and_fire_temp_weapon(vehicle_id, &spec);
        }
        if vehicle_crash_destroys_vehicle(outcome) {
            if let Some(v) = self.objects.get_mut(&vehicle_id) {
                // Damage authority: HP last-writer via damage log; destroy flag stays host.
                let hp = v.health.current.max(1.0);
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    crate::game_logic::host_damage_log::record(
                        vehicle_id,
                        hp,
                        Some(vehicle_id),
                        true,
                    );
                } else {
                    v.health.current = 0.0;
                }
                v.status.destroyed = true;
                v.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Exploded;
            }
            if let Some(weapon) = weapon {
                self.queue_audio_event(
                    AudioEventRequest::new(weapon)
                        .with_object(vehicle_id)
                        .with_position(pos)
                        .with_priority(200),
                );
            }
        } else if let Some(weapon) = weapon {
            self.queue_audio_event(
                AudioEventRequest::new(weapon)
                    .with_object(vehicle_id)
                    .with_position(pos)
                    .with_priority(160),
            );
        }
        Some(weapon.unwrap_or(""))
    }
    /// C++ `OpenContain::getContainedItemsMass` walk — refresh hull cache so
    /// `Object::physics_get_mass` matches `m_mass + contain->getContainedItemsMass()`.
    pub fn sync_contained_items_mass(&mut self, container_id: ObjectId) {
        let occ = self
            .objects
            .get(&container_id)
            .map(|o| o.contained_units())
            .unwrap_or_default();
        let mut mass = 0.0;
        for oid in occ {
            if oid == container_id {
                continue;
            }
            self.sync_contained_items_mass(oid);
            if let Some(o) = self.objects.get(&oid) {
                mass += o.physics_get_mass();
            }
        }
        if let Some(c) = self.objects.get_mut(&container_id) {
            c.contained_items_mass = mass;
        }
    }

    /// Refresh cargo mass on every container before physics forces.
    pub fn sync_all_contained_items_mass(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| !o.contained_units().is_empty())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.sync_contained_items_mass(id);
        }
    }

    /// C++ partition collide residual: pairwise near-object physics collide.
    ///
    /// Partition cell broadphase (cell size 40) + selection_radius XZ spheres.
    /// Advances overlap frame after pairs. Fail-closed vs full ghost/shroud cells.
    /// Returns number of pairs that invoked try_physics_collide successfully.

    /// C++ AIUpdateInterface::privateFaceObject residual.
    ///
    /// Enter persist-until-faced Face (ANGLE vs POSITION_EXPLICIT). Do not
    /// one-frame yaw-snap; leftover `AIFaceState` marches via doLocomotor.
    pub fn private_face_object(&mut self, unit_id: ObjectId, target_id: ObjectId) -> bool {
        let Some(target_pos) = self.objects.get(&target_id).map(|o| o.get_position()) else {
            return false;
        };
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        if !u.can_move() {
            return false;
        }
        u.is_blocked = false;
        u.is_blocked_and_stuck = false;
        u.target = Some(target_id);
        u.face_goal_pos = None;
        u.face_can_turn_in_place = u.min_speed == 0.0;
        u.face_active = true;
        u.face_loco_frame = 0;
        let _ = u.arm_face_locomotor_goal(target_pos);
        if !matches!(u.ai_state, AIState::SpecialAbility | AIState::Capturing) {
            u.set_ai_state(AIState::FacingObject);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(unit_id, target_id);
        }
        true
    }

    /// C++ AIUpdateInterface::privateFacePosition residual.
    pub fn private_face_position(&mut self, unit_id: ObjectId, pos: glam::Vec3) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        if !u.can_move() {
            return false;
        }
        u.is_blocked = false;
        u.is_blocked_and_stuck = false;
        u.face_goal_pos = Some(pos);
        u.face_can_turn_in_place = u.min_speed == 0.0;
        u.face_active = true;
        u.face_loco_frame = 0;
        let _ = u.arm_face_locomotor_goal(pos);
        if !matches!(u.ai_state, AIState::SpecialAbility | AIState::Capturing) {
            u.set_ai_state(AIState::FacingPosition);
        }
        true
    }

    /// C++ AIUpdateInterface::privateIdle residual.
    pub fn private_idle(&mut self, unit_id: ObjectId) -> bool {
        let Some(u) = self.objects.get(&unit_id) else {
            return false;
        };
        if u.is_kind_of(crate::game_logic::KindOf::Projectile) {
            return false;
        }
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        if decision_auth {
            // Stop movement residual stays host; engagement/state via decision log.
            if let Some(u) = self.objects.get_mut(&unit_id) {
                u.stop_moving();
            }
            crate::game_logic::host_ai_decision_log::record_stop_attack(unit_id);
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 0);
        // Idle
        } else if let Some(u) = self.objects.get_mut(&unit_id) {
            u.stop_moving();
            u.set_status_attacking(false);
            u.target = None;
            u.set_ai_state(AIState::Idle);
        }
        true
    }

    #[cfg(test)]
    pub fn private_idle_for_test(&mut self, unit_id: ObjectId) -> bool {
        self.private_idle(unit_id)
    }

    /// C++ AIAttackAimAtTargetState::onEnter residual.

    /// C++ AIAttackState::onEnter residual — start nested AttackStateMachine at AIM.

    /// C++ Object::chooseBestWeaponForTarget / AIAttackState::chooseWeapon residual.
    ///
    /// Locks `active_weapon_slot` to the PreferMostDamage residual choice.
    /// Returns false when no legal weapon exists for the victim (or ground).
    pub fn choose_best_weapon_for_target(
        &mut self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        current_time: f32,
    ) -> bool {
        let Some(u) = self.objects.get(&unit_id) else {
            return false;
        };
        if !u.is_alive() {
            return false;
        }
        // C++ WeaponSet::chooseBestWeaponForTarget immediately accepts the
        // current slot while locked.  In particular, do not auto-choose
        // PRIMARY/SECONDARY over an explicitly requested TERTIARY while its
        // clip is reloading or the target is currently out of range.
        if u.weapon_lock_type != WeaponLockType::NotLocked {
            return u.weapon_slot(u.weapon_lock_slot).is_some();
        }
        // Leftover chooseBest no-victim: lock already returned; else PRIMARY.
        let Some(vid) = victim_id else {
            let has =
                u.weapon.is_some() || u.secondary_weapon.is_some() || u.tertiary_weapon.is_some();
            if has {
                if let Some(uu) = self.objects.get_mut(&unit_id) {
                    uu.leftover_choose_best_reset_primary_for_ground();
                }
            }
            return has;
        };
        let Some(v) = self.objects.get(&vid) else {
            return false;
        };
        // Snapshot selection without holding mut borrow across get_mut.
        let slot = u.select_combat_weapon_slot(v, current_time);
        let Some(slot) = slot else {
            // Leftover chooseBest: no valid weapon → reset PRIMARY, return FALSE.
            if let Some(uu) = self.objects.get_mut(&unit_id) {
                uu.set_active_weapon_slot(0);
            }
            return false;
        };
        if let Some(uu) = self.objects.get_mut(&unit_id) {
            uu.set_active_weapon_slot(slot);
        }
        true
    }

    /// C++ cannotPossiblyAttackObject state condition residual.
    ///
    /// True when the attacker cannot possibly continue attacking the victim
    /// (dead, no weapon, same team residual, stealthed undetected).

    /// C++ WeaponSet::getAbleToAttackSpecificObject residual (host-simplified).

    /// C++ AIUpdateInterface::transferAttack residual.
    ///
    /// Retargets units attacking `from_id` onto `to_id` (rebuild hole / create-object die).
    pub fn transfer_attack(&mut self, from_id: ObjectId, to_id: ObjectId) -> usize {
        let new_alive = self
            .objects
            .get(&to_id)
            .map(|o| o.is_alive() && !o.status.destroyed)
            .unwrap_or(false);
        if !new_alive {
            return 0;
        }
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        let mut transferred = 0usize;
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            if id == from_id || id == to_id {
                continue;
            }
            // Snapshot engagement before mut borrow.
            let (had_target, had_turret) = {
                let Some(u) = self.objects.get(&id) else {
                    continue;
                };
                (
                    u.target == Some(from_id),
                    u.turret_target_id == Some(from_id),
                )
            };
            if !had_target && !had_turret {
                continue;
            }
            if had_target {
                // C++ transferAttack always retargets on the host immediately
                // (rebuild-hole / death handoff). Still log under decision authority
                // so GameWorld can last-write, but never leave host pointing at a corpse.
                if let Some(u) = self.objects.get_mut(&id) {
                    u.target = Some(to_id);
                }
                if decision_auth {
                    crate::game_logic::host_ai_decision_log::record_attack(id, to_id);
                }
            }
            if had_turret {
                // Turret aim residual stays host (not AI decision channel).
                if let Some(u) = self.objects.get_mut(&id) {
                    u.turret_target_id = Some(to_id);
                    u.turret_force_attacking = true;
                    if matches!(
                        u.turret_substate,
                        crate::game_logic::object::TurretSubState::Idle
                            | crate::game_logic::object::TurretSubState::Hold
                            | crate::game_logic::object::TurretSubState::Recenter
                    ) {
                        u.turret_substate = crate::game_logic::object::TurretSubState::Aim;
                    }
                }
            }
            transferred += 1;
        }
        transferred
    }

    /// C++ `Locomotor::handleBehaviorZ` live pose (Locomotor.cpp:2196-2323).
    ///
    /// `Z_SURFACE_RELATIVE_HEIGHT` / `Z_SMOOTH_RELATIVE_TO_HIGHEST_LAYER` apply
    /// lift via `calcLiftToUseAtPt` + `applyMotiveForce` — never `setPosition`
    /// on Z (hq-ygdfb). Host march skips full Physics `pos+=v` while pathing,
    /// so integrate the leftover Y Euler step here (maxLift / speedLimitZ / vel.y).
    pub(in super::super) fn apply_live_handle_behavior_z(
        obj: &mut Object,
        ground_y: f32,
        goal_y: Option<f32>,
    ) {
        // C++ handleBehaviorZ always receives locomotor goalPos; PRECISE_Z_POS
        // selects goal.z over preferredHeight + surface (Locomotor.cpp:2296-2300).
        let goal_y = goal_y.or_else(|| obj.movement.target_position.map(|p| p.y));
        let hover = matches!(
            obj.loco_appearance,
            LocomotorAppearance::Hover | LocomotorAppearance::Wings
        );
        let z_motive = matches!(
            obj.loco_behavior_z,
            LocomotorBehaviorZ::SurfaceRelativeHeight
                | LocomotorBehaviorZ::SmoothRelativeToHighestLayer
                | LocomotorBehaviorZ::AbsoluteHeight
                | LocomotorBehaviorZ::SeaLevel
                | LocomotorBehaviorZ::FixedSurfaceRelativeHeight
                | LocomotorBehaviorZ::FixedAbsoluteHeight
                | LocomotorBehaviorZ::RelativeToGroundAndBuildings
        );
        if !z_motive && !hover {
            return;
        }
        // Hover with no authored Z still uses Z_SURFACE_RELATIVE_HEIGHT
        // (Locomotor.cpp:2288).
        if hover && matches!(obj.loco_behavior_z, LocomotorBehaviorZ::NoZMotiveForce) {
            obj.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        }
        let _ = obj.handle_behavior_z(ground_y, goal_y);
        match obj.loco_behavior_z {
            LocomotorBehaviorZ::SurfaceRelativeHeight
            | LocomotorBehaviorZ::SmoothRelativeToHighestLayer
            | LocomotorBehaviorZ::AbsoluteHeight => {
                // Leftover PhysicsBehavior Euler Z: vel += a; pos += vel.
                // C++ PhysicsUpdate.cpp:626-636 applyGravitationalForces unless
                // DISABLED_HELD, then integrate. calc_lift_to_use_at_pt returns
                // desiredAccel - gravity so lift + gravity nets climb/hold (hq-g8oig).
                if !obj.is_physics_held() {
                    obj.apply_gravitational_forces();
                    let y_a = obj.physics_accel.y;
                    if y_a.abs() > 1.0e-8 {
                        obj.movement.velocity.y += y_a;
                        obj.physics_accel.y = 0.0;
                        obj.invalidate_velocity_magnitude();
                        let mut p = obj.get_position();
                        p.y += obj.movement.velocity.y;
                        obj.set_position(p);
                        obj.record_host_movement();
                    }
                }
            }
            _ => {}
        }
    }

    /// C++ Object::getCarrierDeckHeight — producer's ParkingPlace landing offset.
    pub(crate) fn carrier_deck_height_for(&self, obj: &Object) -> f32 {
        let Some(pid) = obj.producer_id else {
            return 0.0;
        };
        self.objects
            .get(&pid)
            .and_then(|p| p.thing.template.parking_place.as_ref())
            .map(|pp| pp.landing_deck_height_offset)
            .unwrap_or(0.0)
    }

    /// C++ PhysicsUpdate.cpp:739-743 groundZ += getCarrierDeckHeight when DECK_HEIGHT_OFFSET.
    pub(crate) fn physics_ground_y_with_deck(&self, obj: &Object, terrain_y: f32) -> f32 {
        if !obj.has_object_status_bit("DECK_HEIGHT_OFFSET") {
            return terrain_y;
        }
        terrain_y + self.carrier_deck_height_for(obj)
    }

    /// C++ PhysicsBehavior::update obj->onCollide(NULL) after landing.
    pub(crate) fn dispatch_physics_ground_collide(&mut self, id: ObjectId) {
        let (container, is_chute) = {
            let Some(o) = self.objects.get_mut(&id) else {
                return;
            };
            o.pending_ground_collide = false;
            o.last_collidee = None;
            let chute =
                o.is_parachuting() || o.template_name.to_ascii_lowercase().contains("parachute");
            (o.contained_by, chute)
        };
        let chute_id = if is_chute {
            Some(id)
        } else {
            container.filter(|cid| {
                self.objects.get(cid).is_some_and(|c| {
                    c.is_parachuting() || c.template_name.to_ascii_lowercase().contains("parachute")
                })
            })
        };
        if let Some(cid) = chute_id {
            // C++ ParachuteContain::onCollide(NULL) land residual.
            self.tick_eject_parachute_residual(cid);
        }
    }

    #[cfg(test)]
    pub fn apply_live_handle_behavior_z_for_test(
        obj: &mut Object,
        ground_y: f32,
        goal_y: Option<f32>,
    ) {
        Self::apply_live_handle_behavior_z(obj, ground_y, goal_y);
    }
    #[cfg(test)]
    pub fn transfer_attack_for_test(&mut self, from_id: ObjectId, to_id: ObjectId) -> usize {
        self.transfer_attack(from_id, to_id)
    }
}
