//! C++ support-state dispatcher and per-object update loop.
use super::super::super::*;
use super::guard_states::{
    host_guard_xy_dist_sq, host_guardee_moved_beyond_return_threshold, GUARD_CHASE_PHASE_INNER,
    GUARD_RETURN_CLOSE_SQ,
};
use super::special_abilities::{clear_raising_flag_model, LeftoverSaTick};

impl GameLogic {
    pub(in super::super::super) fn update_support_states(
        &mut self,
        object_ids: &[ObjectId],
        dt: f32,
    ) {
        self.update_leftover_laser_guided_channels(dt);
        self.expire_leftover_disable_fx();
        // C++ OpenContain::update zeros m_playerEnteredMask every logic frame
        // after scripts have already sampled last frame's enter pulse.
        self.clear_open_contain_player_who_entered();
        self.update_open_contain_exit_doors();

        const GUARD_MIN_RADIUS: f32 = 80.0;
        const INTERACT_RANGE: f32 = crate::game_logic::host_repair::HOST_REPAIR_INTERACT_RANGE;
        const SPECIAL_ABILITY_RANGE_PADDING: f32 = 4.0;
        // Authored capture durations are integral milliseconds, but the host
        // channel stores the running remainder as `f32` seconds.  A sequence
        // such as 20.0 - 19.9 - 0.1 can otherwise leave one floating-point
        // ulp and defer the C++ frame-boundary trigger by another logic tick.
        // This is far below one authored millisecond (and one 30 Hz logic
        // frame), so it only removes representation residue—not gameplay
        // time.
        const CAPTURE_CHANNEL_COMPLETE_EPSILON: f32 = 0.000_1;
        const HEAL_RATE: f32 = crate::game_logic::host_repair::HOST_HEAL_RATE_HP_PER_SEC;

        for &object_id in object_ids {
            let snapshot = match self.objects.get(&object_id) {
                Some(obj) => (
                    obj.ai_state.clone(),
                    obj.team,
                    obj.owner_player_id,
                    obj.get_position(),
                    obj.target,
                    obj.guard_position,
                    obj.guard_target,
                    obj.guard_radius,
                    obj.guard_mode,
                    obj.can_move(),
                    obj.can_attack(),
                    obj.health.current,
                    obj.health.maximum,
                    obj.selection_radius,
                    obj.is_alive(),
                ),
                None => continue,
            };

            let (
                ai_state,
                team,
                owner_player_id,
                position,
                target_id,
                guard_position,
                guard_target,
                guard_radius,
                guard_mode,
                can_move,
                can_attack,
                health_current,
                health_maximum,
                selection_radius,
                is_alive,
            ) = snapshot;

            if !is_alive {
                continue;
            }
            self.expire_temporary_stealth_grant(object_id);

            if ai_state != AIState::SpecialAbility {
                let leftover_laser_persist = self
                    .hero_abilities
                    .leftover_channel(object_id)
                    .is_some_and(|ch| {
                        ch.kind
                            == crate::game_logic::host_hero_abilities::LeftoverSaKind::LaserGuided
                            && ai_state == AIState::Attacking
                            && self
                                .objects
                                .get(&object_id)
                                .is_some_and(|o| o.target == Some(ch.target_id))
                    });
                if leftover_laser_persist {
                    // C++ triggerAbilityEffect aiAttackObject(..., CMD_FROM_AI)
                    // must not onExit the PersistentPrepTime channel.
                } else {
                    self.pending_special_abilities.remove(&object_id);
                    // An explicit replacement order must cancel an in-flight HDB
                    // channel without overwriting that new order's target/state.
                    // The normal packed completion path below remains responsible
                    // for putting a completed channel back to Idle.
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        if object.hacker_disable_channel.is_some() {
                            object.hacker_disable_channel = None;
                            object.set_status_using_ability(false);
                        }
                    }
                    self.abort_leftover_sa_channel_on_new_order(object_id);
                }
            }
            // C++ SpecialAbilityUpdate::update: any non-AI command source
            // immediately onExit. Leftover capture must not keep
            // IS_USING_ABILITY / capture_channel after a player move.
            if ai_state != AIState::Capturing {
                let has_capture = self
                    .objects
                    .get(&object_id)
                    .is_some_and(|o| o.capture_channel.is_some());
                if has_capture {
                    self.abort_capture_channel_on_new_order(object_id);
                }
            }

            match ai_state {
                AIState::GuardingArea => {
                    let anchor = guard_position.unwrap_or(position);
                    let (std_inner, std_outer) = self.host_std_guard_ranges(object_id);
                    let mood = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.ai_attitude)
                        .unwrap_or(0);
                    // C++ Sleep mood → getStdGuardRange 0. Do not fall back to 80.
                    let inner = if mood <= -2 {
                        0.0
                    } else if std_inner > 0.0 {
                        std_inner
                    } else if guard_radius > 0.0 {
                        guard_radius
                    } else {
                        GUARD_MIN_RADIUS
                    };
                    let _outer = if std_outer > 0.0 {
                        std_outer
                    } else {
                        inner * 1.5
                    };
                    let flying_only =
                        matches!(guard_mode, crate::game_logic::GuardMode::FlyingUnitsOnly);
                    let polygon_name = self
                        .objects
                        .get(&object_id)
                        .and_then(|o| o.guard_area_trigger.clone());
                    let polygon = polygon_name
                        .as_deref()
                        .filter(|n| !n.is_empty())
                        .and_then(Self::host_named_guard_area_polygon);
                    // C++ lookForInnerTarget: inner ring, or polygon bounding radius + point-in-trigger.
                    let (scan_anchor, acquire_radius) = if let Some((c, r, _)) = polygon.as_ref() {
                        (*c, if *r > 0.0 { *r } else { inner })
                    } else {
                        (anchor, inner)
                    };
                    let enter_guard = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.thing.template.enter_guard)
                        .unwrap_or(false);
                    let hijack_guard = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.thing.template.hijack_guard)
                        .unwrap_or(false);
                    let picking_crate = self
                        .objects
                        .get(&object_id)
                        .and_then(|o| o.requested_victim_id)
                        .is_some();

                    if can_attack && self.try_guard_last_attacker(object_id, team) {
                        continue;
                    }

                    let returning =
                        inner > 0.0 && host_guard_xy_dist_sq(position, anchor) > inner * inner;
                    if self.guard_acquire_scan_due(object_id, returning) && can_attack {
                        if let Some(team_id) = self.host_team_common_target(object_id) {
                            if self.engage_guard_target(object_id, team_id, false) {
                                continue;
                            }
                        }
                        if let Some(enemy_id) = self.scan_guard_inner_target(
                            object_id,
                            team,
                            scan_anchor,
                            acquire_radius,
                            flying_only,
                            enter_guard,
                            hijack_guard,
                            polygon.as_ref().map(|(_, _, t)| t),
                        ) {
                            self.set_host_team_common_target(object_id, Some(enemy_id));
                            if enter_guard {
                                if self.try_guard_enter_or_hijack(
                                    object_id,
                                    enemy_id,
                                    hijack_guard,
                                    team,
                                ) {
                                    continue;
                                }
                            } else if self.engage_guard_target(object_id, enemy_id, false) {
                                continue;
                            }
                        }
                    }

