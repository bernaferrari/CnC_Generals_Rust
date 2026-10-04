//! Object-target slot selection, aiming/pre-attack gates and approach paths.
use super::*;

impl GameLogic {
    pub(super) fn update_object_target_combat(
        &mut self,
        input: CombatAttackInput,
        target_id: ObjectId,
    ) -> CombatShotResult {
        let CombatAttackInput {
            attacker_id,
            attacker_team,
            current_time,
            overcharge,
            ..
        } = input;
        let mut shot = CombatShotResult::NoShot;
        let target_status = self.objects.get(&target_id).map(|target| {
            (
                target.is_alive(),
                target.apply_sneaky_targeting_offset(target.get_position(), self.frame),
                target.is_temporarily_preventing_aim_success(self.frame),
            )
        });

        let Some((target_alive, target_position, lockon_block)) = target_status else {
            self.stop_attack_decision_aware(attacker_id);
            return CombatShotResult::NoShot;
        };

        if !target_alive {
            if let Some(atk) = self.objects.get_mut(&attacker_id) {
                atk.notify_jet_victim_is_dead(self.frame);
            }
            self.stop_attack_decision_aware(attacker_id);
            return CombatShotResult::NoShot;
        }
        if lockon_block {
            return CombatShotResult::NoShot;
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
            return CombatShotResult::NoShot;
        }

        // C++ chooseBestWeaponForTarget commits m_curWeapon before
        // the attack state asks current-weapon range or line of sight.
        // Keep the same owned slot for all remaining checks, including
        // an unready backup weapon chosen for approaching the victim.
        if let Some(slot) = selected_slot {
            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                attacker.set_active_weapon_slot(slot);
            }
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
                return CombatShotResult::NoShot;
            }
            in_range.then_some(slot)
        } else {
            None
        };

        if let Some(slot) = selected_slot {
            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                let flying = attacker.is_above_terrain()
                    && (attacker.locomotor_surfaces & crate::game_logic::object::LOCO_SURFACE_AIR)
                        != 0;
                if attacker.has_max_shots_remaining()
                    && attacker.ai_state != AIState::AttackMoving
                    && (!attacker.movement.path.is_empty()
                        || attacker.movement.velocity.x != 0.0
                        || attacker.movement.velocity.z != 0.0)
                {
                    attacker.movement.path.clear();
                    attacker.movement.target_position = None;
                    attacker.movement.velocity.x = 0.0;
                    attacker.movement.velocity.z = 0.0;
                    attacker.set_status_moving(false);
                }
                if flying
                    && attacker.has_max_shots_remaining()
                    && attacker.ai_state != AIState::AttackMoving
                {
                    attacker.requested_destination = None;
                }
            }
            // C++ DeployStyleAIUpdate::update only enters DEPLOY once
            // its current victim is within the current weapon's attack
            // range.  Do this after slot/range selection, rather than
            // before it, so an out-of-range attack can keep its target
            // and approach path instead of packing in place.
            if !self.ensure_deploy_style_ready_to_fire(attacker_id) {
                return CombatShotResult::NoShot;
            }

            // Same predicate AIAttackState uses. In-range was already
            // required to reach here, so a true result is the obstacle check.
            if self.out_of_weapon_range_object(attacker_id, target_id) {
                // Ready but LOS blocked → findAttackPath residual (firing cell).
                let combat_chase_ok = self
                    .objects
                    .get(&attacker_id)
                    .map(|attacker| {
                        attacker.can_move()
                            && !(attacker.is_above_terrain()
                                && (attacker.locomotor_surfaces
                                    & crate::game_logic::object::LOCO_SURFACE_AIR)
                                    != 0)
                            && crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(
                                attacker,
                            )
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
                    let wname = self.objects.get(&attacker_id).and_then(|a| {
                        a.selected_weapon_slot().and_then(|slot| {
                            a.weapon_name_for_slot(slot).map(|name| name.to_owned())
                        })
                    });
                    let wrange = self
                        .objects
                        .get(&attacker_id)
                        .and_then(|a| {
                            let under =
                                crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
                            a.selected_weapon_slot().and_then(|slot| {
                                a.weapon_slot(slot)
                                    .map(|w| (a.effective_weapon_range(w.range) - under).max(0.0))
                            })
                        })
                        .unwrap_or(50.0);
                    let approach = self.approach_pos_for_attack(
                        attacker_id,
                        target_position,
                        wrange,
                        wname.as_deref(),
                        Some(target_id),
                    );
                    let _ = self.assign_unit_attack_path(attacker_id, Some(target_id), approach);
                } else if self.objects.get(&attacker_id).is_some_and(|attacker| {
                    attacker.can_move()
                        && attacker.is_above_terrain()
                        && (attacker.locomotor_surfaces
                            & crate::game_logic::object::LOCO_SURFACE_AIR)
                            != 0
                }) {
                    let wname = self.objects.get(&attacker_id).and_then(|a| {
                        a.selected_weapon_slot().and_then(|slot| {
                            a.weapon_name_for_slot(slot).map(|name| name.to_owned())
                        })
                    });
                    let wrange = self
                        .objects
                        .get(&attacker_id)
                        .and_then(|a| {
                            let under =
                                crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
                            a.selected_weapon_slot().and_then(|slot| {
                                a.weapon_slot(slot)
                                    .map(|w| (a.effective_weapon_range(w.range) - under).max(0.0))
                            })
                        })
                        .unwrap_or(50.0);
                    let goal = self.approach_pos_for_attack(
                        attacker_id,
                        target_position,
                        wrange,
                        wname.as_deref(),
                        Some(target_id),
                    );
                    if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                        attacker.requested_destination = Some(goal);
                        let stale = attacker.movement.path.last().is_some_and(|point| {
                            let dx = point.x - goal.x;
                            let dz = point.z - goal.z;
                            dx * dx + dz * dz > 0.25
                        });
                        if stale {
                            attacker.movement.path.clear();
                            attacker.movement.target_position = None;
                        }
                        attacker.set_status_moving(true);
                    }
                }
                return CombatShotResult::NoShot;
            }

            // C++ AIStates AcceptableAimDelta residual: do not fire until facing
            // is within aim delta; turn in place toward the target instead.
            {
                let aim_ok = if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                    attacker.note_attack_target(target_id);
                    if !matches!(
                        attacker.ai_state,
                        AIState::Patrolling | AIState::AttackMoving | AIState::Attacking
                    ) {
                        attacker.set_ai_state(AIState::Attacking);
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
                    return CombatShotResult::NoShot;
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
                            let h = (t.thing.geometry.bounds_max.y - t.thing.geometry.bounds_min.y)
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
                    let mut entered_attack = false;
                    if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                        attacker.note_attack_target(target_id);
                        if !matches!(
                            attacker.ai_state,
                            AIState::Patrolling | AIState::AttackMoving | AIState::Attacking
                        ) {
                            attacker.set_ai_state(AIState::Attacking);
                            entered_attack = true;
                        }
                    }
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_attack(
                            attacker_id,
                            target_id,
                        );
                        if entered_attack {
                            crate::game_logic::host_ai_decision_log::record_set_state(
                                attacker_id,
                                2,
                            );
                        }
                    }
                    return CombatShotResult::NoShot;
                }
            }

            // C++ PreAttackType residual: wind-up before first discharge of
            // shot / attack / clip. Shared Object helpers match fire_at.
            {
                let pre_blocked = if let Some(attacker) = self.objects.get_mut(&attacker_id) {
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
                    let apply =
                        attacker.pre_attack_delay_applies(slot, target_id, prefire, pre_delay);
                    if apply {
                        let needs_arm = attacker.pre_attack_target != Some(target_id)
                            || attacker.pre_attack_ready_at <= 0.0;
                        if needs_arm {
                            attacker.pre_attack_target = Some(target_id);
                            attacker.pre_attack_ready_at = current_time + pre_delay;
                            attacker.activate_leech_range_for_slot(slot);
                        }
                        if current_time + 1e-6 < attacker.pre_attack_ready_at {
                            attacker.note_attack_target(target_id);
                            let entered_attack = !matches!(
                                attacker.ai_state,
                                AIState::Patrolling | AIState::AttackMoving | AIState::Attacking
                            );
                            if entered_attack {
                                attacker.set_ai_state(AIState::Attacking);
                            }
                            attacker.set_status_attacking(true);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_attack(
                                    attacker_id,
                                    target_id,
                                );
                                if entered_attack {
                                    crate::game_logic::host_ai_decision_log::record_set_state(
                                        attacker_id,
                                        2,
                                    );
                                }
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
                    return CombatShotResult::NoShot;
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
                return CombatShotResult::NoShot;
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

            shot = CombatShotResult::Fired(slot);

            self.apply_combat_object_hit(
                attacker_id,
                attacker_team,
                target_id,
                target_position,
                slot,
                enemy_or_forced,
                weapon_damage,
            );
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
                    let under = crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
                    let min_r = w.map(|w| (w.min_range - under).max(0.0)).unwrap_or(0.0);
                    let max_r = w
                        .map(|w| (attacker.effective_weapon_range(w.range) - under).max(0.0))
                        .unwrap_or(0.0);
                    let can = attacker.can_move()
                        && !(attacker.is_above_terrain()
                            && (attacker.locomotor_surfaces
                                & crate::game_logic::object::LOCO_SURFACE_AIR)
                                != 0)
                        && crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(
                            attacker,
                        )
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
                let (src, src_r) = self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        (
                            a.get_position(),
                            a.thing.template.geometry_info.bounding_circle_radius(),
                        )
                    })
                    .unwrap_or((target_position, 0.0));
                let tgt_r = self
                    .objects
                    .get(&target_id)
                    .map(|o| o.thing.template.geometry_info.bounding_circle_radius())
                    .unwrap_or(0.0);
                let dx = src.x - target_position.x;
                let dz = src.z - target_position.z;
                let center = (dx * dx + dz * dz).sqrt();
                let dist = (center - src_r - tgt_r).max(0.0);
                crate::game_logic::weapon_bootstrap::is_inside_minimum_attack_range(dist, min_r)
            };
            let air = self.objects.get(&attacker_id).is_some_and(|attacker| {
                attacker.is_above_terrain()
                    && (attacker.locomotor_surfaces & crate::game_logic::object::LOCO_SURFACE_AIR)
                        != 0
            });
            if air {
                if too_close && min_r > crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE {
                    let _ = self.try_min_range_backup_between(
                        attacker_id,
                        target_position,
                        min_r,
                        max_r,
                        Some(target_id),
                    );
                    return CombatShotResult::NoShot;
                } else {
                    let wname = self.objects.get(&attacker_id).and_then(|a| {
                        a.selected_weapon_slot().and_then(|slot| {
                            a.weapon_name_for_slot(slot).map(|name| name.to_owned())
                        })
                    });
                    let goal = self.approach_pos_for_attack(
                        attacker_id,
                        target_position,
                        max_r,
                        wname.as_deref(),
                        Some(target_id),
                    );
                    if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                        if attacker.can_move() {
                            attacker.requested_destination = Some(goal);
                            // C++ keeps the one-node path when squared distance < 0.25.
                            let stale = attacker.movement.path.last().is_some_and(|point| {
                                let dx = point.x - goal.x;
                                let dz = point.z - goal.z;
                                dx * dx + dz * dz > 0.25
                            });
                            if stale {
                                attacker.movement.path.clear();
                                attacker.movement.target_position = None;
                            }
                            attacker.set_status_moving(true);
                        }
                    }
                }
            } else if too_close
                && can_chase
                && min_r > crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE
            {
                let _ = self.try_min_range_backup_between(
                    attacker_id,
                    target_position,
                    min_r,
                    max_r,
                    Some(target_id),
                );
                return CombatShotResult::NoShot;
            }
            // Pathfind toward target (not straight-line through buildings).
            // Do not clobber interaction orders that also set `target`
            // (CaptureBuilding, SpecialAbility, Repair, Enter, etc.).
            let chase_dist = {
                let (src, src_r) = self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        (
                            a.get_position(),
                            a.thing.template.geometry_info.bounding_circle_radius(),
                        )
                    })
                    .unwrap_or((target_position, 0.0));
                let tgt_r = self
                    .objects
                    .get(&target_id)
                    .map(|o| o.thing.template.geometry_info.bounding_circle_radius())
                    .unwrap_or(0.0);
                let dx = src.x - target_position.x;
                let dz = src.z - target_position.z;
                let center = (dx * dx + dz * dz).sqrt();
                (center - src_r - tgt_r).max(0.0)
            };
            let combat_chase_ok = can_chase && chase_dist > max_r;
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
                if repath_due
                    && self.objects.get(&attacker_id).is_some_and(|attacker| {
                        !(attacker.is_above_terrain()
                            && (attacker.locomotor_surfaces
                                & crate::game_logic::object::LOCO_SURFACE_AIR)
                                != 0)
                            && crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(
                                attacker,
                            )
                    })
                {
                    let (wrange, wname) = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| {
                            let slot = a.selected_weapon_slot();
                            let under =
                                crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
                            let r = slot
                                .and_then(|s| a.weapon_slot(s))
                                .map(|w| (a.effective_weapon_range(w.range) - under).max(0.0))
                                .unwrap_or(50.0);
                            let n = slot.and_then(|s| a.weapon_name_for_slot(s).map(str::to_owned));
                            (r, n)
                        })
                        .unwrap_or((50.0, None));
                    let approach = self.approach_pos_for_attack(
                        attacker_id,
                        target_position,
                        wrange,
                        wname.as_deref(),
                        Some(target_id),
                    );
                    let _ = self.assign_unit_attack_path(attacker_id, Some(target_id), approach);
                    // C++ findAttackPath NULL: leave the unit halted
                    // (AIStates.cpp:1771-1778). Do not install a
                    // straight-line through walls.
                }
            }
        }
        shot
    }
}
