//! Ground-target firing/approach and actual victim acquisition.
use super::*;

impl GameLogic {
    pub(super) fn update_ground_target_combat(
        &mut self,
        input: CombatAttackInput,
        target_location: Vec3,
    ) -> CombatShotResult {
        self.update_ground_target_combat_with_effects(
            input,
            target_location,
            &mut Default::default(),
        )
    }

    pub(super) fn update_ground_target_combat_with_effects(
        &mut self,
        input: CombatAttackInput,
        target_location: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> CombatShotResult {
        let CombatAttackInput {
            attacker_id,
            attacker_team,
            current_time,
            overcharge,
            ..
        } = input;
        let mut shot = CombatShotResult::NoShot;
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
        if let Some(slot) = ground_slot {
            if !self.objects.get(&attacker_id).is_some_and(|attacker| {
                attacker.selected_projectile_is_admitted(
                    slot,
                    self.weapon_definitions
                        .as_ref()
                        .map(|_| &self.projectile_definitions),
                )
            }) {
                return CombatShotResult::NoShot;
            }
        }

        let rocket_pod_ground = {
            use crate::game_logic::host_comanche_rocket_pods::{
                COMANCHE_ROCKET_POD_WEAPON, UPGRADE_COMANCHE_ROCKET_PODS, rocket_pod_ground_fire_active,
            };
            self.objects
                .get(&attacker_id)
                .map(|a| {
                    rocket_pod_ground_fire_active(
                        a.weapon_name_for_slot(2) == Some(COMANCHE_ROCKET_POD_WEAPON),
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
                        let rof = self
                            .receiving_weapon_bonus_fields_for_slot(attacker, slot)
                            .2;
                        attacker.can_fire_slot_with_owned_rof(slot, current_time, rof)
                            && attacker.has_max_shots_remaining()
                            && attacker.weapon_allows_target_anti_mask(
                                weapon,
                                Some(slot),
                                gamelogic::weapon::WeaponAntiMask::GROUND,
                            )
                            && attacker.is_within_attack_range_pos_for_slot_with_solo_set(
                                slot,
                                target_location,
                                &self.solo_weapon_bonus_set,
                            )
                    })
                })
            })
            .unwrap_or(false);
        let can_fire_at_location = if can_fire_at_location {
            let (immobile, contained_by, spawns_weapons) = self
                .objects
                .get(&attacker_id)
                .map(|attacker| {
                    (
                        attacker.is_kind_of(crate::game_logic::KindOf::Immobile),
                        attacker.contained_by,
                        attacker.is_kind_of(crate::game_logic::KindOf::SpawnsAreTheWeapons),
                    )
                })
                .unwrap_or((false, None, false));
            let container_ground = contained_by.is_some_and(|id| {
                self.objects.get(&id).is_some_and(|container| {
                    container.is_kind_of(crate::game_logic::KindOf::Structure)
                        || !container.status.airborne_target
                })
            });
            let on_ground = immobile
                || spawns_weapons
                || container_ground
                || self.objects.get(&attacker_id).is_some_and(|attacker| {
                    crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(attacker)
                });
            !(on_ground && self.attack_view_blocked(attacker_id, None, target_location))
        } else {
            false
        };
        if !can_fire_at_location {
            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                if attacker.pre_attack_target.is_none() && attacker.pre_attack_ready_at > 0.0 {
                    attacker.pre_attack_ready_at = 0.0;
                    attacker.clear_leech_range_mode_for_all_weapons();
                    attacker.refresh_weapon_fire_status(current_time);
                }
            }
        }
        if can_fire_at_location {
            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                let flying = attacker.is_above_terrain()
                    && (attacker.locomotor_surfaces & crate::game_logic::object::LOCO_SURFACE_AIR)
                        != 0;
                if (!attacker.movement.path.is_empty()
                    || attacker.movement.velocity.x != 0.0
                    || attacker.movement.velocity.z != 0.0)
                {
                    attacker.movement.path.clear();
                    attacker.movement.target_position = None;
                    attacker.movement.velocity.x = 0.0;
                    attacker.movement.velocity.z = 0.0;
                    attacker.set_status_moving(false);
                }
                if flying && attacker.ai_state() != AIState::AttackMoving {
                    attacker.requested_destination = None;
                }
            }
            // AcceptableAimDelta residual for force-attack-ground.
            let Some(ground_slot) = ground_slot else {
                return CombatShotResult::NoShot;
            };
            // Match DeployStyleAIUpdate's in-range victim-position
            // path: force-fire may begin unpacking only after its
            // selected weapon can actually reach the location.
            if !self.ensure_deploy_style_ready_to_fire(attacker_id) {
                return CombatShotResult::NoShot;
            }
            let aim_ok = if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                attacker.set_ai_state(
                    AIState::AttackingGround,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                {}
                attacker.set_status_attacking(true);
                let max_step = if attacker.status.moving && attacker.can_move() {
                    0.2
                } else {
                    std::f32::consts::PI
                };
                attacker.turn_toward_position_with_operation(
                    target_location,
                    ground_slot,
                    max_step,
                    self.frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                )
            } else {
                true
            };
            if !aim_ok {
                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                    if attacker.pre_attack_target.is_none() && attacker.pre_attack_ready_at > 0.0 {
                        attacker.pre_attack_ready_at = 0.0;
                        attacker.clear_leech_range_mode_for_all_weapons();
                        attacker.refresh_weapon_fire_status(current_time);
                    }
                }
                return CombatShotResult::NoShot;
            }

            // C++ Weapon::fireWeapon(source, pos) fires the selected WeaponTemplate;
            // every authored weapon shares the one shot delivery operation.
            if self.slot_fires_authored_weapon(attacker_id, ground_slot) {
                // FireWeaponState enters PRE_ATTACK before invoking the position
                // shot operation. A null victim always has zero consecutive
                // shots (FiringTracker::getNumConsecutiveShotsAtVictim).
                let pre_bonus = self.objects.get(&attacker_id)
                    .map(|attacker| self.receiving_weapon_bonus_fields_for_slot(attacker, ground_slot).3)
                    .unwrap_or(1.0);
                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                    if let Some(rule) = attacker.selected_native_weapon_rule(ground_slot) {
                        let first_in_clip = attacker.weapon_slot(ground_slot).is_some_and(|weapon| {
                            weapon.clip_size <= 0 || weapon.ammo.unwrap_or(weapon.clip_size) >= weapon.clip_size
                        });
                        let applies = rule.prefire_type != gamelogic::weapon::WeaponPrefireType::PrefirePerClip
                            || first_in_clip;
                        let delay = ((rule.pre_attack_delay.max(0) as f32 * pre_bonus) as u32) as f32 / 30.0;
                        if applies && delay > 0.0 {
                            if attacker.pre_attack_target.is_some() || attacker.pre_attack_ready_at <= 0.0 {
                                attacker.pre_attack_target = None;
                                attacker.pre_attack_ready_at = current_time + delay;
                                attacker.weapon_fire_status = crate::game_logic::object::WeaponFireStatus::PreAttack;
                                attacker.sync_weapon_model_conditions_from_status();
                                attacker.activate_leech_range_for_slot(ground_slot);
                            }
                            if current_time + 1e-6 < attacker.pre_attack_ready_at {
                                return CombatShotResult::NoShot;
                            }
                        }
                    }
                }
                return self.fire_authored_ground_shot_with_effects(
                    input,
                    target_location,
                    ground_slot,
                    effects,
                );
            }

            // C++ has no pitch gate for ground-position fire:
            // Weapon::isWithinTargetPitch(source, victim) is only reached
            // for Object victims (WeaponSet.cpp:414 isAnyWithinTargetPitch,
            // :842 chooseBestWeaponForTarget). hq-2qedt.49.
            let mut weapon_damage = self
                .objects
                .get(&attacker_id)
                .map(|attacker| {
                    let name = attacker.weapon_name_for_slot(ground_slot);
                    let base = attacker.selected_native_weapon_rule(ground_slot).map(|rule| rule.primary_damage)
                        .or_else(|| name.and_then(crate::game_logic::weapon_bootstrap::host_primary_damage_for_weapon_name))
                        .or_else(|| {
                            attacker.weapon_slot(ground_slot).map(|weapon| weapon.damage)
                        })
                        .unwrap_or(0.0);
                    base * self.receiving_weapon_bonus_fields_for_slot(attacker, ground_slot).0
                })
                .unwrap_or(0.0);
            if overcharge {
                weapon_damage *= 1.1;
            }

            let origin = self
                .objects
                .get(&attacker_id)
                .map_or(Vec3::ZERO, Object::get_position);
            let visual = self.begin_owned_weapon_effects_with_effects(
                attacker_id,
                ground_slot,
                None,
                Some(target_location),
                origin,
                None,
                effects,
            );
            shot = CombatShotResult::Fired(ground_slot, visual);

            // Unadmitted bare weapon (no WeaponTemplate): damage the nearest
            // victim at the impact with C++ WeaponTemplate defaults
            // (DAMAGE_EXPLOSION / DEATH_NORMAL, Weapon.cpp:249).
            if let Some(ground_target_id) =
                self.find_ground_attack_victim(attacker_id, target_location, ground_slot)
            {
                let damage_type = crate::game_logic::combat::DamageType::Explosive;
                let context = crate::game_logic::object::DamageHitContext::new_selected(
                    self.objects.get(&attacker_id),
                    None,
                    damage_type,
                    None,
                );
                let _ = self.apply_owned_damage_with_effects(
                    ground_target_id,
                    weapon_damage,
                    Some(attacker_id),
                    damage_type,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                    None,
                    &context,
                    effects,
                );
            }
            // The common successful-shot finalizer owns ammo, temporary-slot
            // unlock, stealth disruption and accepted presentation/barrel commit
            // for both Object and position targets (CPP Weapon.cpp2692-2703).
            // Keep only this ground command's max-shot/target cleanup here.
            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                attacker.consume_max_shot_count();
                if attacker.max_shots_to_fire == 0 {
                    attacker.target_location = None;
                    attacker.set_force_attack(false);
                    attacker.set_ai_state(
                        AIState::Idle,
                        self.frame,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                }
            }
            if self
                .objects
                .get(&attacker_id)
                .is_some_and(|attacker| attacker.max_shots_to_fire == 0)
            {
                self.set_turret_target_position(attacker_id, None);
            }
        } else if self.objects.get(&attacker_id).is_some_and(|attacker| {
            attacker.can_attack()
                && attacker.can_move()
                && !(attacker.is_above_terrain()
                    && (attacker.locomotor_surfaces & crate::game_logic::object::LOCO_SURFACE_AIR)
                        != 0)
                && crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(attacker)
                && attacker.has_max_shots_remaining()
                && attacker.movement.path.is_empty()
                && !attacker.waiting_for_path
        }) {
            let planned = self.objects.get(&attacker_id).map(|attacker| {
                let slot = ground_slot.unwrap_or(0);
                let under = crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
                let (min_range, max_range) = attacker
                    .weapon_slot(slot)
                    .map(|w| {
                        (
                            (w.min_range - under).max(0.0),
                            (attacker.effective_weapon_range_for_slot_with_solo_set(
                                slot,
                                w.range,
                                &self.solo_weapon_bonus_set,
                            ) - under)
                                .max(0.0),
                        )
                    })
                    .unwrap_or((0.0, 0.0));
                let dist = attacker.distance_to_pos(target_location);
                let wname = attacker
                    .selected_weapon_slot()
                    .and_then(|s| attacker.weapon_name_for_slot(s).map(|n| n.to_owned()));
                let backup = if min_range > crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE
                    && dist + 1e-4 < min_range
                {
                    let away = attacker.get_position() - target_location;
                    let len = away.length().max(0.01);
                    let src_r = (*attacker.geometry_info()).bounding_circle_radius();
                    let stand = (min_range + max_range) * 0.5 + src_r;
                    Some(target_location + away / len * stand)
                } else {
                    None
                };
                (backup, max_range, wname)
            });
            let goal = if let Some((Some(backup), _, _)) = &planned {
                *backup
            } else {
                let (max_range, wname) = planned
                    .map(|(_, max_range, wname)| (max_range, wname))
                    .unwrap_or((0.0, None));
                self.approach_pos_for_attack(
                    attacker_id,
                    target_location,
                    max_range,
                    wname.as_deref(),
                    None,
                )
            };
            if self.assign_unit_path(attacker_id, goal, &[]) {
                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                    attacker.set_ai_state(
                        AIState::AttackingGround,
                        self.frame,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                    attacker.ignored_obstacle_id = None;
                }
                self.set_turret_target_position(attacker_id, Some(target_location));
            }
        } else if self.objects.get(&attacker_id).is_some_and(|attacker| {
            attacker.can_move()
                && attacker.has_max_shots_remaining()
                && attacker.is_above_terrain()
                && (attacker.locomotor_surfaces & crate::game_logic::object::LOCO_SURFACE_AIR) != 0
        }) {
            let planned = self.objects.get(&attacker_id).map(|attacker| {
                let slot = ground_slot.unwrap_or(0);
                let under = crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
                let (min_range, max_range) = attacker
                    .weapon_slot(slot)
                    .map(|w| {
                        (
                            (w.min_range - under).max(0.0),
                            (attacker.effective_weapon_range_for_slot_with_solo_set(
                                slot,
                                w.range,
                                &self.solo_weapon_bonus_set,
                            ) - under)
                                .max(0.0),
                        )
                    })
                    .unwrap_or((0.0, 0.0));
                let dist = attacker.distance_to_pos(target_location);
                let flight_y = attacker.get_position().y;
                let wname = attacker
                    .selected_weapon_slot()
                    .and_then(|s| attacker.weapon_name_for_slot(s).map(|n| n.to_owned()));
                let backup = if min_range > crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE
                    && dist + 1e-4 < min_range
                {
                    let away = attacker.get_position() - target_location;
                    let len = away.length().max(0.01);
                    let src_r = (*attacker.geometry_info()).bounding_circle_radius();
                    let stand = (min_range + max_range) * 0.5 + src_r;
                    Some(target_location + away / len * stand)
                } else {
                    None
                };
                (backup, flight_y, max_range, wname)
            });
            if let Some((backup, flight_y, max_range, wname)) = planned {
                let mut goal = if let Some(backup) = backup {
                    backup
                } else {
                    self.approach_pos_for_attack(
                        attacker_id,
                        target_location,
                        max_range,
                        wname.as_deref(),
                        None,
                    )
                };
                goal.y = flight_y;
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
                    attacker.set_ai_state(
                        AIState::AttackingGround,
                        self.frame,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                }
            }
        }
        shot
    }

    pub(in super::super::super) fn find_ground_attack_victim(
        &self,
        attacker_id: ObjectId,
        target_location: Vec3,
        firing_slot: u8,
    ) -> Option<ObjectId> {
        let attacker = self.objects.get(&attacker_id)?;
        let force_attack = attacker.force_attack;
        let attacker_team = attacker.team;
        const GROUND_IMPACT_RADIUS: f32 = 12.0;

        // Pure residual acquire: nearest attackable victim near ground impact (3D).
        let candidate_ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(&candidate_id, candidate)| {
                if candidate_id == attacker_id
                    || !candidate.is_alive()
                    || (!candidate.is_attackable() && !candidate.is_disarmable_mine())
                    || candidate.contained_by.is_some()
                {
                    return None;
                }
                if !force_attack && candidate.team == attacker_team {
                    return None;
                }
                Some(candidate_id)
            })
            .collect();

        let candidates: Vec<_> = candidate_ids
            .into_iter()
            .filter_map(|id| {
                let candidate = self.objects.get(&id)?;
                if attacker.selected_native_weapon_rule(firing_slot).is_some() {
                    let weapon=attacker.weapon_slot(firing_slot)?;
                    if !attacker.weapon_allows_target_anti_mask(weapon,Some(firing_slot),candidate.weapon_target_anti_mask()) { return None; }
                }
                let air =
                    candidate.is_kind_of(KindOf::Aircraft) || candidate.status.airborne_target;
                if air {
                    let can_air = attacker
                        .weapon_slot(firing_slot)
                        .map(|w| w.can_target_air)
                        .unwrap_or(false);
                    if !can_air {
                        return None;
                    }
                }
                if candidate.is_eject_invulnerable() {
                    return None;
                }
                if candidate.status.under_construction && !attacker.force_attack {
                    return None;
                }
                if candidate.status.sold {
                    return None;
                }
                if attacker.producer_id == Some(id) {
                    return None;
                }
                let mask = candidate.weapon_target_anti_mask();
                let Some(weapon) = attacker.weapon_slot(firing_slot).cloned() else {
                    return None;
                };
                if !attacker.weapon_allows_target_anti_mask(&weapon, Some(firing_slot), mask) {
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
                            && !(attacker.selected_native_weapon_rule(firing_slot).map(|rule| rule.damage_type == gamelogic::damage::DamageType::Disarm)
                            .unwrap_or_else(|| crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage(attacker.weapon_name_for_slot(firing_slot).unwrap_or(""))) && candidate.is_disarmable_mine()),
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
