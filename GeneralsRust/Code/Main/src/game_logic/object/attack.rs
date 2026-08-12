use super::*;

impl Object {
    /// Fire at target. `target_is_infantry` selects ScatterRadiusVsInfantry residual.
    pub fn fire_at(&mut self, target_id: ObjectId, current_time: f32) -> bool {
        self.fire_at_ex(target_id, current_time, false, false)
    }

    /// Fire at target with KindOf-aware scatter residual.
    /// `target_has_faerie_fire`: C++ TARGET_FAERIE_FIRE WeaponBonus ROF residual.
    pub fn fire_at_ex(
        &mut self,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
    ) -> bool {
        // C++ Weapon::getMaxShotCount residual — AI burst / scatter limits.
        if !self.has_max_shots_remaining() {
            return false;
        }

        // C++ canFireWeapon residual: jammed / disabled units cannot discharge.
        if self.status.weapons_jammed || self.is_disabled() {
            return false;
        }
        // Prefer an explicit locked/active slot when ready.  The normal
        // fallback remains PRIMARY then SECONDARY; TERTIARY is deliberately
        // excluded from autonomous selection (retail Comanche rocket pods
        // declare `AutoChooseSources = TERTIARY NONE`).
        let slot = {
            let mut rof = self.weapon_bonus_fields().2;
            if target_has_faerie_fire {
                rof *= crate::game_logic::host_avenger::FAERIE_FIRE_ROF_MULTIPLIER;
            }
            let primary_name = self.primary_weapon_name().map(|s| s.to_string());
            let secondary_name = self.secondary_weapon_name().map(|s| s.to_string());
            let primary_ready = self.weapon_slot(0).is_some_and(|w| {
                let reload = (w.reload_time / rof).max(0.0);
                Self::weapon_ready_named(w, current_time, primary_name.as_deref(), reload)
            });
            let secondary_ready = self.secondary_weapon.as_ref().is_some_and(|w| {
                let reload = (w.reload_time / rof).max(0.0);
                Self::weapon_ready_named(w, current_time, secondary_name.as_deref(), reload)
            });
            let tertiary_name = self.tertiary_weapon_name().map(str::to_owned);
            let tertiary_ready = self.tertiary_weapon.as_ref().is_some_and(|w| {
                let reload = (w.reload_time / rof).max(0.0);
                Self::weapon_ready_named(w, current_time, tertiary_name.as_deref(), reload)
            });

            if self.weapon_lock_type != WeaponLockType::NotLocked {
                // The lock owns its own concrete slot identity.  Do not use
                // `active_weapon_slot` here: an asynchronous UI/writeback
                // update can change the displayed slot while a permanent
                // lock is still in effect, and must not redirect a tertiary
                // command into PRIMARY or SECONDARY.
                match self.weapon_lock_slot {
                    0 if primary_ready => 0u8,
                    1 if secondary_ready => 1u8,
                    2 if tertiary_ready => 2u8,
                    // A lock must not silently redirect an explicit manual
                    // weapon command to PRIMARY when the requested slot is
                    // unavailable or still reloading.
                    _ => return false,
                }
            } else if self.active_weapon_slot == 2 && tertiary_ready {
                // Active TERTIARY is an explicit weapon toggle, not an auto
                // candidate.  Preserve that user selection.
                2u8
            } else if self.active_weapon_slot == 1 && secondary_ready {
                1u8
            } else if primary_ready {
                0u8
            } else if secondary_ready {
                1u8
            } else {
                return false;
            }
        };

        // C++ Weapon::getPreAttackDelay / PreAttackType residual.
        let pre_delay = {
            let base = self
                .weapon_slot(slot)
                .map(|w| w.pre_attack_delay.max(0.0))
                .unwrap_or(0.0);
            base * self.weapon_bonus_fields().3
        };
        let prefire = {
            let name = self.weapon_name_for_slot(slot);
            name.map(crate::game_logic::weapon_bootstrap::host_prefire_type_for_weapon_name)
                .unwrap_or(crate::game_logic::weapon_bootstrap::HostPrefireType::PerShot)
        };
        let apply_delay = self.pre_attack_delay_applies(slot, target_id, prefire, pre_delay);
        if apply_delay {
            // Arm a wind-up when:
            // - new target, or
            // - ready_at == 0 (previous shot completed / no active cycle).
            // Once armed, wait until ready_at; do NOT re-arm while ready_at is set
            // (even after it elapses) until record_shot_at_target clears it.
            let needs_arm =
                self.pre_attack_target != Some(target_id) || self.pre_attack_ready_at <= 0.0;
            if needs_arm {
                self.pre_attack_target = Some(target_id);
                self.record_host_combat_attack();
                self.pre_attack_ready_at = current_time + pre_delay;
                self.weapon_fire_status = WeaponFireStatus::PreAttack;
                self.sync_weapon_model_conditions_from_status();
                self.record_host_combat_attack();
                // C++ Weapon::preFireWeapon LeechRange activate residual.
                self.activate_leech_range_for_slot(slot);
            }
            if current_time + 1e-6 < self.pre_attack_ready_at {
                // Decision authority: engagement state is GameWorld last-writer.
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_attack(self.id, target_id);
                    crate::game_logic::host_ai_decision_log::record_set_state(self.id, 2);
                // Attacking
                } else {
                    self.target = Some(target_id);
                    self.set_ai_state(AIState::Attacking);
                }
                self.status.attacking = true;
                return false;
            }
            // Delay complete — fall through to fire; record_shot clears ready_at.
        } else {
            self.pre_attack_target = Some(target_id);
            self.record_host_combat_attack();
        }

