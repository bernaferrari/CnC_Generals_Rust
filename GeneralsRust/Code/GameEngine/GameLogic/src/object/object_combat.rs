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

    /// C++ ActiveBody::setCorrectDamageState rubble effects. Used when the
    /// body cannot `try_write` this object because the caller already holds it.
    pub fn apply_structure_rubble_pose(&mut self) {
        if !self.is_kind_of(KindOf::Structure) {
            return;
        }
        let is_rubble = self
            .body
            .as_ref()
            .and_then(|body| {
                body.lock()
                    .ok()
                    .map(|guard| guard.get_damage_state() == crate::common::BodyDamageType::Rubble)
            })
            .unwrap_or(false);
        if !is_rubble {
            return;
        }
        let authored = self.get_template().structure_rubble_height().unwrap_or(0);
        let rubble_height = if authored > 0 {
            authored as f32
        } else {
            game_engine::common::global_data::read_safe()
                .map(|g| g.default_structure_rubble_height)
                .unwrap_or(1.0)
        };
        self.set_geometry_info_z(rubble_height);
        let object_id = self.get_id();
        let ai_store = crate::ai::the_ai();
        if let Ok(ai_guard) = ai_store.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf_guard) = pathfinder.write() {
                    pf_guard.remove_object_from_map(object_id, &[]);
                    pf_guard.add_object_to_map(object_id, &[], false);
                }
            }
        }
        self.set_status(crate::common::ObjectStatusMaskType::NO_COLLISIONS, true);
    }

    /// C++ ActiveBody.cpp:655-701 after doDamageFX. The body cannot lock this object.
    fn apply_post_damage_object_effects(&mut self, damage_info: &crate::damage::DamageInfo) {
        let enable_repulsors = crate::ai::the_ai()
            .read()
            .ok()
            .map(|ai| ai.get_ai_data().enable_repulsors)
            .unwrap_or(false);
        if enable_repulsors && self.is_kind_of(KindOf::CanBeRepulsed) {
            self.set_status(ObjectStatusTypes::Repulsor.into(), true);
        }
        let source_id = damage_info.input.source_id;
        // with_object would read-lock this object while attempt_damage holds the write.
        if source_id != crate::common::INVALID_ID && source_id != self.id {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(source_id, |damager| {
                crate::object::body::active_body::retaliate_nearby_friends(self, damager);
            });
        }
    }

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
        self.sync_effectively_dead_from_body();
        self.apply_structure_rubble_pose();

        Ok(())
    }
    pub fn attempt_healing_from_source_id(
        &mut self,
        amount: Real,
        source_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        self.sync_effectively_dead_from_body();
        self.apply_structure_rubble_pose();
        Ok(())
    }

    pub fn attempt_healing_from_sole_benefactor_id(
        &mut self,
        amount: Real,
        source_id: ObjectID,
        duration: UnsignedInt,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let now = TheGameLogic::get_frame();
        if now > self.sole_healing_benefactor_expiration_frame
            || self.sole_healing_benefactor_id == source_id
        {
            self.sole_healing_benefactor_id = source_id;
            self.sole_healing_benefactor_expiration_frame = now + duration;
            self.attempt_healing_from_source_id(amount, source_id)?;
            return Ok(true);
        }
        Ok(false)
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
            self.apply_structure_rubble_pose();

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

    pub fn notify_subdual_damage(&mut self, amount: Real) {
        if self.subdual_damage_helper.is_some() {
            let heal_rate = self
                .get_body_module()
                .and_then(|body| {
                    body.lock()
                        .ok()
                        .map(|guard| guard.get_subdual_damage_heal_rate())
                })
                .unwrap_or(0);
            if let Some(helper) = &mut self.subdual_damage_helper {
                helper.notify_subdual_damage(amount, heal_rate);
            }
        }

        if let Some(drawable) = self.get_drawable() {
            if let Ok(mut draw_guard) = drawable.write() {
                if amount > 0.0 {
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

    pub fn do_status_damage(&mut self, status: ObjectStatusTypes, duration: Real) {
        let Some(mut helper) = self.status_damage_helper.take() else {
            return;
        };
        helper.do_status_damage_in_owner(status, duration, self);
        self.status_damage_helper = Some(helper);
    }

    pub fn do_temp_weapon_bonus(
        &mut self,
        status: WeaponBonusConditionType,
        duration: UnsignedInt,
    ) {
        let Some(mut helper) = self.temp_weapon_bonus_helper.take() else {
            return;
        };
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        helper.do_temp_weapon_bonus_in_owner(status, duration, current_frame, self);
        self.temp_weapon_bonus_helper = Some(helper);
    }
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
            let flags = self.weapon_bonus_condition
                | crate::weapon::weapon_bonus::container_passenger_bonus_flags(
                    self.get_contained_by(),
                );
            let _ = self.weapon_set.weapon_set_on_weapon_bonus_change(flags);
        }
    }

    pub fn clear_weapon_bonus_condition(&mut self, condition: WeaponBonusConditionType) {
        // C++ Object.cpp:4663-4672
        let old = self.weapon_bonus_condition;
        self.weapon_bonus_condition.clear(condition);
        if old != self.weapon_bonus_condition {
            let flags = self.weapon_bonus_condition
                | crate::weapon::weapon_bonus::container_passenger_bonus_flags(
                    self.get_contained_by(),
                );
            let _ = self.weapon_set.weapon_set_on_weapon_bonus_change(flags);
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
        // if the other player is not a playable side (i.e. they are civilian, observer, whatever)
        // we shouldn't count the kill.
        if !victim
            .with_controlling_player(|g| g.is_playable_side())
            .unwrap_or(false)
        {
            return;
        }

        // Ignore kills on GUI-ignored objects
        if victim.is_kind_of(KindOf::IgnoredInGui) {
            return;
        }

        // Record object lost for victim's player
        victim.with_controlling_player_mut(|guard| {
            guard.get_score_keeper_mut().add_object_lost_obj(victim);
        });

        // Check relationship - only score kills on enemies
        let relationship = self.relationship_to(victim);
        if relationship != Relationship::Enemies {
            return;
        }

        // Don't count kills that I do on my own buildings or units, cause that's just silly.
        if let Some(controller_idx) = self.with_controlling_player(|g| g.get_player_index()) {
            if let Some(victim_idx) = victim.with_controlling_player(|g| g.get_player_index()) {
                if controller_idx == victim_idx {
                    return;
                }
            }
        }

        // Record kill for controlling player
        self.with_controlling_player_mut(|guard| {
            guard
                .get_score_keeper_mut()
                .add_object_destroyed_obj(victim);
            guard.add_skill_points_for_kill_obj(self, victim);
            guard.do_bounty_for_kill_obj(self, victim);
        });

        // Now handle experience, if we can gain any
        let template = self.get_template();
        let template_trainable = template.is_trainable();
        let required = [
            template.get_experience_required(0),
            template.get_experience_required(1),
            template.get_experience_required(2),
            template.get_experience_required(3),
        ];
        let victim_level = victim
            .experience_tracker
            .as_ref()
            .map(|tracker| tracker.get_veterancy_level() as usize)
            .unwrap_or(0);
        // C++ ExperienceTracker::getExperienceValue: ally → 0,
        // else template table at the victim's current level.
        // score_the_kill has already required Enemies.
        let experience_value = victim.get_template().get_experience_value(victim_level);
        let promotion = if let Some(tracker) = &mut self.experience_tracker {
            let accepting = template_trainable || tracker.has_experience_sink();
            if accepting {
                // srj sez: per dustin, no experience (et al) for killing things under construction.
                if !victim.test_status(ObjectStatusTypes::UnderConstruction) {
                    tracker
                        .add_experience_points_already_accepted(experience_value, true, &required)
                        .map(|old_level| (old_level, tracker.get_veterancy_level()))
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

    /// C++ Object::scoreTheKill when killer and victim are the same object.
    /// The playable/non-ignored victim loss is recorded before the relationship
    /// check; self-relationship means no killer rewards are issued.
    pub(crate) fn score_self_kill(&mut self) {
        if !self
            .with_controlling_player(|player| player.is_playable_side())
            .unwrap_or(false)
            || self.is_kind_of(KindOf::IgnoredInGui)
        {
            return;
        }
        self.with_controlling_player_mut(|player| {
            player.get_score_keeper_mut().add_object_lost_obj(self);
        });
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
            if let Ok(center) = crate::upgrade::center::get_upgrade_center().read() {
                if let Some(upgrade) = center.find_veterancy_upgrade(level_str) {
                    self.give_upgrade(&upgrade);
                }
            }
        }

        if new_level > old_level && provide_feedback {
            let mut sound = match new_level {
                VeterancyLevel::Veteran => self.get_template().get_sound_promoted_veteran(),
                VeterancyLevel::Elite => self.get_template().get_sound_promoted_elite(),
                VeterancyLevel::Heroic => self.get_template().get_sound_promoted_hero(),
                _ => crate::common::audio::AudioEventRts::default(),
            };
            sound.set_object_id(self.id as u32);
            if let Some(audio) = crate::helpers::TheAudio::get() {
                audio.add_audio_event(&sound);
            }
            let selected = crate::player::player_list()
                .read()
                .ok()
                .and_then(|list| {
                    let index = list.get_local_player_index();
                    if index < 0 {
                        return None;
                    }
                    let manager = crate::commands::selection::get_selection_manager();
                    let manager = manager.read().ok()?;
                    let selection = manager.get_player_selection_ref(index)?;
                    Some(selection.get_selected_objects())
                })
                .unwrap_or_default();
            let container = self.get_contained_by();
            if selected.contains(&self.id)
                || (selected.len() == 1 && container.is_some_and(|id| selected.contains(&id)))
            {
                crate::control_bar::mark_ui_dirty();
            }
        }

        // Notify body module (C++ lines 3018-3020)
        if let Some(body) = &self.body {
            if let Ok(mut body_guard) = body.lock() {
                let _ =
                    body_guard.on_veterancy_level_changed(old_level, new_level, provide_feedback);
            }
        }
        self.sync_effectively_dead_from_body();

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

            if let Some(data) = game_engine::common::ini::get_global_data() {
                let data = data.read();
                if !data.level_gain_animation_name.is_empty()
                    && game_engine::common::ini::get_anim2d_collection().is_some()
                {
                    crate::helpers::TheInGameUI::add_world_animation(
                        &data.level_gain_animation_name,
                        &pos_with_offset,
                        true,
                        data.level_gain_animation_display_time_in_seconds,
                        data.level_gain_animation_z_rise_per_second,
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

    pub(crate) fn sync_effectively_dead_from_body(&mut self) {
        let dead = self
            .body
            .as_ref()
            .and_then(|body| body.lock().ok())
            .map(|guard| guard.get_health() <= 0.0);
        if let Some(dead) = dead {
            if self.is_effectively_dead() != dead {
                self.set_effectively_dead(dead);
            }
        }
    }

    /// Run `f` with shared access to this object's experience tracker.
    ///
    /// Returns `None` when the object has no tracker. Replaces the former
    /// Arc-cloning `get_experience_tracker`, so no caller can retain a
    /// lockable handle to per-object state.
    pub fn with_experience_tracker<R>(
        &self,
        f: impl FnOnce(&ExperienceTracker) -> R,
    ) -> Option<R> {
        self.experience_tracker.as_deref().map(f)
    }

    /// Run `f` with exclusive access to this object's experience tracker.
    pub fn with_experience_tracker_mut<R>(
        &mut self,
        f: impl FnOnce(&mut ExperienceTracker) -> R,
    ) -> Option<R> {
        self.experience_tracker.as_deref_mut().map(f)
    }

    pub fn get_veterancy_level(&self) -> VeterancyLevel {
        if let Some(tracker) = &self.experience_tracker {
            return tracker.get_veterancy_level();
        }
        VeterancyLevel::Regular
    }

    /// C++ ExperienceTracker mutators fire `Object::onVeterancyLevelChanged`
    /// internally on every actual level change (ExperienceTracker.cpp:82-95 and
    /// :158-164; Object.h:213 `provideFeedback = TRUE` gates only the promotion
    /// anim + UnitPromoted sound — weapon-set flags, veterancy upgrade, body
    /// notify/healthBonus/armor always run, ActiveBody.cpp:1388-1477). The Rust
    /// tracker is split from side effects, so explicit level-set call sites must
    /// forward the promotion to keep the C++ fan-out.
    ///
    /// C++ parity for `ExperienceTracker::setVeterancyLevel` (explicit setting;
    /// ignores IsTrainable). Returns true when the level actually changed.
    pub fn set_veterancy_level_with_side_effects(
        &mut self,
        new_level: VeterancyLevel,
        provide_feedback: bool,
    ) -> bool {
        let old_level = self
            .with_experience_tracker_mut(|tracker| tracker.set_veterancy_level(new_level))
            .flatten();
        let Some(old_level) = old_level else {
            return false;
        };
        let current_level = self.get_veterancy_level();
        self.on_veterancy_level_changed(old_level, current_level, provide_feedback);
        true
    }

    /// C++ parity for `ExperienceTracker::setExperienceAndLevel` — demoting to
    /// Regular still fires `onVeterancyLevelChanged` (ExperienceTracker.cpp:169-207).
    pub fn set_experience_and_level_with_side_effects(
        &mut self,
        experience: i32,
        provide_feedback: bool,
    ) -> bool {
        let Some(experience_sink) = self.with_experience_tracker(|tracker| {
            tracker.get_experience_sink()
        }) else {
            return false;
        };

        // C++ reads IsTrainable and ExperienceRequired from this Object's
        // template. Looking the owner up through ExperienceTracker while this
        // Object is write-locked would fail that self-read and silently skip
        // the reset. Resolve the owned facts directly, as addExperience does.
        let (trainable, experience_required) = {
            let template = self.get_template();
            (
                template.is_trainable(),
                [
                    template.get_experience_required(0),
                    template.get_experience_required(1),
                    template.get_experience_required(2),
                    template.get_experience_required(3),
                ],
            )
        };

        // C++ forwards the call to the sink Object's tracker. Route through
        // that Object so its own template thresholds and level-change effects
        // are used; never apply the sink's returned transition to this source.
        if experience_sink != ExperienceTracker::INVALID_ID {
            if let Some(sink) = crate::helpers::TheGameLogic::find_object_by_id(experience_sink) {
                let Ok(mut sink_guard) = sink.write() else {
                    return false;
                };
                return sink_guard
                    .set_experience_and_level_with_side_effects(experience, provide_feedback);
            }
            // C++ falls through to this Object's own trainability/reset path
            // if the configured sink ID no longer resolves to a live Object.
        }

        if !trainable {
            return false;
        }

        let old_level = self
            .with_experience_tracker_mut(|tracker_guard| {
                tracker_guard
                    .set_experience_and_level_already_accepted(experience, &experience_required)
            })
            .flatten();
        let Some(old_level) = old_level else {
            return false;
        };
        let current_level = self.get_veterancy_level();
        self.on_veterancy_level_changed(old_level, current_level, provide_feedback);
        true
    }

    /// C++ parity for `ExperienceTracker::addExperiencePoints`:
    /// `canScaleForBonus` defaults TRUE (ExperienceTracker.h:32) and promotion
    /// fires `onVeterancyLevelChanged` (ExperienceTracker.cpp:158-164).
    pub fn add_experience_points_with_side_effects(
        &mut self,
        experience_gain: i32,
        can_scale_for_bonus: bool,
    ) -> bool {
        let template_trainable = self.get_template().is_trainable();
        let required = [
            self.get_template().get_experience_required(0),
            self.get_template().get_experience_required(1),
            self.get_template().get_experience_required(2),
            self.get_template().get_experience_required(3),
        ];
        let old_level = self
            .with_experience_tracker_mut(|tracker_guard| {
                if !template_trainable && !tracker_guard.has_experience_sink() {
                    return None;
                }
                tracker_guard.add_experience_points_already_accepted(
                    experience_gain,
                    can_scale_for_bonus,
                    &required,
                )
            })
            .flatten();
        let Some(old_level) = old_level else {
            return false;
        };
        let current_level = self.get_veterancy_level();
        self.on_veterancy_level_changed(old_level, current_level, true);
        true
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

                let source_pos = *self.get_position();
                weapon.set_caller_held_source(self.id, source_pos);
                weapon.set_caller_veterancy(self.get_veterancy_level());
                weapon.set_caller_team(self.get_team());
                if let Some(player) = self.get_controlling_player() {
                    weapon.set_caller_player(Some(std::sync::Arc::clone(&player)));
                    if let Ok(guard) = player.try_read() {
                        weapon.set_caller_player_mask(guard.get_player_mask());
                    }
                }
                if let Some(drawable) = self.get_drawable() {
                    if let Ok(draw) = drawable.try_read() {
                        weapon.set_caller_barrel_count(
                            draw.get_barrel_count(weapon.get_weapon_slot()),
                        );
                    }
                }
                let reloaded = weapon.fire_weapon_at_position_with_bonus_and_reload_flag(
                    self.id,
                    pos,
                    source_bonus_flags,
                    container_bonus_flags,
                );
                weapon.clear_caller_held_source();
                let reloaded =
                    reloaded.map_err(|e| ObjectError::WeaponFireFailed(e.to_string()))?;

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
        weapon_set.apply_pending_shared_fire();
        self.weapon_set = weapon_set;
        self.record_pending_mine_cleared();
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

            let source_pos = *self.get_position();
            weapon.set_caller_held_source(self.id, source_pos);
            weapon.set_caller_veterancy(self.get_veterancy_level());
            weapon.set_caller_team(self.get_team());
            if let Some(player) = self.get_controlling_player() {
                weapon.set_caller_player(Some(std::sync::Arc::clone(&player)));
                if let Ok(guard) = player.try_read() {
                    weapon.set_caller_player_mask(guard.get_player_mask());
                }
            }
            if let Some(drawable) = self.get_drawable() {
                if let Ok(draw) = drawable.try_read() {
                    weapon.set_caller_barrel_count(draw.get_barrel_count(weapon.get_weapon_slot()));
                }
            }
            let reloaded = weapon.fire_weapon_at_position_with_bonus_and_reload_flag(
                self.id,
                pos,
                source_bonus_flags,
                container_bonus_flags,
            );
            weapon.clear_caller_held_source();
            let reloaded = reloaded.map_err(|e| ObjectError::WeaponFireFailed(e.to_string()))?;

            self.notify_firing_tracker_shot_fired(weapon, INVALID_ID);

            let name = weapon.get_name().to_string();
            Ok((name, reloaded))
        })();
        weapon_set.apply_pending_shared_fire();
        self.weapon_set = weapon_set;
        self.record_pending_mine_cleared();
        let (weapon_name, reloaded) = weapon_result?;

        if reloaded {
            self.weapon_set
                .release_weapon_lock(WeaponLockType::LockedTemporarily);
        }

        self.friend_set_undetected_defector(false);
        self.fire_weapon_fired_event(&weapon_name, None);
        Ok(())
    }
    /// C++ linked turrets call `Weapon::fireWeapon` directly. That is not
    /// `fireCurrentWeapon`: no defector clear, no fired-event, and the tracker
    /// runs even when the shot is refused after leech/assist.
    pub fn fire_linked_turret_slot_at_position(&mut self, slot: WeaponSlotType, pos: &Coord3D) {
        if dual_world_registry_unavailable() {
            return;
        }

        let mut weapon_set = std::mem::take(&mut self.weapon_set);
        let reloaded = if let Some(weapon) = weapon_set.get_weapon_in_slot_mut(slot) {
            let source_pos = *self.get_position();
            let mut flags =
                crate::weapon::helpers::map_common_bonus_flags(self.get_weapon_bonus_condition());
            let container = crate::weapon::weapon_bonus::container_passenger_bonus_flags(
                self.get_contained_by(),
            );
            flags.union(crate::weapon::helpers::map_common_bonus_flags(container));
            weapon.set_caller_held_source(self.id, source_pos);
            weapon.set_caller_veterancy(self.get_veterancy_level());
            weapon.set_caller_team(self.get_team());
            if let Some(player) = self.get_controlling_player() {
                weapon.set_caller_player(Some(std::sync::Arc::clone(&player)));
                if let Ok(guard) = player.try_read() {
                    weapon.set_caller_player_mask(guard.get_player_mask());
                }
            }
            if let Some(drawable) = self.get_drawable() {
                if let Ok(draw) = drawable.try_read() {
                    weapon.set_caller_barrel_count(draw.get_barrel_count(weapon.get_weapon_slot()));
                }
            }
            match weapon.fire_weapon_at_position_no_range_check(self.id, pos, flags) {
                Ok(reloaded) => {
                    weapon.clear_caller_held_source();
                    self.notify_firing_tracker_shot_fired(weapon, INVALID_ID);
                    reloaded
                }
                Err(_) => {
                    weapon.clear_caller_held_source();
                    false
                }
            }
        } else {
            false
        };
        weapon_set.apply_pending_shared_fire();
        self.weapon_set = weapon_set;
        self.record_pending_mine_cleared();
        if reloaded {
            self.weapon_set
                .release_weapon_lock(WeaponLockType::LockedTemporarily);
        }
    }

    fn record_pending_mine_cleared(&mut self) {
        if let Some(mut info) = self.weapon_set.take_pending_self_damage() {
            let _ = self.attempt_damage(&mut info);
        }
        if !self.weapon_set.take_pending_mine_cleared() {
            return;
        }
        let Some(player) = self.get_controlling_player() else {
            return;
        };
        if let Ok(mut player_guard) = player.write() {
            player_guard.get_academy_stats_mut().record_mine_cleared();
        }
    }

    pub fn pre_fire_current_weapon(&mut self, victim: Option<ObjectID>) {
        let mut weapon_set = std::mem::take(&mut self.weapon_set);
        let started = if let Some(weapon) = weapon_set.get_current_weapon_mut() {
            let next_frame = TheGameLogic::get_frame().saturating_add(1);
            if next_frame >= weapon.get_possible_next_shot_frame() {
                let victim_id = victim.unwrap_or(INVALID_ID);
                let consecutive = self.get_num_consecutive_shots_fired_at_target(victim_id);
                let mut flags = crate::weapon::helpers::map_common_bonus_flags(
                    self.get_weapon_bonus_condition(),
                );
                let container = crate::weapon::weapon_bonus::container_passenger_bonus_flags(
                    self.get_contained_by(),
                );
                flags.union(crate::weapon::helpers::map_common_bonus_flags(container));
                let _ = weapon.pre_fire_weapon_with_consecutive(
                    self.id,
                    victim_id,
                    Some(consecutive),
                    Some(flags),
                );
                true
            } else {
                false
            }
        } else {
            false
        };
        self.weapon_set = weapon_set;
        if started {
            self.friend_set_undetected_defector(false);
        }
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
        let handles = self.update_module_handles.clone();
        for entry in handles {
            let mut used = false;
            entry.with_module(|module| {
                if let Some(tracker_module) = module_behavior_utility_kind(module)
                    .and_then(BehaviorUtilityModuleKindMut::into_firing_tracker)
                {
                    tracker_module
                        .behavior_mut()
                        .shot_fired_with_owner(self, weapon, victim_id);
                    used = true;
                }
            });
            if used {
                handled = true;
                break;
            }
        }

        if !handled {
            if let Some(mut tracker) = self.firing_tracker.take() {
                tracker.shot_fired_with_owner(self, weapon, victim_id);
                self.firing_tracker = Some(tracker);
            }
        }
    }

    /// C++ `AIAttackFireWeaponState` linked-turret position shot (`AIStates.cpp:5268-5280`).
    pub fn fire_linked_turrets_at_position(&mut self, position: &Coord3D) {
        for slot in [
            crate::weapon::WeaponSlotType::Primary,
            crate::weapon::WeaponSlotType::Secondary,
            crate::weapon::WeaponSlotType::Tertiary,
        ] {
            self.fire_linked_turret_slot_at_position(slot, position);
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
        // C++ Object.cpp:3153-3307.
        if self.test_status(ObjectStatusTypes::NoAttack) {
            return false;
        }
        if let Some(container_id) = self.get_contained_by() {
            if let Some(container) = crate::helpers::TheGameLogic::find_object_by_id(container_id) {
                if let Ok(guard) = container.try_read() {
                    if let Some(contain) = guard.get_contain() {
                        if let Ok(contain) = contain.try_lock() {
                            if !contain.is_passenger_allowed_to_fire(Some(self.id)) {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        if self.test_status(ObjectStatusTypes::UnderConstruction)
            || self.test_status(ObjectStatusTypes::Sold)
            || self.is_disabled_by_type(DisabledType::DisabledSubdued)
        {
            return false;
        }
        if self.is_kind_of(KindOf::PortableStructure)
            || self.is_kind_of(KindOf::SpawnsAreTheWeapons)
        {
            if self.is_disabled_by_type(DisabledType::DisabledHacked)
                || self.is_disabled_by_type(DisabledType::DisabledEmp)
            {
                return false;
            }
            if self.is_kind_of(KindOf::Infantry) {
                let slaver_subdued = self
                    .with_slaved_update_interface(|slaved| slaved.slaver_id())
                    .flatten()
                    .and_then(crate::helpers::TheGameLogic::find_object_by_id)
                    .and_then(|slaver| {
                        let guard = slaver.try_read().ok()?;
                        Some(guard.is_disabled_by_type(DisabledType::DisabledSubdued))
                    })
                    .unwrap_or(false);
                if slaver_subdued {
                    return false;
                }
            }
        }
        if !self.is_kind_of(KindOf::CanAttack) {
            if let Some(ai) = self.get_ai() {
                if let Ok(ai) = ai.try_lock() {
                    let mut any_weapon = false;
                    let mut any_enabled = false;
                    for slot in [
                        WeaponSlotType::Primary,
                        WeaponSlotType::Secondary,
                        WeaponSlotType::Tertiary,
                    ] {
                        if self.get_weapon_in_weapon_slot(slot).is_none() {
                            continue;
                        }
                        any_weapon = true;
                        let turret = ai.get_which_turret_for_weapon_slot(slot);
                        if turret == crate::common::types::TurretType::Invalid
                            || ai.is_turret_enabled(turret)
                        {
                            any_enabled = true;
                            break;
                        }
                    }
                    if any_weapon && !any_enabled {
                        return false;
                    }
                }
            }
        }
        if self.is_kind_of(KindOf::CanAttack) || self.test_status(ObjectStatusTypes::CanAttack) {
            return true;
        }
        if let Some(contain) = self.get_contain() {
            if let Ok(contain) = contain.try_lock() {
                if contain.is_passenger_allowed_to_fire(Some(self.id))
                    && contain.get_contain_count() > 0
                {
                    return true;
                }
            }
        }
        if self.get_ai().is_some() && self.has_any_weapon() {
            return true;
        }
        if self
            .with_spawn_behavior_full_interface(|spawn| spawn.can_any_slaves_attack())
            .unwrap_or(false)
        {
            return true;
        }
        self.get_template().is_enter_guard()
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
        let flags = self.weapon_bonus_condition
            | crate::weapon::weapon_bonus::container_passenger_bonus_flags(self.get_contained_by());
        self.weapon_set
            .reload_all_ammo_with_flags(self.id, flags, now)
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
        crate::object::registry::OBJECT_REGISTRY.get_object(victim_id)
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
    pub fn set_weapon_lock(
        &mut self,
        weapon_slot: WeaponSlotType,
        lock_type: WeaponLockType,
    ) -> bool {
        let locked = self.weapon_set.set_weapon_lock(weapon_slot, lock_type);
        if !locked {
            log::debug!(
                "Object {} failed to set weapon lock {:?} on slot {:?}",
                self.id,
                lock_type,
                weapon_slot
            );
        }
        locked
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
    /// * `Err(ObjectError::NoBodyModule)` - Object has no body module
    ///
    /// # Behavior
    /// - Sets health to max_health
    /// - Fires healing event
    /// - Dead non-bridges are ignored by the body; dead bridges still heal
    pub fn heal_completely(&mut self) -> Result<(), ObjectError> {
        // C++ Object::healCompletely is attemptHealing(HUGE_DAMAGE_AMOUNT, NULL).
        // The body returns for a dead non-bridge and still heals a dead bridge.

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
        self.apply_structure_rubble_pose();

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
        let health_before_damage = self.get_health();
        if damage_info.input.source_id == self.id {
            damage_info.input.source_template = Some(self.get_template().clone());
        }

        // C++ does not reject a negative non-healing amount here. Armor clamps
        // it to 0, except unresistable, and doDamageFX still runs.

        // Delegate to body module for damage processing
        if let Some(body) = &self.body {
            let mut body_guard = body.lock().map_err(|_| ObjectError::LockPoisoned)?;
            let body_context = crate::object::body::BodyDamageContext {
                owner_is_preferred_source: self.is_kind_of(KindOf::Vehicle)
                    || self.is_kind_of(KindOf::Infantry)
                    || self.is_faction_structure(),
            };

            body_guard
                .attempt_damage_with_context(damage_info, &body_context)
                .map_err(|e| ObjectError::BodyModuleError(e.to_string()))?;
            drop(body_guard);
        }
        self.apply_structure_rubble_pose();
        // C++ ActiveBody.cpp:649-653: onDie, then doDamageFX. The body skipped
        // FX because this caller holds the object write lock.
        if self.get_health() <= 0.0 {
            if health_before_damage > 0.0 && damage_info.input.source_id != INVALID_ID {
                if damage_info.input.source_id == self.id {
                    self.score_self_kill();
                } else {
                    let _ = crate::object::registry::OBJECT_REGISTRY
                        .with_object_mut(damage_info.input.source_id, |damager| {
                            damager.score_the_kill(self)
                        });
                }
            }
            self.handle_death(Some(damage_info));
            if let Some(body) = &self.body {
                if let Ok(mut body_guard) = body.lock() {
                    body_guard.do_damage_fx_after_death(damage_info);
                }
            }
        }
        self.apply_post_damage_object_effects(damage_info);

        if let Some(contain) = &self.contain {
            if let Ok(mut contain_guard) = contain.lock() {
                if let Err(err) = contain_guard.on_damage(damage_info) {
                    log::warn!("Object {} contain on_damage failed: {}", self.id, err);
                }
            }
        }

        // Process shockwave forces (C++ Object.cpp:1800-1835).
        // The impulse depends only on DamageInfo input. Copy it out while no
        // body guard is live, then lock physics. Physics update holds this
        // mutex and can lock the body; holding body across this lock deadlocks.
        if damage_info.input.shock_wave_amount > 0.0 && damage_info.input.shock_wave_radius > 0.0 {
            // Check if object is eligible for shockwave (not airborne, not projectile)
            if self.shockwave_applies() {
                let shock_wave_length = damage_info.input.shock_wave_vector.length();
                let distance_from_center =
                    (shock_wave_length / damage_info.input.shock_wave_radius).min(1.0);
                let distance_taper =
                    distance_from_center * (1.0 - damage_info.input.shock_wave_taper_off);
                let shock_taper_mult = 1.0 - distance_taper;

                let mut shock_wave_force = damage_info.input.shock_wave_vector;
                let _ = shock_wave_force.normalize();
                shock_wave_force *= damage_info.input.shock_wave_amount * shock_taper_mult;
                shock_wave_force.z = shock_wave_force.length();

                // Clone the Arc so the physics lock ends before the model-condition
                // write. Do not borrow self.physics across set_shockwave_stunned_flailing.
                let physics = self.physics.clone();
                let shocked = if let Some(physics) = physics {
                    if let Ok(mut physics_guard) = physics.lock() {
                        physics_guard.apply_shock(&shock_wave_force);
                        physics_guard.apply_random_rotation();
                        physics_guard.set_stunned(true);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if shocked {
                    self.set_shockwave_stunned_flailing();
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
                let under_attack_local = self
                    .get_controlling_player()
                    .and_then(|player| {
                        player.read().ok().map(|guard| {
                            !damage_info
                                .input
                                .source_player_mask
                                .intersects(guard.get_player_mask())
                                && guard.is_local_player()
                        })
                    })
                    .unwrap_or(false);
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
        self.fire_current_weapon_at_target_id(target.get_id())
    }

    pub fn fire_current_weapon_at_target_id(
        &mut self,
        target_id: ObjectID,
    ) -> Result<(), ObjectError> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let destroyed = crate::helpers::TheGameLogic::find_object_by_id(target_id)
            .and_then(|arc| arc.read().ok().map(|guard| guard.is_destroyed()))
            .unwrap_or(true);
        if destroyed {
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
                if weapon.get_status() != WeaponStatus::ReadyToFire {
                    return Err(ObjectError::WeaponNotReady);
                }
                let source_pos = *self.get_position();
                weapon.set_caller_held_source(self.id, source_pos);
                weapon.set_caller_veterancy(self.get_veterancy_level());
                weapon.set_caller_team(self.get_team());
                if let Some(player) = self.get_controlling_player() {
                    weapon.set_caller_player(Some(std::sync::Arc::clone(&player)));
                    if let Ok(guard) = player.try_read() {
                        weapon.set_caller_player_mask(guard.get_player_mask());
                    }
                }
                if let Some(drawable) = self.get_drawable() {
                    if let Ok(draw) = drawable.try_read() {
                        weapon.set_caller_barrel_count(
                            draw.get_barrel_count(weapon.get_weapon_slot()),
                        );
                    }
                }
                let reloaded = weapon.fire_weapon_with_bonus_and_reload_flag(
                    self.id,
                    target_id,
                    current_frame,
                    source_bonus_flags,
                    container_bonus_flags,
                );
                weapon.clear_caller_held_source();
                let reloaded =
                    reloaded.map_err(|e| ObjectError::WeaponFireFailed(e.to_string()))?;

                // Notify firing tracker for statistics
                // Note: C++ Object.cpp does NOT set OBJECT_STATUS_IS_FIRING_WEAPON here;
                // that is done in AIUpdate, not in fireCurrentWeapon.
                self.notify_firing_tracker_shot_fired(weapon, target_id);
                (weapon.get_name().to_string(), reloaded)
            };

            if reloaded {
                weapon_set.release_weapon_lock(WeaponLockType::LockedTemporarily);
            }

            Ok(name)
        })();
        // Restore the weapon set before propagating results.
        weapon_set.apply_pending_shared_fire();
        self.weapon_set = weapon_set;
        self.record_pending_mine_cleared();
        let weapon_name = weapon_result?;

        // Clear undetected defector flag - firing reveals us
        self.friend_set_undetected_defector(false);

        // Fire weapon fired event
        self.fire_weapon_fired_event(&weapon_name, Some(target_id));

        log::trace!("Object {} fired weapon at object {}", self.id, target_id);

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
            return tracker.get_last_shot_frame();
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

    pub fn get_most_percent_ready_to_fire_any_weapon(&self) -> u32 {
        self.weapon_set.get_most_percent_ready_to_fire_any_weapon()
    }

    pub fn get_weapon_in_weapon_slot_command_source_mask(&self, slot: WeaponSlotType) -> u32 {
        self.weapon_set.get_nth_command_source_mask(slot)
    }

    pub fn get_last_victim_id(&self) -> ObjectID {
        self.firing_tracker
            .as_ref()
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
        for entry in &self.update_module_handles {
            let mut count: Option<i32> = None;
            entry.with_module(|module| {
                if let Some(tracker_module) = module_behavior_utility_kind(module)
                    .and_then(BehaviorUtilityModuleKindMut::into_firing_tracker)
                {
                    count = Some(
                        tracker_module
                            .behavior()
                            .get_num_consecutive_shots_at_victim(victim_id),
                    );
                }
            });
            if let Some(count) = count {
                return count;
            }
        }
        self.firing_tracker
            .as_ref()
            .map(|t| t.get_num_consecutive_shots_at_victim(victim_id))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod veterancy_side_effect_tests {
    use super::*;
    use crate::common::DefaultThingTemplate;
    use crate::experience::ExperienceTracker;

    fn tracked_object(id: ObjectID) -> Object {
        let mut obj = Object::new_test(id, 100.0);
        let mut template = DefaultThingTemplate::new(format!("TrainableExperience{id}"));
        let mut fields = std::collections::HashMap::new();
        fields.insert("IsTrainable".to_string(), "Yes".to_string());
        fields.insert(
            "ExperienceRequired".to_string(),
            "0 100 300 600".to_string(),
        );
        template.parse_object_fields_from_ini(&fields);
        obj.set_template_for_test(Arc::new(template));
        obj.experience_tracker = Some(Box::new(ExperienceTracker::new(id)));
        obj
    }

    #[test]
    fn add_experience_applies_cpp_default_scalar_and_side_effects() {
        // C++ addExperiencePoints defaults canScaleForBonus = TRUE
        // (ExperienceTracker.h:32) and fires Object::onVeterancyLevelChanged on
        // promotion (ExperienceTracker.cpp:158-164): 50 XP × scalar 2.0 = 100,
        // the DEFAULT_EXPERIENCE_REQUIRED Veteran threshold.
        let mut obj = tracked_object(4242);
        if let Some(tracker) = &mut obj.experience_tracker {
            tracker.set_experience_scalar(2.0);
        }

        assert!(obj.add_experience_points_with_side_effects(50, true));
        assert_eq!(obj.get_veterancy_level(), VeterancyLevel::Veteran);
        assert!(
            obj.test_weapon_set_flag(WeaponSetType::Veteran),
            "promotion must swap the weapon set, not just the tracker level"
        );
    }

    #[test]
    fn add_experience_without_level_change_fires_no_side_effects() {
        let mut obj = tracked_object(4243);

        assert!(!obj.add_experience_points_with_side_effects(10, true));
        assert_eq!(obj.get_veterancy_level(), VeterancyLevel::Regular);
        assert!(!obj.test_weapon_set_flag(WeaponSetType::Veteran));
    }

    #[test]
    fn set_veterancy_level_side_effects_forward_feedback_flag() {
        // Demotion path: C++ setVeterancyLevel fires onVeterancyLevelChanged
        // whenever the level differs (ExperienceTracker.cpp:87-95) and the fan-out
        // clears the previous level's weapon-set flags (Object.cpp:3064-3160).
        let mut obj = tracked_object(4244);
        assert!(obj.set_veterancy_level_with_side_effects(VeterancyLevel::Elite, false));
        assert!(obj.test_weapon_set_flag(WeaponSetType::Elite));
        assert!(obj.set_veterancy_level_with_side_effects(VeterancyLevel::Regular, false));
        assert!(!obj.test_weapon_set_flag(WeaponSetType::Elite));
        // Same-level set is a no-op in C++ (level equality short-circuit).
        assert!(!obj.set_veterancy_level_with_side_effects(VeterancyLevel::Regular, false));
    }
}
