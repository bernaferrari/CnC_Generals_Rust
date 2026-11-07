use super::*;

impl GameLogic {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn tick_entering_docking_support_state(
        &mut self,
        object_id: ObjectId,
        state: AIState,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        target_id: Option<ObjectId>,
        can_move: bool,
        selection_radius: f32,
        is_alive: bool,
        tree_move_frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(container_id) = target_id else {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
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
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.ignored_obstacle_id = Some(container_id);
                        }
                        self.path_approach_with_state_ignoring(
                            object_id,
                            vehicle_pos,
                            AIState::Entering,
                            Some(container_id),
                        );
                        return Ok(());
                    }
                    let health_bonus = self.veterancy_health_bonus;
                    let transferred = self
                        .with_object_recrew_inputs(
                            container_id,
                            |vehicle, ownership, audio, feedback, partition, locomotors| {
                                vehicle.apply_pilot_recrew_with_veterancy_rules(
                                    pilot_team,
                                    pilot_owner_player_id,
                                    pilot_level,
                                    ownership,
                                    audio,
                                    feedback,
                                    partition,
                                    &health_bonus,
                                    locomotors,
                                )
                            },
                        )
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
                    let msg = localization::localize("hud.pilot.recrew", "Vehicle recrewed");
                    self.queue_radar_message_for_team(pilot_team, msg);
                    self.mark_destroyed_authority_aware(object_id, None);
                    self.mark_object_for_destruction_with_effects(
                        object_id,
                        Some(pilot_team),
                        effects,
                    )?;
                    return Ok(());
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
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.ignored_obstacle_id = Some(container_id);
                    }
                    self.path_approach_with_state_ignoring(
                        object_id,
                        vehicle_pos,
                        AIState::Entering,
                        Some(container_id),
                    );
                    return Ok(());
                }
                let was_unmanned = self
                    .objects
                    .get(&container_id)
                    .is_some_and(|veh| veh.status.disabled_unmanned);
                if was_unmanned {
                    if let Some(veh) = self.objects.get_mut(&container_id) {
                        veh.set_status_disabled_unmanned(false);
                        veh.status.unmanned_owner_team = None;
                        veh.status.unmanned_owner_player_id = None;
                        veh.set_status_disabled_hacked(false);
                        veh.status.disabled_hacked_until_frame = 0;
                        veh.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                            &mut self.queued_audio_events,
                        ));
                        veh.target = None;
                    }
                    self.set_ai_state_decision_aware_with_effects(
                        container_id,
                        AIState::Idle,
                        effects,
                    )?;
                    if let Some(veh) = self.objects.get_mut(&container_id) {
                        veh.set_team_and_owner_with_partition_owner(
                            inf_team,
                            inf_owner,
                            &mut self.ownership_events,
                            &mut self.partition_manager,
                        );
                        veh.set_private_captured(true);
                    }
                }
                self.admit_host_radar_disabled_edges(container_id);
                let _ = self.transfer_script_object_name(object_id, container_id);
                self.unmanned_reclaims = self.unmanned_reclaims.saturating_add(1);
                self.mark_destroyed_authority_aware(object_id, None);
                self.mark_object_for_destruction_with_effects(object_id, Some(inf_team), effects)?;
                return Ok(());
            }
        }

        // Normal `MSG_ENTER` was already accepted by the command
        // executor, but the target can change while the unit walks
        // toward it.  Revalidate through the same centralized
        // ContainModule/owner/capacity authority at arrival.  Dock
        // deliberately stays on its separate state machine below.
        let normal_enter = state == AIState::Entering;
        let requested_container_id = container_id;
        let container_id = if normal_enter {
            self.redirected_containment_owner(container_id)
        } else {
            container_id
        };
        // A held passenger exits through the shared tracker; it
        // cannot be re-admitted at another entrance. C++
        // OpenContain::addToContain rejects already-contained
        // objects, while cross-entrance exit uses AI_EXIT.
        let shared_target = self.objects.get(&container_id).is_some_and(|container| {
            container.is_tunnel_network_style_container() || container.is_cave_style_container()
        });
        if shared_target
            && (self
                .objects
                .get(&object_id)
                .is_some_and(|rider| rider.contained_by.is_some())
                || self.tunnel_network.player_holding_unit(object_id).is_some()
                || self.cave_system.index_holding_unit(object_id).is_some())
        {
            return Ok(());
        }
        // C++ OpenContain::onCollide ejects foreign riders first,
        // then isValidContainerFor + addToContain.
        if normal_enter {
            self.eject_foreign_occupants_on_enter_with_effects(container_id, object_id, effects)?;
        }
        if normal_enter && !self.can_unit_enter_normal_target(object_id, requested_container_id) {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
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
                container
                    .contained_by
                    .is_some_and(|outer| self.redirected_containment_owner(outer) == container_id),
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
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
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
                (o.is_kind_of(KindOf::Infantry) || self.host_object_is_hero(o))
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
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
                return Ok(());
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
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
        }
        // Combat Chinook ForbidInsideKindOf = AIRCRAFT HUGE_VEHICLE residual.
        if !normal_enter
            && container_is_combat_chinook
            && (unit_is_aircraft || unit_is_huge_vehicle)
        {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
        }

        if !can_move
            || !container_is_alive
            || container_under_construction
            || !container_can_contain
        {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
        }

        if !normal_enter
            && container_is_tunnel_network
            && matches!(
                (owner_player_id, container_owner_player_id),
                (Some(a), Some(b)) if a != b
            )
        {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
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
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
        }

        let enter_range = selection_radius + container_radius + 4.0;
        if can_move && position.distance(container_pos) > enter_range {
            if matches!(state, AIState::Entering) {
                self.path_approach_with_state_ignoring(
                    object_id,
                    container_pos,
                    state,
                    Some(container_id),
                );
            } else {
                self.path_approach_with_state(object_id, container_pos, state);
            }
            return Ok(());
        }

        // RiderChangeContain is an atomic replacement, never a
        // generic one-slot `add_occupant`.  Keep every parsed
        // RiderChange target on this authoritative branch so an
        // unsupported/custom roster cannot fall through to the
        // legacy Combat Cycle template-name refresh below.
        let is_rider_change_target = normal_enter
            && self.objects.get(&container_id).is_some_and(|container| {
                container.thing().template.contain_module.kind
                    == crate::game_logic::ContainModuleKind::RiderChange
            });
        if is_rider_change_target {
            if !self.rider_change_enter_at_arrival_with_effects(object_id, container_id, effects)? {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
            }
            return Ok(());
        }

        // Tunnel shared capacity (MaxTunnelCapacity=10) overrides local space.
        let tunnel_player = crate::game_logic::host_tunnel_network::tunnel_system_key(
            container_owner_player_id,
            container_team,
        );
        let tunnel_has_space = if container_is_tunnel_network {
            self.tunnel_network.has_capacity(tunnel_player)
        } else if container_is_cave {
            let idx = self
                .objects
                .get(&container_id)
                .map(|c| c.cave_index)
                .unwrap_or(0);
            self.cave_system.has_capacity(idx)
        } else {
            true
        };
        // C++ OpenContain::onCollide: eject other-player riders first.
        if !container_is_tunnel_network && !container_is_cave {
            self.kick_other_controller_occupants_for_enter_with_effects(
                container_id,
                object_id,
                effects,
            );
        }
        let can_enter = if container_is_tunnel_network || container_is_cave {
            // The tracker owns admission; a stale or empty local
            // mirror cannot make a full shared pool accept a rider.
            tunnel_has_space
        } else {
            let space_after_kick = self
                .objects
                .get(&container_id)
                .is_some_and(|c| c.has_capacity_for(1) || c.contained_units().contains(&object_id));
            container_has_unit || container_has_space || space_after_kick
        };
        if !can_enter {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
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
                return Ok(());
            }
        }

        // C++ OpenContain::addToContain checkAndDetonateBoobyTrap(rider).
        if !container_has_unit
            && self.should_cancel_containment_after_booby_trap_with_effects(
                container_id,
                object_id,
                effects,
            )
        {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
            return Ok(());
        }

        // GarrisonContain's later onContaining callback changes
        // the building to the first rider's receiving default
        // Team. Validate that required destination before the
        // irreversible roster/contained-by transition below.
        if container_is_structure {
            self.validate_garrison_owner_team_transfer(container_id, object_id)?;
        }

        let entered = if container_has_unit {
            true
        } else {
            self.objects
                .get_mut(&container_id)
                .map(|container| {
                    if container_is_tunnel_network || container_is_cave {
                        container.add_shared_tracker_occupant(object_id)
                    } else {
                        container.add_occupant(object_id)
                    }
                })
                .unwrap_or(false)
        };
        if !entered {
            return Ok(());
        }

        // Shared pool bookkeeping for tunnel residual.
        if container_is_tunnel_network {
            if !self
                .tunnel_network
                .record_enter(tunnel_player, object_id, container_id)
            {
                // Capacity race: undo local occupant add.
                if let Some(container) = self.objects.get_mut(&container_id) {
                    container.remove_occupant_with_partition_owner(
                        object_id,
                        &mut self.ownership_events,
                        &mut self.partition_manager,
                    );
                }
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
                return Ok(());
            }
        }
        if container_is_cave {
            let idx = self
                .objects
                .get(&container_id)
                .map(|c| c.cave_index)
                .unwrap_or(0);
            let (ok, ev) = self
                .cave_system
                .record_enter(idx, object_id, container_id, team);
            if !ok {
                if let Some(container) = self.objects.get_mut(&container_id) {
                    container.remove_occupant_with_partition_owner(
                        object_id,
                        &mut self.ownership_events,
                        &mut self.partition_manager,
                    );
                }
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                    obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                        &mut self.queued_audio_events,
                    ));
                }
                return Ok(());
            }
            self.apply_cave_capture_event_with_effects(idx, ev, container_id, effects)?;
        }
        self.stamp_player_who_entered(container_id, object_id);
        // C++ OpenContain::addToContain doLoadSound — leftover TheAudio.
        self.play_container_enter_sound(container_id, effects);

        let container_is_heal_contain = self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.thing().template.contain_module.kind.is_heal_contain());
        self.tunnel_network
            .stamp_contained_by_frame(object_id, self.frame);

        let enclosing_garrison = self
            .objects
            .get(&container_id)
            .map(|c| c.is_enclosing_garrison_container())
            .unwrap_or(true);
        let occupant_owner = self.objects.get(&object_id).and_then(|o| o.owner_player_id);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            obj.set_status_attacking(false);
            obj.target_location = None;
            obj.set_status_force_attack(false);
            obj.target = Some(container_id);
            let containment_actor_id = obj.id;
            Self::set_owned_containment_in_parts(
                &mut self.objects,
                &mut self.partition_manager,
                &mut self.ai_groups,
                &mut self.pathfinding_system,
                containment_actor_id,
                Some(container_id),
                enclosing_garrison,
            );
            // C++ onContaining snaps enclosing occupants to the building
            // origin. Fire Base (IsEnclosingContainer=No) stays at stations.
            if enclosing_garrison || !container_is_structure {
                if !self.set_owned_object_position_with_effects(
                    containment_actor_id,
                    container_pos,
                    tree_move_frame,
                    effects,
                ) {
                    return Ok(());
                }
            }
            let Some(obj) = self.objects.get_mut(&containment_actor_id) else {
                return Ok(());
            };
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
            {}
            obj.set_ai_state(
                __ai_st,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            obj.set_status_moving(false);
            // C++ GarrisonContain::onContaining TheInGameUI->deselectDrawable.
            if container_is_structure {
                obj.deselect();
            }
        }
        self.admit_object_drawable_deselection(object_id);
        if container_is_tunnel_network {
            // Enter counter already incremented in record_enter.
        } else if container_is_heal_contain {
            // C++ HealContain is not a garrison / transport load.
        } else if container_is_structure {
            self.record_garrison_residual_enter();
            self.apply_garrison_contain_on_enter_with_effects(container_id, object_id, effects)?;
            if let Some(pid) = occupant_owner {
                if let Some(player) = self.players.get_mut(&pid) {
                    player.selected_objects.retain(|id| *id != object_id);
                }
            }
            self.admit_local_drawable_deselected(object_id);
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
        Ok(())
    }
}
