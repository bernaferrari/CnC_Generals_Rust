//! Host tick `impl GameLogic` — `combat`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    pub(crate) fn update_combat(&mut self, object_ids: &[ObjectId], _dt: f32) {
        for &attacker_id in object_ids {
            // Empty-clip RTB is JetAI (guard/hunt interrupt or idle), not every attack.

            // Early gates + docked/garrisoned flags in one immutable scope.
            let (docked_sortie, docked_passenger, garrisoned) = {
                let Some(attacker) = self.objects.get(&attacker_id) else {
                    continue;
                };
                // Need at least one weapon slot bound.
                if ![0u8, 1, 2]
                    .into_iter()
                    .any(|slot| attacker.weapon_slot(slot).is_some())
                {
                    continue;
                }
                // ECM jam residual: C++ canFireWeapon DISABLED_SUBDUED — no fire while jammed.
                if attacker.status.weapons_jammed || attacker.is_disabled() {
                    continue;
                }
                // Nested AttackStateMachine residual owns aim/fire/approach for these units.
                if attacker.status.is_aiming_weapon
                    || attacker.status.is_firing_weapon
                    || !matches!(
                        attacker.attack_substate,
                        crate::game_logic::AttackSubState::AimAtTarget
                    )
                {
                    continue;
                }
                // Interaction orders set `target` without being attacks.
                if matches!(
                    attacker.ai_state,
                    AIState::Capturing
                        | AIState::SpecialAbility
                        | AIState::Repairing
                        | AIState::Entering
                        | AIState::Docking
                        | AIState::Constructing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::SeekingRepair
                        | AIState::SeekingHealing
                ) {
                    continue;
                }
                let is_ac = attacker.is_kind_of(KindOf::Aircraft)
                    || attacker.object_type == ObjectType::Aircraft;
                let docked_sortie =
                    attacker.ai_state == AIState::Docked && is_ac && attacker.target.is_some();
                let docked_passenger = attacker.ai_state == AIState::Docked && !docked_sortie;
                let garrisoned = attacker.ai_state == AIState::Garrisoned;
                (docked_sortie, docked_passenger, garrisoned)
            };
            if docked_sortie {
                let _ = self.try_runway_takeoff_from_airfield(attacker_id);
            } else if docked_passenger {
                self.try_transport_passenger_residual_fire(attacker_id);
                continue;
            }
            if garrisoned {
                self.try_garrison_residual_fire(attacker_id);
                continue;
            }
            let Some(attacker) = self.objects.get(&attacker_id) else {
                continue;
            };
            // Base-defense residual: Patriot / Gattling (and FSBaseDefense) auto-acquire
            // and fire at nearby enemies without a manual AttackObject order.
            // Respect skirmish AI pause so golden clear is not structure-counterfired.
            {
                let is_defense = crate::game_logic::host_base_defense::is_base_defense_structure(
                    &attacker.template_name,
                    attacker.is_kind_of(KindOf::Structure),
                    attacker.is_kind_of(KindOf::FSBaseDefense),
                );
                let defense_auto_ok = is_defense
                    && attacker.is_constructed()
                    && attacker.can_attack()
                    && matches!(
                        attacker.ai_state,
                        AIState::Idle | AIState::Attacking | AIState::Patrolling
                    )
                    && !self.skirmish_ai_auto_engage_paused(attacker.team);
                if defense_auto_ok {
                    // Residual owns base-defense fire (nearest-in-range each shot).
                    // Manual AttackObject is not required; structures never chase.
                    self.try_base_defense_residual_fire(attacker_id);
                    continue;
                }
            }
            // Strategy Center Bombardment turret residual: StrategyCenterGun auto-fire
            // only while Bombardment plan is active (C++ enableTurret residual).
            {
                use crate::game_logic::host_strategy_center::{
                    HostBattlePlan, is_strategy_center_template,
                };
                let is_sc = is_strategy_center_template(&attacker.template_name)
                    || attacker.is_kind_of(KindOf::FSStrategyCenter);
                let sc_auto_ok = is_sc
                    && attacker.is_constructed()
                    && attacker.weapon.is_some()
                    && attacker.can_attack()
                    && matches!(
                        attacker.ai_state,
                        AIState::Idle | AIState::Attacking | AIState::Patrolling
                    )
                    && !self.skirmish_ai_auto_engage_paused(attacker.team);
                if sc_auto_ok {
                    // Player residual gate: active plan must be Bombardment.
                    let pid = self.player_id_for_team(attacker.team).unwrap_or(0);
                    if self.battle_plans.active_plan_for_player(pid)
                        == Some(HostBattlePlan::Bombardment)
                    {
                        self.try_strategy_center_bombardment_turret_fire(attacker_id);
                        continue;
                    }
                }
            }
            // Sentry Drone residual: with gun upgrade, AutoAcquireEnemiesWhenIdle
            // fires at nearest enemy without manual AttackObject.
            // Fail-closed: not full DeployStyle pack/unpack / turret-only-deployed.
            {
                use crate::game_logic::host_sentry_drone::{
                    is_sentry_drone_template, sentry_auto_fire_eligible,
                };
                let is_sentry = is_sentry_drone_template(&attacker.template_name);
                let idle_ok = matches!(
                    attacker.ai_state,
                    AIState::Idle | AIState::Attacking | AIState::Patrolling
                );
                let sentry_auto_ok = sentry_auto_fire_eligible(
                    is_sentry,
                    attacker.weapon.is_some(),
                    attacker.is_alive(),
                    attacker.can_attack(),
                    idle_ok,
                ) && !self.skirmish_ai_auto_engage_paused(attacker.team)
                    // Only residual-own auto-fire when no explicit player target.
                    && attacker.target.is_none()
                    && attacker.target_location.is_none();
                if sentry_auto_ok {
                    self.try_sentry_drone_residual_fire(attacker_id);
                    continue;
                }
            }
            // Hellfire Drone residual: AutoAcquireEnemiesWhenIdle fires at nearest enemy.
            // Fail-closed: not full SlavedUpdate wander / master attack bonus matrix.
            {
                use crate::game_logic::host_slave_drones::{
                    hellfire_auto_fire_eligible, is_hellfire_drone_template,
                };
                let is_hf = is_hellfire_drone_template(&attacker.template_name);
                let idle_ok = matches!(
                    attacker.ai_state,
                    AIState::Idle | AIState::Attacking | AIState::Patrolling
                );
                let hf_auto_ok = hellfire_auto_fire_eligible(
                    is_hf,
                    attacker.weapon.is_some(),
                    attacker.is_alive(),
                    attacker.can_attack(),
                    idle_ok,
                ) && !self.skirmish_ai_auto_engage_paused(attacker.team)
                    && attacker.target.is_none()
                    && attacker.target_location.is_none();
                if hf_auto_ok {
                    self.try_hellfire_drone_residual_fire(attacker_id);
                    continue;
                }
            }
            // Portable Overlord/Helix gattling: independent auto-acquire (HelixContain.cpp:340).
            if self
                .objects
                .get(&attacker_id)
                .map(|a| a.has_overlord_gattling_residual())
                .unwrap_or(false)
            {
                self.try_overlord_gattling_addon_independent_fire(attacker_id);
            }
            let Some(attacker) = self.objects.get(&attacker_id) else {
                continue;
            };
            let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

            // TARGET_FAERIE_FIRE residual: painted targets grant 150% ROF readiness.
            let target_has_faerie = attacker
                .target
                .and_then(|tid| self.objects.get(&tid))
                .map(|t| t.is_faerie_fire())
                .unwrap_or(false);

            // Any auto-legal slot ready on reload timer? Button-only
            // AutoChooseSources=NONE secondaries (Jarmen snipe / MD laser)
            // must not keep the cycle alive or the chooser will either
            // auto-fire them or fall through to chase.
            let secondary_explicit = attacker.active_weapon_slot == 1
                || (attacker.weapon_lock_type != WeaponLockType::NotLocked
                    && attacker.weapon_lock_slot == 1);
            let tertiary_explicit = attacker.active_weapon_slot == 2
                || (attacker.weapon_lock_type != WeaponLockType::NotLocked
                    && attacker.weapon_lock_slot == 2);
            let any_ready = attacker.weapon_slot(0).is_some_and(|w| {
                Object::weapon_ready_vs_target(w, current_time, target_has_faerie)
            }) || ((secondary_explicit
                || attacker.thing.template.slot_allows_auto_choose(1))
                && attacker.secondary_weapon.as_ref().is_some_and(|w| {
                    Object::weapon_ready_vs_target(w, current_time, target_has_faerie)
                }))
                || (tertiary_explicit
                    && attacker.tertiary_weapon.as_ref().is_some_and(|w| {
                        Object::weapon_ready_vs_target(w, current_time, target_has_faerie)
                    }));
            if !any_ready {
                continue;
            }

            let attacker_team = attacker.team;
            let target_id = attacker.target;
            let target_location = attacker.target_location;
            let overcharge = attacker.overcharge_enabled;
            drop(attacker);

            let mut fired_slot: Option<u8> = None;

            // Standard object-to-object attack.
            if let Some(target_id) = target_id {
                let target_status = self.objects.get(&target_id).map(|target| {
                    (
                        target.is_alive(),
                        target.apply_sneaky_targeting_offset(target.get_position(), self.frame),
                        target.is_temporarily_preventing_aim_success(self.frame),
                    )
                });

                let Some((target_alive, target_position, lockon_block)) = target_status else {
                    self.stop_attack_decision_aware(attacker_id);
                    continue;
                };

                if !target_alive {
                    if let Some(atk) = self.objects.get_mut(&attacker_id) {
                        atk.notify_jet_victim_is_dead(self.frame);
                    }
                    self.stop_attack_decision_aware(attacker_id);
                    continue;
                }
                if lockon_block {
                    continue;
                }
                if let Some(tgt) = self.objects.get_mut(&target_id) {
                    tgt.add_jet_targeter(attacker_id, true, self.frame);
                }

                // Choose a legal explicit/automatic combat slot, then fire.
                // Stealthed + undetected: drop the engagement (C++ AIStates residual).
                let (selected_slot, enemy_or_forced, target_stealthed_hidden) = {
                    if let (Some(attacker), Some(target)) =
                        (self.objects.get(&attacker_id), self.objects.get(&target_id))
                    {
                        let is_enemy = if self.has_object_ownership_provenance(attacker, target) {
                            self.object_relationship(attacker, target)
                                == gamelogic::common::Relationship::Enemies
                        } else {
                            attacker.team != target.team
                        };
                        let stealthed_hidden = target.is_effectively_stealthed() && is_enemy;
                        // InvulnerableTime residual: enemies treat as ALLIES (skip auto fire).
                        let invuln_hidden = target.is_eject_invulnerable() && is_enemy;
                        let enemy_or_forced = attacker.force_attack || is_enemy;
                        let slot = if enemy_or_forced && !stealthed_hidden && !invuln_hidden {
                            attacker.select_combat_weapon_slot(target, current_time)
                        } else {
                            None
                        };
                        (slot, enemy_or_forced, stealthed_hidden || invuln_hidden)
                    } else {
                        (None, false, false)
                    }
                };

                if target_stealthed_hidden {
                    self.stop_attack_decision_aware(attacker_id);
                    continue;
                }

                // C++ WeaponSet.cpp:782-783 chooseBest lock returns TRUE;
                // FireCurrentWeapon then waits if that slot is not READY or
                // not in range. Do not PreferMostDamage-fall-through to PRIMARY.
                let selected_slot = if let Some(slot) = selected_slot {
                    let (ready, in_range) = if let (Some(attacker), Some(target)) =
                        (self.objects.get(&attacker_id), self.objects.get(&target_id))
                    {
                        let faerie = target.is_faerie_fire();
                        let ready = attacker.weapon_slot(slot).is_some_and(|w| {
                            attacker.weapon_ready_vs_target_bonused(w, current_time, faerie)
                        });
                        let in_range = attacker
                            .weapon_slot(slot)
                            .is_some_and(|w| attacker.can_target_with_slot(target, w, Some(slot)));
                        (ready, in_range)
                    } else {
                        (false, false)
                    };
                    if !ready {
                        continue;
                    }
                    in_range.then_some(slot)
                } else {
                    None
                };

                if let Some(slot) = selected_slot {
                    // C++ DeployStyleAIUpdate::update only enters DEPLOY once
                    // its current victim is within the current weapon's attack
                    // range.  Do this after slot/range selection, rather than
                    // before it, so an out-of-range attack can keep its target
                    // and approach path instead of packing in place.
                    if !self.ensure_deploy_style_ready_to_fire(attacker_id) {
                        continue;
                    }

                    // C++ isAttackViewBlockedByObstacle residual: do not fire through
                    // buildings; chase instead (falls through to OOR chase when we
                    // clear selected fire by treating as out-of-LOS).
                    if self.attack_view_blocked(attacker_id, Some(target_id), target_position) {
                        // Ready but LOS blocked → findAttackPath residual (firing cell).
                        let combat_chase_ok = self
                            .objects
                            .get(&attacker_id)
                            .map(|attacker| {
                                attacker.can_move()
                                    && matches!(
                                        attacker.ai_state,
                                        AIState::Idle
                                            | AIState::Moving
                                            | AIState::Attacking
                                            | AIState::AttackMoving
                                            | AIState::Patrolling
                                            | AIState::AttackingGround
                                    )
                            })
                            .unwrap_or(false);
                        if combat_chase_ok {
                            let _ = self.assign_unit_attack_path(
                                attacker_id,
                                Some(target_id),
                                target_position,
                            );
                        }
                        continue;
                    }

                    // C++ AIStates AcceptableAimDelta residual: do not fire until facing
                    // is within aim delta; turn in place toward the target instead.
                    {
                        let decision_auth =
                            crate::gameworld_shadow::gameworld_ai_decision_authority_live();
                        let aim_ok = if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                            if !decision_auth {
                                if !matches!(
                                    attacker.ai_state,
                                    AIState::Patrolling | AIState::AttackMoving
                                ) {
                                    attacker.set_ai_state(AIState::Attacking);
                                }
                                attacker.set_status_attacking(true);
                                attacker.target = Some(target_id);
                            }
                            // Stationary / can-turn-in-place residual: complete the yaw
                            // this frame (fail-closed vs loco turn-rate matrix). Moving
                            // attackers use a bounded step so chase still turns gradually.
                            let max_step = if attacker.status.moving && attacker.can_move() {
                                0.2
                            } else {
                                std::f32::consts::PI
                            };
                            attacker.turn_toward_position(target_position, slot, max_step)
                        } else {
                            true
                        };
                        if !aim_ok {
                            continue;
                        }
                    }

                    // C++ Weapon MinTargetPitch/MaxTargetPitch residual: reject shots
                    // whose elevation angle is outside the weapon loft window.
                    {
                        let pitch_ok = if let Some(attacker) = self.objects.get(&attacker_id) {
                            let wname = attacker.weapon_name_for_slot(slot);
                            let limits = wname
                                .map(crate::game_logic::weapon_bootstrap::host_target_pitch_limits_for_weapon_name)
                                .unwrap_or_default();
                            let src_half = {
                                let b = &attacker.thing.geometry.bounds_max.y
                                    - attacker.thing.geometry.bounds_min.y;
                                (b * 0.5).max(0.0)
                            };
                            let (tgt_above, tgt_below) = self
                                .objects
                                .get(&target_id)
                                .map(|t| {
                                    let h = (t.thing.geometry.bounds_max.y
                                        - t.thing.geometry.bounds_min.y)
                                        .max(0.0);
                                    // Position is typically feet; above ≈ full height, below ≈ 0.
                                    (h, 0.0_f32)
                                })
                                .unwrap_or((0.0, 0.0));
                            crate::game_logic::weapon_bootstrap::is_pitch_within_limits_geom(
                                attacker.get_position(),
                                target_position,
                                &limits,
                                src_half,
                                tgt_above,
                                tgt_below,
                            )
                        } else {
                            true
                        };
                        if !pitch_ok {
                            // Out of pitch: keep engagement but do not fire this frame
                            // (C++ AI continues aiming / repositioning).
                            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                attacker.set_ai_state(AIState::Attacking);
                                attacker.set_status_attacking(true);
                                attacker.set_target(Some(target_id));
                            }
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_attack(
                                    attacker_id,
                                    target_id,
                                );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    attacker_id,
                                    2,
                                );
                            }
                            continue;
                        }
                    }

                    // C++ PreAttackType residual: wind-up before first discharge of
                    // shot / attack / clip. Shared Object helpers match fire_at.
                    {
                        let pre_blocked = if let Some(attacker) = self.objects.get_mut(&attacker_id)
                        {
                            // Mirror Object::fire_at pre-attack gate without spawning.
                            let pre_delay = attacker
                                .weapon_slot(slot)
                                .map(|w| w.pre_attack_delay.max(0.0))
                                .unwrap_or(0.0);
                            let prefire = {
                                attacker.weapon_name_for_slot(slot).map(
                                    crate::game_logic::weapon_bootstrap::host_prefire_type_for_weapon_name,
                                )
                                .unwrap_or(
                                    crate::game_logic::weapon_bootstrap::HostPrefireType::PerShot,
                                )
                            };
                            let apply = attacker
                                .pre_attack_delay_applies(slot, target_id, prefire, pre_delay);
                            if apply {
                                let needs_arm = attacker.pre_attack_target != Some(target_id)
                                    || attacker.pre_attack_ready_at <= 0.0;
                                if needs_arm {
                                    attacker.pre_attack_target = Some(target_id);
                                    attacker.pre_attack_ready_at = current_time + pre_delay;
                                    attacker.activate_leech_range_for_slot(slot);
                                }
                                if current_time + 1e-6 < attacker.pre_attack_ready_at {
                                    attacker.set_target(Some(target_id));
                                    attacker.set_ai_state(AIState::Attacking);
                                    attacker.set_status_attacking(true);
                                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live(
                                    ) {
                                        crate::game_logic::host_ai_decision_log::record_attack(
                                            attacker_id,
                                            target_id,
                                        );
                                        crate::game_logic::host_ai_decision_log::record_set_state(
                                            attacker_id,
                                            2,
                                        );
                                    }
                                    true
                                } else {
                                    false
                                }
                            } else {
                                attacker.pre_attack_target = Some(target_id);
                                false
                            }
                        } else {
                            false
                        };
                        if pre_blocked {
                            continue;
                        }
                    }

                    // GLA car-bomb residual: firing the SuicideCarBomb weapon detonates
                    // at self (DamageDealtAtSelfPosition) and destroys the car bomb.
                    let is_carbomb = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.status.is_carbomb)
                        .unwrap_or(false);
                    if is_carbomb {
                        let _ = self.detonate_car_bomb(attacker_id);
                        continue;
                    }

                    let mut weapon_damage = self
                        .objects
                        .get(&attacker_id)
                        .map(|attacker| {
                            let name = attacker.weapon_name_for_slot(slot);
                            let base = name
                                .and_then(
                                    crate::game_logic::weapon_bootstrap::host_primary_damage_for_weapon_name,
                                )
                                .or_else(|| attacker.weapon_slot(slot).map(|w| w.damage))
                                .unwrap_or(0.0);
                            attacker.effective_weapon_damage(base)
                        })
                        .unwrap_or(0.0);
                    if overcharge {
                        weapon_damage *= 1.1;
                    }

                    fired_slot = Some(slot);

                    // Aurora dive bomb residual: queue delayed area damage at target.
                    // AuroraBomb projectile flight residual closed; FuelAir gas OCL path.
                    // Instant single-target take_damage is skipped; AOE applies after delay.
                    // Keep fired_slot so last_fire_time / particles / audio still run.
                    let aurora_queued = {
                        use crate::game_logic::host_aurora_bomb::{
                            aurora_bomb_kind_for_template, is_aurora_aircraft_template,
                        };
                        let aurora = self.objects.get(&attacker_id).and_then(|a| {
                            if is_aurora_aircraft_template(&a.template_name) {
                                Some(aurora_bomb_kind_for_template(&a.template_name))
                            } else {
                                None
                            }
                        });
                        if let Some(kind) = aurora {
                            let impact = target_position;
                            let _ =
                                self.queue_aurora_bomb(kind, attacker_id, attacker_team, impact);
                            true
                        } else {
                            false
                        }
                    };

                    if aurora_queued {
                        // Shot consumed; delayed dive residual pending (no instant HP damage).
                    } else {
                        // Avenger Target Designator residual: paint FAERIE_FIRE (no HP damage).
                        let avenger_paint = {
                            use crate::game_logic::host_avenger::{
                                AVENGER_FAERIE_FIRE_DURATION_FRAMES, AVENGER_PAINT_AUDIO,
                                is_avenger_template, should_apply_faerie_fire_paint,
                            };
                            self.objects
                                .get(&attacker_id)
                                .map(|a| {
                                    should_apply_faerie_fire_paint(
                                        is_avenger_template(&a.template_name),
                                        slot,
                                        true,
                                        enemy_or_forced,
                                    )
                                })
                                .unwrap_or(false)
                        };
                        let avenger_air = {
                            use crate::game_logic::host_avenger::{
                                is_avenger_template, should_apply_avenger_air_laser,
                            };
                            let target_is_air = self
                                .objects
                                .get(&target_id)
                                .map(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
                                .unwrap_or(false);
                            self.objects
                                .get(&attacker_id)
                                .map(|a| {
                                    should_apply_avenger_air_laser(
                                        is_avenger_template(&a.template_name),
                                        slot,
                                        target_is_air,
                                        true,
                                        enemy_or_forced,
                                    )
                                })
                                .unwrap_or(false)
                        };
                        // Humvee air TOW residual damage boost vs aircraft.
                        let humvee_air_tow = {
                            use crate::game_logic::host_humvee::{
                                HUMVEE_AIR_TOW_DAMAGE, humvee_prefer_air_tow, is_humvee_template,
                            };
                            let target_is_air = self
                                .objects
                                .get(&target_id)
                                .map(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
                                .unwrap_or(false);
                            self.objects
                                .get(&attacker_id)
                                .map(|a| {
                                    humvee_prefer_air_tow(
                                        is_humvee_template(&a.template_name),
                                        a.has_upgrade_tag(
                                            crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW,
                                        ) || a.has_upgrade_tag("Upgrade_AmericaTOWMissile"),
                                        target_is_air,
                                    ) && slot == 1
                                })
                                .unwrap_or(false)
                        };
                        if humvee_air_tow {
                            weapon_damage = crate::game_logic::host_humvee::HUMVEE_AIR_TOW_DAMAGE;
                        }

                        // TARGET_FAERIE_FIRE ROF honesty when shooting a painted target.
                        if self
                            .objects
                            .get(&target_id)
                            .map(|t| t.is_faerie_fire())
                            .unwrap_or(false)
                        {
                            self.avenger.record_rof_grant();
                        }

                        if avenger_paint {
                            use crate::game_logic::host_avenger::{
                                AVENGER_FAERIE_FIRE_DURATION_FRAMES, AVENGER_PAINT_AUDIO,
                            };
                            let until = self
                                .frame
                                .saturating_add(AVENGER_FAERIE_FIRE_DURATION_FRAMES);
                            if let Some(target) = self.objects.get_mut(&target_id) {
                                target.apply_faerie_fire(until);
                            }
                            self.avenger.record_paint();
                            let muzzle = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| a.get_position())
                                .unwrap_or(target_position);
                            self.queue_audio_event(
                                AudioEventRequest::new(AVENGER_PAINT_AUDIO)
                                    .with_object(attacker_id)
                                    .with_position(muzzle)
                                    .with_priority(140),
                            );
                            // Status residual: no hitpoint damage from designator.
                        } else if avenger_air {
                            self.avenger.record_air_laser_fire();
                            if let Some(target) = self.objects.get_mut(&target_id) {
                                let destroyed =
                                    target.take_damage_from(weapon_damage, Some(attacker_id));
                                if destroyed {
                                    let victim_pos = target.get_position();
                                    let victim_team = target.team;
                                    self.mark_object_for_destruction(
                                        target_id,
                                        Some(attacker_team),
                                    );
                                    let wname = self.objects.get(&attacker_id).and_then(|a| {
                                        a.thing.template.primary_weapon_name.clone().or_else(|| {
                                            a.thing.template.secondary_weapon_name.clone()
                                        })
                                    });
                                    self.continue_or_stop_after_kill(
                                        attacker_id,
                                        target_id,
                                        victim_pos,
                                        victim_team,
                                        wname.as_deref(),
                                        20.0,
                                    );
                                }
                            }
                        } else {
                            // Nuke Cannon primary residual: area shell + medium radiation field.
                            let nuke_primary = {
                                use crate::game_logic::host_nuke_cannon::{
                                    is_nuke_cannon_template, should_apply_nuke_cannon_primary,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_nuke_cannon_primary(
                                            is_nuke_cannon_template(&a.template_name),
                                            slot,
                                        )
                                    })
                                    .unwrap_or(false)
                            };

                            // Neutron shell residual: Nuke Cannon secondary applies blast
                            // (kill infantry / unman vehicles) instead of HP take_damage.
                            let neutron_blast = {
                                use crate::game_logic::host_neutron_shell::{
                                    UPGRADE_CHINA_NEUTRON_SHELLS, is_nuke_cannon_template,
                                    should_apply_neutron_blast,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_neutron_blast(
                                            a.has_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS)
                                                || a.has_upgrade_tag("Upgrade_ChinaNeutronShells"),
                                            slot,
                                            is_nuke_cannon_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            };

                            if nuke_primary {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_nuke_cannon_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    // Honesty fire count is recorded on impact via apply;
                                    // count residual fire at spawn for combat-gate honesty.
                                    (1, false)
                                } else {
                                    self.apply_nuke_cannon_primary_at(
                                        impact,
                                        Some(attacker_id),
                                        attacker_team,
                                    )
                                };
                            } else if neutron_blast {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_neutron_cannon_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (ik, vu, _vk) = if spawned {
                                    // Blast deferred to shell DetonateCallsKill residual.
                                    self.neutron_shell_residual_blasts =
                                        self.neutron_shell_residual_blasts.saturating_add(1);
                                    (0, 0, 0)
                                } else {
                                    self.apply_neutron_blast_at(
                                        impact,
                                        attacker_team,
                                        Some(attacker_id),
                                        true,
                                    )
                                };
                                // Stop attack after residual blast shot (slow reload residual).
                            } else if {
                                // Helix residual: PRIMARY HelixMinigunWeapon intended-only.
                                // When portable gattling addon is installed, the Overlord/Helix
                                // gattling residual path already applies primary + passenger.
                                use crate::game_logic::host_helix_minigun::should_apply_helix_minigun_residual;
                                use crate::game_logic::host_overlord_addons::is_helix_template;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_helix_minigun_residual(
                                            is_helix_template(&a.template_name),
                                            slot,
                                        ) && !a.has_overlord_gattling_residual()
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self.apply_helix_minigun_residual_at(
                                    impact,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                            } else if {
                                // Comanche residual: 20mm primary / anti-tank secondary /
                                // manual rocket pods tertiary.
                                use crate::game_logic::host_comanche_rocket_pods::{
                                    UPGRADE_COMANCHE_ROCKET_PODS, is_comanche_template,
                                    should_apply_comanche_antitank_residual,
                                    should_apply_comanche_cannon_residual,
                                    should_apply_rocket_pod_area_attack,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| is_comanche_template(&a.template_name))
                                    .unwrap_or(false)
                            } {
                                use crate::game_logic::host_comanche_rocket_pods::{
                                    UPGRADE_COMANCHE_ROCKET_PODS, is_comanche_template,
                                    should_apply_comanche_antitank_residual,
                                    should_apply_comanche_cannon_residual,
                                    should_apply_rocket_pod_area_attack,
                                };
                                let impact = target_position;
                                let (has_pods, is_comanche) = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        (
                                            a.has_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS)
                                                || a.has_upgrade_tag("Upgrade_ComancheRocketPods"),
                                            is_comanche_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or((false, false));
                                let (hits, _destroyed_any) = if should_apply_rocket_pod_area_attack(
                                    is_comanche,
                                    has_pods,
                                    slot,
                                ) {
                                    {
                                        use crate::game_logic::host_comanche_rocket_pods::{
                                            ROCKET_POD_CLIP_SIZE, rocket_pod_scatter_impact,
                                        };
                                        let idx = {
                                            let shot = self
                                                .comanche_rocket_pod_shot_index
                                                .entry(attacker_id)
                                                .or_insert(0);
                                            let i = *shot;
                                            *shot = shot.saturating_add(1)
                                                % ROCKET_POD_CLIP_SIZE.max(1);
                                            i
                                        };
                                        let (sx, sy, sz) = rocket_pod_scatter_impact(
                                            impact.x, impact.y, impact.z, idx,
                                        );
                                        let aim = Vec3::new(sx, sy, sz);
                                        let from = self
                                            .objects
                                            .get(&attacker_id)
                                            .map(|o| o.get_position())
                                            .unwrap_or(impact);
                                        let _ = self.spawn_comanche_rocket_pod_projectile(
                                            attacker_id,
                                            from,
                                            aim,
                                            idx,
                                        );
                                        // Area residual still centers on intended aim
                                        // (ScatterTarget is projectile flight residual).
                                        self.apply_comanche_rocket_pod_area_at(
                                            impact,
                                            Some(attacker_id),
                                        )
                                    }
                                } else if should_apply_comanche_antitank_residual(
                                    is_comanche,
                                    slot,
                                    has_pods,
                                ) {
                                    self.apply_comanche_antitank_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                } else if should_apply_comanche_cannon_residual(is_comanche, slot) {
                                    self.apply_comanche_cannon_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                } else {
                                    (0, false)
                                };
                            } else if {
                                // GLA Rocket Buggy residual: long-range rocket + splash / scatter.
                                use crate::game_logic::host_rocket_buggy::{
                                    is_rocket_buggy_template, should_apply_rocket_buggy_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_rocket_buggy_residual(
                                            is_rocket_buggy_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_rocket_buggy_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.rocket_buggy_residual_fires =
                                        self.rocket_buggy_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_rocket_buggy_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // GLA SCUD launcher residual: area blast (+ toxin field on secondary).
                                use crate::game_logic::host_scud_launcher::{
                                    is_scud_launcher_template, should_apply_scud_area,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_scud_area(is_scud_launcher_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let toxin = {
                                    use crate::game_logic::host_scud_launcher::scud_toxin_warhead_for_slot;
                                    self.objects
                                        .get(&attacker_id)
                                        .map(|a| {
                                            scud_toxin_warhead_for_slot(&a.template_name, slot)
                                        })
                                        .unwrap_or(slot == 1)
                                };
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_scud_launcher_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        toxin,
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    (1, false) // fire residual honesty; blast deferred to impact
                                } else {
                                    self.apply_scud_area_at(
                                        impact,
                                        Some(attacker_id),
                                        attacker_team,
                                        toxin,
                                    )
                                };
                            } else if {
                                // GLA Technical residual: MG direct or cannon/RPG splash salvage tiers.
                                use crate::game_logic::host_technical::{
                                    TechnicalWeaponTier, is_technical_template,
                                    should_apply_technical_splash,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        if !is_technical_template(&a.template_name) {
                                            return false;
                                        }
                                        let tier = Self::technical_tier_from_object(a);
                                        // Always apply residual path for technical (MG direct or splash).
                                        let _ = should_apply_technical_splash(true, tier);
                                        true
                                    })
                                    .unwrap_or(false)
                            } {
                                use crate::game_logic::host_technical::{
                                    TechnicalWeaponTier as TechTier,
                                    should_apply_technical_cannon_shell,
                                    should_apply_technical_rpg_missile,
                                };
                                let impact = target_position;
                                let tier = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| Self::technical_tier_from_object(a))
                                    .unwrap_or(TechTier::Base);
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let (hits, _destroyed_any) =
                                    if should_apply_technical_rpg_missile(true, tier) {
                                        let spawned = self
                                            .spawn_technical_rpg_missile_projectile(
                                                attacker_id,
                                                from,
                                                impact,
                                                Some(target_id),
                                            )
                                            .is_some();
                                        if spawned {
                                            self.technical_residual_fires =
                                                self.technical_residual_fires.saturating_add(1);
                                            (1, false)
                                        } else {
                                            self.apply_technical_residual_at(
                                                impact,
                                                Some(attacker_id),
                                                Some(target_id),
                                            )
                                        }
                                    } else if should_apply_technical_cannon_shell(true, tier) {
                                        let spawned = self
                                            .spawn_technical_cannon_shell_projectile(
                                                attacker_id,
                                                from,
                                                impact,
                                                Some(target_id),
                                            )
                                            .is_some();
                                        if spawned {
                                            self.technical_residual_fires =
                                                self.technical_residual_fires.saturating_add(1);
                                            (1, false)
                                        } else {
                                            self.apply_technical_residual_at(
                                                impact,
                                                Some(attacker_id),
                                                Some(target_id),
                                            )
                                        }
                                    } else {
                                        self.apply_technical_residual_at(
                                            impact,
                                            Some(attacker_id),
                                            Some(target_id),
                                        )
                                    };
                            } else if {
                                // GLA Marauder residual: salvage fire-rate tiers + small splash.
                                use crate::game_logic::host_marauder::{
                                    is_marauder_template, should_apply_marauder_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_marauder_residual(is_marauder_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                use crate::game_logic::host_marauder::{
                                    MARAUDER_SPEED_TIER0, MARAUDER_SPEED_TIER1,
                                    MARAUDER_SPEED_TIER2, MarauderWeaponTier,
                                };
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let speed = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| match Self::marauder_tier_from_object(a) {
                                        MarauderWeaponTier::Two => MARAUDER_SPEED_TIER2,
                                        MarauderWeaponTier::One => MARAUDER_SPEED_TIER1,
                                        MarauderWeaponTier::Base => a
                                            .weapon
                                            .as_ref()
                                            .map(|w| w.projectile_speed)
                                            .filter(|s| *s > 1.0)
                                            .unwrap_or(MARAUDER_SPEED_TIER0),
                                    })
                                    .unwrap_or(MARAUDER_SPEED_TIER0);
                                let spawned = self
                                    .spawn_marauder_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        speed,
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.marauder_residual_fires =
                                        self.marauder_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_marauder_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // GLA Scorpion residual: gun splash or rocket dual-radius secondary.
                                use crate::game_logic::host_scorpion::{
                                    is_scorpion_template, should_apply_scorpion_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_scorpion_residual(is_scorpion_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = if slot == 0 {
                                    self.spawn_scorpion_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        slot,
                                    )
                                    .is_some()
                                } else {
                                    self.spawn_scorpion_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        slot,
                                    )
                                    .is_some()
                                };
                                let (hits, _destroyed_any) = if spawned {
                                    if slot == 0 {
                                        self.scorpion_residual_fires =
                                            self.scorpion_residual_fires.saturating_add(1);
                                    } else {
                                        self.scorpion_residual_missile_fires =
                                            self.scorpion_residual_missile_fires.saturating_add(1);
                                    }
                                    (1, false)
                                } else {
                                    self.apply_scorpion_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                        slot,
                                    )
                                };
                            } else if {
                                // USA Tomahawk residual: dual-radius long-range missile.
                                use crate::game_logic::host_tomahawk::{
                                    is_tomahawk_template, should_apply_tomahawk_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_tomahawk_residual(is_tomahawk_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_tomahawk_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.tomahawk_residual_fires =
                                        self.tomahawk_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_tomahawk_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // USA Raptor residual: jet missiles + Laser Missiles splash.
                                use crate::game_logic::host_raptor::{
                                    is_raptor_template, should_apply_raptor_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_raptor_residual(is_raptor_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_raptor_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.raptor_residual_fires =
                                        self.raptor_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_raptor_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // China MiG residual: dual-radius napalm / Nuke missiles + field residual.
                                use crate::game_logic::host_mig::{
                                    is_mig_template, should_apply_mig_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_mig_residual(is_mig_template(&a.template_name))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_mig_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.mig_residual_fires =
                                        self.mig_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_mig_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // America Fire Base residual: howitzer primary-radius splash.
                                use crate::game_logic::host_fire_base::{
                                    is_fire_base_template, should_apply_fire_base_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_fire_base_residual(is_fire_base_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_fire_base_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.fire_base_residual_fires =
                                        self.fire_base_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_fire_base_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // USA Stealth Fighter residual: jet missiles splash + bunker-buster structure path.
                                use crate::game_logic::host_stealth_fighter::{
                                    is_stealth_fighter_template,
                                    should_apply_stealth_fighter_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_stealth_fighter_residual(
                                            is_stealth_fighter_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_stealth_jet_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.stealth_fighter_residual_fires =
                                        self.stealth_fighter_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_stealth_fighter_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // USA Battle Drone residual: intended-only MG fire.
                                use crate::game_logic::host_slave_drones::is_battle_drone_template;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| is_battle_drone_template(&a.template_name))
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self.apply_battle_drone_residual_at(
                                    impact,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                            } else if {
                                // China Overlord / Emperor residual: dual-radius main gun (no gattling addon).
                                use crate::game_logic::host_overlord_gun::{
                                    is_overlord_gun_chassis, should_apply_overlord_gun_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_overlord_gun_residual(
                                            is_overlord_gun_chassis(&a.template_name),
                                            a.has_overlord_gattling_residual(),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_overlord_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    (1, false)
                                } else {
                                    self.apply_overlord_gun_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // GLA Jarmen Kell residual: primary sniper (intended-only).
                                use crate::game_logic::host_jarmen_kell::{
                                    is_jarmen_kell_template, should_apply_jarmen_kell_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_jarmen_kell_residual(is_jarmen_kell_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self.apply_jarmen_kell_residual_at(
                                    impact,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                            } else if {
                                // USA Crusader/Paladin residual: GenericTankShell Bezier + splash.
                                use crate::game_logic::host_usa_tanks::{
                                    CRUSADER_WEAPON_SPEED, PALADIN_WEAPON_SPEED,
                                    is_paladin_template, should_apply_usa_tank_gun_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| should_apply_usa_tank_gun_residual(&a.template_name))
                                    .unwrap_or(false)
                            } {
                                use crate::game_logic::host_usa_tanks::{
                                    CRUSADER_WEAPON_SPEED, PALADIN_WEAPON_SPEED,
                                    is_paladin_template,
                                };
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let (speed, is_pal) = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        let pal = is_paladin_template(&a.template_name);
                                        let spd = a
                                            .weapon
                                            .as_ref()
                                            .map(|w| w.projectile_speed)
                                            .filter(|s| *s > 1.0)
                                            .unwrap_or(if pal {
                                                PALADIN_WEAPON_SPEED
                                            } else {
                                                CRUSADER_WEAPON_SPEED
                                            });
                                        (spd, pal)
                                    })
                                    .unwrap_or((CRUSADER_WEAPON_SPEED, false));
                                let _ = is_pal;
                                let spawned = self
                                    .spawn_usa_tank_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        speed,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    (1, false)
                                } else {
                                    self.apply_usa_tank_gun_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        if let Some(w) = attacker.weapon.as_mut() {
                                            w.last_fire_time =
                                                self.frame as f32 * LOGIC_FRAME_TIMESTEP;
                                        }
                                    }
                                }
                                let _ = hits;
                            } else if {
                                // China Battlemaster residual: tank gun splash + Uranium damage residual.
                                use crate::game_logic::host_battlemaster::{
                                    is_battlemaster_template, should_apply_battlemaster_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_battlemaster_residual(
                                            is_battlemaster_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_battlemaster_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    (1, false)
                                } else {
                                    self.apply_battlemaster_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // China Tank Hunter residual: RPG splash + AA capable residual.
                                use crate::game_logic::host_tank_hunter::{
                                    is_tank_hunter_template, should_apply_tank_hunter_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_tank_hunter_residual(is_tank_hunter_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_tank_hunter_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.tank_hunter_residual_fires =
                                        self.tank_hunter_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_tank_hunter_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // China Red Guard residual: bayonet one-shot vs close infantry, else gun.
                                use crate::game_logic::host_red_guard::{
                                    is_red_guard_template, should_apply_red_guard_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_red_guard_residual(is_red_guard_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let (hits, _destroyed_any) = self.apply_red_guard_residual_at(
                                    target_position,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                            } else if {
                                // GLA RPG Trooper residual: rocket splash + AA capable residual.
                                use crate::game_logic::host_rpg_trooper::{
                                    is_rpg_trooper_template, should_apply_rpg_trooper_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_rpg_trooper_residual(is_rpg_trooper_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_rpg_trooper_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.rpg_trooper_residual_fires =
                                        self.rpg_trooper_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_rpg_trooper_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // GLA Terrorist residual: SuicideDynamitePack self-detonation.
                                use crate::game_logic::host_terrorist::{
                                    is_terrorist_template, should_apply_terrorist_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_terrorist_residual(is_terrorist_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let (hits, _destroyed_any) = self.apply_terrorist_residual_at(
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                            } else if {
                                // USA Missile Defender residual: missile splash + laser guided secondary.
                                use crate::game_logic::host_missile_defender::{
                                    is_missile_defender_template,
                                    should_apply_missile_defender_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_missile_defender_residual(
                                            is_missile_defender_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let laser_slot = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.active_weapon_slot == 1)
                                    .unwrap_or(false);
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_missile_defender_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        laser_slot,
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    if laser_slot {
                                        self.missile_defender_residual_laser_fires = self
                                            .missile_defender_residual_laser_fires
                                            .saturating_add(1);
                                    } else {
                                        self.missile_defender_residual_fires =
                                            self.missile_defender_residual_fires.saturating_add(1);
                                    }
                                    (1, false)
                                } else {
                                    self.apply_missile_defender_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                        laser_slot,
                                    )
                                };
                            } else if {
                                // GLA Rebel residual: machine gun intended-only residual.
                                use crate::game_logic::host_gla_rebel::{
                                    is_gla_rebel_template, should_apply_rebel_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_rebel_residual(is_gla_rebel_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let (hits, _destroyed_any) = self.apply_rebel_residual_at(
                                    target_position,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                            } else if {
                                // USA Ranger residual: rifle intended-only or FlashBang dual-radius splash.
                                use crate::game_logic::host_ranger::{
                                    is_ranger_template, should_apply_ranger_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_ranger_residual(is_ranger_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let flash_slot = slot == 1;
                                let impact = target_position;
                                let (hits, _destroyed_any) = if flash_slot {
                                    let from = self
                                        .objects
                                        .get(&attacker_id)
                                        .map(|a| a.get_position())
                                        .unwrap_or(impact);
                                    let spawned = self
                                        .spawn_flashbang_grenade_projectile(
                                            attacker_id,
                                            from,
                                            impact,
                                            Some(target_id),
                                        )
                                        .is_some();
                                    if spawned {
                                        self.ranger_residual_flashbang_fires =
                                            self.ranger_residual_flashbang_fires.saturating_add(1);
                                        (1, false)
                                    } else {
                                        self.apply_ranger_residual_at(
                                            impact,
                                            Some(attacker_id),
                                            Some(target_id),
                                            true,
                                        )
                                    }
                                } else {
                                    self.apply_ranger_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                        false,
                                    )
                                };
                            } else if {
                                // USA Humvee TOW residual: HumveeMissile / PatriotMissile flight + splash.
                                use crate::game_logic::host_humvee::{
                                    is_humvee_template, should_apply_humvee_tow_residual,
                                };
                                use crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW;
                                let (is_hv, has_tow) = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        (
                                            is_humvee_template(&a.template_name),
                                            a.has_upgrade_tag(UPGRADE_AMERICA_TOW)
                                                || a.has_upgrade_tag("Upgrade_AmericaTOWMissile"),
                                        )
                                    })
                                    .unwrap_or((false, false));
                                should_apply_humvee_tow_residual(is_hv, has_tow, slot == 1)
                            } {
                                use crate::game_logic::host_humvee::{
                                    HUMVEE_TOW_FIRE_AUDIO as HV_TOW_AUDIO,
                                    humvee_prefer_air_tow as hv_air_tow,
                                };
                                let impact = target_position;
                                let target_is_air = self
                                    .objects
                                    .get(&target_id)
                                    .map(|t| {
                                        t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target
                                    })
                                    .unwrap_or(false);
                                let air = hv_air_tow(true, true, target_is_air);
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_humvee_tow_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        air,
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.humvee_tow_residual_fires =
                                        self.humvee_tow_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.humvee_tow_residual_fires =
                                        self.humvee_tow_residual_fires.saturating_add(1);
                                    self.apply_humvee_tow_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                        air,
                                    )
                                };
                                let _ = HV_TOW_AUDIO;
                            } else if {
                                // China MiniGunner residual: ground gun or AA secondary hit.
                                use crate::game_logic::host_minigunner::{
                                    is_minigunner_template, should_apply_minigunner_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_minigunner_residual(is_minigunner_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let (hits, _destroyed_any) = self.apply_minigunner_residual_at(
                                    target_position,
                                    Some(attacker_id),
                                    Some(target_id),
                                    slot,
                                );
                            } else if {
                                // Colonel Burton residual: knife one-shot vs close infantry, else sniper.
                                use crate::game_logic::host_colonel_burton::{
                                    is_colonel_burton_template, should_apply_burton_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_burton_residual(is_colonel_burton_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let (hits, _destroyed_any) = self.apply_burton_residual_at(
                                    target_position,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                            } else if {
                                // China Troop Crawler residual: TroopCrawlerAssault DEPLOY → unload + attack.
                                use crate::game_logic::host_troop_crawler::{
                                    is_troop_crawler_template,
                                    should_apply_troop_crawler_assault_deploy,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_troop_crawler_assault_deploy(
                                            is_troop_crawler_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let _ordered =
                                    self.apply_troop_crawler_assault_deploy(attacker_id, target_id);
                                // DEPLOY residual deals no meaningful HP damage (PrimaryDamage ~0).
                            } else if {
                                // China Dragon Tank residual: DragonTankFlameProjectile flight + dual-radius splash.
                                use crate::game_logic::host_dragon_tank::{
                                    is_dragon_tank_template, should_apply_dragon_flame_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_dragon_flame_residual(is_dragon_tank_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_dragon_flame_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.dragon_tank_residual_fires =
                                        self.dragon_tank_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.dragon_tank_residual_fires =
                                        self.dragon_tank_residual_fires.saturating_add(1);
                                    self.apply_dragon_flame_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                            } else if {
                                // China Gattling Tank residual: ground gun or AA secondary hit.
                                use crate::game_logic::host_gattling_tank::is_gattling_tank_template;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| is_gattling_tank_template(&a.template_name))
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self.apply_gattling_tank_residual_at(
                                    impact,
                                    Some(attacker_id),
                                    Some(target_id),
                                    slot,
                                );
                            } else if {
                                // Portable gattling is independent (try_overlord_gattling_addon_independent_fire).
                                // Do not piggyback stacked +10 onto the host chassis shot.
                                false
                            } {
                                let _ = slot;
                            } else if {
                                // GLA Combat Cycle residual: rider weapon fire / suicide residual.
                                use crate::game_logic::host_combat_cycle::{
                                    is_combat_cycle_template, should_apply_combat_cycle_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_combat_cycle_residual(
                                            is_combat_cycle_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self.apply_combat_cycle_residual_at(
                                    impact,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                            } else if {
                                // GLA Toxin Tractor residual: poison stream primary or contaminate spray.
                                use crate::game_logic::host_toxin_tractor::is_toxin_tractor_template;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| is_toxin_tractor_template(&a.template_name))
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let spray = slot == 1;
                                let (hits, _destroyed_any) = if spray {
                                    self.apply_toxin_tractor_spray_at(
                                        impact,
                                        Some(attacker_id),
                                        attacker_team,
                                    )
                                } else {
                                    let from = self
                                        .objects
                                        .get(&attacker_id)
                                        .map(|a| a.get_position())
                                        .unwrap_or(impact);
                                    let spawned = self
                                        .spawn_toxin_stream_projectile(
                                            attacker_id,
                                            from,
                                            impact,
                                            Some(target_id),
                                        )
                                        .is_some();
                                    if spawned {
                                        (1, false)
                                    } else {
                                        self.apply_toxin_tractor_stream_at(
                                            impact,
                                            Some(attacker_id),
                                            Some(target_id),
                                            attacker_team,
                                        )
                                    }
                                };
                            } else {
                                // Bunker Buster residual: kill garrisoned occupants + amplify bunker damage.
                                // KILL_GARRISONED residual: leftover store DamageType only
                                // (C++ ActiveBody.cpp:421-460). AllowAttackGarrisonedBldgs is
                                // estimate-only and must not skip structure HP.
                                let (bunker_buster_hit, kill_garrisoned_hit) = {
                                    use crate::game_logic::host_bunker_buster::{
                                        UPGRADE_AMERICA_BUNKER_BUSTERS, is_bunker_buster_carrier,
                                        should_apply_bunker_buster, should_apply_kill_garrisoned,
                                    };
                                    let target_is_structure = self
                                        .objects
                                        .get(&target_id)
                                        .map(|t| t.is_kind_of(KindOf::Structure))
                                        .unwrap_or(false);
                                    self.objects
                                        .get(&attacker_id)
                                        .map(|a| {
                                            let has_upgrade = a
                                                .has_upgrade_tag(UPGRADE_AMERICA_BUNKER_BUSTERS)
                                                || a.has_upgrade_tag(
                                                    "Upgrade_AmericaBunkerBusters",
                                                );
                                            let carrier =
                                                is_bunker_buster_carrier(&a.template_name);
                                            let kill_garrisoned = a
                                                .weapon_name_for_slot(slot)
                                                .map(crate::game_logic::weapon_bootstrap::host_weapon_is_kill_garrisoned_damage)
                                                .unwrap_or(false);
                                            (
                                                should_apply_bunker_buster(
                                                    has_upgrade,
                                                    carrier,
                                                    target_is_structure,
                                                ),
                                                should_apply_kill_garrisoned(
                                                    kill_garrisoned,
                                                    target_is_structure,
                                                ),
                                            )
                                        })
                                        .unwrap_or((false, false))
                                };

                                if bunker_buster_hit {
                                    let (_kills, _structure_dmg, destroyed) = self
                                        .apply_bunker_buster_to_target(
                                            target_id,
                                            attacker_team,
                                            weapon_damage,
                                            Some(attacker_id),
                                        );
                                    if destroyed {
                                        self.stop_attack_decision_aware(attacker_id);
                                    }
                                } else if kill_garrisoned_hit {
                                    let _kills = self.apply_kill_garrisoned_to_target(
                                        target_id,
                                        attacker_team,
                                        weapon_damage,
                                        Some(attacker_id),
                                    );
                                } else if {
                                    let table_offset =
                                        self.objects.get_mut(&attacker_id).and_then(|attacker| {
                                            let name = attacker
                                                .weapon_name_for_slot(slot)
                                                .map(str::to_owned);
                                            attacker
                                                .take_scatter_table_offset(slot, name.as_deref())
                                        });
                                    let (sc_miss, sc_impact, sc_splash) = self
                                        .resolve_instant_scatter_shot(
                                            attacker_id,
                                            target_id,
                                            slot,
                                            target_position,
                                            table_offset,
                                        );
                                    if sc_miss {
                                        // C++ ScatterRadius residual: miss intended; splash at offset.
                                        if sc_splash > 0.0 {
                                            let wname_splash = self
                                                .objects
                                                .get(&attacker_id)
                                                .and_then(|attacker| {
                                                    attacker
                                                        .weapon_name_for_slot(slot)
                                                        .map(str::to_owned)
                                                });
                                            let hits = self.apply_scatter_miss_splash_at(
                                                sc_impact,
                                                weapon_damage,
                                                sc_splash,
                                                attacker_id,
                                                attacker_team,
                                                target_id,
                                                wname_splash.as_deref(),
                                            );
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                } {
                                    // Miss path handled above (splash optional).
                                } else {
                                    // C++ Weapon.cpp:1378-1380 dealDamage copies
                                    // WeaponTemplate DamageType/DeathType onto DamageInfo.
                                    // take_damage_from is UNRESISTABLE (script kill /
                                    // empty-hulk); live object-vs-object fire must use
                                    // the firing Weapon.ini type so Armor.ini applies.
                                    let fire_wname =
                                        self.objects.get(&attacker_id).and_then(|attacker| {
                                            attacker.weapon_name_for_slot(slot).map(str::to_owned)
                                        });
                                    let damage_type = fire_wname.as_deref().map(
                                        crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name,
                                    )
                                    // C++ WeaponTemplate ctor defaults m_damageType
                                    // to DAMAGE_EXPLOSION (Weapon.cpp:249); an
                                    // unnamed host weapon fires Explosion, not Bullet.
                                    .unwrap_or(crate::game_logic::combat::DamageType::Explosive);
                                    let death_type = crate::game_logic::host_armor_residual::resolve_host_death_type(
                                        fire_wname.as_deref(),
                                        damage_type,
                                    );
                                    crate::game_logic::object::prime_live_damage_context(
                                        self.objects.get(&attacker_id),
                                        fire_wname.as_deref(),
                                        damage_type,
                                    );
                                    let at_self = fire_wname.as_deref().map(
                                        crate::game_logic::weapon_bootstrap::host_damage_dealt_at_self_position_for_weapon_name,
                                    )
                                    .unwrap_or(false);
                                    let shooter_pos = self
                                        .objects
                                        .get(&attacker_id)
                                        .map(|a| a.get_position())
                                        .unwrap_or(target_position);
                                    if !at_self {
                                        if let Some(target) = self.objects.get_mut(&target_id) {
                                            if target
                                                .get_sneaky_targeting_offset(self.frame)
                                                .is_some()
                                            {
                                                // C++ fireWeaponTemplate clears victimObj; the shot
                                                // flies at the offset point and does not connect.
                                            } else {
                                                let destroyed = target
                                                    .take_damage_from_typed_death(
                                                        weapon_damage,
                                                        Some(attacker_id),
                                                        damage_type,
                                                        death_type,
                                                    );
                                                if destroyed {
                                                    // C++ parity: XP is victim ExperienceValue at current level.
                                                    let kill_xp = target.kill_experience_value();
                                                    let victim_pos = target.get_position();
                                                    let victim_team = target.team;
                                                    self.mark_object_for_destruction(
                                                        target_id,
                                                        Some(attacker_team),
                                                    );
                                                    self.continue_or_stop_after_kill(
                                                        attacker_id,
                                                        target_id,
                                                        victim_pos,
                                                        victim_team,
                                                        fire_wname.as_deref(),
                                                        kill_xp,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    // C++ dual-radius splash residual after direct hit.
                                    // DamageDealtAtSelfPosition recenters on the shooter and
                                    // clears primary-victim skip (Weapon.cpp:1008, 1035).
                                    {
                                        use crate::game_logic::weapon_bootstrap::{
                                            host_primary_damage_radius_for_weapon_name,
                                            host_secondary_damage_for_weapon_name,
                                            host_secondary_damage_radius_for_weapon_name,
                                        };
                                        let wname =
                                            self.objects.get(&attacker_id).and_then(|attacker| {
                                                attacker
                                                    .weapon_name_for_slot(slot)
                                                    .map(str::to_owned)
                                            });
                                        let (pr, sr, sd) = if let Some(ref n) = wname {
                                            (
                                                host_primary_damage_radius_for_weapon_name(n),
                                                host_secondary_damage_radius_for_weapon_name(n),
                                                host_secondary_damage_for_weapon_name(n),
                                            )
                                        } else {
                                            (0.0, 0.0, 0.0)
                                        };
                                        let (splash_weapon, radius_mult) = self
                                            .objects
                                            .get(&attacker_id)
                                            .map(|a| {
                                                (
                                                    a.weapon_slot(slot)
                                                        .map(|w| w.splash_radius.max(0.0))
                                                        .unwrap_or(0.0),
                                                    a.weapon_bonus_radius(),
                                                )
                                            })
                                            .unwrap_or((0.0, 1.0));
                                        // C++ getPrimary/SecondaryDamageRadius — RADIUS field.
                                        let primary_r = (if pr > 0.0 { pr } else { splash_weapon })
                                            * radius_mult;
                                        let secondary_r = sr * radius_mult;
                                        if primary_r > 0.0 || secondary_r > 0.0 {
                                            let sec_dmg = sd;
                                            let splash_pos = if at_self {
                                                shooter_pos
                                            } else {
                                                target_position
                                            };
                                            let splash_intended =
                                                if at_self { ObjectId(0) } else { target_id };
                                            let hits = self.apply_instant_hit_splash_at(
                                                splash_pos,
                                                weapon_damage,
                                                sec_dmg,
                                                primary_r,
                                                secondary_r,
                                                attacker_id,
                                                attacker_team,
                                                splash_intended,
                                                wname.as_deref(),
                                            );
                                        }
                                    }
                                }
                            }
                        } // end !avenger_paint / !avenger_air residual branch
                    } // end !aurora_queued

                    // Inferno Cannon residual: InfernoTankShell Bezier flight → FireFieldSmall.
                    // Fail-closed: instant FireFieldSmall zone if shell spawn fails.
                    // Skipped for Aurora (delayed dive residual already queued).
                    if !aurora_queued {
                        use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
                        let is_inferno = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| is_inferno_cannon_template(&a.template_name))
                            .unwrap_or(false);
                        if is_inferno {
                            let impact = target_position;
                            let upgraded = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| {
                                    crate::game_logic::host_inferno_cannon::has_black_napalm_upgrade(
                                        &a.applied_upgrades,
                                    )
                                })
                                .unwrap_or(false);
                            let from = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| a.get_position())
                                .unwrap_or(impact);
                            let spawned = self
                                .spawn_inferno_shell_projectile(
                                    attacker_id,
                                    from,
                                    impact,
                                    Some(target_id),
                                    upgraded,
                                )
                                .is_some();
                            if !spawned {
                                let _ = self.spawn_inferno_fire_zone(
                                    attacker_id,
                                    attacker_team,
                                    impact,
                                    upgraded,
                                );
                            }
                        }
                    }
                } else if enemy_or_forced {
                    // Ready weapons but out of range / cannot hit.
                    // MinimumAttackRange residual: if too close, back away instead
                    // of chasing into the dead zone (artillery / rocket safety).
                    let (min_r, max_r, can_chase) = self
                        .objects
                        .get(&attacker_id)
                        .map(|attacker| {
                            let slot = attacker.selected_weapon_slot();
                            let w = slot.and_then(|s| attacker.weapon_slot(s));
                            let min_r = w.map(|w| w.min_range).unwrap_or(0.0);
                            let max_r = w.map(|w| w.range).unwrap_or(0.0)
                                * attacker.battle_plan_range_multiplier();
                            let can = attacker.can_move()
                                && matches!(
                                    attacker.ai_state,
                                    AIState::Idle
                                        | AIState::Moving
                                        | AIState::Attacking
                                        | AIState::AttackMoving
                                        | AIState::Patrolling
                                        | AIState::AttackingGround
                                );
                            (min_r, max_r, can)
                        })
                        .unwrap_or((0.0, 0.0, false));
                    let too_close = {
                        let src = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| a.get_position())
                            .unwrap_or(target_position);
                        let dx = src.x - target_position.x;
                        let dz = src.z - target_position.z;
                        let dist = (dx * dx + dz * dz).sqrt();
                        crate::game_logic::weapon_bootstrap::is_inside_minimum_attack_range(
                            dist, min_r,
                        )
                    };
                    if too_close && can_chase {
                        let _ = self.try_min_range_backup(attacker_id, target_position, min_r);
                        continue;
                    }
                    // Pathfind toward target (not straight-line through buildings).
                    // Do not clobber interaction orders that also set `target`
                    // (CaptureBuilding, SpecialAbility, Repair, Enter, etc.).
                    let combat_chase_ok = can_chase;
                    let _ = max_r;
                    if combat_chase_ok {
                        // findAttackPath residual: path to in-range LOS cell, not target cell.
                        // Contact weapons path to the target; others stand off at range*0.9.
                        //
                        // Repath throttle: A* every OOR frame thrash-hangs Lone Eagle
                        // (hundreds of attackers × large static grid). Keep following an
                        // existing path; repath periodically or when path is exhausted.
                        let has_active_path = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| {
                                a.movement.current_path_index < a.movement.path.len()
                                    || a.movement.target_position.is_some()
                            })
                            .unwrap_or(false);
                        // ~0.5s at 30 Hz when already marching; always plan when idle/stuck.
                        let repath_due = !has_active_path || (self.frame % 15 == 0);
                        if repath_due {
                            let (wrange, wname) = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| {
                                    let slot = a.selected_weapon_slot();
                                    let r = slot
                                        .and_then(|s| a.weapon_slot(s))
                                        .map(|w| w.range)
                                        .unwrap_or(50.0);
                                    let n = slot
                                        .and_then(|s| a.weapon_name_for_slot(s).map(str::to_owned));
                                    (r, n)
                                })
                                .unwrap_or((50.0, None));
                            let approach = self.approach_pos_for_attack(
                                attacker_id,
                                target_position,
                                wrange,
                                wname.as_deref(),
                            );
                            let _ = self.assign_unit_attack_path(
                                attacker_id,
                                Some(target_id),
                                approach,
                            );
                            // C++ findAttackPath NULL: leave the unit halted
                            // (AIStates.cpp:1771-1778). Do not install a
                            // straight-line through walls.
                        }
                    }
                }
            } else if let Some(target_location) = target_location {
                // Force-attack-ground: consume a shot when the location is in range and apply damage
                // to the nearest hittable object around the designated impact point.
                // Leftover chooseBest no-victim: lock keeps the slot (FireWeapon
                // / rocket pods); unlocked ground fire leftover-resets PRIMARY.
                let ground_slot = self.objects.get(&attacker_id).and_then(|attacker| {
                    let requested = attacker.leftover_choose_best_ground_slot();
                    if attacker.weapon_slot(requested).is_some() {
                        Some(requested)
                    } else {
                        attacker.weapon_slot(0).is_some().then_some(0)
                    }
                });
                let rocket_pod_ground = {
                    use crate::game_logic::host_comanche_rocket_pods::{
                        UPGRADE_COMANCHE_ROCKET_PODS, is_comanche_template,
                        rocket_pod_ground_fire_active,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            rocket_pod_ground_fire_active(
                                is_comanche_template(&a.template_name),
                                a.has_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS)
                                    || a.has_upgrade_tag("Upgrade_ComancheRocketPods"),
                                a.tertiary_weapon.is_some(),
                                ground_slot.unwrap_or(0),
                            )
                        })
                        .unwrap_or(false)
                };

                let can_fire_at_location = ground_slot
                    .and_then(|slot| {
                        self.objects.get(&attacker_id).and_then(|attacker| {
                            attacker.weapon_slot(slot).map(|weapon| {
                                Object::weapon_ready(weapon, current_time)
                                    && attacker.weapon_allows_target_anti_mask(
                                        weapon,
                                        Some(slot),
                                        gamelogic::weapon::WeaponAntiMask::GROUND,
                                    )
                                    && attacker.position.distance(target_location) <= weapon.range
                            })
                        })
                    })
                    .unwrap_or(false);

                if can_fire_at_location {
                    // AcceptableAimDelta residual for force-attack-ground.
                    let Some(ground_slot) = ground_slot else {
                        continue;
                    };
                    // Match DeployStyleAIUpdate's in-range victim-position
                    // path: force-fire may begin unpacking only after its
                    // selected weapon can actually reach the location.
                    if !self.ensure_deploy_style_ready_to_fire(attacker_id) {
                        continue;
                    }
                    let aim_ok = if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                        attacker.set_ai_state(AIState::AttackingGround);
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            crate::game_logic::host_ai_decision_log::record_set_state(
                                attacker_id,
                                4,
                            ); // AttackingGround
                        }
                        attacker.set_status_attacking(true);
                        let max_step = if attacker.status.moving && attacker.can_move() {
                            0.2
                        } else {
                            std::f32::consts::PI
                        };
                        attacker.turn_toward_position(target_location, ground_slot, max_step)
                    } else {
                        true
                    };
                    if !aim_ok {
                        continue;
                    }

                    // Pitch window residual for ground fire.
                    {
                        let pitch_ok = if let Some(attacker) = self.objects.get(&attacker_id) {
                            let wname = attacker.weapon_name_for_slot(ground_slot);
                            let limits = wname
                                .map(crate::game_logic::weapon_bootstrap::host_target_pitch_limits_for_weapon_name)
                                .unwrap_or_default();
                            crate::game_logic::weapon_bootstrap::is_pitch_within_limits(
                                attacker.get_position(),
                                target_location,
                                &limits,
                            )
                        } else {
                            true
                        };
                        if !pitch_ok {
                            continue;
                        }
                    }
                    let mut weapon_damage = self
                        .objects
                        .get(&attacker_id)
                        .map(|attacker| {
                            let name = attacker.weapon_name_for_slot(ground_slot);
                            let base = name
                                .and_then(
                                    crate::game_logic::weapon_bootstrap::host_primary_damage_for_weapon_name,
                                )
                                .or_else(|| {
                                    attacker.weapon_slot(ground_slot).map(|weapon| weapon.damage)
                                })
                                .unwrap_or(0.0);
                            attacker.effective_weapon_damage(base)
                        })
                        .unwrap_or(0.0);
                    if overcharge {
                        weapon_damage *= 1.1;
                    }

                    fired_slot = Some(ground_slot);

                    // Aurora dive bomb residual on force-attack-ground.
                    let aurora_ground_queued = {
                        use crate::game_logic::host_aurora_bomb::{
                            aurora_bomb_kind_for_template, is_aurora_aircraft_template,
                        };
                        let aurora = self.objects.get(&attacker_id).and_then(|a| {
                            if is_aurora_aircraft_template(&a.template_name) {
                                Some(aurora_bomb_kind_for_template(&a.template_name))
                            } else {
                                None
                            }
                        });
                        if let Some(kind) = aurora {
                            let _ = self.queue_aurora_bomb(
                                kind,
                                attacker_id,
                                attacker_team,
                                target_location,
                            );
                            true
                        } else {
                            false
                        }
                    };

                    if !aurora_ground_queued {
                        // GLA Rocket Buggy / SCUD residual ground force-fire AOE.
                        let buggy_ground = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| {
                                crate::game_logic::host_rocket_buggy::is_rocket_buggy_template(
                                    &a.template_name,
                                )
                            })
                            .unwrap_or(false);
                        let scud_ground = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| {
                                crate::game_logic::host_scud_launcher::is_scud_launcher_template(
                                    &a.template_name,
                                )
                            })
                            .unwrap_or(false);
                        let tomahawk_ground = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| {
                                crate::game_logic::host_tomahawk::is_tomahawk_template(
                                    &a.template_name,
                                )
                            })
                            .unwrap_or(false);

                        if rocket_pod_ground {
                            // Retail FIRE_WEAPON tertiary at position → scatter projectile + area.
                            use crate::game_logic::host_comanche_rocket_pods::{
                                ROCKET_POD_CLIP_SIZE, rocket_pod_scatter_impact,
                            };
                            let idx = {
                                let shot = self
                                    .comanche_rocket_pod_shot_index
                                    .entry(attacker_id)
                                    .or_insert(0);
                                let i = *shot;
                                *shot = shot.saturating_add(1) % ROCKET_POD_CLIP_SIZE.max(1);
                                i
                            };
                            let (sx, sy, sz) = rocket_pod_scatter_impact(
                                target_location.x,
                                target_location.y,
                                target_location.z,
                                idx,
                            );
                            let aim = Vec3::new(sx, sy, sz);
                            let from = self
                                .objects
                                .get(&attacker_id)
                                .map(|o| o.get_position())
                                .unwrap_or(target_location);
                            let _ = self.spawn_comanche_rocket_pod_projectile(
                                attacker_id,
                                from,
                                aim,
                                idx,
                            );
                            let (hits, _) = self.apply_comanche_rocket_pod_area_at(
                                target_location,
                                Some(attacker_id),
                            );
                            let _ = weapon_damage; // area residual owns damage
                        } else if buggy_ground {
                            let from = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| a.get_position())
                                .unwrap_or(target_location);
                            let spawned = self
                                .spawn_rocket_buggy_missile_projectile(
                                    attacker_id,
                                    from,
                                    target_location,
                                    None,
                                )
                                .is_some();
                            let (hits, _) = if spawned {
                                self.rocket_buggy_residual_fires =
                                    self.rocket_buggy_residual_fires.saturating_add(1);
                                (1, false)
                            } else {
                                self.apply_rocket_buggy_residual_at(
                                    target_location,
                                    Some(attacker_id),
                                    None,
                                )
                            };
                            let _ = weapon_damage;
                        } else if scud_ground {
                            // Ground force-fire: stock uses primary explosive; Chem SCUD
                            // residual uses anthrax primary (slot 0 toxin warhead).
                            let toxin = {
                                use crate::game_logic::host_scud_launcher::scud_toxin_warhead_for_slot;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        scud_toxin_warhead_for_slot(&a.template_name, ground_slot)
                                    })
                                    .unwrap_or(false)
                            };
                            let from = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| a.get_position())
                                .unwrap_or(target_location);
                            let spawned = self
                                .spawn_scud_launcher_missile_projectile(
                                    attacker_id,
                                    from,
                                    target_location,
                                    None,
                                    toxin,
                                )
                                .is_some();
                            let (hits, _) = if spawned {
                                (1, false)
                            } else {
                                self.apply_scud_area_at(
                                    target_location,
                                    Some(attacker_id),
                                    attacker_team,
                                    toxin,
                                )
                            };
                            let _ = weapon_damage;
                        } else if tomahawk_ground {
                            let from = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| a.get_position())
                                .unwrap_or(target_location);
                            let spawned = self
                                .spawn_tomahawk_missile_projectile(
                                    attacker_id,
                                    from,
                                    target_location,
                                    None,
                                )
                                .is_some();
                            let (hits, _) = if spawned {
                                self.tomahawk_residual_fires =
                                    self.tomahawk_residual_fires.saturating_add(1);
                                (1, false)
                            } else {
                                self.apply_tomahawk_residual_at(
                                    target_location,
                                    Some(attacker_id),
                                    None,
                                )
                            };
                            let _ = weapon_damage;
                        } else if let Some(ground_target_id) =
                            self.find_ground_attack_victim(attacker_id, target_location)
                        {
                            let ground_wname =
                                self.objects.get(&attacker_id).and_then(|attacker| {
                                    attacker
                                        .weapon_name_for_slot(ground_slot)
                                        .map(str::to_owned)
                                });
                            let damage_type = ground_wname
                                .as_deref()
                                .map(
                                    crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name,
                                )
                                // C++ WeaponTemplate ctor defaults m_damageType to
                                // DAMAGE_EXPLOSION (Weapon.cpp:249); an unnamed host
                                // weapon fires Explosion, not Bullet.
                                .unwrap_or(crate::game_logic::combat::DamageType::Explosive);
                            crate::game_logic::object::prime_live_damage_context(
                                self.objects.get(&attacker_id),
                                ground_wname.as_deref(),
                                damage_type,
                            );
                            if let Some(target) = self.objects.get_mut(&ground_target_id) {
                                let destroyed = target.take_damage_from_typed(
                                    weapon_damage,
                                    Some(attacker_id),
                                    damage_type,
                                );
                                if destroyed {
                                    self.mark_object_for_destruction(
                                        ground_target_id,
                                        Some(attacker_team),
                                    );
                                    self.award_score_the_kill_experience(
                                        attacker_id,
                                        ground_target_id,
                                    );
                                }
                            }
                        }

                        // Inferno Cannon residual: ground attack also seeds FireFieldSmall.
                        if !rocket_pod_ground && !buggy_ground && !scud_ground {
                            use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
                            let is_inferno = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| is_inferno_cannon_template(&a.template_name))
                                .unwrap_or(false);
                            if is_inferno {
                                let upgraded = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        crate::game_logic::host_inferno_cannon::has_black_napalm_upgrade(
                                            &a.applied_upgrades,
                                        )
                                    })
                                    .unwrap_or(false);
                                let _ = self.spawn_inferno_fire_zone(
                                    attacker_id,
                                    attacker_team,
                                    target_location,
                                    upgraded,
                                );
                            }
                        }
                    }
                }
            }

            if let Some(slot) = fired_slot {
                // Pathfinder residual sniper honesty (any successful fire from residual).
                if self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        crate::game_logic::host_pathfinder::is_pathfinder_template(&a.template_name)
                    })
                    .unwrap_or(false)
                {
                    self.pathfinder_residual_sniper_fires =
                        self.pathfinder_residual_sniper_fires.saturating_add(1);
                }

                // Quad Cannon residual honesty: ground primary vs AA secondary fires.
                if self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        crate::game_logic::host_quad_cannon::is_quad_cannon_template(
                            &a.template_name,
                        )
                    })
                    .unwrap_or(false)
                {
                    if slot == 1 {
                        self.quad_cannon_residual_aa_fires =
                            self.quad_cannon_residual_aa_fires.saturating_add(1);
                    } else {
                        self.quad_cannon_residual_ground_fires =
                            self.quad_cannon_residual_ground_fires.saturating_add(1);
                    }
                }

                // China Gattling Tank residual: advance continuous-fire ramp + honesty.
                if self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        crate::game_logic::host_gattling_tank::is_gattling_tank_template(
                            &a.template_name,
                        )
                    })
                    .unwrap_or(false)
                {
                    self.advance_gattling_continuous_fire(attacker_id, target_id, slot);
                }

                // China MiniGunner residual: advance continuous-fire ramp + honesty.
                if self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        crate::game_logic::host_minigunner::is_minigunner_template(&a.template_name)
                    })
                    .unwrap_or(false)
                {
                    self.advance_minigunner_continuous_fire(attacker_id, target_id, slot);
                }

                // Combat particle residual: weapon fire → muzzle (+ impact) registry entries.
                let muzzle_pos = self
                    .objects
                    .get(&attacker_id)
                    .map(|a| a.get_position())
                    .unwrap_or(Vec3::ZERO);
                let fire_target = target_id.filter(|id| self.objects.contains_key(id));
                let impact_pos = fire_target
                    .and_then(|id| self.objects.get(&id).map(|t| t.get_position()))
                    .or(target_location);
                let fire_frame = self.frame;
                let (fire_fx, det_fx) = {
                    self.objects
                        .get(&attacker_id)
                        .and_then(|attacker| {
                            let veterancy = attacker.experience.level;
                            attacker.weapon_name_for_slot(slot).map(|weapon_name| {
                                (
                                    crate::game_logic::weapon_bootstrap::host_fire_fx_for_weapon_name_at_veterancy(
                                        weapon_name,
                                        veterancy,
                                    ),
                                    crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name_at_veterancy(
                                        weapon_name,
                                        veterancy,
                                    ),
                                )
                            })
                        })
                        .unwrap_or_default()
                };
                let (fire_ocl, det_ocl) = {
                    self.objects
                        .get(&attacker_id)
                        .and_then(|attacker| {
                            let veterancy = attacker.experience.level;
                            attacker.weapon_name_for_slot(slot).map(|weapon_name| {
                                (
                                    crate::game_logic::weapon_bootstrap::host_fire_ocl_for_weapon_name_at_veterancy(
                                        weapon_name,
                                        veterancy,
                                    ),
                                    crate::game_logic::weapon_bootstrap::host_detonation_ocl_for_weapon_name_at_veterancy(
                                        weapon_name,
                                        veterancy,
                                    ),
                                )
                            })
                        })
                        .unwrap_or_default()
                };
                // C++ Weapon::fireWeaponTemplate invokes FireOCL with the
                // firing object (`Weapon.cpp:943-949`).  This normal combat
                // path does not route through PendingProjectile, so retain
                // the firing context for the parsed OCL below.
                let fire_ocl_source = self.objects.get(&attacker_id).map(|attacker| {
                    (
                        attacker.team,
                        attacker.experience.level,
                        attacker.get_orientation(),
                        attacker.movement.velocity,
                    )
                });
                // C++ Weapon::fireWeaponTemplate FireFX stealth gate residual:
                // stealthed+undetected+non-disguised suppress muzzle FX unless
                // the observer locally controls the source, PlayFXWhenStealthed
                // is set, or the source is KINDOF_MINE.
                let suppress_fire_fx = {
                    let a = self.objects.get(&attacker_id);
                    a.map(|o| {
                        let locally_controlled = source_is_locally_controlled(
                            o.owner_player_id,
                            self.local_player_id(),
                        );
                        let is_mine = o.is_kind_of(KindOf::Mine);
                        let hidden = !locally_controlled
                            && o.status.stealthed
                            && !o.status.detected
                            && !o.status.disguised
                            && !is_mine;
                        if !hidden {
                            return false;
                        }
                        let wname = o.weapon_name_for_slot(slot);
                        let play = wname
                            .map(
                                crate::game_logic::weapon_bootstrap::host_play_fx_when_stealthed_for_weapon_name,
                            )
                            .unwrap_or(false);
                        !play
                    })
                    .unwrap_or(false)
                };
                // C++ Weapon.cpp:904-939: doFXPos only when FireFX is non-null.
                // Dispatch play_dispatch_fire_fx fail-closes on empty FireFX
                // (TestTank has no Weapon.ini name). Residual muzzle/impact
                // still registers so fire is observable; named FireFX stays
                // on the dispatch path. See combat_fire_fx.rs.
                self.spawn_residual_muzzle_when_dispatch_has_no_fire_fx(
                    suppress_fire_fx,
                    &fire_fx,
                    &det_fx,
                    muzzle_pos,
                    impact_pos,
                    fire_frame,
                    attacker_id,
                    fire_target,
                );
                let _ = det_ocl;
                // C++ performs FireFX before FireOCL (`Weapon.cpp:889-949`).
                // FireOCL is intentionally outside the visual stealth gate:
                // hiding a muzzle effect does not suppress the authored game
                // object creation effect.
                if !fire_ocl.is_empty() {
                    if let Some((
                        source_team,
                        source_veterancy,
                        source_orientation,
                        source_velocity,
                    )) = fire_ocl_source
                    {
                        let _ = self.execute_parsed_weapon_ocl_at(
                            &fire_ocl,
                            Some(attacker_id),
                            source_team,
                            source_veterancy,
                            source_orientation,
                            source_velocity,
                            muzzle_pos,
                        );
                    }
                }
                // C++ Weapon.ini LaserName residual: short-lived combat beam for
                // presentation / laser_segment_upload observe path.
                {
                    let weapon_name = self
                        .objects
                        .get(&attacker_id)
                        .and_then(|attacker| attacker.weapon_name_for_slot(slot));
                    let laser_name = weapon_name
                        .map(crate::game_logic::weapon_bootstrap::host_laser_name_for_weapon_name)
                        .unwrap_or_default();
                    if !laser_name.is_empty() {
                        let laser_bone = weapon_name
                            .map(crate::game_logic::weapon_bootstrap::host_laser_bone_name_for_weapon_name)
                            .unwrap_or_default();
                        let to = impact_pos.unwrap_or(muzzle_pos);
                        let laser_name_owned = laser_name.clone();
                        self.weapon_lasers.push(
                            crate::game_logic::host_weapon_laser::ResidualWeaponLaser::with_bone(
                                laser_name,
                                laser_bone,
                                attacker_id,
                                fire_target,
                                (muzzle_pos.x, muzzle_pos.y, muzzle_pos.z),
                                (to.x, to.y, to.z),
                                fire_frame,
                            ),
                        );
                        let _ = self.spawn_weapon_laser_beam_object(
                            &laser_name_owned,
                            attacker_id,
                            fire_target,
                            muzzle_pos,
                            to,
                        );
                    }
                }

                // Audio residual (hq-7zxm slice): weapon fire → real AudioEventRequest.
                // C++ FiringTracker::shotFired plays `weaponFired->getFireSound()`
                // (FiringTracker.cpp:144-155) — the WeaponTemplate's authored
                // AudioEventRTS parsed from `FireSound` (Weapon.cpp:171,
                // Weapon.h:678). AudioManager::addAudioEvent returns AHSV_NoSound
                // for an empty event name (GameAudio.cpp:384-386): a weapon with
                // no authored FireSound is silent. Never queue an invented
                // generic token — no "WeaponFire" AudioEvent exists in retail
                // SoundEffects.ini, so it can only dead-end as ERR(no-info).
                let fire_sound = self
                    .objects
                    .get(&attacker_id)
                    .and_then(|attacker| attacker.weapon_name_for_slot(slot))
                    .map(crate::game_logic::weapon_bootstrap::host_fire_sound_for_weapon_name)
                    .filter(|sound| !sound.is_empty());
                if let Some(fire_sound) = fire_sound {
                    self.queue_audio_event(
                        AudioEventRequest::new(fire_sound.as_str())
                            .with_object(attacker_id)
                            .with_position(muzzle_pos)
                            .with_priority(160),
                    );
                }

                // Capture weapon name before mut borrow for RETURN_TO_BASE peels.
                let fire_wname = self
                    .objects
                    .get(&attacker_id)
                    .and_then(|attacker| attacker.weapon_name_for_slot(slot).map(str::to_owned));
                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                    let _ = attacker.capture_pending_weapon_visual_dispatch(
                        slot,
                        self.frame,
                        fire_target,
                        impact_pos,
                    );
                    let auto_reloaded_clip = if let Some(weapon) = attacker.weapon_slot_mut(slot) {
                        Object::consume_ammo_on_fire_named(
                            weapon,
                            current_time,
                            fire_wname.as_deref(),
                        );
                        Object::auto_reloaded_clip_after_firing(weapon, fire_wname.as_deref())
                    } else {
                        false
                    };
                    // Match Object::fireCurrentWeapon: only the actual
                    // temporarily locked slot can end that lock by finishing
                    // an auto-reloading clip.  Do not let an unrelated
                    // PRIMARY/SECONDARY fallback discharge a TERTIARY lock.
                    if auto_reloaded_clip
                        && attacker.weapon_lock_type == WeaponLockType::LockedTemporarily
                        && attacker.weapon_lock_slot == slot
                    {
                        attacker.release_weapon_lock(WeaponLockType::LockedTemporarily);
                    }
                    if let Some(tid) = attacker.target {
                        attacker.record_shot_at_target(tid);
                        attacker.stamp_continuous_fire_coast(self.frame);
                        attacker.stamp_auto_reload_when_idle_from_slot(slot, self.frame);
                    }
                    // C++ STEALTH_NOT_WHILE_ATTACKING residual: combat fire breaks stealth.
                    if attacker.stealth_breaks_on_attack && attacker.status.stealthed {
                        attacker.break_stealth();
                    }
                }
                // This direct normal-combat finalizer does not route through
                // Object::fire_at_ex.  Its accepted shot already consumed
                // ammo above, so normalize the exact pre-advance barrel here
                // rather than leaving this live route cursor-static or
                // synthesizing recoil from a fire-intent writeback.
                if self
                    .record_accepted_weapon_discharge(attacker_id, slot)
                    .is_none()
                {
                    // Keep gameplay cursor progression sound even if a
                    // malformed source state cannot produce presentation.
                    if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                        attacker.advance_weapon_barrel_after_shot(slot);
                    }
                }
            }
        }

        // AssistedTargeting residual: advance pending Patriot assist clips after
        // primary fire this combat pass (AssistingClipSize / DelayBetweenShots).
        // Wave 824: under coupled shadow, pending patriot assists sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_pending_patriot_assists();
        }
        // BinaryDataStream laser residual: expire DeletionUpdate lifetime beams.
        // Wave 823: under coupled shadow, patriot assist lasers sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_patriot_assist_lasers();
        }
        // Weapon.ini LaserName residual lifetime / scroll.
        crate::game_logic::host_weapon_laser::update_weapon_lasers(
            &mut self.weapon_lasers,
            self.frame,
        );
    }

    pub(in super::super) fn find_ground_attack_victim(
        &self,
        attacker_id: ObjectId,
        target_location: Vec3,
    ) -> Option<ObjectId> {
        const GROUND_IMPACT_RADIUS: f32 = 12.0;

        let attacker = self.objects.get(&attacker_id)?;
        let force_attack = attacker.force_attack;
        let attacker_team = attacker.team;

        // Pure residual acquire: nearest attackable victim near ground impact (3D).
        let candidate_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(&candidate_id, candidate)| {
                if candidate_id == attacker_id
                    || !candidate.is_alive()
                    || (!candidate.is_attackable() && !candidate.is_disarmable_mine())
                {
                    return None;
                }
                if !force_attack && candidate.team == attacker_team {
                    return None;
                }
                Some(candidate_id)
            })
            .collect();

        let attacker = self.objects.get(&attacker_id)?;
        let candidates: Vec<_> = candidate_ids
            .into_iter()
            .filter_map(|id| {
                let candidate = self.objects.get(&id)?;
                if !attacker.can_target(candidate) {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: candidate.team,
                        position: candidate.get_position(),
                        is_alive: true,
                        is_neutral: candidate.team == Team::Neutral,
                        under_construction: candidate.status.under_construction,
                        // DISARM exception: DozerAIUpdate::clearMines scans the
                        // partition manager without a stealth gate, so a hidden
                        // mine must not be skipped by residual acquire either.
                        effectively_stealthed: candidate.is_effectively_stealthed()
                            && !(crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage(
                                attacker.weapon_name_for_slot(0).unwrap_or(""),
                            ) && candidate.is_disarmable_mine()),
                        is_air: candidate.is_kind_of(KindOf::Aircraft)
                            || candidate.status.airborne_target,
                        combat_kind: true,
                        eject_invulnerable: candidate.is_eject_invulnerable(),
                    },
                )
            })
            .collect();

        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            attacker_id,
            attacker_team,
            target_location,
            candidates,
            |_| GROUND_IMPACT_RADIUS,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }
}
