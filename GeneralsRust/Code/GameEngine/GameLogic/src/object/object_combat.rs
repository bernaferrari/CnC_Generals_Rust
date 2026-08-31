//! Split-out inherent `combat (damage, weapons, firing, armor, health)` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ParticleSpawn {
    object_id: ObjectID,
    bone_base: String,
    template_id: u32,
    max_systems: i32,
}

static PARTICLE_MANAGER: once_cell::sync::Lazy<parking_lot::Mutex<Vec<ParticleSpawn>>> =
    once_cell::sync::Lazy::new(|| parking_lot::Mutex::new(Vec::new()));

impl Object {
    /// Queue body particle system spawn requests for the runtime particle bridge.
    pub fn spawn_body_particle_systems(
        &mut self,
        _bone_base_name: &str,
        _system_template_id: u32,
        _max_systems: i32,
    ) {
        PARTICLE_MANAGER.lock().push(ParticleSpawn {
            object_id: self.id,
            bone_base: _bone_base_name.to_string(),
            template_id: _system_template_id,
            max_systems: _max_systems,
        });
    }

    /// Remove all queued body particle system requests for this object.
    pub fn remove_body_particle_systems(&mut self) {
        PARTICLE_MANAGER.lock().retain(|p| p.object_id != self.id);
    }

