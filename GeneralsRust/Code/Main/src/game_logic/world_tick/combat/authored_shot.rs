//! Direct-combat discharge of an authored WeaponTemplate.
//!
//! Mirrors C++ `Object::fireCurrentWeapon` -> `Weapon::fireWeapon` ->
//! `Weapon::privateFireWeapon` / `WeaponTemplate::fireWeaponTemplate`
//! (GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp:889-1100 and
//! 2617-2669) and `FiringTracker::shotFired` (FiringTracker.cpp:60-160):
//!
//! 1. FireFX, then FireOCL, at the source (`deliver_owned_weapon_shot`).
//! 2. Delivery from the frozen WeaponTemplate: a ProjectileObject launch, a
//!    WeaponSpeed-delayed damage, or instant `dealDamageInternal` with the
//!    authored primary/secondary radii, DamageType, DeathType and status.
//! 3. Ammo, reload timers, shot count and FiringTracker commit after the
//!    template has fired (`commit_deferred_weapon_fire_after_effects`).
//!
//! No unit, template or weapon name selects behavior here: everything comes
//! from the receiving session's WeaponTemplate selected for the slot. This is
//! the same shot operation the nested AttackStateMachine and passive weapons
//! use, so every attacker shares one authored delivery path.
use super::*;

impl GameLogic {
    /// True when `slot` of `attacker_id` fires an admitted WeaponTemplate.
    pub(super) fn slot_fires_authored_weapon(&self, attacker_id: ObjectId, slot: u8) -> bool {
        self.objects
            .get(&attacker_id)
            .is_some_and(|attacker| attacker.selected_native_weapon_rule(slot).is_some())
    }

