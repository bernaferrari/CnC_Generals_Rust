//! Combat/firing/damage leftover impls for Weapon.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::Relationship;
use crate::common::{Coord3D, INVALID_ID};
use crate::common::{KindOf, LocomotorSetType, PathfindLayerEnum};
use crate::common::{Matrix3D, ObjectStatusTypes, TurretType};
use crate::damage::{DamageType, DeathType, HUGE_DAMAGE_AMOUNT};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{
    TheGameLogic, TheTerrainLogic, TheThingFactory, get_game_logic_random_value,
    get_game_logic_random_value_real,
};
use crate::modules::CountermeasuresBehaviorInterface;
use crate::object::collide::GameObject;
use crate::object::drawable::DrawableArcExt;
use crate::object::update::MissileAIUpdateModuleData;
use crate::system::game_logic::TheObjectFactory;
use crate::weapon::projectile_launch_cast::{
    ProjectileLaunchKindMut, module_projectile_launch_kind,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::ini::ini_particle_sys::ParticleSystemTemplate;
use game_engine::common::system::Snapshotable;

use super::helpers::{
    INVALID_OBJECT_ID, ObjectId, ammo_count_for_clip_size, dual_world_registry_unavailable,
    map_weapon_slot_to_common,
};
use super::masks_enums::*;
use super::store::with_weapon_store_mut;
use super::weapon_instance::Weapon;

impl Weapon {
    pub fn handle_projectileless_flight_damage(
        &self,
        source_obj_id: ObjectId,
        source_pos: &Coord3D,
        target_obj_id: Option<ObjectId>,
        target_position: &Coord3D,
        speed: f32,
        bonus: &WeaponBonus,
        inflict_damage: bool,
    ) -> Result<(), WeaponError> {
        if !inflict_damage {
            return Ok(());
        }

        let delay_in_frames = if speed > 0.0 {
            source_pos.distance(*target_position) / speed
        } else {
            0.0
        };

        let (damage_id, damage_position) = if self.template.damage_dealt_at_self_position {
            (INVALID_OBJECT_ID, *source_pos)
        } else {
            (target_obj_id.unwrap_or(INVALID_OBJECT_ID), *target_position)
        };

        if delay_in_frames < 1.0 {
            let victim = (damage_id != INVALID_OBJECT_ID).then_some(damage_id);
            self.deal_damage_internal(source_obj_id, victim, &damage_position, bonus, false)?;
            return Ok(());
        }

        let delay_whole_frames = delay_in_frames.ceil() as u32;
        let when = TheGameLogic::get_frame().saturating_add(delay_whole_frames);

        let queue_result = with_weapon_store_mut(|store| {
            store.set_delayed_damage(
                &self.template,
                &damage_position,
                when,
                source_obj_id,
                damage_id,
                bonus,
            );
        });

        if let Err(err) = queue_result {
            log::warn!(
                "Failed to queue delayed damage for '{}' (source {}, delay {}): {:?}; applying immediately",
                self.template.name,
                source_obj_id,
                delay_whole_frames,
                err
            );
            let victim = (damage_id != INVALID_OBJECT_ID).then_some(damage_id);
            self.deal_damage_internal(source_obj_id, victim, &damage_position, bonus, false)?;
        }

        Ok(())
    }

    /// Calculate scatter for target position.
    ///
    /// C++ Weapon.cpp:953-995 — `scatterRadius = m_scatterRadius`; infantry
    /// adds `m_infantryInaccuracyDist`. No type multipliers. Structures are
    /// re-centered by the caller before this runs.
    pub fn calculate_scatter(
        &self,
        target: Coord3D,
        _distance_to_target: f32,
        target_object_type: ObjectType,
    ) -> Coord3D {
        self.scatter_aim_point(target, target_object_type).0
    }

    /// Returns `(aim_point, rolled_scatter_radius)` after C++ randomization.
    pub(crate) fn scatter_aim_point(
        &self,
        mut target: Coord3D,
        target_object_type: ObjectType,
    ) -> (Coord3D, f32) {
        let mut scatter_radius = self.template.scatter_radius;
        if target_object_type == ObjectType::Infantry
            && self.template.infantry_inaccuracy_dist > 0.0
        {
            scatter_radius += self.template.infantry_inaccuracy_dist;
        }
        if scatter_radius <= 0.0 {
            return (target, 0.0);
        }

        let rolled = self.random_float(0.0, scatter_radius);
        let angle = self.random_float(0.0, std::f32::consts::PI * 2.0);
        target.x += rolled * angle.cos();
        target.y += rolled * angle.sin();
        if let Some(terrain) = TheTerrainLogic::get() {
            target.z = terrain.get_ground_height(target.x, target.y, None);
        }
        (target, rolled)
    }

    /// C++ Weapon.cpp:2584-2609 — one unused scatter index per shot.
    pub(crate) fn take_scatter_target_pos(
        &mut self,
        primary_target_pos: &Coord3D,
    ) -> Option<Coord3D> {
        if self.scatter_targets_unused.is_empty() {
            return None;
        }
        let last = self.scatter_targets_unused.len() as i32 - 1;
        let random_pick = get_game_logic_random_value(0, last) as usize;
        let target_index = self.scatter_targets_unused[random_pick] as usize;
        let scatter_target = self.template.scatter_targets.get(target_index).copied()?;
        let scalar = self.template.get_scatter_target_scalar();
        let mut pos = *primary_target_pos;
        pos.x += scatter_target.x * scalar;
        pos.y += scatter_target.y * scalar;
        if let Some(terrain) = TheTerrainLogic::get() {
            pos.z = terrain.get_ground_height(pos.x, pos.y, None);
        }
        self.scatter_targets_unused.swap_remove(random_pick);
        Some(pos)
    }

    /// C++ Weapon.cpp:2896-2910 processRequestAssistance.
    pub(crate) fn process_request_assistance(
        &self,
        source_obj_id: ObjectId,
        victim_obj_id: ObjectId,
    ) {
        if dual_world_registry_unavailable() {
            return;
        }
        let Some(requesting_arc) = TheGameLogic::find_object_by_id(source_obj_id) else {
            return;
        };
        let Ok(requesting_guard) = requesting_arc.read() else {
            return;
        };
        let Some(player_arc) = requesting_guard.get_controlling_player() else {
            return;
        };
        let template_name = requesting_guard.get_template_name().to_string();
        let range = self.template.get_request_assist_range();
        if range <= 0.0 {
            return;
        }
        let request_dist_sqr = range * range;
        let requesting_pos = *requesting_guard.get_position();
        drop(requesting_guard);

        let Ok(player_guard) = player_arc.read() else {
            return;
        };
        for object_id in player_guard.get_all_objects() {
            if object_id == source_obj_id {
                continue;
            }
            let Some(behaviors) = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.get_template_name() != template_name {
                        return None;
                    }
                    let dx = object_guard.get_position().x - requesting_pos.x;
                    let dy = object_guard.get_position().y - requesting_pos.y;
                    if dx * dx + dy * dy > request_dist_sqr {
                        return None;
                    }
                    Some(object_guard.get_behavior_modules())
                })
                .flatten()
            else {
                continue;
            };
            for behavior in behaviors {
                if let Ok(mut behavior_guard) = behavior.lock() {
                    let Some(assist) = behavior_guard.get_assisted_targeting_update_interface()
                    else {
                        continue;
                    };
                    if assist.is_free_to_assist() {
                        assist.assist_attack(source_obj_id, victim_obj_id);
                    }
                    break;
                }
            }
        }
    }

    pub(crate) fn build_engine_damage_info(
        &self,
        damage_info: &crate::damage::DamageInfo,
    ) -> crate::damage::DamageInfo {
        damage_info.clone()
    }

    /// Deal damage internally (THE CRITICAL BRIDGE TO OBJECT DAMAGE)
    /// C++ Reference: Weapon.cpp lines 1221-1500 (dealDamageInternal)
    ///
    /// # Behavior
    /// - Creates DamageInfo from weapon template
    /// - Handles radius/splash damage
    /// - Handles single-target damage
    /// - Calls object.attempt_damage() for each target
    /// - Returns total damage applied
    pub(crate) fn deal_damage_internal(
        &self,
        source_obj_id: ObjectId,
        target_obj_id: Option<ObjectId>,
        impact_pos: &Coord3D,
        bonus: &WeaponBonus,
        is_projectile_detonation: bool,
    ) -> Result<u32, WeaponError> {
        if source_obj_id == INVALID_OBJECT_ID {
            return Ok(0);
        }

        if !self.template.projectile_name.is_empty() && !is_projectile_detonation {
            return Err(WeaponError::SystemError(
                "Projectile weapons should not call deal_damage_internal directly".to_string(),
            ));
        }

        let source_arc = TheGameLogic::find_object_by_id(source_obj_id);
        let source_guard = source_arc.as_ref().and_then(|arc| arc.read().ok());
        let damage_source_id = self.projectile_damage_source_id(source_obj_id);
        let damage_source_arc = if damage_source_id == source_obj_id {
            source_arc.clone()
        } else {
            TheGameLogic::find_object_by_id(damage_source_id)
        };
        let damage_source_guard = damage_source_arc.as_ref().and_then(|arc| arc.read().ok());

        let mut impact_pos = *impact_pos;
        self.template
            .apply_historic_bonus(source_obj_id, &impact_pos);
        let mut primary_victim_id = None;
        if let Some(target_id) = target_obj_id {
            if let Some(target_arc) = TheGameLogic::find_object_by_id(target_id) {
                if let Ok(target_guard) = target_arc.read() {
                    impact_pos = *target_guard.get_position();
                    primary_victim_id = Some(target_id);
                }
            }
        }

        // Create base damage info
        let mut damage_info = crate::damage::DamageInfo::new();
        damage_info.input.source_id = damage_source_id;
        damage_info.input.damage_type = self.template.damage_type.into();
        damage_info.input.damage_fx_override = self.template.damage_type.into();
        damage_info.input.damage_status_type = self.template.damage_status_type.into();
        damage_info.input.death_type = self.template.death_type.into();
        damage_info.input.amount = self.template.get_primary_damage(bonus);
        damage_info.input.shock_wave_amount = self.template.shock_wave_amount;
        damage_info.input.shock_wave_radius = self.template.shock_wave_radius;
        damage_info.input.shock_wave_taper_off = self.template.shock_wave_taper_off;
        if let Some(source) = damage_source_guard.as_ref() {
            if let Some(player) = source.get_controlling_player() {
                if let Ok(player_guard) = player.read() {
                    damage_info.input.source_player_mask = player_guard.get_player_mask();
                }
            }
        }
        damage_info.sync_from_input();

        let primary_radius = self.template.get_primary_damage_radius(bonus);
        let secondary_radius = self.template.get_secondary_damage_radius(bonus);

        let mut total_damage = 0u32;

        // Determine if this is radius damage
        let max_radius = primary_radius.max(secondary_radius);
        if max_radius > 0.0 {
            // RADIUS DAMAGE - affect multiple targets in area
            let targets = self.find_objects_in_radius(source_obj_id, &impact_pos, max_radius)?;

            for (obj_id, obj_pos, _relationship) in targets {
                let Some(victim_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                    continue;
                };
                let Ok(victim_guard) = victim_arc.read() else {
                    continue;
                };

                let is_primary_victim = primary_victim_id == Some(obj_id);
                let mut kill_self = false;

                if !is_primary_victim {
                    if self
                        .template
                        .affects_mask
                        .contains(WeaponAffectsMask::KILLS_SELF)
                        && obj_id == source_obj_id
                    {
                        kill_self = true;
                    } else {
                        if !self.template.affects_mask.contains(WeaponAffectsMask::SELF) {
                            let producer_id = source_guard
                                .as_ref()
                                .map(|source| source.get_producer_id())
                                .unwrap_or(INVALID_OBJECT_ID);
                            if obj_id == source_obj_id || producer_id == obj_id {
                                continue;
                            }
                        }

                        if self
                            .template
                            .affects_mask
                            .contains(WeaponAffectsMask::DOESNT_AFFECT_SIMILAR)
                        {
                            if let Some(source) = source_guard.as_ref() {
                                let rel = source.relationship_to(&victim_guard);
                                if matches!(rel, Relationship::Allies)
                                    && source
                                        .get_template()
                                        .is_equivalent_to(victim_guard.get_template().as_ref())
                                {
                                    continue;
                                }
                            }
                        }

                        if self
                            .template
                            .affects_mask
                            .contains(WeaponAffectsMask::DOESNT_AFFECT_AIRBORNE)
                            && victim_guard.is_significantly_above_terrain()
                        {
                            continue;
                        }

                        let relationship = source_guard
                            .as_ref()
                            .map(|source| victim_guard.relationship_to(source))
                            .unwrap_or(Relationship::Neutral);
                        let required_mask = match relationship {
                            Relationship::Allies => WeaponAffectsMask::ALLIES,
                            Relationship::Enemies => WeaponAffectsMask::ENEMIES,
                            _ => WeaponAffectsMask::NEUTRALS,
                        };
                        if !self.template.affects_mask.contains(required_mask) {
                            continue;
                        }
                    }
                }

                // Directional radius damage check (cone)
                if self.template.radius_damage_angle < std::f32::consts::PI {
                    let Some(source) = source_guard.as_ref() else {
                        continue;
                    };
                    let source_pos = source.get_position();
                    let dx = obj_pos.x - source_pos.x;
                    let dy = obj_pos.y - source_pos.y;
                    let dz = obj_pos.z - source_pos.z;
                    let len = (dx * dx + dy * dy + dz * dz).sqrt();
                    if len <= f32::EPSILON {
                        continue;
                    }
                    let (fx, fy) = source.get_unit_direction_vector_2d();
                    let fx = fx;
                    let fy = fy;
                    let inv_len = 1.0 / len;
                    let dot = (fx * dx + fy * dy) * inv_len;
                    if dot < self.template.radius_damage_angle.cos() {
                        continue;
                    }
                }

                // Calculate distance and damage falloff
                let distance = impact_pos.distance(obj_pos);
                let damage_amount = self.calculate_radius_damage_falloff(
                    distance,
                    primary_radius,
                    secondary_radius,
                    self.template.get_primary_damage(bonus),
                    self.template.get_secondary_damage(bonus),
                );

                if damage_amount > 0.0 {
                    let mut target_damage_info = damage_info.clone();
                    target_damage_info.input.amount = if kill_self {
                        HUGE_DAMAGE_AMOUNT
                    } else {
                        damage_amount
                    };
                    if self.template.shock_wave_amount > 0.0 {
                        let Some(source) = source_guard.as_ref() else {
                            continue;
                        };
                        let source_pos = source.get_position();
                        let mut shock_wave_vector = Coord3D::new(
                            obj_pos.x - source_pos.x,
                            obj_pos.y - source_pos.y,
                            obj_pos.z - source_pos.z,
                        );
                        if shock_wave_vector.x.abs() < f32::EPSILON
                            && shock_wave_vector.y.abs() < f32::EPSILON
                            && shock_wave_vector.z.abs() < f32::EPSILON
                        {
                            shock_wave_vector.z = 1.0;
                        }
                        target_damage_info.input.shock_wave_vector = shock_wave_vector;
                    }

                    // Apply damage to target
                    if let Ok(actual_damage) =
                        self.apply_damage_to_object(obj_id, &mut target_damage_info)
                    {
                        total_damage += actual_damage as u32;
                    }
                }
            }
        } else {
            // SINGLE TARGET DAMAGE
            if let Some(target_id) = target_obj_id {
                if self
                    .template
                    .affects_mask
                    .contains(WeaponAffectsMask::KILLS_SELF)
                {
                    if let Some(source) = source_guard.as_ref() {
                        let mut self_damage = damage_info.clone();
                        self_damage.input.amount = HUGE_DAMAGE_AMOUNT;
                        if let Ok(actual_damage) =
                            self.apply_damage_to_object(source.get_id(), &mut self_damage)
                        {
                            total_damage = actual_damage as u32;
                        }
                        return Ok(total_damage);
                    }
                }
                if let Ok(actual_damage) = self.apply_damage_to_object(target_id, &mut damage_info)
                {
                    total_damage = actual_damage as u32;
                }
            }
        }

        Ok(total_damage)
    }

    /// Calculate radius damage falloff
    /// C++ Reference: Weapon.cpp (damage calculation logic)
    pub(crate) fn calculate_radius_damage_falloff(
        &self,
        distance: f32,
        primary_radius: f32,
        secondary_radius: f32,
        primary_damage: f32,
        secondary_damage: f32,
    ) -> f32 {
        if distance <= primary_radius {
            // Within primary radius - full damage
            primary_damage
        } else if distance <= secondary_radius {
            // Between primary and secondary - secondary damage
            secondary_damage
        } else {
            // Outside damage radius
            0.0
        }
    }

    /// Compute whether the weapon is aimed at the target position.
    /// Matches C++ turret-aiming logic — checks if facing direction is within `aimDelta` of target.
    /// Contact weapons always return `true`.
    pub fn compute_aim(&self, source_id: ObjectId, target_pos: &Coord3D) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.is_contact_weapon() {
            return true;
        }

        let aim_delta = self.template.aim_delta;
        if aim_delta <= 0.0 {
            return true;
        }

        let Some(ok) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_id, |source_guard| {
                let source_pos = source_guard.get_position();
                let dx = target_pos.x - source_pos.x;
                let dy = target_pos.y - source_pos.y;
                if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
                    return true;
                }
                let angle_to_target = dy.atan2(dx);
                let current_angle = source_guard.get_orientation();

                let mut angle_diff = angle_to_target - current_angle;
                if angle_diff > std::f32::consts::PI {
                    angle_diff -= 2.0 * std::f32::consts::PI;
                } else if angle_diff < -std::f32::consts::PI {
                    angle_diff += 2.0 * std::f32::consts::PI;
                }

                angle_diff.abs() <= aim_delta
            })
        else {
            return false;
        };
        ok
    }

    /// Apply damage to a target object.
    /// Matches C++ `Object::attemptDamage(&DamageInfo)`. Armor/FX/death handled by body module.
    /// Returns actual damage dealt after armor, or 0.0 on failure.
    pub fn deal_damage(
        &self,
        target_id: ObjectId,
        amount: f32,
        damage_type: crate::damage::DamageType,
        source_id: Option<ObjectId>,
    ) -> f32 {
        // Wave 265: empty dual-world → zero.
        if dual_world_registry_unavailable() {
            return 0.0;
        }

        let mut damage_info = crate::damage::DamageInfo::with_simple(
            amount,
            source_id.unwrap_or(INVALID_OBJECT_ID),
            damage_type,
            crate::damage::DeathType::Normal,
        );

        if let Some(src_id) = source_id {
            if let Some(mask) =
                crate::object::registry::OBJECT_REGISTRY.with_object(src_id, |src_guard| {
                    src_guard
                        .get_controlling_player()
                        .and_then(|player| player.read().ok().map(|p| p.get_player_mask()))
                })
            {
                if let Some(mask) = mask {
                    damage_info.input.source_player_mask = mask;
                }
            }
        }
        damage_info.sync_from_input();

        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(target_id, |target_guard| {
                if target_guard.is_destroyed() {
                    return 0.0;
                }
                match target_guard.attempt_damage_with_return(&mut damage_info) {
                    Ok(actual) => actual,
                    Err(_) => 0.0,
                }
            })
            .unwrap_or(0.0)
    }

    /// Enable or disable a weapon bonus condition on the owning object.
    /// Matches C++ `Object::setWeaponBonusCondition(WeaponBonusConditionType, Bool)`.
    pub fn set_weapon_bonus_condition(
        &self,
        source_id: ObjectId,
        condition: crate::common::types::WeaponBonusConditionType,
        enabled: bool,
    ) {
        // Wave 265: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ =
            crate::object::registry::OBJECT_REGISTRY.with_object_mut(source_id, |source_guard| {
                if enabled {
                    source_guard.set_weapon_bonus_condition(condition);
                } else {
                    source_guard.clear_weapon_bonus_condition(condition);
                }

                for slot_idx in 0..crate::common::WEAPONSLOT_COUNT {
                    let slot = match slot_idx {
                        0 => WeaponSlotType::Primary,
                        1 => WeaponSlotType::Secondary,
                        _ => WeaponSlotType::Tertiary,
                    };
                    if let Some(weapon) = source_guard.get_weapon_in_slot_mut(slot) {
                        let _ = weapon.on_weapon_bonus_change(source_id);
                    }
                }
            });
    }

    /// Update weapon state per frame
    /// C++ Reference: Weapon.cpp update logic
    ///
    /// # Behavior
    /// - Decrements cooldown timers
    /// - Transitions weapon status when ready
    /// - Handles continuous firing state
    pub fn update(&mut self, _delta_time: f32, current_frame: u32) -> Result<(), WeaponError> {
        // Check if cooldown has expired
        if current_frame >= self.when_we_can_fire_again {
            match self.status {
                WeaponStatus::BetweenFiringShots => {
                    self.status = WeaponStatus::ReadyToFire;
                }
                WeaponStatus::ReloadingClip => {
                    // C++ refills in reloadWithBonus (Weapon.cpp:1884-1886).
                    // Keep a late refill only when ammo is still empty so
                    // start-of-reload unlimited clips (0x7fffffff) are not
                    // overwritten with clip_size as u32 (0).
                    if self.ammo_in_clip == 0 {
                        self.ammo_in_clip = ammo_count_for_clip_size(self.template.clip_size);
                    }
                    self.status = WeaponStatus::ReadyToFire;
                }
                _ => {}
            }
        }

        if self.status == WeaponStatus::PreAttack && current_frame >= self.when_pre_attack_finished
        {
            // C++ Weapon::getStatus (Weapon.cpp:2743-2748): Ready only with ammo.
            if self.ammo_in_clip > 0 {
                self.status = WeaponStatus::ReadyToFire;
            } else {
                self.status = WeaponStatus::OutOfAmmo;
            }
        }

        Ok(())
    }

    /// Get object position (interfaces with object manager)
    ///
    /// Matches C++ Object->getPosition() calls throughout Weapon.cpp
    pub(crate) fn get_object_position(&self, _obj_id: ObjectId) -> Result<Coord3D, WeaponError> {
        let Some(obj_arc) = TheGameLogic::find_object_by_id(_obj_id) else {
            return Err(WeaponError::InvalidTarget);
        };
        let obj_guard = obj_arc
            .read()
            .map_err(|_| WeaponError::SystemError("Failed to lock object position".to_string()))?;
        Ok(*obj_guard.get_position())
    }

    /// Get object type (interfaces with object manager)
    ///
    /// Used for scatter calculation - infantry get more inaccuracy
    pub(crate) fn get_object_type(&self, _obj_id: ObjectId) -> ObjectType {
        let Some(obj_arc) = TheGameLogic::find_object_by_id(_obj_id) else {
            return ObjectType::Unknown;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return ObjectType::Unknown;
        };

        if obj_guard.is_kind_of(KindOf::Projectile) {
            return ObjectType::Projectile;
        }
        if obj_guard.is_kind_of(KindOf::Structure) || obj_guard.is_kind_of(KindOf::Building) {
            return ObjectType::Structure;
        }
        if obj_guard.is_kind_of(KindOf::Infantry) {
            return ObjectType::Infantry;
        }
        if obj_guard.is_kind_of(KindOf::Vehicle) || obj_guard.is_kind_of(KindOf::Aircraft) {
            return ObjectType::Vehicle;
        }

        ObjectType::Unknown
    }

    /// Check if target is valid and alive
    ///
    /// Validates target still exists and hasn't been destroyed
    pub(crate) fn is_target_valid(&self, _obj_id: ObjectId) -> bool {
        let Some(obj_arc) = TheGameLogic::find_object_by_id(_obj_id) else {
            return false;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return false;
        };
        !obj_guard.is_destroyed()
    }

    /// Check if source can see target within vision range
    ///
    /// # Behavior
    /// - Gets vision range from source object
    /// - Calculates distance to target
    /// - Returns true if target is within vision range
    /// - Returns false if target is beyond vision range
    pub(crate) fn can_see_target(&self, source_id: ObjectId, target_id: ObjectId) -> bool {
        use crate::object_manager::get_object_manager;

        let object_manager = get_object_manager();
        let Ok(obj_mgr) = object_manager.read() else {
            return false; // Can't see if we can't access objects
        };

        // Borrow-first: extract pose/vision under the manager lock (no Arc clone).
        let Some((source_pos, vision_range)) = obj_mgr.with_object(source_id, |src| {
            let pos = src.get_position().clone();
            let vision = src
                .base()
                .read()
                .map(|base| base.get_vision_range())
                .unwrap_or(0.0);
            (pos, vision)
        }) else {
            return false; // Source not found / lock poisoned
        };
        let Some(target_pos) = obj_mgr.with_object(target_id, |tgt| tgt.get_position().clone())
        else {
            return false; // Target not found / lock poisoned
        };

        // Calculate distance and check if within vision range
        let distance = source_pos.distance(target_pos);
        distance <= vision_range
    }

    /// Check line-of-sight between two positions
    /// For direct-fire weapons that can't fire through obstacles
    ///
    /// # Implementation
    /// - Checks height differences are within weapon capability
    /// - Basic terrain height validation
    /// - Full raycast through obstacles would be next enhancement
    #[cfg(test)]
    pub(crate) fn check_line_of_sight(&self, from: &Coord3D, to: &Coord3D) -> bool {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return true;
        };
        guard.is_clear_line_of_sight(from, to)
    }

    /// Check if target is an enemy (not on same team/alliance)
    /// Weapons can only fire on enemies, not on friendlies
    ///
    /// # Behavior
    /// - Gets teams for both source and target objects
    /// - Uses Team.get_relationship() to check team attitudes
    /// - Returns true if target is an Enemy (not Ally, Friend, or self)
    /// - Returns false for friendlies and self
    pub(crate) fn is_enemy_target(&self, source_obj_id: ObjectId, target_obj_id: ObjectId) -> bool {
        use crate::common::Relationship;
        use crate::object_manager::get_object_manager;

        // Early exit: can't be enemy to self
        if source_obj_id == target_obj_id {
            return false;
        }

        let object_manager = get_object_manager();
        let Ok(obj_mgr) = object_manager.read() else {
            return true; // If lock fails, assume enemy (safe fallback)
        };

        // Borrow-first: clone team Arcs under manager lock (no object Arc clone).
        let Some(source_team) = obj_mgr.with_object(source_obj_id, |src| src.get_team()) else {
            return true; // Source not found, assume enemy
        };
        let Some(target_team) = obj_mgr.with_object(target_obj_id, |tgt| tgt.get_team()) else {
            return true; // Target not found, assume enemy
        };
        drop(obj_mgr);

        // Check team relationship
        match (source_team, target_team) {
            (Some(source_team_lock), Some(target_team_lock)) => {
                // Both have teams, check relationship
                if let (Ok(source_t), Ok(target_t)) =
                    (source_team_lock.read(), target_team_lock.read())
                {
                    let relationship = source_t.get_relationship(&target_t);
                    // Only enemies can be targeted; not allies, friends, or self
                    matches!(relationship, Relationship::Enemies)
                } else {
                    true // Lock error, assume enemy
                }
            }
            (None, None) => {
                // Neither has a team - treat as enemies (can fire on neutral objects)
                true
            }
            (Some(_source_team_lock), None) => {
                // Target has no team but source does - assume neutral, can fire
                true
            }
            (None, Some(_target_team_lock)) => {
                // Source has no team but target does - assume neutral, can fire
                true
            }
        }
    }

    /// Determine fire mode based on weapon template
    pub(crate) fn determine_fire_mode(&self) -> FireMode {
        if self.template.is_contact_weapon() {
            // Contact weapon - instant impact
            FireMode::InstantImpact {
                splash_radius: self.template.primary_damage_radius,
            }
        } else if !self.template.laser_name.is_empty() {
            // Laser weapon - continuous beam
            FireMode::ContinuousBeam {
                duration: 1.0,
                damage_per_frame: self.template.primary_damage / LOGICFRAMES_PER_SECOND as f32,
            }
        } else {
            // Projectile weapon
            let speed = self
                .template
                .weapon_speed
                .max(self.template.min_weapon_speed);
            let lifetime = if speed > 0.0 {
                (self.template.attack_range / speed).max(0.0)
            } else {
                0.0
            };
            FireMode::Projectile { speed, lifetime }
        }
    }

    /// Create projectile object
    pub(crate) fn create_projectile(
        &self,
        source_obj_id: ObjectId,
        source_pos: &Coord3D,
        target_pos: &Coord3D,
        target_obj_id: Option<ObjectId>,
        speed: f32,
        lifetime: f32,
        bonus: &WeaponBonus,
    ) -> Result<ObjectId, WeaponError> {
        // PARITY_NOTE: C++ projectile creation uses simple distance/speed for flight time,
        // not a fabricated ballistics calculator. The trajectory variable is unused here
        // because C++ creates projectiles as Object instances with DumbProjectileBehavior/MissileAIUpdate.
        let _flight_time = if speed > 0.1 {
            source_pos.distance(*target_pos) / speed
        } else {
            lifetime
        };

        log::debug!(
            "Creating projectile '{}' from {:?} to {:?}",
            self.template.projectile_name,
            source_pos,
            target_pos
        );

        if let Some(projectile_template) =
            TheObjectFactory::find_template(&self.template.projectile_name)
        {
            let mut owning_player = None;
            let mut projectile_team = None;
            let mut source_veterancy = crate::common::VeterancyLevel::Regular;

            if let Some(source_arc) = TheGameLogic::find_object_by_id(source_obj_id) {
                if let Ok(source_guard) = source_arc.read() {
                    owning_player = source_guard.get_controlling_player();
                    source_veterancy = source_guard.get_veterancy_level();
                    if let Some(player_arc) = &owning_player {
                        if let Ok(player_guard) = player_arc.read() {
                            projectile_team = player_guard.get_default_team();
                        }
                    }
                    if projectile_team.is_none() {
                        projectile_team = source_guard.get_team();
                    }
                }
            }

            let projectile_arc = TheObjectFactory::new_object(
                projectile_template,
                projectile_team.as_ref().map(Arc::clone),
            )
            .map_err(|e| WeaponError::SystemError(format!("Projectile create failed: {}", e)))?;

            let projectile_id = projectile_arc
                .read()
                .map_err(|_| WeaponError::SystemError("Projectile lock failed".to_string()))?
                .get_id();

            {
                let mut proj_guard = projectile_arc
                    .write()
                    .map_err(|_| WeaponError::SystemError("Projectile lock failed".to_string()))?;
                let _ = proj_guard.set_position(source_pos);

                if let Some(source_arc) = TheGameLogic::find_object_by_id(source_obj_id) {
                    if let Ok(source_guard) = source_arc.read() {
                        proj_guard.set_producer(Some(&source_guard));
                        if source_guard.notify_special_power_completion_die() {
                            proj_guard.set_special_power_completion_creator(INVALID_OBJECT_ID);
                        } else {
                            proj_guard.set_special_power_completion_creator(source_obj_id);
                        }
                    }
                }
            }

            if let Some(player_arc) = owning_player {
                if let Ok(player_guard) = player_arc.read() {
                    if player_guard.get_num_battle_plans_active() > 0 {
                        if let Ok(mut proj_guard) = projectile_arc.write() {
                            player_guard.apply_battle_plan_bonuses_for_object(&mut proj_guard);
                        }
                    }
                }
            }

            let exhaust = self.template.get_projectile_exhaust(source_veterancy);

            let weapon_template = Arc::clone(&self.template);
            let mut launched = false;
            if let Ok(mut proj_guard) = projectile_arc.write() {
                let modules = proj_guard.behavior_modules();
                drop(proj_guard);

                for module in modules {
                    let mut did_launch = false;
                    module.with_module(|behavior| {
                        let Some(projectile_behavior) = module_projectile_launch_kind(behavior)
                        else {
                            return;
                        };

                        match projectile_behavior {
                            ProjectileLaunchKindMut::MissileAIUpdateBehavior(missile) => {
                                missile.projectile_launch_at_object_or_position(
                                    target_obj_id,
                                    target_pos,
                                    Some(source_obj_id),
                                    self.weapon_slot,
                                    self.current_barrel,
                                    Some(Arc::downgrade(&weapon_template)),
                                    exhaust.clone(),
                                );
                                did_launch = true;
                            }
                            ProjectileLaunchKindMut::NeutronMissileUpdate(neutron) => {
                                if let Some(launcher_arc) =
                                    TheGameLogic::find_object_by_id(source_obj_id)
                                {
                                    if let Ok(launcher_guard) = launcher_arc.read() {
                                        if let Some(victim_id) = target_obj_id {
                                            if let Some(victim_arc) =
                                                TheGameLogic::find_object_by_id(victim_id)
                                            {
                                                if let Ok(victim_guard) = victim_arc.read() {
                                                    neutron
                                                        .projectile_launch_at_object_or_position(
                                                            Some(&victim_guard),
                                                            Some(target_pos),
                                                            Some(&launcher_guard),
                                                            map_weapon_slot_to_common(
                                                                self.weapon_slot,
                                                            ),
                                                            self.current_barrel,
                                                            Some(&weapon_template),
                                                            None,
                                                        );
                                                    did_launch = true;
                                                }
                                            }
                                        } else {
                                            neutron.projectile_launch_at_object_or_position(
                                                None,
                                                Some(target_pos),
                                                Some(&launcher_guard),
                                                map_weapon_slot_to_common(self.weapon_slot),
                                                self.current_barrel,
                                                Some(&weapon_template),
                                                None,
                                            );
                                            did_launch = true;
                                        }
                                    }
                                }
                            }
                            ProjectileLaunchKindMut::DumbProjectileBehavior(dumb) => {
                                dumb.projectile_launch_at_object_or_position(
                                    target_obj_id,
                                    target_pos,
                                    source_obj_id,
                                    self.weapon_slot,
                                    self.current_barrel,
                                    Some(Arc::clone(&weapon_template)),
                                );
                                did_launch = true;
                            }
                        }
                    });

                    if did_launch {
                        launched = true;
                        break;
                    }
                }
            }

            if !launched {
                if let Ok(mut proj_guard) = projectile_arc.write() {
                    let _ = proj_guard.set_position(target_pos);
                }
            }
            report_missile_for_countermeasures(projectile_id, target_obj_id);

            return Ok(projectile_id);
        }

        Err(WeaponError::SystemError(format!(
            "Projectile template '{}' not found",
            self.template.projectile_name
        )))
    }

    pub(crate) fn create_laser_object(
        &self,
        source_obj_id: ObjectId,
        target_obj_id: Option<ObjectId>,
        target_pos: &Coord3D,
        damage_per_frame: f32,
        duration: f32,
    ) -> Result<Option<ObjectId>, WeaponError> {
        if self.template.laser_name.is_empty() {
            return Ok(None);
        }

        let Some(source_arc) = TheGameLogic::find_object_by_id(source_obj_id) else {
            return Err(WeaponError::InvalidTarget);
        };
        let (team_arc, source_pos) = {
            let source_guard = source_arc
                .read()
                .map_err(|_| WeaponError::SystemError("Source object lock failed".to_string()))?;
            let team_arc = source_guard
                .get_controlling_player()
                .and_then(|player| {
                    player
                        .read()
                        .ok()
                        .and_then(|guard| guard.get_default_team())
                })
                .or_else(|| source_guard.get_team());
            let Some(team_arc) = team_arc else {
                return Err(WeaponError::SystemError(
                    "Laser creation requires source player default team".to_string(),
                ));
            };
            (team_arc, *source_guard.get_position())
        };

        let team_guard = team_arc
            .read()
            .map_err(|_| WeaponError::SystemError("Source team lock failed".to_string()))?;

        let Some(template) =
            crate::helpers::TheThingFactory::find_template(&self.template.laser_name)
        else {
            return Err(WeaponError::SystemError(format!(
                "Laser template '{}' not found",
                self.template.laser_name
            )));
        };

        let factory = crate::helpers::TheThingFactory::get()
            .map_err(|e| WeaponError::SystemError(e.to_string()))?;
        let laser_obj = factory
            .new_object(template, &team_guard)
            .map_err(|e| WeaponError::SystemError(e.to_string()))?;

        let mut laser_guard = laser_obj
            .write()
            .map_err(|_| WeaponError::SystemError("Laser object lock failed".to_string()))?;
        let mut end_pos = if let Some(target_id) = target_obj_id {
            TheGameLogic::find_object_by_id(target_id)
                .and_then(|arc| arc.read().ok().map(|guard| *guard.get_position()))
                .unwrap_or(*target_pos)
        } else {
            *target_pos
        };
        if let Some(target_id) = target_obj_id {
            let raise = TheGameLogic::find_object_by_id(target_id)
                .and_then(|arc| {
                    arc.read().ok().map(|guard| {
                        !guard.is_kind_of(KindOf::Projectile) && !guard.is_airborne_target()
                    })
                })
                .unwrap_or(false);
            if raise {
                end_pos.z += 10.0;
            }
        }
        let _ = laser_guard.set_position(&source_pos);
        let laser_id = laser_guard.get_id();

        let _ = (damage_per_frame, duration);

        let client_modules = laser_guard.client_update_modules();
        drop(laser_guard);

        let source_guard = source_arc
            .read()
            .map_err(|_| WeaponError::SystemError("Source object lock failed".to_string()))?;
        let target_arc = target_obj_id.and_then(TheGameLogic::find_object_by_id);
        let target_guard = match target_arc.as_ref() {
            Some(arc) => arc.read().ok(),
            None => None,
        };
        let target_ref = target_guard.as_deref();

        for module in client_modules {
            module.with_module(|module| {
                if let Some(laser_update) = module.get_laser_update_interface() {
                    laser_update.init_laser(
                        Some(source_guard.get_id()),
                        target_ref.map(|target| target.get_id()),
                        Some(source_pos.to_array()),
                        Some(end_pos.to_array()),
                        self.template.laser_bone_name.clone(),
                        0,
                    );
                }
            });
        }

        Ok(Some(laser_id))
    }

    /// Fire weapon effects (sound, VFX).
    ///
    /// C++ Weapon.cpp:899-950 — detonation uses projectile detonate FX/OCL;
    /// SuspendFXDelay nulls FX; undetected stealth skips muzzle FX/sound.
    pub(crate) fn fire_weapon_effects(
        &self,
        source_obj_id: ObjectId,
        source_pos: &Coord3D,
        impact_pos: &Coord3D,
        is_projectile_detonation: bool,
    ) -> Result<(), WeaponError> {
        let current_frame = TheGameLogic::get_frame();
        let fx_suspended = current_frame < self.suspend_fx_frame;

        let (veterancy, skip_muzzle_fx, play_sound) = crate::object::registry::OBJECT_REGISTRY
            .with_object(source_obj_id, |source| {
                let stealthed_hidden = !source.is_locally_controlled()
                    && source.test_status(ObjectStatusTypes::Stealthed)
                    && !source.test_status(ObjectStatusTypes::Detected)
                    && !source.test_status(ObjectStatusTypes::Disguised)
                    && !source.is_kind_of(KindOf::Mine)
                    && !self.template.play_fx_when_stealthed;
                (
                    source.get_veterancy_level(),
                    stealthed_hidden || fx_suspended,
                    !stealthed_hidden,
                )
            })
            .unwrap_or((crate::common::VeterancyLevel::Regular, fx_suspended, true));

        if play_sound && !self.template.fire_sound.is_empty() {
            log::debug!("Playing fire sound for weapon '{}'", self.template.name);
            game_engine::common::audio::dispatch_weapon_fire(
                self.template.fire_sound.name(),
                source_pos.x,
                source_pos.y,
                source_pos.z,
            );
        }

        let fx_pos = if is_projectile_detonation {
            impact_pos
        } else {
            source_pos
        };

        if !skip_muzzle_fx {
            let fx = if is_projectile_detonation {
                self.template.get_projectile_detonate_fx(veterancy)
            } else {
                self.template.get_fire_fx(veterancy)
            };
            if let Some(fx_list) = fx {
                let _ = fx_list.do_fx_at_position(fx_pos);
            }
        }

        let ocl = if is_projectile_detonation {
            self.template.get_projectile_detonation_ocl(veterancy)
        } else {
            self.template.get_fire_ocl(veterancy)
        };
        if let Some(ocl) = ocl {
            let _ = ocl.create_at_position(fx_pos, source_obj_id);
        }

        Ok(())
    }

    pub(crate) fn projectile_damage_source_id(&self, source_obj_id: ObjectId) -> ObjectId {
        let Some(source_arc) = TheGameLogic::find_object_by_id(source_obj_id) else {
            return source_obj_id;
        };
        let Ok(source) = source_arc.read() else {
            return source_obj_id;
        };
        if !source.is_kind_of(KindOf::Projectile) {
            return source_obj_id;
        }

        for behavior in source.get_behavior_modules() {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            let Some(projectile) = guard.get_projectile_update_interface() else {
                continue;
            };
            let launcher_id = projectile.projectile_get_launcher_id();
            if launcher_id != INVALID_ID {
                return launcher_id;
            }
        }

        source_obj_id
    }

    /// Find objects in radius - queries spatial partition for objects in blast radius
    /// Returns (object_id, position, relationship_flags) for all objects in area
    pub(crate) fn find_objects_in_radius(
        &self,
        source_obj_id: ObjectId,
        center: &Coord3D,
        radius: f32,
    ) -> Result<Vec<(ObjectId, Coord3D, u32)>, WeaponError> {
        // Wave 265: empty dual-world → Ok(empty).
        if dual_world_registry_unavailable() {
            return Ok(Vec::new());
        }

        use crate::object::registry::OBJECT_REGISTRY;
        use crate::object_manager::get_object_manager;

        // Get the global object manager
        let object_manager = get_object_manager();
        let obj_mgr = object_manager.read().map_err(|e| {
            WeaponError::SystemError(format!("Failed to access object manager: {}", e))
        })?;

        // Query spatial partition for objects in radius
        let object_ids = obj_mgr.find_objects_in_radius(*center, radius);
        drop(obj_mgr);

        let mut results = Vec::new();
        for obj_id in object_ids {
            // Nested borrow-first access keeps neither source nor target Arc at the call site.
            let Some((pos, relationship_mask)) = OBJECT_REGISTRY
                .with_object(source_obj_id, |source| {
                    OBJECT_REGISTRY.with_object(obj_id, |obj| {
                        let pos = *obj.get_position();
                        let relationship_mask = match source.relationship_to(obj) {
                            Relationship::Allies => WeaponAffectsMask::ALLIES,
                            Relationship::Enemies => WeaponAffectsMask::ENEMIES,
                            _ => WeaponAffectsMask::NEUTRALS,
                        };
                        (pos, relationship_mask)
                    })
                })
                .flatten()
            else {
                continue;
            };
            results.push((obj_id, pos, relationship_mask));
        }

        Ok(results)
    }

    /// Apply damage to a specific object - THE CRITICAL CONNECTION to Object system
    /// Gets object from ObjectManager and calls attempt_damage() to apply actual damage
    pub(crate) fn apply_damage_to_object(
        &self,
        obj_id: ObjectId,
        damage_info: &mut crate::damage::DamageInfo,
    ) -> Result<f32, WeaponError> {
        use crate::object_manager::get_object_manager;

        log::debug!(
            "Applying {} damage (type: {:?}) to object {}",
            damage_info.input.amount,
            damage_info.input.damage_type,
            obj_id
        );

        // Get the global object manager
        let object_manager = get_object_manager();
        let obj_mgr = object_manager.read().map_err(|e| {
            WeaponError::SystemError(format!("Failed to read object manager: {}", e))
        })?;

        // Borrow-first mutable access (no Arc clone).
        let dealt = obj_mgr
            .with_object_mut(obj_id, |obj| -> Result<f32, WeaponError> {
                let mut engine_damage_info = self.build_engine_damage_info(damage_info);
                let __base_arc = obj.base();
                if let Ok(mut base) = __base_arc.write() {
                    base.attempt_damage(&mut engine_damage_info).map_err(|e| {
                        WeaponError::SystemError(format!("Failed to apply damage: {}", e))
                    })?;
                } else {
                    return Err(WeaponError::SystemError(
                        "Failed to acquire base object lock".to_string(),
                    ));
                }
                damage_info.output.actual_damage_dealt =
                    engine_damage_info.output.actual_damage_dealt;
                damage_info.output.actual_damage_clipped =
                    engine_damage_info.output.actual_damage_clipped;
                damage_info.output.no_effect = engine_damage_info.output.no_effect;
                Ok(engine_damage_info.output.actual_damage_dealt)
            })
            .ok_or(WeaponError::InvalidTarget)??;

        Ok(dealt)
    }

    /// Random float generator using synchronized game-logic RNG.
    pub(crate) fn random_float(&self, min: f32, max: f32) -> f32 {
        get_game_logic_random_value_real(min, max)
    }
}

/// C++ Weapon.cpp:1144-1155 — SMALL_MISSILE vs non-supersonic countermeasure jets.
fn report_missile_for_countermeasures(projectile_id: ObjectId, victim_id: Option<ObjectId>) {
    let Some(victim_id) = victim_id else {
        return;
    };
    let Some(proj_arc) = TheGameLogic::find_object_by_id(projectile_id) else {
        return;
    };
    let Ok(proj_guard) = proj_arc.read() else {
        return;
    };
    if !proj_guard.is_kind_of(KindOf::SmallMissile) {
        return;
    }
    drop(proj_guard);

    let Some(victim_arc) = TheGameLogic::find_object_by_id(victim_id) else {
        return;
    };
    let Ok(victim_guard) = victim_arc.read() else {
        return;
    };
    if !victim_guard.has_countermeasures() {
        return;
    }
    let supersonic = victim_guard
        .get_ai()
        .and_then(|ai| {
            ai.lock().ok().map(|ai_guard| {
                ai_guard.get_cur_locomotor_set_type() == LocomotorSetType::Supersonic
            })
        })
        .unwrap_or(false);
    if supersonic {
        return;
    }
    victim_guard.report_missile_for_countermeasures(projectile_id);
}
