//! Canonical leftover Weapon instance extracted from weapon/mod.rs.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::Coord3D;
use crate::common::Relationship;
use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::{KindOf, PathfindLayerEnum};
use crate::common::{Matrix3D, TurretType};
use crate::common::{ObjectID, Real, UnsignedInt, Xfer, XferMode, XferVersion, INVALID_ID};
use crate::damage::{DamageType, DeathType};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{
    get_game_logic_random_value, get_game_logic_random_value_real, TheGameLogic, TheTerrainLogic,
    TheThingFactory,
};
use crate::modules::CountermeasuresBehaviorInterface;
use crate::object::collide::GameObject;
use crate::object::drawable::DrawableArcExt;
use crate::object::update::MissileAIUpdateModuleData;
use crate::system::game_logic::TheObjectFactory;
use crate::weapon::projectile_launch_cast::{
    module_projectile_launch_kind, ProjectileLaunchKindMut,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::ini::ini_particle_sys::ParticleSystemTemplate;
use game_engine::common::system::Snapshotable;

use super::audio_event::AudioEventRts;
use super::helpers::{
    ammo_count_for_clip_size, dual_world_registry_unavailable, map_common_bonus_flags,
    map_weapon_slot_to_common, ObjectId, INVALID_OBJECT_ID, NO_MAX_SHOTS_LIMIT,
};
use super::masks_enums::*;
use super::template::WeaponTemplate;

#[derive(Debug)]
pub struct Weapon {
    /// Template defining weapon properties
    pub(crate) template: Arc<WeaponTemplate>,

    /// Weapon slot type
    pub(crate) weapon_slot: WeaponSlotType,

    /// Current weapon status
    pub(crate) status: WeaponStatus,

    /// Ammunition in current clip
    pub(crate) ammo_in_clip: u32,

    /// Frame when weapon can fire again
    pub(crate) when_we_can_fire_again: u32,

    /// Frame when pre-attack will finish
    pub(crate) when_pre_attack_finished: u32,

    /// Frame when last reload started
    pub(crate) when_last_reload_started: u32,

    /// Frame when weapon was last fired
    pub(crate) last_fire_frame: u32,

    /// Frame when FX will be unsuspended
    pub(crate) suspend_fx_frame: u32,

    /// Projectile stream object ID
    pub(crate) projectile_stream_id: ObjectId,

    /// Maximum shot count limit
    pub(crate) max_shot_count: i32,

    /// Current barrel being used for firing
    pub(crate) current_barrel: i32,

    /// Number of shots fired from current barrel
    pub(crate) num_shots_for_current_barrel: i32,

    /// Unused scatter targets tracking
    pub(crate) scatter_targets_unused: Vec<i32>,

    /// Whether weapon is pitch limited
    pub(crate) pitch_limited: bool,

    /// Whether leech range is currently active
    pub(crate) leech_weapon_range_active: bool,
}

impl Weapon {
    pub fn new(template: Arc<WeaponTemplate>, weapon_slot: WeaponSlotType) -> Self {
        let min_pitch = template.min_target_pitch;
        let max_pitch = template.max_target_pitch;
        let shots_per_barrel = template.shots_per_barrel;
        let pitch_limited = min_pitch > -std::f32::consts::PI || max_pitch < std::f32::consts::PI;
        let suspend_fx_frame =
            TheGameLogic::get_frame().saturating_add(template.suspend_fx_delay as UnsignedInt);

        Self {
            template,
            weapon_slot,
            status: WeaponStatus::OutOfAmmo,
            ammo_in_clip: 0,
            when_we_can_fire_again: 0,
            when_pre_attack_finished: 0,
            when_last_reload_started: 0,
            last_fire_frame: 0,
            suspend_fx_frame,
            projectile_stream_id: INVALID_OBJECT_ID,
            max_shot_count: NO_MAX_SHOTS_LIMIT,
            current_barrel: 0,
            num_shots_for_current_barrel: shots_per_barrel,
            scatter_targets_unused: Vec::new(),
            pitch_limited,
            leech_weapon_range_active: false,
        }
    }

    pub fn is_within_target_pitch(&self, source_obj: ObjectId, target_obj: ObjectId) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.is_contact_weapon() || !self.pitch_limited {
            return true;
        }

        let Some((src_pos, src_geom)) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source_guard| {
                (
                    *source_guard.get_position(),
                    source_guard.get_geometry_info().clone(),
                )
            })
        else {
            return true;
        };
        let Some((dst_pos, dst_geom)) =
            crate::object::registry::OBJECT_REGISTRY.with_object(target_obj, |target_guard| {
                (
                    *target_guard.get_position(),
                    target_guard.get_geometry_info().clone(),
                )
            })
        else {
            return true;
        };

        const ACCEPTABLE_DZ: Real = 10.0;
        if (dst_pos.z - src_pos.z).abs() < ACCEPTABLE_DZ {
            return true;
        }

        let (min_pitch, max_pitch) = src_geom.calc_pitches(&src_pos, &dst_geom, &dst_pos);

        let min_target = self.template.min_target_pitch;
        let max_target = self.template.max_target_pitch;

        (min_pitch >= min_target && min_pitch <= max_target)
            || (max_pitch >= min_target && max_pitch <= max_target)
            || (min_pitch <= min_target && max_pitch >= max_target)
    }

    /// Fire weapon at target object
    pub fn fire_weapon_at_object(
        &mut self,
        source: ObjectId,
        target: ObjectId,
    ) -> Result<(), WeaponError> {
        self.fire(source, target)
    }

    /// Fire weapon at position
    pub fn fire_weapon_at_position(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
    ) -> Result<(), WeaponError> {
        self.fire_at_position(source, position)
    }

    pub fn fire_weapon_at_position_with_bonus(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
        source_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        container_bonus_flags: Option<crate::common::types::WeaponBonusConditionFlags>,
    ) -> Result<(), WeaponError> {
        self.fire_weapon_at_position_with_bonus_and_reload_flag(
            source,
            position,
            source_bonus_flags,
            container_bonus_flags,
        )?;
        Ok(())
    }

    /// Fire weapon at position with bonus integration and report whether the clip completed.
    pub fn fire_weapon_at_position_with_bonus_and_reload_flag(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
        source_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        container_bonus_flags: Option<crate::common::types::WeaponBonusConditionFlags>,
    ) -> Result<bool, WeaponError> {
        let current_frame = TheGameLogic::get_frame();
        self.check_can_fire(source, None, Some(position), current_frame)?;

        let mut combined_flags = source_bonus_flags;
        if let Some(container_flags) = container_bonus_flags {
            combined_flags |= container_flags;
        }
        let bonus = self.compute_bonus(source, map_common_bonus_flags(combined_flags));
        self.private_fire_weapon(source, None, Some(position), &bonus, false, false, true)?;
        Ok(self.apply_post_fire_state(current_frame, &bonus))
    }

    /// Fire projectile detonation weapon
    pub fn fire_projectile_detonation_weapon(
        &mut self,
        source: ObjectId,
        target: Option<ObjectId>,
        position: Option<&Coord3D>,
        extra_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        inflict_damage: bool,
    ) -> Result<(), WeaponError> {
        let internal_flags = map_common_bonus_flags(extra_bonus_flags);
        let bonus = self.compute_bonus(source, internal_flags);
        self.fire_projectile_detonation_weapon_with_bonus(
            source,
            target,
            position,
            &bonus,
            inflict_damage,
        )
    }

    /// Fire projectile detonation weapon using a precomputed bonus snapshot.
    pub fn fire_projectile_detonation_weapon_with_bonus(
        &mut self,
        source: ObjectId,
        target: Option<ObjectId>,
        position: Option<&Coord3D>,
        bonus: &WeaponBonus,
        inflict_damage: bool,
    ) -> Result<(), WeaponError> {
        self.private_fire_weapon(source, target, position, bonus, true, false, inflict_damage)
    }

    /// Fire weapon with full bonus integration
    /// Matches C++ Object.cpp fireCurrentWeapon which passes source and container bonus flags
    pub fn fire_weapon(
        &mut self,
        source_id: ObjectId,
        target_id: ObjectId,
        current_frame: u32,
        source_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        container_bonus_flags: Option<crate::common::types::WeaponBonusConditionFlags>,
    ) -> Result<(), WeaponError> {
        self.fire_weapon_with_bonus_and_reload_flag(
            source_id,
            target_id,
            current_frame,
            source_bonus_flags,
            container_bonus_flags,
        )?;
        Ok(())
    }

    /// Fire weapon with full bonus integration and report whether the clip completed.
    pub fn fire_weapon_with_bonus_and_reload_flag(
        &mut self,
        source_id: ObjectId,
        target_id: ObjectId,
        current_frame: u32,
        source_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        container_bonus_flags: Option<crate::common::types::WeaponBonusConditionFlags>,
    ) -> Result<bool, WeaponError> {
        self.check_can_fire(source_id, Some(target_id), None, current_frame)?;

        // Combine source and container bonus flags
        let mut combined_flags = source_bonus_flags;
        if let Some(container_flags) = container_bonus_flags {
            combined_flags |= container_flags;
        }

        // Convert to internal WeaponBonusConditionFlags type
        let internal_flags = map_common_bonus_flags(combined_flags);

        let bonus = self.compute_bonus(source_id, internal_flags);
        self.private_fire_weapon(source_id, Some(target_id), None, &bonus, false, false, true)?;
        Ok(self.apply_post_fire_state(current_frame, &bonus))
    }

    pub(crate) fn apply_post_fire_state(&mut self, current_frame: u32, bonus: &WeaponBonus) -> bool {
        let delay = self.template.get_delay_between_shots(bonus);
        self.when_we_can_fire_again = current_frame + (delay as u32);
        self.last_fire_frame = current_frame;
        self.status = WeaponStatus::BetweenFiringShots;

        if self.ammo_in_clip > 0 {
            self.ammo_in_clip -= 1;
        }

        if self.ammo_in_clip == 0 {
            if self.template.get_auto_reloads_clip() {
                let reload_time = self.template.get_clip_reload_time(bonus);
                self.when_we_can_fire_again = current_frame + (reload_time as u32);
                self.status = WeaponStatus::ReloadingClip;
            } else {
                self.status = WeaponStatus::OutOfAmmo;
            }
            return true;
        }

        false
    }

    /// Pre-fire weapon (for weapons with pre-attack delay)
    pub fn pre_fire_weapon(&mut self, _source: ObjectId, _victim: ObjectId) -> GameLogicResult<()> {
        let bonus = self.compute_bonus(_source, WeaponBonusConditionFlags::new());
        let delay = self.template.get_pre_attack_delay(&bonus);
        if delay > 0 {
            self.status = WeaponStatus::PreAttack;
            self.when_pre_attack_finished = TheGameLogic::get_frame() + (delay as u32);
            if self.template.leech_range_weapon {
                self.leech_weapon_range_active = true;
            }
        }
        Ok(())
    }

    /// Force fire weapon and return projectile object
    pub fn force_fire_weapon(
        &mut self,
        source: ObjectId,
        position: &Coord3D,
    ) -> GameLogicResult<Option<ObjectId>> {
        let current_frame = TheGameLogic::get_frame();
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());

        self.private_fire_weapon(source, None, Some(position), &bonus, false, true, true)?;

        let delay = self.template.get_delay_between_shots(&bonus);
        self.when_we_can_fire_again = current_frame + (delay as u32);
        self.last_fire_frame = current_frame;
        self.status = WeaponStatus::BetweenFiringShots;

        if self.ammo_in_clip > 0 {
            self.ammo_in_clip -= 1;
        }

        if self.ammo_in_clip == 0 {
            if self.template.get_auto_reloads_clip() {
                let reload_time = self.template.get_clip_reload_time(&bonus);
                self.when_we_can_fire_again = current_frame + (reload_time as u32);
                self.status = WeaponStatus::ReloadingClip;
            } else {
                self.status = WeaponStatus::OutOfAmmo;
            }
        }

        Ok(None)
    }

    /// Estimate weapon damage against target
    pub fn estimate_weapon_damage(
        &self,
        source_obj: ObjectId,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> f32 {
        let bonus = self.compute_bonus(source_obj, WeaponBonusConditionFlags::new());
        self.template.estimate_weapon_template_damage(
            source_obj as crate::common::ObjectID,
            target_obj.map(|id| id as crate::common::ObjectID),
            target_pos,
            &bonus,
        )
    }

    /// Check if target is within attack range
    pub fn is_within_attack_range(
        &self,
        source_obj: ObjectId,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let Some(source_pos) = crate::object::registry::OBJECT_REGISTRY
            .with_object(source_obj, |guard| *guard.get_position())
        else {
            return false;
        };

        let target_pos = if let Some(pos) = target_pos {
            *pos
        } else if let Some(target_id) = target_obj {
            let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_id, |guard| *guard.get_position())
            else {
                return false;
            };
            pos
        } else {
            return false;
        };

        let bonus = self.compute_bonus(source_obj, WeaponBonusConditionFlags::new());
        let max_range = self.template.get_attack_range(&bonus);
        let min_range = self.template.get_minimum_attack_range();

        let dx = source_pos.x - target_pos.x;
        let dy = source_pos.y - target_pos.y;
        let dist_sqr = dx * dx + dy * dy;
        let max_range_sqr = max_range * max_range;
        let min_range_sqr = min_range * min_range;

        dist_sqr <= max_range_sqr && dist_sqr >= min_range_sqr
    }

    /// Check if target is too close
    pub fn is_too_close(
        &self,
        source_obj: ObjectId,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let Some(source_pos) = crate::object::registry::OBJECT_REGISTRY
            .with_object(source_obj, |guard| *guard.get_position())
        else {
            return false;
        };

        let target_pos = if let Some(pos) = target_pos {
            *pos
        } else if let Some(target_id) = target_obj {
            let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_id, |guard| *guard.get_position())
            else {
                return false;
            };
            pos
        } else {
            return false;
        };

        let min_range = self.template.get_minimum_attack_range();
        let dx = source_pos.x - target_pos.x;
        let dy = source_pos.y - target_pos.y;
        let dist_sqr = dx * dx + dy * dy;
        let min_range_sqr = min_range * min_range;

        dist_sqr < min_range_sqr
    }

    /// Get the attack distance including object bounding radii.
    ///
    /// Matches C++ Weapon::getAttackDistance() from Weapon.cpp line 2352.
    /// Returns `getAttackRange(source)` plus the bounding circle radii of both
    /// the source and victim objects (2D path via ATTACK_RANGE_IS_2D).
    pub fn get_attack_distance(&self, source_obj: ObjectId, victim_obj: Option<ObjectId>) -> f32 {
        // Wave 265: empty dual-world → zero.
        if dual_world_registry_unavailable() {
            return 0.0;
        }

        let mut range = self.get_attack_range(source_obj);

        if let Some(victim_id) = victim_obj {
            let source_radius = crate::object::registry::OBJECT_REGISTRY
                .with_object(source_obj, |guard| {
                    guard.get_geometry_info().get_bounding_circle_radius()
                })
                .unwrap_or(0.0);

            let victim_radius = crate::object::registry::OBJECT_REGISTRY
                .with_object(victim_id, |guard| {
                    guard.get_geometry_info().get_bounding_circle_radius()
                })
                .unwrap_or(0.0);

            range += source_radius + victim_radius;
        }

        range
    }

    /// Check if the source object's goal position is within attack range of the target.
    ///
    /// Matches C++ Weapon::isSourceObjectWithGoalPositionWithinAttackRange() from
    /// Weapon.cpp line 2110. Uses 2D distance squared comparison with the same
    /// fudge factor and min-range guard as the C++ original.
    ///
    /// C++ uses `FROM_BOUNDINGSPHERE_2D`, which measures 2D boundary distance
    /// after subtracting source/target bounding circle radii.
    pub fn is_source_object_with_goal_position_within_attack_range(
        &self,
        source_obj: ObjectId,
        goal_pos: &Coord3D,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let source_radius = crate::object::registry::OBJECT_REGISTRY
            .with_object(source_obj, |guard| {
                guard.get_geometry_info().get_bounding_circle_radius()
            })
            .unwrap_or(0.0);

        let (tgt_pos, target_radius) = if let Some(target_id) = target_obj {
            let Some(pair) =
                crate::object::registry::OBJECT_REGISTRY.with_object(target_id, |target_guard| {
                    (
                        *target_guard.get_position(),
                        target_guard
                            .get_geometry_info()
                            .get_bounding_circle_radius(),
                    )
                })
            else {
                return false;
            };
            pair
        } else if let Some(pos) = target_pos {
            (*pos, 0.0)
        } else {
            return false;
        };

        let dx = goal_pos.x - tgt_pos.x;
        let dy = goal_pos.y - tgt_pos.y;
        let center_dist = (dx * dx + dy * dy).sqrt();
        let boundary_dist = (center_dist - source_radius - target_radius).max(0.0);
        let dist_sqr = boundary_dist * boundary_dist;

        let attack_range = self.get_attack_range(source_obj);
        let min_attack_range = self.template.get_minimum_attack_range();

        // C++ line 2134: if (distSqr < minAttackRangeSqr - 0.5f) return false;
        if dist_sqr < min_attack_range * min_attack_range - 0.5 {
            return false;
        }

        dist_sqr <= attack_range * attack_range
    }

    /// Load ammo instantly (for newly created units)
    pub fn load_ammo_now(&mut self, _source: ObjectId) -> GameLogicResult<()> {
        self.ammo_in_clip = ammo_count_for_clip_size(self.template.clip_size);
        self.status = WeaponStatus::ReadyToFire;
        Ok(())
    }

    /// Reload ammo with delay
    pub fn reload_ammo(&mut self, source: ObjectId) -> GameLogicResult<()> {
        self.reload_with_bonus(source, &WeaponBonus::new(), false)
    }

    /// Reload with bonus and optional instant load
    pub(crate) fn reload_with_bonus(
        &mut self,
        source: ObjectId,
        bonus: &WeaponBonus,
        load_instantly: bool,
    ) -> GameLogicResult<()> {
        self.ammo_in_clip = ammo_count_for_clip_size(self.template.clip_size);
        if load_instantly {
            self.status = WeaponStatus::ReadyToFire;
        } else {
            self.status = WeaponStatus::ReloadingClip;
            self.when_last_reload_started = TheGameLogic::get_frame();
            let reload_time = self.template.get_clip_reload_time(bonus);
            self.when_we_can_fire_again = self.when_last_reload_started + (reload_time as u32);
        }
        Ok(())
    }

    /// Get weapon status
    pub fn get_status(&self) -> WeaponStatus {
        let current_frame = TheGameLogic::get_frame();

        if self.status == WeaponStatus::PreAttack {
            if current_frame < self.when_pre_attack_finished {
                return WeaponStatus::PreAttack;
            }
        }

        if current_frame >= self.when_we_can_fire_again {
            if self.ammo_in_clip > 0 || self.template.clip_size <= 0 {
                return WeaponStatus::ReadyToFire;
            }
            return WeaponStatus::OutOfAmmo;
        }

        self.status
    }

    /// Get remaining ammunition
    pub fn get_remaining_ammo(&self) -> u32 {
        match self.status {
            WeaponStatus::ReloadingClip => 0,
            _ => self.ammo_in_clip,
        }
    }

    /// Get percent ready to fire (0.0 to 1.0)
    pub fn get_percent_ready_to_fire(&self) -> f32 {
        match self.status {
            WeaponStatus::ReadyToFire => 1.0,
            WeaponStatus::OutOfAmmo => 0.0,
            WeaponStatus::PreAttack => 0.5, // Pre-attack is halfway ready
            WeaponStatus::BetweenFiringShots | WeaponStatus::ReloadingClip => {
                // Calculate based on remaining time
                let current_frame = TheGameLogic::get_frame();
                if current_frame >= self.when_we_can_fire_again {
                    1.0
                } else {
                    let total_time = self.when_we_can_fire_again - self.when_last_reload_started;
                    let elapsed_time = current_frame - self.when_last_reload_started;
                    if total_time > 0 {
                        (elapsed_time as f32) / (total_time as f32)
                    } else {
                        0.0
                    }
                }
            }
        }
    }

    /// Get attack range for this weapon
    pub fn get_attack_range(&self, source: ObjectId) -> f32 {
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());
        self.template.get_attack_range(&bonus)
    }

    /// Get minimum attack range for this weapon.
    pub fn get_minimum_attack_range(&self) -> f32 {
        self.template.get_minimum_attack_range()
    }

    /// Aim delta in radians (matches C++ Weapon::getAimDelta).
    pub fn get_aim_delta(&self) -> f32 {
        self.template.aim_delta
    }

    /// Get clip reload time for this weapon
    pub fn get_clip_reload_time(&self, source: ObjectId) -> i32 {
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());
        self.template.get_clip_reload_time(&bonus)
    }

    /// Get primary damage radius for this weapon
    pub fn get_primary_damage_radius(&self, source: ObjectId) -> f32 {
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());
        self.template.get_primary_damage_radius(&bonus)
    }

    /// Get pre-attack delay for this weapon
    pub fn get_pre_attack_delay(&self, source: ObjectId, _victim: ObjectId) -> i32 {
        let bonus = self.compute_bonus(source, WeaponBonusConditionFlags::new());
        self.template.get_pre_attack_delay(&bonus)
    }

    /// Check if this is a damage weapon
    pub fn is_damage_weapon(&self) -> bool {
        self.template.primary_damage > 0.0 || self.template.secondary_damage > 0.0
    }

    /// Check if this is a contact weapon (requires collision with target)
    pub fn is_contact_weapon(&self) -> bool {
        self.template.is_contact_weapon()
    }

    /// Get the damage type for this weapon
    pub fn get_damage_type(&self) -> DamageType {
        self.template.get_damage_type()
    }

    /// Check if weapon is pitch limited
    pub fn is_pitch_limited(&self) -> bool {
        self.pitch_limited
    }

    /// Set leech range active state
    pub fn set_leech_range_active(&mut self, active: bool) {
        self.leech_weapon_range_active = active;
    }

    /// Check if leech range is active
    pub fn has_leech_range(&self) -> bool {
        self.leech_weapon_range_active
    }

    /// Set the frame when the weapon can fire next (matches C++ Weapon::setPossibleNextShotFrame)
    pub fn set_possible_next_shot_frame(&mut self, frame: u32) {
        self.when_we_can_fire_again = frame;
    }

    /// Set weapon status directly (matches C++ Weapon::setStatus)
    pub fn set_status(&mut self, status: WeaponStatus) {
        self.status = status;
    }

    /// Set maximum shot count
    pub fn set_max_shot_count(&mut self, max_shots: i32) {
        self.max_shot_count = max_shots;
    }

    /// Get maximum shot count
    pub fn get_max_shot_count(&self) -> i32 {
        self.max_shot_count
    }

    /// Set clip percent full
    pub fn set_clip_percent_full(&mut self, percent: f32, allow_reduction: bool) {
        let new_ammo = ((self.template.clip_size as f32) * percent.clamp(0.0, 1.0)) as u32;

        if allow_reduction || new_ammo >= self.ammo_in_clip {
            self.ammo_in_clip = new_ammo;
            self.status = if new_ammo > 0 {
                WeaponStatus::ReadyToFire
            } else {
                WeaponStatus::OutOfAmmo
            };
        }
    }

    /// Transfer next shot stats from another weapon
    pub fn transfer_next_shot_stats_from(&mut self, other: &Weapon) {
        self.when_we_can_fire_again = other.when_we_can_fire_again;
        self.when_pre_attack_finished = other.when_pre_attack_finished;
        self.when_last_reload_started = other.when_last_reload_started;
    }

    /// Update weapon on bonus change
    pub fn on_weapon_bonus_change(&mut self, _source: ObjectId) -> GameLogicResult<()> {
        // Implementation would recalculate timing based on new bonuses
        Ok(())
    }

    /// Compute weapon bonus
    pub(crate) fn compute_bonus(
        &self,
        source: ObjectId,
        extra_bonus_flags: WeaponBonusConditionFlags,
    ) -> WeaponBonus {
        let mut bonus = WeaponBonus::new();

        // Apply extra bonuses from template
        if let Some(extra_bonus_set) = &self.template.extra_bonus {
            extra_bonus_set.append_bonuses(extra_bonus_flags, &mut bonus);
        }

        // Additional bonus computation would be based on source object state

        bonus
    }

    /// Get weapon template
    pub fn get_template(&self) -> &Arc<WeaponTemplate> {
        &self.template
    }

    /// Get weapon slot
    pub fn get_weapon_slot(&self) -> WeaponSlotType {
        self.weapon_slot
    }

    /// Notify weapon stream systems that a new projectile was fired (matches C++ Weapon::newProjectileFired).
    pub fn new_projectile_fired(
        &mut self,
        source_obj_id: ObjectId,
        projectile_id: ObjectId,
        victim_obj: Option<ObjectId>,
        victim_pos: Option<&Coord3D>,
    ) {
        let stream_name = self.template.projectile_stream_name.trim();
        if stream_name.is_empty() {
            return;
        }

        let mut stream_arc = if self.projectile_stream_id != INVALID_OBJECT_ID {
            TheGameLogic::find_object_by_id(self.projectile_stream_id)
        } else {
            None
        };

        if stream_arc.is_none() {
            self.projectile_stream_id = INVALID_OBJECT_ID;

            let Some(source_arc) = TheGameLogic::find_object_by_id(source_obj_id) else {
                return;
            };
            let Ok(source_guard) = source_arc.read() else {
                return;
            };
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
                return;
            };
            let Ok(team_guard) = team_arc.read() else {
                return;
            };
            let Some(template) = TheThingFactory::find_template(stream_name) else {
                return;
            };
            let factory = match TheThingFactory::get() {
                Ok(factory) => factory,
                Err(_) => return,
            };
            let stream_obj = match factory.new_object(template, &team_guard) {
                Ok(obj) => obj,
                Err(_) => return,
            };

            self.projectile_stream_id = stream_obj
                .read()
                .ok()
                .map(|guard| guard.get_id())
                .unwrap_or(INVALID_OBJECT_ID);
            stream_arc = Some(stream_obj);
        }

        let Some(stream_arc) = stream_arc else {
            return;
        };
        let Ok(mut stream_guard) = stream_arc.write() else {
            return;
        };
        for behavior in stream_guard.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
                continue;
            };
            let Some(stream_update) = behavior.get_projectile_stream_update_interface() else {
                continue;
            };
            if let Some(source_arc) = TheGameLogic::find_object_by_id(source_obj_id) {
                if let Ok(source_guard) = source_arc.read() {
                    let pos = *source_guard.get_position();
                    stream_update.set_position(&pos);
                }
            }
            stream_update.add_projectile(
                source_obj_id,
                projectile_id,
                victim_obj.unwrap_or(INVALID_OBJECT_ID),
                victim_pos,
            );
            break;
        }
    }

    /// Get weapon name
    pub fn get_name(&self) -> &str {
        &self.template.name
    }

    /// Get last shot frame
    pub fn get_last_shot_frame(&self) -> u32 {
        self.last_fire_frame
    }

    /// Get possible next shot frame
    pub fn get_possible_next_shot_frame(&self) -> u32 {
        self.when_we_can_fire_again
    }

    /// Get pre-attack finished frame
    pub fn get_pre_attack_finished_frame(&self) -> u32 {
        self.when_pre_attack_finished
    }

    pub fn set_pre_attack_finished_frame(&mut self, frame: u32) {
        self.when_pre_attack_finished = frame;
    }

    /// Get last reload started frame
    pub fn get_last_reload_started_frame(&self) -> u32 {
        self.when_last_reload_started
    }

    /// Get suspend FX frame
    pub fn get_suspend_fx_frame(&self) -> u32 {
        self.suspend_fx_frame
    }

    pub fn get_continue_attack_range(&self) -> crate::common::Real {
        self.template.continue_attack_range
    }

    pub fn get_lock_on_range(&self) -> crate::common::Real {
        self.template.continue_attack_range
    }

    pub fn get_anti_mask(&self) -> u32 {
        self.template.get_anti_mask()
    }

    /// Get the scatter radius for this weapon.
    /// C++ Reference: Weapon.h line 670
    pub fn get_scatter_radius(&self) -> f32 {
        self.template.scatter_radius
    }

    /// Get whether this weapon is capable of following waypoint paths.
    /// C++ Reference: Weapon.h line 673
    pub fn is_capable_of_following_waypoint(&self) -> bool {
        self.template.capable_of_following_waypoint
    }

    /// Get the continuous fire coast frames.
    /// C++ Reference: Weapon.h line 676
    pub fn get_continuous_fire_coast_frames(&self) -> u32 {
        self.template.continuous_fire_coast_frames
    }

    /// Get the auto-reload when idle frames.
    /// C++ Reference: Weapon.h line 677
    pub fn get_auto_reload_when_idle_frames(&self) -> u32 {
        self.template.auto_reload_when_idle_frames
    }

    /// Get the reload type for this weapon.
    /// C++ Reference: Weapon.h line 667
    pub fn get_reload_type(&self) -> WeaponReloadType {
        self.template.reload_type
    }

    /// Get the death type for this weapon.
    /// C++ Reference: Weapon.h line 681
    pub fn get_death_type(&self) -> DeathType {
        self.template.death_type
    }

    /// Set the last reload started frame.
    /// C++ Reference: Weapon.h line 642
    pub fn set_last_reload_started_frame(&mut self, frame: u32) {
        self.when_last_reload_started = frame;
    }

    /// Get the fire sound for this weapon.
    /// C++ Reference: Weapon.h line 678 (inline getFireSound)
    pub fn get_fire_sound(&self) -> &AudioEventRts {
        &self.template.fire_sound
    }

    /// Get the fire sound loop time.
    /// C++ Reference: Weapon.h line 679
    pub fn get_fire_sound_loop_time(&self) -> u32 {
        self.template.fire_sound_loop_time
    }

    /// Check if there is clear terrain line of sight from source to victim object.
    /// C++ Reference: Weapon.cpp line 3066 (isClearFiringLineOfSightTerrain)
    ///
    /// Adjusts source position upward by geometry height (eye level), then
    /// checks terrain LOS to the victim's geometry center.
    pub fn is_clear_firing_line_of_sight_terrain(
        &self,
        source_obj: ObjectId,
        target_obj: ObjectId,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        use crate::object::collide::Coord3D as CollideCoord;

        let Some(origin) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source_guard| {
                let source_pos = source_guard.get_position();
                let source_height = source_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                CollideCoord::new(source_pos.x, source_pos.y, source_pos.z + source_height)
            })
        else {
            return true;
        };

        let Some(victim_pos) =
            crate::object::registry::OBJECT_REGISTRY.with_object(target_obj, |target_guard| {
                let target_pos = target_guard.get_position();
                let target_height = target_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                CollideCoord::new(target_pos.x, target_pos.y, target_pos.z + target_height)
            })
        else {
            return true;
        };

        crate::object::collide::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
            None,
            &origin,
            None,
            &victim_pos,
        )
    }

    /// Check if there is clear terrain LOS from source to a specific position.
    /// C++ Reference: Weapon.cpp line 3083
    pub fn is_clear_firing_line_of_sight_terrain_pos(
        &self,
        source_obj: ObjectId,
        victim_pos: &Coord3D,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        use crate::object::collide::Coord3D as CollideCoord;

        let Some(origin) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source_guard| {
                let source_pos = source_guard.get_position();
                let source_height = source_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                CollideCoord::new(source_pos.x, source_pos.y, source_pos.z + source_height)
            })
        else {
            return true;
        };

        let victim = CollideCoord::new(victim_pos.x, victim_pos.y, victim_pos.z);

        crate::object::collide::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
            None, &origin, None, &victim,
        )
    }

    /// Check if source at goal position would have clear terrain LOS to victim object.
    /// C++ Reference: Weapon.cpp line 3096 (isClearGoalFiringLineOfSightTerrain)
    ///
    /// Used by AI to pre-check if moving to a goal position would give clear shot.
    pub fn is_clear_goal_firing_line_of_sight_terrain(
        &self,
        source_obj: ObjectId,
        goal_pos: &Coord3D,
        target_obj: ObjectId,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        use crate::object::collide::Coord3D as CollideCoord;

        let Some(origin) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source_guard| {
                let source_height = source_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                CollideCoord::new(goal_pos.x, goal_pos.y, goal_pos.z + source_height)
            })
        else {
            return true;
        };

        let Some(victim_pos) =
            crate::object::registry::OBJECT_REGISTRY.with_object(target_obj, |target_guard| {
                let target_pos = target_guard.get_position();
                let target_height = target_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                CollideCoord::new(target_pos.x, target_pos.y, target_pos.z + target_height)
            })
        else {
            return true;
        };

        crate::object::collide::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
            None,
            &origin,
            None,
            &victim_pos,
        )
    }

    /// Check if source at goal position would have clear terrain LOS to a position.
    /// C++ Reference: Weapon.cpp line 3110
    pub fn is_clear_goal_firing_line_of_sight_terrain_pos(
        &self,
        source_obj: ObjectId,
        goal_pos: &Coord3D,
        victim_pos: &Coord3D,
    ) -> bool {
        // Wave 265: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        use crate::object::collide::Coord3D as CollideCoord;

        let Some(origin) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source_guard| {
                let source_height = source_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                CollideCoord::new(goal_pos.x, goal_pos.y, goal_pos.z + source_height)
            })
        else {
            return true;
        };

        let victim = CollideCoord::new(victim_pos.x, victim_pos.y, victim_pos.z);

        crate::object::collide::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
            None, &origin, None, &victim,
        )
    }

    /// Get clip reload time for this weapon with source object context.
    /// C++ Reference: Weapon.h line 688
    pub fn get_clip_reload_time_obj(&self, source_obj: ObjectId) -> i32 {
        let bonus = self.compute_bonus(source_obj, WeaponBonusConditionFlags::new());
        self.template.get_clip_reload_time(&bonus)
    }

    /// Get pre-attack delay for a specific target.
    /// C++ Reference: Weapon.h line 692
    pub fn get_pre_attack_delay_obj(&self, source_obj: ObjectId) -> i32 {
        let bonus = self.compute_bonus(source_obj, WeaponBonusConditionFlags::new());
        self.template.get_pre_attack_delay(&bonus)
    }

    /// Get primary damage radius for this weapon with source object context.
    /// C++ Reference: Weapon.h line 690
    pub fn get_primary_damage_radius_obj(&self, source_obj: ObjectId) -> f32 {
        let bonus = self.compute_bonus(source_obj, WeaponBonusConditionFlags::new());
        self.template.get_primary_damage_radius(&bonus)
    }

    // ========================================================================
    // CRITICAL WEAPON FIRING METHODS
    // ========================================================================
    // C++ Reference: Weapon.cpp lines 1400-1600

    /// Fire weapon at object target
    /// C++ Reference: Weapon.cpp lines 1400-1450 (main firing entry point)
    ///
    /// # Behavior
    /// - Checks ammunition and weapon status
    /// - Validates range and line-of-sight
    /// - Fires weapon via private_fire_weapon()
    /// - Updates cooldown and ammo counters
    /// - Returns success or specific error
    pub fn fire(
        &mut self,
        source_obj_id: ObjectId,
        target_obj_id: ObjectId,
    ) -> Result<(), WeaponError> {
        // Get current frame from game logic
        let current_frame = TheGameLogic::get_frame();

        // Check if we can fire
        self.check_can_fire(source_obj_id, Some(target_obj_id), None, current_frame)?;

        // Fire the weapon through private implementation
        let bonus = self.compute_bonus(source_obj_id, WeaponBonusConditionFlags::new());

        // Call private fire weapon
        self.private_fire_weapon(
            source_obj_id,
            Some(target_obj_id),
            None,
            &bonus,
            false, // not projectile detonation
            false, // don't ignore ranges
            true,  // inflict damage
        )?;

        // Update weapon state after firing
        let delay = self.template.get_delay_between_shots(&bonus);
        self.when_we_can_fire_again = current_frame + (delay as u32);
        self.last_fire_frame = current_frame;
        self.status = WeaponStatus::BetweenFiringShots;

        // Decrement ammunition
        if self.ammo_in_clip > 0 {
            self.ammo_in_clip -= 1;
        }

        // Check if we need to reload
        if self.ammo_in_clip == 0 {
            if self.template.get_auto_reloads_clip() {
                let reload_time = self.template.get_clip_reload_time(&bonus);
                self.when_we_can_fire_again = current_frame + (reload_time as u32);
                self.status = WeaponStatus::ReloadingClip;
            } else {
                self.status = WeaponStatus::OutOfAmmo;
            }
        }

        Ok(())
    }

    /// Fire weapon at position target
    pub fn fire_at_position(
        &mut self,
        source_obj_id: ObjectId,
        target_pos: &Coord3D,
    ) -> Result<(), WeaponError> {
        let current_frame = TheGameLogic::get_frame();

        // Check if we can fire
        self.check_can_fire(source_obj_id, None, Some(target_pos), current_frame)?;

        let bonus = self.compute_bonus(source_obj_id, WeaponBonusConditionFlags::new());

        // Call private fire weapon
        self.private_fire_weapon(
            source_obj_id,
            None,
            Some(target_pos),
            &bonus,
            false,
            false,
            true,
        )?;

        // Update weapon state
        let delay = self.template.get_delay_between_shots(&bonus);
        self.when_we_can_fire_again = current_frame + (delay as u32);
        self.last_fire_frame = current_frame;
        self.status = WeaponStatus::BetweenFiringShots;

        if self.ammo_in_clip > 0 {
            self.ammo_in_clip -= 1;
        }

        if self.ammo_in_clip == 0 {
            if self.template.get_auto_reloads_clip() {
                let reload_time = self.template.get_clip_reload_time(&bonus);
                self.when_we_can_fire_again = current_frame + (reload_time as u32);
                self.status = WeaponStatus::ReloadingClip;
            } else {
                self.status = WeaponStatus::OutOfAmmo;
            }
        }

        Ok(())
    }

    /// Check if weapon can fire at target
    /// C++ Reference: Weapon.cpp various checks scattered throughout
    ///
    /// # Validates
    /// - Ammunition available
    /// - Weapon is ready (not on cooldown)
    /// - Target is in range
    /// - Line of sight is clear
    /// - Target is valid and alive
    pub fn check_can_fire(
        &self,
        source_obj_id: ObjectId,
        target_obj_id: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
        current_frame: u32,
    ) -> Result<(), WeaponError> {
        // Check ammunition - prevent firing if no ammo remaining
        if self.ammo_in_clip == 0 {
            return Err(WeaponError::NoAmmo);
        }

        // Check weapon status (cooldown)
        if self.status != WeaponStatus::ReadyToFire && current_frame < self.when_we_can_fire_again {
            let frames_remaining = self.when_we_can_fire_again - current_frame;
            let time_remaining = (frames_remaining as f32) / LOGICFRAMES_PER_SECOND as f32;
            return Err(WeaponError::NotReady { time_remaining });
        }

        // Get source and target positions (object manager integration)
        let source_pos = self.get_object_position(source_obj_id)?;
        let target_position = if let Some(target_id) = target_obj_id {
            self.get_object_position(target_id)?
        } else if let Some(pos) = target_pos {
            *pos
        } else {
            return Err(WeaponError::InvalidTarget);
        };

        // Check range
        let distance = source_pos.distance(target_position);
        let bonus = self.compute_bonus(source_obj_id, WeaponBonusConditionFlags::new());
        let max_range = self.template.get_attack_range(&bonus);
        let min_range = self.template.get_minimum_attack_range();

        if distance > max_range {
            return Err(WeaponError::OutOfRange {
                distance,
                max_range,
            });
        }

        if distance < min_range {
            return Err(WeaponError::OutOfRange {
                distance,
                max_range: min_range,
            });
        }

        // Check line of sight (if weapon requires it)
        if self.template.must_travel_pfx || !self.template.capable_of_following_waypoint {
            let clear_line_of_sight = if let Some(target_id) = target_obj_id {
                self.is_clear_firing_line_of_sight_terrain(source_obj_id, target_id)
            } else {
                self.is_clear_firing_line_of_sight_terrain_pos(source_obj_id, &target_position)
            };
            if !clear_line_of_sight {
                return Err(WeaponError::TargetObstructed);
            }
        }

        // Check if target object is valid and alive
        if let Some(target_id) = target_obj_id {
            if !self.is_target_valid(target_id) {
                return Err(WeaponError::InvalidTarget);
            }

            // Check vision range - source must be able to see target
            if !self.can_see_target(source_obj_id, target_id) {
                return Err(WeaponError::TargetNotVisible);
            }

            // Check team relationships - can't fire on friendlies
            if !self.is_enemy_target(source_obj_id, target_id) {
                return Err(WeaponError::InvalidTarget); // Can't target friendlies
            }
        }

        Ok(())
    }

    /// Private weapon firing implementation
    /// C++ Reference: Weapon.cpp lines 1475-1550 (privateFireWeapon)
    ///
    /// # Behavior
    /// - Determines fire mode (projectile vs instant)
    /// - Applies scatter to target position
    /// - Creates projectiles or applies instant damage
    /// - Fires weapon events (sound, VFX)
    /// - Handles scatter targets
    pub(crate) fn private_fire_weapon(
        &mut self,
        source_obj_id: ObjectId,
        target_obj_id: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
        bonus: &WeaponBonus,
        is_projectile_detonation: bool,
        ignore_ranges: bool,
        inflict_damage: bool,
    ) -> Result<(), WeaponError> {
        // Get positions
        let source_pos = self.get_object_position(source_obj_id)?;
        let mut target_position = if let Some(target_id) = target_obj_id {
            self.get_object_position(target_id)?
        } else if let Some(pos) = target_pos {
            *pos
        } else {
            return Err(WeaponError::InvalidTarget);
        };

        // Apply scatter
        let distance = source_pos.distance(target_position);
        let target_type = if let Some(target_id) = target_obj_id {
            self.get_object_type(target_id)
        } else {
            ObjectType::Unknown
        };
        target_position = self.calculate_scatter(target_position, distance, target_type);

        if is_projectile_detonation {
            if inflict_damage {
                self.deal_damage_internal(
                    source_obj_id,
                    target_obj_id,
                    &target_position,
                    bonus,
                    true,
                )?;
            }
            self.fire_weapon_effects(source_obj_id, &source_pos)?;
            if !self.template.scatter_targets.is_empty() {
                self.fire_scatter_targets(source_obj_id, &target_position, bonus, inflict_damage)?;
            }
            return Ok(());
        }

        // Determine fire mode
        let fire_mode = self.determine_fire_mode();

        match fire_mode {
            FireMode::InstantImpact { splash_radius } => {
                // Instant damage weapon - deal damage immediately
                if inflict_damage {
                    self.deal_damage_internal(
                        source_obj_id,
                        target_obj_id,
                        &target_position,
                        bonus,
                        is_projectile_detonation,
                    )?;
                }
            }
            FireMode::Projectile { speed, lifetime } => {
                // C++ parity: weapons without a projectile object still use
                // travel-time delayed damage instead of hard-failing template lookup.
                if self.template.projectile_name.trim().is_empty() {
                    self.handle_projectileless_flight_damage(
                        source_obj_id,
                        &source_pos,
                        target_obj_id,
                        &target_position,
                        speed,
                        bonus,
                        inflict_damage,
                    )?;
                } else {
                    // Projectile weapon - create projectile object
                    self.create_projectile(
                        source_obj_id,
                        &source_pos,
                        &target_position,
                        target_obj_id,
                        speed,
                        lifetime,
                        bonus,
                    )?;
                }
            }
            FireMode::ContinuousBeam {
                duration,
                damage_per_frame,
            } => {
                let _ = self.create_laser_object(
                    source_obj_id,
                    target_obj_id,
                    &target_position,
                    damage_per_frame,
                    duration,
                );

                let _ = inflict_damage;
            }
        }

        // Fire weapon effects (sound, visual FX)
        self.fire_weapon_effects(source_obj_id, &source_pos)?;

        // Handle scatter targets if configured
        if !self.template.scatter_targets.is_empty() {
            self.fire_scatter_targets(source_obj_id, &target_position, bonus, inflict_damage)?;
        }

        Ok(())
    }

}