                    let return_goal = polygon.as_ref().map(|(c, _, _)| *c).unwrap_or(anchor);
                    if can_move
                        && !picking_crate
                        && host_guard_xy_dist_sq(position, return_goal) > GUARD_RETURN_CLOSE_SQ
                    {
                        self.path_approach_with_state(
                            object_id,
                            return_goal,
                            AIState::GuardingArea,
                        );
                    }
                }
                AIState::GuardingObject => {
                    let guard_target_id = match guard_target {
                        Some(id) => id,
                        None => {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    };

                    let Some(guard_anchor) = self
                        .objects
                        .get(&guard_target_id)
                        .filter(|o| o.is_alive())
                        .map(|o| o.get_position())
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_guard_target(None);
                        }
                        self.clear_target_decision_aware(object_id);
                        continue;
                    };

                    let (std_inner, _) = self.host_std_guard_ranges(object_id);
                    let mood = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.ai_attitude)
                        .unwrap_or(0);
                    let inner = if mood <= -2 {
                        0.0
                    } else if std_inner > 0.0 {
                        std_inner
                    } else if guard_radius > 0.0 {
                        guard_radius
                    } else {
                        GUARD_MIN_RADIUS
                    };
                    let flying_only =
                        matches!(guard_mode, crate::game_logic::GuardMode::FlyingUnitsOnly);
                    // C++ lookForInnerTarget always uses getStdGuardRange (inner).
                    let acquire_radius = inner;
                    let picking_crate = self
                        .objects
                        .get(&object_id)
                        .and_then(|o| o.requested_victim_id)
                        .is_some();
                    let enter_guard = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.thing.template.enter_guard)
                        .unwrap_or(false);
                    let hijack_guard = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.thing.template.hijack_guard)
                        .unwrap_or(false);

                    if can_attack && self.try_guard_last_attacker(object_id, team) {
                        continue;
                    }
                    let returning = inner > 0.0
                        && host_guard_xy_dist_sq(position, guard_anchor) > inner * inner;
                    if self.guard_acquire_scan_due(object_id, returning) && can_attack {
                        if let Some(team_id) = self.host_team_common_target(object_id) {
                            if self.engage_guard_target(object_id, team_id, false) {
                                continue;
                            }
                        }
                        if enter_guard {
                            if let Some(enemy_id) = self.scan_guard_inner_target(
                                object_id,
                                team,
                                guard_anchor,
                                acquire_radius,
                                flying_only,
                                true,
                                hijack_guard,
                                None,
                            ) {
                                if self.try_guard_enter_or_hijack(
                                    object_id,
                                    enemy_id,
                                    hijack_guard,
                                    team,
                                ) {
                                    continue;
                                }
                            }
                        } else {
                            let tunnel_nemesis = {
                                let guard_is_tunnel = self.objects.get(&guard_target_id).is_some_and(
                                    |g| {
                                        g.is_tunnel_network_style_container()
                                            || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                                                &g.template_name,
                                            )
                                    },
                                );
                                if guard_is_tunnel {
                                    let key = self
                                        .objects
                                        .get(&guard_target_id)
                                        .map(|g| g.tunnel_system_key());
                                    key.and_then(|k| self.resolved_tunnel_nemesis(k))
                                } else {
                                    None
                                }
                            };
                            if let Some(enemy_id) = tunnel_nemesis {
                                if self.engage_guard_target(object_id, enemy_id, false) {
                                    continue;
                                }
                            }
                            if let Some(enemy_id) = self.scan_guard_inner_target(
                                object_id,
                                team,
                                guard_anchor,
                                acquire_radius,
                                flying_only,
                                false,
                                false,
                                None,
                            ) {
                                if self.engage_guard_target(object_id, enemy_id, false) {
                                    continue;
                                }
                            }
                        }
                    }

                    if !self.guard_guardee_pos.contains_key(&object_id) {
                        self.guard_guardee_pos.insert(object_id, guard_anchor);
                    }
                    let drifted = self.guard_guardee_pos.get(&object_id).is_some_and(|prev| {
                        host_guardee_moved_beyond_return_threshold(*prev, guard_anchor)
                    });
                    if drifted {
                        self.guard_guardee_pos.insert(object_id, guard_anchor);
                        if can_move && !picking_crate {
                            self.path_approach_with_state(
                                object_id,
                                guard_anchor,
                                AIState::GuardingObject,
                            );
                        }
                    } else if can_move
                        && !picking_crate
                        && host_guard_xy_dist_sq(position, guard_anchor) > GUARD_RETURN_CLOSE_SQ
                    {
                        self.path_approach_with_state(
                            object_id,
                            guard_anchor,
                            AIState::GuardingObject,
                        );
                    }
                }
                AIState::Repairing => {
                    let Some(repair_target_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let actor_can_repair = self
                        .objects
                        .get(&object_id)
                        .map(|obj| obj.can_repair() && obj.contained_by.is_none())
                        .unwrap_or(false);
                    if !actor_can_repair {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    let Some((
                        repair_target_pos,
                        repair_target_selection_radius,
                        repair_target_alive,
                        repair_target_is_structure,
                        repair_target_under_construction,
                        repair_target_name,
                        repair_target_rubble,
                    )) = self.objects.get(&repair_target_id).map(|target| {
                        let name = target.template_name.clone();
                        let is_bridge =
                            crate::game_logic::host_bridge_behavior::is_bridge_or_tower_template(
                                &name,
                            ) || target.is_kind_of(KindOf::Bridge)
                                || target.is_kind_of(KindOf::BridgeTower);
                        let rubble = is_bridge
                            && (target.status.keep_as_rubble
                                || target.status.effectively_dead
                                || target.body_damage_state
                                    == crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                                || target.health.current <= 0.0);
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Structure) || is_bridge,
                            target.status.under_construction,
                            name,
                            rubble,
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if (!repair_target_alive && !repair_target_rubble)
                        || !repair_target_is_structure
                        || repair_target_under_construction
                        || !self.repair_relationship_is_not_enemy(object_id, repair_target_id)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let interact = crate::game_logic::host_repair::repair_action_range(
                        repair_target_selection_radius,
                    );
                    if position.distance(repair_target_pos) > interact {
                        // Do not replace a live A* route every support tick.
                        // Re-path only if its endpoint is no longer a viable
                        // interaction point, or the mover has stopped; that
                        // preserves obstacle recovery without restarting the
                        // route before movement can consume its next node.
                        let has_valid_active_approach_path =
                            self.objects.get(&object_id).is_some_and(|obj| {
                                obj.status.moving
                                    && obj.movement.current_path_index < obj.movement.path.len()
                                    && obj.movement.path.last().is_some_and(|endpoint| {
                                        endpoint.distance(repair_target_pos) <= interact
                                    })
                            });
                        if can_move && !has_valid_active_approach_path {
                            let airborne = self.objects.get(&object_id).is_some_and(|o| {
                                o.is_kind_of(KindOf::Aircraft) || o.status.airborne_target
                            });
                            let approach = self.find_good_build_or_repair_position(
                                position,
                                repair_target_pos,
                                repair_target_selection_radius,
                                airborne,
                                airborne.then_some(repair_target_id),
                                Some(object_id),
                            );
                            self.path_approach_with_state(object_id, approach, AIState::Repairing);
                        }
                        // Never heal remotely. This also keeps a valid route
                        // in flight instead of falling through to the repair
                        // effect while still out of range.
                        continue;
                    }

                    // C++ DozerAIUpdate.cpp:665-688 createBridgeScaffolding + canHeal.
                    if crate::game_logic::host_bridge_behavior::is_bridge_or_tower_template(
                        &repair_target_name,
                    ) {
                        let span_id = self.resolve_bridge_span_for_repair(repair_target_id);
                        if let Some(sid) = span_id {
                            if !self.bridge_behavior.is_scaffold_present(sid) {
                                self.spawn_bridge_scaffolding(sid);
                            }
                            if self.bridge_behavior.is_scaffold_in_motion(sid) {
                                continue;
                            }
                        }
                    }

                    // Dozer structure-repair residual: heal HP over time while in range.
                    // C++ DozerAIUpdate.cpp:694-699 percent heal, no 8.75 HP/s floor.
                    // C++ DozerAIUpdate.cpp:670: ACTIVELY_CONSTRUCTING only at the dock.
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_actively_constructing(true);
                    }
                    let max_hp = self
                        .objects
                        .get(&repair_target_id)
                        .map(|t| t.health.maximum)
                        .unwrap_or(0.0);
                    let heal_per_sec =
                        crate::game_logic::host_repair::dozer_repair_hp_per_sec(max_hp);
                    let heal_amount = heal_per_sec * dt;
                    // C++ attemptHealingFromSoleBenefactor(health, dozer, 2) residual.
                    let now = self.frame;
                    let sole = if let Some(target) = self.objects.get_mut(&repair_target_id) {
                        if repair_target_rubble {
                            target.revive_from_bridge_rubble();
                        }
                        let max_before = target.health.maximum.max(1.0);
                        let healed = target.attempt_healing_from_sole_benefactor(
                            heal_amount,
                            object_id,
                            2,
                            now,
                        );
                        if healed && heal_amount > 0.0 {
                            crate::game_logic::host_bridge_behavior::record_mirror(
                                repair_target_id,
                                heal_amount,
                                max_before,
                                Some(object_id),
                                crate::game_logic::combat::DamageType::Healing.to_store() as u32,
                                0,
                                crate::game_logic::host_bridge_behavior::HostBridgeMirrorKind::Heal,
                            );
                        }
                        let full = target.health.current >= target.health.maximum - 0.01;
                        let pos = target.get_position();
                        Some((full, healed, pos))
                    } else {
                        None
                    };
                    let (target_full, healed, repair_pos) = match sole {
                        Some(v) => v,
                        None => {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                if crate::gameworld_shadow::gameworld_ai_decision_authority_enabled(
                                ) {
                                    crate::game_logic::host_ai_decision_log::record_stop_attack(
                                        object_id,
                                    );
                                    crate::game_logic::host_ai_decision_log::record_set_state(
                                        object_id, 0,
                                    );
                                } else {
                                    obj.set_target(None);
                                    obj.set_ai_state(AIState::Idle);
                                }
                                obj.set_actively_constructing(false);
                            }
                            self.dozer_internal_task_complete(object_id, true);
                            let _ = self.dozer_idle_resume_pending_build(object_id);
                            continue;
                        }
                    };
                    if !healed && !target_full {
                        // Another dozer owns sole-benefactor claim — cancel this dozer task.
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_stop_attack(
                                    object_id,
                                );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, 0,
                                );
                            }
                            obj.set_actively_constructing(false);
                        }
                        self.dozer_internal_task_complete(object_id, true);
                        let _ = self.dozer_idle_resume_pending_build(object_id);
                        self.sole_benefactor_repair_rejects =
                            self.sole_benefactor_repair_rejects.saturating_add(1);
                        continue;
                    }
                    if healed {
                        self.record_structure_repair_residual_heal();
                    }
                    if target_full {
                        // C++ WorkerAIUpdate.cpp:830 removeBridgeScaffolding on repair complete.
                        if crate::game_logic::host_bridge_behavior::is_bridge_or_tower_template(
                            &repair_target_name,
                        ) {
                            if let Some(sid) = self.resolve_bridge_span_for_repair(repair_target_id)
                            {
                                self.remove_bridge_scaffolding(sid);
                            }
                        }
                        // C++ DOZER:RepairComplete residual.
                        let msg = localization::localize("DOZER:RepairComplete", "Repair complete");
                        self.queue_radar_message_at(
                            msg,
                            repair_pos,
                            radar_notifications::RadarKind::Generic,
                        );
                        self.repair_complete_events = self.repair_complete_events.saturating_add(1);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_stop_attack(
                                    object_id,
                                );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, 0,
                                );
                            }
                            obj.set_actively_constructing(false);
                        }
                        self.dozer_internal_task_complete(object_id, true);
                        let _ = self.dozer_idle_resume_pending_build(object_id);
                    }
                }
                state @ (AIState::SeekingRepair | AIState::SeekingHealing) => {
                    if health_current >= health_maximum - 0.01 {
                        if let Some(tid) = target_id {
                            if matches!(state, AIState::SeekingRepair) {
                                self.send_to_rally_after_repair_dock(object_id, tid);
                            }
                            self.release_dock_if_holder(tid, object_id);
                        }
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                    }

                    let Some(support_target_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let Some((
                        support_target_pos,
                        support_target_selection_radius,
                        support_target_alive,
                        support_target_under_construction,
                        support_target_sold,
                        support_target_contained,
                        support_target_is_repair_pad,
                        support_target_is_heal_pad,
                        support_target_is_airfield,
                    )) = self.objects.get(&support_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.is_alive(),
                            target.status.under_construction,
                            target.status.sold,
                            target.contained_by.is_some(),
                            target.is_kind_of(KindOf::RepairPad),
                            target.is_kind_of(KindOf::HealPad),
                            target.is_kind_of(KindOf::FSAirfield),
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !support_target_alive
                        || support_target_under_construction
                        || support_target_sold
                        || support_target_contained
                        // C++ `canGetRepairedAt` / `canGetHealedAt` uses the
                        // controlling players' relationship, not a faction
                        // comparison. Repeat that authority check after the
                        // order has begun: capture, diplomacy, or a stale
                        // owner record cannot keep servicing an enemy unit.
                        || !self.service_relationship_is_allies(object_id, support_target_id)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    let source_can_use_support = self
                        .objects
                        .get(&object_id)
                        .map(|obj| {
                            if obj.contained_by.is_some() {
                                return false;
                            }
                            let is_aircraft = obj.is_kind_of(KindOf::Aircraft);
                            let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                            // C++ ActionManager::canGetRepairedAt accepts an
                            // aircraft only while it is above terrain.  Keep
                            // this mutable-state revalidation identical to
                            // command acceptance so landing cannot turn a
                            // pre-existing service order into a free repair.
                            let is_above_terrain = obj.status.airborne_target
                                || (obj.ground_height_from_terrain
                                    && obj.get_position().y > obj.ground_height + 0.01);
                            match state {
                                AIState::SeekingRepair => {
                                    is_vehicle
                                        && if is_aircraft {
                                            support_target_is_airfield && is_above_terrain
                                        } else {
                                            support_target_is_repair_pad
                                        }
                                }
                                AIState::SeekingHealing => {
                                    obj.is_kind_of(KindOf::Infantry) && support_target_is_heal_pad
                                }
                                _ => false,
                            }
                        })
                        .unwrap_or(false);
                    if !source_can_use_support {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    if position.distance(support_target_pos) > INTERACT_RANGE {
                        // Keep a moving, target-valid A* path rather than
                        // restarting it every frame. Re-path only after the
                        // endpoint ceased to be a viable interaction point or
                        // the mover stopped, retaining obstacle recovery.
                        let has_valid_active_approach_path =
                            self.objects.get(&object_id).is_some_and(|obj| {
                                obj.status.moving
                                    && obj.movement.current_path_index < obj.movement.path.len()
                                    && obj.movement.path.last().is_some_and(|endpoint| {
                                        endpoint.distance(support_target_pos) <= INTERACT_RANGE
                                    })
                            });
                        if can_move && !has_valid_active_approach_path {
                            let approach =
                                crate::game_logic::host_repair::support_approach_position(
                                    position,
                                    support_target_pos,
                                    support_target_selection_radius,
                                );
                            self.path_approach_with_state(object_id, approach, state.clone());
                        }
                        // An out-of-range source is never permitted to apply
                        // the repair/heal effect, including after a failed
                        // route allocation.
                        if matches!(state, AIState::SeekingRepair) {
                            self.release_dock_if_holder(support_target_id, object_id);
                        }

                        continue;
                    }

                    // Airfields have no RepairDockUpdate. ParkingPlaceBehavior
                    // heals after landing; never TimeForFullHeal while airborne.
                    let seeking_aircraft = matches!(state, AIState::SeekingRepair)
                        && (support_target_is_airfield
                            || self
                                .objects
                                .get(&object_id)
                                .is_some_and(|o| o.is_kind_of(KindOf::Aircraft)));
                    if seeking_aircraft {
                        self.try_aircraft_land_for_repair(object_id, support_target_id);
                        continue;
                    }

                    // Pad/war-factory: C++ RepairDockUpdate::action
                    // TimeForFullHeal. One activeDocker; rate computed once
                    // from missing HP so Humvee ≠ Overlord.
                    if matches!(state, AIState::SeekingRepair)
                        && !self.try_claim_dock(support_target_id, object_id)
                    {
                        continue;
                    }
                    let mut vehicle_healed = false;
                    let mut heal_pad_healed = false;
                    let repair_rate = if matches!(state, AIState::SeekingRepair) {
                        self.repair_dock_rate_for_docker(
                            support_target_id,
                            object_id,
                            health_maximum,
                            health_current,
                        )
                    } else {
                        0.0
                    };
                    let seeking_repair = matches!(state, AIState::SeekingRepair);
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        let rate = match state {
                            AIState::SeekingRepair => repair_rate,
                            AIState::SeekingHealing => HEAL_RATE,
                            _ => 0.0,
                        };
                        let before = obj.health.current;
                        obj.heal(rate * dt);
                        let healed = obj.health.current > before + 0.0001;
                        if healed && matches!(state, AIState::SeekingRepair) {
                            vehicle_healed = true;
                        }
                        if healed && matches!(state, AIState::SeekingHealing) {
                            heal_pad_healed = true;
                        }
                        if obj.health.current >= obj.health.maximum - 0.01 {
                            obj.set_target(None);
                        } else if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            let ordinal =
                                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                                    &state,
                                );
                            crate::game_logic::host_ai_decision_log::record_set_state(
                                object_id, ordinal,
                            );
                            obj.set_ai_state(state);
                        } else {
                            obj.set_ai_state(state);
                        }
                    }
                    if seeking_repair {
                        self.heal_slave_drone_with_repair_dock(object_id);
                    }
                    let fully_repaired = self
                        .objects
                        .get(&object_id)
                        .is_some_and(|o| o.health.current >= o.health.maximum - 0.01);
                    if seeking_repair && fully_repaired {
                        self.send_to_rally_after_repair_dock(object_id, support_target_id);
                        self.release_dock_if_holder(support_target_id, object_id);
                    }

                    if vehicle_healed {
                        self.record_vehicle_repair_residual_heal();
                    }
                    if heal_pad_healed {
                        self.record_heal_pad_residual_heal();
                    }
                }
                state @ (AIState::Entering | AIState::Docking) => {
                    let Some(container_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    };

                    // USA Pilot residual: Enter unmanned vehicle → recrew (not transport load).
                    // Retail VeterancyCrateCollide IsPilot path residual.  The
                    // same parsed authority predicate is repeated here at
                    // arrival so an Enter accepted before an owner/flight
                    // transition cannot re-crew a changed target.
                    {
                        let pilot_snapshot = self.objects.get(&object_id).map(|o| {
                            (
                                o.team,
                                o.owner_player_id,
                                o.experience.level,
                                o.get_position(),
                                o.selection_radius,
                                o.can_move(),
                            )
                        });
                        let vehicle_snapshot = self
                            .objects
                            .get(&container_id)
                            .map(|v| (v.get_position(), v.selection_radius));
                        if let (
                            Some((
                                pilot_team,
                                pilot_owner_player_id,
                                pilot_level,
                                pilot_pos,
                                pilot_radius,
                                pilot_can_move,
                            )),
                            Some((vehicle_pos, vehicle_radius)),
                        ) = (pilot_snapshot, vehicle_snapshot)
                        {
                            if self.can_execute_pilot_recrew(object_id, container_id) {
                                let enter_range = pilot_radius + vehicle_radius + 4.0;
                                if pilot_can_move && pilot_pos.distance(vehicle_pos) > enter_range {
                                    self.path_approach_with_state(
                                        object_id,
                                        vehicle_pos,
                                        AIState::Entering,
                                    );
                                    continue;
                                }
                                let transferred = self
                                    .objects
                                    .get_mut(&container_id)
                                    .map(|v| {
                                        v.apply_pilot_recrew(
                                            pilot_team,
                                            pilot_owner_player_id,
                                            pilot_level,
                                        )
                                    })
                                    .unwrap_or(false);
                                self.usa_pilot.record_recrew(transferred);
                                self.queue_audio_event(
                                    AudioEventRequest::new(
                                        crate::game_logic::host_usa_pilot::PILOT_RECREW_AUDIO,
                                    )
                                    .with_object(container_id)
                                    .with_position(vehicle_pos)
                                    .with_priority(170),
                                );
                                let msg =
                                    localization::localize("hud.pilot.recrew", "Vehicle recrewed");
                                self.queue_radar_message_for_team(pilot_team, msg);
                                self.mark_destroyed_authority_aware(object_id, None);
                                self.mark_object_for_destruction(object_id, Some(pilot_team));
                                continue;
                            }
                        }
                    }

                    // C++ canEnterObject unmanned: any !REJECT_UNMANNED infantry
                    // steals the husk (clear UNMANNED, defect to infantry team,
                    // destroy infantry). Not the USA Pilot veterancy-crate path.
                    if self.can_execute_infantry_unmanned_recrew(object_id, container_id) {
                        let infantry_snapshot = self.objects.get(&object_id).map(|o| {
                            (
                                o.team,
                                o.owner_player_id,
                                o.get_position(),
                                o.selection_radius,
                                o.can_move(),
                            )
                        });
                        let vehicle_snapshot = self
                            .objects
                            .get(&container_id)
                            .map(|v| (v.get_position(), v.selection_radius));
                        if let (
                            Some((inf_team, inf_owner, inf_pos, inf_radius, inf_can_move)),
                            Some((vehicle_pos, vehicle_radius)),
                        ) = (infantry_snapshot, vehicle_snapshot)
                        {
                            let enter_range = inf_radius + vehicle_radius + 4.0;
                            if inf_can_move && inf_pos.distance(vehicle_pos) > enter_range {
                                self.path_approach_with_state(
                                    object_id,
                                    vehicle_pos,
                                    AIState::Entering,
                                );
                                continue;
                            }
                            if let Some(veh) = self.objects.get_mut(&container_id) {
                                if veh.status.disabled_unmanned {
                                    veh.set_status_disabled_unmanned(false);
                                    veh.status.unmanned_owner_team = None;
                                    veh.status.unmanned_owner_player_id = None;
                                    veh.set_status_disabled_hacked(false);
                                    veh.status.disabled_hacked_until_frame = 0;
                                    veh.stop_moving();
                                    veh.target = None;
                                    veh.set_ai_state(AIState::Idle);
                                    veh.set_team_and_owner(inf_team, inf_owner);
                                    veh.set_private_captured(true);
                                }
                            }
                            let _ = self.transfer_script_object_name(object_id, container_id);
                            self.unmanned_reclaims = self.unmanned_reclaims.saturating_add(1);
                            self.mark_destroyed_authority_aware(object_id, None);
                            self.mark_object_for_destruction(object_id, Some(inf_team));
                            continue;
                        }
                    }

                    // Normal `MSG_ENTER` was already accepted by the command
                    // executor, but the target can change while the unit walks
                    // toward it.  Revalidate through the same centralized
                    // ContainModule/owner/capacity authority at arrival.  Dock
                    // deliberately stays on its separate state machine below.
                    let normal_enter = state == AIState::Entering;
                    // C++ OpenContain::onCollide ejects foreign riders first,
                    // then isValidContainerFor + addToContain.
                    if normal_enter {
                        self.eject_foreign_occupants_on_enter(container_id, object_id);
                    }
                    if normal_enter && !self.can_unit_enter_normal_target(object_id, container_id) {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }
                    let normal_enter_has_space = normal_enter
                        && self
                            .normal_enter_available_capacity_for(object_id, container_id)
                            .is_some_and(|available| available > 0);

                    let Some((
                        container_pos,
                        container_radius,
                        container_team,
                        container_owner_player_id,
                        container_is_structure,
                        container_is_faction_structure,
                        container_is_overlord_bunker,
                        container_is_battle_bus,
                        container_is_technical,
                        container_is_combat_cycle,
                        container_is_combat_chinook,
                        container_is_listening_outpost,
                        container_is_troop_crawler,
                        container_is_tunnel_network,
                        container_is_cave,
                        container_is_alive,
                        container_under_construction,
                        container_can_contain,
                        container_has_space,
                        container_has_unit,
                        container_occupant_count,
                    )) = self.objects.get(&container_id).map(|container| {
                        (
                            container.get_position(),
                            container.selection_radius,
                            container.team,
                            container.owner_player_id,
                            container.is_kind_of(KindOf::Structure),
                            container.is_faction_structure(),
                            container.is_overlord_style_container()
                                && container.overlord_bunker_slot_capacity() > 0,
                            container.is_battle_bus_style_container(),
                            container.is_technical_style_container(),
                            container.is_combat_cycle_style_container(),
                            container.is_combat_chinook_style_container(),
                            container.is_listening_outpost_style_container(),
                            container.is_troop_crawler_style_container(),
                            container.is_tunnel_network_style_container(),
                            container.is_cave_style_container(),
                            container.is_alive(),
                            container.status.under_construction,
                            container.can_contain(),
                            if normal_enter {
                                normal_enter_has_space
                            } else {
                                container.has_capacity_for(1)
                            },
                            container.contained_units().contains(&object_id),
                            container.contained_units().len(),
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    };

                    // Dock's legacy per-role restrictions stay isolated from
                    // normal Enter.  Normal Enter was just checked by the
                    // typed `ContainModule` authority above (including
                    // RiderChange fail-closed), so it must not fall back to a
                    // host-specialized name/flag rule here.
                    let unit_can_garrison_structure = self
                        .objects
                        .get(&object_id)
                        .map(|o| {
                            (o.is_kind_of(KindOf::Infantry) || o.is_hero())
                                && !o.is_kind_of(KindOf::NoGarrison)
                        })
                        .unwrap_or(false);
                    let unit_is_aircraft = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.is_kind_of(KindOf::Aircraft))
                        .unwrap_or(false);
                    let unit_is_huge_vehicle = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.is_kind_of(KindOf::HugeVehicle))
                        .unwrap_or(false);
                    if !normal_enter && (container_is_tunnel_network || container_is_cave) {
                        // TunnelContain residual: reject aircraft only.
                        if unit_is_aircraft {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                            continue;
                        }
                    } else if !normal_enter
                        && (container_is_structure
                            || container_is_overlord_bunker
                            || container_is_battle_bus
                            || container_is_technical
                            || container_is_combat_cycle
                            || container_is_listening_outpost
                            || container_is_troop_crawler)
                        && !unit_can_garrison_structure
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }
                    // Combat Chinook ForbidInsideKindOf = AIRCRAFT HUGE_VEHICLE residual.
                    if !normal_enter
                        && container_is_combat_chinook
                        && (unit_is_aircraft || unit_is_huge_vehicle)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Tunnel network residual: units already in the shared pool may
                    // transfer to another allied tunnel without walking (can_move false).
                    let already_in_tunnel_network = (container_is_tunnel_network
                        && self.tunnel_network.player_holding_unit(object_id).is_some())
                        || (container_is_cave
                            && self.cave_system.index_holding_unit(object_id).is_some());

                    if (!can_move && !already_in_tunnel_network)
                        || !container_is_alive
                        || container_under_construction
                        || !container_can_contain
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if !normal_enter
                        && container_is_tunnel_network
                        && matches!(
                            (owner_player_id, container_owner_player_id),
                            (Some(a), Some(b)) if a != b
                        )
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }
                    if !normal_enter
                        && container_team != team
                        && container_team != Team::Neutral
                        && (container_is_faction_structure
                            || self
                                .objects
                                .get(&container_id)
                                .is_some_and(|c| self.stealth_garrison_occupant_counts(c).1 > 0))
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let enter_range = selection_radius + container_radius + 4.0;
                    // Cross-tunnel residual transfer: skip walk when already in pool.
                    if !already_in_tunnel_network
                        && can_move
                        && position.distance(container_pos) > enter_range
                    {
                        self.path_approach_with_state(object_id, container_pos, state);
                        continue;
                    }

                    // RiderChangeContain is an atomic replacement, never a
                    // generic one-slot `add_occupant`.  Keep every parsed
                    // RiderChange target on this authoritative branch so an
                    // unsupported/custom roster cannot fall through to the
                    // legacy Combat Cycle template-name refresh below.
                    let is_rider_change_target = normal_enter
                        && self.objects.get(&container_id).is_some_and(|container| {
                            container.thing.template.contain_module.kind
                                == crate::game_logic::ContainModuleKind::RiderChange
                        });
                    if is_rider_change_target {
                        if !self.rider_change_enter_at_arrival(object_id, container_id) {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        continue;
                    }

                    // Tunnel shared capacity (MaxTunnelCapacity=10) overrides local space.
                    let tunnel_player = crate::game_logic::host_tunnel_network::tunnel_system_key(
                        container_owner_player_id,
                        container_team,
                    );
                    let tunnel_has_space = if container_is_tunnel_network {
                        self.tunnel_network.is_in_network(tunnel_player, object_id)
                            || self.tunnel_network.has_capacity(tunnel_player)
                    } else if container_is_cave {
                        let idx = self
                            .objects
                            .get(&container_id)
                            .map(|c| c.cave_index)
                            .unwrap_or(0);
                        self.cave_system.is_in_network(idx, object_id)
                            || self.cave_system.has_capacity(idx)
                    } else {
                        true
                    };
                    // C++ OpenContain::onCollide: eject other-player riders first.
                    if !container_is_tunnel_network && !container_is_cave {
                        self.kick_other_controller_occupants_for_enter(container_id, object_id);
                    }
                    let space_after_kick = self.objects.get(&container_id).is_some_and(|c| {
                        c.has_capacity_for(1) || c.contained_units().contains(&object_id)
                    });
                    let can_enter = container_has_unit
                        || (container_has_space && tunnel_has_space)
                        || already_in_tunnel_network
                        || space_after_kick;
                    if !can_enter {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // C++ ChinookAIUpdate::getAiFreeToExit — WAIT_TO_EXIT while flying.
                    if container_is_combat_chinook {
                        let allow = self.objects.get_mut(&container_id).is_some_and(|c| {
                            let p = c.get_position();
                            let moving = c.status.moving;
                            if let Some(ai) = c.chinook_ai.as_mut() {
                                ai.pos = [p.x, p.z, p.y];
                                ai.wanting_enter_or_exit = true;
                                ai.parent_idle = !moving;
                                ai.tick_idle_auto_land();
                                ai.ai_free_to_exit(false)
                                    == crate::game_logic::host_combat_chinook::HostChinookFreeToExit::FreeToExit
                            } else {
                                true
                            }
                        });
                        if !allow {
                            continue;
                        }
                    }

                    // C++ OpenContain::addToContain checkAndDetonateBoobyTrap(rider).
                    if !container_has_unit
                        && self.should_cancel_containment_after_booby_trap(container_id, object_id)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let entered = if container_has_unit {
                        true
                    } else {
                        self.objects
                            .get_mut(&container_id)
                            .map(|container| container.add_occupant(object_id))
                            .unwrap_or(false)
                    };
                    if !entered {
                        continue;
                    }

                    // Shared pool bookkeeping for tunnel residual.
                    if container_is_tunnel_network {
                        if !self
                            .tunnel_network
                            .record_enter(tunnel_player, object_id, container_id)
                        {
                            // Capacity race: undo local occupant add.
                            if let Some(container) = self.objects.get_mut(&container_id) {
                                container.remove_occupant(object_id);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }
                    if container_is_cave {
                        let idx = self
                            .objects
                            .get(&container_id)
                            .map(|c| c.cave_index)
                            .unwrap_or(0);
                        let (ok, ev) =
                            self.cave_system
                                .record_enter(idx, object_id, container_id, team);
                        if !ok {
                            if let Some(container) = self.objects.get_mut(&container_id) {
                                container.remove_occupant(object_id);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                            continue;
                        }
                        self.apply_cave_capture_event(idx, ev);
                    }
                    self.stamp_player_who_entered(container_id, object_id);
                    // C++ OpenContain::addToContain doLoadSound — leftover TheAudio.
                    self.play_container_enter_sound(container_id);

                    let container_is_heal_contain = self
                        .objects
                        .get(&container_id)
                        .is_some_and(|c| c.thing.template.contain_module.kind.is_heal_contain());
                    self.tunnel_network
                        .stamp_contained_by_frame(object_id, self.frame);

                    let enclosing_garrison = self
                        .objects
                        .get(&container_id)
                        .map(|c| c.is_enclosing_garrison_container())
                        .unwrap_or(true);
                    let occupant_owner =
                        self.objects.get(&object_id).and_then(|o| o.owner_player_id);
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.set_status_attacking(false);
                        obj.target_location = None;
                        obj.set_status_force_attack(false);
                        obj.target = Some(container_id);
                        obj.set_contained_by_enclosing(Some(container_id), enclosing_garrison);
                        if container_is_overlord_bunker {
                            // C++ OverlordContain::onContaining ExperienceSinkForRider
                            // (`OverlordContain.cpp:354-355`, default TRUE). Live
                            // BattleBunker infantry are the rider analog — bunker
                            // kills must level the tank, not the occupant.
                            obj.set_experience_sink(Some(container_id));
                        }
                        // C++ onContaining snaps enclosing occupants to the building
                        // origin. Fire Base (IsEnclosingContainer=No) stays at stations.
                        if enclosing_garrison || !container_is_structure {
                            obj.set_position(container_pos);
                            crate::game_logic::host_ground_height_log::record(
                                obj.id,
                                container_pos.y,
                                false,
                            );
                            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                                crate::game_logic::host_move_log::record(
                                    obj.id,
                                    Some([container_pos.x, container_pos.y, container_pos.z]),
                                );
                                obj.record_host_movement();
                            }
                        }
                        let __ai_st = if container_is_heal_contain || container_is_tunnel_network {
                            // C++ HealContain / TunnelContain::isGarrisonable is FALSE.
                            // Tunnel occupants are DISABLED_HELD and never shoot out
                            // of an entrance (OpenContain PassengersAllowedToFire default).
                            AIState::Docked
                        } else if container_is_structure {
                            AIState::Garrisoned
                        } else {
                            AIState::Docked
                        };
                        // Host-immediate garrison/dock residual under decision auth.
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            let ordinal =
                                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                                    &__ai_st,
                                );
                            crate::game_logic::host_ai_decision_log::record_set_state(
                                object_id, ordinal,
                            );
                        }
                        obj.set_ai_state(__ai_st);
                        obj.set_status_moving(false);
                        // C++ GarrisonContain::onContaining TheInGameUI->deselectDrawable.
                        if container_is_structure {
                            obj.deselect();
                        }
                    }
                    if container_is_tunnel_network {
                        // Enter counter already incremented in record_enter.
                    } else if container_is_heal_contain {
                        // C++ HealContain is not a garrison / transport load.
                    } else if container_is_structure {
                        self.record_garrison_residual_enter();
                        self.apply_garrison_contain_on_enter(container_id, object_id);
                        if let Some(pid) = occupant_owner {
                            if let Some(player) = self.players.get_mut(&pid) {
                                player.selected_objects.retain(|id| *id != object_id);
                            }
                        }
                        self.selected_objects.retain(|id| *id != object_id);
                    } else if container_is_overlord_bunker {
                        // China Overlord BattleBunker residual load (redirected bunker slots).
                        self.record_overlord_bunker_residual_enter();
                    } else if container_is_battle_bus {
                        // GLA Battle Bus residual load (Slots=8 infantry transport).
                        self.record_battle_bus_residual_load();
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    } else if container_is_technical {
                        // GLA Technical residual load (Slots=5 infantry; no passenger fire).
                        self.record_technical_residual_load();
                    } else if container_is_combat_cycle {
                        // GLA Combat Cycle residual load (Slots=1) + rider weapon switch.
                        self.record_combat_cycle_residual_load();
                        self.refresh_combat_cycle_rider_weapon(container_id);
                    } else if container_is_combat_chinook {
                        // AirF Combat Chinook residual load (Slots=8 + passenger fire).
                        self.record_combat_chinook_residual_load();
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    } else if container_is_listening_outpost {
                        // China Listening Outpost residual load (Slots=2 + passenger fire).
                        self.record_listening_outpost_residual_load();
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    } else if container_is_troop_crawler {
                        // China Troop Crawler residual load (Slots=8; exit-to-fight).
                        self.record_troop_crawler_residual_load();
                    } else {
                        // Vehicle transport residual load (Humvee / generic transport).
                        self.record_transport_residual_load();
                        // Humvee-style PassengersAllowedToFire still refreshes weapon set
                        // when ArmedRidersUpgradeMyWeaponSet is set.
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    }
                }
                AIState::Capturing => {
                    let Some(capture_target_id) = target_id else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };

                    let (
                        capture_power,
                        capture_start_range,
                        capture_unpack_time_ms,
                        capture_preparation_time_ms,
                        capture_pack_time_ms,
                        capture_channel,
                        capturer_moving,
                    ) = self
                        .objects
                        .get(&object_id)
                        .map(|obj| {
                            (
                                obj.thing.template.capture_power,
                                obj.thing.template.capture_start_ability_range,
                                obj.thing.template.capture_unpack_time_ms,
                                obj.thing.template.capture_preparation_time_ms,
                                obj.thing.template.capture_pack_time_ms,
                                obj.capture_channel,
                                obj.host_ai_is_moving(),
                            )
                        })
                        .unwrap_or((
                            crate::game_logic::CapturePowerKind::None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            false,
                        ));
                    let Some(power_type) = capture_power.special_power_type() else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };

                    // Once C++ `startPacking` has run, the target may already
                    // have defected or vanished.  Packing is deliberately
                    // independent of capture legality and completes before
                    // this source becomes idle again.
                    if let Some(channel) = capture_channel {
                        if channel.phase == crate::game_logic::CaptureChannelPhase::Packing {
                            let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                            if remaining > CAPTURE_CHANNEL_COMPLETE_EPSILON {
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.capture_channel =
                                        Some(crate::game_logic::CaptureChannelState {
                                            phase: crate::game_logic::CaptureChannelPhase::Packing,
                                            remaining_seconds: remaining,
                                        });
                                }
                            } else {
                                self.finish_capture_channel(object_id);
                            }
                            continue;
                        }
                    }
                    // C++ SpecialAbilityUpdate.cpp:209-220: isMoving && isPowerCurrentlyInUse
                    // && !m_facingInitiated → onExit. Live capture has no facing phase, so any
                    // movement during unpack/prep (flag raise) aborts. Approach (no channel)
                    // stays legal.
                    if capturer_moving
                        && matches!(
                            capture_channel.map(|c| c.phase),
                            Some(
                                crate::game_logic::CaptureChannelPhase::Unpacking
                                    | crate::game_logic::CaptureChannelPhase::Preparing,
                            )
                        )
                    {
                        self.abort_capture_channel(object_id);
                        continue;
                    }

                    let Some((target_position, target_team)) = self
                        .objects
                        .get(&capture_target_id)
                        .map(|target| (target.get_position(), target.team))
                    else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };

                    // All timing fields come from the same exact
                    // SpecialAbilityUpdate module.  A partial/unsupported
                    // parse must not invent a zero-duration capture ability.
                    let Some(authored_range) = capture_start_range else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };
                    let Some(unpack_time_ms) = capture_unpack_time_ms else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };
                    let Some(preparation_time_ms) = capture_preparation_time_ms else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };
                    let Some(pack_time_ms) = capture_pack_time_ms else {
                        self.abort_capture_channel(object_id);
                        continue;
                    };

                    // C++ checks the target at issue/start-preparation and
                    // continues to abort if an ally/dead/immune/garrisoned
                    // target is no longer legal.  Do not re-demand readiness:
                    // the timer begins only below in start_capture_preparation.
                    if !self.can_unit_capture_building(object_id, capture_target_id, false) {
                        self.abort_capture_channel(object_id);
                        continue;
                    }

                    // C++ `isWithinStartAbilityRange`: 2D bounding-sphere vs
                    // authored StartAbilityRange, then ApproachRequiresLOS.
                    if capture_channel.is_none()
                        && can_move
                        && !self.leftover_sa_within_start_range(
                            object_id,
                            capture_target_id,
                            authored_range,
                        )
                    {
                        if self.assign_unit_path(object_id, target_position, &[]) {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                if crate::gameworld_shadow::gameworld_ai_decision_authority_enabled(
                                ) {
                                    crate::game_logic::host_ai_decision_log::record_set_state(
                                        object_id, 19,
                                    ); // Capturing
                                } else {
                                    obj.set_ai_state(AIState::Capturing);
                                }
                            }
                        } else if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(target_position);
                            obj.set_ai_state(AIState::Capturing);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, 19,
                                ); // Capturing
                            }
                        }
                        continue;
                    }

                    // C++ initializes `STATE_UNPACKED` immediately when
                    // UnpackTime is zero; otherwise the first logical tick
                    // after entering range consumes the unpack timer.
                    let mut preparation_complete = false;
                    match capture_channel {
                        None => {
                            if unpack_time_ms > 0 {
                                let factor = self
                                    .objects
                                    .get(&object_id)
                                    .map(|object| {
                                        object.thing.template.capture_pack_unpack_variation_factor
                                    })
                                    .unwrap_or(0.0);
                                let unpack_time_ms =
                                    crate::game_logic::vary_pack_unpack_duration_ms(
                                        unpack_time_ms,
                                        factor,
                                    );
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.capture_channel =
                                        Some(crate::game_logic::CaptureChannelState::new(
                                            crate::game_logic::CaptureChannelPhase::Unpacking,
                                            unpack_time_ms,
                                        ));
                                }
                                self.begin_capture_unpacking_pose(object_id, capture_power);

                                continue;
                            }
                            if !self.start_capture_preparation(
                                object_id,
                                capture_target_id,
                                capture_power,
                                preparation_time_ms,
                            ) {
                                self.abort_capture_channel(object_id);
                                continue;
                            }
                            preparation_complete = preparation_time_ms == 0;
                        }
                        Some(channel)
                            if channel.phase
                                == crate::game_logic::CaptureChannelPhase::Unpacking =>
                        {
                            let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                            if remaining > CAPTURE_CHANNEL_COMPLETE_EPSILON {
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.capture_channel =
                                        Some(crate::game_logic::CaptureChannelState {
                                            phase:
                                                crate::game_logic::CaptureChannelPhase::Unpacking,
                                            remaining_seconds: remaining,
                                        });
                                }
                                continue;
                            }
                            if !self.start_capture_preparation(
                                object_id,
                                capture_target_id,
                                capture_power,
                                preparation_time_ms,
                            ) {
                                self.abort_capture_channel(object_id);
                                continue;
                            }
                            preparation_complete = preparation_time_ms == 0;
                        }
                        Some(channel)
                            if channel.phase
                                == crate::game_logic::CaptureChannelPhase::Preparing =>
                        {
                            let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                            if remaining > CAPTURE_CHANNEL_COMPLETE_EPSILON {
                                // C++ `continuePreparation` restarts infantry
                                // capture ReloadTime every preparation frame.
                                // Black Lotus instead resets its zero timer at
                                // the successful trigger below.
                                if capture_power != crate::game_logic::CapturePowerKind::BlackLotus
                                {
                                    if let Some(object) = self.objects.get_mut(&object_id) {
                                        object.start_power_recharge(&power_type);
                                    }
                                }
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.capture_channel =
                                        Some(crate::game_logic::CaptureChannelState {
                                            phase:
                                                crate::game_logic::CaptureChannelPhase::Preparing,
                                            remaining_seconds: remaining,
                                        });
                                }
                                self.apply_leftover_capture_fx(
                                    object_id,
                                    capture_target_id,
                                    remaining,
                                    preparation_time_ms,
                                );
                                continue;
                            }
                            preparation_complete = true;
                        }
                        Some(_) => unreachable!("Packing is handled before capture legality"),
                    }

                    if !preparation_complete {
                        continue;
                    }

                    // C++ checks an enemy trap again at trigger; one planted
                    // during the visible preparation bar can still interrupt
                    // the actual defection.
                    let planter_ally = self
                        .booby_trap
                        .plant(capture_target_id)
                        .map(|plant| plant.planter_team == team)
                        .unwrap_or(false);
                    let target_is_trapped = self.booby_trap.is_booby_trapped(capture_target_id)
                        || self
                            .objects
                            .get(&capture_target_id)
                            .map(|target| target.status.booby_trapped)
                            .unwrap_or(false);
                    if !planter_ally && target_is_trapped {
                        let _ = self.detonate_booby_trap_at(
                            capture_target_id,
                            target_position,
                            Some(object_id),
                            true,
                            false,
                        );
                    }

                    // C++ `triggerAbilityEffect` awards XP before the capture
                    // switch (`SpecialAbilityUpdate.cpp:1248-1253`), including
                    // garrison-evac triggers that do not defect the building.
                    self.award_ability_trigger_experience(
                        object_id,
                        Self::award_xp_for_capture_trigger(capture_power),
                    );
                    // C++ `triggerAbilityEffect` always plays INI TriggerSound
                    // (`SpecialAbilityUpdate.cpp:1267-1269`) before the switch.
                    self.queue_capture_trigger_sound(object_id, capture_power);

                    let did_capture =
                        if self.can_unit_capture_building(object_id, capture_target_id, false) {
                            let target_is_garrisonable =
                                self.objects.get(&capture_target_id).is_some_and(|target| {
                                    target.thing.template.garrison_contain_max.is_some()
                                });
                            if target_is_garrisonable {
                                // C++ `removeAllContained(TRUE); break;`: clearing
                                // a garrison is a successful ability trigger but
                                // never defects that structure on the same use.
                                self.evacuate_garrison_for_capture(capture_target_id);
                                false
                            } else {
                                // C++ SpecialAbilityUpdate.cpp:1436-1442:
                                // isLocallyControlled (owner player, not faction
                                // Team) then defect. Leftover try_eva_building_stolen
                                // already matches; call it before the flip so a
                                // 2v2 same-faction ally victim stays silent.
                                self.try_eva_building_stolen(capture_target_id);
                                // C++ capture uses Object::defect (SpecialAbilityUpdate.cpp:1442).
                                // defect cancelAndRefundAllProduction (Object.cpp:6136-6139)
                                // before setTeam; onCapture (Object.cpp:4509) then keeps the
                                // emptied ProductionUpdate module. Do not transfer the queue.
                                self.cancel_all_production(capture_target_id);
                                let transferred = match owner_player_id {
                                    Some(player_id) => {
                                        self.transfer_object_to_player(capture_target_id, player_id)
                                    }
                                    None => {
                                        if let Some(target) =
                                            self.objects.get_mut(&capture_target_id)
                                        {
                                            target.set_team(team);
                                            true
                                        } else {
                                            false
                                        }
                                    }
                                };
                                if transferred {
                                    self.objects
                                        .get_mut(&capture_target_id)
                                        .map(|target| {
                                            // C++ Object::defect does not restore HP
                                            // (SpecialAbilityUpdate.cpp:1442 / Object.cpp:6111).
                                            // C++ defect(..., 1) one-frame flash residual.
                                            target.flash_as_selected();
                                            true
                                        })
                                        .unwrap_or(false)
                                } else {
                                    false
                                }
                            }
                        } else {
                            false
                        };

                    // `endPreparation` clears this status before C++ starts
                    // PackTime. The source remains in the capture machine so
                    // a completed capture is not reported as immediately idle.
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        if object.is_alive() {
                            object.stop_moving();
                            object.set_status_using_ability(false);
                            // C++ startPacking: clear UNPACKING|RAISING_FLAG, set PACKING.
                            clear_raising_flag_model(object);
                            object.capture_channel =
                                Some(crate::game_logic::CaptureChannelState::new(
                                    crate::game_logic::CaptureChannelPhase::Packing,
                                    crate::game_logic::vary_pack_unpack_duration_ms(
                                        pack_time_ms,
                                        object.thing.template.capture_pack_unpack_variation_factor,
                                    ),
                                ));

                            object.set_ai_state(AIState::Capturing);
                        }
                    }
                    if self.objects.get(&object_id).is_some_and(|object| {
                        object.is_alive()
                            && object.capture_channel.map(|channel| channel.phase)
                                == Some(crate::game_logic::CaptureChannelPhase::Packing)
                    }) {
                        self.begin_capture_packing_pose(object_id, capture_power, true);
                    }

                    if pack_time_ms == 0 {
                        self.finish_capture_channel(object_id);
                    }

                    if did_capture {
                        // C++ Object::onCapture residual (kick/idle/AI-sell/deselect).
                        self.on_capture_object_residual(capture_target_id, target_team, team);
                        // C++ getAcademyStats()->recordBuildingCapture() residual.
                        let player = match owner_player_id {
                            Some(player_id) => self.get_player_mut(player_id),
                            None => self.get_player_mut_by_team(team),
                        };
                        if let Some(p) = player {
                            p.record_building_capture();
                        }
                        if capture_power == crate::game_logic::CapturePowerKind::BlackLotus {
                            self.hero_abilities.record_building_capture();
                        }

                        if capture_power == crate::game_logic::CapturePowerKind::BlackLotus {
                            if let Some(object) = self.objects.get_mut(&object_id) {
                                // C++ triggerAbilityEffect restarts only the
                                // Black Lotus capture timer here; infantry
                                // capture repeatedly reset it during prep.
                                object.start_power_recharge(&power_type);
                            }
                        }
                        // C++ EVA_BuildingStolen already fired pre-flip via leftover
                        // is_object_locally_controlled. Do not re-gate on faction Team.
                        let msg =
                            localization::localize("hud.capture.complete", "Building captured");
                        self.queue_radar_message_for_team(team, msg);
                    }
                }
                AIState::SpecialAbility => {
                    let Some(ability) = self.pending_special_abilities.get(&object_id).copied()
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.hacker_disable_channel = None;
                            obj.set_status_using_ability(false);
                            obj.set_target(None);
                        }
                        continue;
                    };
                    if matches!(ability, PendingSpecialAbility::HelixNapalmBomb { .. }) {
                        self.update_helix_napalm_bomb_channel(object_id, ability);
                        continue;
                    }

                    let special_target_id = ability.target_id();

                    // HDB is an authored, persistent SpecialAbilityUpdate
                    // channel.  Keep it wholly outside the legacy generic
                    // special branch: that branch uses a fixed range and used
                    // to apply the disable instantly (and reject an already
                    // disabled target), none of which matches C++.
                    if matches!(ability, PendingSpecialAbility::HackerDisableBuilding { .. }) {
                        self.update_hacker_disable_building_channel(
                            object_id,
                            special_target_id,
                            dt,
                        );
                        continue;
                    }

                    let Some((
                        target_position,
                        target_radius,
                        target_team,
                        target_alive,
                        target_is_vehicle,
                        target_is_structure,
                        target_is_airborne,
                        target_is_carbomb,
                        target_is_hijacked,
                        target_is_hacked,
                        target_is_unmanned,
                    )) = self.objects.get(&special_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.team,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Vehicle),
                            target.is_kind_of(KindOf::Structure),
                            target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                            target.status.is_carbomb,
                            target.status.hijacked,
                            target.status.disabled_hacked,
                            target.status.disabled_unmanned,
                        )
                    })
                    else {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let requires_enemy_target = !matches!(
                        ability,
                        PendingSpecialAbility::CarBomb { .. }
                            | PendingSpecialAbility::DisguiseAsVehicle { .. }
                    );
                    // C++ SpecialAbilityDisguiseAsVehicle StartAbilityRange = 1e6
                    // residual: complete without approach walk.
                    let disguise_instant =
                        matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. });
                    let black_lotus_range = matches!(
                        ability,
                        PendingSpecialAbility::StealCashHack { .. }
                            | PendingSpecialAbility::DisableVehicleHack { .. }
                    );
                    let snipe_range = matches!(ability, PendingSpecialAbility::SnipeVehicle { .. });
                    let booby_trap_range =
                        matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. });
                    let plant_range = matches!(
                        ability,
                        PendingSpecialAbility::PlantTimedDemoCharge { .. }
                            | PendingSpecialAbility::PlantRemoteDemoCharge { .. }
                    );
                    let leftover_busy = self.hero_abilities.leftover_channel(object_id).is_some();
                    let out_of_start_range = if plant_range {
                        !self.leftover_sa_within_start_range(
                            object_id,
                            special_target_id,
                            crate::game_logic::host_hero_abilities::PLANT_START_ABILITY_RANGE,
                        )
                    } else if black_lotus_range {
                        !self.leftover_lotus_within_start_range(
                            object_id,
                            special_target_id,
                            crate::game_logic::host_hero_abilities::BLACK_LOTUS_START_ABILITY_RANGE,
                        )
                    } else if snipe_range {
                        crate::game_logic::host_hero_abilities::leftover_bounding_sphere_2d(
                            position,
                            selection_radius,
                            target_position,
                            target_radius,
                        ) > crate::game_logic::host_jarmen_kell::JARMEN_PILOT_SNIPE_RANGE
                    } else if booby_trap_range {
                        position.distance(target_position)
                            > crate::game_logic::host_booby_trap::BOOBY_START_ABILITY_RANGE
                                + selection_radius
                                + target_radius
                    } else {
                        position.distance(target_position)
                            > selection_radius + target_radius + SPECIAL_ABILITY_RANGE_PADDING
                    };
                    if !leftover_busy && !disguise_instant && can_move && out_of_start_range {
                        self.path_approach_with_state(
                            object_id,
                            target_position,
                            AIState::SpecialAbility,
                        );
                        continue;
                    }

                    // Disguise: C++ ActionManager SPECIAL_DISGUISE_AS_VEHICLE.
                    // Bomb-truck same-template reject is commented out in retail;
                    // boats / trains / aircraft are illegal.
                    if matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. }) {
                        use crate::game_logic::host_bomb_truck_disguise::is_legal_disguise_target;
                        use crate::game_logic::host_car_bomb::object_definition_has_kind;
                        let legal = self
                            .objects
                            .get(&special_target_id)
                            .map(|t| {
                                is_legal_disguise_target(
                                    t.is_alive(),
                                    t.is_kind_of(crate::game_logic::KindOf::Vehicle),
                                    t.is_kind_of(crate::game_logic::KindOf::Aircraft)
                                        || t.status.airborne_target,
                                    t.is_kind_of(crate::game_logic::KindOf::Boat)
                                        || object_definition_has_kind(&t.template_name, "BOAT"),
                                    &t.template_name,
                                    t.status.disguised,
                                )
                            })
                            .unwrap_or(false);
                        if !legal {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // ConvertToCarBomb: C++ isValidToExecute — boat, IS_CARBOMB,
                    // WEAPONSET_CARBOMB template set, already-flagged weaponset.
                    if matches!(ability, PendingSpecialAbility::CarBomb { .. }) {
                        let reject = self
                            .objects
                            .get(&special_target_id)
                            .map(crate::game_logic::host_car_bomb::carbomb_target_rejected)
                            .unwrap_or(true);
                        if reject {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // Hijack: C++ isValidToExecute — ImmuneToCapture, boat, drone,
                    // already hijacked, occupied KINDOF_TRANSPORT.
                    if matches!(ability, PendingSpecialAbility::Hijack { .. }) {
                        let reject = self
                            .objects
                            .get(&special_target_id)
                            .map(crate::game_logic::host_car_bomb::hijack_target_rejected)
                            .unwrap_or(true);
                        if reject {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // Disable vehicle hack: unmanned still matches ActionManager.
                    // Already-hacked is legal — C++ triggerAbilityEffect refreshes.
                    if matches!(ability, PendingSpecialAbility::DisableVehicleHack { .. })
                        && target_is_unmanned
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if matches!(ability, PendingSpecialAbility::Sabotage { .. })
                        && !target_is_structure
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Burton timed/remote/Helix: leftover can_place_special_object_charge
                    // rejects dead / Bridge / BridgeTower. Tank Hunter TNT is the
                    // leftover Structure-or-(Vehicle && !Aircraft) arm — bridges legal.
                    if matches!(
                        ability,
                        PendingSpecialAbility::PlantTimedDemoCharge { .. }
                            | PendingSpecialAbility::PlantRemoteDemoCharge { .. }
                    ) {
                        let is_tank_hunter = self.objects.get(&object_id).is_some_and(|o| {
                            crate::game_logic::host_tank_hunter::is_tank_hunter_template(
                                &o.template_name,
                            )
                        });
                        let plant_ok = if is_tank_hunter {
                            crate::game_logic::host_hero_abilities::leftover_tank_hunter_tnt_target_ok(
                                target_alive,
                                target_is_structure,
                                target_is_vehicle,
                                target_is_airborne,
                            )
                        } else {
                            crate::game_logic::host_hero_abilities::leftover_charge_plant_target_ok(
                                target_alive,
                                self.objects
                                    .get(&special_target_id)
                                    .is_some_and(|t| t.is_kind_of(KindOf::Bridge)),
                                self.objects
                                    .get(&special_target_id)
                                    .is_some_and(|t| t.is_kind_of(KindOf::BridgeTower)),
                                target_is_structure,
                                target_is_vehicle && !target_is_airborne,
                            )
                        };
                        if !plant_ok {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // Black Lotus cash hack: enemy cash-generator structures only.
                    if matches!(ability, PendingSpecialAbility::StealCashHack { .. }) {
                        let is_cash_gen = self
                            .objects
                            .get(&special_target_id)
                            .map(|t| {
                                crate::game_logic::host_hero_abilities::is_cash_hack_target(
                                    &t.template_name,
                                    t.is_kind_of(KindOf::SupplyCenter),
                                    t.is_kind_of(KindOf::FSSupplyCenter),
                                    t.is_kind_of(KindOf::FSBlackMarket),
                                    t.is_kind_of(KindOf::FSSupplyDropzone),
                                )
                            })
                            .unwrap_or(false);
                        if !target_is_structure || !is_cash_gen {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // GLA Rebel BoobyTrap: allied/neutral structures only
                    // (C++ ActionManager.cpp:1610-1618).
                    if matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. }) {
                        use gamelogic::common::Relationship;
                        let (src_pid, tgt_pid) = match (
                            self.objects.get(&object_id),
                            self.objects.get(&special_target_id),
                        ) {
                            (Some(src), Some(tgt)) => (src.owner_player_id, tgt.owner_player_id),
                            _ => (None, None),
                        };
                        let rel = match (src_pid, tgt_pid) {
                            (Some(a), Some(b)) => self.player_relationship(a, b),
                            _ => Relationship::Neutral,
                        };
                        if !target_is_structure
                            || !matches!(rel, Relationship::Neutral | Relationship::Allies)
                        {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    if let Some(kind) = Self::leftover_sa_kind(ability) {
                        match self.tick_leftover_special_ability(
                            object_id,
                            special_target_id,
                            kind,
                            dt,
                        ) {
                            LeftoverSaTick::Waiting | LeftoverSaTick::Finished => continue,
                            LeftoverSaTick::Trigger => {
                                self.leftover_sa_queue_trigger_sound(object_id, kind);
                            }
                        }
                    }

                    match ability {
                        PendingSpecialAbility::Hijack { .. } => {
                            let is_hijacker = self.objects.get(&object_id).is_some_and(|unit| {
                                unit.template_name.to_ascii_lowercase().contains("hijacker")
                            });
                            if !is_hijacker {
                                self.pending_special_abilities.remove(&object_id);
                                continue;
                            }
                            // C++ ConvertToHijackedVehicleCrateCollide residual:
                            // walk → transfer team + OBJECT_STATUS_HIJACKED;
                            // ride-hide (drawable + partition unRegister) or consume.
                            // Endow MAX veterancy + hijacker vision/shroud + cancel dozer tasks.
                            // C++ order: tryInfiltrationEvent → EVA_VehicleStolen → setTeam.
                            self.try_infiltration_event(special_target_id);
                            self.try_eva_vehicle_stolen(special_target_id);
                            let donor_snap = self.objects.get(&object_id).cloned();
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_hijacked_from(donor_snap.as_ref());
                            }
                            match owner_player_id {
                                Some(player_id) => {
                                    let _ = self
                                        .transfer_object_to_player(special_target_id, player_id);
                                }
                                None => {
                                    if let Some(target) = self.objects.get_mut(&special_target_id) {
                                        target.set_team(team);
                                    }
                                }
                            }
                            // C++ transferObjectName residual.
                            let _ = self.transfer_script_object_name(object_id, special_target_id);
                            self.car_bomb.record_hijack();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_car_bomb::HIJACK_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
                            let msg =
                                localization::localize("hud.hijack.complete", "Vehicle hijacked");
                            self.queue_radar_message_for_team(team, msg);
                            // C++: if target has EjectPilotDie → hide hijacker in vehicle;
                            // else destroy hijacker immediately.
                            // Wave 753: ride-hide only when the hijacker is infantry
                            // (HijackerUpdate module). Non-infantry steal path destroys
                            // the attacker immediately (test/tank harness + C++ shape).
                            // C++: if target has EjectPilotDie and hijacker is infantry
                            // (HijackerUpdate) → hide in vehicle; else consume attacker.
                            // Wave 753: ride-hide only for infantry; non-infantry steal
                            // destroys immediately. SlowDeath must not clear destroyed —
                            // hijacker consume is same-frame (begin_slow_death clears the
                            // destroyed flag for delayed peels).
                            let hijacker_is_infantry = self
                                .objects
                                .get(&object_id)
                                .map(|h| {
                                    h.is_kind_of(KindOf::Infantry)
                                        || h.object_type == ObjectType::Infantry
                                })
                                .unwrap_or(false);
                            if hijacker_is_infantry
                                && self.vehicle_supports_hijacker_ride(special_target_id)
                            {
                                if let Some(h) = self.objects.get_mut(&object_id) {
                                    h.begin_hijacker_in_vehicle(special_target_id);
                                }
                                // C++ ThePartitionManager->unRegisterObject(hijacker).
                                self.partition_manager.unregister_object(object_id.0);
                            } else {
                                self.mark_destroyed_authority_aware(object_id, None);
                                // Suppress SlowDeath/jet/heli peels so consume sticks.
                                if let Some(o) = self.objects.get_mut(&object_id) {
                                    o.slow_death = None;
                                    o.jet_slow_death = None;
                                    o.helicopter_slow_death = None;
                                    o.status.effectively_dead = true;
                                    o.status.destroyed = true;
                                }
                                self.mark_object_for_destruction(object_id, Some(team));
                                // mark_object may re-enter SlowDeath and clear destroyed;
                                // re-assert consume residual for hijack steal.
                                if let Some(o) = self.objects.get_mut(&object_id) {
                                    o.slow_death = None;
                                    o.jet_slow_death = None;
                                    o.helicopter_slow_death = None;
                                    o.status.effectively_dead = true;
                                    o.status.destroyed = true;
                                    if !crate::gameworld_shadow::gameworld_damage_authority_live()
                                        && o.health.current > 0.0
                                    {
                                        o.health.current = 0.0;
                                    }
                                }
                            }
                        }
                        PendingSpecialAbility::Sabotage { .. } => {
                            // C++ Sabotage*CrateCollide residual: type-specific structure
                            // sabotage; saboteur consumed on success (mobile crate).
                            use crate::game_logic::host_saboteur::{
                                classify_sabotage_target, is_saboteur_template, SaboteurEffectKind,
                                SABOTEUR_CASH_STEAL_AUDIO, SABOTEUR_RESET_TIMER_AUDIO,
                                SABOTEUR_STEAL_CASH_AMOUNT, SABOTEUR_SUCCESS_AUDIO,
                            };
                            let saboteur_ok = self
                                .objects
                                .get(&object_id)
                                .map(|o| is_saboteur_template(&o.template_name))
                                .unwrap_or(false);
                            let effect = self.objects.get(&special_target_id).and_then(|t| {
                                classify_sabotage_target(
                                    &t.template_name,
                                    t.is_kind_of(KindOf::FSPower),
                                    t.is_kind_of(KindOf::PowerPlant),
                                    t.is_kind_of(KindOf::FSSupplyCenter),
                                    t.is_kind_of(KindOf::SupplyCenter),
                                    t.is_kind_of(KindOf::FSBarracks),
                                    t.is_kind_of(KindOf::FSWarFactory),
                                    t.is_kind_of(KindOf::FSAirfield),
                                    t.is_kind_of(KindOf::FSSuperweapon),
                                    t.is_kind_of(KindOf::FSStrategyCenter),
                                    t.is_kind_of(KindOf::CommandCenter),
                                    t.is_kind_of(KindOf::FSInternetCenter),
                                    t.is_kind_of(KindOf::FSFake),
                                    t.is_kind_of(KindOf::FSSupplyDropzone),
                                    crate::game_logic::host_saboteur::is_aircraft_carrier_template(
                                        &t.template_name,
                                    ),
                                )
                            });
                            if saboteur_ok {
                                if let Some(kind) = effect {
                                    let mut cash_stolen = 0u32;
                                    match kind {
                                        SaboteurEffectKind::PowerPlant => {
                                            let until = self.frame.saturating_add(
                                                crate::game_logic::host_saboteur::SABOTEUR_POWER_DURATION_FRAMES,
                                            );
                                            // C++ SabotagePowerPlantCrateCollide.cpp:112-120
                                            // other->getControllingPlayer(), never first same-faction slot.
                                            let victim_id = self
                                                .objects
                                                .get(&special_target_id)
                                                .and_then(|target| {
                                                    self.player_owner_for_host_object(target)
                                                });
                                            if let Some(player) =
                                                victim_id.and_then(|id| self.players.get_mut(&id))
                                            {
                                                player.power_sabotaged_till_frame = until;
                                            }
                                        }
                                        SaboteurEffectKind::SupplyCenter
                                        | SaboteurEffectKind::SupplyDropzone => {
                                            if matches!(kind, SaboteurEffectKind::SupplyDropzone) {
                                                // C++ OCLUpdate::resetTimer
                                                // (SabotageSupplyDropzoneCrateCollide.cpp:112-117).
                                                self.supply_drop_zones
                                                    .reset_timer(special_target_id, self.frame);
                                            }
                                            // C++ other/obj getControllingPlayer() money,
                                            // never first same-faction slot.
                                            let from_player_id = self
                                                .objects
                                                .get(&special_target_id)
                                                .and_then(|target| {
                                                    self.player_owner_for_host_object(target)
                                                });
                                            let to_player_id =
                                                self.objects.get(&object_id).and_then(|caster| {
                                                    self.player_owner_for_host_object(caster)
                                                });
                                            cash_stolen = match (from_player_id, to_player_id) {
                                                (Some(from), Some(to)) => self
                                                    .steal_cash_between_players(
                                                        from,
                                                        to,
                                                        SABOTEUR_STEAL_CASH_AMOUNT,
                                                    ),
                                                _ => 0,
                                            };
                                        }
                                        SaboteurEffectKind::MilitaryFactory => {
                                            if let Some(until) =
                                                kind.disabled_hacked_until(self.frame)
                                            {
                                                if let Some(target) =
                                                    self.objects.get_mut(&special_target_id)
                                                {
                                                    target.apply_disabled_hacked(until);
                                                }
                                            }
                                        }
                                        SaboteurEffectKind::InternetCenter => {
                                            // C++ SabotageInternetCenterCrateCollide residual:
                                            // 1) disable SpyVisionUpdate on ALL team internet centers
                                            // 2) DISABLED_HACKED on the sabotaged center
                                            // 3) DISABLED_HACKED on contained hackers
                                            let until = kind
                                                .disabled_hacked_until(self.frame)
                                                .unwrap_or_else(|| {
                                                    self.frame.saturating_add(
                                                        crate::game_logic::host_saboteur::SABOTEUR_INTERNET_DURATION_FRAMES,
                                                    )
                                                });
                                            let (centers, hackers) = self
                                                .apply_internet_center_sabotage_residual(
                                                    special_target_id,
                                                    target_team,
                                                    until,
                                                );
                                            self.saboteur.record_internet_spy_vision_disable(
                                                centers, hackers,
                                            );
                                        }
                                        SaboteurEffectKind::SuperweaponOrCommand => {
                                            // C++ SabotageSuperweaponCrateCollide: reset ALL
                                            // SpecialPowerModule interfaces via startPowerRecharge.
                                            // Host residual: object-level special power + strike
                                            // registry timers for this structure.
                                            let reset_ok = self
                                                .apply_superweapon_sabotage_recharge(
                                                    special_target_id,
                                                );
                                            if reset_ok {
                                                self.saboteur.record_superweapon_power_reset();
                                            }
                                        }
                                        SaboteurEffectKind::FakeBuilding => {
                                            // C++ SabotageFakeBuildingCrateCollide:
                                            // DAMAGE_UNRESISTABLE / DEATH_DETONATED for max health.
                                            let destroyed = self
                                                .objects
                                                .get_mut(&special_target_id)
                                                .map(|target| {
                                                    let max_hp = target
                                                        .health
                                                        .maximum
                                                        .max(target.max_health)
                                                        .max(1.0);
                                                    target.take_damage_from_typed_death(
                                                        max_hp,
                                                        Some(object_id),
                                                        crate::game_logic::combat::DamageType::Unresistable,
                                                        crate::game_logic::host_usa_pilot::HostDeathType::Detonated,
                                                    )
                                                })
                                                .unwrap_or(false);
                                            if destroyed {
                                                self.mark_object_for_destruction(
                                                    special_target_id,
                                                    Some(team),
                                                );
                                                self.saboteur.record_fake_detonated();
                                            }
                                        }
                                    }
                                    self.saboteur.record(kind, cash_stolen);
                                    // C++ TheRadar->tryInfiltrationEvent(other) residual
                                    // (victim local player warning).
                                    self.try_infiltration_event(special_target_id);
                                    // C++ TheEva->setShouldPlay residual when victim local.
                                    // Supply center: CashStolen if cash taken, else BuildingSabotaged.
                                    if kind.steals_cash() && cash_stolen > 0 {
                                        // C++ controller ScoreKeeper::addMoneyEarned residual.
                                        if let Some(p) = self
                                            .objects
                                            .get(&object_id)
                                            .and_then(|caster| {
                                                self.player_owner_for_host_object(caster)
                                            })
                                            .and_then(|id| self.get_player_mut(id))
                                        {
                                            p.add_money_earned(cash_stolen);
                                        }
                                        self.try_eva_cash_stolen(special_target_id);
                                        // C++ GUI:AddCash / GUI:LoseCash floating text residual.
                                        self.spawn_sabotage_cash_floating_texts(
                                            object_id,
                                            special_target_id,
                                            cash_stolen,
                                        );
                                    } else {
                                        self.try_eva_building_sabotaged(special_target_id);
                                    }
                                    // C++ doSabotageFeedbackFX residual (type audio + flash).
                                    self.do_sabotage_feedback_fx(special_target_id, kind);
                                    let msg = localization::localize(
                                        "hud.saboteur.complete",
                                        "Building sabotaged",
                                    );
                                    self.queue_radar_message_for_team(team, msg);
                                    // C++ CrateCollide: destroy saboteur (mobile crate).
                                    self.mark_destroyed_authority_aware(object_id, None);
                                    self.mark_object_for_destruction(object_id, Some(team));
                                    self.saboteur.record_consumed();
                                } else if let Some(obj) = self.objects.get_mut(&object_id) {
                                    // Fail-closed: non-matching structure — cancel residual.
                                    obj.stop_moving();
                                    obj.set_target(None);
                                }
                            } else if let Some(obj) = self.objects.get_mut(&object_id) {
                                // Fail-closed: non-saboteur cannot complete residual.
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::SnipeVehicle { .. } => {
                            // C++ DAMAGE_KILLPILOT residual: no HP damage; vehicle becomes
                            // unmanned + Neutral so it can be recrewed/captured.
                            // C++ car-bomb dead-man: IS_CARBOMB detonates instead.
                            let is_bomb = self
                                .objects
                                .get(&special_target_id)
                                .map(|t| t.is_car_bomb())
                                .unwrap_or(false);
                            if is_bomb {
                                let _ = self.maybe_detonate_carbomb_on_unmanned(special_target_id);
                            } else if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_kill_pilot_unmanned();
                                target.set_team(Team::Neutral);
                            }
                            self.hero_abilities.record_snipe();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_hero_abilities::SNIPE_VEHICLE_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
                            let msg = localization::localize(
                                "hud.snipe.vehicle_unmanned",
                                "Vehicle unmanned",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::PlantTimedDemoCharge { .. } => {
                            // C++ triggerAbilityEffect checkAndDetonateBoobyTrap before plant.
                            if self.leftover_probe_booby_at_target(
                                object_id,
                                special_target_id,
                                team,
                            ) {
                                self.hero_abilities.take_leftover_channel(object_id);
                                self.pending_special_abilities.remove(&object_id);
                                continue;
                            }
                            // Burton / Tank Hunter TNT residual: plant sticky timed charge at target.
                            let is_tank_hunter = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    crate::game_logic::host_tank_hunter::is_tank_hunter_template(
                                        &o.template_name,
                                    )
                                })
                                .unwrap_or(false);
                            // Tank Hunter TNT reload residual (7500ms / 225 frames).
                            let tnt_ready = if is_tank_hunter {
                                crate::game_logic::host_tank_hunter::tnt_ready(
                                    self.frame,
                                    self.tank_hunter_tnt_last_frame.get(&object_id).copied(),
                                )
                            } else {
                                true
                            };
                            let charge_id = if tnt_ready {
                                self.place_timed_demo_charge(
                                    team,
                                    target_position,
                                    Some(object_id),
                                    Some(special_target_id),
                                    None,
                                )
                            } else {
                                None
                            };
                            if charge_id.is_some() {
                                self.hero_abilities.record_timed_charge_plant();
                                if is_tank_hunter {
                                    self.tank_hunter_residual_tnt_plants =
                                        self.tank_hunter_residual_tnt_plants.saturating_add(1);
                                    self.tank_hunter_tnt_last_frame
                                        .insert(object_id, self.frame);
                                    self.queue_audio_event(
                                        AudioEventRequest::new(
                                            crate::game_logic::host_tank_hunter::TNT_INITIATE_AUDIO,
                                        )
                                        .with_object(object_id)
                                        .with_position(target_position)
                                        .with_priority(160),
                                    );
                                }
                                let msg = localization::localize(
                                    "hud.demo_charge.planted",
                                    "Demo charge planted",
                                );
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::PlantRemoteDemoCharge { .. } => {
                            if self.leftover_probe_booby_at_target(
                                object_id,
                                special_target_id,
                                team,
                            ) {
                                self.hero_abilities.take_leftover_channel(object_id);
                                self.pending_special_abilities.remove(&object_id);
                                continue;
                            }
                            // Burton residual: plant sticky remote charge (no auto-timer).
                            let charge_id = self.place_remote_demo_charge(
                                team,
                                target_position,
                                Some(object_id),
                                Some(special_target_id),
                            );
                            if charge_id.is_some() {
                                self.hero_abilities.record_remote_charge_plant();
                                let msg = localization::localize(
                                    "hud.remote_demo_charge.planted",
                                    "Remote demo charge planted",
                                );
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::StealCashHack { .. } => {
                            self.leftover_kill_special_objects(object_id);
                            // C++ `triggerAbilityEffect` AwardXPForTriggering
                            // (`SpecialAbilityUpdate.cpp:1248-1253`). Retail
                            // `SpecialAbilityBlackLotusStealCashHack` = 20.
                            self.award_ability_trigger_experience(
                                object_id,
                                crate::game_logic::host_hero_abilities::LOTUS_STEAL_AWARD_XP as i32,
                            );
                            // Black Lotus residual: steal cash from enemy economy.
                            // C++ SPECIAL_BLACKLOTUS_STEAL_CASH_HACK:
                            // target->getControllingPlayer() / object->getControllingPlayer()
                            // withdraw/deposit, scorekeeper money earned, EVA_CashStolen
                            // when victim local, GUI:AddCash/LoseCash floating texts.
                            // Never debit/credit the first same-faction player.
                            let amount =
                                crate::game_logic::host_hero_abilities::STEAL_CASH_DEFAULT_AMOUNT;
                            let from_player_id = self
                                .objects
                                .get(&special_target_id)
                                .and_then(|target| self.player_owner_for_host_object(target));
                            let to_player_id = self
                                .objects
                                .get(&object_id)
                                .and_then(|caster| self.player_owner_for_host_object(caster));
                            let stolen = match (from_player_id, to_player_id) {
                                (Some(from), Some(to)) => {
                                    self.steal_cash_between_players(from, to, amount)
                                }
                                _ => 0,
                            };
                            if stolen > 0 {
                                self.hero_abilities.record_cash_steal(stolen);
                                // C++ controller->getScoreKeeper()->addMoneyEarned(cash)
                                if let Some(p) = to_player_id.and_then(|id| self.get_player_mut(id))
                                {
                                    p.add_money_earned(stolen);
                                }
                                self.try_eva_cash_stolen(special_target_id);
                                self.spawn_sabotage_cash_floating_texts(
                                    object_id,
                                    special_target_id,
                                    stolen,
                                );
                                let msg =
                                    localization::localize("hud.cash_hack.complete", "Cash stolen");

                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::CarBomb { .. } => {
                            let is_terrorist = self.objects.get(&object_id).is_some_and(|unit| {
                                crate::game_logic::is_terrorist_template(&unit.template_name)
                            });
                            if !is_terrorist {
                                self.pending_special_abilities.remove(&object_id);
                                continue;
                            }
                            // C++ ConvertToCarBombCrateCollide residual:
                            // vehicle defects to converter team, gains IS_CARBOMB +
                            // SuicideCarBomb weapon residual. Converter is consumed.
                            // Detonation happens later when the car bomb attacks.
                            // Booby-trap residual: cancel if mine detonates and either dies.
                            let booby = self
                                .objects
                                .get(&special_target_id)
                                .map(|t| t.status.booby_trapped)
                                .unwrap_or(false);
                            if booby {
                                // Detonate trap residual damage on both.
                                if let Some(t) = self.objects.get_mut(&special_target_id) {
                                    let _ = t.take_damage_from(
                                        t.health.maximum.max(1.0),
                                        Some(object_id),
                                    );
                                }
                                if let Some(b) = self.objects.get_mut(&object_id) {
                                    let _ = b.take_damage_from(
                                        b.health.maximum.max(1.0),
                                        Some(special_target_id),
                                    );
                                }
                                let t_dead = self
                                    .objects
                                    .get(&special_target_id)
                                    .map(|t| !t.is_alive() || t.status.destroyed)
                                    .unwrap_or(true);
                                let b_dead = self
                                    .objects
                                    .get(&object_id)
                                    .map(|b| !b.is_alive() || b.status.destroyed)
                                    .unwrap_or(true);
                                if t_dead || b_dead {
                                    if t_dead {
                                        self.mark_object_for_destruction(
                                            special_target_id,
                                            Some(team),
                                        );
                                    }
                                    if b_dead {
                                        self.mark_object_for_destruction(object_id, Some(team));
                                    }
                                    continue;
                                }
                            }
                            // Snapshot donor residual (vision/vet) before consume.
                            let donor_snap = self.objects.get(&object_id).cloned();
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_convert_to_car_bomb_from(donor_snap.as_ref());
                            }
                            match owner_player_id {
                                Some(player_id) => {
                                    let _ = self
                                        .transfer_object_to_player(special_target_id, player_id);
                                }
                                None => {
                                    if let Some(target) = self.objects.get_mut(&special_target_id) {
                                        target.set_team(team);
                                    }
                                }
                            }
                            // C++ transferObjectName residual (script named object).
                            let _ = self.transfer_script_object_name(object_id, special_target_id);
                            self.car_bomb.record_conversion();
                            // C++ FXList::doFXObj(m_fxList, other) → FX_MakeCarBombSuccess
                            // (Sound nugget TerroristCarBomb). Never play the FXList name.
                            let fx = crate::game_logic::host_car_bomb::CAR_BOMB_CONVERT_FX_LIST;
                            if !self.dispatch_fx_list_at_host_object(
                                fx,
                                special_target_id,
                                Some(object_id),
                            ) {
                                let sounds = crate::game_logic::sound_names_for_fx_list(fx);
                                if sounds.is_empty() {
                                    self.queue_audio_event(
                                        AudioEventRequest::new(
                                            crate::game_logic::host_car_bomb::CAR_BOMB_CONVERT_FX_SOUND,
                                        )
                                        .with_object(special_target_id)
                                        .with_position(target_position)
                                        .with_priority(170),
                                    );
                                } else {
                                    for sound in sounds {
                                        self.queue_audio_event(
                                            AudioEventRequest::new(&sound)
                                                .with_object(special_target_id)
                                                .with_position(target_position)
                                                .with_priority(170),
                                        );
                                    }
                                }
                            }
                            let msg = localization::localize(
                                "hud.carbomb.converted",
                                "Vehicle converted to car bomb",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            self.mark_destroyed_authority_aware(object_id, None);
                            self.mark_object_for_destruction(object_id, Some(team));
                        }
                        PendingSpecialAbility::DisableVehicleHack { .. } => {
                            // C++ SpecialAbilityUpdate BLACKLOTUS_DISABLE_VEHICLE_HACK:
                            // setDisabledUntil(DISABLED_HACKED, now + EffectDuration).
                            let until = self.frame.saturating_add(
                                crate::game_logic::host_hero_abilities::DISABLE_VEHICLE_HACK_DURATION_FRAMES,
                            );
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_disabled_hacked(until);
                            }
                            let do_fx = self
                                .hero_abilities
                                .leftover_channel(object_id)
                                .map(|ch| ch.do_disable_fx_particles)
                                .unwrap_or(true);
                            let new_do_fx = self.leftover_spawn_disable_fx(
                                object_id,
                                special_target_id,
                                crate::game_logic::host_hero_abilities::LOTUS_DISABLE_FX_PARTICLE,
                                crate::game_logic::host_hero_abilities::DISABLE_VEHICLE_HACK_DURATION_FRAMES,
                                do_fx,
                            );
                            if let Some(mut ch) =
                                self.hero_abilities.leftover_channel(object_id).copied()
                            {
                                ch.do_disable_fx_particles = new_do_fx;
                                self.hero_abilities.set_leftover_channel(object_id, ch);
                            }
                            self.leftover_kill_special_objects(object_id);
                            self.hero_abilities.record_vehicle_disable();
                            let msg = localization::localize(
                                "hud.vehicle_hack.disabled",
                                "Vehicle disabled",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::HackerDisableBuilding { .. } => {
                            unreachable!("HDB is intercepted by its typed persistent channel")
                        }
                        PendingSpecialAbility::HelixNapalmBomb { .. } => {
                            unreachable!(
                                "Helix NapalmBomb is intercepted by its typed approach channel"
                            )
                        }
                        PendingSpecialAbility::DisguiseAsVehicle { .. } => {
                            // C++ StealthUpdate::disguiseAsObject residual:
                            // if target already disguised, copy *its* disguise
                            // template + player; else copy target template + team.
                            // set OBJECT_STATUS_DISGUISED + STEALTHED.
                            let (tpl, as_team, copied_disguise) = self
                                .objects
                                .get(&special_target_id)
                                .map(|t| {
                                    if t.status.disguised {
                                        if let (Some(dt), Some(dteam)) =
                                            (t.disguise_as_template.as_ref(), t.disguise_as_team)
                                        {
                                            return (dt.clone(), dteam, true);
                                        }
                                    }
                                    (t.template_name.clone(), t.team, false)
                                })
                                .unwrap_or_else(|| {
                                    ("UnknownVehicle".to_string(), target_team, false)
                                });
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.apply_disguise(&tpl, as_team);
                                obj.stop_moving();
                                if crate::gameworld_shadow::gameworld_ai_decision_authority_enabled(
                                ) {
                                    crate::game_logic::host_ai_decision_log::record_stop_attack(
                                        object_id,
                                    );
                                    crate::game_logic::host_ai_decision_log::record_set_state(
                                        object_id, 0,
                                    );
                                } else {
                                    obj.set_target(None);
                                    obj.set_ai_state(AIState::Idle);
                                }
                            }
                            self.bomb_truck_disguise.record_disguise(object_id, &tpl);
                            self.bomb_truck_disguise.record_transition_start();
                            if copied_disguise {
                                self.bomb_truck_disguise.record_disguise_copy();
                            }
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_AUDIO,
                                )
                                .with_object(object_id)
                                .with_position(position)
                                .with_priority(160),
                            );
                            let msg = localization::localize(
                                "hud.bombtruck.disguised",
                                "Bomb truck disguised",
                            );
                            self.queue_radar_message_for_team(team, msg);
                        }
                        PendingSpecialAbility::PlantBoobyTrap { .. } => {
                            // C++ triggerAbilityEffect SPECIAL_BOOBY_TRAP:
                            // checkAndDetonateBoobyTrap first; cancel if either dies;
                            // refuse a second plant while BOOBY_TRAPPED remains.
                            use crate::game_logic::host_booby_trap::{
                                has_booby_trap_upgrade, is_booby_trap_planter_template,
                                BOOBY_TRAP_INSTALL_AUDIO,
                            };
                            if self.leftover_probe_booby_at_target(
                                object_id,
                                special_target_id,
                                team,
                            ) {
                                self.hero_abilities.take_leftover_channel(object_id);
                                self.pending_special_abilities.remove(&object_id);
                                continue;
                            }
                            let still_trapped = self.booby_trap.is_booby_trapped(special_target_id)
                                || self
                                    .objects
                                    .get(&special_target_id)
                                    .is_some_and(|target| target.status.booby_trapped);
                            if still_trapped {
                                self.pending_special_abilities.remove(&object_id);
                                if let Some(obj) = self.objects.get_mut(&object_id) {
                                    obj.stop_moving();
                                    obj.set_target(None);
                                }
                                continue;
                            }
                            let (can_plant, ready) = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    let planter_ok =
                                        is_booby_trap_planter_template(&o.template_name)
                                            && has_booby_trap_upgrade(&o.applied_upgrades);
                                    let ready = self.booby_trap.plant_ready(object_id, self.frame);
                                    (planter_ok, ready)
                                })
                                .unwrap_or((false, false));
                            if can_plant
                                && ready
                                && self.booby_trap.can_place_special_object(object_id)
                            {
                                let geom = self
                                    .objects
                                    .get(&special_target_id)
                                    .map(|t| t.selection_radius.max(8.0))
                                    .unwrap_or(8.0);
                                let _ = self.booby_trap.install(
                                    special_target_id,
                                    object_id,
                                    team,
                                    self.frame,
                                    geom,
                                    None,
                                );
                                if let Some(cid) = self.spawn_booby_trap_special_object(
                                    object_id,
                                    team,
                                    special_target_id,
                                ) {
                                    self.booby_trap.set_charge_object(special_target_id, cid);
                                }
                                if let Some(target) = self.objects.get_mut(&special_target_id) {
                                    target.set_status_booby_trapped(true);
                                }
                                self.queue_audio_event(
                                    AudioEventRequest::new(BOOBY_TRAP_INSTALL_AUDIO)
                                        .with_object(special_target_id)
                                        .with_position(target_position)
                                        .with_priority(160),
                                );
                                let msg = localization::localize(
                                    "hud.booby_trap.planted",
                                    "Booby trap planted",
                                );
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                    }

                    if let Some(kind) = Self::leftover_sa_kind(ability) {
                        let (timings, variation) = self.leftover_timings_for(object_id, kind);
                        if timings.flee_range > 0.0 {
                            self.leftover_flee_after_plant(
                                object_id,
                                special_target_id,
                                team,
                                timings.flee_range,
                                timings.flip_after_unpack,
                            );
                        } else {
                            let pack_ms = crate::game_logic::vary_pack_unpack_duration_ms(
                                timings.pack_ms,
                                variation,
                            );
                            self.leftover_begin_packing(
                                object_id,
                                special_target_id,
                                kind,
                                pack_ms,
                            );
                        }
                    } else {
                        self.pending_special_abilities.remove(&object_id);
                    }
                }
                AIState::Gathering => {
                    // Retail GameData.ini `ValuePerSupplyBox = 75` (ZH override of
                    // C++ GlobalData.cpp default 100). Player::getSupplyBoxValue
                    // (Player.cpp:1928-1930) reads TheGlobalData->m_baseValuePerSupplyBox.
                    const SUPPLY_BOX_VALUE: u32 =
                        crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX
                            as u32;

                    // C++ WorkerAIUpdate.cpp:283-287 — harvest path arms
                    // WEAPONSET_MINE_CLEARING_DETAIL so GLA workers can be
                    // diverted onto mines while ferrying.
                    self.arm_worker_harvest_mine_clearing(object_id);
                    // C++ ChinookAIUpdate::isAvailableForSupplying
                    // (ChinookAIUpdate.cpp:982-991): loaded / enter-exit
                    // pending Chinooks must not auto-dock warehouses.
                    if !self.collector_available_for_supplying(object_id) {
                        self.set_ai_state_decision_aware(object_id, AIState::Idle);
                        continue;
                    }

                    let Some(source_id) = target_id else {
                        self.set_ai_state_decision_aware(object_id, AIState::Idle);
                        continue;
                    };

                    // Extract source state before any mutations.
                    let (
                        source_alive,
                        source_pos,
                        source_supplies,
                        source_is_warehouse,
                        delete_when_empty,
                    ) = self
                        .objects
                        .get(&source_id)
                        .map(|s| {
                            (
                                s.is_alive(),
                                s.get_position(),
                                s.stored_resources.supplies,
                                s.thing.template.dock_kind
                                    == crate::game_logic::DockKind::SupplyWarehouse,
                                s.thing.template.dock_delete_when_empty,
                            )
                        })
                        .unwrap_or((false, position, 0, false, false));

                    if !source_alive {
                        // C++ dock session ends → WANTING. Cargo banks first.
                        self.route_supply_wanting(
                            object_id,
                            team,
                            owner_player_id,
                            position,
                            can_move,
                        );
                        continue;
                    }

                    let collector_metadata_early = self
                        .objects
                        .get(&object_id)
                        .and_then(|object| object.thing.template.supply_truck_metadata);
                    if source_is_warehouse && collector_metadata_early.is_some() {
                        let docker_r = self
                            .objects
                            .get(&object_id)
                            .map(|o| {
                                crate::game_logic::host_supply_gather::host_bounding_circle_radius(
                                    o.thing.template.geometry_info.authored,
                                    o.thing.template.geometry_info.bounding_circle_radius(),
                                    o.thing.geometry.radius,
                                )
                            })
                            .unwrap_or(1.0);
                        let warehouse_r = self
                            .objects
                            .get(&source_id)
                            .map(|o| {
                                crate::game_logic::host_supply_gather::host_bounding_circle_radius(
                                    o.thing.template.geometry_info.authored,
                                    o.thing.template.geometry_info.bounding_circle_radius(),
                                    o.thing.geometry.radius,
                                )
                            })
                            .unwrap_or(0.0);
                        if crate::game_logic::host_supply_gather::warehouse_too_far_2d(
                            (position.x, position.z),
                            (source_pos.x, source_pos.z),
                            docker_r,
                            warehouse_r,
                        ) {
                            let close = docker_r * 2.0;
                            if can_move && position.distance(source_pos) > close + 1.0 {
                                self.path_approach_with_state(
                                    object_id,
                                    source_pos,
                                    AIState::Gathering,
                                );
                                continue;
                            }
                            let (dx, dz) =
                                crate::game_logic::host_supply_gather::warehouse_twitch_delta(
                                    crate::game_logic::host_supply_gather::twitch_seed(
                                        object_id, self.frame,
                                    ),
                                    1,
                                );
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                let mut pos = obj.get_position();
                                pos.x += dx;
                                pos.z += dz;
                                obj.set_position(pos);
                            }
                            continue;
                        }
                    } else if can_move && position.distance(source_pos) > INTERACT_RANGE {
                        self.path_approach_with_state(object_id, source_pos, AIState::Gathering);
                        continue;
                    }
                    if source_is_warehouse && !self.try_claim_dock(source_id, object_id) {
                        continue;
                    }

                    // C++ AIDock waits for the authored warehouse action
                    // delay, then SupplyWarehouseDockUpdate transfers one
                    // box. Only an authored SupplyTruckAIUpdate uses this
                    // path; the older generic harvest command remains intact.
                    let collector_metadata = self
                        .objects
                        .get(&object_id)
                        .and_then(|object| object.thing.template.supply_truck_metadata);
                    if let Some(metadata) = collector_metadata {
                        let (state, next_frame) = self
                            .objects
                            .get(&object_id)
                            .map(|object| {
                                (
                                    object.supply_truck_state,
                                    object.supply_truck_next_dock_action_frame,
                                )
                            })
                            .unwrap_or((SupplyTruckState::Idle, 0));
                        if state != SupplyTruckState::DockingWarehouse {
                            if let Some(object) = self.objects.get_mut(&object_id) {
                                object.supply_truck_force_pending = false;
                                object.supply_truck_state = SupplyTruckState::DockingWarehouse;
                                object.supply_truck_next_dock_action_frame =
                                    self.frame.saturating_add(metadata.warehouse_delay_frames);
                            }
                            continue;
                        }
                        if self.frame < next_frame {
                            continue;
                        }
                    }

                    // In range — gather resources.  The host tracks cash
                    // value rather than C++ individual boxes, but a warehouse
                    // still cannot grant more than its authored stock.  This
                    // avoids turning an empty `SupplyWarehouseDockUpdate`
                    // into an infinite source.
                    let gather_amount = collector_metadata
                        .map(|_| SUPPLY_BOX_VALUE)
                        .unwrap_or_else(|| (100.0 * dt) as u32);
                    let max_carry = collector_metadata
                        .map(|metadata| metadata.max_boxes.saturating_mul(SUPPLY_BOX_VALUE))
                        .unwrap_or(1000);
                    let current_carry = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.stored_resources.supplies)
                        .unwrap_or(0);
                    // C++ SupplyTruckAIUpdate::gainOneBox (SupplyTruckAIUpdate.cpp:134-135)
                    // fails when m_numberBoxes >= m_maxBoxesData. Warehouse action
                    // (SupplyWarehouseDockUpdate.cpp:89-111) then ++m_boxesStored
                    // to take the tentative debit back.
                    let already_at_max_boxes = collector_metadata.is_some_and(|metadata| {
                        let (_remaining, _carry, transferred) =
                            crate::game_logic::host_supply_gather::warehouse_action_transfer_one_box(
                                source_supplies / SUPPLY_BOX_VALUE,
                                current_carry / SUPPLY_BOX_VALUE,
                                metadata.max_boxes,
                            );
                        !transferred && current_carry / SUPPLY_BOX_VALUE >= metadata.max_boxes
                    });
                    // Keep the legacy generic-resource path intact.  The
                    // precise stock gate is required specifically for the
                    // newly-authored SupplyWarehouseDockUpdate target.
                    let taken = if source_is_warehouse {
                        gather_amount.min(source_supplies)
                    } else {
                        gather_amount
                    };
                    if source_is_warehouse {
                        let crippled = self.objects.get(&source_id).is_some_and(|s| {
                            matches!(
                                s.body_damage_state,
                                crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged
                                    | crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                            )
                        });
                        if crippled {
                            let airborne = self.objects.get(&object_id).is_some_and(|o| {
                                o.is_kind_of(KindOf::Aircraft) || o.status.airborne_target
                            });
                            let docker_r = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    crate::game_logic::host_supply_gather::host_bounding_circle_radius(
                                        o.thing.template.geometry_info.authored,
                                        o.thing.template.geometry_info.bounding_circle_radius(),
                                        o.thing.geometry.radius,
                                    )
                                })
                                .unwrap_or(1.0);
                            let warehouse_r = self
                                .objects
                                .get(&source_id)
                                .map(|o| {
                                    crate::game_logic::host_supply_gather::host_bounding_circle_radius(
                                        o.thing.template.geometry_info.authored,
                                        o.thing.template.geometry_info.bounding_circle_radius(),
                                        o.thing.geometry.radius,
                                    )
                                })
                                .unwrap_or(0.0);
                            let inside =
                                !crate::game_logic::host_supply_gather::warehouse_too_far_2d(
                                    (position.x, position.z),
                                    (source_pos.x, source_pos.z),
                                    docker_r,
                                    warehouse_r,
                                );
                            match crate::game_logic::host_supply_gather::dock_cripple_victim_action(
                                true, inside, airborne,
                            ) {
                                crate::game_logic::host_supply_gather::DockCrippleVictimAction::KillGround => {
                                    if let Some(obj) = self.objects.get_mut(&object_id) {
                                        Self::mark_object_destroyed_authority_aware(obj, None);
                                    }
                                    self.mark_object_for_destruction(object_id, None);
                                }
                                crate::game_logic::host_supply_gather::DockCrippleVictimAction::IdleAndForceWanting => {
                                    if let Some(obj) = self.objects.get_mut(&object_id) {
                                        obj.supply_truck_force_pending = true;
                                        obj.supply_truck_state = SupplyTruckState::Wanting;
                                    }
                                    self.stop_attack_decision_aware(object_id);
                                    self.set_ai_state_decision_aware(object_id, AIState::Idle);
                                }
                                crate::game_logic::host_supply_gather::DockCrippleVictimAction::None => {}
                            }
                            self.release_dock_if_holder(source_id, object_id);

                            continue;
                        }
                    }
                    if source_is_warehouse && already_at_max_boxes {
                        self.release_dock_if_holder(source_id, object_id);
                        let refinery_dest = self
                            .preferred_or_allied_supply_center(
                                object_id,
                                team,
                                owner_player_id,
                                position,
                            )
                            .and_then(|rid| self.objects.get(&rid).map(|r| r.get_position()));
                        if let Some(dest) = refinery_dest {
                            self.path_approach_with_state(
                                object_id,
                                dest,
                                AIState::ReturningResources,
                            );
                        }
                        continue;
                    }
                    if source_is_warehouse && taken == 0 {
                        self.release_dock_if_holder(source_id, object_id);
                        // C++ action() FALSE → AIDock SUCCESS → WANTING.
                        // Partial cargo banks; empty seeks another warehouse.
                        self.route_supply_wanting(
                            object_id,
                            team,
                            owner_player_id,
                            position,
                            can_move,
                        );
                        continue;
                    }
                    let is_full = current_carry + taken >= max_carry;

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_stored_supplies(
                            obj.stored_resources
                                .supplies
                                .saturating_add(taken)
                                .min(max_carry),
                        );
                        if let Some(metadata) = collector_metadata {
                            obj.supply_truck_next_dock_action_frame =
                                self.frame.saturating_add(metadata.warehouse_delay_frames);
                        }
                    }

                    let remaining_after = source_supplies.saturating_sub(taken);
                    // Deplete the supply source.
                    if let Some(source) = self.objects.get_mut(&source_id) {
                        source.set_stored_supplies(remaining_after);
                        if remaining_after == 0 && (!source_is_warehouse || delete_when_empty) {
                            Self::mark_object_destroyed_authority_aware(source, None);
                            self.mark_object_for_destruction(source_id, None);
                        }
                    }
                    if remaining_after == 0 && collector_metadata.is_some() {
                        let scan = self
                            .collector_warehouse_scan(object_id, owner_player_id)
                            .unwrap_or(0.0);
                        let next_dist = self
                            .find_nearest_harvestable_supply_within(
                                team,
                                position,
                                Some(scan).filter(|d| *d > 0.0),
                                object_id,
                            )
                            .and_then(|nid| {
                                self.objects
                                    .get(&nid)
                                    .map(|s| s.get_position().distance(position))
                            });
                        let voice = self
                            .objects
                            .get(&object_id)
                            .map(|o| o.thing.template.supplies_depleted_voice.clone())
                            .unwrap_or_default();
                        if crate::game_logic::host_supply_gather::should_play_supplies_depleted_voice(
                            next_dist, scan, &voice,
                        ) {
                            self.queue_audio_event(
                                crate::game_logic::AudioEventRequest::new(&voice)
                                    .with_object(object_id)
                                    .with_position(position)
                                    .with_priority(160),
                            );
                        }
                    }

                    if source_is_warehouse && (is_full || remaining_after == 0) {
                        self.release_dock_if_holder(source_id, object_id);
                    }

                    if is_full {
                        // Full — `SupplyTruckAIUpdate::m_preferredDock` wins
                        // over ResourceManager's nearest-center search when
                        // AI assigned this collector to a specific depot.
                        let refinery_dest = self
                            .preferred_or_allied_supply_center(
                                object_id,
                                team,
                                owner_player_id,
                                position,
                            )
                            .and_then(|rid| self.objects.get(&rid).map(|r| r.get_position()));
                        if let Some(dest) = refinery_dest {
                            self.path_approach_with_state(
                                object_id,
                                dest,
                                AIState::ReturningResources,
                            );
                        }
                    }
                }
                AIState::ReturningResources => {
                    self.arm_worker_harvest_mine_clearing(object_id);
                    if !self.collector_available_for_supplying(object_id) {
                        self.set_ai_state_decision_aware(object_id, AIState::Idle);
                        continue;
                    }
                    // Deposit resources when close to a supply center.
                    let refinery_id = self.preferred_or_allied_supply_center(
                        object_id,
                        team,
                        owner_player_id,
                        position,
                    );

                    if let Some(rid) = refinery_id {
                        if !self.try_claim_dock(rid, object_id) {
                            continue;
                        }

                        let collector_metadata = self
                            .objects
                            .get(&object_id)
                            .and_then(|object| object.thing.template.supply_truck_metadata);
                        if let Some(metadata) = collector_metadata {
                            let (state, next_frame) = self
                                .objects
                                .get(&object_id)
                                .map(|object| {
                                    (
                                        object.supply_truck_state,
                                        object.supply_truck_next_dock_action_frame,
                                    )
                                })
                                .unwrap_or((SupplyTruckState::Idle, 0));
                            if state != SupplyTruckState::DockingCenter {
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.supply_truck_state = SupplyTruckState::DockingCenter;
                                    object.supply_truck_next_dock_action_frame =
                                        self.frame.saturating_add(metadata.center_delay_frames);
                                }
                                continue;
                            }
                            if self.frame < next_frame {
                                continue;
                            }
                        }
                        // Deposit.
                        // C++ SupplyCenterDockUpdate::action always returns FALSE
                        // (AIDock SUCCESS) even at 0 boxes, then banks
                        // getUpgradedSupplyBoost (Supply Lines / Worker Shoes).
                        // Never leave the exclusive dock claimed.
                        let deposit_amount = self
                            .objects
                            .get(&object_id)
                            .map(|o| o.stored_resources.supplies)
                            .unwrap_or(0);

                        // Snapshot carrier for C++ getUpgradedSupplyBoost identity.
                        let (
                                carrier_is_gla_worker,
                                carrier_has_worker_shoes,
                                carrier_is_chinook,
                                carrier_is_combat_chinook,
                                carrier_authored_boost,
                            ) = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    let is_w = crate::game_logic::host_gla_worker::is_gla_worker_template(
                                        &o.template_name,
                                    );
                                    // C++ WorkerAIUpdate::getUpgradedSupplyBoost
                                    // (WorkerAIUpdate.cpp:1376-1384): controlling
                                    // player hasUpgradeComplete only. Object tags
                                    // and any same Team enum leak 2v2 allies.
                                    let shoes = self
                                        .player_owner_for_host_object(o)
                                        .and_then(|pid| self.players.get(&pid))
                                        .is_some_and(|p| {
                                            p.has_unlocked_upgrade(
                                                crate::game_logic::host_gla_worker::UPGRADE_GLA_WORKER_SHOES,
                                            )
                                        });
                                    let is_chinook =
                                        crate::game_logic::host_supply_gather::is_chinook_supply_collector(
                                            &o.template_name,
                                        ) || o.chinook_ai.is_some()
                                            || o.is_combat_chinook_style_container();
                                    let is_combat = o.is_combat_chinook_style_container()
                                        || crate::game_logic::host_combat_chinook::is_combat_chinook_template(
                                            &o.template_name,
                                        );
                                    let authored = o
                                        .thing
                                        .template
                                        .supply_truck_metadata
                                        .map(|m| m.upgraded_supply_boost)
                                        .unwrap_or(0);
                                    (is_w, shoes, is_chinook, is_combat, authored)
                                })
                                .unwrap_or((false, false, false, false, 0));

                        // Clear carried resources.
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_stored_supplies(0);
                        }
                        // C++ SupplyCenterDockUpdate::action + Chinook
                        // getUpgradedSupplyBoost: INI boost only for Chinooks
                        // with Upgrade_AmericaSupplyLines. Trucks return 0.
                        let has_supply_lines = self.players.values().any(|p| {
                            p.team == team
                                && p.has_unlocked_upgrade(
                                    crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES,
                                )
                        });
                        let supply_lines_boost =
                            crate::game_logic::host_supply_gather::collector_supply_lines_boost(
                                carrier_is_chinook,
                                carrier_is_combat_chinook,
                                carrier_authored_boost,
                                has_supply_lines,
                            );
                        // GLA WorkerShoes residual: +8 per drop-off when unlocked.
                        let worker_shoes_boost =
                                crate::game_logic::host_gla_worker::residual_worker_shoes_drop_off_boost(
                                    carrier_is_gla_worker,
                                    carrier_has_worker_shoes,
                                );
                        let boost = supply_lines_boost.saturating_add(worker_shoes_boost);
                        let credited = deposit_amount.saturating_add(boost);
                        // Credit the player (carried supplies + optional economy boost).
                        // Capture the concrete owner before the mutable
                        // credit so the typed event below is tied to this
                        // real ReturningResources deposit, not a later
                        // resource-total observation or passive income.
                        // C++ SupplyCenterDockUpdate::action credits the
                        // *center* controlling player, so allied drop-offs
                        // pay the dock owner instead of vanishing.
                        let credited_player_id = refinery_id.and_then(|rid| {
                            self.objects
                                .get(&rid)
                                .and_then(|center| self.player_owner_for_host_object(center))
                        });

                        if credited > 0 {
                            if let Some(player_id) = credited_player_id {
                                let credited_player =
                                    if let Some(player) = self.get_player_mut(player_id) {
                                        player.credit_supplies(credited);
                                        // C++ SupplyCenterDockUpdate::action:
                                        // deposit(value) and ScoreKeeper::addMoneyEarned(value).
                                        player.add_money_earned(credited);
                                        true
                                    } else {
                                        false
                                    };
                                if credited_player {
                                    self.record_supply_dropoff_event(
                                        crate::game_logic::SupplyDropoffEvent {
                                            carrier_id: object_id,
                                            player_id,
                                            carried_amount: deposit_amount,
                                        },
                                    );
                                    let center_status = self
                                        .objects
                                        .get(&refinery_id.expect("checked above"))
                                        .map(|c| (c.status.stealthed, c.status.detected));
                                    let local = self
                                        .players
                                        .get(&player_id)
                                        .map(|p| p.is_local)
                                        .unwrap_or(false);
                                    let hide = center_status.is_some_and(|(stealth, detected)| {
                                        crate::game_logic::host_supply_gather::hide_stealth_supply_cash(
                                            stealth, local, detected,
                                        )
                                    });

                                    if !hide && credited > 0 {
                                        let ground_y =
                                            self.terrain_height_at(position).unwrap_or(position.y);
                                        let color = self
                                            .players
                                            .get(&player_id)
                                            .map(|p| p.color_rgb)
                                            .unwrap_or((0, 255, 0));
                                        self.oil_derricks.record_floating_text(
                                            crate::game_logic::host_oil_derrick::HostAutoDepositFloatingText {
                                                text: crate::game_logic::host_supply_gather::format_gui_add_cash(credited),
                                                text_key: crate::game_logic::host_supply_gather::SUPPLY_CENTER_ADD_CASH_KEY
                                                    .to_string(),
                                                position: glam::Vec3::new(position.x, ground_y, position.z),
                                                color_rgba: (
                                                    color.0,
                                                    color.1,
                                                    color.2,
                                                    crate::game_logic::host_supply_gather::SUPPLY_CENTER_FLOATING_TEXT_ALPHA,
                                                ),
                                                amount: credited,
                                                spawn_frame: self.frame,
                                                source_id: object_id,
                                                is_capture_bonus: false,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                        if let Some(rid) = refinery_id {
                            if credited > 0 {
                                self.grant_center_temporary_stealth(rid, object_id);
                            }
                            self.release_dock_if_holder(rid, object_id);
                        }

                        if supply_lines_boost > 0 {
                            self.supply_lines_bonus_cash_total = self
                                .supply_lines_bonus_cash_total
                                .saturating_add(supply_lines_boost);
                        }
                        if worker_shoes_boost > 0 {
                            self.gla_worker
                                .record_shoes_drop_off_boost(worker_shoes_boost);
                        }
                        if deposit_amount > 0 {
                            // Head back to gather more from the original source.
                            let source_dest = target_id.and_then(|sid| {
                                self.objects
                                    .get(&sid)
                                    .filter(|s| s.is_alive())
                                    .map(|s| s.get_position())
                            });
                            if let Some(dest) = source_dest {
                                if let Some(object) = self.objects.get_mut(&object_id) {
                                    object.supply_truck_state = SupplyTruckState::Wanting;
                                    object.supply_truck_next_dock_action_frame = 0;
                                }
                                self.path_approach_with_state(object_id, dest, AIState::Gathering);
                            } else if let Some(next) = self.find_nearest_harvestable_supply_within(
                                team,
                                position,
                                self.collector_warehouse_scan(object_id, owner_player_id),
                                object_id,
                            ) {
                                if let Some(dest) =
                                    self.objects.get(&next).map(|s| s.get_position())
                                {
                                    if let Some(obj) = self.objects.get_mut(&object_id) {
                                        obj.set_target(Some(next));
                                    }
                                    self.path_approach_with_state(
                                        object_id,
                                        dest,
                                        AIState::Gathering,
                                    );
                                }
                            } else {
                                self.begin_supply_regroup(
                                    object_id,
                                    team,
                                    owner_player_id,
                                    position,
                                );
                            }
                        } else {
                            // C++ action() FALSE → AIDock SUCCESS → WANTING.
                            self.route_supply_wanting(
                                object_id,
                                team,
                                owner_player_id,
                                position,
                                can_move,
                            );
                        }
                    } else if can_move {
                        let _ = can_move;
                    }
                }
                AIState::Docked | AIState::Garrisoned => {
                    // Aircraft parking: C++ JetTakeoffOrLandingState reserves the
                    // stall-column runway and keeps the hangar when
                    // KeepsParkingSpaceWhenAirborne (JetAIUpdate.cpp:1630, 897-900).
                    let wants_sortie = self
                        .objects
                        .get(&object_id)
                        .map(|o| {
                            (o.is_kind_of(KindOf::Aircraft)
                                || o.object_type == ObjectType::Aircraft)
                                && (o.movement.target_position.is_some()
                                    || o.target.is_some()
                                    || o.target_location.is_some())
                        })
                        .unwrap_or(false);
                    if wants_sortie {
                        let _ = self.try_runway_takeoff_from_airfield(object_id);
                        continue;
                    }
                    // Prefer contained_by (authoritative residual link) over target.
                    let container_id = self
                        .objects
                        .get(&object_id)
                        .and_then(|o| o.container_id())
                        .or(target_id);
                    let Some(container_id) = container_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_contained_by(None);
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let Some((
                        container_pos,
                        container_alive,
                        container_has_unit,
                        station_pin,
                        enclosing,
                    )) = self.objects.get(&container_id).map(|container| {
                        // C++ GarrisonContain::positionObjectsAtStationGarrisonPoints:
                        // non-enclosing Fire Base stays on STATION, not building center.
                        let enclosing = container.is_enclosing_garrison_container();
                        let station_pin = if matches!(ai_state, AIState::Garrisoned) && !enclosing {
                            container.building_data.as_ref().and_then(|bd| {
                                bd.garrison_point_occupant
                                    .iter()
                                    .enumerate()
                                    .find(|(_, id)| **id == Some(object_id))
                                    .and_then(|(i, _)| bd.garrison_station_points.get(i).copied())
                            })
                        } else {
                            None
                        };
                        (
                            container.get_position(),
                            container.is_alive(),
                            container.contained_units().contains(&object_id),
                            station_pin,
                            enclosing,
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_contained_by(None);
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !container_alive || !container_has_unit {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_contained_by(None);
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let pin_pos = station_pin.unwrap_or(container_pos);
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_contained_by_enclosing(Some(container_id), enclosing);
                        obj.set_position(pin_pos);
                        crate::game_logic::host_ground_height_log::record(obj.id, pin_pos.y, false);
                        if crate::gameworld_shadow::gameworld_movement_authority_live() {
                            crate::game_logic::host_move_log::record(
                                obj.id,
                                Some([pin_pos.x, pin_pos.y, pin_pos.z]),
                            );
                            obj.record_host_movement();
                        }
                        obj.stop_moving();
                        obj.set_status_moving(false);
                    }
                }
                AIState::Idle => {
                    // C++ SupplyTruckStateMachine Idle: isForcedIntoWantingState
                    // → ST_WANTING. Regrouping success → ST_WANTING
                    // (SupplyTruckAIUpdate.cpp:383-418).
                    self.tick_supply_force_wanting(
                        object_id,
                        team,
                        owner_player_id,
                        position,
                        can_move,
                    );
                }
                AIState::Attacking => {
                    // C++ parent stays AI_GUARD; live peels to Attacking for fire.
                    // hasAttackedMeAndICanReturnFire is registered on INNER.
                    let phase = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.guard_chase_phase)
                        .unwrap_or(0);
                    if phase == GUARD_CHASE_PHASE_INNER
                        && can_attack
                        && self.try_guard_last_attacker(object_id, team)
                    {
                        continue;
                    }
                    let _ = self.tick_guard_chase_exits(object_id);
                }

                _ => {}
            }
        }

        // C++ RiderChangeContain::update: after an ordinary rider exit, the
        // bike remains as an unselectable toppled shell for ScuttleDelay, then
        // dies with DEATH_TOPPLED.  Replacement keeps m_containing=true and
        // therefore never reaches this list.
        let scuttles_due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(&id, object)| {
                let delay = object
                    .thing
                    .template
                    .contain_module
                    .rider_change_scuttle_delay_frames?;
                let started = object.rider_change_scuttled_on_frame;
                (object.thing.template.contain_module.kind
                    == crate::game_logic::ContainModuleKind::RiderChange
                    && started != 0
                    && !object.status.destroyed
                    && self.frame >= started.saturating_add(delay))
                .then_some(id)
            })
            .collect();
        for object_id in scuttles_due {
            if let Some(object) = self.objects.get_mut(&object_id) {
                object.status.death_type =
                    crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
            }
            self.mark_destroyed_authority_aware(object_id, None);
            self.mark_object_for_destruction(object_id, None);
        }

        // C++ HealContain::update + TunnelContain::update → TunnelTracker::healObjects.
        self.tick_heal_contain_and_tunnel();
        // C++ TunnelContain::update nemesis + AITNGuard::lookForInnerTarget.
        self.tick_tunnel_network_nemesis();
    }
}