    /// Fire `slot`'s authored weapon at `target_id` through the shared shot
    /// operation. Returns `NoShot` when the weapon declines to fire (not
    /// ready, pre-attack wind-up, jammed); otherwise the shot is fully
    /// delivered and committed, so the caller must not commit it again.
    pub(super) fn fire_authored_object_shot_with_effects(
        &mut self,
        input: CombatAttackInput,
        target_id: ObjectId,
        slot: u8,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> CombatShotResult {
        let attacker_id = input.attacker_id;
        let (victim_infantry, victim_faerie) = self
            .objects
            .get(&target_id)
            .map_or((false, false), |victim| {
                (victim.is_kind_of(KindOf::Infantry), victim.is_faerie_fire())
            });
        let projectile_team_id = self
            .objects
            .get(&attacker_id)
            .and_then(|object| object.owner_player_id)
            .and_then(|player_id| self.host_default_team_id(player_id));
        let Some((rates, bonus, origin)) = self.objects.get(&attacker_id).map(|attacker| {
            let rates = self.receiving_weapon_rates_of_fire(attacker);
            let bonus = self.receiving_weapon_bonus_fields_for_slot(attacker, slot);
            (rates, bonus, attacker.get_position())
        }) else {
            return CombatShotResult::NoShot;
        };
        let prepared = {
            let Some(attacker) = self.objects.get_mut(&attacker_id) else {
                return CombatShotResult::NoShot;
            };
            attacker.prepare_passive_weapon_shot_with_operation_and_solo_set(
                slot,
                target_id,
                input.current_time,
                victim_infantry,
                victim_faerie,
                self.frame,
                origin,
                &mut self.combat_system.fire_sound_loop_events,
                &mut self.attack_events,
                self.weapon_definitions
                    .as_ref()
                    .map(|_| &self.projectile_definitions),
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                self.frame,
                &mut effects.reborrow_for_object(&mut self.partition_manager),
                &self.solo_weapon_bonus_set,
                bonus.0,
                bonus.3,
                rates,
                &self.logic_random,
                projectile_team_id,
                false,
            )
        };
        let Some((fired_slot, shot)) = prepared else {
            return CombatShotResult::NoShot;
        };
        self.deliver_and_commit_authored_shot(attacker_id, fired_slot, shot, effects);
        self.queue_authored_shot_presentation(attacker_id, fired_slot, Some(target_id), None, effects);
        // C++ AssaultTransportAIUpdate (AssaultTransportAIUpdate.cpp:150-260)
        // sends the transport's passengers at the victim while it attacks.
        // Keyed on the authored module, never the transport's name.
        if self.objects.get(&attacker_id).is_some_and(|attacker| {
            attacker
                .thing()
                .template
                .authors_behavior_module("AssaultTransportAIUpdate")
        }) {
            let _ = self.apply_troop_crawler_assault_deploy_with_effects(
                attacker_id,
                target_id,
                effects,
            );
        }
        CombatShotResult::Delivered
    }

    /// Fire `slot`'s authored weapon at a ground position (force-attack ground,
    /// C++ `Weapon::fireWeapon(source, pos)`) through the shared operation.
    pub(super) fn fire_authored_ground_shot_with_effects(
        &mut self,
        input: CombatAttackInput,
        target_location: Vec3,
        slot: u8,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> CombatShotResult {
        let attacker_id = input.attacker_id;
        let projectile_team_id = self
            .objects
            .get(&attacker_id)
            .and_then(|object| object.owner_player_id)
            .and_then(|player_id| self.host_default_team_id(player_id));
        let Some((rates, bonus, origin)) = self.objects.get(&attacker_id).map(|attacker| {
            let rates = self.receiving_weapon_rates_of_fire(attacker);
            let bonus = self.receiving_weapon_bonus_fields_for_slot(attacker, slot);
            (rates, bonus, attacker.get_position())
        }) else {
            return CombatShotResult::NoShot;
        };
        let prepared = {
            let Some(attacker) = self.objects.get_mut(&attacker_id) else {
                return CombatShotResult::NoShot;
            };
            attacker.prepare_passive_position_shot_with_operation_and_solo_set(
                slot,
                target_location,
                input.current_time,
                false,
                false,
                self.frame,
                origin,
                &mut self.combat_system.fire_sound_loop_events,
                &mut self.attack_events,
                self.weapon_definitions
                    .as_ref()
                    .map(|_| &self.projectile_definitions),
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                self.frame,
                &mut effects.reborrow_for_object(&mut self.partition_manager),
                &self.solo_weapon_bonus_set,
                bonus.0,
                bonus.3,
                rates,
                &self.logic_random,
                projectile_team_id,
            )
        };
        let Some((fired_slot, shot)) = prepared else {
            return CombatShotResult::NoShot;
        };
        self.deliver_and_commit_authored_shot(attacker_id, fired_slot, shot, effects);
        self.queue_authored_shot_presentation(
            attacker_id,
            fired_slot,
            None,
            Some(target_location),
            effects,
        );
        // AIAttackState: a ground command whose MaxShotsToFire are spent ends.
        let spent = if let Some(attacker) = self.objects.get_mut(&attacker_id) {
            let spent = attacker.max_shots_to_fire == 0;
            if spent {
                attacker.target_location = None;
                attacker.set_force_attack(false);
                attacker.set_ai_state(
                    AIState::Idle,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            spent
        } else {
            false
        };
        if spent {
            self.set_turret_target_position(attacker_id, None);
        }
        CombatShotResult::Delivered
    }

    /// FireFX -> FireOCL -> delivery, then the post-fire ammo/timer commit
    /// (Weapon.cpp:889-949, 2617-2669) for a prepared authored shot.
    fn deliver_and_commit_authored_shot(
        &mut self,
        attacker_id: ObjectId,
        slot: u8,
        shot: crate::game_logic::combat::PendingProjectile,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let capture = self
            .objects
            .get_mut(&attacker_id)
            .and_then(Object::take_pending_weapon_visual_capture);
        if !self.deliver_owned_weapon_shot_with_effects(slot, shot, capture, effects) {
            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                attacker.advance_weapon_barrel_after_shot(slot);
            }
        }
        self.commit_owned_weapon_fire_after_effects(attacker_id);
    }

    pub(in crate::game_logic) fn commit_owned_weapon_fire_after_effects(&mut self, id: ObjectId) {
        // reloadAmmo recomputes bonuses after fireWeaponTemplate callbacks;
        // between-shot delay uses the bonus frozen for the actual shot.
        let rates = self.objects.get(&id).map(|object| self.receiving_weapon_rates_of_fire(object));
        let committed = self.objects.get_mut(&id).and_then(|object| {
            object.commit_deferred_weapon_fire_after_effects(
                &mut self.combat_system.fire_sound_loop_events,
                Some(&self.solo_weapon_bonus_set), Some(&self.logic_random),
                rates.as_ref(),
            )
        });
        let Some((slot, target, name)) = committed else { return };
        self.record_owned_weapon_shot(id, slot, target, name.as_deref());
    }

    /// FiringTracker.cpp:66-160: condition callbacks precede the coast stamp,
    /// then speedUp/coolDown emit each changed bit in original source order.
    fn record_owned_weapon_shot(&mut self, id: ObjectId, slot: u8, target: Option<ObjectId>, name: Option<&str>) {
        let faerie = target.and_then(|id| self.objects.get(&id)).is_some_and(Object::is_faerie_fire);
        let Some(object) = self.objects.get_mut(&id) else { return };
        if object.weapon_bonus_target_faerie_fire != faerie {
            object.weapon_bonus_target_faerie_fire = faerie;
            self.notify_owned_weapon_bonus_change(id);
        }
        let Some(object) = self.objects.get_mut(&id) else { return };
        let rule = object.selected_native_weapon_rule(slot);
        let Some(rule) = rule else {
            let next = object.possible_next_shot_frame_for_slot_with_solo_set(
                slot, self.frame, faerie, &self.solo_weapon_bonus_set,
            );
            object.record_shot_at_target_with_optional_solo_set(
                target, self.frame, next, Some(&self.solo_weapon_bonus_set), None, None,
            );
            object.stamp_fire_sound_loop_after_shot(
                self.frame, name, &mut self.combat_system.fire_sound_loop_events,
            );
            object.stamp_auto_reload_when_idle_from_slot(slot, self.frame);
            return;
        };
        if object.consecutive_shot_target == target || self.frame < object.continuous_fire_coast_until_frame {
            object.consecutive_shots_at_target = object.consecutive_shots_at_target.saturating_add(1);
        } else {
            object.consecutive_shots_at_target = 1;
        }
        object.consecutive_shot_target = target;
        object.pre_attack_ready_at = 0.0;
        object.stamp_auto_reload_when_idle_from_slot(slot, self.frame);
        let next = object.weapon_slot(slot).map_or(self.frame, |weapon| weapon.next_fire_frame);
        object.continuous_fire_coast_until_frame = if rule.continuous_fire_coast_frames == 0 {
            0
        } else { next.wrapping_add(rule.continuous_fire_coast_frames) };
        let count = i64::from(object.consecutive_shots_at_target);
        let one = i64::from(rule.continuous_fire_one_shots_needed);
        let two = i64::from(rule.continuous_fire_two_shots_needed);
        let level = object.continuous_fire_level;
        let cool = (level == 1 && count < one) || (level == 2 && count < two);
        let speed = (level == 0 && count > one) || (level == 1 && count > two);
        if cool {
            self.cool_down_owned_firing_tracker(id);
        } else if speed {
            if level == 1 {
                // FAST set while MEAN is still present, then MEAN cleared.
                self.objects.get_mut(&id).unwrap().continuous_fire_level = 3;
                self.notify_owned_weapon_bonus_change(id);
                self.objects.get_mut(&id).unwrap().continuous_fire_level = 2;
                self.notify_owned_weapon_bonus_change(id);
            } else {
                self.objects.get_mut(&id).unwrap().continuous_fire_level = 1;
                self.notify_owned_weapon_bonus_change(id);
            }
        }
        let Some(object) = self.objects.get_mut(&id) else { return };
        object.stamp_fire_sound_loop_after_shot(
            self.frame, Some(&rule.name), &mut self.combat_system.fire_sound_loop_events,
        );
        object.refresh_weapon_fire_status_at_frame(self.frame, &[1.0; 3]);
    }

    fn cool_down_owned_firing_tracker(&mut self, id: ObjectId) {
        let Some(object) = self.objects.get(&id) else { return };
        let level = object.continuous_fire_level;
        if level & 2 != 0 {
            self.objects.get_mut(&id).unwrap().continuous_fire_level &= !2;
            self.notify_owned_weapon_bonus_change(id);
        }
        if level & 1 != 0 {
            self.objects.get_mut(&id).unwrap().continuous_fire_level &= !1;
            self.notify_owned_weapon_bonus_change(id);
        }
        let object = self.objects.get_mut(&id).unwrap();
        if level == 0 {
            object.continuous_fire_coast_until_frame = 0;
        }
        object.consecutive_shots_at_target = 0;
        object.consecutive_shot_target = None;
    }

    pub(in crate::game_logic) fn tick_owned_firing_tracker_coast(&mut self, id: ObjectId) {
        let Some(object) = self.objects.get(&id) else { return };
        if !(0..3).any(|slot| object.selected_native_weapon_rule(slot).is_some()) { return; }
        let until = object.continuous_fire_coast_until_frame;
        if until == 0 || self.frame <= until { return; }
        self.objects.get_mut(&id).unwrap().continuous_fire_coast_until_frame = self.frame.wrapping_add(30);
        self.cool_down_owned_firing_tracker(id);
    }

    /// FiringTracker::shotFired FireSound and the WeaponTemplate LaserName beam
    /// for a shot already delivered and committed (FiringTracker.cpp:144-155,
    /// Weapon.cpp LaserName). Both come from the selected WeaponTemplate.
    pub(super) fn queue_authored_shot_presentation(
        &mut self,
        attacker_id: ObjectId,
        slot: u8,
        target_id: Option<ObjectId>,
        target_location: Option<Vec3>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(rule) = self
            .objects
            .get(&attacker_id)
            .and_then(|attacker| attacker.selected_native_weapon_rule(slot))
        else {
            return;
        };
        let muzzle_pos = self
            .objects
            .get(&attacker_id)
            .map_or(Vec3::ZERO, Object::get_position);
        let fire_target = target_id.filter(|id| self.objects.contains_key(id));
        let impact_pos = fire_target
            .and_then(|id| self.objects.get(&id).map(Object::get_position))
            .or(target_location);
        if !rule.laser_name.is_empty() {
            let to = impact_pos.unwrap_or(muzzle_pos);
            self.weapon_lasers.push(
                crate::game_logic::host_weapon_laser::ResidualWeaponLaser::with_bone(
                    rule.laser_name.clone(),
                    rule.laser_bone_name.clone(),
                    attacker_id,
                    fire_target,
                    (muzzle_pos.x, muzzle_pos.y, muzzle_pos.z),
                    (to.x, to.y, to.z),
                    self.frame,
                ),
            );
            let _ = self.spawn_weapon_laser_beam_object_with_effects(
                &rule.laser_name,
                attacker_id,
                fire_target,
                muzzle_pos,
                to,
                effects,
            );
        }
        let fire_sound = rule.fire_sound.name();
        if !fire_sound.is_empty() {
            self.queue_audio_event(
                AudioEventRequest::new(fire_sound)
                    .with_object(attacker_id)
                    .with_position(muzzle_pos)
                    .with_priority(160),
            );
        }
    }
}

#[cfg(test)]
#[path = "authored_shot_tests.rs"]
mod authored_shot_tests;
