use super::*;

impl GameLogic {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn tick_repairing_support_state(
        &mut self,
        object_id: ObjectId,
        position: Vec3,
        target_id: Option<ObjectId>,
        can_move: bool,
        selection_radius: f32,
        is_alive: bool,
        dt: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(repair_target_id) = target_id else {
            self.release_dozer_repair_order(object_id)?;
            return Ok(());
        };

        let actor_can_repair = self
            .objects
            .get(&object_id)
            .map(|obj| {
                obj.can_repair()
                    && obj.is_alive()
                    && !obj.status.effectively_dead
                    && obj.contained_by.is_none()
                    && !obj.status.under_construction
            })
            .unwrap_or(false);
        if !actor_can_repair {
            self.release_dozer_repair_order(object_id)?;
            return Ok(());
        }

        let Some((
            repair_target_pos,
            repair_target_selection_radius,
            repair_target_alive,
            repair_target_is_structure,
            repair_target_under_construction,
        )) = self.objects.get(&repair_target_id).map(|target| {
            (
                target.get_position(),
                target.selection_radius,
                target.is_alive() && !target.status.effectively_dead,
                target.is_kind_of(KindOf::Structure),
                target.status.under_construction,
            )
        })
        else {
            self.release_dozer_repair_order(object_id)?;
            return Ok(());
        };

        let bridge_or_hole = self.objects.get(&repair_target_id).is_some_and(|target| {
            target.is_kind_of(KindOf::Bridge)
                || target.is_kind_of(KindOf::BridgeTower)
                || target.is_rebuild_hole
        });
        if bridge_or_hole
            || !repair_target_alive
            || !repair_target_is_structure
            || repair_target_under_construction
            || !self.repair_relationship_is_not_enemy(object_id, repair_target_id)
        {
            self.release_dozer_repair_order(object_id)?;
            return Ok(());
        }

        let (dozer_radius, airborne) = self
            .objects
            .get(&object_id)
            .map(|obj| {
                (
                    obj.selection_radius,
                    obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                )
            })
            .unwrap_or((0.0, false));
        let dock = self.find_good_build_or_repair_position(
            position,
            repair_target_pos,
            repair_target_selection_radius,
            airborne,
            airborne.then_some(repair_target_id),
            Some(object_id),
        );
        let at_dock =
            crate::game_logic::host_repair::dozer_within_action_dock(position, dozer_radius, dock);
        if !at_dock {
            let has_valid_active_approach_path = self.objects.get(&object_id).is_some_and(|obj| {
                obj.status.moving
                    && obj.movement.current_path_index < obj.movement.path.len()
                    && obj.movement.path.last().is_some_and(|endpoint| {
                        crate::game_logic::host_repair::dozer_within_action_dock(
                            *endpoint,
                            dozer_radius,
                            dock,
                        )
                    })
            });
            if can_move && !has_valid_active_approach_path {
                if let Some(dozer) = self.objects.get_mut(&object_id) {
                    dozer.set_adjust_destinations(false);
                }
                self.path_approach_with_state_ignoring(
                    object_id,
                    dock,
                    AIState::Repairing,
                    Some(repair_target_id),
                );
            }
            // Never heal remotely. This also keeps a valid route
            // in flight instead of falling through to the repair
            // effect while still out of range.
            return Ok(());
        }
        if let Some(dozer) = self.objects.get_mut(&object_id) {
            dozer.movement.path.clear();
            dozer.movement.current_path_index = 0;
            dozer.movement.target_position = None;
            dozer.waiting_for_path = false;
            dozer.set_status_moving(false);
            dozer.set_locomotor_goal_none();
        }

        if self.objects.get(&repair_target_id).is_some_and(|target| {
            target.health.maximum > 0.0 && target.health.current >= target.health.maximum
        }) {
            let msg = localization::localize("DOZER:RepairComplete", "Repair complete");
            self.queue_radar_message_at(
                msg,
                repair_target_pos,
                radar_notifications::RadarKind::Generic,
            );
            self.repair_complete_events = self.repair_complete_events.saturating_add(1);
            self.release_dozer_repair_order(object_id)?;
            return Ok(());
        }

        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.set_actively_constructing(true);
        }