        let fire_weapon_name = self.weapon_name_for_slot(slot).map(str::to_owned);
        let name = fire_weapon_name.as_deref();
        // C++ WeaponTemplate keeps each of these references per veterancy
        // level. Capture the shooter rank before the mutable weapon-slot
        // borrow below so the launched projectile carries the exact names
        // selected for this shot.
        let veterancy = self.experience.level;
        let (base_damage, fallback_range, fallback_min_range) = self
            .weapon_slot(slot)
            .map(|weapon| (weapon.damage, weapon.range, weapon.min_range))
            .unwrap_or((0.0, 0.0, 0.0));
        let weapon_damage = self.effective_weapon_damage(base_damage);

        // Resolve immutable Weapon.ini peels before borrowing the live slot.
        // Keeping the selected slot's name here prevents tertiary fire from
        // borrowing primary/secondary presentation and damage data.
        let projectile_object_name = name
            .map(crate::game_logic::weapon_bootstrap::host_projectile_name_for_weapon_name)
            .unwrap_or_default();
        let fire_fx_name = name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_fire_fx_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default();
        let fire_ocl_name = name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_fire_ocl_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default();
        let detonation_fx_name = name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default();
        let detonation_ocl_name = name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_detonation_ocl_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default();
        let exhaust_name = name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_projectile_exhaust_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default();
        let secondary_damage = name
            .map(crate::game_logic::weapon_bootstrap::host_secondary_damage_for_weapon_name)
            .unwrap_or(0.0);
        let secondary_damage_radius = name
            .map(crate::game_logic::weapon_bootstrap::host_secondary_damage_radius_for_weapon_name)
            .unwrap_or(0.0);
        let shock_wave_amount = name
            .map(crate::game_logic::weapon_bootstrap::host_shock_wave_amount_for_weapon_name)
            .unwrap_or(0.0);
        let shock_wave_radius = name
            .map(crate::game_logic::weapon_bootstrap::host_shock_wave_radius_for_weapon_name)
            .unwrap_or(0.0);
        let shock_wave_taper_off = name
            .map(crate::game_logic::weapon_bootstrap::host_shock_wave_taper_for_weapon_name)
            .unwrap_or(0.0);
        let radius_damage_affects = name
            .map(crate::game_logic::weapon_bootstrap::host_radius_damage_affects_for_weapon_name)
            .unwrap_or(
                crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                    | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            );
        let projectile_collides = name
            .map(crate::game_logic::weapon_bootstrap::host_projectile_collides_for_weapon_name)
            .unwrap_or(crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT);
        let scatter_radius = name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_effective_scatter_radius(
                    weapon_name,
                    target_is_infantry,
                )
            })
            .unwrap_or(0.0);
        let speed_peel = name
            .map(crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name)
            .unwrap_or_default();
        let historic_bonus = name
            .map(crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name)
            .unwrap_or_default();

        let (weapon_speed, weapon_splash, weapon_homing, auto_reloaded_clip) =
            match self.weapon_slot_mut(slot) {
                Some(weapon) => {
                    Self::consume_ammo_on_fire_named(weapon, current_time, name);
                    (
                        weapon.projectile_speed,
                        weapon.splash_radius,
                        // AA residual: air-only weapons home on the live target.
                        weapon.can_target_air && !weapon.can_target_ground,
                        Self::auto_reloaded_clip_after_firing(weapon, name),
                    )
                }
                None => return false,
            };
        let shooter_id = self.id;
        let shooter_pos = self.get_position();
        self.target = Some(target_id);

        // Prefer Weapon.ini DamageType via store name; shape residual if the
        // source name has no usable host store entry.
        let weapon_dtype = if let Some(weapon_name) = name {
            let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
            if crate::game_logic::thing::ThingTemplate::weapon_from_store(weapon_name).is_some() {
                crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
                    weapon_name,
                )
            } else if weapon_speed <= 0.0 || weapon_speed >= 999_000.0 {
                super::combat::DamageType::Laser
            } else if weapon_splash > 0.0 {
                super::combat::DamageType::Explosive
            } else {
                super::combat::DamageType::Bullet
            }
        } else if weapon_speed <= 0.0 || weapon_speed >= 999_000.0 {
            super::combat::DamageType::Laser
        } else if weapon_splash > 0.0 {
            super::combat::DamageType::Explosive
        } else {
            super::combat::DamageType::Bullet
        };
        super::combat::queue_projectile(super::combat::PendingProjectile {
            shooter_id,
            shooter_pos,
            source_context: Some(super::combat::ProjectileLaunchContext {
                source_team: self.team,
                source_veterancy: veterancy,
                source_orientation: self.get_orientation(),
                source_velocity: self.movement.velocity,
            }),
            target_id: Some(target_id),
            target_pos: None,
            damage: weapon_damage,
            speed: weapon_speed,
            splash_radius: weapon_splash,
            is_homing: weapon_homing,
            damage_type: weapon_dtype,
            death_type: crate::game_logic::host_armor_residual::resolve_host_death_type(
                name,
                weapon_dtype,
            ),
            projectile_object_name,
            projectile_lifecycle: None,
            fire_fx_name,
            fire_ocl_name,
            detonation_fx_name,
            detonation_ocl_name,
            exhaust_name,
            secondary_damage,
            secondary_damage_radius,
            shock_wave_amount,
            shock_wave_radius,
            shock_wave_taper_off,
            radius_damage_affects,
            projectile_collides,
            // C++ ScatterRadius + ScatterRadiusVsInfantry residual.
            scatter_radius,
            min_weapon_speed: speed_peel.min_weapon_speed,
            scale_weapon_speed: speed_peel.scale_weapon_speed,
            attack_range: if speed_peel.attack_range > 0.0 {
                speed_peel.attack_range
            } else {
                fallback_range
            },
            min_attack_range: if speed_peel.min_attack_range > 0.0 {
                speed_peel.min_attack_range
            } else {
                fallback_min_range
            },
            historic_weapon_key: fire_weapon_name.clone().unwrap_or_default(),
            historic_bonus_time_frames: historic_bonus.time_frames,
            historic_bonus_count: historic_bonus.count,
            historic_bonus_radius: historic_bonus.radius,
            historic_bonus_weapon: historic_bonus.bonus_weapon,
            die_on_detonate: name
                .map(crate::game_logic::weapon_bootstrap::host_die_on_detonate_for_weapon_name)
                .unwrap_or(false),
        });
        // C++ fireWeaponTemplate LeechRange activate residual.
        self.activate_leech_range_for_slot(slot);
        self.record_shot_at_target(target_id);
        // C++ Weapon::m_numShotsForCurBarrel / m_curBarrel residual.
        self.advance_weapon_barrel_after_shot();
        // C++ --m_maxShotCount residual.
        self.consume_max_shot_count();
        self.refresh_weapon_fire_status(current_time);
        // C++ Object::fireCurrentWeapon releases a temporary lock only when
        // the *locked* weapon completed an auto-reloading clip.  A temporary
        // third-slot command must not be unlocked by a primary fallback shot
        // while that third slot is still reloading.
        if auto_reloaded_clip
            && self.weapon_lock_type == WeaponLockType::LockedTemporarily
            && self.weapon_lock_slot == slot
        {
            self.release_weapon_lock(WeaponLockType::LockedTemporarily);
        }
        {
            let frame = crate::game_logic::host_historic_bonus::logic_frame();
            self.stamp_fire_sound_loop_after_shot(frame, fire_weapon_name.as_deref());
        }
        {
            let (dmg, rng) = self
                .weapon_slot(slot)
                .map(|w| (w.damage, w.range))
                .unwrap_or((0.0, 0.0));
            let frame = crate::game_logic::host_historic_bonus::logic_frame();
            let next_count = self.fire_intent_count.saturating_add(1);
            // When AI attack authority is on, GameWorld SetFireIntent writeback is
            // last-writer — log the intent without dual-writing host last_fire_*.
            if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                crate::game_logic::host_fire_intent_log::record(
                    self.id,
                    target_id.0,
                    slot,
                    dmg,
                    rng,
                    current_time,
                    frame,
                    next_count,
                );
                // Keep counter monotonic for subsequent shots this frame.
                self.fire_intent_count = next_count;
            } else {
                self.last_fire_victim_host = target_id.0;
                self.last_fire_slot = slot;
                self.last_fire_damage = dmg;
                self.last_fire_range = rng;
                self.last_fire_sim_time = current_time;
                self.last_fire_frame = frame;
                self.fire_intent_count = next_count;
                self.record_host_fire_intent();
            }
        }

        // C++ STEALTH_NOT_WHILE_ATTACKING / IS_FIRING_WEAPON residual:
        // firing breaks stealth (default host residual).
        if self.stealth_breaks_on_attack && self.status.stealthed {
            self.break_stealth();
        }
        true
    }

    pub fn move_to(&mut self, position: Vec3) {
        if self.is_mobile() && self.is_alive() {
            self.movement.target_position = Some(position);
            self.set_ai_state(AIState::Moving);
            self.set_status_moving(true);
            crate::game_logic::host_move_log::record(
                self.id,
                Some([position.x, position.y, position.z]),
            );
        }
    }

    pub fn stop_moving(&mut self) {
        self.movement.target_position = None;
        self.movement.velocity = Vec3::ZERO;
        crate::game_logic::host_move_log::record(self.id, None);
        self.movement.path.clear();
        self.movement.current_path_index = 0;
        self.set_status_moving(false);
        self.waiting_for_path = false;
        self.is_attack_path = false;
        self.is_approach_path = false;
        self.record_host_locomotor();
        self.is_safe_path = false;
        self.temporary_move_frames = 0;
        self.record_host_combat_attack();
        self.is_blocked = false;
        self.is_blocked_and_stuck = false;
        // Only pure locomotion returns to Idle when the destination is reached.
        // Interaction states (Capturing, Repairing, SpecialAbility, Entering, …)
        // set a destination while remaining in-state; clobbering them to Idle
        // aborted capture/repair on arrival before support-state resolution.
        if matches!(self.ai_state, AIState::Moving | AIState::AttackMoving) {
            self.set_ai_state(AIState::Idle);
        }
        self.record_host_movement();
    }

    pub fn attack_target(&mut self, target_id: ObjectId) {
        if !self.is_alive() {
            return;
        }
        // Shock stun residual: ignore new attack orders while stunned.
        if self.is_shock_stunned() {
            return;
        }
        // Jet takeoff residual: leave hangar before engaging.
        let _ = self.takeoff_from_airfield_parking();
        if self.can_attack() {
            if self.pre_attack_target != Some(target_id) {
                // New target — fire_at will start PRE_ATTACK clock.
                self.pre_attack_target = None;
                self.record_host_combat_attack();
                self.pre_attack_ready_at = 0.0;
                self.record_host_combat_attack();
            }
            self.target = Some(target_id);
            self.target_location = None;
            self.set_status_force_attack(false);
            self.set_ai_state(AIState::Attacking);
            self.status.attacking = true;
            crate::game_logic::host_attack_log::record(self.id, Some(target_id));
        }
    }

    /// C++ Weapon::setLeechRangeActive residual for a weapon slot.

    /// C++ Weapon barrel rotation residual after a shot.
    /// Decrements shots on current barrel; when exhausted, advances `weapon_cur_barrel`.

    /// C++ FiringTracker::shotFired FireSoundLoopTime residual.
    /// Extends the looping fire-audio deadline; records start when newly armed.
    pub fn stamp_fire_sound_loop_after_shot(&mut self, frame: u32, weapon_name: Option<&str>) {
        let loop_frames = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_fire_sound_loop_frames_for_weapon_name)
            .unwrap_or(0);
        if loop_frames == 0 {
            return;
        }
        let sound = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_fire_sound_for_weapon_name)
            .unwrap_or_default();
        if sound.is_empty() {
            return;
        }
        let was_active = self.fire_sound_loop_until_frame > frame;
        self.fire_sound_loop_until_frame = frame.saturating_add(loop_frames);
        self.fire_sound_loop_name = sound.clone();
        if !was_active {
            crate::game_logic::host_fire_sound_loop_log::record(self.id, sound, true);
        }
    }

    /// C++ FiringTracker::update stop-loop residual when deadline elapses.
    pub fn tick_fire_sound_loop(&mut self, frame: u32) {
        if self.fire_sound_loop_until_frame == 0 {
            return;
        }
        if frame >= self.fire_sound_loop_until_frame {
            let sound = std::mem::take(&mut self.fire_sound_loop_name);
            self.fire_sound_loop_until_frame = 0;
            if !sound.is_empty() {
                crate::game_logic::host_fire_sound_loop_log::record(self.id, sound, false);
            }
        }
    }

    pub fn advance_weapon_barrel_after_shot(&mut self) {
        let spb = self.weapon_shots_per_barrel.max(1);
        let barrels = self.weapon_barrel_count.max(1) as u32;
        if self.weapon_shots_left_on_barrel == 0 {
            self.weapon_shots_left_on_barrel = spb;
        }
        self.weapon_shots_left_on_barrel = self.weapon_shots_left_on_barrel.saturating_sub(1);
        if self.weapon_shots_left_on_barrel == 0 {
            self.weapon_cur_barrel = ((self.weapon_cur_barrel as u32 + 1) % barrels) as u8;
            self.weapon_shots_left_on_barrel = spb;
        }
    }

    pub fn activate_leech_range_for_slot(&mut self, slot: u8) {
        let name = self.weapon_name_for_slot(slot);
        let is_leech = name
            .map(crate::game_logic::weapon_bootstrap::host_leech_range_weapon_for_weapon_name)
            .unwrap_or(false);
        if !is_leech {
            return;
        }
        match slot {
            0 => {
                self.leech_range_active_primary = true;
                self.record_host_weapon_stats();
            }
            1 => {
                self.leech_range_active_secondary = true;
                self.record_host_weapon_stats();
            }
            // The host only has persisted leech flags for A/B.  Do not alias
            // a tertiary leech weapon to primary; it would alter a different
            // slot's targeting range.
            _ => {}
        }
    }

    /// C++ Object::clearLeechRangeModeForAllWeapons residual.
    pub fn clear_leech_range_mode_for_all_weapons(&mut self) {
        self.leech_range_active_primary = false;
        self.record_host_weapon_stats();
        self.leech_range_active_secondary = false;
        self.record_host_weapon_stats();
    }

    pub fn stop_attack(&mut self) {
        // C++ WeaponSet::releaseWeaponLock: any non-special player order and
        // an exited attack state release a temporary special-weapon lock.
        // Permanent user locks remain intact.
        self.release_weapon_lock(WeaponLockType::LockedTemporarily);
        self.target = None;
        self.target_location = None;
        self.record_host_target_location();
        self.set_status_force_attack(false);
        self.pre_attack_target = None;
        self.record_host_combat_attack();
        self.pre_attack_ready_at = 0.0;
        self.record_host_combat_attack();
        self.consecutive_shot_target = None;
        self.consecutive_shots_at_target = 0;
        self.record_host_combat_attack();
        self.clear_leech_range_mode_for_all_weapons();
        self.status.attacking = false;
        crate::game_logic::host_attack_log::record(self.id, None);
        // C++ parity: guard units return to their guard state after a kill
        // rather than going fully idle. The guard anchor/radius are preserved
        // so the support-states update loop will re-engage nearby enemies.
        if self.guard_target.is_some() {
            self.set_ai_state(AIState::GuardingObject);
        } else if self.guard_position.is_some() {
            self.set_ai_state(AIState::GuardingArea);
        } else {
            self.set_ai_state(AIState::Idle);
        }
    }

    pub fn clear_all_occupants(&mut self) {
        if let Some(building) = self.building_data.as_mut() {
            building.garrisoned_units.clear();
        }
        self.occupants.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_tertiary_fire_uses_the_real_third_slot() {
        let mut attacker = Object::new(
            ThingTemplate::new("ThreeSlotAttacker"),
            ObjectId(1),
            Team::USA,
        );
        attacker.weapon = Some(Weapon {
            damage: 5.0,
            range: 100.0,
            ..Weapon::default()
        });
        attacker.tertiary_weapon = Some(Weapon {
            damage: 37.0,
            range: 250.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        assert!(attacker.set_weapon_lock(2, WeaponLockType::LockedPermanently));

        // Simulate a stale displayed active slot.  The stored weapon lock is
        // authoritative and must retain tertiary identity while firing.
        attacker.set_active_weapon_slot(0);

        assert!(attacker.fire_at(ObjectId(2), 1.0));
        assert_eq!(attacker.last_fire_slot, 2);
        assert!((attacker.last_fire_damage - 37.0).abs() < f32::EPSILON);
        assert_eq!(
            attacker
                .tertiary_weapon
                .as_ref()
                .map(|weapon| weapon.last_fire_time),
            Some(1.0)
        );
        assert_eq!(
            attacker.weapon.as_ref().map(|weapon| weapon.last_fire_time),
            Some(0.0),
            "primary must not be consumed by an explicit tertiary fire"
        );
    }
}
