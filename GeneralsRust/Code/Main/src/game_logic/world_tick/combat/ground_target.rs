//! Ground-target firing/approach and actual victim acquisition.
use super::*;

impl GameLogic {
    pub(super) fn update_ground_target_combat(
        &mut self,
        input: CombatAttackInput,
        target_location: Vec3,
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
        let rocket_pod_ground = {
            use crate::game_logic::host_comanche_rocket_pods::{
                UPGRADE_COMANCHE_ROCKET_PODS, is_comanche_template, rocket_pod_ground_fire_active,
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
                        attacker.can_fire_slot(slot, current_time)
                            && attacker.has_max_shots_remaining()
                            && attacker.weapon_allows_target_anti_mask(
                                weapon,
                                Some(slot),
                                gamelogic::weapon::WeaponAntiMask::GROUND,
                            )
                            && attacker.is_within_attack_range_pos_for_slot(slot, target_location)
                    })
                })
            })
            .unwrap_or(false);
        let can_fire_at_location = if can_fire_at_location {
            let (immobile, contained_by, spawns_weapons) = self
                .objects
                .get(&attacker_id)
                .map(|attacker| {
                    let name = attacker.template_name.to_ascii_lowercase();
                    let spawns = name.contains("spawnsaretheweapons") || name.contains("stinger");
                    (
                        attacker.is_kind_of(crate::game_logic::KindOf::Immobile),
                        attacker.contained_by,
                        spawns,
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
                if flying && attacker.ai_state != AIState::AttackMoving {
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
                attacker.set_ai_state(AIState::AttackingGround);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(attacker_id, 4); // AttackingGround
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
                return CombatShotResult::NoShot;
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
                    return CombatShotResult::NoShot;
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

            shot = CombatShotResult::Fired(ground_slot);

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
                    let _ =
                        self.queue_aurora_bomb(kind, attacker_id, attacker_team, target_location);
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
                        crate::game_logic::host_tomahawk::is_tomahawk_template(&a.template_name)
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
                    let _ = self.spawn_comanche_rocket_pod_projectile(attacker_id, from, aim, idx);
                    let (hits, _) =
                        self.apply_comanche_rocket_pod_area_at(target_location, Some(attacker_id));
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
                            .map(|a| scud_toxin_warhead_for_slot(&a.template_name, ground_slot))
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
                        .spawn_tomahawk_missile_projectile(attacker_id, from, target_location, None)
                        .is_some();
                    let (hits, _) = if spawned {
                        self.tomahawk_residual_fires =
                            self.tomahawk_residual_fires.saturating_add(1);
                        (1, false)
                    } else {
                        self.apply_tomahawk_residual_at(target_location, Some(attacker_id), None)
                    };
                    let _ = weapon_damage;
                } else if let Some(ground_target_id) =
                    self.find_ground_attack_victim(attacker_id, target_location, ground_slot)
                {
                    let ground_wname = self.objects.get(&attacker_id).and_then(|attacker| {
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
                    let context = crate::game_logic::object::DamageHitContext::new(
                        self.objects.get(&attacker_id),
                        ground_wname.as_deref(),
                        damage_type,
                    );
                    if let Some(target) = self.objects.get_mut(&ground_target_id) {
                        let destroyed = target.take_damage_with_context(
                            weapon_damage,
                            Some(attacker_id),
                            damage_type,
                            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                                damage_type,
                            ),
                            None,
                            self.frame,
                            &context,
                        );
                        if destroyed {
                            self.mark_object_for_destruction(ground_target_id, Some(attacker_team));
                            self.award_score_the_kill_experience(attacker_id, ground_target_id);
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
            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                let slot = ground_slot;
                let name = attacker.weapon_name_for_slot(slot).map(str::to_owned);
                let auto_reloaded = if let Some(w) = attacker.weapon_slot_mut(slot) {
                    Object::consume_ammo_on_fire_named(w, current_time, name.as_deref());
                    Object::auto_reloaded_clip_after_firing(w, name.as_deref())
                } else {
                    false
                };
                if auto_reloaded
                    && attacker.weapon_lock_type == WeaponLockType::LockedTemporarily
                    && attacker.weapon_lock_slot == slot
                {
                    attacker.release_weapon_lock(WeaponLockType::LockedTemporarily);
                }
                if attacker.stealth_breaks_on_attack && attacker.status.stealthed {
                    attacker.break_stealth();
                }
                attacker.consume_max_shot_count();
                if attacker.max_shots_to_fire == 0 {
                    attacker.target_location = None;
                    attacker.set_force_attack(false);
                    attacker.set_ai_state(AIState::Idle);
                    crate::game_logic::host_attack_log::record(attacker_id, None);
                }
            }
            if self
                .objects
                .get(&attacker_id)
                .is_some_and(|attacker| attacker.max_shots_to_fire == 0)
            {
                self.set_turret_target_position(attacker_id, None);
            }
            let _ = self.record_accepted_weapon_discharge(attacker_id, ground_slot);
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
                            (attacker.effective_weapon_range(w.range) - under).max(0.0),
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
                    let src_r = attacker
                        .thing()
                        .template
                        .geometry_info
                        .bounding_circle_radius();
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
                    attacker.set_ai_state(AIState::AttackingGround);
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
                            (attacker.effective_weapon_range(w.range) - under).max(0.0),
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
                    let src_r = attacker
                        .thing()
                        .template
                        .geometry_info
                        .bounding_circle_radius();
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
                    attacker.set_ai_state(AIState::AttackingGround);
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
            .iter()
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
                            && !(crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage(
                                attacker.weapon_name_for_slot(firing_slot).unwrap_or(""),
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
