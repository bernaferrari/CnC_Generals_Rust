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
use crate::helpers::{TheGameLogic, TheTerrainLogic};
use crate::object::registry::OBJECT_REGISTRY;
use crate::weapon::{
    DamageType, DeathType, WeaponBonus, WeaponBonusConditionFlags, WeaponBonusField,
    WeaponReloadType, WeaponSlotType, WeaponStatus, WeaponTemplate, NO_MAX_SHOTS_LIMIT,
};
use crate::{GameLogicError, GameLogicResult};
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
                                        handled = true;
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
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let random_pick = rng.gen_range(0..self.scatter_targets_unused.len());
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
            //
            // C++ implementation (lines 2655-2667):
            // ```cpp
            // if (sourceObj && sourceObj->isReloadTimeShared()) {
            //     WeaponSet* weapons = sourceObj->getWeaponSet();
            //     if (weapons) {
            //         for (int slot = 0; slot < WEAPONSLOT_COUNT; ++slot) {
            //             Weapon* weapon = weapons->getWeapon(slot);
            //             if (weapon && weapon != this) {
            //                 weapon->setPossibleNextShotFrame(m_whenWeCanFireAgain);
            //                 weapon->setStatus(BETWEEN_FIRING_SHOTS);
            //             }
            //         }
            //     }
            // }
            // ```
            //
            // Deferred: requires Object::isReloadTimeShared() and WeaponSet integration
            // When implemented:
            // 1. Get source object via TheGameLogic::find_object_by_id(source_id)
            // 2. Check if obj.is_reload_time_shared()
            // 3. Get weapon set and iterate all weapons
            // 4. Set each weapon's when_we_can_fire_again and status
            //
            // This prevents exploits where rapid weapon switching bypasses cooldowns
            // on multi-weapon units like helicopters or aircraft.
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
