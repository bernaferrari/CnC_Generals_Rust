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
    ) -> (bool, glam::Vec3, f32) {
        use crate::game_logic::weapon_bootstrap::{
            host_effective_scatter_radius, host_primary_damage_radius_for_weapon_name,
            host_secondary_damage_radius_for_weapon_name, scatter_impact_offset,
            scatter_misses_intended_target, scatter_seed_for_shot, DEFAULT_SCATTER_HIT_RADIUS,
        };
        let (wname, tgt_inf, hit_r, weapon_splash) = {
            let attacker = match self.objects.get(&attacker_id) {
                Some(a) => a,
                None => return (false, target_pos, 0.0),
            };
            let target = match self.objects.get(&target_id) {
                Some(t) => t,
                None => return (false, target_pos, 0.0),
            };
            let wname = if slot == 1 {
                attacker
                    .thing
                    .template
                    .secondary_weapon_name
                    .clone()
                    .or_else(|| attacker.thing.template.primary_weapon_name.clone())
            } else {
                attacker.thing.template.primary_weapon_name.clone()
            };
            let hit_r = if target.selection_radius > 0.0 {
                target.selection_radius
            } else {
                DEFAULT_SCATTER_HIT_RADIUS
            };
            let splash = attacker
                .weapon_slot(slot)
                .map(|w| w.splash_radius.max(0.0))
                .unwrap_or(0.0);
            (wname, target.is_kind_of(KindOf::Infantry), hit_r, splash)
        };
        let Some(name) = wname else {
            return (false, target_pos, weapon_splash);
        };
        let scatter = host_effective_scatter_radius(&name, tgt_inf);
        let primary_r = host_primary_damage_radius_for_weapon_name(&name);
        let secondary_r = host_secondary_damage_radius_for_weapon_name(&name);
        let splash_r = weapon_splash.max(primary_r).max(secondary_r);
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
    /// as DAMAGE_WATER (DEATH_NORMAL). Airborne/aircraft and projectiles skip.
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
            if obj.is_kind_of(KindOf::Aircraft) || obj.is_kind_of(KindOf::Projectile) {
                continue;
            }
            // Naval/hover peels: skip if template suggests boat/ship/amphibious.
            let n = obj.template_name.to_ascii_lowercase();
            if n.contains("boat")
                || n.contains("ship")
                || n.contains("hover")
                || n.contains("amphib")
                || n.contains("carrier")
                || n.contains("destroyer")
                || n.contains("battleship")
            {
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

    /// Refresh underwater/cliff cells; on dry→wet edge apply residual water damage.
    ///
    /// C++ only damages on water-rise events; edge detection approximates that when
    /// terrain water state changes under units (flood scripts / map water).
    pub fn refresh_surface_cells_and_water_edge_damage(&mut self, edge_damage: f32) -> u32 {
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        let mut hit = 0u32;
        let mut destroy: Vec<ObjectId> = Vec::new();
        for id in ids {
            let (pos, was_under) = match self.objects.get(&id) {
                Some(o) => (o.get_position(), o.cell_is_underwater),
                None => continue,
            };
            let (_cliff, water) = self.sample_stun_surface_at(pos);
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            obj.cell_is_cliff = _cliff;
            obj.cell_is_underwater = water;
            let entered = water && !was_under;
            if !entered || !(edge_damage > 0.0) {
                continue;
            }
            if !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            if obj.is_kind_of(KindOf::Aircraft) || obj.is_kind_of(KindOf::Projectile) {
                continue;
            }
            let n = obj.template_name.to_ascii_lowercase();
            if n.contains("boat")
                || n.contains("ship")
                || n.contains("hover")
                || n.contains("amphib")
                || n.contains("carrier")
                || n.contains("destroyer")
                || n.contains("battleship")
            {
                continue;
            }
            let killed = obj.take_damage_from_typed(
                edge_damage,
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
            a_team,
            b_team,
            b_immobile,
            a_infantry,
            b_unmanned,
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
                a.team,
                b.team,
                b.is_kind_of(crate::game_logic::KindOf::Structure)
                    || b.is_kind_of(
                        crate::game_logic::KindOf::Structure, /* immobile residual */
                    )
                    || !b.can_move(),
                a.is_kind_of(crate::game_logic::KindOf::Infantry),
                b.status.disabled_unmanned,
            )
        };
        if a_ignore_b || b_ignore_a {
            return true; // ignore = handled (no bounce)
        }
        // C++ both parachuting: never collide.
        if a_para && b_para {
            return true;
        }
        // C++ PhysicsUpdate infantry→unmanned vehicle pilot residual.
        if a_infantry && b_unmanned {
            if self.try_infantry_unmanned_reclaim(a_id, b_id) {
                if let Some(a) = self.objects.get_mut(&a_id) {
                    a.last_collidee = Some(b_id);
                }
                return true;
            }
        } else {
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
            if b_inf && a_unm {
                if self.try_infantry_unmanned_reclaim(b_id, a_id) {
                    if let Some(a) = self.objects.get_mut(&a_id) {
                        a.last_collidee = Some(b_id);
                    }
                    return true;
                }
            }
        }

        let same_team = a_team == b_team;
        // C++ ToppleUpdate::onCollide residual: crusher_level > 1 topples trees/props.
        if self.try_topple_on_collide(a_id, b_id) || self.try_topple_on_collide(b_id, a_id) {
            if let Some(a) = self.objects.get_mut(&a_id) {
                a.last_collidee = Some(b_id);
            }
            return true;
        }
        // Overlap crush (may handle the pair).
        if self.apply_overlap_crush_check(a_id, b_id, same_team) {
            if let Some(a) = self.objects.get_mut(&a_id) {
                a.last_collidee = Some(b_id);
            }
            return true;
        }
        // Immobile bounce path.
        if b_immobile {
            // Still honor allowCollideForce residual.
            let allow = self
                .objects
                .get(&a_id)
                .map(|a| a.allow_collide_force)
                .unwrap_or(true);
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
            Some(a) => a.ai_process_collision(&b_snap, frame),
            None => return false,
        };
        let (req_away, a_pos) = {
            let Some(a) = self.objects.get_mut(&a_id) else {
                return false;
            };
            a.last_collidee = Some(b_id);
            if a.is_blocked {
                a.apply_blocked_speed_cap();
            }
            let req = a.request_other_move_away.take();
            (req, a.get_position())
        };
        if let Some(other_id) = req_away {
            if let Some(other) = self.objects.get_mut(&other_id) {
                other.ai_move_away_from_unit(a_id, a_pos);
                // Absolute ignore-until frame (2 sec residual if already yielding later).
                if other.ignore_collisions_until_frame > 0
                    && other.ignore_collisions_until_frame < 100_000
                {
                    // relative sentinel → absolute
                    other.ignore_collisions_until_frame = frame.saturating_add(60);
                }
            }
        }
        if !allow_force {
            return true; // AI handled / no force
        }
        // Panic bounce residual: small separation impulse on XZ.
        if let Some(a) = self.objects.get_mut(&a_id) {
            let us = a.get_position();
            let them = b_snap.get_position();
            let mut dx = us.x - them.x;
            let mut dz = us.z - them.z;
            let len = (dx * dx + dz * dz).sqrt().max(1.0);
            dx /= len;
            dz /= len;
            a.movement.velocity.x += dx * 0.5;
            a.movement.velocity.z += dz * 0.5;
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
        let (kill_now, handled) = {
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
            (kill_now, handled)
        };
        if kill_now {
            self.mark_object_for_destruction(prop_id, None);
        }
        handled
    }

    pub fn apply_overlap_crush_check(
        &mut self,
        crusher_id: ObjectId,
        crushee_id: ObjectId,
        same_team: bool,
    ) -> bool {
        // Split borrow: take crushee out, mutate both, put back.
        let Some(mut crushee) = self.objects.remove(&crushee_id) else {
            return false;
        };
        let result = if let Some(crusher) = self.objects.get_mut(&crusher_id) {
            crusher.check_for_overlap_collision(&mut crushee, same_team)
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
            let imm_ok = imm.is_kind_of(crate::game_logic::KindOf::Structure)
                || imm.is_kind_of(
                    crate::game_logic::KindOf::Structure, /* immobile residual */
                )
                || !imm.can_move();
            (m.is_parachuting(), imm.get_position(), imm_ok)
        };
        if !imm_ok {
            return false;
        }
        let mut applied = false;
        if let Some(m) = self.objects.get_mut(&mover_id) {
            if mover_para {
                m.apply_parachute_building_bounce_out(imm_center, us_radius);
                return true;
            }
            if m.status.destroyed {
                return false;
            }
            let _ = m.apply_structure_stiffness_bounce(
                imm_center,
                PHYSICS_STRUCTURE_STIFFNESS_DEFAULT_RESIDUAL,
                crate::game_logic::Object::SHOCK_MASS,
            );
            applied = true;
        }
        if applied {
            // After stiffness bounce, try vehicle crash residual if applicable.
            let _ = self.apply_vehicle_crash_into_immobile(mover_id, immobile_id);
        }
        applied
    }

    pub fn apply_vehicle_crash_into_immobile(
        &mut self,
        vehicle_id: ObjectId,
        other_id: ObjectId,
    ) -> Option<&'static str> {
        use crate::game_logic::host_partition_collision_physics_residual::{
            vehicle_crash_destroys_vehicle, vehicle_crash_weapon_name,
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
        let weapon = vehicle_crash_weapon_name(outcome)?;
        let pos = self
            .objects
            .get(&vehicle_id)
            .map(|o| o.get_position())
            .unwrap_or(glam::Vec3::ZERO);
        // Residual temp weapon: deal explosion damage to vehicle (and mark crash).
        // Fail-closed vs full WeaponStore::createAndFireTempWeapon OCL/FX matrix.
        const CRASH_DAMAGE: f32 = 100.0;
        if let Some(v) = self.objects.get_mut(&vehicle_id) {
            let _ = v.take_damage_from_typed(
                CRASH_DAMAGE,
                Some(vehicle_id),
                crate::game_logic::combat::DamageType::Explosive,
            );
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
            // Also queue crash audio residual.
            self.queue_audio_event(
                AudioEventRequest::new(weapon)
                    .with_object(vehicle_id)
                    .with_position(pos)
                    .with_priority(200),
            );
        } else {
            self.queue_audio_event(
                AudioEventRequest::new(weapon)
                    .with_object(vehicle_id)
                    .with_position(pos)
                    .with_priority(160),
            );
        }
        Some(weapon)
    }

    /// C++ partition collide residual: pairwise near-object physics collide.
    ///
    /// Partition cell broadphase (cell size 40) + selection_radius XZ spheres.
    /// Advances overlap frame after pairs. Fail-closed vs full ghost/shroud cells.
    /// Returns number of pairs that invoked try_physics_collide successfully.

    /// C++ AIUpdateInterface::privateFaceObject residual.
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
        // Host-immediate engagement residual; log for GameWorld last-write.
        u.target = Some(target_id);
        // Face without full path — spin in place residual.
        let _ = u.face_position(target_pos, 1.0 / 30.0);
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
        let _ = u.face_position(pos, 1.0 / 30.0);
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
        // Ground attack residual: any ready weapon is enough.
        let Some(vid) = victim_id else {
            let has = u.weapon.is_some() || u.secondary_weapon.is_some();
            return has;
        };
        let Some(v) = self.objects.get(&vid) else {
            return false;
        };
        // Snapshot selection without holding mut borrow across get_mut.
        let slot = u.select_combat_weapon_slot(v, current_time);
        let Some(slot) = slot else {
            // Not ready this frame — still pick a legal slot that can target
            // (ammo/reload residual may clear next frame).
            let primary_legal = u.weapon.as_ref().is_some_and(|w| u.can_target_with(v, w));
            let secondary_legal = u
                .secondary_weapon
                .as_ref()
                .is_some_and(|w| u.can_target_with(v, w));
            if !primary_legal && !secondary_legal {
                return false;
            }
            let fallback = if primary_legal { 0u8 } else { 1u8 };
            if let Some(uu) = self.objects.get_mut(&unit_id) {
                uu.set_active_weapon_slot(fallback);
            }
            return true;
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

    #[cfg(test)]
    pub fn transfer_attack_for_test(&mut self, from_id: ObjectId, to_id: ObjectId) -> usize {
        self.transfer_attack(from_id, to_id)
    }
}