    // Health and damage
    /// Legacy attempt_damage method (backward compatible)
    /// Wraps attempt_damage_with_return for existing code
    pub fn attempt_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self.attempt_damage_with_return(damage_info) {
            Ok(_) => Ok(()),
            Err(ObjectError::AlreadyDead) => Ok(()), // Silently ignore damage to dead objects for compatibility
            Err(e) => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        }
    }

    pub fn attempt_healing(
        &mut self,
        amount: Real,
        source: Option<&Object>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if amount <= 0.0 {
            return Ok(());
        }

        let source_id = source.map(|obj| obj.get_id()).unwrap_or(INVALID_ID);
        let mut healing_info = DamageInfo {
            input: DamageInfoInput {
                damage_type: DamageType::Healing,
                death_type: DeathType::None,
                source_id,
                amount,
                ..Default::default()
            },
            ..Default::default()
        };
        healing_info.sync_from_input();

        if let Some(body) = &self.body {
            if let Ok(mut body_guard) = body.lock() {
                body_guard.attempt_healing(&mut healing_info)?;
            }
        }

        Ok(())
    }

    pub fn attempt_healing_from_sole_benefactor(
        &mut self,
        amount: Real,
        source: Option<&Object>,
        duration: UnsignedInt,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let Some(source) = source else {
            return Ok(false);
        };

        let now = TheGameLogic::get_frame();
        let source_id = source.get_id();

        if now > self.sole_healing_benefactor_expiration_frame
            || self.sole_healing_benefactor_id == source_id
        {
            self.sole_healing_benefactor_id = source_id;
            self.sole_healing_benefactor_expiration_frame = now + duration;

            let mut healing_info = DamageInfo {
                input: DamageInfoInput {
                    damage_type: DamageType::Healing,
                    death_type: DeathType::None,
                    source_id,
                    amount,
                    ..Default::default()
                },
                ..Default::default()
            };
            healing_info.sync_from_input();

            if let Some(body) = &self.body {
                if let Ok(mut body_guard) = body.lock() {
                    body_guard.attempt_healing(&mut healing_info)?;
                }
            }

            return Ok(true);
        }

        Ok(false)
    }

    pub fn get_sole_healing_benefactor(&self) -> ObjectID {
        let now = TheGameLogic::get_frame();
        if now > self.sole_healing_benefactor_expiration_frame {
            return INVALID_ID;
        }
        self.sole_healing_benefactor_id
    }

    pub fn estimate_damage(&self, _damage_info: &DamageInfoInput) -> Real {
        if let Some(body) = &self.body {
            if let Ok(body_guard) = body.lock() {
                return body_guard.estimate_damage(_damage_info).unwrap_or(0.0);
            }
        }
        0.0
    }

    /// Legacy kill method (backward compatible)
    /// Wraps kill_with_type for existing code compatibility
    pub fn kill(&mut self, damage_type: Option<DamageType>, death_type: Option<DeathType>) {
        let _ = self.kill_with_type(damage_type, death_type);
    }

    pub fn notify_subdual_damage(&mut self, _amount: Real) {
        let Some(body) = self.get_body_module() else {
            return;
        };
        let Ok(body_guard) = body.lock() else {
            return;
        };
        let heal_rate = body_guard.get_subdual_damage_heal_rate();

        if _amount > 0.0 && self.subdual_damage_helper.is_none() {
            self.subdual_damage_helper = Some(Arc::new(Mutex::new(SubdualDamageHelper::new(
                self.id,
                crate::object::helper::SubdualDamageHelperModuleData::new(),
            ))));
        }

        if let Some(helper) = &self.subdual_damage_helper {
            if let Ok(mut helper_guard) = helper.lock() {
                helper_guard.notify_subdual_damage(_amount, heal_rate);
            }
        }

        if let Some(drawable) = self.get_drawable() {
            if let Ok(mut draw_guard) = drawable.write() {
                if _amount > 0.0 {
                    draw_guard.set_tint_status(
                        crate::object::drawable::TintStatus::GAINING_SUBDUAL_DAMAGE,
                    );
                } else {
                    draw_guard.clear_tint_status(
                        crate::object::drawable::TintStatus::GAINING_SUBDUAL_DAMAGE,
                    );
                }
            }
        }
    }

    pub fn do_status_damage(&mut self, _status: ObjectStatusTypes, _duration: Real) {
        use crate::object::helper::{StatusDamageHelper, StatusDamageHelperModuleData};

        if self.status_damage_helper.is_none() {
            self.status_damage_helper = Some(Arc::new(Mutex::new(StatusDamageHelper::new(
                self.id,
                StatusDamageHelperModuleData::new(),
            ))));
        }

        if let Some(helper) = &self.status_damage_helper {
            if let Ok(mut helper_guard) = helper.lock() {
                helper_guard.do_status_damage(_status, _duration);
            }
        }
    }

    pub fn do_temp_weapon_bonus(
        &mut self,
        status: WeaponBonusConditionType,
        duration: UnsignedInt,
    ) {
        use crate::object::helper::{TempWeaponBonusHelper, TempWeaponBonusHelperModuleData};

        let current_frame = crate::helpers::TheGameLogic::get_frame();

        if self.temp_weapon_bonus_helper.is_none() {
            self.temp_weapon_bonus_helper = Some(Arc::new(Mutex::new(TempWeaponBonusHelper::new(
                self.id,
                TempWeaponBonusHelperModuleData::new(),
            ))));
        }

        if let Some(helper) = &self.temp_weapon_bonus_helper {
            if let Ok(mut helper_guard) = helper.lock() {
                let _ = helper_guard.do_temp_weapon_bonus(status, duration, current_frame);
            }
        }
    }

    /// Get the weapon bonus condition flags for this object
    ///
    /// Matches C++ Object::getWeaponBonusCondition() from Object.h line 541
    pub fn get_weapon_bonus_condition(&self) -> WeaponBonusConditionFlags {
        self.weapon_bonus_condition
    }

    pub fn set_weapon_bonus_condition(&mut self, condition: WeaponBonusConditionType) {
        // C++ Object.cpp:4650-4659 — notify WeaponSet only when the mask changes
        // so in-flight RELOADING_CLIP / BETWEEN_FIRING_SHOTS restart at the new ROF.
        let old = self.weapon_bonus_condition;
        self.weapon_bonus_condition.set_condition(condition);
        if old != self.weapon_bonus_condition {
            let _ = self.weapon_set.weapon_set_on_weapon_bonus_change(self.id);
        }
    }

    pub fn clear_weapon_bonus_condition(&mut self, condition: WeaponBonusConditionType) {
        // C++ Object.cpp:4663-4672
        let old = self.weapon_bonus_condition;
        self.weapon_bonus_condition.clear(condition);
        if old != self.weapon_bonus_condition {
            let _ = self.weapon_set.weapon_set_on_weapon_bonus_change(self.id);
        }
    }

    /// Set a multiplicative weapon bonus (e.g., from upgrades/veterancy).
    /// Matches C++ Object::setWeaponBonusMultiplier.
    pub fn set_weapon_bonus_multiplier(&mut self, multiplier: f32) {
        self.weapon_bonus_multiplier = multiplier.max(0.0);
    }

    /// Get current weapon bonus multiplier.
    pub fn weapon_bonus_multiplier(&self) -> f32 {
        self.weapon_bonus_multiplier
    }

    /// Set/unset the player-upgrade weapon set flag.
    /// C++: obj->setWeaponSetFlag(WEAPONSET_PLAYER_UPGRADE)
    pub fn set_weapon_set_flag_player_upgrade(&mut self, flag: bool) {
        if flag {
            self.cur_weapon_set_flags
                .set(crate::weapon::WeaponSetType::PlayerUpgrade);
        } else {
            self.cur_weapon_set_flags
                .clear(crate::weapon::WeaponSetType::PlayerUpgrade);
        }
        let _ = self
            .weapon_set
            .update_weapon_set(self.id, &self.cur_weapon_set_flags);
    }

    // Experience and veterancy
    /// Score a kill for this object (called when this object kills another)
    /// C++ Reference: Object.cpp lines 2896-2948 (scoreTheKill)
    ///
    /// This method handles:
    /// - Score tracking for both killer and victim players
    /// - Skill points and bounty rewards
    /// - Experience point gains
    /// - No experience for killing objects under construction
    ///
    /// # Arguments
    /// * `victim` - The object that was killed by this object
    pub fn score_the_kill(&mut self, victim: &Object) {
        // Do stuff that has nothing to do with experience points here, like tell our Player we killed something
        // Multiplayer score hook location?

        // Get victim's controlling player
        let victim_controller = victim.get_controlling_player();

        // if the other player is not a playable side (i.e. they are civilian, observer, whatever)
        // we shouldn't count the kill.
        if let Some(ref victim_player) = victim_controller {
            if !victim_player
                .read()
                .map(|g| g.is_playable_side())
                .unwrap_or(false)
            {
                return;
            }
        }

        // Ignore kills on GUI-ignored objects
        if victim.is_kind_of(KindOf::IgnoredInGui) {
            return;
        }

        let controller = self.get_controlling_player();

        // Record object lost for victim's player
        if let Some(ref victim_player) = victim_controller {
            if let Ok(mut guard) = victim_player.write() {
                guard.get_score_keeper_mut().add_object_lost_obj(victim);
            }
        }

        // Check relationship - only score kills on enemies
        let relationship = self.relationship_to(victim);
        if relationship != Relationship::Enemies {
            return;
        }

        // Don't count kills that I do on my own buildings or units, cause that's just silly.
        if let (Some(controller_player), Some(victim_player)) = (&controller, &victim_controller) {
            let controller_idx = controller_player.read().ok().map(|g| g.get_player_index());
            let victim_idx = victim_player.read().ok().map(|g| g.get_player_index());
            if controller_idx.is_some() && victim_idx.is_some() && controller_idx == victim_idx {
                return;
            }
        }

        // Record kill for controlling player
        if let Some(ref controller_player) = controller {
            if let Ok(mut guard) = controller_player.write() {
                guard
                    .get_score_keeper_mut()
                    .add_object_destroyed_obj(victim);
                guard.add_skill_points_for_kill_obj(self, victim);
                guard.do_bounty_for_kill_obj(self, victim);
            }
        }

        // Now handle experience, if we can gain any
        let promotion = if let Some(tracker) = &self.experience_tracker {
            if let Ok(mut tracker_guard) = tracker.lock() {
                if tracker_guard.is_accepting_experience_points() {
                    // srj sez: per dustin, no experience (et al) for killing things under construction.
                    if !victim.test_status(ObjectStatusTypes::UnderConstruction) {
                        if let Some(victim_tracker) = &victim.experience_tracker {
                            if let Ok(victim_guard) = victim_tracker.lock() {
                                let victim_cost = victim.get_build_cost();
                                let killer_is_ally = relationship != Relationship::Enemies;
                                let experience_value =
                                    victim_guard.get_experience_value(victim_cost, killer_is_ally);
                                tracker_guard
                                    .add_experience_points(experience_value, true, &[])
                                    .map(|old_level| {
                                        (old_level, tracker_guard.get_veterancy_level())
                                    })
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        if let Some((old_level, new_level)) = promotion {
            self.on_veterancy_level_changed(old_level, new_level, true);
        }
    }

    pub fn on_veterancy_level_changed(
        &mut self,
        old_level: VeterancyLevel,
        new_level: VeterancyLevel,
        provide_feedback: bool,
    ) {
        // Update upgrade modules (C++ Object.cpp line 3013)
        self.update_upgrade_modules_from_player();

        // Find and apply veterancy upgrade (C++ lines 3014-3016)
        let level_name = match new_level {
            VeterancyLevel::Regular => None,
            VeterancyLevel::Veteran => Some("VETERAN"),
            VeterancyLevel::Elite => Some("ELITE"),
            VeterancyLevel::Heroic => Some("HEROIC"),
        };
        if let Some(level_str) = level_name {
            if let Ok(center) = crate::upgrade::center::THE_UPGRADE_CENTER.read() {
                if let Some(upgrade) = center.find_veterancy_upgrade(level_str) {
                    self.give_upgrade(&upgrade);
                }
            }
        }

        // Notify body module (C++ lines 3018-3020)
        if let Some(body) = &self.body {
            if let Ok(mut body_guard) = body.lock() {
                let _ =
                    body_guard.on_veterancy_level_changed(old_level, new_level, provide_feedback);
            }
        }

        // Determine if we should hide animation for stealth (C++ lines 3022-3029)
        let hide_animation_for_stealth = !self.is_locally_controlled()
            && self.test_status(ObjectStatusTypes::Stealthed)
            && !self.test_status(ObjectStatusTypes::Detected)
            && !self.test_status(ObjectStatusTypes::Disguised);

        // Plan to do animation if level went up
        let mut do_animation = !hide_animation_for_stealth
            && (new_level > old_level)
            && !self.is_kind_of(KindOf::IgnoredInGui);

        // Update weapon set flags and weapon bonus conditions based on veterancy level
        match new_level {
            VeterancyLevel::Regular => {
                self.clear_weapon_set_flag(WeaponSetType::Veteran);
                self.clear_weapon_set_flag(WeaponSetType::Elite);
                self.clear_weapon_set_flag(WeaponSetType::Hero);
                self.clear_weapon_bonus_condition(WeaponBonusConditionType::Veteran);
                self.clear_weapon_bonus_condition(WeaponBonusConditionType::Elite);
                self.clear_weapon_bonus_condition(WeaponBonusConditionType::Hero);
                do_animation = false; // Not if somehow up to Regular
            }
            VeterancyLevel::Veteran => {
                self.set_weapon_set_flag(WeaponSetType::Veteran);
                self.clear_weapon_set_flag(WeaponSetType::Elite);
                self.clear_weapon_set_flag(WeaponSetType::Hero);
                self.set_weapon_bonus_condition(WeaponBonusConditionType::Veteran);
                self.clear_weapon_bonus_condition(WeaponBonusConditionType::Elite);
                self.clear_weapon_bonus_condition(WeaponBonusConditionType::Hero);
            }
            VeterancyLevel::Elite => {
                self.clear_weapon_set_flag(WeaponSetType::Veteran);
                self.set_weapon_set_flag(WeaponSetType::Elite);
                self.clear_weapon_set_flag(WeaponSetType::Hero);
                self.clear_weapon_bonus_condition(WeaponBonusConditionType::Veteran);
                self.set_weapon_bonus_condition(WeaponBonusConditionType::Elite);
                self.clear_weapon_bonus_condition(WeaponBonusConditionType::Hero);
            }
            VeterancyLevel::Heroic => {
                self.clear_weapon_set_flag(WeaponSetType::Veteran);
                self.clear_weapon_set_flag(WeaponSetType::Elite);
                self.set_weapon_set_flag(WeaponSetType::Hero);
                self.clear_weapon_bonus_condition(WeaponBonusConditionType::Veteran);
                self.clear_weapon_bonus_condition(WeaponBonusConditionType::Elite);
                self.set_weapon_bonus_condition(WeaponBonusConditionType::Hero);
            }
        }

        // C++ Object::onVeterancyLevelChanged: animation + unitPromoted only when
        // doAnimation && TheGameLogic->getDrawIconUI() && provideFeedback.
        if do_animation && crate::helpers::TheGameLogic::get_draw_icon_ui() && provide_feedback {
            let pos = *self.get_position();
            let pos_with_offset = Coord3D::new(
                pos.x + self.health_box_offset.x,
                pos.y + self.health_box_offset.y,
                pos.z + self.health_box_offset.z,
            );

            if let Some(tracker) = &self.experience_tracker {
                if let Ok(mut _tracker_guard) = tracker.lock() {
                    let _ = crate::experience::PromotionEffectSpawner::spawn_effect(
                        &crate::experience::PromotionEffect::for_level(new_level),
                        pos_with_offset,
                        self.id,
                    );
                }
            }

            if let Some(audio) = crate::helpers::TheAudio::get() {
                let mut sound = crate::helpers::TheAudio::get_misc_audio()
                    .unit_promoted
                    .clone();
                sound.set_object_id(self.id as u32);
                audio.add_audio_event(&sound);
            }
        }

        // Fire veterancy event
        self.fire_veterancy_event(old_level, new_level);

        log::debug!(
            "Object {} veterancy changed from {:?} to {:?}",
            self.id,
            old_level,
            new_level
        );
    }

    pub fn get_experience_tracker(&self) -> Option<Arc<Mutex<ExperienceTracker>>> {
        self.experience_tracker.clone()
    }

    pub fn get_veterancy_level(&self) -> VeterancyLevel {
        if let Some(tracker) = &self.experience_tracker {
            if let Ok(tracker_guard) = tracker.lock() {
                return tracker_guard.get_veterancy_level();
            }
        }
        VeterancyLevel::Regular
    }

    // Weapon management
    pub fn get_weapon_in_weapon_slot(&self, slot: WeaponSlotType) -> Option<&Weapon> {
        self.weapon_set.get_weapon_in_weapon_slot(slot)
    }

    pub fn get_current_weapon(&self) -> Option<(&Weapon, WeaponSlotType)> {
        self.weapon_set.get_current_weapon()
    }

    /// Set the max shots-to-fire limit on the current weapon (C++ Weapon::setMaxShotCount).
    pub fn set_current_weapon_max_shot_count(&mut self, max_shots: i32) {
        if let Some(weapon) = self.weapon_set.get_current_weapon_mut() {
            weapon.set_max_shot_count(max_shots);
        }
    }

    pub fn fire_current_weapon_at_object(
        &mut self,
        target: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.fire_current_weapon_at_target(target)
            .map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>)
    }

    pub fn fire_current_weapon_at_position(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let source_bonus_flags = self.weapon_bonus_condition;
        let container_bonus_flags = self.get_container_id().and_then(|container_id| {
            crate::object::registry::OBJECT_REGISTRY
                .with_object(container_id, |container| {
                    if let Some(contain_module) = &container.contain {
                        if let Ok(contain) = contain_module.try_lock() {
                            if contain.passes_weapon_bonus_to_passengers() {
                                return Some(container.weapon_bonus_condition);
                            }
                        }
                    }
                    None
                })
                .flatten()
        });

        let mut weapon_set = std::mem::take(&mut self.weapon_set);
        let weapon_result = (|| {
            let (name, reloaded) = {
                let weapon = weapon_set
                    .get_current_weapon_mut()
                    .ok_or(ObjectError::NoWeapon)?;

                if weapon.get_status() != WeaponStatus::ReadyToFire {
                    return Err(ObjectError::WeaponNotReady);
                }

                let reloaded = weapon
                    .fire_weapon_at_position_with_bonus_and_reload_flag(
                        self.id,
                        pos,
                        source_bonus_flags,
                        container_bonus_flags,
                    )
                    .map_err(|e| ObjectError::WeaponFireFailed(e.to_string()))?;

                // Note: C++ Object.cpp does NOT set OBJECT_STATUS_IS_FIRING_WEAPON here;
                // that is done in AIUpdate, not in fireCurrentWeapon.
                self.notify_firing_tracker_shot_fired(weapon, INVALID_ID);
                (weapon.get_name().to_string(), reloaded)
            };

            if reloaded {
                weapon_set.release_weapon_lock(WeaponLockType::LockedTemporarily);
            }

            Ok(name)
        })();
        self.weapon_set = weapon_set;
        let weapon_name = weapon_result?;

        self.friend_set_undetected_defector(false);
        self.fire_weapon_fired_event(&weapon_name, None);
        Ok(())
    }

    pub fn fire_weapon_in_slot_at_position(
        &mut self,
        slot: WeaponSlotType,
        pos: &Coord3D,
    ) -> Result<(), ObjectError> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let source_bonus_flags = self.weapon_bonus_condition;
        let container_bonus_flags = self.get_container_id().and_then(|container_id| {
            crate::object::registry::OBJECT_REGISTRY
                .with_object(container_id, |container| {
                    if let Some(contain_module) = &container.contain {
                        if let Ok(contain) = contain_module.try_lock() {
                            if contain.passes_weapon_bonus_to_passengers() {
                                return Some(container.weapon_bonus_condition);
                            }
                        }
                    }
                    None
                })
                .flatten()
        });

        let mut weapon_set = std::mem::take(&mut self.weapon_set);
        let weapon_result = (|| {
            let weapon = weapon_set
                .get_weapon_in_slot_mut(slot)
                .ok_or(ObjectError::NoWeapon)?;

            if weapon.get_status() != WeaponStatus::ReadyToFire {
                return Err(ObjectError::WeaponNotReady);
            }

            let reloaded = weapon
                .fire_weapon_at_position_with_bonus_and_reload_flag(
                    self.id,
                    pos,
                    source_bonus_flags,
                    container_bonus_flags,
                )
                .map_err(|e| ObjectError::WeaponFireFailed(e.to_string()))?;

            self.notify_firing_tracker_shot_fired(weapon, INVALID_ID);

            let name = weapon.get_name().to_string();
            Ok((name, reloaded))
        })();
        self.weapon_set = weapon_set;
        let (weapon_name, reloaded) = weapon_result?;

        if reloaded {
            self.weapon_set
                .release_weapon_lock(WeaponLockType::LockedTemporarily);
        }

        self.friend_set_undetected_defector(false);
        self.fire_weapon_fired_event(&weapon_name, None);
        Ok(())
    }

    pub fn pre_fire_current_weapon(&mut self, victim: Option<ObjectID>) {
        let mut weapon_set = std::mem::take(&mut self.weapon_set);
        if let Some(weapon) = weapon_set.get_current_weapon_mut() {
            let victim_id = victim.unwrap_or(INVALID_ID);
            let _ = weapon.pre_fire_weapon(self.id, victim_id);
        }
        self.weapon_set = weapon_set;
    }

    pub fn set_firing_condition_for_current_weapon(&mut self) {
        self.set_status(
            ObjectStatusMaskType::from_status(ObjectStatusTypes::IsFiringWeapon),
            true,
        );
    }

    pub fn cancel_pre_attack_for_current_weapon(&mut self) {
        let mut weapon_set = std::mem::take(&mut self.weapon_set);
        if let Some(weapon) = weapon_set.get_current_weapon_mut() {
            weapon.set_pre_attack_finished_frame(0);
        }
        self.weapon_set = weapon_set;
    }

    pub(super) fn notify_firing_tracker_shot_fired(
        &mut self,
        weapon: &crate::weapon::Weapon,
        victim_id: ObjectID,
    ) {
        let mut handled = false;
        for entry in &self.update_module_handles {
            entry.with_module(|module| {
                if let Some(tracker_module) = module_behavior_utility_kind(module)
                    .and_then(BehaviorUtilityModuleKindMut::into_firing_tracker)
                {
                    tracker_module.behavior_mut().shot_fired(weapon, victim_id);
                    handled = true;
                }
            });
            if handled {
                break;
            }
        }

        if !handled {
            if let Some(tracker) = &self.firing_tracker {
                if let Ok(mut tracker_guard) = tracker.lock() {
                    tracker_guard.shot_fired(weapon, victim_id);
                }
            }
        }
    }

    pub(super) fn has_firing_tracker_module(&self) -> bool {
        for entry in &self.update_module_handles {
            let found = entry.with_module(|module| {
                matches!(
                    module_behavior_utility_kind(module),
                    Some(BehaviorUtilityModuleKindMut::FiringTracker(_))
                )
            });
            if found {
                return true;
            }
        }
        false
    }

    pub fn choose_best_weapon_for_target(
        &mut self,
        target: &Object,
        criteria: WeaponChoiceCriteria,
        cmd_source: CommandSourceType,
    ) -> bool {
        self.choose_best_weapon_for_target_id(target.get_id(), criteria, cmd_source)
    }

    pub fn choose_best_weapon_for_target_id(
        &mut self,
        target_id: ObjectID,
        criteria: WeaponChoiceCriteria,
        cmd_source: CommandSourceType,
    ) -> bool {
        self.weapon_set
            .choose_best_weapon_for_target(self.id, target_id, criteria, cmd_source)
            .unwrap_or(false)
    }

    pub fn is_able_to_attack(&self) -> bool {
        // Check if object can attack
        self.has_any_weapon()
    }

    pub fn has_any_weapon(&self) -> bool {
        self.weapon_set.has_any_weapon()
    }

    pub fn has_any_damage_weapon(&self) -> bool {
        self.weapon_set.has_any_damage_weapon()
    }

    pub fn is_out_of_ammo(&self) -> bool {
        self.weapon_set.is_out_of_ammo()
    }

    /// Check if current weapon is locked
    ///
    /// Matches C++ Object::isCurWeaponLocked() from Object.h line 525
    pub fn is_cur_weapon_locked(&self) -> bool {
        self.weapon_set.is_current_weapon_locked()
    }

    /// Get largest weapon range across all weapon slots
    ///
    /// Matches C++ Object::getLargestWeaponRange() from Object.h line 455
    pub fn get_largest_weapon_range(&self) -> f32 {
        let mut max_range: f32 = 0.0;
        for slot in [
            WeaponSlotType::Primary,
            WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary,
        ] {
            if let Some(weapon) = self.weapon_set.get_weapon_in_slot(slot) {
                let range = weapon.get_attack_range(self.id);
                if range > max_range {
                    max_range = range;
                }
            }
        }
        max_range
    }

    /// Check if weapon set can deal a specific damage type
    ///
    /// Matches C++ Object::hasWeaponToDealDamageType() from Object.h line 454
    pub fn has_weapon_to_deal_damage_type(&self, damage_type: crate::weapon::DamageType) -> bool {
        self.weapon_set
            .has_weapon_to_deal_damage_type(damage_type.into())
    }

    /// Check if this object shares reload time across all weapons
    ///
    /// When true, firing any weapon sets the cooldown on all weapons.
    /// Used by multi-weapon units like aircraft to prevent simultaneous firing.
    ///
    /// Matches C++ Object::isReloadTimeShared() from Object.h
    pub fn is_reload_time_shared(&self) -> bool {
        self.weapon_set.is_shared_reload_time()
    }

    pub fn get_able_to_attack_specific_object(
        &self,
        attack_type: AbleToAttackType,
        target: &Object,
        cmd_source: CommandSourceType,
    ) -> CanAttackResult {
        self.weapon_set.get_able_to_attack_specific_object(
            attack_type,
            self.get_id(),
            target.get_id(),
            cmd_source,
            None,
        )
    }

    pub fn get_able_to_use_weapon_against_target(
        &self,
        attack_type: AbleToAttackType,
        victim: &Object,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
    ) -> CanAttackResult {
        self.weapon_set.get_able_to_use_weapon_against_target(
            attack_type,
            self.get_id(),
            Some(victim.get_id()),
            Some(pos),
            cmd_source,
            None,
        )
    }

    pub fn get_able_to_use_weapon_against_position(
        &self,
        attack_type: AbleToAttackType,
        pos: &Coord3D,
        cmd_source: CommandSourceType,
    ) -> CanAttackResult {
        self.weapon_set.get_able_to_use_weapon_against_target(
            attack_type,
            self.get_id(),
            None,
            Some(pos),
            cmd_source,
            None,
        )
    }

    /// Flag helpers for salvage-style weapon upgrades.
    pub fn test_weapon_set_flag(&self, flag: WeaponSetType) -> bool {
        self.cur_weapon_set_flags.test(flag)
    }

    pub fn set_weapon_set_flag(&mut self, flag: WeaponSetType) {
        self.cur_weapon_set_flags.set(flag);
        let _ = self
            .weapon_set
            .update_weapon_set(self.id, &self.cur_weapon_set_flags);
        if let Some(condition) = weapon_set_model_condition(flag) {
            self.set_model_condition_state(condition);
        }
    }

    pub fn has_weapon_set_template(&self, flag: WeaponSetType) -> bool {
        let mut flags = WeaponSetFlags::new();
        flags.set(flag);
        self.weapon_set.find_weapon_template_set(&flags).is_some()
    }

    pub fn clear_weapon_set_flag(&mut self, flag: WeaponSetType) {
        self.cur_weapon_set_flags.clear(flag);
        let _ = self
            .weapon_set
            .update_weapon_set(self.id, &self.cur_weapon_set_flags);
        if let Some(condition) = weapon_set_model_condition(flag) {
            self.clear_model_condition_state(condition);
        }
    }

    /// Flag helpers for salvage armor upgrades.
    pub fn test_armor_set_flag(&self, flag: ArmorSetFlag) -> bool {
        if let Some(body) = &self.body {
            if let Ok(body_guard) = body.lock() {
                return body_guard.test_armor_set_flag(armor_set_type_for_flag(flag));
            }
        }
        self.armor_set_flags.test(flag)
    }

    pub fn set_armor_set_flag(&mut self, flag: ArmorSetFlag) {
        if let Some(body) = &self.body {
            if let Ok(mut body_guard) = body.lock() {
                let _ = body_guard.set_armor_set_flag(armor_set_type_for_flag(flag));
            }
        }
        self.armor_set_flags.set(flag);
    }

    pub fn clear_armor_set_flag(&mut self, flag: ArmorSetFlag) {
        if let Some(body) = &self.body {
            if let Ok(mut body_guard) = body.lock() {
                let _ = body_guard.clear_armor_set_flag(armor_set_type_for_flag(flag));
            }
        }
        self.armor_set_flags.clear(flag);
    }

    pub fn get_ammo_pip_info(&self) -> (i32, i32) {
        match self.weapon_set.find_ammo_pip_showing_weapon() {
            Some(w) => (
                w.get_template().get_clip_size(),
                w.get_remaining_ammo() as i32,
            ),
            None => (0, 0),
        }
    }

    pub fn reload_all_ammo(&mut self, now: bool) -> GameLogicResult<()> {
        self.weapon_set.reload_all_ammo(self.id, now)
    }

    pub fn release_weapon_lock(&mut self, lock_type: WeaponLockType) {
        self.weapon_set.release_weapon_lock(lock_type);
    }

    /// Get weapon in a specific slot (alias for get_weapon_in_weapon_slot for compatibility)
    pub fn get_weapon_in_slot(&self, slot: WeaponSlotType) -> Option<&Weapon> {
        self.get_weapon_in_weapon_slot(slot)
    }

    /// Get a mutable reference to weapon in the specified slot
    pub fn get_weapon_in_slot_mut(&mut self, slot: WeaponSlotType) -> Option<&mut Weapon> {
        self.weapon_set.get_weapon_in_slot_mut(slot)
    }

    /// Get the current victim/target of this object
    /// Returns the object this unit is currently targeting
    pub fn get_current_victim_id(&self) -> Option<ObjectID> {
        let ai = self.ai.as_ref()?;
        let guard = ai.lock().ok()?;
        guard.get_current_victim()
    }

    pub fn get_current_victim(&self) -> Option<Arc<RwLock<Object>>> {
        // Wave 264: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let victim_id = self.get_current_victim_id()?;
        crate::helpers::TheGameLogic::find_object_by_id(victim_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(victim_id))
    }

    /// Get the current victim/target position of this object
    pub fn get_current_victim_pos(&self) -> Option<Coord3D> {
        // Wave 264: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let victim_id = self.get_current_victim_id()?;
        crate::object::registry::OBJECT_REGISTRY.with_object(victim_id, |v| *v.get_position())
    }

    pub fn get_health_percentage(&self) -> f32 {
        if let Some(body) = &self.body {
            if let Ok(guard) = body.lock() {
                let max_health = guard.get_max_health().max(f32::EPSILON);
                return (guard.get_health() / max_health).clamp(0.0, 1.0);
            }
        }
        1.0
    }

    pub fn get_max_damage_potential(&self) -> f32 {
        let mut max_damage = 0.0;
        let slots = [
            WeaponSlotType::Primary,
            WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary,
        ];

        for slot in slots {
            if let Some(weapon) = self.weapon_set.get_weapon_in_weapon_slot(slot) {
                let damage = weapon.estimate_weapon_damage(self.id, None, None);
                if damage > max_damage {
                    max_damage = damage;
                }
            }
        }

        max_damage
    }

    /// Returns the crushing power rating for this object.
    /// C++ Reference: Object.cpp line 1156 (Object::getCrusherLevel)
    pub fn get_crusher_level(&self) -> u32 {
        self.thing_template.get_crusher_level() as u32
    }

    /// Returns the crushable vulnerability level for this object.
    /// C++ Reference: Object.cpp line 1162 (Object::getCrushableLevel)
    pub fn get_crushable_level(&self) -> u32 {
        self.thing_template.get_crushable_level() as u32
    }

    /// Check if this object can crush or squish another object.
    /// C++ Reference: Object.cpp line 1076 (Object::canCrushOrSquish)
    pub fn can_crush_or_squish(&self, other: &Object, test_type: CrushSquishTestType) -> bool {
        if self.is_disabled_by_type(DisabledType::DisabledUnmanned) {
            return false;
        }

        let crusher_level = self.get_crusher_level();

        // Order matters: we want to know if I consider it to be an ally, not vice versa
        if self.relationship_to(other) == Relationship::Allies {
            return false;
        }

        if crusher_level == 0 {
            return false;
        }

        // Check squish module on other object
        if test_type == CrushSquishTestType::TestSquishOnly
            || test_type == CrushSquishTestType::TestCrushOrSquish
        {
            if other.find_module_by_name("SquishCollide").is_some() {
                return true;
            }
        }

        let crushable_level = other.get_crushable_level();

        if test_type == CrushSquishTestType::TestCrushOnly
            || test_type == CrushSquishTestType::TestCrushOrSquish
        {
            if crusher_level > crushable_level {
                return true;
            }
        }

        false
    }

    pub fn get_anti_mask(&self) -> u32 {
        let mut mask = 0;

        if self.is_kind_of(KindOf::Projectile) {
            mask |= WeaponAntiMask::PROJECTILE;
        }
        if self.is_kind_of(KindOf::Mine) {
            mask |= WeaponAntiMask::MINE;
        }
        if self.test_status(ObjectStatusTypes::Parachuting) {
            mask |= WeaponAntiMask::PARACHUTE;
        }

        if self.is_airborne_target() || self.is_kind_of(KindOf::Aircraft) {
            if self.is_kind_of(KindOf::Infantry) {
                mask |= WeaponAntiMask::AIRBORNE_INFANTRY;
            } else {
                mask |= WeaponAntiMask::AIRBORNE_VEHICLE;
            }
        } else if mask == 0 {
            mask |= WeaponAntiMask::GROUND;
        }

        mask
    }

    /// Match C++ Object::calculateCountermeasureToDivertTo.
    pub fn calculate_countermeasure_to_divert_to(&self, victim: &Object) -> ObjectID {
        if self.get_ai_update_interface().is_none() {
            return INVALID_ID;
        }

        let countermeasures_key = NameKeyGenerator::name_to_key("CountermeasuresBehavior");
        victim
            .with_friend_module::<
                crate::object::behavior::countermeasures_behavior::CountermeasuresBehaviorModule,
                _,
                _,
            >(countermeasures_key, |module| {
                module
                    .behavior()
                    .calculate_countermeasure_to_divert_to(victim.get_id())
                    .unwrap_or(INVALID_ID)
            })
            .unwrap_or(INVALID_ID)
    }

    /// Set weapon lock state for a specific weapon slot
    /// C++ Reference: Object.cpp - weapon locking mechanism
    pub fn set_weapon_lock(&mut self, weapon_slot: WeaponSlotType, lock_type: WeaponLockType) {
        let locked = self.weapon_set.set_weapon_lock(weapon_slot, lock_type);
        if !locked {
            log::debug!(
                "Object {} failed to set weapon lock {:?} on slot {:?}",
                self.id,
                lock_type,
                weapon_slot
            );
        }
    }
    //=========================================================================
    // CRITICAL OBJECT SYSTEM METHODS
    // C++ Reference: Object.cpp lines 1424-1976
    //=========================================================================

    /// Get current health
    /// C++ Reference: Object.cpp - health accessor
    pub fn get_health(&self) -> f32 {
        if let Some(body) = &self.body {
            if let Ok(body_guard) = body.lock() {
                return body_guard.get_health();
            }
        }
        100.0 // Default health
    }

    /// Last DamageInfo recorded by the body module (`BodyModule::getLastDamageInfo`).
    /// Includes the death type of a killing blow (e.g. `DeathType::Flooded`).
    pub fn get_last_damage_info(&self) -> Option<DamageInfo> {
        self.body.as_ref().and_then(|body| {
            body.lock()
                .ok()
                .and_then(|body_guard| body_guard.get_last_damage_info())
        })
    }

    /// Last death type stored on this object via the body last-damage snapshot.
    /// C++: `getBodyModule()->getLastDamageInfo()->in.m_deathType`.
    pub fn get_last_death_type(&self) -> Option<DeathType> {
        self.get_last_damage_info()
            .map(|info| info.input.death_type)
    }

    /// Get maximum health
    /// C++ Reference: Object.cpp - max health accessor
    pub fn get_max_health(&self) -> f32 {
        if let Some(body) = &self.body {
            if let Ok(body_guard) = body.lock() {
                return body_guard.get_max_health();
            }
        }
        100.0 // Default max health
    }

    /// Set health to a specific value
    /// C++ Reference: Object.cpp lines 1424-1459 (implied through body module)
    ///
    /// # Arguments
    /// * `new_health` - The health value to set (will be clamped between 0 and max_health)
    ///
    /// # Returns
    /// * `Ok(())` - Health set successfully
    /// * `Err(ObjectError::AlreadyDead)` - Object is already dead
    /// * `Err(ObjectError::NoBodyModule)` - Object has no body module
    ///
    /// # Behavior
    /// - Clamps health between 0 and max_health
    /// - If setting to 0 or below, triggers death
    /// - Returns error if object is already effectively dead
    pub fn set_health(&mut self, new_health: f32) -> Result<(), ObjectError> {
        // Check if already dead
        if self.is_effectively_dead() {
            return Err(ObjectError::AlreadyDead);
        }

        // Get body module
        let body = self.body.as_ref().ok_or(ObjectError::NoBodyModule)?;

        let max_health = {
            let body_guard = body.lock().map_err(|_| ObjectError::LockPoisoned)?;
            body_guard.get_max_health()
        };

        // Clamp health between 0 and max
        let clamped_health = new_health.max(0.0).min(max_health);

        // Apply the health change through body module's internal method
        {
            let mut body_guard = body.lock().map_err(|_| ObjectError::LockPoisoned)?;

            let current_health = body_guard.get_health();
            let delta = clamped_health - current_health;

            // Use internal_change_health to bypass armor/fx
            body_guard
                .internal_change_health(delta)
                .map_err(|e| ObjectError::BodyModuleError(e.to_string()))?;
        }

        // Check if this caused death
        if clamped_health <= 0.0 {
            self.check_health_and_die(None);
        }

        Ok(())
    }

    /// Heal the object by a specific amount
    /// Helper method that adds to current health up to maximum
    pub fn heal(&mut self, amount: f32) -> Result<(), ObjectError> {
        let current = self.get_health();
        let max = self.get_max_health();
        let new_health = (current + amount).min(max);
        self.set_health(new_health)
    }

    /// Restore object to full health
    /// C++ Reference: Object.cpp lines 1973-1976 (healCompletely)
    ///
    /// # Returns
    /// * `Ok(())` - Healed successfully
    /// * `Err(ObjectError::AlreadyDead)` - Cannot heal dead objects
    /// * `Err(ObjectError::NoBodyModule)` - Object has no body module
    ///
    /// # Behavior
    /// - Sets health to max_health
    /// - Fires healing event
    /// - Returns error if object is already dead
    pub fn heal_completely(&mut self) -> Result<(), ObjectError> {
        // Cannot heal dead objects
        if self.is_effectively_dead() {
            return Err(ObjectError::AlreadyDead);
        }

        // Use attemptHealing with huge amount (legacy approach)
        let _max_health = self.get_max_health();
        let mut healing_info = DamageInfo {
            input: DamageInfoInput {
                damage_type: DamageType::Healing,
                death_type: DeathType::None,
                amount: HUGE_DAMAGE_AMOUNT, // Will be clamped to max
                source_id: INVALID_ID,
                ..Default::default()
            },
            ..Default::default()
        };

        if let Some(body) = &self.body {
            let mut body_guard = body.lock().map_err(|_| ObjectError::LockPoisoned)?;

            body_guard
                .attempt_healing(&mut healing_info)
                .map_err(|e| ObjectError::BodyModuleError(e.to_string()))?;
        } else {
            return Err(ObjectError::NoBodyModule);
        }

        // Fire healing event (if health changed)
        if healing_info.output.actual_damage_dealt > 0.0 {
            log::debug!(
                "Object {} healed completely to {}",
                self.id,
                self.get_health()
            );
        }

        Ok(())
    }

    /// Attempt to damage this object
    /// C++ Reference: Object.cpp lines 1818-1880 (attemptDamage)
    /// **THE CRITICAL BLOCKER** - Foundation of all combat
    ///
    /// # Arguments
    /// * `damage_info` - Mutable damage information (input and output)
    ///
    /// # Returns
    /// * `Ok(damage_dealt)` - Damage applied successfully, returns actual damage amount
    /// * `Err(ObjectError::AlreadyDead)` - Object is already dead
    /// * `Err(ObjectError::InvalidDamage)` - Invalid damage parameters
    /// * `Err(ObjectError::Invulnerable)` - Object is invulnerable to this damage
    ///
    /// # Behavior
    /// - Checks if object is dead (returns error if so)
    /// - Delegates to body module for armor/resistance calculations
    /// - Processes shockwave forces if present (applies physics impulse)
    /// - Triggers death if health <= 0
    /// - Fires radar/event notifications
    /// - Returns actual damage applied
    pub fn attempt_damage_with_return(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<f32, ObjectError> {
        // C++ DamageInfo only has input.m_deathType / m_damageType. Callers that
        // set input (OCL diesOnBadLand: DAMAGE_WATER + DEATH_FLOODED) must have
        // those values visible on the compatibility fields after this call.
        damage_info.sync_from_input();

        // Prevent damage to dead objects
        if self.is_effectively_dead() {
            return Err(ObjectError::AlreadyDead);
        }

        // Validate damage amount
        if damage_info.input.amount < 0.0 && damage_info.input.damage_type != DamageType::Healing {
            return Err(ObjectError::InvalidDamage(damage_info.input.amount));
        }

        // Delegate to body module for damage processing
        if let Some(body) = &self.body {
            let mut body_guard = body.lock().map_err(|_| ObjectError::LockPoisoned)?;

            body_guard
                .attempt_damage(damage_info)
                .map_err(|e| ObjectError::BodyModuleError(e.to_string()))?;
        }

        if let Some(contain) = &self.contain {
            if let Ok(mut contain_guard) = contain.lock() {
                if let Err(err) = contain_guard.on_damage(damage_info) {
                    log::warn!("Object {} contain on_damage failed: {}", self.id, err);
                }
            }
        }

        // Process shockwave forces (C++ lines 1824-1860)
        if damage_info.input.shock_wave_amount > 0.0 && damage_info.input.shock_wave_radius > 0.0 {
            // Check if object is eligible for shockwave (not airborne, not projectile)
            if self.shockwave_applies() {
                if let Some(physics) = &self.physics {
                    let mut physics_guard =
                        physics.lock().map_err(|_| ObjectError::LockPoisoned)?;
                    let mut stunned = false;

                    // Calculate shockwave taper based on distance
                    let shock_wave_length = damage_info.input.shock_wave_vector.length();
                    if shock_wave_length > 0.0 {
                        let distance_from_center =
                            (shock_wave_length / damage_info.input.shock_wave_radius).min(1.0);
                        let distance_taper =
                            distance_from_center * (1.0 - damage_info.input.shock_wave_taper_off);
                        let shock_taper_mult = 1.0 - distance_taper;

                        // Calculate shockwave force vector
                        let mut shock_wave_force = damage_info.input.shock_wave_vector;
                        let _ = shock_wave_force.normalize();
                        shock_wave_force *= damage_info.input.shock_wave_amount * shock_taper_mult;

                        // Apply upward force equal to lateral force for dramatic effect
                        shock_wave_force.z = shock_wave_force.length();

                        // Apply shock through physics behavior
                        physics_guard.apply_shock(&shock_wave_force);
                        physics_guard.apply_random_rotation();
                        physics_guard.set_stunned(true);
                        stunned = true;
                    }

                    drop(physics_guard);

                    // Set stunned model condition
                    if stunned {
                        self.set_shockwave_stunned_flailing();
                    }
                }
            }
        }

        // Get actual damage dealt for return value
        let actual_damage = damage_info.output.actual_damage_dealt;

        // C++ Object.cpp:1847-1854 Object::attemptDamage radar event:
        // actualDamageDealt>0 && type not PENALTY/HEALING && controllingPlayer
        // && !BitTest(sourcePlayerMask, controllingPlayerMask) && m_radarData
        // && controllingPlayer == ThePlayerList->getLocalPlayer().
        if actual_damage > 0.0
            && damage_info.input.damage_type != DamageType::Penalty
            && damage_info.input.damage_type != DamageType::Healing
        {
            let attacker_id = if damage_info.input.source_id != INVALID_ID {
                Some(damage_info.input.source_id)
            } else {
                None
            };
            self.fire_damaged_event(actual_damage, attacker_id);

            if self.radar_data.is_some() {
                // C++ Object.cpp:1847-1854 gate: source mask differs from the
                // controlling player's and the victim is the local player.
                let under_attack_local = self.get_controlling_player().and_then(|player| {
                    player.read().ok().map(|guard| {
                        !damage_info
                            .input
                            .source_player_mask
                            .intersects(guard.get_player_mask())
                            && guard.is_local_player()
                    })
                }).unwrap_or(false);
                if under_attack_local {
                    // C++ Object.cpp:1854 — TheRadar->tryUnderAttackEvent(this):
                    // single pipeline — throttled UnderAttack ping, then
                    // per-kind message/audio/EVA (Radar.cpp:1147-1226) gated
                    // on the event actually being created. The player read
                    // guard is dropped above: tryUnderAttackEvent re-reads the
                    // controlling player.
                    let _ = crate::helpers::TheRadar::try_under_attack_event_for_object(self);
                }
            }
        }

        // Check if object died from damage
        let died = self.check_health_and_die(Some(damage_info));

        if died {
            log::debug!(
                "Object {} died from damage (took {} damage)",
                self.id,
                actual_damage
            );
        }

        Ok(actual_damage)
    }

    /// Kill the object instantly
    /// C++ Reference: Object.cpp lines 1954-1968 (kill)
    ///
    /// # Arguments
    /// * `damage_type` - Optional damage type (defaults to Unresistable)
    /// * `death_type` - Optional death type (defaults to Normal)
    ///
    /// # Returns
    /// * `Ok(())` - Object killed successfully
    /// * `Err(ObjectError::AlreadyDead)` - Object is already dead
    ///
    /// # Behavior
    /// - Creates DamageInfo with damage = max_health
    /// - Sets kill flag to TRUE (bypasses armor)
    /// - Calls attemptDamage()
    /// - Object dies regardless of resistance
    pub fn kill_with_type(
        &mut self,
        damage_type: Option<DamageType>,
        death_type: Option<DeathType>,
    ) -> Result<(), ObjectError> {
        // Prevent killing already dead objects
        if self.is_effectively_dead() {
            return Err(ObjectError::AlreadyDead);
        }

        // Objects without a body module still need to be killable for compatibility with
        // tests and legacy call sites (the C++ `Object::kill` forces a death state).
        if self.body.is_none() {
            self.handle_death(None);
            return Ok(());
        }

        // Get max health for lethal damage
        let max_health = self.get_max_health();

        // Create damage info for instant kill
        let mut damage_info = DamageInfo {
            input: DamageInfoInput {
                damage_type: damage_type.unwrap_or(DamageType::Unresistable),
                death_type: death_type.unwrap_or(DeathType::Normal),
                amount: max_health, // Exactly max health to ensure death
                kill: true,         // Force kill flag - bypasses armor/resistance
                source_id: INVALID_ID,
                ..Default::default()
            },
            ..Default::default()
        };

        // Apply the lethal damage
        let _ = self.attempt_damage_with_return(&mut damage_info)?;

        // Verify object died (should always be true with kill flag)
        if !damage_info.output.no_effect {
            Ok(())
        } else {
            // This shouldn't happen with kill flag set
            log::warn!(
                "Object {} failed to die despite kill command (might be InactiveBody)",
                self.id
            );
            Err(ObjectError::IndestructibleBody)
        }
    }

    /// Fire the current weapon at a target object
    /// C++ Reference: Object.cpp lines 1475-1495 (fireCurrentWeapon)
    ///
    /// # Arguments
    /// * `target` - Target object to fire at
    ///
    /// # Returns
    /// * `Ok(())` - Weapon fired successfully
    /// * `Err(ObjectError::NoWeapon)` - No current weapon available
    /// * `Err(ObjectError::WeaponNotReady)` - Weapon is not ready to fire
    /// * `Err(ObjectError::TargetInvalid)` - Target is invalid (null or destroyed)
    ///
    /// # Behavior
    /// - Gets current weapon from weapon set
    /// - Checks if weapon status is READY_TO_FIRE
    /// - Calls weapon.fire(target)
    /// - Marks weapon as not ready (starts cooldown)
    /// - Clears stealth defector flag (firing reveals stealth units)
    /// - Notifies firing tracker for statistics
    /// - Releases temporary weapon locks if reloaded
    pub fn fire_current_weapon_at_target(&mut self, target: &Object) -> Result<(), ObjectError> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        // Check if target is valid
        if target.is_destroyed() {
            return Err(ObjectError::TargetInvalid);
        }

        // Get bonus flags from this object (matches C++ Weapon.cpp line 1800)
        let source_bonus_flags = self.weapon_bonus_condition;

        // Get container bonus flags if we're in a transport (matches C++ Weapon.cpp lines 1804-1810)
        let container_bonus_flags = self.get_container_id().and_then(|container_id| {
            crate::object::registry::OBJECT_REGISTRY
                .with_object(container_id, |container| {
                    if let Some(contain_module) = &container.contain {
                        if let Ok(contain) = contain_module.try_lock() {
                            if contain.passes_weapon_bonus_to_passengers() {
                                return Some(container.weapon_bonus_condition);
                            }
                        }
                    }
                    None
                })
                .flatten()
        });

        // Get current frame from game logic
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        // Get current weapon
        // Temporarily take the weapon set to avoid aliasing `self` during firing.
        let mut weapon_set = std::mem::take(&mut self.weapon_set);
        let weapon_result = (|| {
            let (name, reloaded) = {
                let weapon = weapon_set
                    .get_current_weapon_mut()
                    .ok_or(ObjectError::NoWeapon)?;

                // Check if weapon is ready
                if weapon.get_status() != WeaponStatus::ReadyToFire {
                    return Err(ObjectError::WeaponNotReady);
                }

                // Fire the weapon with full bonus integration (matches C++ Object.cpp fireCurrentWeapon)
                // This passes source object's bonus flags (veterancy, horde, nationalism, etc.)
                // and container bonus flags if in transport
                let reloaded = weapon
                    .fire_weapon_with_bonus_and_reload_flag(
                        self.id,
                        target.get_id(),
                        current_frame,
                        source_bonus_flags,
                        container_bonus_flags,
                    )
                    .map_err(|e| ObjectError::WeaponFireFailed(e.to_string()))?;

                // Notify firing tracker for statistics
                // Note: C++ Object.cpp does NOT set OBJECT_STATUS_IS_FIRING_WEAPON here;
                // that is done in AIUpdate, not in fireCurrentWeapon.
                self.notify_firing_tracker_shot_fired(weapon, target.get_id());
                (weapon.get_name().to_string(), reloaded)
            };

            if reloaded {
                weapon_set.release_weapon_lock(WeaponLockType::LockedTemporarily);
            }

            Ok(name)
        })();
        // Restore the weapon set before propagating results.
        self.weapon_set = weapon_set;
        let weapon_name = weapon_result?;

        // Clear undetected defector flag - firing reveals us
        self.friend_set_undetected_defector(false);

        // Fire weapon fired event
        self.fire_weapon_fired_event(&weapon_name, Some(target.get_id()));

        log::trace!(
            "Object {} fired weapon at object {}",
            self.id,
            target.get_id()
        );

        Ok(())
    }

    /// Get the last frame when this object fired a weapon
    /// Returns 0 if no firing tracker exists or never fired
    pub fn get_last_shot_fired_frame(&self) -> u32 {
        for entry in &self.update_module_handles {
            let mut last_frame: Option<u32> = None;
            entry.with_module(|module| {
                if let Some(tracker_module) = module_behavior_utility_kind(module)
                    .and_then(BehaviorUtilityModuleKindMut::into_firing_tracker)
                {
                    last_frame = Some(tracker_module.behavior().last_shot_frame());
                }
            });
            if let Some(frame) = last_frame {
                return frame;
            }
        }

        if let Some(tracker) = &self.firing_tracker {
            if let Ok(tracker_guard) = tracker.lock() {
                return tracker_guard.get_last_shot_frame();
            }
        }
        0
    }

    pub(super) fn all_weapon_fire_flags(slot: WeaponSlotType) -> ModelConditionFlags {
        match slot {
            WeaponSlotType::Primary => {
                ModelConditionFlags::FiringA
                    | ModelConditionFlags::BetweenFiringShotsA
                    | ModelConditionFlags::ReloadingA
                    | ModelConditionFlags::PreAttackA
                    | ModelConditionFlags::UsingWeaponA
            }
            WeaponSlotType::Secondary => {
                ModelConditionFlags::FiringB
                    | ModelConditionFlags::BetweenFiringShotsB
                    | ModelConditionFlags::ReloadingB
                    | ModelConditionFlags::PreAttackB
                    | ModelConditionFlags::UsingWeaponB
            }
            WeaponSlotType::Tertiary => {
                ModelConditionFlags::FiringC
                    | ModelConditionFlags::BetweenFiringShotsC
                    | ModelConditionFlags::ReloadingC
                    | ModelConditionFlags::PreAttackC
                    | ModelConditionFlags::UsingWeaponC
            }
        }
    }

    pub fn adjust_model_condition_for_weapon_status(&mut self) {
        let Some(drawable) = self.drawable.clone() else {
            return;
        };

        let now = crate::helpers::TheGameLogic::get_frame();
        let current_slot = self.weapon_set.get_current_weapon_slot();

        for slot_index in 0..WEAPONSLOT_COUNT {
            let slot = match slot_index {
                0 => WeaponSlotType::Primary,
                1 => WeaponSlotType::Secondary,
                _ => WeaponSlotType::Tertiary,
            };

            let weapon_data = self.weapon_set.get_weapon_in_slot(slot).map(|weapon| {
                (
                    weapon.get_remaining_ammo(),
                    weapon.get_template().clip_size as u32,
                    weapon.get_last_shot_frame(),
                    weapon.get_status(),
                )
            });
            let Some((remaining_ammo, clip_size, last_shot_frame, weapon_status)) = weapon_data
            else {
                self.last_weapon_condition[slot_index] =
                    crate::weapon::WeaponSetConditionType::None as u8;
                if let Err(err) = self.clear_and_set_model_condition_flags(
                    Self::all_weapon_fire_flags(slot),
                    ModelConditionFlags::empty(),
                ) {
                    log::debug!("Object::update_weapon_firing_status clear flags failed: {err}");
                }
                continue;
            };

            if let Ok(mut draw_guard) = drawable.write() {
                let common_slot = match slot {
                    WeaponSlotType::Primary => crate::common::WeaponSlotType::Primary,
                    WeaponSlotType::Secondary => crate::common::WeaponSlotType::Secondary,
                    WeaponSlotType::Tertiary => crate::common::WeaponSlotType::Tertiary,
                };
                draw_guard.update_drawable_clip_status(remaining_ammo, clip_size, common_slot);
            }

            let mut condition_to_set = if slot != current_slot {
                crate::weapon::WeaponSetConditionType::None
            } else if last_shot_frame == now {
                crate::weapon::WeaponSetConditionType::Firing
            } else if !self.test_status(ObjectStatusTypes::IsAttacking) {
                crate::weapon::WeaponSetConditionType::None
            } else {
                match weapon_status {
                    WeaponStatus::BetweenFiringShots => {
                        crate::weapon::WeaponSetConditionType::Between
                    }
                    WeaponStatus::ReloadingClip => crate::weapon::WeaponSetConditionType::Reloading,
                    WeaponStatus::PreAttack => crate::weapon::WeaponSetConditionType::PreAttack,
                    _ => crate::weapon::WeaponSetConditionType::None,
                }
            };

            if weapon_status == WeaponStatus::ReadyToFire
                && condition_to_set == crate::weapon::WeaponSetConditionType::None
                && self.test_status(ObjectStatusTypes::IsAttacking)
                && (self.test_status(ObjectStatusTypes::IsAimingWeapon)
                    || self.test_status(ObjectStatusTypes::IsFiringWeapon))
            {
                condition_to_set = crate::weapon::WeaponSetConditionType::Between;
            }

            let last_condition = self.last_weapon_condition[slot_index];
            if condition_to_set as u8 != last_condition {
                self.last_weapon_condition[slot_index] = condition_to_set as u8;
                let set_flags =
                    WeaponSet::get_model_condition_for_weapon_slot(slot, condition_to_set);
                if let Err(err) = self.clear_and_set_model_condition_flags(
                    Self::all_weapon_fire_flags(slot),
                    set_flags,
                ) {
                    log::debug!("Object::update_weapon_firing_status set flags failed: {err}");
                }
                self.stretch_preattack_animation(condition_to_set, slot);
            }
        }
    }

    /// Check if this object is currently attacking
    /// C++ Reference: Object.cpp - Combat state query
    ///
    /// # Returns
    /// * `true` - Object is currently attacking
    /// * `false` - Object is not attacking
    pub fn is_attacking(&self) -> bool {
        // Check multiple indicators of attack state
        // Matches C++ Object::isAttacking() behavior

        if let Some(ai) = self.get_ai_update_interface() {
            if let Ok(ai_guard) = ai.lock() {
                if ai_guard.is_attacking() {
                    return true;
                }
            }
        }

        // Status flags exposed by combat systems.
        if self.status.test(ObjectStatusTypes::IsAttacking)
            || self.status.test(ObjectStatusTypes::IsFiringWeapon)
        {
            return true;
        }

        if let Some((weapon, _slot)) = self.weapon_set.get_current_weapon() {
            if matches!(
                weapon.get_status(),
                crate::weapon::WeaponStatus::PreAttack
                    | crate::weapon::WeaponStatus::BetweenFiringShots
                    | crate::weapon::WeaponStatus::ReloadingClip
            ) {
                return true;
            }
        }

        // Check if we recently fired (within last second)
        let last_shot_frame = self.get_last_shot_fired_frame();
        if last_shot_frame > 0 {
            let current_frame = crate::helpers::TheGameLogic::get_frame();
            let frames_since_shot = current_frame.saturating_sub(last_shot_frame);
            // 30 frames = 1 second at 30 FPS
            if frames_since_shot < 30 {
                return true;
            }
        }

        false
    }

    // ========================================================================
    // WEAPON COMBAT (5 methods)
    // C++ Reference: Object.cpp getMostPercentReadyToFireAnyWeapon, etc.
    // ========================================================================

    pub fn get_most_percent_ready_to_fire_any_weapon(&self) -> f32 {
        self.weapon_set.get_most_percent_ready_to_fire_any_weapon()
    }

    pub fn get_weapon_in_weapon_slot_command_source_mask(&self, slot: WeaponSlotType) -> u32 {
        self.weapon_set.get_nth_command_source_mask(slot)
    }

    pub fn get_last_victim_id(&self) -> ObjectID {
        self.firing_tracker
            .as_ref()
            .and_then(|t| t.lock().ok())
            .map(|t| t.get_last_shot_victim())
            .unwrap_or(INVALID_ID)
    }

    pub fn find_waypoint_following_capable_weapon(&mut self) -> Option<&mut Weapon> {
        self.weapon_set.find_waypoint_following_capable_weapon()
    }

    pub fn clear_leech_range_mode_for_all_weapons(&mut self) {
        self.weapon_set.clear_leech_range_mode_for_all_weapons();
    }

    // ========================================================================
    // COUNTERMEASURES (3 methods)
    // C++ Reference: Object.cpp hasCountermeasures, reportMissileForCountermeasures, etc.
    // ========================================================================

    pub fn has_countermeasures(&self) -> bool {
        for behavior in &self.behaviors {
            let Ok(guard) = behavior.lock() else {
                continue;
            };
            if let Some(cbi) = guard.get_countermeasures_behavior_interface_const() {
                if cbi.is_active() {
                    return true;
                }
            }
        }
        false
    }

    pub fn report_missile_for_countermeasures(&self, missile_id: ObjectID) {
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if let Some(cbi) = guard.get_countermeasures_behavior_interface() {
                let _ = cbi.report_missile_for_countermeasures(missile_id);
            }
        }
    }

    pub fn get_countermeasures_behavior_interface(
        &self,
    ) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if guard.get_countermeasures_behavior_interface().is_some() {
                drop(guard);
                return Some(behavior.clone());
            }
        }
        None
    }

    pub fn get_num_consecutive_shots_fired_at_target(&self, victim_id: ObjectID) -> i32 {
        self.firing_tracker
            .as_ref()
            .and_then(|t| t.lock().ok())
            .map(|t| t.get_num_consecutive_shots_at_victim(victim_id))
            .unwrap_or(0)
    }
}
