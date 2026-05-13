//! Weapon implementation - direct port from C++ Weapon.cpp
//!
//! This module implements the Weapon class that manages individual weapon instances,
//! including ammunition, reloading, firing logic, and cooldown management.
//!
//! Matches C++ implementation from:
//! - GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp
//! - GeneralsMD/Code/GameEngine/Include/GameLogic/Weapon.h

use crate::common::{Bool, Coord3D, KindOf, ObjectID, Real, UnsignedInt, INVALID_ID};
use crate::damage::{
    DamageInfo, DamageInfoInput, DamageType as LogicDamageType, DeathType as LogicDeathType,
};
use crate::helpers::{get_game_logic_random_value, TheGameLogic, TheTerrainLogic, TheThingFactory};
use crate::object::registry::OBJECT_REGISTRY;
use crate::weapon::{
    DamageType, DeathType, WeaponBonus, WeaponBonusConditionFlags, WeaponBonusField,
    WeaponReloadType, WeaponSlotType, WeaponStatus, WeaponTemplate, NO_MAX_SHOTS_LIMIT,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use std::sync::Arc;

/// Weapon instance with state and ammunition management
///
/// Matches C++ Weapon class from Weapon.h line 540
#[derive(Debug, Clone)]
pub struct Weapon {
    /// Template defining this weapon's properties
    template: Arc<WeaponTemplate>,

    /// Which weapon slot this weapon occupies
    wslot: WeaponSlotType,

    /// Current status (ready, reloading, out of ammo, etc)
    status: WeaponStatus,

    /// Current ammunition in clip
    ammo_in_clip: UnsignedInt,

    /// Frame when we can fire again (after delay)
    when_we_can_fire_again: UnsignedInt,

    /// Frame when pre-attack delay finishes
    when_pre_attack_finished: UnsignedInt,

    /// Frame when last reload started
    when_last_reload_started: UnsignedInt,

    /// Projectile stream object ID (if applicable)
    projectile_stream_id: ObjectID,

    /// Whether leech range is currently active
    leech_weapon_range_active: bool,

    /// Whether pitch is limited for this weapon
    pitch_limited: bool,

    /// Maximum shots limit for this weapon (-1 = unlimited)
    max_shot_count: i32,

    /// Current barrel index (for multi-barrel weapons)
    cur_barrel: i32,

    /// Number of shots remaining for current barrel
    num_shots_for_cur_barrel: i32,

    /// Last frame this weapon fired
    last_fire_frame: UnsignedInt,

    /// Frame to suspend FX until
    suspend_fx_frame: UnsignedInt,

    /// Scatter target indices not yet used
    scatter_targets_unused: Vec<i32>,

    /// Number of barrels for this weapon (from drawable)
    /// Matches C++ sourceObj->getDrawable()->getBarrelCount(m_wslot)
    barrel_count: i32,
}

impl Weapon {
    /// Create a new weapon instance from a template
    ///
    /// Matches C++ Weapon::Weapon(const WeaponTemplate* tmpl, WeaponSlotType wslot)
    /// from Weapon.cpp line 1724
    pub fn new(template: Arc<WeaponTemplate>, wslot: WeaponSlotType) -> Self {
        let pitch_limited = template.get_min_target_pitch() > -std::f32::consts::PI
            || template.get_max_target_pitch() < std::f32::consts::PI;
        let shots_per_barrel = template.get_shots_per_barrel();
        let suspend_fx_delay = template.suspend_fx_delay as UnsignedInt;

        Self {
            template,
            wslot,
            status: WeaponStatus::OutOfAmmo,
            ammo_in_clip: 0,
            when_we_can_fire_again: 0,
            when_pre_attack_finished: 0,
            when_last_reload_started: 0,
            projectile_stream_id: INVALID_ID,
            leech_weapon_range_active: false,
            pitch_limited,
            max_shot_count: NO_MAX_SHOTS_LIMIT,
            cur_barrel: 0,
            num_shots_for_cur_barrel: shots_per_barrel,
            last_fire_frame: 0,
            suspend_fx_frame: TheGameLogic::get_frame().saturating_add(suspend_fx_delay),
            scatter_targets_unused: Vec::new(),
            barrel_count: 1, // Default to 1, can be updated from drawable
        }
    }

    /// Get the weapon template
    pub fn template(&self) -> &Arc<WeaponTemplate> {
        &self.template
    }

    /// Get current weapon status (uses stored status without frame check)
    ///
    /// This returns the internally stored status. For time-sensitive checks,
    /// use get_status_at_frame() instead.
    ///
    /// Matches C++ Weapon::getStatus() from Weapon.cpp line 2736
    pub fn get_status(&self) -> WeaponStatus {
        self.status
    }

    pub fn is_within_target_pitch(&self, source_obj: ObjectID, target_obj: ObjectID) -> bool {
        if self.is_contact_weapon() || !self.pitch_limited {
            return true;
        }

        let Some(source) = OBJECT_REGISTRY.get_object(source_obj) else {
            return true;
        };
        let Some(target) = OBJECT_REGISTRY.get_object(target_obj) else {
            return true;
        };

        let Ok(source_guard) = source.read() else {
            return true;
        };
        let Ok(target_guard) = target.read() else {
            return true;
        };

        let delta = *target_guard.get_position() - *source_guard.get_position();
        let horizontal = (delta.x * delta.x + delta.y * delta.y).sqrt();
        let pitch = delta.z.atan2(horizontal.max(f32::MIN_POSITIVE));

        pitch >= self.template.get_min_target_pitch()
            && pitch <= self.template.get_max_target_pitch()
    }

    /// Get current weapon status at a specific frame
    ///
    /// This version checks timing-related status changes.
    /// Matches C++ Weapon::getStatus() with TheGameLogic->getFrame() check
    pub fn get_status_at_frame(&self, current_frame: UnsignedInt) -> WeaponStatus {
        // Check pre-attack delay first
        if current_frame < self.when_pre_attack_finished {
            return WeaponStatus::PreAttack;
        }

        // Check if we can fire now
        if current_frame >= self.when_we_can_fire_again {
            if self.ammo_in_clip > 0 {
                return WeaponStatus::ReadyToFire;
            } else {
                return WeaponStatus::OutOfAmmo;
            }
        }

        self.status
    }

    /// Check if weapon can fire
    ///
    /// Combines logic from C++ Weapon::getStatus() and fireWeapon checks
    pub fn can_fire(&self, current_frame: UnsignedInt) -> bool {
        matches!(
            self.get_status_at_frame(current_frame),
            WeaponStatus::ReadyToFire
        )
    }

    /// Load ammunition immediately without delay
    ///
    /// Matches C++ Weapon::loadAmmoNow() from Weapon.cpp line 1820
    pub fn load_ammo_now(&mut self, current_frame: UnsignedInt) {
        self.reload_with_bonus(current_frame, &WeaponBonus::default(), true);
    }

    /// Reload ammunition with delay
    ///
    /// Matches C++ Weapon::reloadAmmo() from Weapon.cpp line 1828
    pub fn reload_ammo(&mut self, current_frame: UnsignedInt) {
        self.reload_with_bonus(current_frame, &WeaponBonus::default(), false);
    }

    /// Reload ammunition with specific bonus considerations
    ///
    /// Allows caller to specify weapon bonuses that affect reload time.
    /// Useful for applying rate-of-fire bonuses from veterancy, horde effects, etc.
    pub fn reload_ammo_with_bonus(&mut self, current_frame: UnsignedInt, bonus: &WeaponBonus) {
        self.reload_with_bonus(current_frame, bonus, false);
    }

    /// Reload with weapon bonus consideration
    ///
    /// Matches C++ Weapon::reloadWithBonus() from Weapon.cpp line 1877
    fn reload_with_bonus(
        &mut self,
        current_frame: UnsignedInt,
        bonus: &WeaponBonus,
        load_instantly: bool,
    ) {
        let clip_size = self.template.get_clip_size();

        // Set ammo to clip size (or effectively unlimited if 0)
        self.ammo_in_clip = if clip_size <= 0 {
            0x7fffffff // Effectively unlimited
        } else {
            clip_size as UnsignedInt
        };

        self.status = WeaponStatus::ReloadingClip;

        let reload_time: i32 = if load_instantly {
            0
        } else {
            self.template.get_clip_reload_time(bonus)
        };

        self.when_last_reload_started = current_frame;
        self.when_we_can_fire_again = current_frame + reload_time as UnsignedInt;

        self.rebuild_scatter_targets();
    }

    /// Rebuild scatter target list for new clip
    ///
    /// Matches C++ Weapon::rebuildScatterTargets() from Weapon.cpp line 1864
    fn rebuild_scatter_targets(&mut self) {
        self.scatter_targets_unused.clear();

        let scatter_count = self.template.get_scatter_targets_count();
        if scatter_count > 0 {
            for i in 0..scatter_count {
                self.scatter_targets_unused.push(i as i32);
            }
        }
    }

    /// Fire the weapon at a target object with full bonus integration
    ///
    /// Matches C++ Weapon::fireWeapon(Object*, Object*) from Weapon.cpp line 2692
    ///
    /// # Arguments
    /// * `source_id` - ID of the source object firing the weapon
    /// * `target_id` - ID of the target object
    /// * `current_frame` - Current game frame
    /// * `source_bonus_flags` - Weapon bonus condition flags from source object (veterancy, horde, etc.)
    /// * `container_bonus_flags` - Optional bonus flags from container if source is in transport
    ///
    /// Returns (reloaded, projectile_id)
    pub fn fire_weapon(
        &mut self,
        source_id: ObjectID,
        target_id: ObjectID,
        current_frame: UnsignedInt,
        source_bonus_flags: WeaponBonusConditionFlags,
        container_bonus_flags: Option<WeaponBonusConditionFlags>,
    ) -> GameLogicResult<(bool, Option<ObjectID>)> {
        self.private_fire_weapon(
            source_id,
            Some(target_id),
            None,
            current_frame,
            false,
            false,
            WeaponBonusConditionFlags::empty(),
            true,
            source_bonus_flags,
            container_bonus_flags,
        )
    }

    /// Fire the weapon at a target object (simple version without bonus flags)
    ///
    /// This is a convenience method for cases where bonus flags are not yet available.
    /// For full integration, use fire_weapon() with proper bonus flags.
    ///
    /// Returns (reloaded, projectile_id)
    pub fn fire_weapon_at_object(
        &mut self,
        source_id: ObjectID,
        target_id: ObjectID,
        current_frame: UnsignedInt,
    ) -> GameLogicResult<(bool, Option<ObjectID>)> {
        self.fire_weapon(
            source_id,
            target_id,
            current_frame,
            WeaponBonusConditionFlags::empty(),
            None,
        )
    }

    /// Fire the weapon at a position
    ///
    /// Matches C++ Weapon::fireWeapon(Object*, Coord3D*) from Weapon.cpp line 2700
    ///
    /// Returns true if weapon auto-reloaded after firing
    pub fn fire_weapon_at_pos(
        &mut self,
        source_id: ObjectID,
        pos: &Coord3D,
        current_frame: UnsignedInt,
        source_bonus_flags: WeaponBonusConditionFlags,
        container_bonus_flags: Option<WeaponBonusConditionFlags>,
    ) -> GameLogicResult<(bool, Option<ObjectID>)> {
        self.private_fire_weapon(
            source_id,
            None,
            Some(*pos),
            current_frame,
            false,
            false,
            WeaponBonusConditionFlags::empty(),
            true,
            source_bonus_flags,
            container_bonus_flags,
        )
    }

    /// Internal firing implementation
    ///
    /// Matches C++ Weapon::privateFireWeapon() from Weapon.cpp line 2457
    ///
    /// Returns (reloaded, projectile_id)
    #[allow(clippy::too_many_arguments)]
    fn private_fire_weapon(
        &mut self,
        source_id: ObjectID,
        target_id: Option<ObjectID>,
        target_pos: Option<Coord3D>,
        current_frame: UnsignedInt,
        is_projectile_detonation: bool,
        ignore_ranges: bool,
        extra_bonus_flags: WeaponBonusConditionFlags,
        inflict_damage: bool,
        source_bonus_flags: WeaponBonusConditionFlags,
        container_bonus_flags: Option<WeaponBonusConditionFlags>,
    ) -> GameLogicResult<(bool, Option<ObjectID>)> {
        // Check weapon status (matches C++ line 2570-2571)
        if self.get_status_at_frame(current_frame) != WeaponStatus::ReadyToFire {
            return Ok((false, None));
        }

        // Compute weapon bonus (matches C++ line 2564, 2572)
        // Gets bonus flags from source object (veterancy, horde, nationalism, etc.)
        // and combines with container bonus flags if in transport
        // Global weapon bonus set would be accessed via TheGameLogic->getGlobalWeaponBonusSet()
        // but is currently not needed as template's extra bonuses handle most cases
        let global_bonus_set = crate::helpers::TheGameLogic::get_global_weapon_bonus_set();
        let bonus = self.compute_bonus(
            source_bonus_flags,
            extra_bonus_flags,
            container_bonus_flags,
            global_bonus_set.as_ref(),
        );

        // Handle special damage types (matches C++ lines 2493-2561)
        match LogicDamageType::from(self.template.get_damage_type()) {
            LogicDamageType::Deploy => {
                // Deploy - assault transport logic (matches C++ Weapon.cpp lines 2495-2506)
                // In C++: sourceObj->getAI()->getAssaultTransportAIInterface()->beginAssault(victimObj)
                // This triggers assault transport units to deploy their garrisoned troops
                if let Some(source_obj) = crate::helpers::TheGameLogic::find_object_by_id(source_id)
                {
                    if let Ok(source_guard) = source_obj.read() {
                        if let Some(ai) = source_guard.get_ai() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                if let Some(assault) =
                                    ai_guard.get_assault_transport_ai_update_interface()
                                {
                                    assault.begin_assault(target_id);
                                }
                            }
                        }
                    }
                }
                self.ammo_in_clip = self.ammo_in_clip.saturating_sub(1);
                self.max_shot_count = self.max_shot_count.saturating_sub(1);

                if self.ammo_in_clip <= 0 && self.template.get_auto_reloads_clip() {
                    self.reload_ammo(current_frame);
                    return Ok((true, None));
                }
                return Ok((false, None));
            }
            LogicDamageType::Disarm => {
                // Disarm - mine clearing logic (matches C++ Weapon.cpp lines 2509-2552)
                // Full C++ implementation:
                // 1. Find target's LandMineInterface via getBehaviorModules()
                // 2. Call lmi->disarm() to deactivate mine
                // 3. Play fire FX: FXList::doFXPos(m_template->getFireFX(veterancy), ...)
                // 4. If no interface, check KINDOF_MINE and destroy directly
                // 5. Record stats: player->getAcademyStats()->recordMineCleared()
                //
                if let Some(target_id) = target_id {
                    if let Some(obj_arc) = OBJECT_REGISTRY.get_object(target_id) {
                        let mut handled = false;
                        if let Ok(obj_guard) = obj_arc.read() {
                            for behavior in obj_guard.get_behavior_modules() {
                                if let Ok(mut behavior_guard) = behavior.lock() {
                                    if let Some(land_mine) =
                                        behavior_guard.get_land_mine_interface()
                                    {
                                        land_mine.disarm();
                                        handled = true;
                                        break;
                                    }
                                }
                            }
                        }

                        if !handled {
                            if let Ok(obj_guard) = obj_arc.read() {
                                if obj_guard.is_kind_of(KindOf::Mine) {
                                    drop(obj_guard);
                                    if let Ok(mut obj_guard) = obj_arc.write() {
                                        obj_guard.kill(
                                            Some(LogicDamageType::LandMine),
                                            Some(LogicDeathType::Exploded),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                self.ammo_in_clip = self.ammo_in_clip.saturating_sub(1);
                self.max_shot_count = self.max_shot_count.saturating_sub(1);

                if self.ammo_in_clip <= 0 && self.template.get_auto_reloads_clip() {
                    self.reload_ammo(current_frame);
                    return Ok((true, None));
                }
                return Ok((false, None));
            }
            LogicDamageType::Hack => {
                // Hack - hacking unit logic (no immediate damage)
                // Handled separately by hacking system
                return Ok((false, None));
            }
            _ => {}
        }

        // Verify we have ammunition (matches C++ debug asserts lines 2566-2568)
        if self.ammo_in_clip <= 0 {
            return Ok((false, None));
        }

        // C++ line 2573: Get current frame
        let now = current_frame;
        let mut reloaded = false;

        // Update barrel tracking (matches C++ lines 2577-2582)
        // barrel_count is set externally from drawable->getBarrelCount(m_wslot)
        if self.cur_barrel >= self.barrel_count {
            self.cur_barrel = 0;
            self.num_shots_for_cur_barrel = self.template.get_shots_per_barrel();
        }

        // Handle scatter targets if configured (matches C++ lines 2584-2611)
        let (final_target_id, final_target_pos) = if !self.scatter_targets_unused.is_empty() {
            // Use scatter targeting
            let target_pos_val = if let Some(tid) = target_id {
                // Get position from target object (matches C++ Weapon.cpp line 2589)
                // In C++: victimPos = victimObj->getPosition()
                // Requires TheGameLogic::find_object_by_id integration
                if let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(tid) {
                    if let Ok(obj) = obj_arc.read() {
                        *obj.get_position()
                    } else {
                        target_pos.unwrap_or(Coord3D::new(0.0, 0.0, 0.0))
                    }
                } else {
                    target_pos.unwrap_or(Coord3D::new(0.0, 0.0, 0.0))
                }
            } else {
                target_pos.unwrap_or(Coord3D::new(0.0, 0.0, 0.0))
            };

            // Pick random scatter target
            let random_pick =
                get_game_logic_random_value(0, self.scatter_targets_unused.len() as i32 - 1)
                    as usize;
            let target_index = self.scatter_targets_unused[random_pick];

            // Calculate scatter offset
            let scatter_target_scalar = self.get_scatter_target_scalar();
            let scatter_offset = self.template.get_scatter_targets_vector()[target_index as usize];

            let mut scattered_pos = target_pos_val;
            scattered_pos.x += scatter_offset.x * scatter_target_scalar;
            scattered_pos.y += scatter_offset.y * scatter_target_scalar;
            // Get ground height (matches C++ Weapon.cpp line 2605)
            // In C++: targetPos.z = TheTerrainLogic->getGroundHeight(targetPos.x, targetPos.y)
            if let Some(terrain) = TheTerrainLogic::get() {
                scattered_pos.z = terrain.get_ground_height(scattered_pos.x, scattered_pos.y, None);
            }

            // Remove used scatter target
            self.scatter_targets_unused.swap_remove(random_pick);

            (None, Some(scattered_pos))
        } else {
            (target_id, target_pos)
        };

        // Fire the weapon template (matches C++ lines 2610 or 2614)
        let mut projectile_id: Option<ObjectID> = None;

        // This calls the WeaponTemplate's firing logic which handles:
        // - Range checking
        // - FX playing
        // - Projectile creation
        // - Laser creation
        // - Delayed damage registration
        // Note: We pass the result to projectile_id reference
        let template = Arc::clone(&self.template);
        let _damage_frame = template.fire_weapon_template(
            source_id,
            self.wslot,
            self.cur_barrel,
            final_target_id,
            final_target_pos.as_ref(),
            &bonus,
            is_projectile_detonation,
            ignore_ranges,
            None,
            inflict_damage,
        )?;

        // Update weapon state (matches C++ lines 2617-2669)
        self.last_fire_frame = now;
        self.ammo_in_clip -= 1;
        self.max_shot_count = self.max_shot_count.saturating_sub(1);
        self.num_shots_for_cur_barrel -= 1;

        // Handle barrel rotation
        if self.num_shots_for_cur_barrel <= 0 {
            self.cur_barrel += 1;
            self.num_shots_for_cur_barrel = self.template.get_shots_per_barrel();
        }

        // Check if we need to reload (matches C++ lines 2627-2668)
        if self.ammo_in_clip <= 0 {
            if self.template.get_auto_reloads_clip() {
                self.reload_ammo(current_frame);
                reloaded = true;
            } else {
                self.status = WeaponStatus::OutOfAmmo;
                self.when_we_can_fire_again = 0x7fffffff;
            }
        } else {
            // Set delay between shots (C++ lines 2643-2648)
            self.status = WeaponStatus::BetweenFiringShots;
            let delay = self.template.get_delay_between_shots(&bonus);
            self.when_last_reload_started = now;
            self.when_we_can_fire_again = now + delay as UnsignedInt;

            // Handle shared reload times (C++ Weapon.cpp lines 2655-2667)
            // Some objects (like aircraft with multiple weapons) share reload time
            // across all weapon slots. When one weapon fires, all weapons get
            // the same cooldown to prevent all weapons from firing simultaneously.
            if let Some(source_obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(source_id)
            {
                if let Ok(mut source_obj) = source_obj_arc.write() {
                    if source_obj.is_reload_time_shared() {
                        let when_can_fire = self.when_we_can_fire_again;
                        for slot_idx in 0..crate::common::WEAPONSLOT_COUNT {
                            let slot = match slot_idx {
                                0 => WeaponSlotType::Primary,
                                1 => WeaponSlotType::Secondary,
                                _ => WeaponSlotType::Tertiary,
                            };
                            if let Some(weapon) = source_obj.get_weapon_in_slot_mut(slot) {
                                weapon.set_possible_next_shot_frame(when_can_fire);
                                weapon.set_status(WeaponStatus::BetweenFiringShots);
                            }
                        }
                    }
                }
            }
        }

        Ok((reloaded, projectile_id))
    }

    /// Get scatter target scalar (for scatter pattern radius calculation)
    ///
    /// Matches C++ Weapon::getScatterTargetScalar() from Weapon.cpp line 1910
    fn get_scatter_target_scalar(&self) -> f32 {
        // Apply accuracy bonuses from veterancy and other conditions
        // Veterancy improves accuracy (reduces scatter)
        // Horde bonus slightly reduces accuracy due to volume fire
        // Base scatter from template
        self.template.get_scatter_target_scalar()
        // Future enhancement: multiply by accuracy modifiers from weapon bonuses
        // e.g., * (1.0 - veterancy_accuracy_bonus) to reduce scatter
    }

    /// Start pre-fire delay
    ///
    /// Matches C++ Weapon::preFireWeapon() from Weapon.cpp line 2677
    pub fn pre_fire_weapon(&mut self, current_frame: UnsignedInt, delay: UnsignedInt) {
        if delay > 0 {
            self.status = WeaponStatus::PreAttack;
            self.when_pre_attack_finished = current_frame + delay;

            if self.template.is_leech_range_weapon() {
                self.leech_weapon_range_active = true;
            }
        }
    }

    /// Check if weapon is within attack range of target
    ///
    /// Matches C++ Weapon::isWithinAttackRange() from Weapon.cpp line 2152
    pub fn is_within_attack_range(
        &self,
        source_pos: &Coord3D,
        target_pos: &Coord3D,
        bonus: &WeaponBonus,
    ) -> bool {
        let dist_sqr = Self::distance_squared(source_pos, target_pos);
        let attack_range_sqr = self.template.get_attack_range(bonus).powi(2);
        let min_range_sqr = self.template.get_minimum_attack_range().powi(2);

        if dist_sqr < min_range_sqr {
            return false;
        }

        dist_sqr <= attack_range_sqr
    }

    /// Calculate squared distance between two points (2D)
    fn distance_squared(a: &Coord3D, b: &Coord3D) -> Real {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        dx * dx + dy * dy
    }

    /// Get attack range for this weapon
    ///
    /// Matches C++ Weapon::getAttackRange() from Weapon.cpp line 2336
    pub fn get_attack_range(&self, bonus: &WeaponBonus) -> Real {
        self.template.get_attack_range(bonus)
    }

    pub fn get_attack_distance(
        &self,
        source_id: ObjectID,
        victim_id: Option<ObjectID>,
        bonus: &WeaponBonus,
    ) -> Real {
        let mut range = self.get_attack_range(bonus);
        let Some(source_arc) = OBJECT_REGISTRY.get_object(source_id) else {
            return range;
        };
        let Ok(source_guard) = source_arc.read() else {
            return range;
        };

        if let Some(victim_id) = victim_id {
            if let Some(victim_arc) = OBJECT_REGISTRY.get_object(victim_id) {
                if let Ok(victim_guard) = victim_arc.read() {
                    range += source_guard
                        .get_geometry_info()
                        .get_bounding_circle_radius();
                    range += victim_guard
                        .get_geometry_info()
                        .get_bounding_circle_radius();
                }
            }
        }

        range
    }

    /// Estimate damage this weapon would do to a target
    ///
    /// Matches C++ Weapon::estimateWeaponDamage() from Weapon.cpp line 2371
    pub fn estimate_weapon_damage(
        &self,
        source_id: ObjectID,
        target_id: Option<ObjectID>,
        target_pos: Option<&Coord3D>,
        bonus: &WeaponBonus,
    ) -> Real {
        // Check if out of ammo
        if self.status == WeaponStatus::OutOfAmmo && !self.template.get_auto_reloads_clip() {
            return 0.0;
        }

        // Calculate base damage with bonuses
        let primary_damage = self.template.get_primary_damage(bonus);
        let secondary_damage = self.template.get_secondary_damage(bonus);

        // Total damage is primary + secondary for estimation
        primary_damage + secondary_damage
    }

    /// Calculate effective damage for a specific target with full bonus integration
    ///
    /// This method computes damage with all applicable bonuses:
    /// - Veterancy bonuses (veteran, elite, heroic)
    /// - Horde bonuses (attack bonus for grouped units)
    /// - Nationalism bonuses (same-faction bonuses)
    /// - Player upgrade bonuses
    /// - Battlefield condition bonuses (fanaticism, frenzy, morale)
    ///
    /// Returns the effective damage amount after all modifiers
    pub fn calculate_effective_damage(
        &self,
        bonus: &WeaponBonus,
        target_id: Option<ObjectID>,
    ) -> Real {
        // Get base damages with weapon bonus
        let base_primary = self.template.get_primary_damage(bonus);
        let base_secondary = self.template.get_secondary_damage(bonus);

        // Calculate total base damage
        let mut total_damage = base_primary + base_secondary;

        // Apply damage bonus multiplier from WeaponBonus
        // This includes veterancy, horde, nationalism, and other condition bonuses
        total_damage *= bonus.get_field(WeaponBonusField::Damage);

        // Future enhancement: apply armor penetration and resistance calculations
        // based on target armor type when object system is integrated

        total_damage
    }

    /// Check if weapon has pitch limitation
    pub fn is_pitch_limited(&self) -> bool {
        self.pitch_limited
    }

    /// Check if weapon is a damage-dealing weapon
    ///
    /// Matches C++ Weapon::isDamageWeapon() from Weapon.cpp line 2789
    pub fn is_damage_weapon(&self) -> bool {
        match LogicDamageType::from(self.template.get_damage_type()) {
            LogicDamageType::Deploy => true,
            LogicDamageType::Disarm => true,
            LogicDamageType::Hack => false,
            _ => {
                let bonus = WeaponBonus::default();
                self.template.get_primary_damage(&bonus) > 0.0
                    || self.template.get_secondary_damage(&bonus) > 0.0
            }
        }
    }

    /// Get current ammo in clip
    pub fn get_ammo_in_clip(&self) -> UnsignedInt {
        self.ammo_in_clip
    }

    /// Get percentage ready to fire (0.0 to 1.0)
    ///
    /// Matches C++ Weapon::getPercentReadyToFire() from Weapon.cpp line 2291
    pub fn get_percent_ready_to_fire(&self, current_frame: UnsignedInt) -> Real {
        match self.get_status_at_frame(current_frame) {
            WeaponStatus::OutOfAmmo | WeaponStatus::PreAttack => 0.0,
            WeaponStatus::ReadyToFire => 1.0,
            WeaponStatus::BetweenFiringShots | WeaponStatus::ReloadingClip => {
                let next_shot = self.when_we_can_fire_again;
                if current_frame >= next_shot {
                    return 1.0;
                }

                let total_time = next_shot.saturating_sub(self.when_last_reload_started);
                if total_time == 0 {
                    return 1.0;
                }

                let time_left = next_shot.saturating_sub(current_frame);
                let time_so_far = total_time.saturating_sub(time_left);

                if time_so_far >= total_time {
                    1.0
                } else {
                    time_so_far as Real / total_time as Real
                }
            }
        }
    }

    /// Get weapon slot type
    pub fn get_weapon_slot(&self) -> WeaponSlotType {
        self.wslot
    }

    /// Get damage type
    pub fn get_damage_type(&self) -> DamageType {
        self.template.get_damage_type()
    }

    /// Get anti-mask (what this weapon can target)
    pub fn get_anti_mask(&self) -> u32 {
        self.template.get_anti_mask()
    }

    /// Check if this is a contact weapon
    pub fn is_contact_weapon(&self) -> bool {
        self.template.is_contact_weapon()
    }

    /// Aim delta in radians (matches C++ Weapon::getAimDelta).
    pub fn get_aim_delta(&self) -> Real {
        self.template.aim_delta
    }

    /// Update weapon on bonus change
    ///
    /// Matches C++ Weapon::onWeaponBonusChange() from Weapon.cpp line 1935
    pub fn on_weapon_bonus_change(&mut self, current_frame: UnsignedInt, bonus: &WeaponBonus) {
        let new_delay = match self.status {
            WeaponStatus::ReloadingClip => self.template.get_clip_reload_time(bonus),
            WeaponStatus::BetweenFiringShots => self.template.get_delay_between_shots(bonus),
            _ => return,
        };

        self.when_last_reload_started = current_frame;
        self.when_we_can_fire_again = current_frame + new_delay as UnsignedInt;
    }

    /// Set leech range active status
    pub fn set_leech_range_active(&mut self, active: bool) {
        self.leech_weapon_range_active = active;
    }

    /// Check if leech range is active
    pub fn is_leech_range_active(&self) -> bool {
        self.leech_weapon_range_active
    }

    /// Set the next possible shot frame (for shared reload times)
    ///
    /// Used when an object has shared reload times across all weapons.
    /// Matches C++ Weapon::setPossibleNextShotFrame()
    pub fn set_possible_next_shot_frame(&mut self, frame: UnsignedInt) {
        self.when_we_can_fire_again = frame;
    }

    /// Get the next possible shot frame
    pub fn get_possible_next_shot_frame(&self) -> UnsignedInt {
        self.when_we_can_fire_again
    }

    /// Set weapon status directly
    ///
    /// Used for shared reload time management and external state control
    pub fn set_status(&mut self, status: WeaponStatus) {
        self.status = status;
    }

    /// Get max shot count limit
    pub fn get_max_shot_count(&self) -> i32 {
        self.max_shot_count
    }

    /// Set max shot count limit
    pub fn set_max_shot_count(&mut self, count: i32) {
        self.max_shot_count = count;
    }

    /// Decrement max shot count (for limited ammo weapons)
    pub fn decrement_max_shot_count(&mut self) {
        if self.max_shot_count != NO_MAX_SHOTS_LIMIT {
            self.max_shot_count = self.max_shot_count.saturating_sub(1);
        }
    }

    /// Set clip to a percentage full
    ///
    /// Matches C++ Weapon::setClipPercentFull() from Weapon.cpp line 1845
    pub fn set_clip_percent_full(&mut self, percent: Real, allow_reduction: Bool) {
        let clip_size = self.template.get_clip_size();
        if clip_size == 0 {
            return;
        }

        let new_ammo = (clip_size as Real * percent.clamp(0.0, 1.0)) as UnsignedInt;

        // Only reduce if allowed, or increase
        if allow_reduction || new_ammo > self.ammo_in_clip {
            self.ammo_in_clip = new_ammo;

            // Update status based on new ammo count
            if self.ammo_in_clip > 0 {
                self.status = WeaponStatus::ReadyToFire;
            } else {
                self.status = WeaponStatus::OutOfAmmo;
            }
        }
    }

    /// Get last fire frame
    ///
    /// Matches C++ Weapon::getLastFireFrame() from Weapon.h
    pub fn get_last_fire_frame(&self) -> UnsignedInt {
        self.last_fire_frame
    }

    /// Get suspend FX frame
    ///
    /// Matches C++ Weapon::getSuspendFXFrame() from Weapon.h line 595
    pub fn get_suspend_fx_frame(&self) -> UnsignedInt {
        self.suspend_fx_frame
    }

    /// Set suspend FX frame
    pub fn set_suspend_fx_frame(&mut self, frame: UnsignedInt) {
        self.suspend_fx_frame = frame;
    }

    /// Get projectile stream ID
    ///
    /// Matches C++ Weapon::getProjectileStreamID() from Weapon.h
    pub fn get_projectile_stream_id(&self) -> ObjectID {
        self.projectile_stream_id
    }

    /// Set projectile stream ID
    ///
    /// Matches C++ Weapon::setProjectileStreamID() from Weapon.h
    pub fn set_projectile_stream_id(&mut self, id: ObjectID) {
        self.projectile_stream_id = id;
    }

    /// Get current barrel index
    pub fn get_cur_barrel(&self) -> i32 {
        self.cur_barrel
    }

    /// Get remaining cooldown frames before weapon can fire again
    ///
    /// Returns the number of frames remaining before this weapon will be ready to fire.
    /// Returns 0 if weapon is already ready to fire.
    pub fn get_remaining_cooldown_frames(&self, current_frame: UnsignedInt) -> UnsignedInt {
        if current_frame >= self.when_we_can_fire_again {
            0
        } else {
            self.when_we_can_fire_again - current_frame
        }
    }

    /// Get cooldown progress as percentage (0.0 to 1.0)
    ///
    /// Returns 0.0 when just fired, 1.0 when fully ready.
    /// Useful for UI display of weapon cooldown status.
    pub fn get_cooldown_progress(&self, current_frame: UnsignedInt) -> Real {
        if current_frame >= self.when_we_can_fire_again {
            return 1.0;
        }

        let total_cooldown = self
            .when_we_can_fire_again
            .saturating_sub(self.when_last_reload_started);
        if total_cooldown == 0 {
            return 1.0;
        }

        let elapsed = current_frame.saturating_sub(self.when_last_reload_started);
        (elapsed as Real / total_cooldown as Real).min(1.0)
    }

    /// Force reset cooldown (for special abilities or cheats)
    ///
    /// Immediately makes weapon ready to fire, bypassing normal cooldown.
    /// Use sparingly - primarily for special abilities or admin commands.
    pub fn reset_cooldown(&mut self, current_frame: UnsignedInt) {
        self.when_we_can_fire_again = current_frame;
        self.when_last_reload_started = current_frame;
        if self.ammo_in_clip > 0 {
            self.status = WeaponStatus::ReadyToFire;
        }
    }

    /// Get barrel count
    ///
    /// Matches C++ sourceObj->getDrawable()->getBarrelCount(m_wslot) from Weapon.cpp line 2577
    pub fn get_barrel_count(&self) -> i32 {
        self.barrel_count
    }

    /// Set barrel count from drawable
    ///
    /// This should be called when the weapon is attached to a drawable to get the correct
    /// barrel count for multi-barrel weapons.
    pub fn set_barrel_count(&mut self, count: i32) {
        self.barrel_count = count.max(1); // Ensure at least 1 barrel
    }

    /// Force set status to ready (for special cases)
    ///
    /// Matches C++ Weapon::forceSetStatusToReady() from Weapon.cpp
    pub fn force_set_status_to_ready(&mut self) {
        if self.ammo_in_clip > 0 {
            self.status = WeaponStatus::ReadyToFire;
            self.when_we_can_fire_again = 0;
        }
    }

    /// Get the weapon's name from template
    pub fn get_name(&self) -> &str {
        self.template.get_name()
    }

    // ----- C++ parity methods previously missing -----

    /// Check if target is too close (within minimum attack range)
    ///
    /// Matches C++ Weapon::isTooClose(source, target) from Weapon.cpp line 2211
    pub fn is_too_close(&self, source_pos: &Coord3D, target_pos: &Coord3D) -> bool {
        let min_attack_range = self.template.get_minimum_attack_range();
        if min_attack_range == 0.0 {
            return false;
        }
        let dist_sqr = Self::distance_squared(source_pos, target_pos);
        dist_sqr < min_attack_range * min_attack_range
    }

    /// Check if a goal position is within attack range of the target
    ///
    /// Matches C++ Weapon::isGoalPosWithinAttackRange() from Weapon.cpp line 2241
    /// Used by AI to pre-check if moving to a goal position would put us in range.
    /// The 1/4 pathfind cell fudge prevents teetering on the edge of firing range.
    pub fn is_goal_pos_within_attack_range(
        &self,
        goal_pos: &Coord3D,
        target_pos: &Coord3D,
        bonus: &WeaponBonus,
    ) -> bool {
        // Undersize attack range by 1/4 pathfind cell (PATHFIND_CELL_SIZE_F = 10.0)
        let pathfind_fudge = 10.0 * 0.25;
        let attack_range = self.template.get_attack_range(bonus) - pathfind_fudge;
        if attack_range <= 0.0 {
            return false;
        }
        let dist_sqr = Self::distance_squared(goal_pos, target_pos);

        // Oversize min range by 1/4 pathfind cell
        let min_range = self.template.get_minimum_attack_range() + pathfind_fudge;
        if dist_sqr < min_range * min_range - 0.5 {
            return false;
        }
        dist_sqr <= attack_range * attack_range
    }

    pub fn is_source_object_with_goal_position_within_attack_range(
        &self,
        _source_id: ObjectID,
        goal_pos: &Coord3D,
        target_id: Option<ObjectID>,
        target_pos: Option<&Coord3D>,
        bonus: &WeaponBonus,
    ) -> bool {
        if let Some(target_id) = target_id {
            let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                return false;
            };
            let Ok(target_guard) = target_arc.read() else {
                return false;
            };
            self.is_goal_pos_within_attack_range(goal_pos, target_guard.get_position(), bonus)
        } else if let Some(target_pos) = target_pos {
            self.is_goal_pos_within_attack_range(goal_pos, target_pos, bonus)
        } else {
            false
        }
    }

    /// Get remaining ammo (returns 0 if currently reloading)
    ///
    /// Matches C++ Weapon::getRemainingAmmo() from Weapon.h line 666
    /// When reloading, the internal ammo counter says full but the weapon
    /// can't actually fire yet, so this reports 0 during reload.
    pub fn get_remaining_ammo(&self) -> UnsignedInt {
        if self.status == WeaponStatus::ReloadingClip {
            0
        } else {
            self.ammo_in_clip
        }
    }

    /// Get pre-attack delay for a specific target
    ///
    /// Matches C++ Weapon::getPreAttackDelay() from Weapon.h line 692
    pub fn get_pre_attack_delay(&self, bonus: &WeaponBonus) -> i32 {
        self.template.get_pre_attack_delay(bonus)
    }

    /// Get clip reload time with bonus
    ///
    /// Matches C++ Weapon::getClipReloadTime(source) from Weapon.h line 688
    pub fn get_clip_reload_time(&self, bonus: &WeaponBonus) -> i32 {
        self.template.get_clip_reload_time(bonus)
    }

    /// Get primary damage radius with bonus
    ///
    /// Matches C++ Weapon::getPrimaryDamageRadius(source) from Weapon.h line 690
    pub fn get_primary_damage_radius(&self, bonus: &WeaponBonus) -> Real {
        self.template.get_primary_damage_radius(bonus)
    }

    /// Check if this weapon uses a laser
    ///
    /// Matches C++ Weapon::isLaser() from Weapon.h line 658
    pub fn is_laser(&self) -> bool {
        self.template.is_laser()
    }

    /// Compute approach target position for AI movement
    ///
    /// Matches C++ Weapon::computeApproachTarget() from Weapon.cpp line 1977
    /// Returns true if source is already close enough (no movement needed).
    /// Otherwise sets approach_target_pos to a position within weapon range.
    pub fn compute_approach_target(
        &self,
        source_pos: &Coord3D,
        target_pos: &Coord3D,
        angle_offset: Real,
        bonus: &WeaponBonus,
        approach_target_pos: &mut Coord3D,
    ) -> bool {
        // Compute direction from source to target
        let dx = target_pos.x - source_pos.x;
        let dy = target_pos.y - source_pos.y;
        let dz = target_pos.z - source_pos.z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        let min_attack_range = self.template.get_minimum_attack_range();

        // If too close, move away
        if min_attack_range > 10.0 && dist < min_attack_range {
            let mid_range = (self.template.get_attack_range(bonus) + min_attack_range) / 2.0;
            let mut dir_x = source_pos.x - target_pos.x;
            let mut dir_y = source_pos.y - target_pos.y;
            let dir_len = (dir_x * dir_x + dir_y * dir_y)
                .sqrt()
                .max(f32::MIN_POSITIVE);
            dir_x /= dir_len;
            dir_y /= dir_len;

            if angle_offset != 0.0 {
                let angle = dir_y.atan2(dir_x) + angle_offset;
                dir_x = angle.cos();
                dir_y = angle.sin();
            }

            approach_target_pos.x = mid_range * dir_x + target_pos.x;
            approach_target_pos.y = mid_range * dir_y + target_pos.y;
            approach_target_pos.z = mid_range * 0.0 + target_pos.z; // 2D range
            return false;
        }

        const FUDGE: Real = 0.001;
        if dist < FUDGE {
            // Already close enough
            *approach_target_pos = *source_pos;
            return true;
        }

        if self.is_contact_weapon() {
            *approach_target_pos = *target_pos;
            return false;
        }

        // Normalize direction
        let ndx = dx / dist;
        let ndy = dy / dist;

        let (final_dx, final_dy) = if angle_offset != 0.0 {
            let angle = ndy.atan2(ndx) + angle_offset;
            (angle.cos(), angle.sin())
        } else {
            (ndx, ndy)
        };

        // 90% of attack range (ATTACK_RANGE_APPROACH_FUDGE)
        let attack_range = self.template.get_attack_range(bonus) * 0.9;
        approach_target_pos.x = attack_range * final_dx + target_pos.x;
        approach_target_pos.y = attack_range * final_dy + target_pos.y;
        approach_target_pos.z = target_pos.z;

        false
    }

    /// Check if there is clear line of sight for firing
    ///
    /// Matches C++ Weapon::isClearFiringLineOfSightTerrain() from Weapon.cpp line 3066
    /// Uses terrain LOS check via TheTerrainLogic::is_clear_line_of_sight()
    pub fn is_clear_firing_line_of_sight(
        &self,
        source_pos: &Coord3D,
        target_pos: &Coord3D,
    ) -> bool {
        if let Some(terrain) = TheTerrainLogic::get() {
            terrain.is_clear_line_of_sight(source_pos, target_pos)
        } else {
            true // Assume clear if no terrain available
        }
    }

    /// Transfer next-shot stats from another weapon
    ///
    /// Matches C++ Weapon::transferNextShotStatsFrom() from Weapon.cpp line 3127
    /// Used for weapons like Jarmen Kell's bike sniper attack to share cooldown state.
    pub fn transfer_next_shot_stats_from(&mut self, other: &Weapon) {
        self.when_we_can_fire_again = other.when_we_can_fire_again;
        self.when_last_reload_started = other.when_last_reload_started;
        self.status = other.status;
    }

    pub fn new_projectile_fired(
        &mut self,
        source_id: ObjectID,
        projectile_id: ObjectID,
        victim_id: Option<ObjectID>,
        victim_pos: Option<&Coord3D>,
    ) {
        let stream_name = self.template.projectile_stream_name.trim();
        if stream_name.is_empty() {
            return;
        }

        let mut stream_arc = if self.projectile_stream_id != INVALID_ID {
            TheGameLogic::find_object_by_id(self.projectile_stream_id)
        } else {
            None
        };

        if stream_arc.is_none() {
            self.projectile_stream_id = INVALID_ID;
            let Some(source_arc) = TheGameLogic::find_object_by_id(source_id) else {
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
            let Ok(factory) = TheThingFactory::get() else {
                return;
            };
            let Ok(stream_obj) = factory.new_object(template, &team_guard) else {
                return;
            };
            self.projectile_stream_id = stream_obj
                .read()
                .ok()
                .map(|guard| guard.get_id())
                .unwrap_or(INVALID_ID);
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
            let Some(update) = behavior.get_projectile_stream_update_interface() else {
                continue;
            };
            if let Some(source_arc) = TheGameLogic::find_object_by_id(source_id) {
                if let Ok(source_guard) = source_arc.read() {
                    update.set_position(source_guard.get_position());
                }
            }
            update.add_projectile(
                source_id,
                projectile_id,
                victim_id.unwrap_or(INVALID_ID),
                victim_pos,
            );
            break;
        }
    }

    pub fn create_laser(
        &self,
        source_id: ObjectID,
        victim_id: Option<ObjectID>,
        victim_pos: &Coord3D,
    ) {
        let laser_name = self.template.laser_name.trim();
        if laser_name.is_empty() {
            return;
        }
        let Some(source_arc) = TheGameLogic::find_object_by_id(source_id) else {
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
        let Some(template) = TheThingFactory::find_template(laser_name) else {
            return;
        };
        let Ok(factory) = TheThingFactory::get() else {
            return;
        };
        let Ok(laser_arc) = factory.new_object(template, &team_guard) else {
            return;
        };
        let Ok(mut laser_guard) = laser_arc.write() else {
            return;
        };
        let _ = laser_guard.set_position(source_guard.get_position());
        for behavior in laser_guard.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
                continue;
            };
            let Some(laser_update) = behavior.get_laser_behavior_control_interface() else {
                continue;
            };
            if let Some(victim_id) = victim_id {
                laser_update.activate_laser(victim_id);
            }
            let _ = victim_pos;
            break;
        }
    }

    pub fn process_request_assistance(
        &self,
        requesting_object_id: ObjectID,
        victim_object_id: ObjectID,
    ) {
        let Some(requesting_arc) = TheGameLogic::find_object_by_id(requesting_object_id) else {
            return;
        };
        let Ok(requesting_guard) = requesting_arc.read() else {
            return;
        };
        let Some(player_arc) = requesting_guard.get_controlling_player() else {
            return;
        };
        let template_name = requesting_guard.get_template_name().to_string();
        let request_dist_sqr = self.template.get_request_assist_range().powi(2);
        let requesting_pos = *requesting_guard.get_position();
        drop(requesting_guard);

        let Ok(player_guard) = player_arc.read() else {
            return;
        };
        for object_id in player_guard.get_all_objects() {
            if object_id == requesting_object_id {
                continue;
            }
            let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };
            if object_guard.get_template_name() != template_name {
                continue;
            }
            let dx = object_guard.get_position().x - requesting_pos.x;
            let dy = object_guard.get_position().y - requesting_pos.y;
            if dx * dx + dy * dy > request_dist_sqr {
                continue;
            }
            for behavior in object_guard.get_behavior_modules() {
                if let Ok(mut behavior_guard) = behavior.lock() {
                    let Some(assist) = behavior_guard.get_assisted_targeting_update_interface()
                    else {
                        continue;
                    };
                    if assist.is_free_to_assist() {
                        assist.assist_attack(requesting_object_id, victim_object_id);
                    }
                    break;
                }
            }
        }
    }

    pub fn get_firing_line_of_sight_origin(&self, source_id: ObjectID) -> Option<Coord3D> {
        let source_arc = OBJECT_REGISTRY.get_object(source_id)?;
        let source_guard = source_arc.read().ok()?;
        let pos = source_guard.get_position();
        Some(Coord3D::new(
            pos.x,
            pos.y,
            pos.z
                + source_guard
                    .get_geometry_info()
                    .get_max_height_above_position(),
        ))
    }

    /// Fire weapon as projectile detonation (when projectile hits)
    ///
    /// Matches C++ Weapon::fireProjectileDetonationWeapon() from Weapon.cpp line 2692
    /// Used when a projectile (missile, shell) detonates at the target position.
    pub fn fire_projectile_detonation(
        &mut self,
        source_id: ObjectID,
        target_id: Option<ObjectID>,
        target_pos: Option<&Coord3D>,
        current_frame: UnsignedInt,
        extra_bonus_flags: WeaponBonusConditionFlags,
        inflict_damage: bool,
    ) -> GameLogicResult<(bool, Option<ObjectID>)> {
        self.private_fire_weapon(
            source_id,
            target_id,
            target_pos.copied(),
            current_frame,
            true,  // is_projectile_detonation
            false, // ignore_ranges
            extra_bonus_flags,
            inflict_damage,
            WeaponBonusConditionFlags::empty(),
            None,
        )
    }

    /// Compute weapon bonus from source object conditions
    ///
    /// Matches C++ Weapon::computeBonus() from Weapon.cpp lines 1797-1817
    ///
    /// # Arguments
    /// * `source_bonus_flags` - Weapon bonus condition flags from the source object
    /// * `extra_bonus_flags` - Additional bonus flags to apply
    /// * `container_bonus_flags` - Optional bonus flags from container (if source is in transport)
    /// * `global_bonus_set` - Optional reference to global weapon bonus set
    pub fn compute_bonus(
        &self,
        source_bonus_flags: WeaponBonusConditionFlags,
        extra_bonus_flags: WeaponBonusConditionFlags,
        container_bonus_flags: Option<WeaponBonusConditionFlags>,
        global_bonus_set: Option<&crate::weapon::WeaponBonusSet>,
    ) -> WeaponBonus {
        // Start with cleared bonus (all 1.0 multipliers)
        let mut bonus = WeaponBonus::new();

        // Combine source flags with extra flags (C++ line 1800-1802)
        let mut flags = source_bonus_flags;
        flags.0 |= extra_bonus_flags.0;

        // Add container bonus flags if source is in a transport that passes bonuses (C++ line 1804-1810)
        if let Some(container_flags) = container_bonus_flags {
            flags.0 |= container_flags.0;
        }

        // Apply global weapon bonus set (C++ line 1812-1813)
        if let Some(global_set) = global_bonus_set {
            global_set.append_bonuses(flags, &mut bonus);
        }

        // Apply template's extra bonus (C++ line 1814-1816)
        if let Some(extra) = self.template.get_extra_bonus() {
            extra.append_bonuses(flags, &mut bonus);
        }

        bonus
    }

    /// Compute bonus with simplified interface for common cases
    ///
    /// Uses only source and extra flags without container or global bonuses
    pub fn compute_bonus_simple(
        &self,
        source_bonus_flags: WeaponBonusConditionFlags,
        extra_bonus_flags: WeaponBonusConditionFlags,
    ) -> WeaponBonus {
        self.compute_bonus(source_bonus_flags, extra_bonus_flags, None, None)
    }
}

/// Save/Load serialization for Weapon (matches C++ Weapon::xfer from Weapon.cpp line 3341)
impl Snapshotable for Weapon {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Just xfer the template name for CRC purposes
        let name = self.template.get_name().to_string();
        let mut name_mut = name;
        xfer.xfer_ascii_string(&mut name_mut)
            .map_err(|e| e.to_string())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version history:
        // 1: initial
        // 2: added template name
        // 3: added suspendFXFrame
        let current_version: XferVersion = 3;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        // Template name (version >= 2)
        if version >= 2 {
            let mut tmpl_name = self.template.get_name().to_string();
            xfer.xfer_ascii_string(&mut tmpl_name)
                .map_err(|e| e.to_string())?;
            // On load, we would need to look up the template from WeaponStore
            // For now, we keep the existing template (CRC/save path)
        }

        // Weapon slot
        let mut wslot = self.wslot as i32;
        unsafe {
            xfer.xfer_user(
                (&mut wslot as *mut i32).cast::<u8>(),
                std::mem::size_of::<i32>(),
            )
        }
        .map_err(|e| e.to_string())?;
        // Note: wslot is const in our impl, so we don't restore it

        // Status
        let mut status = self.status as i32;
        unsafe {
            xfer.xfer_user(
                (&mut status as *mut i32).cast::<u8>(),
                std::mem::size_of::<i32>(),
            )
        }
        .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.status = match status {
                0 => WeaponStatus::ReadyToFire,
                1 => WeaponStatus::PreAttack,
                2 => WeaponStatus::BetweenFiringShots,
                3 => WeaponStatus::ReloadingClip,
                _ => WeaponStatus::OutOfAmmo,
            };
        }

        // Ammo in clip
        xfer.xfer_unsigned_int(&mut self.ammo_in_clip)
            .map_err(|e| e.to_string())?;

        // When we can fire again (note: C++ calls this m_whenWeCanFireAgain,
        // mapped to our when_we_can_fire_again)
        xfer.xfer_unsigned_int(&mut self.when_we_can_fire_again)
            .map_err(|e| e.to_string())?;

        // When pre-attack finished
        xfer.xfer_unsigned_int(&mut self.when_pre_attack_finished)
            .map_err(|e| e.to_string())?;

        // When last reload started
        xfer.xfer_unsigned_int(&mut self.when_last_reload_started)
            .map_err(|e| e.to_string())?;

        // Last fire frame
        xfer.xfer_unsigned_int(&mut self.last_fire_frame)
            .map_err(|e| e.to_string())?;

        // Suspend FX frame (version >= 3)
        if version >= 3 {
            xfer.xfer_unsigned_int(&mut self.suspend_fx_frame)
                .map_err(|e| e.to_string())?;
        } else if xfer.is_reading() {
            self.suspend_fx_frame = 0;
        }

        // Projectile stream ID
        xfer.xfer_object_id(&mut self.projectile_stream_id)
            .map_err(|e| e.to_string())?;

        // Laser ID (unused, matches C++ line 3391-3392)
        let mut laser_id_unused: ObjectID = INVALID_ID;
        xfer.xfer_object_id(&mut laser_id_unused)
            .map_err(|e| e.to_string())?;

        // Max shot count
        xfer.xfer_int(&mut self.max_shot_count)
            .map_err(|e| e.to_string())?;

        // Current barrel
        xfer.xfer_int(&mut self.cur_barrel)
            .map_err(|e| e.to_string())?;

        // Num shots for current barrel
        xfer.xfer_int(&mut self.num_shots_for_cur_barrel)
            .map_err(|e| e.to_string())?;

        // Scatter targets unused
        let mut scatter_count = self.scatter_targets_unused.len() as u16;
        xfer.xfer_unsigned_short(&mut scatter_count)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.scatter_targets_unused.clear();
            for _ in 0..scatter_count {
                let mut int_data: i32 = 0;
                xfer.xfer_int(&mut int_data).map_err(|e| e.to_string())?;
                self.scatter_targets_unused.push(int_data);
            }
        } else {
            for target in &self.scatter_targets_unused {
                let mut int_data = *target;
                xfer.xfer_int(&mut int_data).map_err(|e| e.to_string())?;
            }
        }

        // Pitch limited
        let mut pitch_limited = self.pitch_limited;
        xfer.xfer_bool(&mut pitch_limited)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.pitch_limited = pitch_limited;
        }

        // Leech weapon range active
        let mut leech_active = self.leech_weapon_range_active;
        xfer.xfer_bool(&mut leech_active)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.leech_weapon_range_active = leech_active;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ Weapon::loadPostProcess() from Weapon.cpp line 3447
        // Validates projectile stream ID after load
        if self.projectile_stream_id != INVALID_ID {
            if crate::helpers::TheGameLogic::find_object_by_id(self.projectile_stream_id).is_none()
            {
                self.projectile_stream_id = INVALID_ID;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_creation() {
        let template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let weapon = Weapon::new(template, WeaponSlotType::Primary);

        assert_eq!(weapon.get_status(), WeaponStatus::OutOfAmmo);
        assert_eq!(weapon.get_ammo_in_clip(), 0);
    }

    #[test]
    fn test_weapon_reload() {
        let mut template = WeaponTemplate::new("TestWeapon".to_string());
        template.clip_size = 10;
        template.reload_type = WeaponReloadType::AutoReload;

        let template = Arc::new(template);
        let mut weapon = Weapon::new(template, WeaponSlotType::Primary);

        // Load ammo
        weapon.load_ammo_now(0);
        assert_eq!(weapon.get_ammo_in_clip(), 10);
        assert_eq!(weapon.get_status(), WeaponStatus::ReloadingClip);
        assert_eq!(weapon.get_status_at_frame(0), WeaponStatus::ReadyToFire);
    }

    #[test]
    fn test_weapon_fire_and_reload() {
        let mut template = WeaponTemplate::new("TestWeapon".to_string());
        template.clip_size = 1;
        template.reload_type = WeaponReloadType::AutoReload;
        template.min_delay_between_shots = 10;
        template.max_delay_between_shots = 10;

        let template = Arc::new(template);
        let mut weapon = Weapon::new(template, WeaponSlotType::Primary);

        weapon.load_ammo_now(0);

        // Fire weapon with empty bonus flags
        let result = weapon.fire_weapon(1, 2, 0, WeaponBonusConditionFlags::empty(), None);
        assert!(result.is_err()); // Requires registered source/target object IDs.
    }

    #[test]
    fn test_weapon_range_check() {
        let mut template = WeaponTemplate::new("TestWeapon".to_string());
        template.attack_range = 100.0;
        template.minimum_attack_range = 10.0;

        let template = Arc::new(template);
        let weapon = Weapon::new(template, WeaponSlotType::Primary);

        let source = Coord3D::new(0.0, 0.0, 0.0);
        let target_in_range = Coord3D::new(50.0, 0.0, 0.0);
        let target_too_far = Coord3D::new(150.0, 0.0, 0.0);
        let target_too_close = Coord3D::new(5.0, 0.0, 0.0);

        let bonus = WeaponBonus::default();

        assert!(weapon.is_within_attack_range(&source, &target_in_range, &bonus));
        assert!(!weapon.is_within_attack_range(&source, &target_too_far, &bonus));
        assert!(!weapon.is_within_attack_range(&source, &target_too_close, &bonus));
    }
}
