use super::*;

#[derive(Clone, Copy)]
enum WeaponShotRoute {
    Autonomous,
    Passive { slot: u8, origin: Vec3 },
}

impl Object {
    /// Fire at target. `target_is_infantry` selects ScatterRadiusVsInfantry residual.
    pub fn fire_at(
        &mut self,
        target_id: ObjectId,
        current_time: f32,
        logic_frame: u32,
        combat: &mut super::combat::CombatSystem,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
    ) -> bool {
        self.fire_at_ex(
            target_id,
            current_time,
            false,
            false,
            logic_frame,
            combat,
            attack_events,
        )
    }

    /// Fire at target with KindOf-aware scatter residual.
    /// `target_has_faerie_fire`: C++ TARGET_FAERIE_FIRE WeaponBonus ROF residual.
    pub fn fire_at_ex(
        &mut self,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        combat: &mut super::combat::CombatSystem,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
    ) -> bool {
        let Some(slot) = self.fire_at_ex_defer_weapon_barrel_advance(
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            combat,
            attack_events,
        ) else {
            return false;
        };
        // This legacy Object-only entry point has no owning GameLogic from
        // which to allocate a durable actual-discharge sequence. Preserve its
        // logical cursor behavior, but keep it deliberately presentation
        // eventless. Live GameLogic fire paths use the deferred entry point
        // and record_accepted_weapon_discharge instead.
        self.advance_weapon_barrel_after_shot(slot);
        true
    }

    /// Execute one accepted shot without advancing its exact WeaponSet barrel.
    ///
    /// The owning GameLogic must immediately finish this through
    /// `record_accepted_weapon_discharge`, which captures the pre-advance
    /// barrel, advances it once, stamps its durable marker, and freezes a
    /// renderer-facing event. Object-only legacy paths retain `fire_at_ex`.
    pub(crate) fn fire_at_ex_defer_weapon_barrel_advance(
        &mut self,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        combat: &mut super::combat::CombatSystem,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
    ) -> Option<u8> {
        let (slot, pending) = self.prepare_weapon_shot(
            WeaponShotRoute::Autonomous,
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            &mut combat.fire_sound_loop_events,
            attack_events,
            None,
        )?;
        super::combat::queue_projectile(combat, pending);
        Some(slot)
    }

    /// One ordinary current-weapon discharge without changing the passenger's
    /// or turret's AI state. The receiving world emits this exact owned shot
    /// and commits its barrel once; earlier accepted shots are not drained.
    pub(crate) fn prepare_passive_weapon_shot(
        &mut self,
        slot: u8,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        muzzle_origin: Vec3,
        sound_events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot(
            WeaponShotRoute::Passive {
                slot,
                origin: muzzle_origin,
            },
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            sound_events,
            attack_events,
            content,
        )
    }

    pub(crate) fn prepare_autonomous_weapon_shot(
        &mut self,
        target: ObjectId,
        time: f32,
        infantry: bool,
        faerie: bool,
        frame: u32,
        sound: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attacks: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot(
            WeaponShotRoute::Autonomous,
            target,
            time,
            infantry,
            faerie,
            frame,
            sound,
            attacks,
            content,
        )
    }

    /// The receiving content catalogue cannot silently substitute a process
    /// bootstrap profile for an unimplemented authored projectile family.
    pub(in crate::game_logic) fn selected_projectile_is_admitted(
        &self,
        slot: u8,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
    ) -> bool {
        let Some(content) = content else {
            return true;
        };
        let Some(rule) = self.selected_native_weapon_rule(slot) else {
            return false;
        };
        let projectile = rule.projectile_name.trim();
        if projectile.is_empty()
            || projectile.eq_ignore_ascii_case("None")
            || content.get(projectile).is_some()
        {
            return true;
        }
        log::error!(
            "Receiving projectile definition '{}' has no admitted flight family",
            projectile
        );
        false
    }

