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
        // Prefer the locked/active slot when ready; else primary; else secondary.
        let slot = {
            let prefer_secondary = self.active_weapon_slot == 1;
            let mut rof = self.weapon_bonus_fields().2;
            if target_has_faerie_fire {
                rof *= crate::game_logic::host_avenger::FAERIE_FIRE_ROF_MULTIPLIER;
            }
            let primary_name = self.primary_weapon_name().map(|s| s.to_string());
            let secondary_name = self.secondary_weapon_name().map(|s| s.to_string());
            let primary_ready = self.weapon.as_ref().is_some_and(|w| {
                let reload = (w.reload_time / rof).max(0.0);
                Self::weapon_ready_named(w, current_time, primary_name.as_deref(), reload)
            });
            let secondary_ready = self.secondary_weapon.as_ref().is_some_and(|w| {
                let reload = (w.reload_time / rof).max(0.0);
                Self::weapon_ready_named(w, current_time, secondary_name.as_deref(), reload)
            });
            if prefer_secondary && secondary_ready {
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
            let name = if slot == 1 {
                self.thing.template.secondary_weapon_name.as_deref().or(self
                    .thing
                    .template
                    .primary_weapon_name
                    .as_deref())
            } else {
                self.thing.template.primary_weapon_name.as_deref()
            };
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

        let fire_weapon_name = if slot == 1 {
            self.secondary_weapon_name().map(|s| s.to_string())
        } else {
            self.primary_weapon_name().map(|s| s.to_string())
        };
        let base_damage = self.weapon_slot(slot).map(|w| w.damage).unwrap_or(0.0);
        let weapon_damage = self.effective_weapon_damage(base_damage);
        if let Some(weapon) = self.weapon_slot_mut(slot) {
            Self::consume_ammo_on_fire_named(weapon, current_time, fire_weapon_name.as_deref());
            let weapon_speed = weapon.projectile_speed;
            let weapon_splash = weapon.splash_radius;
            // AA residual: air-only weapons home on live target (missile track).
            let weapon_homing = weapon.can_target_air && !weapon.can_target_ground;
            let shooter_id = self.id;
            let shooter_pos = self.get_position();
            self.target = Some(target_id);

            // Prefer Weapon.ini DamageType via store name; shape residual if store empty.
            let weapon_dtype = {
                let slot = self.active_weapon_slot;
                let name = if slot == 1 {
                    self.thing.template.secondary_weapon_name.as_deref().or(self
                        .thing
                        .template
                        .primary_weapon_name
                        .as_deref())
                } else {
                    self.thing.template.primary_weapon_name.as_deref()
                };
                if let Some(n) = name {
                    let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
                    if crate::game_logic::thing::ThingTemplate::weapon_from_store(n).is_some() {
                        crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(n)
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
                }
            };
            super::combat::queue_projectile(super::combat::PendingProjectile {
                shooter_id,
                shooter_pos,
                target_id: Some(target_id),
                target_pos: None,
                damage: weapon_damage,
                speed: weapon_speed,
                splash_radius: weapon_splash,
                is_homing: weapon_homing,
                damage_type: weapon_dtype,
                death_type: {
                    let slot = self.active_weapon_slot;
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    crate::game_logic::host_armor_residual::resolve_host_death_type(
                        name,
                        weapon_dtype,
                    )
                },
                projectile_object_name:
                    crate::game_logic::weapon_bootstrap::host_projectile_name_for_unit_slot(
                        self.template_name.as_str(),
                        self.thing.template.primary_weapon_name.as_deref(),
                        self.thing.template.secondary_weapon_name.as_deref(),
                        slot,
                    ),
                detonation_fx_name: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name,
                    )
                    .unwrap_or_default()
                },
                detonation_ocl_name: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_detonation_ocl_for_weapon_name,
                    )
                    .unwrap_or_default()
                },
                exhaust_name:
                    crate::game_logic::weapon_bootstrap::host_projectile_exhaust_for_unit_slot(
                        self.template_name.as_str(),
                        self.thing.template.primary_weapon_name.as_deref(),
                        self.thing.template.secondary_weapon_name.as_deref(),
                        slot,
                    ),
                secondary_damage: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_secondary_damage_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                secondary_damage_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_secondary_damage_radius_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                shock_wave_amount: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_shock_wave_amount_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                shock_wave_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_shock_wave_radius_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                shock_wave_taper_off: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_shock_wave_taper_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                radius_damage_affects: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_radius_damage_affects_for_weapon_name,
                    )
                    .unwrap_or(
                        crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                            | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
                    )
                },
                projectile_collides: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_projectile_collides_for_weapon_name,
                    )
                    .unwrap_or(crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT)
                },
                // C++ ScatterRadius + ScatterRadiusVsInfantry residual.
                // fire_at cannot query peer KindOf; apply VsInfantry peel whenever a
                // target id is set (infantry-common residual). Ground attacks use base only.
                scatter_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    // C++: base ScatterRadius + ScatterRadiusVsInfantry only vs infantry.
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_effective_scatter_radius(
                            n,
                            target_is_infantry,
                        )
                    })
                    .unwrap_or(0.0)
                },
                min_weapon_speed: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .min_weapon_speed
                    })
                    .unwrap_or(0.0)
                },
                scale_weapon_speed: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .scale_weapon_speed
                    })
                    .unwrap_or(false)
                },
                attack_range: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .attack_range
                    })
                    .or_else(|| self.weapon_slot(slot).map(|w| w.range))
                    .unwrap_or(0.0)
                },
                min_attack_range: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .min_attack_range
                    })
                    .or_else(|| self.weapon_slot(slot).map(|w| w.min_range))
                    .unwrap_or(0.0)
                },
                historic_weapon_key: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.unwrap_or("").to_string()
                },
                historic_bonus_time_frames: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .time_frames
                    })
                    .unwrap_or(0)
                },
                historic_bonus_count: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .count
                    })
                    .unwrap_or(0)
                },
                historic_bonus_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .radius
                    })
                    .unwrap_or(0.0)
                },
                historic_bonus_weapon: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .bonus_weapon
                    })
                    .unwrap_or_default()
                },
                die_on_detonate: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_die_on_detonate_for_weapon_name,
                    )
                    .unwrap_or(false)
                },
            });
            // C++ fireWeaponTemplate LeechRange activate residual.
            self.activate_leech_range_for_slot(slot);
            self.record_shot_at_target(target_id);
            // C++ Weapon::m_numShotsForCurBarrel / m_curBarrel residual.
            self.advance_weapon_barrel_after_shot();
            // C++ --m_maxShotCount residual.
            self.consume_max_shot_count();
            self.refresh_weapon_fire_status(current_time);
            {
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let wname_owned = if slot == 1 {
                    self.thing
                        .template
                        .secondary_weapon_name
                        .clone()
                        .or_else(|| self.thing.template.primary_weapon_name.clone())
                } else {
                    self.thing.template.primary_weapon_name.clone()
                };
                self.stamp_fire_sound_loop_after_shot(frame, wname_owned.as_deref());
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
        } else {
            false
        }
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
        let name = if slot == 1 {
            self.thing.template.secondary_weapon_name.as_deref().or(self
                .thing
                .template
                .primary_weapon_name
                .as_deref())
        } else {
            self.thing.template.primary_weapon_name.as_deref()
        };
        let is_leech = name
            .map(crate::game_logic::weapon_bootstrap::host_leech_range_weapon_for_weapon_name)
            .unwrap_or(false);
        if !is_leech {
            return;
        }
        if slot == 1 {
            self.leech_range_active_secondary = true;
            self.record_host_weapon_stats();
        } else {
            self.leech_range_active_primary = true;
            self.record_host_weapon_stats();
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
