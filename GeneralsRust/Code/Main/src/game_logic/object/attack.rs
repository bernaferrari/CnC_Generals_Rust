use super::*;

#[derive(Clone, Copy)]
enum WeaponShotRoute {
    Autonomous,
    Passive {
        slot: u8,
        origin: Vec3,
        damage_bonus: Option<f32>,
    },
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
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> bool {
        self.fire_at_ex(
            target_id,
            current_time,
            false,
            false,
            logic_frame,
            combat,
            attack_events,
            audio,
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
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> bool {
        let Some(slot) = self.fire_at_ex_defer_weapon_barrel_advance(
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            combat,
            attack_events,
            audio,
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
        audio: &mut crate::game_logic::ObjectAudio<'_>,
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
            audio,
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
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_passive_weapon_shot_with_operation(
            slot,
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            muzzle_origin,
            sound_events,
            attack_events,
            content,
            audio,
            0,
            &mut Default::default(),
        )
    }

    pub(crate) fn prepare_passive_weapon_shot_with_operation(
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
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            WeaponShotRoute::Passive {
                slot,
                origin: muzzle_origin,
                damage_bonus: None,
            },
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            sound_events,
            attack_events,
            content,
            audio,
            operation_frame,
            operation_effects,
            None,
            None,
            None,
        )
    }

    /// Passive/contained-unit discharge using the receiving GameLogic's
    /// admitted SOLO set. The common shot operation retains its read order.
    pub(crate) fn prepare_passive_weapon_shot_with_operation_and_solo_set(
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
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        solo_set: &gamelogic::weapon::WeaponBonusSet,
        damage_bonus: f32,
        random: &game_engine::common::random_value::SessionRandom,
        projectile_team_id: Option<u32>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            WeaponShotRoute::Passive {
                slot,
                origin: muzzle_origin,
                damage_bonus: Some(damage_bonus),
            },
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            sound_events,
            attack_events,
            content,
            audio,
            operation_frame,
            operation_effects,
            Some(solo_set),
            Some(random),
            projectile_team_id,
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
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_autonomous_weapon_shot_with_operation(
            target,
            time,
            infantry,
            faerie,
            frame,
            sound,
            attacks,
            content,
            audio,
            0,
            &mut Default::default(),
        )
    }

    pub(crate) fn prepare_autonomous_weapon_shot_with_operation(
        &mut self,
        target: ObjectId,
        time: f32,
        infantry: bool,
        faerie: bool,
        frame: u32,
        sound: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attacks: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            WeaponShotRoute::Autonomous,
            target,
            time,
            infantry,
            faerie,
            frame,
            sound,
            attacks,
            content,
            audio,
            operation_frame,
            operation_effects,
            None,
            None,
            None,
        )
    }

    /// Autonomous AttackStateMachine shot using its receiving GameLogic owner's
    /// immutable SOLO WeaponBonusSet. The borrow is observed at every existing
    /// bonus read point in the unchanged shot operation order.
    pub(crate) fn prepare_autonomous_weapon_shot_with_operation_and_solo_set(
        &mut self,
        target: ObjectId,
        time: f32,
        infantry: bool,
        faerie: bool,
        frame: u32,
        sound: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attacks: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        solo_set: &gamelogic::weapon::WeaponBonusSet,
        random: &game_engine::common::random_value::SessionRandom,
        projectile_team_id: Option<u32>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            WeaponShotRoute::Autonomous,
            target,
            time,
            infantry,
            faerie,
            frame,
            sound,
            attacks,
            content,
            audio,
            operation_frame,
            operation_effects,
            Some(solo_set),
            Some(random),
            projectile_team_id,
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
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            route,
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            sound_events,
            attack_events,
            content,
            audio,
            0,
            &mut Default::default(),
            None,
            None,
            None,
        )
    }

    fn prepare_weapon_shot_with_operation(
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
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        solo_set: Option<&gamelogic::weapon::WeaponBonusSet>,
        random: Option<&game_engine::common::random_value::SessionRandom>,
        projectile_team_id: Option<u32>,
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
        self.apply_weapon_bonus_rof_restart_with_optional_solo_set(current_time, solo_set);

        // Prefer an explicit locked/active slot when ready.  The normal
        // fallback remains PRIMARY then SECONDARY; TERTIARY is deliberately
        // excluded from autonomous selection (retail Comanche rocket pods
        // declare `AutoChooseSources = TERTIARY NONE`).
        let slot = if let WeaponShotRoute::Passive { slot, .. } = route {
            let weapon = self.weapon_slot(slot)?;
            let name = self.weapon_name_for_slot(slot);
            let mut rof = solo_set.map_or_else(
                || self.weapon_bonus_fields().2,
                |set| self.weapon_bonus_fields_with_solo_set(set).2,
            );
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
            let mut rof = solo_set.map_or_else(
                || self.weapon_bonus_fields().2,
                |set| self.weapon_bonus_fields_with_solo_set(set).2,
            );
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
            base * solo_set.map_or_else(
                || self.weapon_bonus_fields().3,
                |set| self.weapon_bonus_fields_with_solo_set(set).3,
            )
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
                self.note_attack_target_with_operation(
                    target_id,
                    attack_events,
                    audio,
                    operation_frame,
                    operation_effects,
                );
                let entered_attack = !matches!(
                    self.ai_state(),
                    AIState::Patrolling | AIState::AttackMoving | AIState::Attacking
                );
                if autonomous && entered_attack {
                    self.set_ai_state(AIState::Attacking, audio);
                }
                {}
                return None;
            }
            // Delay complete — fall through to fire; record_shot clears ready_at.
        } else {
            self.pre_attack_target = Some(target_id);
        }

