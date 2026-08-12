//! Host objects `impl GameLogic` — `support_states`.
//! update_support_states. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub(in super::super) fn update_support_states(&mut self, object_ids: &[ObjectId], dt: f32) {
        const GUARD_MIN_RADIUS: f32 = 80.0;
        const INTERACT_RANGE: f32 = crate::game_logic::host_repair::HOST_REPAIR_INTERACT_RANGE;
        const CAPTURE_RANGE_PADDING: f32 = 4.0;
        const SPECIAL_ABILITY_RANGE_PADDING: f32 = 4.0;
        // Host residual flat HP/sec (not C++ percent-of-max / TimeForFullHeal matrix).
        const REPAIR_RATE: f32 = crate::game_logic::host_repair::HOST_REPAIR_RATE_HP_PER_SEC;
        const HEAL_RATE: f32 = crate::game_logic::host_repair::HOST_HEAL_RATE_HP_PER_SEC;

        for &object_id in object_ids {
            let snapshot = match self.objects.get(&object_id) {
                Some(obj) => (
                    obj.ai_state.clone(),
                    obj.team,
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

            if ai_state != AIState::SpecialAbility {
                self.pending_special_abilities.remove(&object_id);
            }

            match ai_state {
                AIState::GuardingArea => {
                    let anchor = guard_position.unwrap_or(position);
                    let radius = guard_radius.max(GUARD_MIN_RADIUS);
                    // C++ GuardMode residual (AIGuard.cpp):
                    // Normal — pursue outside (wider acquire).
                    // WithoutPursuit — no outer chase; engage only inside radius.
                    // FlyingUnitsOnly — PartitionFilterIsFlying on acquire.
                    let acquire_radius = match guard_mode {
                        crate::game_logic::GuardMode::Normal => radius * 1.5,
                        _ => radius,
                    };

                    if can_attack {
                        let flying_only =
                            matches!(guard_mode, crate::game_logic::GuardMode::FlyingUnitsOnly);
                        let without_pursuit =
                            matches!(guard_mode, crate::game_logic::GuardMode::WithoutPursuit);
                        // Prefer nearest legal enemy around the guard anchor.
                        let mut best: Option<(ObjectId, f32)> = None;
                        for (cand_id, cand) in self.objects.iter() {
                            if !cand.is_alive() || !cand.is_targetable_by_enemy_of(team) {
                                continue;
                            }
                            if flying_only
                                && !(cand.is_kind_of(KindOf::Aircraft)
                                    || cand.object_type == ObjectType::Aircraft)
                            {
                                continue;
                            }
                            let d = anchor.distance(cand.get_position());
                            if d > acquire_radius {
                                continue;
                            }
                            if without_pursuit && d > radius {
                                continue;
                            }
                            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                                best = Some((*cand_id, d));
                            }
                        }
                        if let Some((enemy_id, _)) = best {
                            // WithoutPursuit: if we already left the bubble, return home first.
                            if without_pursuit && position.distance(anchor) > radius {
                                if can_move {
                                    self.path_approach_with_state(
                                        object_id,
                                        anchor,
                                        AIState::GuardingArea,
                                    );
                                }
                            } else {
                                self.engage_target_decision_aware(object_id, enemy_id);
                                continue;
                            }
                        }
                    }

                    if can_move && position.distance(anchor) > radius * 0.6 {
                        self.path_approach_with_state(object_id, anchor, AIState::GuardingArea);
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

                    let radius = guard_radius.max(GUARD_MIN_RADIUS);
                    if can_attack {
                        if let Some((enemy_id, _)) =
                            crate::ai_decisions::AIDecisionSystem::find_nearest_enemy(
                                self,
                                guard_anchor,
                                team,
                                radius,
                            )
                        {
                            self.engage_target_decision_aware(object_id, enemy_id);
                            continue;
                        }
                    }

                    if can_move && position.distance(guard_anchor) > radius * 0.6 {
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
                        .map(|obj| obj.can_repair())
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
                        repair_target_team,
                        repair_target_alive,
                        repair_target_is_structure,
                        repair_target_under_construction,
                    )) = self.objects.get(&repair_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.team,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Structure),
                            target.status.under_construction,
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !repair_target_alive
                        || !repair_target_is_structure
                        || repair_target_under_construction
                        || (repair_target_team != team && repair_target_team != Team::Neutral)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if can_move && position.distance(repair_target_pos) > INTERACT_RANGE {
                        self.path_approach_with_state(
                            object_id,
                            repair_target_pos,
                            AIState::Repairing,
                        );
                        continue;
                    }

                    // Dozer structure-repair residual: heal HP over time while in range.
                    // C++ DozerAIUpdate DOZER_TASK_REPAIR + MODELCONDITION_ACTIVELY_CONSTRUCTING.
                    // RepairHealthPercentPerSecond residual (2% max HP / sec).
                    // Fail-closed: multi-dozer both allowed (not full sole-benefactor reject).
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_actively_constructing(true);
                    }
                    let max_hp = self
                        .objects
                        .get(&repair_target_id)
                        .map(|t| t.health.maximum)
                        .unwrap_or(0.0);
                    let heal_per_sec =
                        crate::game_logic::host_repair::dozer_repair_hp_per_sec(max_hp)
                            .max(REPAIR_RATE * 0.25);
                    let heal_amount = heal_per_sec * dt;
                    // C++ attemptHealingFromSoleBenefactor(health, dozer, 2) residual.
                    let now = self.frame;
                    let sole = if let Some(target) = self.objects.get_mut(&repair_target_id) {
                        let healed = target.attempt_healing_from_sole_benefactor(
                            heal_amount,
                            object_id,
                            2,
                            now,
                        );
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
                        self.sole_benefactor_repair_rejects =
                            self.sole_benefactor_repair_rejects.saturating_add(1);
                        continue;
                    }
                    if healed {
                        self.record_structure_repair_residual_heal();
                    }
                    if target_full {
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
                    }
                }
                state @ (AIState::SeekingRepair | AIState::SeekingHealing) => {
                    if health_current >= health_maximum - 0.01 {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let Some(support_target_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let Some((
                        support_target_pos,
                        support_target_team,
                        support_target_alive,
                        support_target_under_construction,
                        support_building_type,
                        support_template_name,
                    )) = self.objects.get(&support_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.team,
                            target.is_alive(),
                            target.status.under_construction,
                            target
                                .building_data
                                .as_ref()
                                .map(|b| b.building_type)
                                .unwrap_or(BuildingType::CommandCenter),
                            target.template_name.clone(),
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
                        || support_target_team != team
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
                        .map(|obj| match state {
                            AIState::SeekingRepair => {
                                if obj.is_kind_of(KindOf::Aircraft) {
                                    crate::game_logic::host_repair::building_provides_aircraft_repair(
                                        support_building_type,
                                    )
                                } else if obj.is_kind_of(KindOf::Vehicle) {
                                    // RepairPad (USA) + WarFactory (China RepairDock residual).
                                    crate::game_logic::host_repair::building_provides_vehicle_repair(
                                        support_building_type,
                                    )
                                } else {
                                    false
                                }
                            }
                            AIState::SeekingHealing => {
                                let name = support_template_name.to_ascii_lowercase();
                                let is_heal_pad = support_building_type
                                    == BuildingType::HealPad
                                    || name.contains("hospital")
                                    || name.contains("heal")
                                    || name.contains("medic");
                                obj.is_kind_of(KindOf::Infantry) && is_heal_pad
                            }
                            _ => false,
                        })
                        .unwrap_or(false);
                    if !source_can_use_support {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    if can_move && position.distance(support_target_pos) > INTERACT_RANGE {
                        self.path_approach_with_state(object_id, support_target_pos, state.clone());
                        continue;
                    }

                    // Pad/airfield/war-factory residual: heal self over time while docked in range.
                    // C++ RepairDockUpdate::action TimeForFullHeal residual (flat host rate).
                    // HealPad SeekingHealing residual records heal honesty separately.
                    let mut vehicle_healed = false;
                    let mut heal_pad_healed = false;
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        let rate = match state {
                            AIState::SeekingRepair => REPAIR_RATE,
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
                        } else {
                            // Host-immediate residual: keep SeekingRepair/Healing
                            // authoritative on host; log for GameWorld last-write.
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                let ordinal =
                                    crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                                        &state,
                                    );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, ordinal,
                                );
                            }
                            obj.set_ai_state(state);
                        }
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
                    // Retail VeterancyCrateCollide IsPilot path residual.
                    {
                        let pilot_snapshot = self.objects.get(&object_id).map(|o| {
                            (
                                crate::game_logic::host_usa_pilot::is_pilot_template(
                                    &o.template_name,
                                ),
                                o.team,
                                o.experience.level,
                                o.get_position(),
                                o.selection_radius,
                                o.can_move(),
                            )
                        });
                        let vehicle_snapshot = self.objects.get(&container_id).map(|v| {
                            (
                                v.get_position(),
                                v.selection_radius,
                                v.is_alive(),
                                v.is_kind_of(KindOf::Vehicle),
                                v.is_kind_of(KindOf::Aircraft) || v.status.airborne_target,
                                v.is_unmanned(),
                                v.status.under_construction,
                                v.is_worker()
                                    || v.template_name.to_ascii_lowercase().contains("dozer"),
                            )
                        });
                        if let (
                            Some((
                                is_pilot,
                                pilot_team,
                                pilot_level,
                                pilot_pos,
                                pilot_radius,
                                pilot_can_move,
                            )),
                            Some((
                                vehicle_pos,
                                vehicle_radius,
                                v_alive,
                                v_vehicle,
                                v_air,
                                v_unmanned,
                                v_under_construction,
                                v_dozer,
                            )),
                        ) = (pilot_snapshot, vehicle_snapshot)
                        {
                            let recrewable =
                                crate::game_logic::host_usa_pilot::is_recrewable_unmanned_vehicle(
                                    v_alive,
                                    v_vehicle,
                                    v_air,
                                    v_unmanned,
                                    v_under_construction,
                                    v_dozer,
                                );
                            if crate::game_logic::host_usa_pilot::should_recrew_on_enter(
                                is_pilot, recrewable,
                            ) {
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
                                    .map(|v| v.apply_pilot_recrew(pilot_team, pilot_level))
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

                    let Some((
                        container_pos,
                        container_radius,
                        container_team,
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
                            container.is_alive(),
                            container.status.under_construction,
                            container.can_contain(),
                            container.has_capacity_for(1),
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

                    // Residual garrison / Overlord BattleBunker / Battle Bus:
                    // infantry/heroes only (C++ AllowInsideKindOf = INFANTRY).
                    // Combat Chinook allows INFANTRY + VEHICLE (not AIRCRAFT).
                    // Tunnel Network: C++ allows all units except aircraft.
                    let unit_can_garrison_structure = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.is_kind_of(KindOf::Infantry) || o.is_hero())
                        .unwrap_or(false);
                    let unit_is_aircraft = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.is_kind_of(KindOf::Aircraft))
                        .unwrap_or(false);
                    if container_is_tunnel_network {
                        // TunnelContain residual: reject aircraft only.
                        if unit_is_aircraft {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                            continue;
                        }
                    } else if (container_is_structure
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
                    if container_is_combat_chinook && unit_is_aircraft {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Tunnel network residual: units already in the shared pool may
                    // transfer to another allied tunnel without walking (can_move false).
                    let already_in_tunnel_network = container_is_tunnel_network
                        && self.tunnel_network.team_holding_unit(object_id).is_some();

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

                    if container_team != team
                        && container_team != Team::Neutral
                        && (container_is_faction_structure || container_occupant_count > 0)
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

                    // Tunnel shared capacity (MaxTunnelCapacity=10) overrides local space.
                    let tunnel_has_space = if container_is_tunnel_network {
                        self.tunnel_network.is_in_network(team, object_id)
                            || self.tunnel_network.has_capacity(team)
                    } else {
                        true
                    };
                    let can_enter = container_has_unit
                        || (container_has_space && tunnel_has_space)
                        || already_in_tunnel_network;
                    if !can_enter {
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
                            .record_enter(team, object_id, container_id)
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

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.set_status_attacking(false);
                        obj.target_location = None;
                        obj.set_status_force_attack(false);
                        obj.target = Some(container_id);
                        obj.set_contained_by(Some(container_id));
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
                        let __ai_st = if container_is_structure {
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
                    }
                    if container_is_tunnel_network {
                        // Enter counter already incremented in record_enter.
                    } else if container_is_structure {
                        self.record_garrison_residual_enter();
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
                        self.clear_target_decision_aware(object_id);
                        continue;
                    };

                    let (can_capture_buildings, is_lotus_captor) = self
                        .objects
                        .get(&object_id)
                        .map(|obj| {
                            let is_lotus =
                                crate::game_logic::host_hero_abilities::is_black_lotus_template(
                                    &obj.template_name,
                                );
                            let can =
                                crate::game_logic::host_hero_abilities::can_capture_without_upgrade(
                                    obj.is_hero(),
                                    is_lotus,
                                ) || (obj.is_kind_of(KindOf::Infantry)
                                    && self.team_has_completed_capture_upgrade(obj.team));
                            (can, is_lotus || obj.is_hero())
                        })
                        .unwrap_or((false, false));
                    if !can_capture_buildings {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let Some((
                        target_position,
                        target_radius,
                        target_team,
                        target_alive,
                        target_is_structure,
                        target_under_construction,
                    )) = self.objects.get(&capture_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.team,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Structure),
                            target.status.under_construction,
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !target_alive || !target_is_structure || target_under_construction {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if target_team == team {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    // Black Lotus / hero residual: StartAbilityRange 150.
                    // Infantry residual: selection radii + small pad.
                    let capture_range = if is_lotus_captor {
                        crate::game_logic::host_hero_abilities::BLACK_LOTUS_START_ABILITY_RANGE
                    } else {
                        selection_radius + target_radius + CAPTURE_RANGE_PADDING
                    };
                    if can_move && position.distance(target_position) > capture_range {
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

                    let did_capture = if self
                        .objects
                        .get(&capture_target_id)
                        .map(|target| {
                            target.is_alive()
                                && target.is_kind_of(KindOf::Structure)
                                && !target.status.under_construction
                                && target.team != team
                        })
                        .unwrap_or(false)
                    {
                        // BoobyTrap residual: enemy capture detonate (allies skip).
                        // C++ SpecialAbilityUpdate / checkAndDetonateBoobyTrap(captor).
                        let trap_pos = self
                            .objects
                            .get(&capture_target_id)
                            .map(|t| t.get_position())
                            .unwrap_or(target_position);
                        let planter_ally = self
                            .booby_trap
                            .plant(capture_target_id)
                            .map(|p| p.planter_team == team)
                            .unwrap_or(false);
                        if !planter_ally
                            && (self.booby_trap.is_booby_trapped(capture_target_id)
                                || self
                                    .objects
                                    .get(&capture_target_id)
                                    .map(|t| t.status.booby_trapped)
                                    .unwrap_or(false))
                        {
                            let _ = self.detonate_booby_trap_at(
                                capture_target_id,
                                trap_pos,
                                Some(object_id),
                                true,
                                false,
                            );
                        }
                        // Structure may have been destroyed by trap — re-check.
                        if !self
                            .objects
                            .get(&capture_target_id)
                            .map(|t| t.is_alive())
                            .unwrap_or(false)
                        {
                            false
                        } else {
                            // C++ capture prep residual: warn local victim + infiltration.
                            self.try_eva_building_being_stolen(capture_target_id);
                            self.try_infiltration_event(capture_target_id);
                            self.cancel_all_production(capture_target_id);
                            if let Some(target) = self.objects.get_mut(&capture_target_id) {
                                target.set_team(team);
                                target.health.heal(target.max_health);
                                // C++ defect(..., 1) one-frame flash residual.
                                target.flash_as_selected();
                                true
                            } else {
                                false
                            }
                        }
                    } else {
                        false
                    };

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.set_target(None);
                    }

                    if did_capture {
                        // C++ Object::onCapture residual (kick/idle/AI-sell/deselect).
                        self.on_capture_object_residual(capture_target_id, target_team, team);
                        // C++ getAcademyStats()->recordBuildingCapture() residual.
                        if let Some(p) = self.get_player_mut_by_team(team) {
                            p.record_building_capture();
                        }
                        if is_lotus_captor {
                            self.hero_abilities.record_building_capture();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_hero_abilities::CAPTURE_BUILDING_AUDIO,
                                )
                                .with_object(object_id)
                                .with_position(position)
                                .with_priority(160),
                            );
                        }
                        // C++ EVA_BuildingStolen when victim was local before defect.
                        // (team already flipped — use BeingStolen honesty or explicit
                        // pre-flip: fire BuildingStolen if victim team had local player
                        // that is no longer owner.)
                        // BeingStolen already gated on pre-flip local control; Stolen
                        // should also only fire for former local owner.
                        // Re-check: after flip, former local team lost the building —
                        // if any local player is on previous target_team.
                        let former_local = self
                            .players
                            .values()
                            .any(|p| p.is_local && p.is_alive && p.team == target_team);
                        if former_local {
                            let _ = gamelogic::helpers::TheEva::set_should_play(
                                gamelogic::helpers::EvaEvent::BuildingStolen,
                            );
                            crate::game_logic::host_eva_log::record_event(
                                gamelogic::helpers::EvaEvent::BuildingStolen,
                            );
                            self.hero_abilities.record_eva_building_stolen();
                        }
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
                            obj.set_target(None);
                        }
                        continue;
                    };
                    let special_target_id = ability.target_id();

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

                    // CarBomb allows neutral; DisguiseAsVehicle allows any living
                    // vehicle (ally/enemy/neutral) — C++ ActionManager residual.
                    let requires_enemy_target = !matches!(
                        ability,
                        PendingSpecialAbility::CarBomb { .. }
                            | PendingSpecialAbility::DisguiseAsVehicle { .. }
                    );
                    if !target_alive
                        || (requires_enemy_target
                            && (target_team == team || target_team == Team::Neutral))
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if matches!(
                        ability,
                        PendingSpecialAbility::SnipeVehicle { .. }
                            | PendingSpecialAbility::Hijack { .. }
                            | PendingSpecialAbility::CarBomb { .. }
                            | PendingSpecialAbility::DisableVehicleHack { .. }
                            | PendingSpecialAbility::DisguiseAsVehicle { .. }
                    ) && (!target_is_vehicle || target_is_airborne)
                    {
                        self.pending_special_abilities.remove(&object_id);
                        self.clear_target_decision_aware(object_id);
                        continue;
                    }

                    // Disguise: reject bomb-truck / train name residual targets,
                    // unless the target is already disguised (C++ disguiseAsObject
                    // copies that appearance — true template may still be bomb truck).
                    if matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. }) {
                        use crate::game_logic::host_bomb_truck_disguise::{
                            is_bomb_truck_template, is_legal_disguise_target_template,
                        };
                        let (target_tpl, target_disguised) = self
                            .objects
                            .get(&special_target_id)
                            .map(|t| (t.template_name.clone(), t.status.disguised))
                            .unwrap_or_default();
                        let reject_bomb = is_bomb_truck_template(&target_tpl) && !target_disguised;
                        if reject_bomb || !is_legal_disguise_target_template(&target_tpl) {
                            // is_legal rejects bomb trucks by name; allow when disguised.
                            if !(target_disguised && is_bomb_truck_template(&target_tpl)) {
                                self.pending_special_abilities.remove(&object_id);
                                if let Some(obj) = self.objects.get_mut(&object_id) {
                                    obj.set_target(None);
                                }
                                continue;
                            }
                        }
                    }

                    // ConvertToCarBomb: cannot re-convert an existing car bomb.
                    if matches!(ability, PendingSpecialAbility::CarBomb { .. }) && target_is_carbomb
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Hijack: cannot re-hijack an already hijacked vehicle.
                    if matches!(ability, PendingSpecialAbility::Hijack { .. }) && target_is_hijacked
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Disable vehicle hack: skip already-hacked or unmanned vehicles.
                    if matches!(ability, PendingSpecialAbility::DisableVehicleHack { .. })
                        && (target_is_hacked || target_is_unmanned)
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

                    // Burton plant charge (timed or remote): structure or ground vehicle.
                    if matches!(
                        ability,
                        PendingSpecialAbility::PlantTimedDemoCharge { .. }
                            | PendingSpecialAbility::PlantRemoteDemoCharge { .. }
                    ) && !(target_is_structure || (target_is_vehicle && !target_is_airborne))
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
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

                    // China Hacker DisableBuilding: enemy structures only; skip already-hacked.
                    if matches!(ability, PendingSpecialAbility::HackerDisableBuilding { .. }) {
                        if !target_is_structure || target_is_hacked {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // GLA Rebel BoobyTrap: structures only (enemy/neutral residual).
                    if matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. }) {
                        if !target_is_structure {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // C++ SpecialAbilityDisguiseAsVehicle StartAbilityRange = 1e6
                    // residual: complete without approach walk.
                    let disguise_instant =
                        matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. });
                    // Hacker DisableBuilding residual: StartAbilityRange 150 (not melee pad).
                    let hacker_disable_range =
                        matches!(ability, PendingSpecialAbility::HackerDisableBuilding { .. });
                    // Black Lotus residual specials: StartAbilityRange 150.
                    let black_lotus_range = matches!(
                        ability,
                        PendingSpecialAbility::StealCashHack { .. }
                            | PendingSpecialAbility::DisableVehicleHack { .. }
                    );
                    let booby_trap_range =
                        matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. });
                    let interact_range = if hacker_disable_range {
                        crate::game_logic::host_hacker_disable::HACKER_DISABLE_START_ABILITY_RANGE
                    } else if black_lotus_range {
                        crate::game_logic::host_hero_abilities::BLACK_LOTUS_START_ABILITY_RANGE
                    } else if booby_trap_range {
                        crate::game_logic::host_booby_trap::BOOBY_START_ABILITY_RANGE
                            + selection_radius
                            + target_radius
                    } else {
                        selection_radius + target_radius + SPECIAL_ABILITY_RANGE_PADDING
                    };
                    if !disguise_instant
                        && can_move
                        && position.distance(target_position) > interact_range
                    {
                        self.path_approach_with_state(
                            object_id,
                            target_position,
                            AIState::SpecialAbility,
                        );
                        continue;
                    }

                    match ability {
                        PendingSpecialAbility::Hijack { .. } => {
                            // C++ ConvertToHijackedVehicleCrateCollide residual:
                            // walk → transfer team + OBJECT_STATUS_HIJACKED; hijacker
                            // consumed (fail-closed vs hide-in-vehicle HijackerUpdate).
                            // Endow MAX veterancy + cancel dozer tasks via apply_hijacked_from.
                            // C++ order: tryInfiltrationEvent → EVA_VehicleStolen → setTeam.
                            self.try_infiltration_event(special_target_id);
                            self.try_eva_vehicle_stolen(special_target_id);
                            let donor_snap = self.objects.get(&object_id).cloned();
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_hijacked_from(donor_snap.as_ref());
                                target.set_team(team);
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
                                            if let Some(player) =
                                                self.get_player_mut_by_team(target_team)
                                            {
                                                player.power_sabotaged_till_frame = until;
                                            }
                                        }
                                        SaboteurEffectKind::SupplyCenter => {
                                            cash_stolen = self.steal_cash_from_team(
                                                target_team,
                                                team,
                                                SABOTEUR_STEAL_CASH_AMOUNT,
                                            );
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
                                        if let Some(p) = self.get_player_mut_by_team(team) {
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
                            // Black Lotus residual: steal cash from enemy economy.
                            // C++ SPECIAL_BLACKLOTUS_STEAL_CASH_HACK:
                            // withdraw/deposit, scorekeeper money earned, EVA_CashStolen
                            // when victim local, GUI:AddCash/LoseCash floating texts.
                            let amount =
                                crate::game_logic::host_hero_abilities::STEAL_CASH_DEFAULT_AMOUNT;
                            let stolen = self.steal_cash_from_team(target_team, team, amount);
                            if stolen > 0 {
                                self.hero_abilities.record_cash_steal(stolen);
                                // C++ controller->getScoreKeeper()->addMoneyEarned(cash)
                                if let Some(p) = self.get_player_mut_by_team(team) {
                                    p.add_money_earned(stolen);
                                }
                                self.try_eva_cash_stolen(special_target_id);
                                self.spawn_sabotage_cash_floating_texts(
                                    object_id,
                                    special_target_id,
                                    stolen,
                                );
                                self.queue_audio_event(
                                    AudioEventRequest::new(
                                        crate::game_logic::host_hero_abilities::STEAL_CASH_AUDIO,
                                    )
                                    .with_object(object_id)
                                    .with_position(position)
                                    .with_priority(160),
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
                                target.set_team(team);
                            }
                            // C++ transferObjectName residual (script named object).
                            let _ = self.transfer_script_object_name(object_id, special_target_id);
                            self.car_bomb.record_conversion();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_car_bomb::CAR_BOMB_CONVERT_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
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
                            self.hero_abilities.record_vehicle_disable();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_hero_abilities::DISABLE_VEHICLE_HACK_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
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
                            // C++ SpecialAbilityUpdate SPECIAL_HACKER_DISABLE_BUILDING:
                            // setDisabledUntil(DISABLED_HACKED, now + EffectDuration 2000ms).
                            use crate::game_logic::host_hacker_disable::{
                                hacker_disable_until_frame, HACKER_DISABLE_BUILDING_AUDIO,
                            };
                            let until = hacker_disable_until_frame(self.frame);
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_disabled_hacked(until);
                            }
                            self.hacker_disable_building_count =
                                self.hacker_disable_building_count.saturating_add(1);
                            self.queue_audio_event(
                                AudioEventRequest::new(HACKER_DISABLE_BUILDING_AUDIO)
                                    .with_object(special_target_id)
                                    .with_position(target_position)
                                    .with_priority(170),
                            );
                            let msg = localization::localize(
                                "hud.hacker.building_disabled",
                                "Building disabled",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
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
                            // C++ SpecialAbilityBoobyTrap residual: mark structure BOOBY_TRAPPED.
                            use crate::game_logic::host_booby_trap::{
                                has_booby_trap_upgrade, is_booby_trap_planter_template,
                                BOOBY_TRAP_INSTALL_AUDIO,
                            };
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
                                let prev = self.booby_trap.install(
                                    special_target_id,
                                    object_id,
                                    team,
                                    self.frame,
                                    geom,
                                    None,
                                );
                                if let Some(prev_plant) = prev {
                                    if let Some(cid) = prev_plant.charge_object_id {
                                        self.destroy_booby_trap_special_object(cid);
                                    }
                                }
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

                    self.pending_special_abilities.remove(&object_id);
                }
                AIState::Gathering => {
                    // Accumulate resources when close to the supply source.
                    const GATHER_RATE: f32 = 100.0;
                    const MAX_CARRY: u32 = 1000;

                    let Some(source_id) = target_id else {
                        self.set_ai_state_decision_aware(object_id, AIState::Idle);
                        continue;
                    };

                    // Extract source state before any mutations.
                    let (source_alive, source_pos) = self
                        .objects
                        .get(&source_id)
                        .map(|s| (s.is_alive(), s.get_position()))
                        .unwrap_or((false, position));

                    if !source_alive {
                        // C++ supply truck residual: find another warehouse when pile empties.
                        if let Some(next) = self.find_nearest_harvestable_supply(team, position) {
                            if let Some(dest) = self.objects.get(&next).map(|s| s.get_position()) {
                                if let Some(obj) = self.objects.get_mut(&object_id) {
                                    obj.set_target(Some(next));
                                }
                                self.path_approach_with_state(object_id, dest, AIState::Gathering);
                                continue;
                            }
                        }
                        self.stop_attack_decision_aware(object_id);
                        self.set_ai_state_decision_aware(object_id, AIState::Idle);
                        continue;
                    }

                    if can_move && position.distance(source_pos) > INTERACT_RANGE {
                        self.path_approach_with_state(object_id, source_pos, AIState::Gathering);
                        continue;
                    }

                    // In range — gather resources.
                    // C++ parity (SupplyWarehouseDockUpdate): gathering depletes
                    // the supply source.  The source is destroyed when empty.
                    let gather_amount = (GATHER_RATE * dt) as u32;
                    let is_full = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.stored_resources.supplies)
                        .unwrap_or(0)
                        + gather_amount
                        >= MAX_CARRY;

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_stored_supplies(
                            obj.stored_resources
                                .supplies
                                .saturating_add(gather_amount)
                                .min(MAX_CARRY),
                        );
                    }

                    // Deplete the supply source.
                    if let Some(source) = self.objects.get_mut(&source_id) {
                        let taken = gather_amount.min(source.stored_resources.supplies);
                        source.set_stored_supplies(
                            source.stored_resources.supplies.saturating_sub(taken),
                        );
                        if source.stored_resources.supplies == 0 {
                            Self::mark_object_destroyed_authority_aware(source, None);
                            self.mark_object_for_destruction(source_id, None);
                        }
                    }

                    if is_full {
                        // Full — head to nearest supply center.
                        let refinery_dest = self
                            .find_nearest_supply_center(team, position)
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
                    // Deposit resources when close to a supply center.
                    let (refinery_id, refinery_pos) = self
                        .find_nearest_supply_center(team, position)
                        .and_then(|rid| {
                            self.objects
                                .get(&rid)
                                .map(|r| (Some(rid), r.get_position()))
                        })
                        .unwrap_or((None, position));

                    let at_refinery =
                        refinery_id.is_some() && position.distance(refinery_pos) <= INTERACT_RANGE;

                    if at_refinery {
                        // Deposit.
                        // C++ SupplyCenterDockUpdate::action: base box value +
                        // supplyTruckAI->getUpgradedSupplyBoost() when player has
                        // Upgrade_AmericaSupplyLines (Chinook residual).
                        let deposit_amount = self
                            .objects
                            .get(&object_id)
                            .map(|o| o.stored_resources.supplies)
                            .unwrap_or(0);

                        if deposit_amount > 0 {
                            // Snapshot carrier for residual boost identity (worker shoes).
                            let (
                                carrier_is_gla_worker,
                                carrier_has_worker_shoes,
                            ) = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    let is_w = crate::game_logic::host_gla_worker::is_gla_worker_template(
                                        &o.template_name,
                                    );
                                    let shoes = o.has_upgrade_tag(
                                        crate::game_logic::host_gla_worker::UPGRADE_GLA_WORKER_SHOES,
                                    ) || self.players.values().any(|p| {
                                        p.team == team
                                            && p.has_unlocked_upgrade(
                                                crate::game_logic::host_gla_worker::UPGRADE_GLA_WORKER_SHOES,
                                            )
                                    });
                                    (is_w, shoes)
                                })
                                .unwrap_or((false, false));

                            // Clear carried resources.
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_stored_supplies(0);
                            }
                            // Player-level Supply Lines residual boost (flat per drop-off).
                            let has_supply_lines = self
                                .players
                                .values()
                                .any(|p| {
                                    p.team == team
                                        && p.has_unlocked_upgrade(
                                            crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES,
                                        )
                                });
                            let supply_lines_boost =
                                crate::game_logic::host_upgrades::residual_supply_lines_drop_off_boost(
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
                            let credited_player_id =
                                self.players.iter().find_map(|(&player_id, player)| {
                                    (player.team == team).then_some(player_id)
                                });
                            if let Some(player_id) = credited_player_id {
                                let credited_player =
                                    if let Some(player) = self.get_player_mut(player_id) {
                                        player.credit_supplies(credited);
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
                                }
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
                            // Head back to gather more from the original source.
                            let source_dest = target_id.and_then(|sid| {
                                self.objects
                                    .get(&sid)
                                    .filter(|s| s.is_alive())
                                    .map(|s| s.get_position())
                            });
                            if let Some(dest) = source_dest {
                                self.path_approach_with_state(object_id, dest, AIState::Gathering);
                            } else if let Some(next) =
                                self.find_nearest_harvestable_supply(team, position)
                            {
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
                                self.stop_attack_decision_aware(object_id);
                                self.set_ai_state_decision_aware(object_id, AIState::Idle);
                            }
                        }
                    } else if can_move {
                        // Still heading to refinery.
                        self.path_approach_with_state(
                            object_id,
                            refinery_pos,
                            AIState::ReturningResources,
                        );
                    }
                }
                AIState::Docked | AIState::Garrisoned => {
                    // Aircraft parking: leave hangar when given a move/attack residual.
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
                        self.release_jet_from_airfield_parking(object_id);
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

                    let Some((container_pos, container_alive, container_has_unit)) =
                        self.objects.get(&container_id).map(|container| {
                            (
                                container.get_position(),
                                container.is_alive(),
                                container.contained_units().contains(&object_id),
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

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_contained_by(Some(container_id));
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
                        obj.stop_moving();
                        obj.set_status_moving(false);
                    }
                }
                _ => {}
            }
        }
    }
}