    fn prepare_weapon_shot(
        &mut self,
        route: WeaponShotRoute,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        sound_events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        let autonomous = matches!(route, WeaponShotRoute::Autonomous);
        // C++ Weapon::getMaxShotCount residual — AI burst / scatter limits.
        if !self.has_max_shots_remaining() {
            return None;
        }

        // C++ canFireWeapon residual: jammed / disabled units cannot discharge.
        if self.status.weapons_jammed || self.is_disabled() {
            return None;
        }
        // C++ Object::isAbleToAttack (Object.cpp:3171-3176): sold / under
        // construction cannot discharge even if a weapon is otherwise ready.
        if self.status.under_construction || self.status.sold {
            return None;
        }
        // C++ setWeaponBonusCondition → onWeaponBonusChange: restart an
        // in-progress wait with the new RATE_OF_FIRE delay before the ready
        // check so mid-reload bonuses cannot fire sooner than C++.
        self.apply_weapon_bonus_rof_restart(current_time);

        // Prefer an explicit locked/active slot when ready.  The normal
        // fallback remains PRIMARY then SECONDARY; TERTIARY is deliberately
        // excluded from autonomous selection (retail Comanche rocket pods
        // declare `AutoChooseSources = TERTIARY NONE`).
        let slot = if let WeaponShotRoute::Passive { slot, .. } = route {
            let weapon = self.weapon_slot(slot)?;
            let name = self.weapon_name_for_slot(slot);
            let mut rof = self.weapon_bonus_fields().2;
            if target_has_faerie_fire {
                rof *= crate::game_logic::host_avenger::FAERIE_FIRE_ROF_MULTIPLIER;
            }
            let reload = self.live_reload_interval(weapon, name, rof);
            if !Self::weapon_ready_selected(
                weapon,
                current_time,
                name,
                reload,
                self.selected_native_weapon_rule(slot).as_deref(),
            ) {
                return None;
            }
            slot
        } else {
            let mut rof = self.weapon_bonus_fields().2;
            if target_has_faerie_fire {
                rof *= crate::game_logic::host_avenger::FAERIE_FIRE_ROF_MULTIPLIER;
            }
            let primary_name = self.primary_weapon_name().map(|s| s.to_string());
            let secondary_name = self.secondary_weapon_name().map(|s| s.to_string());
            let primary_ready = self.weapon_slot(0).is_some_and(|w| {
                let reload = self.live_reload_interval(w, primary_name.as_deref(), rof);
                Self::weapon_ready_selected(
                    w,
                    current_time,
                    primary_name.as_deref(),
                    reload,
                    self.selected_native_weapon_rule(0).as_deref(),
                )
            });
            let secondary_ready = self.secondary_weapon.as_ref().is_some_and(|w| {
                let reload = self.live_reload_interval(w, secondary_name.as_deref(), rof);
                Self::weapon_ready_selected(
                    w,
                    current_time,
                    secondary_name.as_deref(),
                    reload,
                    self.selected_native_weapon_rule(1).as_deref(),
                )
            });
            let tertiary_name = self.tertiary_weapon_name().map(str::to_owned);
            let tertiary_ready = self.tertiary_weapon.as_ref().is_some_and(|w| {
                let reload = self.live_reload_interval(w, tertiary_name.as_deref(), rof);
                Self::weapon_ready_selected(
                    w,
                    current_time,
                    tertiary_name.as_deref(),
                    reload,
                    self.selected_native_weapon_rule(2).as_deref(),
                )
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
                    _ => return None,
                }
            } else if self.active_weapon_slot == 2 && tertiary_ready {
                // Active TERTIARY is an explicit weapon toggle, not an auto
                // candidate.  Preserve that user selection.
                2u8
            } else if self.active_weapon_slot == 1 && secondary_ready {
                1u8
            } else if primary_ready {
                0u8
            } else if secondary_ready && self.thing.template.slot_allows_auto_choose(1) {
                1u8
            } else {
                return None;
            }
        };

        if !self.selected_projectile_is_admitted(slot, content) {
            return None;
        }

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
            self.selected_native_weapon_rule(slot)
                .map(|rule| {
                    use crate::game_logic::weapon_bootstrap::HostPrefireType as Host;
                    use gamelogic::weapon::WeaponPrefireType as Native;
                    match rule.prefire_type {
                        Native::PrefirePerShot => Host::PerShot,
                        Native::PrefirePerAttack => Host::PerAttack,
                        Native::PrefirePerClip => Host::PerClip,
                    }
                })
                .unwrap_or_else(|| {
                    name.map(crate::game_logic::weapon_bootstrap::host_prefire_type_for_weapon_name)
                        .unwrap_or(crate::game_logic::weapon_bootstrap::HostPrefireType::PerShot)
                })
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

                self.pre_attack_ready_at = current_time + pre_delay;
                self.weapon_fire_status = WeaponFireStatus::PreAttack;
                self.sync_weapon_model_conditions_from_status();

                // C++ Weapon::preFireWeapon LeechRange activate residual.
                self.activate_leech_range_for_slot(slot);
            }
            if current_time + 1e-6 < self.pre_attack_ready_at {
                self.note_attack_target(target_id, attack_events);
                let entered_attack = !matches!(
                    self.ai_state,
                    AIState::Patrolling | AIState::AttackMoving | AIState::Attacking
                );
                if autonomous && entered_attack {
                    self.set_ai_state(AIState::Attacking);
                }
                {}
                return None;
            }
            // Delay complete — fall through to fire; record_shot clears ready_at.
        } else {
            self.pre_attack_target = Some(target_id);
        }

        self.note_attack_target(target_id, attack_events);
        let entered_attack = !matches!(
            self.ai_state,
            AIState::Patrolling | AIState::AttackMoving | AIState::Attacking
        );
        if autonomous && entered_attack {
            self.set_ai_state(AIState::Attacking);
        }
        {}

        let possible_next_shot_frame =
            self.possible_next_shot_frame_for_slot(slot, logic_frame, target_has_faerie_fire);
        let fire_weapon_name = self.weapon_name_for_slot(slot).map(str::to_owned);
        let name = fire_weapon_name.as_deref();
        // C++ WeaponTemplate keeps each of these references per veterancy
        // level. Capture the shooter rank before the mutable weapon-slot
        // borrow below so the launched projectile carries the exact names
        // selected for this shot.
        let veterancy = self.experience.level;
        let (fallback_damage, fallback_range, fallback_min_range) = self
            .weapon_slot(slot)
            .map(|weapon| (weapon.damage, weapon.range, weapon.min_range))
            .unwrap_or((0.0, 0.0, 0.0));
        // Prefer store PrimaryDamage so baked promotion scalars on PRIMARY
        // are not stacked again with weapon_bonus_fields veterancy.
        let native_rule = self.selected_native_weapon_rule(slot);
        let template_damage = native_rule
            .as_ref()
            .map(|rule| rule.primary_damage)
            .unwrap_or_else(|| {
                name.and_then(
                    crate::game_logic::weapon_bootstrap::host_primary_damage_for_weapon_name,
                )
                .unwrap_or(fallback_damage)
            });
        let weapon_damage = self.effective_weapon_damage(template_damage);

        // Resolve immutable Weapon.ini peels before borrowing the live slot.
        // Keeping the selected slot's name here prevents tertiary fire from
        // borrowing primary/secondary presentation and damage data.
        let projectile_object_name = self
            .selected_projectile_name(slot)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                name.map(crate::game_logic::weapon_bootstrap::host_projectile_name_for_weapon_name)
                    .unwrap_or_default()
            });
        let native_rank = match veterancy {
            crate::game_logic::VeterancyLevel::Rookie => gamelogic::common::VeterancyLevel::Regular,
            crate::game_logic::VeterancyLevel::Veteran => {
                gamelogic::common::VeterancyLevel::Veteran
            }
            crate::game_logic::VeterancyLevel::Elite => gamelogic::common::VeterancyLevel::Elite,
            crate::game_logic::VeterancyLevel::Heroic => gamelogic::common::VeterancyLevel::Heroic,
        };
        let fire_fx_name = native_rule
            .as_ref()
            .map(|rule| {
                rule.get_fire_fx(native_rank)
                    .map(|fx| fx.name().to_owned())
                    .unwrap_or_default()
            })
            .unwrap_or_else(|| {
                name.map(|weapon_name| {
                    crate::game_logic::weapon_bootstrap::host_fire_fx_for_weapon_name_at_veterancy(
                        weapon_name,
                        veterancy,
                    )
                })
                .unwrap_or_default()
            });
        let fire_ocl_name = native_rule
            .as_ref()
            .map(|rule| {
                rule.get_fire_ocl_name(native_rank)
                    .unwrap_or_default()
                    .to_owned()
            })
            .unwrap_or_else(|| {
                name.map(|weapon_name| {
                    crate::game_logic::weapon_bootstrap::host_fire_ocl_for_weapon_name_at_veterancy(
                        weapon_name,
                        veterancy,
                    )
                })
                .unwrap_or_default()
            });
        let detonation_fx_name = native_rule.as_ref().map(|rule| rule.get_projectile_detonate_fx(native_rank).map(|fx| fx.name().to_owned()).unwrap_or_default()).unwrap_or_else(|| name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default());
        let detonation_ocl_name = native_rule.as_ref().map(|rule| rule.get_projectile_detonation_ocl_name(native_rank).unwrap_or_default().to_owned()).unwrap_or_else(|| name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_detonation_ocl_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default());
        let exhaust_name = native_rule.as_ref().map(|rule| rule.get_projectile_exhaust_name(native_rank).unwrap_or_default().to_owned()).unwrap_or_else(|| name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_projectile_exhaust_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default());
        // C++ getSecondaryDamage(bonus) — WeaponBonus::DAMAGE at fire time.
        let secondary_damage = self.effective_weapon_damage(
            native_rule
                .as_ref()
                .map(|rule| rule.secondary_damage)
                .unwrap_or_else(|| {
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_secondary_damage_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                }),
        );
        let secondary_damage_radius = native_rule.as_ref().map(|rule| rule.secondary_damage_radius).unwrap_or_else(||
            name.map(crate::game_logic::weapon_bootstrap::host_secondary_damage_radius_for_weapon_name).unwrap_or(0.0));
        let shock_wave_amount = native_rule
            .as_ref()
            .map(|rule| rule.shock_wave_amount)
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_shock_wave_amount_for_weapon_name,
                )
                .unwrap_or(0.0)
            });
        let shock_wave_radius = native_rule
            .as_ref()
            .map(|rule| rule.shock_wave_radius)
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_shock_wave_radius_for_weapon_name,
                )
                .unwrap_or(0.0)
            });
        let shock_wave_taper_off = native_rule
            .as_ref()
            .map(|rule| rule.shock_wave_taper_off)
            .unwrap_or_else(|| {
                name.map(crate::game_logic::weapon_bootstrap::host_shock_wave_taper_for_weapon_name)
                    .unwrap_or(0.0)
            });
        let radius_damage_affects = native_rule
            .as_ref()
            .map(|rule| rule.affects_mask.bits())
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_radius_damage_affects_for_weapon_name,
                )
                .unwrap_or(crate::game_logic::weapon_bootstrap::WEAPON_AFFECTS_DEFAULT)
            });
        let projectile_collides = native_rule
            .as_ref()
            .map(|rule| rule.collide_mask.bits())
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_projectile_collides_for_weapon_name,
                )
                .unwrap_or(crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT)
            });
        let scatter_table_offset = self.take_scatter_table_offset(slot, name);
        let scatter_radius = if scatter_table_offset.is_some() {
            0.0
        } else {
            native_rule
                .as_ref()
                .map(|rule| {
                    rule.scatter_radius
                        + if target_is_infantry {
                            rule.infantry_inaccuracy_dist
                        } else {
                            0.0
                        }
                })
                .unwrap_or_else(|| {
                    name.map(|weapon_name| {
                        crate::game_logic::weapon_bootstrap::host_effective_scatter_radius(
                            weapon_name,
                            target_is_infantry,
                        )
                    })
                    .unwrap_or(0.0)
                })
        };
        let speed_peel = native_rule
            .as_ref()
            .map(
                |rule| crate::game_logic::weapon_bootstrap::HostWeaponSpeedPeel {
                    weapon_speed: rule.weapon_speed,
                    min_weapon_speed: rule.min_weapon_speed,
                    scale_weapon_speed: rule.is_scale_weapon_speed,
                    attack_range: rule.attack_range,
                    min_attack_range: rule.minimum_attack_range,
                },
            )
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name,
                )
                .unwrap_or_default()
            });
        let historic_bonus = native_rule
            .as_ref()
            .map(
                |rule| crate::game_logic::weapon_bootstrap::HostHistoricBonusPeel {
                    time_frames: rule.historic_bonus_time,
                    count: rule.historic_bonus_count,
                    radius: rule.historic_bonus_radius.max(0.0),
                    bonus_weapon: rule.historic_bonus_weapon_name.clone(),
                },
            )
            .unwrap_or_else(|| {
                name.map(crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name)
                    .unwrap_or_default()
            });

        {
            let _ = self.capture_pending_weapon_visual_dispatch(
                slot,
                logic_frame,
                Some(target_id),
                None,
            );
        }
        let (weapon_speed, weapon_splash, weapon_homing, auto_reloaded_clip, clip_emptied) =
            match self.weapon_slot_mut(slot) {
                Some(weapon) => {
                    Self::consume_ammo_on_fire_selected(
                        weapon,
                        current_time,
                        name,
                        native_rule.as_deref(),
                    );
                    let clip_emptied = weapon.ammo == Some(0);
                    (
                        weapon.projectile_speed,
                        weapon.splash_radius,
                        // AA residual: air-only weapons home on the live target.
                        weapon.can_target_air && !weapon.can_target_ground,
                        Self::auto_reloaded_clip_after_firing_selected(
                            weapon,
                            name,
                            native_rule.as_deref(),
                        ),
                        clip_emptied,
                    )
                }
                None => return None,
            };
        if clip_emptied {
            if let Some(weapon) = self.weapon_slot_mut(slot) {
                weapon.reloading_clip = true;
            }
            self.rebuild_scatter_targets_unused(slot, name);
        } else {
            if let Some(weapon) = self.weapon_slot_mut(slot) {
                weapon.reloading_clip = false;
            }
            if let Some(weapon_name) = name {
                let rof = self.weapon_bonus_fields().2;
                let rolled = native_rule
                    .as_ref()
                    .map(|rule| {
                        let mut bonus = gamelogic::weapon::WeaponBonus::new();
                        bonus.set_field(
                            gamelogic::weapon::WeaponBonusField::RateOfFire,
                            rof.max(0.01),
                        );
                        rule.get_delay_between_shots(&bonus) as f32 / 30.0
                    })
                    .or_else(|| {
                        crate::game_logic::weapon_bootstrap::host_delay_between_shots_secs_with_rof(
                            weapon_name,
                            rof,
                        )
                    });
                if let Some(rolled) = rolled {
                    let nominal = self
                        .weapon_slot(slot)
                        .map(|weapon| self.live_reload_interval(weapon, Some(weapon_name), rof))
                        .unwrap_or(rolled);
                    if (rolled - nominal).abs() > 1e-6 {
                        if let Some(weapon) = self.weapon_slot_mut(slot) {
                            weapon.last_fire_time = current_time + rolled - nominal;
                        }
                    }
                }
            }
        }
        let shooter_id = self.id;
        let shooter_pos = match route {
            WeaponShotRoute::Autonomous => self.get_position(),
            WeaponShotRoute::Passive { origin, .. } => origin,
        };
        self.target = Some(target_id);

        // Prefer Weapon.ini DamageType via store name; shape residual if the
        // source name has no usable host store entry.
        let weapon_dtype = if let Some(rule) = native_rule.as_ref() {
            crate::game_logic::host_armor_residual::map_store_damage_type(rule.damage_type)
        } else if let Some(weapon_name) = name {
            if crate::game_logic::thing::ThingTemplate::weapon_from_host_store(weapon_name)
                .is_some()
            {
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
        } else {
            // C++ WeaponTemplate::WeaponTemplate defaults to DAMAGE_EXPLOSION.
            // Travel speed and radius do not classify an unnamed weapon.
            super::combat::DamageType::Explosive
        };
        let at_self = native_rule.as_ref().map(|rule| rule.damage_dealt_at_self_position).unwrap_or_else(||
            name.map(crate::game_logic::weapon_bootstrap::host_damage_dealt_at_self_position_for_weapon_name).unwrap_or(false));
        // One receiving CombatSystem owns projectileless travel and damage.
        // Never mirror the shot into a different Core world's WeaponStore.
        let pending = super::combat::PendingProjectile {
            shooter_id,
            shooter_pos,
            source_context: Some(super::combat::ProjectileLaunchContext {
                source_team: self.team,
                source_owner_player_id: self.owner_player_id,
                source_veterancy: veterancy,
                source_orientation: self.get_orientation(),
                source_velocity: self.movement.velocity,
            }),
            // C++ DamageDealtAtSelfPosition: damageID=INVALID, damagePos=source.
            target_id: if at_self { None } else { Some(target_id) },
            target_pos: if at_self { Some(shooter_pos) } else { None },
            damage: weapon_damage,
            speed: weapon_speed,
            speed_unit: super::combat::ProjectileSpeedUnit::DistancePerLogicFrame,
            splash_radius: weapon_splash,
            is_homing: weapon_homing,
            damage_type: weapon_dtype,
            death_type: if let Some(rule) = native_rule.as_ref() {
                crate::game_logic::host_usa_pilot::HostDeathType::from_store(rule.death_type)
            } else if name.is_none() {
                // C++ initializes DEATH_NORMAL independently of DamageType.
                crate::game_logic::host_usa_pilot::HostDeathType::Normal
            } else {
                crate::game_logic::host_armor_residual::resolve_host_death_type(name, weapon_dtype)
            },
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
            scatter_table_offset,
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
            die_on_detonate: native_rule
                .as_ref()
                .map(|rule| rule.die_on_detonate)
                .unwrap_or_else(|| {
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_die_on_detonate_for_weapon_name,
                    )
                    .unwrap_or(false)
                }),
        };

        // C++ fireWeaponTemplate LeechRange activate residual.
        self.activate_leech_range_for_slot(slot);
        self.record_shot_at_target(Some(target_id), logic_frame, possible_next_shot_frame);
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
            let frame = logic_frame;
            self.stamp_fire_sound_loop_after_shot(frame, fire_weapon_name.as_deref(), sound_events);
            // C++ FiringTracker::shotFired(weaponFired) — delay from this slot.
            self.stamp_auto_reload_when_idle_from_slot(slot, frame);
        }
        {
            let (dmg, rng) = self
                .weapon_slot(slot)
                .map(|w| (w.damage, w.range))
                .unwrap_or((0.0, 0.0));
            let frame = logic_frame;
            let next_count = self.fire_intent_count.saturating_add(1);
            // When AI attack authority is on, GameWorld SetFireIntent writeback is
            // last-writer — log the intent without dual-writing host last_fire_*.
            {
                self.last_fire_victim_host = target_id.0;
                self.last_fire_slot = slot;
                self.last_fire_damage = dmg;
                self.last_fire_range = rng;
                self.last_fire_sim_time = current_time;
                self.last_fire_frame = frame;
                self.fire_intent_count = next_count;
                #[cfg(test)]
                self.record_host_fire_intent();
            }
        }

        // C++ STEALTH_NOT_WHILE_ATTACKING / IS_FIRING_WEAPON residual:
        // fire clears STEALTHED. receiveGrant (StealthUpdate.cpp:198) keeps
        // OBJECT_STATUS_CAN_STEALTH so the unit re-cloaks after StealthDelay.
        if self.stealth_breaks_on_attack && self.status.stealthed {
            let can_stealth = self.innate_stealth;
            self.break_stealth();
            if can_stealth {
                self.innate_stealth = true;
            }
        }
        Some((slot, pending))
    }

    pub(crate) fn move_to(
        &mut self,
        position: Vec3,
        movement_events: &mut crate::game_logic::movement_events::MovementEventWriter<'_>,
    ) {
        if self.is_mobile() && self.is_alive() {
            self.movement_goal_object = None;
            self.movement.target_position = Some(position);
            self.set_ai_state(AIState::Moving);
            self.set_status_moving(true);
            movement_events.record(self.id, Some([position.x, position.y, position.z]));
        }
    }
    /// Drop a `setTemporaryState` move without `onExit` of the underlying state.
    pub fn end_temporary_move_overlay(&mut self) {
        self.temporary_move_frames = 0;
        if let Some(name) = self.move_loop_audio.take() {
            crate::game_logic::host_move_ambient_audio::record_move_loop_stop(
                self.id,
                name,
                self.get_position(),
            );
        }
        self.movement.velocity = glam::Vec3::ZERO;
        self.movement.path.clear();
        self.movement.current_path_index = 0;
        self.movement.target_position = None;
        self.set_status_moving(false);
        if let Some(dest) = self.temporary_move_saved_dest.take() {
            self.requested_destination = Some(dest);
        }
        if let Some(goal) = self.temporary_move_saved_path_goal.take() {
            self.path_goal_position = Some(goal);
        }
    }

    pub(crate) fn movement_goal_object(&self) -> Option<ObjectId> {
        self.movement_goal_object
    }

    pub(crate) fn set_movement_goal_object(&mut self, goal: Option<ObjectId>) {
        self.movement_goal_object = goal;
    }

    pub fn stop_moving(&mut self) {
        self.movement_goal_object = None;
        let overlay = self.temporary_move_frames > 0;
        self.end_temporary_move_overlay();
        if let Some(name) = self.move_loop_audio.take() {
            crate::game_logic::host_move_ambient_audio::record_move_loop_stop(
                self.id,
                name,
                self.get_position(),
            );
        }
        self.movement.target_position = None;
        self.temporary_move_frames = 0;
        self.movement.velocity = Vec3::ZERO;
        self.set_status_moving(false);
        self.queue_for_path_frames = 0;
        self.movement.path.clear();
        self.movement.current_path_index = 0;
        self.set_status_moving(false);
        self.requested_destination = None;
        self.waiting_for_path = false;
        self.is_attack_path = false;
        self.is_approach_path = false;
        self.is_safe_path = false;
        self.path_extra_distance = 0.0;
        if self.is_kind_of(crate::game_logic::KindOf::Projectile) {
            self.set_precise_z_pos(false);
        }

        self.is_blocked = false;
        self.is_blocked_and_stuck = false;
        // Only pure locomotion returns to Idle when the destination is reached.
        // Interaction states (Capturing, Repairing, SpecialAbility, Entering, …)
        // set a destination while remaining in-state; clobbering them to Idle
        // aborted capture/repair on arrival before support-state resolution.
        if !overlay && matches!(self.ai_state, AIState::Moving | AIState::AttackMoving) {
            self.set_ai_state(AIState::Idle);
        }
    }

    pub fn attack_target(
        &mut self,
        target_id: ObjectId,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
    ) {
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

                self.pre_attack_ready_at = 0.0;
            }
            self.target = Some(target_id);
            self.target_location = None;
            self.set_status_force_attack(false);
            self.set_ai_state(AIState::Attacking);
            self.status.attacking = true;
            attack_events.record(self.id, target_id);
        }
    }

    /// C++ Weapon::setLeechRangeActive residual for a weapon slot.

    /// C++ FiringTracker::shotFired FireSoundLoopTime residual.
    /// Extends the looping fire-audio deadline; records start when newly armed.
    pub fn stamp_fire_sound_loop_after_shot(
        &mut self,
        frame: u32,
        weapon_name: Option<&str>,
        events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
    ) {
        let native = (0..3).find_map(|slot| {
            let rule = self.selected_native_weapon_rule(slot)?;
            weapon_name
                .is_some_and(|name| rule.name.eq_ignore_ascii_case(name))
                .then_some(rule)
        });
        let loop_frames = native.as_ref().map(|rule| rule.fire_sound_loop_time).unwrap_or_else(||
            weapon_name.map(crate::game_logic::weapon_bootstrap::host_fire_sound_loop_frames_for_weapon_name).unwrap_or(0));
        if loop_frames == 0 {
            return;
        }
        let sound = native
            .as_ref()
            .map(|rule| rule.fire_sound.name().to_owned())
            .unwrap_or_else(|| {
                weapon_name
                    .map(crate::game_logic::weapon_bootstrap::host_fire_sound_for_weapon_name)
                    .unwrap_or_default()
            });
        if sound.is_empty() {
            return;
        }
        let was_active = self.fire_sound_loop_until_frame > frame;
        self.fire_sound_loop_until_frame = frame.wrapping_add(loop_frames);
        self.fire_sound_loop_name = sound.clone();
        if !was_active {
            events.record(self.id, sound, true);
        }
    }

    /// C++ FiringTracker::update stop-loop residual when deadline elapses.
    pub fn tick_fire_sound_loop(
        &mut self,
        frame: u32,
        events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
    ) {
        if self.fire_sound_loop_until_frame == 0 {
            return;
        }
        if frame >= self.fire_sound_loop_until_frame {
            let sound = std::mem::take(&mut self.fire_sound_loop_name);
            self.fire_sound_loop_until_frame = 0;
            if !sound.is_empty() {
                events.record(self.id, sound, false);
            }
        }
    }

    pub fn activate_leech_range_for_slot(&mut self, slot: u8) {
        let name = self.weapon_name_for_slot(slot);
        let is_leech = self
            .selected_native_weapon_rule(slot)
            .map(|rule| rule.leech_range_weapon)
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_leech_range_weapon_for_weapon_name,
                )
                .unwrap_or(false)
            });
        if !is_leech {
            return;
        }
        match slot {
            0 => {
                self.leech_range_active_primary = true;
            }
            1 => {
                self.leech_range_active_secondary = true;
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
        self.leech_range_active_secondary = false;
    }

    pub fn stop_attack(&mut self) {
        // C++ WeaponSet::releaseWeaponLock: any non-special player order and
        // an exited attack state release a temporary special-weapon lock.
        // Permanent user locks remain intact.
        self.release_weapon_lock(WeaponLockType::LockedTemporarily);
        self.target = None;
        self.target_location = None;

        self.set_status_force_attack(false);
        self.pre_attack_target = None;

        self.pre_attack_ready_at = 0.0;

        self.consecutive_shot_target = None;
        self.consecutive_shots_at_target = 0;

        self.clear_leech_range_mode_for_all_weapons();
        self.status.attacking = false;
        self.set_status_aiming_weapon(false);
        self.set_status_firing_weapon(false);
        self.attack_substate = crate::game_logic::AttackSubState::AimAtTarget;

        // C++ parity: guard units return to guard; Hunt stays in AI_HUNT;
        // attack-move keeps AI_ATTACK_MOVE_TO so dest can resume.
        let stay_hunt = matches!(self.ai_state, AIState::Patrolling);
        let stay_attack_move = matches!(self.ai_state, AIState::AttackMoving);
        if self.guard_target.is_some() {
            self.set_ai_state(AIState::GuardingObject);
        } else if self.guard_position.is_some() {
            self.set_ai_state(AIState::GuardingArea);
        } else if stay_hunt {
            self.set_ai_state(AIState::Patrolling);
        } else if stay_attack_move {
            // Nested attack just went idle. The next getNextMoodTarget must
            // not wait out the check rate (forceRetargetThisFrame).
            self.next_mood_check_time = 0;
            if let Some(src) = self.attack_move_command_src.take() {
                self.last_command_source = src;
            }
            if let Some(dest) = self.requested_destination {
                self.movement.path.clear();
                self.movement.current_path_index = 0;
                self.movement.target_position = Some(dest);
                self.set_status_moving(true);
            }
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
    /// DelayBetweenShots or ClipReloadTime, both scaled by RATE_OF_FIRE.
    ///
    /// Template delay is preferred so baked PRIMARY reload scalars are not
    /// stacked with `weapon_bonus_fields` veterancy. Ready-checks use leftover
    /// `get_delay_between_shots` on the Min yardstick (no GameLogicRandomValue);
    /// per-shot leftover roll + ROF floor is applied at fire (C++
    /// `privateFireWeapon` / leftover `get_delay_between_shots`).
    pub(super) fn live_reload_interval(
        &self,
        weapon: &Weapon,
        weapon_name: Option<&str>,
        rof: f32,
    ) -> f32 {
        // A supplied immutable row is authoritative even when its delay is zero.
        // Readiness uses the minimum yardstick; only accepted fire draws the range.
        let native = (0..3).find_map(|slot| {
            let rule = self.selected_native_weapon_rule(slot)?;
            weapon_name
                .is_some_and(|name| rule.name.eq_ignore_ascii_case(name))
                .then_some(rule)
        });
        if let Some(rule) = native {
            let waiting_clip = weapon.reloading_clip
                || (rule.get_auto_reloads_clip() && weapon.clip_size > 0 && weapon.ammo == Some(0));
            if waiting_clip {
                return (weapon.clip_reload_time / rof.max(0.01)).max(0.0);
            }
            return ((rule.min_delay_between_shots as f32 / rof.max(0.01)).floor() / 30.0).max(0.0);
        }
        let auto_clip = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_reload_type_for_weapon_name)
            .unwrap_or(crate::game_logic::weapon_bootstrap::HostReloadType::Auto)
            == crate::game_logic::weapon_bootstrap::HostReloadType::Auto;
        let waiting_clip =
            weapon.reloading_clip || (auto_clip && weapon.clip_size > 0 && weapon.ammo == Some(0));
        if waiting_clip {
            return (weapon.clip_reload_time / rof.max(0.01)).max(0.0);
        }
        if let Some(name) = weapon_name {
            if let Some(secs) =
                crate::game_logic::weapon_bootstrap::host_delay_between_shots_secs_nominal_with_rof(
                    name, rof,
                )
            {
                return secs;
            }
        }
        // Fallback: leftover ROF floor on the baked yardstick (seconds → frames).
        let frames = (weapon.reload_time * 30.0).round();
        ((frames / rof.max(0.01)).floor() / 30.0).max(0.0)
    }

    /// C++ `Weapon::rebuildScatterTargets` — refill unused indices for this clip.
    pub(crate) fn rebuild_scatter_targets_unused(&mut self, slot: u8, weapon_name: Option<&str>) {
        let index = usize::from(slot);
        if index >= 3 {
            return;
        }
        let count = self
            .selected_native_weapon_rule(slot)
            .map(|rule| rule.scatter_targets.len())
            .unwrap_or_else(|| {
                weapon_name
                    .map(crate::game_logic::weapon_bootstrap::host_scatter_targets_for_weapon_name)
                    .map(|targets| targets.len())
                    .unwrap_or(0)
            });
        self.weapon_scatter_targets_unused[index] = (0..count)
            .filter_map(|idx| i32::try_from(idx).ok())
            .collect();
        self.weapon_scatter_targets_inited[index] = true;
    }

    /// C++ Weapon.cpp:2584-2609 — one unused ScatterTarget per shot.
    pub(crate) fn take_scatter_table_offset(
        &mut self,
        slot: u8,
        weapon_name: Option<&str>,
    ) -> Option<glam::Vec2> {
        let name = weapon_name?;
        let native = self.selected_native_weapon_rule(slot);
        let targets = native
            .as_ref()
            .map(|rule| {
                rule.scatter_targets
                    .iter()
                    .map(|point| (point.x, point.y))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                crate::game_logic::weapon_bootstrap::host_scatter_targets_for_weapon_name(name)
            });
        if targets.is_empty() {
            return None;
        }
        let index = usize::from(slot);
        if index >= 3 {
            return None;
        }
        if !self.weapon_scatter_targets_inited[index] {
            self.rebuild_scatter_targets_unused(slot, Some(name));
        }
        if self.weapon_scatter_targets_unused[index].is_empty() {
            return None;
        }
        let last = self.weapon_scatter_targets_unused[index].len() as i32 - 1;
        let seed = self.id.0.wrapping_add(self.last_fire_frame);
        let pick =
            crate::game_logic::host_rng_residual::pure_logic_random_int(seed, 0, 0, last) as usize;
        let target_index = self.weapon_scatter_targets_unused[index][pick] as usize;
        let (offset_x, offset_y) = *targets.get(target_index)?;
        let scalar = native
            .as_ref()
            .map(|rule| rule.scatter_target_scalar)
            .unwrap_or_else(|| {
                crate::game_logic::weapon_bootstrap::host_scatter_target_scalar_for_weapon_name(
                    name,
                )
            });
        self.weapon_scatter_targets_unused[index].swap_remove(pick);
        Some(glam::Vec2::new(offset_x * scalar, offset_y * scalar))
    }
}

#[cfg(test)]
#[path = "projectileless_fire_tests.rs"]
mod projectileless_fire_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_tertiary_fire_uses_the_real_third_slot() {
        let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

        let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
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

        assert!(attacker.fire_at(ObjectId(2), 1.0, 0, &mut shot_combat, &mut _attack_output,));
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

    #[test]
    fn fire_at_grant_stealth_keeps_can_stealth() {
        let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

        let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
        // C++ StealthUpdate.cpp:198 receiveGrant stays CAN_STEALTH after fire.
        let mut unit = Object::new(ThingTemplate::new("TestTank"), ObjectId(1), Team::GLA);
        unit.weapon = Some(Weapon {
            damage: 20.0,
            range: 150.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        unit.apply_grant_stealth();
        unit.stealth_breaks_on_attack = true;
        unit.stealth_delay_frames = 30;
        assert!(unit.fire_at(ObjectId(2), 1.0, 0, &mut shot_combat, &mut _attack_output,));
        assert!(!unit.status.stealthed);
        assert!(
            unit.innate_stealth,
            "GPS grant CAN_STEALTH must survive fire"
        );
        assert!(unit.stealth_delay_pending);
        // C++ StealthUpdate.cpp:717-752 evaluates the disruption and rearms
        // the timer during the actual update before a later allowed update.
        assert!(!unit.try_recloak_after_stealth_delay(1, true));
        assert!(!unit.stealth_delay_pending);
        assert_eq!(unit.stealth_allowed_frame, 31);
        assert!(!unit.try_recloak_after_stealth_delay(30, false));
        assert!(!unit.status.stealthed);
        assert!(unit.try_recloak_after_stealth_delay(31, false));
        assert!(unit.status.stealthed);
    }

    #[test]
    fn fire_at_rejects_sold_and_under_construction() {
        let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

        let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
        let mut unit = Object::new(ThingTemplate::new("PatriotBattery"), ObjectId(1), Team::USA);
        unit.weapon = Some(Weapon {
            damage: 20.0,
            range: 150.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        unit.status.under_construction = true;
        assert!(!unit.fire_at(ObjectId(2), 1.0, 0, &mut shot_combat, &mut _attack_output,));
        unit.status.under_construction = false;
        unit.status.sold = true;
        assert!(!unit.fire_at(ObjectId(2), 1.0, 0, &mut shot_combat, &mut _attack_output,));
        unit.status.sold = false;
        assert!(unit.fire_at(ObjectId(2), 1.0, 0, &mut shot_combat, &mut _attack_output,));
    }
}

#[cfg(test)]
#[path = "passive_shot_tests.rs"]
mod passive_shot_tests;