        self.note_attack_target_with_operation(
            target_id,
            attack_events,
            audio,
            operation_frame,
            operation_effects,
        );
        let entered_attack = !matches!(
            self.ai_state(),
            AIState::Patrolling | AIState::AttackMoving | AIState::Attacking
        );
        if autonomous && entered_attack {
            self.set_ai_state(AIState::Attacking, audio);
        }
        {}

        let possible_next_shot_frame = self
            .possible_next_shot_frame_for_slot_with_optional_solo_set(
                slot,
                logic_frame,
                target_has_faerie_fire,
                solo_set,
            );
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
        let damage_bonus = match route {
            WeaponShotRoute::Passive { damage_bonus, .. } => damage_bonus,
            WeaponShotRoute::Autonomous => None,
        };
        let weapon_damage = damage_bonus.map_or_else(
            || {
                solo_set.map_or_else(
                    || self.effective_weapon_damage(template_damage),
                    |set| self.effective_weapon_damage_with_solo_set(template_damage, set),
                )
            },
            |bonus| template_damage * bonus,
        );

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
        let secondary_base = native_rule
            .as_ref()
            .map(|rule| rule.secondary_damage)
            .unwrap_or_else(|| {
                name.map(crate::game_logic::weapon_bootstrap::host_secondary_damage_for_weapon_name)
                    .unwrap_or(0.0)
            });
        let secondary_damage = damage_bonus.map_or_else(
            || {
                solo_set.map_or_else(
                    || self.effective_weapon_damage(secondary_base),
                    |set| self.effective_weapon_damage_with_solo_set(secondary_base, set),
                )
            },
            |bonus| secondary_base * bonus,
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
                let rof = solo_set.map_or_else(
                    || self.weapon_bonus_fields().2,
                    |set| self.weapon_bonus_fields_with_solo_set(set).2,
                );
                let rolled = native_rule
                    .as_ref()
                    .map(|rule| {
                        let mut bonus = gamelogic::weapon::WeaponBonus::new();
                        bonus.set_field(
                            gamelogic::weapon::WeaponBonusField::RateOfFire,
                            rof.max(0.01),
                        );
                        (match random {
                            Some(random) => rule.get_delay_between_shots_with_random(&bonus, random),
                            None => rule.get_delay_between_shots(&bonus),
                        }) as f32 / 30.0
                    })
                    .or_else(|| match random {
                        Some(random) => crate::game_logic::weapon_bootstrap::host_delay_between_shots_secs_with_rof_and_random(
                            weapon_name, rof, random,
                        ),
                        None => crate::game_logic::weapon_bootstrap::host_delay_between_shots_secs_with_rof(
                            weapon_name, rof,
                        ),
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
        let projectile_lifecycle = content
            .and_then(|definitions| definitions.get(&projectile_object_name))
            .map(|profile| profile.lifecycle);
        let pending = super::combat::PendingProjectile {
            shooter_id,
            shooter_pos,
            source_context: Some(super::combat::ProjectileLaunchContext {
                source_team: self.team,
                projectile_team_id,
                source_owner_player_id: self.owner_player_id,
                source_veterancy: veterancy,
                source_orientation: self.get_orientation(),
                source_velocity: self.movement.velocity,
            }),
            // Weapon.cpp uses the intended target to determine travel time,
            // even when the eventual projectileless damage is at the source.
            target_id: Some(target_id),
            target_pos: None,
            self_damage_position: at_self.then_some(shooter_pos),
            selected_weapon_is_laser: native_rule.as_ref().map(|rule| rule.is_laser()),
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
            projectile_lifecycle,
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
        if let Some(set) = solo_set {
            self.record_shot_at_target_with_solo_set(
                Some(target_id),
                logic_frame,
                possible_next_shot_frame,
                set,
            );
        } else {
            self.record_shot_at_target(Some(target_id), logic_frame, possible_next_shot_frame);
        }
        // C++ --m_maxShotCount residual.
        self.consume_max_shot_count();
        if let Some(set) = solo_set {
            self.refresh_weapon_fire_status_with_solo_set(current_time, set);
        } else {
            self.refresh_weapon_fire_status(current_time);
        }
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
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        self.move_to_inner(position, None, movement_events, audio);
    }

    pub(crate) fn move_to_at_frame(
        &mut self,
        position: Vec3,
        frame: u32,
        movement_events: &mut crate::game_logic::movement_events::MovementEventWriter<'_>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        self.move_to_inner(position, Some(frame), movement_events, audio);
    }

    fn move_to_inner(
        &mut self,
        position: Vec3,
        operation_frame: Option<u32>,
        movement_events: &mut crate::game_logic::movement_events::MovementEventWriter<'_>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        if self.is_mobile() && self.is_alive() {
            self.movement_goal_object = None;
            self.movement.target_position = Some(position);
            if let Some(frame) = operation_frame {
                self.set_ai_state_at_frame(AIState::Moving, frame, audio);
            } else {
                self.set_ai_state(AIState::Moving, audio);
            }
            self.set_status_moving(true);
            movement_events.record(self.id, Some([position.x, position.y, position.z]));
        }
    }
    /// Drop a `setTemporaryState` move without `onExit` of the underlying state.
    pub fn end_temporary_move_overlay(&mut self, audio: &mut crate::game_logic::ObjectAudio<'_>) {
        let Some(previous_temporary) = self.unit_ai_runtime.take_temporary_move() else {
            return;
        };
        audio.stop_loop(self);
        self.movement.velocity = glam::Vec3::ZERO;
        self.movement.path.clear();
        self.movement.current_path_index = 0;
        self.movement.target_position = None;
        self.set_status_moving(false);
        self.queue_for_path_frames = 0;
        self.waiting_for_path = false;
        // None is meaningful: do not leave the temporary goal installed when
        // the underlying machine never had a physical destination.
        self.requested_destination = previous_temporary.saved_destination;
        self.path_goal_position = previous_temporary.saved_path_goal;
    }

    pub(crate) fn movement_goal_object(&self) -> Option<ObjectId> {
        self.movement_goal_object
    }

    pub(crate) fn set_movement_goal_object(&mut self, goal: Option<ObjectId>) {
        self.movement_goal_object = goal;
    }

    pub fn stop_moving(&mut self, audio: &mut crate::game_logic::ObjectAudio<'_>) {
        self.stop_moving_with_idle_transition(audio);
    }

    /// Stop locomotion and report whether a normal move leaf reached its
    /// terminal point. The World owns any resulting AI transition.
    pub(crate) fn stop_moving_for_owned_idle_entry(
        &mut self,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> bool {
        self.stop_moving_with_idle_transition(audio)
    }

    fn stop_moving_with_idle_transition(
        &mut self,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> bool {
        let overlay = self.has_temporary_ai_move();
        self.end_temporary_move_overlay(audio);
        if overlay {
            return false;
        }
        self.movement_goal_object = None;
        audio.stop_loop(self);
        self.movement.target_position = None;
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
        // Physical movement can stop without completing the current AI state:
        // the C++ AI state machine, not Object locomotion, owns that transition.
        !matches!(
            self.retained_ai_root_state(),
            AIState::JetReturningForLanding | AIState::JetReturnToDeadAirfield
        ) && matches!(self.ai_state(), AIState::Moving | AIState::AttackMoving)
    }

    pub fn attack_target(
        &mut self,
        target_id: ObjectId,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        self.attack_target_with_operation(
            target_id,
            attack_events,
            audio,
            0,
            &mut Default::default(),
        )
    }

    pub fn attack_target_with_operation(
        &mut self,
        target_id: ObjectId,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if !self.is_alive() {
            return;
        }
        // Shock stun residual: ignore new attack orders while stunned.
        if self.is_shock_stunned() {
            return;
        }
        // Jet takeoff residual: leave hangar before engaging.
        let _ = self.takeoff_from_airfield_parking_with_operation(
            audio,
            operation_frame,
            operation_effects,
        );
        if self.can_attack() {
            if self.pre_attack_target != Some(target_id) {
                // New target — fire_at will start PRE_ATTACK clock.
                self.pre_attack_target = None;

                self.pre_attack_ready_at = 0.0;
            }
            self.target = Some(target_id);
            self.target_location = None;
            self.set_status_force_attack(false);
            self.set_ai_state(AIState::Attacking, audio);
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

    fn clear_attack_scalar_state(&mut self) {
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
        self.set_attack_substate(crate::game_logic::AttackSubState::AimAtTarget);
    }

    /// Clear combat engagement data without changing the retained AI machine.
    /// The owning World applies any state-machine result at its observation point.
    pub fn stop_attack(&mut self, _audio: &mut crate::game_logic::ObjectAudio<'_>) {
        self.clear_attack_scalar_state();
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

        assert!(attacker.fire_at(
            ObjectId(2),
            1.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new())
        ));
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
        assert!(unit.fire_at(
            ObjectId(2),
            1.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new())
        ));
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
        assert!(!unit.fire_at(
            ObjectId(2),
            1.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new())
        ));
        unit.status.under_construction = false;
        unit.status.sold = true;
        assert!(!unit.fire_at(
            ObjectId(2),
            1.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new())
        ));
        unit.status.sold = false;
        assert!(unit.fire_at(
            ObjectId(2),
            1.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new())
        ));
    }
}

#[cfg(test)]
#[path = "passive_shot_tests.rs"]
mod passive_shot_tests;
