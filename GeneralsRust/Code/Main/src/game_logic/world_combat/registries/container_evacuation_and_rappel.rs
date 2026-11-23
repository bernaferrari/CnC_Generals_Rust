//! Container evacuation, rappel and garrison eject flows.

use super::*;

impl GameLogic {
    pub(crate) fn flush_pending_garrison_really_damaged_ejects_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, obj)| obj.status.pending_garrison_really_damaged_eject)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                let _ = obj.take_pending_garrison_really_damaged_eject();
            }
            self.evacuate_container_now_with_effects(id, false, presentation_effects)?;
        }
        Ok(())
    }

    /// C++ move-to-and-evacuate arrival residual: dump all occupants near container.
    /// When `and_exit`, mark the transport sold/destroyed after unload (script exit residual).
    pub fn evacuate_container_now(&mut self, container_id: ObjectId, and_exit: bool) -> gamelogic::GameLogicResult<bool> {
        self.with_own_services_mut(|this| {
            this.evacuate_container_now_with_effects(
                container_id,
                and_exit,
                &mut crate::game_logic::ScriptPresentationEffects::default(),
            )
        })
    }

    pub(crate) fn evacuate_container_now_with_effects(
        &mut self,
        container_id: ObjectId,
        and_exit: bool,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if and_exit
            && !self.host_object(container_id).is_some_and(|object| {
                object.chinook_ai.is_some() || object.transport_return_state.is_some()
            })
        {
            // Return/delete needs the origin recorded by the actual command.
            // An arbitrary contain call cannot invent that state or a death.
            return Ok(false);
        }
        let effective = self.redirected_containment_owner(container_id);
        if effective != container_id {
            if !self
                .host_object(container_id)
                .is_some_and(|outer| outer.is_alive() && !outer.is_subdued_disabled())
            {
                return Ok(false);
            }
            let evacuated = self.evacuate_transport_with_effects(
                effective,
                false,
                TransportEvacuationMode::Command,
                presentation_effects,
            )?;
            let empty = self
                .host_object(effective)
                .is_some_and(|object| object.contained_units().is_empty());
            if and_exit && empty {
                self.finish_transport_return_phase_with_effects(container_id, presentation_effects)?;
            }
            return Ok(evacuated);
        }
        self.evacuate_transport_with_effects(
            container_id,
            and_exit,
            TransportEvacuationMode::Command,
            presentation_effects,
        )
    }

    /// Apply the owned Chinook evacuation state's immediate contain effect.
    /// Flight transitions may succeed only after the actual contain list is empty.
    pub(in super::super::super) fn evacuate_chinook_flight(
        &mut self,
        container_id: ObjectId,
    ) -> gamelogic::GameLogicResult<bool> {
        self.evacuate_chinook_flight_with_effects(
            container_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super::super) fn evacuate_chinook_flight_with_effects(
        &mut self,
        container_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(container_id_birth) = self
            .objects
            .get(&container_id)
            .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token))
        else {
            return Ok(false);
        };

        use crate::game_logic::host_combat_chinook::HostChinookAIState;
        if !self.objects.get(&container_id).is_some_and(|obj| {
            obj.is_alive()
                && obj.chinook_ai.as_ref().is_some_and(|ai| {
                    matches!(
                        ai.state,
                        HostChinookAIState::EvacAndTakeoff | HostChinookAIState::EvacAndExit
                    )
                })
        }) {
            return Ok(false);
        }
        self.evacuate_transport_with_effects(
            container_id,
            false,
            TransportEvacuationMode::ChinookFlight,
            presentation_effects,
        )?;
        if !self.objects.get(&container_id).is_some_and(|object| {
            container_id_birth.ptr_eq(&std::sync::Arc::downgrade(
                &object.visual_draw_identity_token,
            ))
        }) {
            return Ok(false);
        }
        let empty = self
            .objects
            .get(&container_id)
            .is_some_and(|obj| obj.contained_units().is_empty());
        if empty {
            // ChinookEvacuateState::onEnter activates the owner's team.
            self.activate_leftover_team_for_host_object_with_effects(
                container_id,
                presentation_effects,
            );
        }
        Ok(empty)
    }

    pub(super) fn evacuate_transport(
        &mut self,
        container_id: ObjectId,
        and_exit: bool,
        mode: TransportEvacuationMode,
    ) -> gamelogic::GameLogicResult<bool> {
        self.evacuate_transport_with_effects(
            container_id,
            and_exit,
            mode,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(super) fn evacuate_transport_with_effects(
        &mut self,
        container_id: ObjectId,
        and_exit: bool,
        mode: TransportEvacuationMode,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(container_id_birth) = self
            .objects
            .get(&container_id)
            .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token))
        else {
            return Ok(false);
        };

        let Some((alive, is_chinook_dropper, subdued)) = self.objects.get(&container_id).map(|c| {
            (
                c.is_alive(),
                c.is_combat_chinook_style_container() && c.chinook_ai.is_some(),
                c.is_subdued_disabled(),
            )
        }) else {
            return Ok(false);
        };
        if !alive {
            return Ok(false);
        }
        // C++ AIUpdateInterface::privateEvacuate / privateExit:
        // DISABLED_SUBDUED holds the doors shut while a Microwave cooks.
        if subdued && mode == TransportEvacuationMode::Command {
            return Ok(false);
        }
        let rail_ai = self.objects.get(&container_id).is_some_and(|object| {
            object.thing().template.receiving_client_template.as_ref()
                .and_then(|template| template.receiving_ai_module_name())
                .is_some_and(|name| name.eq_ignore_ascii_case("RailedTransportAIUpdate"))
        });
        if rail_ai && mode == TransportEvacuationMode::Command {
            return self.railed_transport_unload_all_with_effects(container_id, presentation_effects);
        }
        if mode == TransportEvacuationMode::Command && self.objects.get(&container_id).is_some_and(|object| {
            object.thing().template.contain_module.kind == crate::game_logic::ContainModuleKind::RailedTransport
        }) {
            if !self.railed_dock_reserve_exit(container_id) { return Ok(false); }
            let Some(config) = self.railed_dock_config(container_id) else { return Ok(false) };
            let riders = self.objects.get(&container_id).unwrap().contained_units();
            let mut any = false;
            for rider in riders {
                if self.objects.get(&rider).is_some_and(|object| object.has_ai_update_interface()) {
                    self.railed_dock_unload(container_id, config, false, presentation_effects)
                        .map_err(gamelogic::GameLogicError::Configuration)?;
                    any = true;
                }
            }
            return Ok(any);
        }
        // C++ privateEvacuate: markAllPassengersDetected before dump.
        if mode == TransportEvacuationMode::Command {
            self.mark_all_passengers_detected(container_id);
        }

        if is_chinook_dropper && mode == TransportEvacuationMode::Command {
            if let Some(c) = self.objects.get_mut(&container_id) {
                let p = c.get_position();
                if let Some(ai) = c.chinook_ai.as_mut() {
                    ai.pos = [p.x, p.z, p.y];
                    if ai.state
                        == crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToCombatDrop
                    {
                        ai.arrive_for_combat_drop();
                    }
                }
            }
            let doing_drop = self.objects.get(&container_id).is_some_and(|c| {
                c.chinook_ai.as_ref().is_some_and(|ai| {
                    ai.flight_status
                        == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::DoingCombatDrop
                })
            });
            if doing_drop {
                return self.combat_drop_rappel_unload(
                    container_id,
                    and_exit,
                    presentation_effects,
                );
            }
            let any_rappeller = self.objects.get(&container_id).is_some_and(|c| {
                c.contained_units().iter().any(|pid| {
                    self.objects.get(pid).is_some_and(|p| {
                        crate::game_logic::host_combat_chinook::HostChinookAI::passenger_kind_can_rappel(
                            p.is_kind_of(KindOf::Infantry),
                        )
                    })
                })
            });
            let wait = self.objects.get(&container_id).is_some_and(|c| {
                c.chinook_ai.as_ref().is_some_and(|ai| {
                    ai.ai_free_to_exit(any_rappeller)
                        != crate::game_logic::host_combat_chinook::HostChinookFreeToExit::FreeToExit
                })
            });
            if wait {
                if let Some(c) = self.objects.get_mut(&container_id) {
                    let p = c.get_position();
                    let contained = c.contained_units().len() as u32;
                    if let Some(ai) = c.chinook_ai.as_mut() {
                        ai.pos = [p.x, p.z, p.y];
                        ai.wanting_enter_or_exit = true;
                        ai.parent_idle = true;
                        ai.contained_count = contained;
                        if and_exit {
                            ai.command_evac([p.x, p.z, 0.0], true);
                        } else {
                            ai.tick_idle_auto_land();
                        }
                    }
                    c.pending_evacuate_on_stop = true;
                    c.pending_exit_after_evacuate = and_exit;
                }
                return Ok(false);
            }
        }

        let Some(container) = self.objects.get(&container_id) else {
            return Ok(false);
        };
        let is_garrison = container.is_garrison_contain();
        let is_tunnel = container.is_tunnel_network_style_container();
        let is_cave = container.is_cave_style_container();
        let Some(container) = self.objects.get(&container_id) else {
            return Ok(false);
        };
        let evac = if is_garrison {
            container.garrison_evac_disposition()
        } else {
            0
        };
        let enclosing = container.is_enclosing_garrison_container();
        let geom = (*container.geometry_info());
        let major = if geom.authored {
            geom.major_radius.max(1.0)
        } else {
            20.0
        };
        let minor = if geom.authored {
            geom.minor_radius.max(1.0)
        } else {
            20.0
        };
        let yaw = container.get_orientation();
        let building_pos = container.get_position();
        let mut passengers = self.contained_units_at(container);
        if passengers.is_empty() && !and_exit {
            // Still clear pending flags.
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.pending_evacuate_on_stop = false;
                c.pending_exit_after_evacuate = false;
                c.pending_stream_exit = false;
            }

            return Ok(false);
        }
        let uses_exit_busy = mode == TransportEvacuationMode::Command
            && !is_garrison
            && self
                .objects
                .get(&container_id)
                .is_some_and(|c| c.uses_transport_contain_exit_busy());
        if uses_exit_busy
            && self
                .objects
                .get(&container_id)
                .is_some_and(|c| c.is_transport_exit_busy(self.frame))
        {
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.pending_evacuate_on_stop = true;
                c.pending_exit_after_evacuate = and_exit;
            }
            return Ok(false);
        }
        let stagger = uses_exit_busy
            && self
                .objects
                .get(&container_id)
                .is_some_and(|c| c.transport_exit_delay_frames() > 0);
        let more_remain = stagger && passengers.len() > 1;
        let container_is_combat_chinook = self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_combat_chinook_style_container());
        let container_is_battle_bus = self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_battle_bus_style_container());
        let container_is_troop_crawler = self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_troop_crawler_style_container());
        if stagger && passengers.len() > 1 {
            passengers.truncate(1);
        }

        // C++ burst start is cliff-adjusted building origin; dest is then
        // adjustToPossibleDestination so occupants walk out (not Idle at center).
        let (burst_start, burst_end) = if is_garrison && evac != 1 && evac != 2 {
            let mut start =
                garrison_evac_cliff_start(building_pos, yaw, major, &self.pathfinding_system.grid);
            if enclosing {
                if let Some(gh) = self.terrain_height_at(start) {
                    start.y = gh;
                }
            }
            let end = {
                let owner = self.world_services.terrain();
                let loan = owner.read().unwrap_or_else(|error| error.into_inner());
                let ground = PathfindingGround::for_terrain(
                    &loan,
                    self.map_loaded,
                    self.terrain.as_ref(),
                    self.pathfinding_height_samples.as_ref(),
                    &self.pathfinding_system.grid,
                );
                garrison_evac_burst_end(start, major, &self.pathfinding_system.grid, &ground)
            };
            (Some(start), Some(end))
        } else {
            (None, None)
        };

        // Identity receipts, not mutable passenger copies: earlier riders may
        // execute authored FX before this original ordered roster reaches a peer.
        let passenger_births: std::collections::HashMap<_, _> = passengers
            .iter()
            .filter_map(|id| {
                self.objects.get(id).map(|object| {
                    (
                        *id,
                        std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                    )
                })
            })
            .collect();
        let mut any = false;
        let mut packing_hackers: Vec<ObjectId> = Vec::new();
        for (i, pid) in passengers.iter().enumerate() {
            if !self.objects.get(&container_id).is_some_and(|object| {
                container_id_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            }) {
                return Ok(false);
            }
            if !self.objects.get(pid).is_some_and(|object| {
                passenger_births.get(pid).is_some_and(|birth| {
                    birth.ptr_eq(&std::sync::Arc::downgrade(
                        &object.visual_draw_identity_token,
                    ))
                })
            }) {
                continue;
            }
            let mut walked_transport = false;
            let mut side_end: Option<glam::Vec3> = None;
            if is_cave || is_tunnel {
                let exited = if is_cave {
                    self.exit_cave_unit_with_effects(*pid, container_id, presentation_effects)?
                } else {
                    self.exit_tunnel_network_unit_with_effects(*pid, container_id, presentation_effects)
                };
                if !exited {
                    continue;
                }
                if !self.objects.get(pid).is_some_and(|object| {
                    passenger_births.get(pid).is_some_and(|birth| {
                        birth.ptr_eq(&std::sync::Arc::downgrade(&object.visual_draw_identity_token))
                    })
                }) {
                    continue;
                }
            } else {
                self.remove_owned_occupant_with_bonus_callback(container_id, *pid, presentation_effects);
            }
            if let Some(p) = self.objects.get_mut(pid) {
                // Wave 201: host_contain_log last-writer (do not bypass set_contained_by).
                let containment_actor_id = p.id;
                if !is_cave && !is_tunnel {
                    Self::set_owned_containment_in_parts(
                        &mut self.objects,
                        &mut self.partition_manager,
                        &mut self.ai_groups,
                        &mut self.pathfinding_system,
                        containment_actor_id,
                        None,
                        true,
                    );
                }
                let mut p = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                p.target = None;
                if is_garrison && (evac == 1 || evac == 2) {
                    let seed = pid.0.wrapping_add(i as u32).wrapping_add(self.frame);
                    let (start, end) =
                        garrison_evac_side_points(building_pos, yaw, major, minor, evac, seed);
                    let _ = p;
                    if !self.set_owned_object_position_with_effects(
                        containment_actor_id,
                        start,
                        self.frame,
                        presentation_effects,
                    ) {
                        continue;
                    }
                    if !self.objects.get(&container_id).is_some_and(|object| {
                        container_id_birth.ptr_eq(&std::sync::Arc::downgrade(
                            &object.visual_draw_identity_token,
                        ))
                    }) {
                        return Ok(false);
                    }
                    p = match self.objects.get_mut(&containment_actor_id) {
                        Some(object) => object,
                        None => continue,
                    };
                    p.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
                    side_end = Some(end);
                } else if is_garrison {
                    // C++ EVAC_BURST_FROM_CENTER: enclosing occupants snap to
                    // start (Y to ground), then aiFollowPath to adjusted dest.
                    if enclosing {
                        if let Some(start) = burst_start {
                            let _ = p;
                            if !self.set_owned_object_position_with_effects(
                                containment_actor_id,
                                start,
                                self.frame,
                                presentation_effects,
                            ) {
                                continue;
                            }
                            if !self.objects.get(&container_id).is_some_and(|object| {
                                container_id_birth.ptr_eq(&std::sync::Arc::downgrade(
                                    &object.visual_draw_identity_token,
                                ))
                            }) {
                                return Ok(false);
                            }
                            p = match self.objects.get_mut(&containment_actor_id) {
                                Some(object) => object,
                                None => continue,
                            };
                        }
                    }
                    p.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
                    if let Some(end) = burst_end {
                        p.movement.path.clear();
                        p.movement.path.push(end);
                        p.movement.path_cursor = p.movement.path.cursor_start();
                        p.is_attack_path = false;
                        p.can_path_through_units = false;
                        p.last_command_source =
                            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
                        p.refresh_follow_path_extra_distance();
                        p.set_adjust_destinations(!p.ultra_accurate);
                        p.set_destination_with_operation(
                            end,
                            &mut self.movement_events.writer(self.frame),
                            &mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ),
                            self.frame,
                            &mut presentation_effects
                                .reborrow_for_world(&mut self.partition_manager, &self.host_trigger_world),
                        );
                    }
                    p.set_ai_state(
                        AIState::Moving,
                        self.frame,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                    p.status.moving = true;
                } else {
                    // C++ OpenContain::exitObjectViaDoor — walk ExitStart/End.
                    // Do not teleport to an Idle 8-unit ring.
                    walked_transport = true;
                }
                if is_garrison || is_tunnel || is_cave {
                    // C++ GarrisonContain/TunnelContain/CaveContain::onRemoving.
                    p.stamp_safe_occlusion_frame(self.frame);
                }

                // C++ HackInternetAIUpdate::aiDoCommand (HackInternetAIUpdate.cpp:105)
                // PACKING on evacuate/exit. Cash must stop immediately.
                if p.thing().template.hack_internet_ai_update.is_some() {
                    packing_hackers.push(*pid);
                }
                any = true;
            }
            if let Some(mut end) = side_end {
                // GarrisonContain.cpp:1548 adjust after setPosition, then follow.
                let _ = self.adjust_to_possible_destination(*pid, &mut end);
                if let Some(p) = self.objects.get_mut(pid) {
                    p.movement.path.clear();
                    p.movement.path.push(end);
                    p.movement.path_cursor = p.movement.path.cursor_start();
                    p.is_attack_path = false;
                    p.can_path_through_units = false;
                    p.last_command_source =
                        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
                    p.refresh_follow_path_extra_distance();
                    p.set_adjust_destinations(!p.ultra_accurate);
                    p.set_destination_with_operation(
                        end,
                        &mut self.movement_events.writer(self.frame),
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                        self.frame,
                        &mut presentation_effects.reborrow_for_world(&mut self.partition_manager, &self.host_trigger_world),
                    );
                    p.set_ai_state(
                        AIState::Moving,
                        self.frame,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                    p.status.moving = true;
                }
            }
            self.resolve_installed_path_layers(*pid);
            if is_garrison {
                if let Some(goal) = self
                    .objects
                    .get(pid)
                    .and_then(|p| p.movement.target_position)
                {
                    self.register_path_goal(*pid, goal);
                }
            }
            if walked_transport {
                self.walk_unit_via_open_contain_exit_with_effects(
                    *pid,
                    container_id,
                    presentation_effects,
                );
                if !self.objects.get(&container_id).is_some_and(|object| {
                    container_id_birth.ptr_eq(&std::sync::Arc::downgrade(
                        &object.visual_draw_identity_token,
                    ))
                }) {
                    return Ok(false);
                }
                if !self.objects.get(pid).is_some_and(|object| {
                    passenger_births.get(pid).is_some_and(|birth| {
                        birth.ptr_eq(&std::sync::Arc::downgrade(
                            &object.visual_draw_identity_token,
                        ))
                    })
                }) {
                    continue;
                }
            } else if !is_garrison && !is_tunnel && !is_cave
                && !self.objects.get(&container_id).is_some_and(|container| container.is_helix_transport)
            {
                self.reset_rider_mood_check_on_exit(container_id, *pid);
                self.play_container_removing_template_sounds(
                    container_id,
                    *pid,
                    presentation_effects,
                );
            }
            if self
                .host_object(container_id)
                .and_then(|child| child.contained_by)
                .is_some_and(|outer| self.redirected_containment_owner(outer) == container_id)
            {
                self.record_overlord_bunker_residual_exit();
            } else if container_is_combat_chinook {
                // Hull-specific channel mirrors the load arm (update.rs:1442);
                // a Combat Chinook unload is not a generic transport unload.
                self.record_combat_chinook_residual_unload();
            } else if container_is_battle_bus {
                // Hull-specific channel mirrors the load arm (update.rs:1428);
                // a Battle Bus unload is not a generic transport unload.
                self.record_battle_bus_residual_unload();
            } else if container_is_troop_crawler {
                // China Troop Crawler residual unload — mirrors the Combat
                // Chinook / Battle Bus hull-specific honesty channels above
                // (a Troop Crawler evac is not a generic transport unload).
                self.record_troop_crawler_residual_unload();
            } else {
                self.record_transport_residual_unload();
            }
        }
        if !self.objects.get(&container_id).is_some_and(|object| {
            container_id_birth.ptr_eq(&std::sync::Arc::downgrade(
                &object.visual_draw_identity_token,
            ))
        }) {
            return Ok(false);
        }
        if is_garrison {
            // C++ GarrisonContain::removeAllContained → recalcApparentControllingPlayer.
            self.recalc_garrison_apparent_controller_with_effects(container_id, presentation_effects)?;
        } else {
            // C++ OpenContain::removeContained / TransportContain
            // onContainListChange: leaving riders re-evaluate
            // ArmedRidersUpgradeMyWeaponSet, so the hull drops its
            // PLAYER_UPGRADE weapon set when the last armed rider exits.
            self.refresh_battle_bus_armed_riders_weapon_set(container_id);
        }
        for hid in packing_hackers {
            if self.objects.get(&hid).is_some_and(|object| {
                passenger_births.get(&hid).is_some_and(|birth| {
                    birth.ptr_eq(&std::sync::Arc::downgrade(
                        &object.visual_draw_identity_token,
                    ))
                })
            }) {
                self.hacker_income.stop_hacking(hid);
            }
        }

        if let Some(c) = self.objects.get_mut(&container_id) {
            if uses_exit_busy {
                let delay = c.transport_exit_delay_frames();
                c.frame_exit_not_busy = self.frame.saturating_add(delay);
            }
            c.pending_evacuate_on_stop = more_remain;
            c.pending_stream_exit = more_remain;
            c.pending_exit_after_evacuate = and_exit && !more_remain;

            let contained_count = if more_remain {
                c.contained_units().len() as u32
            } else {
                0
            };
            if let Some(ai) = c.chinook_ai.as_mut() {
                ai.contained_count = contained_count;
                ai.wanting_enter_or_exit = more_remain;
                if and_exit && !more_remain {
                    // Keep spawn `original_pos`; do not re-issue evac at the LZ.
                    ai.begin_takeoff_and_exit();
                    return Ok(any);
                }
            }
        }

        if and_exit && !more_remain {
            // The admitted MoveAndEvacuate state owns its saved origin and
            // transitions through actual movement before requesting deletion.
            // This contain operation never manufactures a destroyed/body-death latch.
            self.finish_transport_return_phase_with_effects(container_id, presentation_effects)?;
        }

        Ok(any)
    }

    /// C++ `ChinookCombatDropState::update`: rope delay + `aiRappelInto` at `m_rappelSpeed`.
    pub(super) fn combat_drop_rappel_unload(
        &mut self,
        container_id: ObjectId,
        and_exit: bool,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let now = self.frame;
        let hover = match self.objects.get(&container_id) {
            Some(c) if c.is_alive() => c.get_position(),
            _ => return Ok(false),
        };
        let passengers: Vec<ObjectId> = self
            .objects
            .get(&container_id)
            .map(|c| c.contained_units())
            .unwrap_or_default();
        let rappeller = passengers.iter().copied().find(|pid| {
            self.objects.get(pid).is_some_and(|p| {
                crate::game_logic::host_combat_chinook::HostChinookAI::passenger_kind_can_rappel(
                    p.is_kind_of(KindOf::Infantry),
                )
            })
        });
        let Some(pid) = rappeller else {
            if let Some(c) = self.objects.get_mut(&container_id) {
                let remaining = c.contained_units().len() as u32;
                if let Some(ai) = c.chinook_ai.as_mut() {
                    ai.finish_combat_drop();
                    ai.contained_count = remaining;
                    ai.wanting_enter_or_exit = remaining > 0;
                    if remaining > 0 {
                        ai.tick_idle_auto_land();
                    }
                }
                c.pending_evacuate_on_stop = remaining > 0;
                c.pending_exit_after_evacuate = and_exit && remaining == 0;
            }
            return Ok(false);
        };
        let can_release = self.objects.get(&container_id).is_some_and(|c| {
            c.chinook_ai
                .as_ref()
                .is_some_and(|ai| ai.can_release_rappeller(now))
        });
        if !can_release {
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.pending_evacuate_on_stop = true;
                c.pending_exit_after_evacuate = and_exit;
            }
            return Ok(false);
        }
        let rappel_speed = self
            .objects
            .get(&container_id)
            .and_then(|c| c.chinook_ai.as_ref())
            .map(|ai| ai.apply_rappel_speed())
            .unwrap_or(crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_RAPPEL_SPEED);
        // C++ `aiRappelInto(getMachineGoalObject(), *getMachineGoalPosition())`.
        let drop_target = self.objects.get(&container_id).and_then(|c| {
            c.target.or_else(|| {
                c.chinook_ai
                    .as_ref()
                    .and_then(|ai| ai.combat_drop_target)
                    .map(ObjectId)
            })
        });
        let (target_is_bldg, dest_y) = self.combat_drop_rappel_dest(hover, drop_target);

        self.remove_owned_occupant_with_bonus_callback(container_id, pid, presentation_effects);
        if let Some(p) = self.objects.get_mut(&pid) {
            let containment_actor_id = p.id;
            Self::set_owned_containment_in_parts(
                &mut self.objects,
                &mut self.partition_manager,
                &mut self.ai_groups,
                &mut self.pathfinding_system,
                containment_actor_id,
                None,
                true,
            );
            let p = self
                .objects
                .get_mut(&containment_actor_id)
                .expect("containment operation preserves this birth");
            p.target = drop_target;
            let _ = p;
            if !self.set_owned_object_position_with_effects(pid, hover, self.frame, presentation_effects) {
                return Ok(false);
            }
            let Some(p) = self.objects.get_mut(&pid) else { return Ok(false) };
            let dest = glam::Vec3::new(hover.x, dest_y, hover.z);
            p.movement.clear_path();
            p.movement.target_position = Some(dest);
            p.movement.max_speed = rappel_speed;
            p.begin_rappel(dest_y, target_is_bldg, drop_target);
            p.set_ai_state(
                AIState::Moving,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            p.status.moving = true;
        }
        self.record_transport_residual_unload();
        let more = self.objects.get(&container_id).is_some_and(|c| {
            c.contained_units().iter().any(|id| {
                self.objects.get(id).is_some_and(|p| {
                    crate::game_logic::host_combat_chinook::HostChinookAI::passenger_kind_can_rappel(
                        p.is_kind_of(KindOf::Infantry),
                    )
                })
            })
        });
        if let Some(c) = self.objects.get_mut(&container_id) {
            let remaining = c.contained_units().len() as u32;
            if let Some(ai) = c.chinook_ai.as_mut() {
                ai.on_rappeller_released(now);
                ai.contained_count = remaining;
            }
            // Stay pending so the next evac call finishes DO_COMBAT_DROP.
            c.pending_evacuate_on_stop = true;
            c.pending_exit_after_evacuate = and_exit && !more;
        }

        Ok(true)
    }

    /// C++ `AIRappelState::onEnter` dest Z: layer height, plus structure roof.
    pub(super) fn combat_drop_rappel_dest(
        &self,
        hover: glam::Vec3,
        drop_target: Option<ObjectId>,
    ) -> (bool, f32) {
        let layer_y = self.terrain_height_at(hover).unwrap_or(0.0);
        let Some(tid) = drop_target else {
            return (false, layer_y);
        };
        let Some(bldg) = self.objects.get(&tid) else {
            return (false, layer_y);
        };
        if !bldg.is_alive() || !bldg.is_kind_of(KindOf::Structure) {
            return (false, layer_y);
        }
        let roof = (*bldg.geometry_info()).max_height_above_position();
        (true, layer_y + roof)
    }

    /// C++ `AIRappelState::update`: descend at rappel rate, then kill-2 / addToContain.
    pub(crate) fn tick_rappel_into(&mut self, pid: ObjectId) -> gamelogic::GameLogicResult<()> {
        self.with_own_services_mut(|this| {
            this.tick_rappel_into_with_effects(
                pid,
                &mut crate::game_logic::ScriptPresentationEffects::default(),
            )
        })
    }

    pub(crate) fn tick_rappel_into_with_effects(
        &mut self,
        pid: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(obj) = self.objects.get(&pid) else {
            return Ok(());
        };
        if !obj.status.rappelling || !obj.is_alive() {
            return Ok(());
        }
        let mut target_is_bldg = obj.status.rappel_target_is_bldg;
        let mut dest_y = obj.status.rappel_dest_y;
        let target_id = obj.status.rappel_target;
        let pos = obj.get_position();
        let speed = obj.movement.max_speed;
        if target_is_bldg {
            let gone = target_id
                .map(|id| {
                    self.objects
                        .get(&id)
                        .map_or(true, |b| !b.is_alive() || !b.is_kind_of(KindOf::Structure))
                })
                .unwrap_or(true);
            if gone {
                target_is_bldg = false;
                dest_y = self.terrain_height_at(pos).unwrap_or(0.0);
            }
        }
        if !target_is_bldg {
            dest_y = self.terrain_height_at(pos).unwrap_or(dest_y);
        }
        let sink = speed * LOGIC_FRAME_TIMESTEP;
        let mut new_y = pos.y - sink;
        let arrived = new_y <= dest_y;
        if arrived {
            new_y = dest_y;
        }
        if let Some(obj) = self.objects.get_mut(&pid) {
            let mut p = obj.get_position();
            p.y = new_y;
            let _ = obj;
            if !self.set_owned_object_position_with_effects(pid, p, self.frame, presentation_effects) {
                return Ok(());
            }
            let Some(obj) = self.objects.get_mut(&pid) else { return Ok(()) };
            obj.movement.velocity = glam::Vec3::new(0.0, -speed, 0.0);
            obj.status.rappel_target_is_bldg = target_is_bldg;
            obj.status.rappel_dest_y = dest_y;
            if arrived {
                obj.clear_rappel();
                obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
            }
        }
        if arrived && target_is_bldg {
            if let Some(tid) = target_id {
                self.rappel_arrive_at_building_with_effects(pid, tid, presentation_effects)?;
            }
        }
        Ok(())
    }

    /// C++ `AIRappelState::update` arrival: kill up to 2, else addToContain / scatter.
    pub(super) fn rappel_arrive_at_building(&mut self, pid: ObjectId, bldg_id: ObjectId) -> gamelogic::GameLogicResult<()> {
        self.rappel_arrive_at_building_with_effects(
            pid,
            bldg_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(super) fn rappel_arrive_at_building_with_effects(
        &mut self,
        pid: ObjectId,
        bldg_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(bldg) = self.objects.get(&bldg_id) else {
            return Ok(());
        };
        if !bldg.is_alive() {
            return Ok(());
        }
        const MAX_TO_KILL: i32 = 2;
        let num_killed = self.kill_enemies_in_container_with_effects(
            pid,
            bldg_id,
            MAX_TO_KILL,
            presentation_effects,
        )?;
        // C++ AIStates.cpp:561-564: getPerUnitFX(CombatDropKillFX) on the
        // rappeller, FXList::doFXObj on the building, even if the rappeller dies.
        if num_killed > 0 {
            if let Some(fx) = self.objects.get(&pid).and_then(|p| {
                crate::game_logic::host_combat_chinook::leftover_combat_drop_kill_fx_name(
                    &p.template_name,
                )
            }) {
                let _ = self.dispatch_fx_list_at_host_object_with_effects(
                    &fx,
                    bldg_id,
                    None,
                    presentation_effects,
                );
            }
        }
        if num_killed == MAX_TO_KILL {
            self.apply_owned_kill_with_effects(
                pid,
                DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                presentation_effects,
            )?;
            return Ok(());
        }
        let can_enter = self.objects.get(&bldg_id).is_some_and(|b| {
            b.is_alive()
                && b.can_contain()
                && b.has_capacity_for(1)
                && b.garrison_container_accepts_entry()
        });
        if can_enter {
            self.rappel_add_to_contain_with_effects(pid, bldg_id, presentation_effects)?;
        } else {
            self.rappel_scatter_at_building(pid, bldg_id, presentation_effects);
        }
        Ok(())
    }

    /// C++ `killEnemiesInContainer` (AIStates.cpp:415).
    pub(super) fn kill_enemies_in_container(
        &mut self,
        killer_id: ObjectId,
        bldg_id: ObjectId,
        max_to_kill: i32,
    ) -> gamelogic::GameLogicResult<i32> {
        self.kill_enemies_in_container_with_effects(
            killer_id,
            bldg_id,
            max_to_kill,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(super) fn kill_enemies_in_container_with_effects(
        &mut self,
        killer_id: ObjectId,
        bldg_id: ObjectId,
        max_to_kill: i32,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<i32> {
        let mut num_killed = 0;
        while num_killed < max_to_kill {
            let occupants = self
                .objects
                .get(&bldg_id)
                .map(|b| b.contained_units())
                .unwrap_or_default();
            let Some(enemy_id) = occupants.into_iter().find(|&id| {
                self.objects.get(&id).is_some_and(|e| {
                    e.is_alive()
                        && self.objects.get(&killer_id).is_some_and(|killer| {
                            self.object_relationship(killer, e)
                                == gamelogic::common::Relationship::Enemies
                        })
                })
            }) else {
                break;
            };
            let enemy_birth = self.objects.get(&enemy_id).map(|object| object.visual_object_generation);
            if self.objects.get(&bldg_id).is_some_and(|container| {
                container.thing().template.contain_module.kind == crate::game_logic::ContainModuleKind::RailedTransport
            }) {
                if !self.railed_remove_contained_with_effects(bldg_id, enemy_id, presentation_effects)
                    .map_err(gamelogic::GameLogicError::Configuration)?
                {
                    break;
                }
            } else {
            self.remove_owned_occupant_with_bonus_callback(bldg_id, enemy_id, presentation_effects);
            if let Some(e) = self.objects.get_mut(&enemy_id) {
                let containment_actor_id = e.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    &mut self.pathfinding_system,
                    containment_actor_id,
                    None,
                    true,
                );
                let e = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
            }
            }
            if !self.objects.get(&enemy_id).is_some_and(|object| Some(object.visual_object_generation) == enemy_birth) {
                continue;
            }
            // CPP AIStates.cpp435 scores while the enemy is still alive,
            // before the receiving Body/onDie continuation can change it.
            self.score_the_kill_experience(killer_id, enemy_id);
            self.apply_owned_kill_with_effects(
                enemy_id,
                DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                presentation_effects,
            )?;
            num_killed += 1;
        }
        Ok(num_killed)
    }

    /// C++ `bldg->getContain()->addToContain(obj)` + garrison onContaining.
    pub(super) fn rappel_add_to_contain(&mut self, pid: ObjectId, bldg_id: ObjectId) -> gamelogic::GameLogicResult<()> {
        self.rappel_add_to_contain_with_effects(
            pid,
            bldg_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(super) fn rappel_add_to_contain_with_effects(
        &mut self,
        pid: ObjectId,
        bldg_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // C++ OpenContain::addToContain checkAndDetonateBoobyTrap(rider).
        if self.should_cancel_containment_after_booby_trap_with_effects(
            bldg_id,
            pid,
            presentation_effects,
        )? {
            return Ok(());
        }
        let Some(rider) = self.objects.get(&pid) else { return Ok(()) };
        if rider.contained_by.is_some() {
            return Ok(());
        }
        let rider_birth = rider.visual_object_generation;
        let Some(container) = self.objects.get(&bldg_id) else { return Ok(()) };
        let container_birth = container.visual_object_generation;
        let container_pos = container.get_position();
        let enclosing = container.is_enclosing_garrison_container();
        let garrison = container.is_garrison_contain();
        let epoch = self.visual_world_epoch;
        self.validate_garrison_owner_team_transfer(bldg_id, pid)?;
        let entered = self
            .objects
            .get_mut(&bldg_id)
            .map(|b| b.add_occupant(pid))
            .unwrap_or(false);
        if !entered {
            self.rappel_scatter_at_building(pid, bldg_id, presentation_effects);
            return Ok(());
        }
        if let Some(obj) = self.objects.get_mut(&pid) {
            obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            obj.target = Some(bldg_id);
        }
        self.stamp_player_who_entered(bldg_id, pid);
        if enclosing {
            self.change_owned_containment_world_membership_with_track_effects(
                pid, false, presentation_effects,
            );
        }
        self.play_container_on_containing_sound(bldg_id, presentation_effects);
        if garrison {
            // onContaining holds the rider before the enclosing position
            // update, so the pathfinder removes the rider from its old cell.
            self.apply_garrison_contain_on_enter_at(
                bldg_id, pid, enclosing.then_some(container_pos), presentation_effects,
            )?;
        } else if enclosing && !self.set_owned_object_position_with_effects(
            pid, container_pos, self.frame, presentation_effects,
        ) {
            return Ok(());
        }
        if !self.spawn_identity_matches(bldg_id, container_birth, epoch)
            || !self.spawn_identity_matches(pid, rider_birth, epoch)
        {
            return Ok(());
        }
        if let Some(obj) = self.objects.get_mut(&pid) {
            obj.deselect();
            obj.finish_containment_reference_with_partition_owner(
                Some(bldg_id), enclosing, &mut self.partition_manager,
            );
            obj.set_ai_state(
                AIState::Garrisoned,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            obj.set_status_moving(false);
        }
        self.admit_object_drawable_deselection(pid);
        self.record_garrison_residual_enter();
        // C++ OpenContain::addToContain doLoadSound.
        self.play_container_load_sound(bldg_id, presentation_effects);
        Ok(())
    }

    /// C++ full-building scatter + `aiFollowPath` (AIStates.cpp:581-613).
    pub(super) fn rappel_scatter_at_building(
        &mut self,
        pid: ObjectId,
        bldg_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(bldg) = self.objects.get(&bldg_id) else {
            return;
        };
        let bldg_pos = bldg.get_position();
        let exit_angle = bldg.get_orientation();
        let bldg_r = (*bldg.geometry_info()).bounding_circle_radius();
        let pax_r = self
            .objects
            .get(&pid)
            .map(|p| (*p.geometry_info()).bounding_circle_radius())
            .unwrap_or(8.0);
        let offset = pax_r.min(bldg_r).max(1.0);
        let angle = std::f32::consts::PI + garrison_evac_rand(pid.0, 0.0, std::f32::consts::PI);
        let mut start = bldg_pos;
        start.x += offset * angle.cos();
        start.z += offset * angle.sin();
        start.y = self.terrain_height_at(start).unwrap_or(0.0);
        let mut end = start;
        end.z -= 20.0;
        end.y = self.terrain_height_at(end).unwrap_or(end.y);
        if let Some(obj) = self.objects.get_mut(&pid) {
            let _ = obj;
            if !self.set_owned_object_position_with_effects(pid, start, self.frame, presentation_effects) {
                return;
            }
            let Some(obj) = self.objects.get_mut(&pid) else { return };
            obj.set_orientation_with_partition_owner(exit_angle, &mut self.partition_manager);
            obj.movement.set_path_coords(vec![start, end]);
            obj.movement.path_cursor = obj.movement.path.cursor_at_ordinal(1);
            obj.movement.target_position = Some(end);
            obj.is_attack_path = false;
            obj.is_exact_path = false;
            obj.refresh_follow_path_extra_distance();
            obj.set_ai_state(
                AIState::Moving,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            obj.status.moving = true;
        }
        self.resolve_installed_path_layers(pid);
    }
}