        // Dozer structure-repair residual: heal HP over time while in range.
        // C++ DozerAIUpdate.cpp:694-699 percent heal, no 8.75 HP/s floor.
        // C++ DozerAIUpdate.cpp:670: ACTIVELY_CONSTRUCTING only at the dock.
        let max_hp = self
            .objects
            .get(&repair_target_id)
            .map(|t| t.health.maximum)
            .unwrap_or(0.0);
        let heal_per_sec = crate::game_logic::host_repair::dozer_repair_hp_per_sec(max_hp);
        let heal_amount = heal_per_sec * dt;
        // C++ attemptHealingFromSoleBenefactor(health, dozer, 2) residual.
        let now = self.frame;
        let healed = self.attempt_owned_healing_from_sole_benefactor_with_effects(
            repair_target_id,
            heal_amount,
            object_id,
            2,
            now,
            effects,
        );
        let sole = self.objects.get(&repair_target_id).map(|target| {
            (
                target.health.current >= target.health.maximum,
                healed,
                target.get_position(),
            )
        });
        let (target_full, healed, repair_pos) = match sole {
            Some(v) => v,
            None => {
                self.release_dozer_repair_order(object_id)?;
                return Ok(());
            }
        };
        if !healed && !target_full {
            // Another dozer owns sole-benefactor claim — cancel this dozer task.
            self.release_dozer_repair_order(object_id)?;
            self.sole_benefactor_repair_rejects =
                self.sole_benefactor_repair_rejects.saturating_add(1);
            return Ok(());
        }
        if healed {
            self.record_structure_repair_residual_heal();
        }
        if target_full {
            // C++ DOZER:RepairComplete residual.
            let msg = localization::localize("DOZER:RepairComplete", "Repair complete");
            self.queue_radar_message_at(msg, repair_pos, radar_notifications::RadarKind::Generic);
            self.repair_complete_events = self.repair_complete_events.saturating_add(1);
            self.release_dozer_repair_order(object_id)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn tick_seeking_repair_support_state(
        &mut self,
        object_id: ObjectId,
        ai_state: AIState,
        state: AIState,
        position: Vec3,
        target_id: Option<ObjectId>,
        can_move: bool,
        health_current: f32,
        health_maximum: f32,
        selection_radius: f32,
        is_alive: bool,
        dt: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if matches!(state, AIState::SeekingHealing) && health_current >= health_maximum - 0.01 {
            if let Some(tid) = target_id {
                self.release_dock_if_holder(tid, object_id);
            }
            let rallied = self.objects.get(&object_id).is_some_and(|obj| {
                matches!(obj.ai_state(), AIState::Moving | AIState::AttackMoving)
            });
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.clear_order_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                if !rallied {
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.ignored_obstacle_id = None;
                }
            }
            if !rallied {
                self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
            }
            return Ok(());
        }

        let Some(support_target_id) = target_id else {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.ignored_obstacle_id = None;
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
            return Ok(());
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
                target.is_kind_of(KindOf::RepairPad)
                    && target
                        .thing()
                        .template
                        .repair_dock_frames_for_full_heal
                        .is_some(),
                target.is_kind_of(KindOf::HealPad),
                target.is_kind_of(KindOf::FSAirfield),
            )
        })
        else {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.ignored_obstacle_id = None;
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
            return Ok(());
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
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.ignored_obstacle_id = None;
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
            return Ok(());
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
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.ignored_obstacle_id = None;
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
            return Ok(());
        }

        if position.distance(support_target_pos) > INTERACT_RANGE {
            // Keep a moving, target-valid A* path rather than
            // restarting it every frame. Re-path only after the
            // endpoint ceased to be a viable interaction point or
            // the mover stopped, retaining obstacle recovery.
            let has_valid_active_approach_path = self.objects.get(&object_id).is_some_and(|obj| {
                obj.status.moving
                    && obj.movement.current_path_index < obj.movement.path.len()
                    && obj.movement.path.last().is_some_and(|endpoint| {
                        endpoint.distance(support_target_pos) <= INTERACT_RANGE
                    })
            });
            if can_move && !has_valid_active_approach_path {
                let approach = crate::game_logic::host_repair::support_approach_position(
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

            return Ok(());
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
            self.try_aircraft_land_for_repair_with_effects(object_id, support_target_id, effects);
            return Ok(());
        }

        // Pad/war-factory: C++ RepairDockUpdate::action
        // TimeForFullHeal. One activeDocker; rate computed once
        // from missing HP so Humvee ≠ Overlord.
        if matches!(state, AIState::SeekingRepair)
            && !(self.try_claim_dock(support_target_id, object_id)?)
        {
            return Ok(());
        }
        let seeking_repair = matches!(state, AIState::SeekingRepair);
        let repair_continues = !seeking_repair
            || self.repair_dock_action_owned_with_effects(support_target_id, object_id, effects);
        let mut heal_pad_healed = false;
        let heal_pad_before = if seeking_repair {
            None
        } else {
            self.objects.get(&object_id).map(|obj| obj.health.current)
        };
        if let Some(before) = heal_pad_before {
            if self
                .apply_owned_healing_with_effects(object_id, HEAL_RATE * dt, None, effects)
                .is_none()
            {
                // A synchronous body callback removed or replaced
                // this source. Do not continue this iteration with
                // a different incarnation under the same ObjectId.
                return Ok(());
            }
            heal_pad_healed = self
                .objects
                .get(&object_id)
                .is_some_and(|obj| obj.health.current > before + 0.0001);
        }
        let mut vehicle_healed = false;
        if let Some(obj) = self.objects.get_mut(&object_id) {
            let before = if seeking_repair {
                health_current
            } else {
                obj.health.current
            };
            let healed = seeking_repair && obj.health.current > before + 0.0001;
            vehicle_healed = healed && seeking_repair;
            // RepairDockUpdate::action returns TRUE on the pulse that
            // reaches max health. Only the next action completes docking.
            if (seeking_repair && !repair_continues)
                || (!seeking_repair && obj.health.current >= obj.health.maximum - 0.01)
            {
                obj.clear_order_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            } else if !seeking_repair {
                obj.set_ai_state(
                    state,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
        }
        if seeking_repair && !repair_continues {
            let passthrough = self.objects.get(&support_target_id).is_some_and(|dock| {
                crate::game_logic::host_dock_contain_exit_heal_residual::dock_allows_passthrough(
                    &dock.template_name,
                )
            });
            if passthrough {
                if let Some(docker) = self.objects.get_mut(&object_id) {
                    docker.ignored_obstacle_id = Some(support_target_id);
                }
            }
            self.send_to_rally_after_repair_dock(object_id, support_target_id)?;
            self.release_dock_if_holder(support_target_id, object_id);
            let rallied = self.objects.get(&object_id).is_some_and(|obj| {
                matches!(obj.ai_state(), AIState::Moving | AIState::AttackMoving)
            });
            if !rallied {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.ignored_obstacle_id = None;
                }
                self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
            }
        }

        if vehicle_healed {
            self.record_vehicle_repair_residual_heal();
        }
        if heal_pad_healed {
            self.record_heal_pad_residual_heal();
        }
        Ok(())
    }
}
