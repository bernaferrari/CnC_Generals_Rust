//! Enter, exit, evacuate, dock, combat drop, railed transport.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, ObjectType, PendingSpecialAbility, Resources, Team,
    radar_notifications::RadarKind,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::AsciiString;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};

impl<'a> CommandExecutor<'a> {
    /// C++ `AIGroup::groupExecuteRailedTransport` →
    /// `RailedTransportAIUpdate::privateExecuteRailedTransport`.
    pub(crate) fn execute_railed_transport(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            if self.game_logic.execute_railed_transport_for(unit_id) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Find the nearest building that can accept this unit for garrison/enter.
    pub(super) fn find_nearest_garrison_target(&self, unit_id: ObjectId) -> Option<ObjectId> {
        let unit = self.game_logic.host_object(unit_id)?;
        let unit_pos = unit.get_position();
        let unit_team = unit.team;
        // Pure residual acquire: nearest friendly container with capacity (3D).
        let candidates: Vec<_> = self
            .game_logic
            .host_objects()
            .iter()
            .filter_map(|(&obj_id, obj)| {
                if obj.team != unit_team || !obj.is_alive() || !obj.can_contain() {
                    return None;
                }
                // This helper feeds the normal garrison/Enter acquire path,
                // not an internal payload spawn.  Keep its candidate screen
                // on the centralized ContainModule authority so a heavy
                // rider cannot be matched to a transport with only one raw
                // body slot left.
                if !self
                    .game_logic
                    .can_unit_enter_normal_target(unit_id, obj_id)
                {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: obj_id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: false,
                        under_construction: obj.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            unit_id,
            unit_team,
            unit_pos,
            candidates,
            |_| f32::MAX,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    // === Transport Commands ===

    pub(super) fn execute_enter(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        // C++ JetAIUpdate::privateEnter → doLandingCommand.
        let jet_landed = units.iter().copied().any(|unit_id| {
            self.game_logic
                .try_jet_enter_or_repair_airfield(unit_id, target_id)
        });
        if jet_landed {
            return CommandResult::Success;
        }

        // USA Pilot residual: Enter unmanned vehicle for recrew (not transport contain).
        // A non-container target is legal only when at least one selected
        // source passes the parsed `VeterancyCrateCollide IsPilot` authority
        // predicate OR C++ canEnterObject unmanned infantry (not REJECT_UNMANNED).
        let has_unmanned_recrew_source = units.iter().copied().any(|unit_id| {
            self.game_logic.can_execute_pilot_recrew(unit_id, target_id)
                || self
                    .game_logic
                    .can_execute_infantry_unmanned_recrew(unit_id, target_id)
        });
        let target_pos = match self.game_logic.host_object(target_id) {
            Some(transport)
                if transport.is_alive()
                    && !transport.status.under_construction
                    && (transport.can_contain() || has_unmanned_recrew_source) =>
            {
                transport.get_position()
            }
            _ => return CommandResult::InvalidTarget,
        };

        let mut issued = false;
        for &unit_id in units {
            let unmanned_recrew = self.game_logic.host_object(unit_id).is_some_and(|unit| {
                unit.is_alive()
                    && unit.can_move()
                    && (self.game_logic.can_execute_pilot_recrew(unit_id, target_id)
                        || self
                            .game_logic
                            .can_execute_infantry_unmanned_recrew(unit_id, target_id))
            });
            let shrouded = self
                .game_logic
                .is_enter_target_shrouded_for_action(unit_id, target_id);
            let can_issue = self.can_issue_enter(unit_id, target_id);
            if shrouded || (!unmanned_recrew && !can_issue) {
                continue;
            }

            let unit_in_tunnel = self
                .game_logic
                .tunnel_network_residual()
                .player_holding_unit(unit_id)
                .is_some();
            let previous_container = self.game_logic.host_object(unit_id).and_then(|unit| {
                if matches!(unit.ai_state, AIState::Docked | AIState::Garrisoned) || unit_in_tunnel
                {
                    unit.container_id().or(unit.target)
                } else {
                    None
                }
            });
            if let Some(previous_container) = previous_container {
                if previous_container != target_id {
                    // C++ OpenContain::addToContain refuses a rider that is
                    // already contained. Transfer is Exit then a new Enter.
                    continue;
                }
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            // C++ AIEnterState enters immediately when the unit is already
            // close enough to the goal container — the same
            // `selection_radius + container_radius + 4.0` authority the
            // Entering tick uses for doEnter (support update enter_range).
            // An at-goal unit must not fail the order only because no A*
            // allocation is needed.
            let in_enter_range = self
                .game_logic
                .host_object(unit_id)
                .zip(self.game_logic.host_object(target_id))
                .is_some_and(|(unit, container)| {
                    unit.get_position().distance(container.get_position())
                        <= unit.selection_radius + container.selection_radius + 4.0
                });
            let issued_order = if in_enter_range {
                self.game_logic.unit_command_set_ai_state(unit_id, AIState::Entering)
            } else {
                self.path_to_goal_with_state_ignoring(
                    unit_id,
                    target_pos,
                    AIState::Entering,
                    Some(target_id),
                )
            };
            if issued_order {
                issued = true;
            }
        }
        if issued {
            let (heal, structure, enemies, allies) = match (
                units
                    .first()
                    .and_then(|id| self.game_logic.host_object(*id)),
                self.game_logic.host_object(target_id),
            ) {
                (Some(unit), Some(target)) => {
                    let rel = match (unit.owner_player_id, target.owner_player_id) {
                        (Some(a), Some(b)) => self.game_logic.player_relationship(a, b),
                        _ if unit.team == target.team => gamelogic::common::Relationship::Allies,
                        _ => gamelogic::common::Relationship::Neutral,
                    };
                    (
                        target.is_kind_of(crate::game_logic::KindOf::HealPad),
                        target.is_kind_of(crate::game_logic::KindOf::Structure),
                        rel == gamelogic::common::Relationship::Enemies,
                        rel == gamelogic::common::Relationship::Allies,
                    )
                }
                _ => (false, false, false, true),
            };
            let slot = crate::game_logic::audio_dispatch_impl::enter_voice_slot(
                heal, structure, enemies, allies,
            );
            self.game_logic.queue_picked_unit_voice(units, slot);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ MSG_EXIT occupant: the unit is inside a container / tunnel.
    fn unit_is_exit_occupant(&self, id: ObjectId) -> bool {
        let Some(obj) = self.game_logic.host_object(id) else {
            return false;
        };
        let is_contained = matches!(
            obj.ai_state,
            AIState::Docked | AIState::Garrisoned | AIState::Entering | AIState::Docking
        );
        let in_tunnel = self
            .game_logic
            .tunnel_network_residual()
            .player_holding_unit(id)
            .is_some();
        let in_cave = self
            .game_logic
            .cave_system_residual()
            .index_holding_unit(id)
            .is_some();
        is_contained
            || in_tunnel
            || in_cave
            || obj.container_id().is_some()
            || obj.contained_by.is_some()
    }

    pub(super) fn execute_exit(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut to_unload: Vec<(ObjectId, Option<ObjectId>, Vec3)> = Vec::new();
        let mut seen_units: HashSet<ObjectId> = HashSet::new();
        // Tunnel network residual: exit tunnel id for shared-pool bookkeeping.
        let mut tunnel_exit_for: HashMap<ObjectId, ObjectId> = HashMap::new();
        // CaveSystem residual: exit cave id so leftover record_exit + LastEmpty run.
        let mut cave_exit_for: HashMap<ObjectId, ObjectId> = HashMap::new();

        // C++ GameLogicDispatch.cpp:978-1004 MSG_EXIT selects the occupant.
        // If any selected unit is an occupant, do not dump containers (MSG_EVACUATE).
        let occupant_selected = units.iter().any(|&id| self.unit_is_exit_occupant(id));

        for &selected_id in units {
            let Some(selected_obj) = self.game_logic.host_object(selected_id) else {
                continue;
            };

            if selected_obj.can_contain() {
                if occupant_selected {
                    continue;
                }
                // C++ AIUpdateInterface::privateEvacuate — DISABLED_SUBDUED
                // solders the doors shut (Microwave Tank).
                if selected_obj.is_subdued_disabled() {
                    continue;
                }
                // Prefer get_position() (authoritative Thing pos). The pub `position`
                // field is often left at default ZERO after create_object set_position.
                let origin = selected_obj
                    .building_data
                    .as_ref()
                    .and_then(|b| b.rally_point)
                    .unwrap_or_else(|| selected_obj.get_position());

                // Tunnel Network residual: Evacuate/Exit on THIS player's tunnel dumps the
                // shared MaxTunnelCapacity pool at THIS tunnel (cross-tunnel path).
                if selected_obj.is_tunnel_network_style_container() {
                    let player_id = selected_obj.tunnel_system_key();
                    let shared = self
                        .game_logic
                        .tunnel_network_contained_for_player(player_id);
                    for contained in shared {
                        if seen_units.insert(contained) {
                            to_unload.push((contained, Some(selected_id), origin));
                            tunnel_exit_for.insert(contained, selected_id);
                        }
                    }
                    // Also include any local-only occupants not yet in the shared list.
                    for contained in selected_obj.contained_units() {
                        if seen_units.insert(contained) {
                            to_unload.push((contained, Some(selected_id), origin));
                            tunnel_exit_for.insert(contained, selected_id);
                        }
                    }
                    continue;
                }

                // C++ CaveContain shared CaveIndex pool. Exit on this cave dumps
                // leftover CaveSystem occupants so record_exit + LastEmpty run.
                if selected_obj.is_cave_style_container() {
                    let idx = selected_obj.cave_index;
                    let shared = self
                        .game_logic
                        .cave_system_residual()
                        .contained_for_index(idx);
                    for contained in shared {
                        if seen_units.insert(contained) {
                            to_unload.push((contained, Some(selected_id), origin));
                            cave_exit_for.insert(contained, selected_id);
                        }
                    }
                    for contained in selected_obj.contained_units() {
                        if seen_units.insert(contained) {
                            to_unload.push((contained, Some(selected_id), origin));
                            cave_exit_for.insert(contained, selected_id);
                        }
                    }
                    continue;
                }

                for contained in selected_obj.contained_units() {
                    if seen_units.insert(contained) {
                        to_unload.push((contained, Some(selected_id), origin));
                    }
                }
                continue;
            }

            let is_contained = matches!(
                selected_obj.ai_state,
                AIState::Docked | AIState::Garrisoned | AIState::Entering | AIState::Docking
            );
            // Units in tunnel network may only have contained_by set.
            let in_tunnel = self
                .game_logic
                .tunnel_network_residual()
                .player_holding_unit(selected_id)
                .is_some();
            let in_cave = self
                .game_logic
                .cave_system_residual()
                .index_holding_unit(selected_id)
                .is_some();
            if !is_contained && !in_tunnel && !in_cave {
                continue;
            }

            // Prefer contained_by (authoritative) over target for residual garrison exit.
            let (origin, container_id) = if let Some(container_id) = selected_obj.container_id() {
                if let Some(container) = self.game_logic.host_object(container_id) {
                    let rally = container.building_data.as_ref().and_then(|b| b.rally_point);
                    (
                        rally.unwrap_or_else(|| container.get_position()),
                        Some(container_id),
                    )
                } else {
                    (selected_obj.get_position(), None)
                }
            } else if in_cave {
                let cave_id = self
                    .game_logic
                    .cave_system_residual()
                    .index_holding_unit(selected_id)
                    .and_then(|idx| {
                        self.game_logic
                            .cave_system_residual()
                            .cave_ids_for_index(idx)
                            .into_iter()
                            .next()
                    });
                if let Some(cid) = cave_id {
                    if let Some(container) = self.game_logic.host_object(cid) {
                        let rally = container.building_data.as_ref().and_then(|b| b.rally_point);
                        (rally.unwrap_or_else(|| container.get_position()), Some(cid))
                    } else {
                        (selected_obj.get_position(), Some(cid))
                    }
                } else {
                    (selected_obj.get_position(), None)
                }
            } else {
                (selected_obj.get_position(), None)
            };
            if container_id
                .and_then(|cid| self.game_logic.host_object(cid))
                .is_some_and(|c| c.is_subdued_disabled())
            {
                continue;
            }

            if seen_units.insert(selected_id) {
                to_unload.push((selected_id, container_id, origin));
                if let Some(cid) = container_id {
                    if self
                        .game_logic
                        .host_object(cid)
                        .map(|c| c.is_tunnel_network_style_container())
                        .unwrap_or(false)
                    {
                        tunnel_exit_for.insert(selected_id, cid);
                    } else if self
                        .game_logic
                        .host_object(cid)
                        .map(|c| c.is_cave_style_container())
                        .unwrap_or(false)
                    {
                        cave_exit_for.insert(selected_id, cid);
                    }
                }
            }
        }

        if to_unload.is_empty() {
            return CommandResult::InvalidCommand;
        }

        // C++ AIUpdateInterface::privateEvacuate → markAllPassengersDetected
        // immediately before orderAllPassengersToExit. Leftover
        // order_all_passengers_to_exit does the same. Dump-all (Evac / Exit
        // on the container) must destalth STEALTH_GARRISON riders.
        if !occupant_selected {
            let mut marked = HashSet::new();
            for (_, container_id, _) in &to_unload {
                if let Some(cid) = container_id {
                    if marked.insert(*cid) {
                        self.game_logic.mark_all_passengers_detected(*cid);
                    }
                }
            }
        }

        let frame = self.game_logic.frame;
        let mut dropped_from: HashSet<ObjectId> = HashSet::new();
        let mut pending_containers: HashSet<ObjectId> = HashSet::new();

        for (i, (unit_id, container_id, origin)) in to_unload.into_iter().enumerate() {
            let tunnel_exit = tunnel_exit_for.get(&unit_id).copied();
            let is_tunnel_unit = tunnel_exit.is_some()
                || container_id.is_some_and(|cid| {
                    self.game_logic
                        .tunnel_network_residual()
                        .player_holding_unit(unit_id)
                        .is_some()
                        && self
                            .game_logic
                            .host_object(cid)
                            .is_some_and(|c| c.is_tunnel_network_style_container())
                });
            let cave_exit = cave_exit_for.get(&unit_id).copied();
            let is_cave_unit = cave_exit.is_some()
                || container_id.is_some_and(|cid| {
                    self.game_logic
                        .cave_system_residual()
                        .index_holding_unit(unit_id)
                        .is_some()
                        && self
                            .game_logic
                            .host_object(cid)
                            .is_some_and(|c| c.is_cave_style_container())
                });

            // C++ TransportContain::isExitBusy / reserveDoorForExit.
            if !is_tunnel_unit && !is_cave_unit {
                if let Some(cid) = container_id {
                    if let Some(c) = self.game_logic.host_object(cid) {
                        if c.uses_transport_contain_exit_busy()
                            && (c.is_transport_exit_busy(frame) || dropped_from.contains(&cid))
                        {
                            pending_containers.insert(cid);
                            continue;
                        }
                    }
                }
            }

            // Stagger exits deterministically to avoid clumping on the same point.
            let angle = (unit_id.0 as f32 + i as f32 * 1.37).sin().atan2(1.0) + i as f32 * 0.7;
            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 6.0;
            let drop_position = origin + offset;

            let was_tunnel = if let Some(exit_tid) = tunnel_exit {
                self.game_logic.exit_tunnel_network_unit(unit_id, exit_tid)
            } else if let Some(cid) = container_id {
                // Fallback: unit in shared pool exiting via entry tunnel.
                if self
                    .game_logic
                    .tunnel_network_residual()
                    .player_holding_unit(unit_id)
                    .is_some()
                {
                    self.game_logic.exit_tunnel_network_unit(unit_id, cid)
                } else {
                    false
                }
            } else {
                false
            };

            // C++ CaveContain::onRemoving → leftover record_exit + LastEmpty team revert.
            let was_cave = if let Some(exit_cid) = cave_exit {
                self.game_logic.exit_cave_unit(unit_id, exit_cid)
            } else if is_cave_unit {
                if let Some(cid) = container_id {
                    self.game_logic.exit_cave_unit(unit_id, cid)
                } else {
                    false
                }
            } else {
                false
            };

            if !was_tunnel && !was_cave {
                if let Some(container_id) = container_id {
                    // Wave 233: remove occupant via GameLogic authority API.
                    let _ = self
                        .game_logic
                        .unit_command_remove_occupant(container_id, unit_id);
                }
            }

            // Classify residual exit before mutating unit state.
            // Prefer AI state; fall back to container kind when only contained_by is set.
            // Overlord BattleBunker / GLA Battle Bus / Combat Chinook / Listening Outpost
            // residuals are vehicle-docked but tracked separately from generic Humvee residual.
            let (
                was_garrisoned,
                was_overlord_bunker,
                was_battle_bus,
                was_technical,
                was_combat_chinook,
                was_listening_outpost,
                was_troop_crawler,
                was_transport,
            ) = if was_tunnel || was_cave {
                (false, false, false, false, false, false, false, false)
            } else if let Some(unit) = self.game_logic.host_object(unit_id) {
                let garrisoned = matches!(unit.ai_state, AIState::Garrisoned);
                let docked = matches!(unit.ai_state, AIState::Docked);
                let cid = unit.contained_by.or(container_id);
                let container = cid.and_then(|id| self.game_logic.host_object(id));
                let is_overlord = container
                    .map(|c| c.is_overlord_style_container())
                    .unwrap_or(false);
                let is_battle_bus = container
                    .map(|c| c.is_battle_bus_style_container())
                    .unwrap_or(false);
                let is_technical = container
                    .map(|c| c.is_technical_style_container())
                    .unwrap_or(false);
                let is_combat_chinook = container
                    .map(|c| c.is_combat_chinook_style_container())
                    .unwrap_or(false);
                let is_listening_outpost = container
                    .map(|c| c.is_listening_outpost_style_container())
                    .unwrap_or(false);
                let is_troop_crawler = container
                    .map(|c| c.is_troop_crawler_style_container())
                    .unwrap_or(false);
                let is_garrison = container.map(|c| c.is_garrison_contain()).unwrap_or(false);
                // Generic TransportContain residual (Humvee-style vehicle that
                // is not one of the separately tracked containers above).
                // C++ TransportContain::onRemoving (TransportContain.cpp:306)
                // is the generic rider-exit path these counters observe.
                let is_generic_transport = container
                    .map(|c| {
                        c.can_contain()
                            && !c.is_garrison_contain()
                            && !c.is_overlord_style_container()
                            && !c.is_battle_bus_style_container()
                            && !c.is_technical_style_container()
                            && !c.is_combat_chinook_style_container()
                            && !c.is_listening_outpost_style_container()
                            && !c.is_troop_crawler_style_container()
                    })
                    .unwrap_or(false);
                // C++ GarrisonContain::exitObjectViaDoor — burst/left/right walk.
                // Do not treat every KINDOF_STRUCTURE as garrison (HealContain).
                if garrisoned || is_garrison {
                    (true, false, false, false, false, false, false, false)
                } else if docked {
                    if is_overlord {
                        (false, true, false, false, false, false, false, false)
                    } else if is_battle_bus {
                        (false, false, true, false, false, false, false, false)
                    } else if is_technical {
                        (false, false, false, true, false, false, false, false)
                    } else if is_combat_chinook {
                        (false, false, false, false, true, false, false, false)
                    } else if is_listening_outpost {
                        (false, false, false, false, false, true, false, false)
                    } else if is_troop_crawler {
                        (false, false, false, false, false, false, true, false)
                    } else {
                        (false, false, false, false, false, false, false, is_generic_transport)
                    }
                } else if unit.contained_by.is_some() || container_id.is_some() {
                    if is_overlord {
                        (false, true, false, false, false, false, false, false)
                    } else if is_technical {
                        (false, false, false, true, false, false, false, false)
                    } else if is_combat_chinook {
                        (false, false, false, false, true, false, false, false)
                    } else if is_listening_outpost {
                        (false, false, false, false, false, true, false, false)
                    } else if is_troop_crawler {
                        (false, false, false, false, false, false, true, false)
                    } else if is_battle_bus {
                        (false, false, true, false, false, false, false, false)
                    } else {
                        (false, false, false, false, false, false, false, is_generic_transport)
                    }
                } else {
                    (false, false, false, false, false, false, false, false)
                }
            } else {
                (false, false, false, false, false, false, false, false)
            };

            // C++ GarrisonContain::exitObjectViaDoor: burst / left / right walk.
            // Do not teleport to the 6-unit Idle ring used for generic dumps.
            if was_garrisoned {
                if let Some(cid) = container_id {
                    let _ = self
                        .game_logic
                        .garrison_exit_occupant_via_door(unit_id, cid);
                } else {
                    let _ = self
                        .game_logic
                        .unit_command_exit_drop(unit_id, drop_position);
                }
            } else if let Some(cid) = container_id {
                // C++ OpenContain::exitObjectViaDoor — ExitStart/End walk.
                // TunnelContain does not override; walk the *exit* entrance.
                // Do not Idle-teleport a 6-unit ring around the hull.
                if !self
                    .game_logic
                    .unit_command_exit_via_open_contain(unit_id, cid)
                {
                    let _ = self
                        .game_logic
                        .unit_command_exit_drop(unit_id, drop_position);
                }
            } else {
                let _ = self
                    .game_logic
                    .unit_command_exit_drop(unit_id, drop_position);
            }
            if was_tunnel || was_cave {
                // Counters already recorded in leftover exit_tunnel / exit_cave_unit.
            } else if was_garrisoned {
                self.game_logic.record_garrison_residual_exit();
            } else if was_overlord_bunker {
                self.game_logic.record_overlord_bunker_residual_exit();
            } else if was_battle_bus {
                self.game_logic.record_battle_bus_residual_unload();
            } else if was_technical {
                self.game_logic.record_technical_residual_unload();
            } else if was_combat_chinook {
                self.game_logic.record_combat_chinook_residual_unload();
            } else if was_listening_outpost {
                self.game_logic.record_listening_outpost_residual_unload();
            } else if was_troop_crawler {
                self.game_logic.record_troop_crawler_residual_unload();
            } else if was_transport {
                self.game_logic.record_transport_residual_unload();
            }
            debug!(
                "Unit {} exiting transport/garrison near {:?}",
                unit_id.0, drop_position
            );

            // Refresh armed-riders weapon set after unload residual.
            if let Some(cid) = container_id {
                if was_battle_bus || was_combat_chinook || was_listening_outpost {
                    self.game_logic
                        .refresh_battle_bus_armed_riders_weapon_set(cid);
                }
                if !was_tunnel {
                    if let Some(c) = self.game_logic.host_object_mut(cid) {
                        if c.uses_transport_contain_exit_busy() {
                            let delay = c.transport_exit_delay_frames();
                            c.frame_exit_not_busy = frame.saturating_add(delay);
                            if delay > 0 || c.transport_delay_exit_in_air() {
                                dropped_from.insert(cid);
                            }
                        }
                    }
                }
            }
        }

        for cid in pending_containers {
            if let Some(c) = self.game_logic.host_object_mut(cid) {
                c.pending_evacuate_on_stop = true;
                // C++ orderAllPassengersToExit → AIExitState, no stop required.
                c.pending_stream_exit = true;
            }
        }

        CommandResult::Success
    }

    pub(crate) fn execute_evacuate(&mut self, units: &[ObjectId]) -> CommandResult {
        // C++ AIGroup::groupEvacuate (AIGroup.cpp:2408-2440):
        //  - airborne aircraft: dest Z = terrain height, then aiMoveToAndEvacuate
        //  - structures without AI: orderAllPassengersToExit
        //  - other AI containers: aiEvacuate
        let mut ground_containers: Vec<ObjectId> = Vec::new();
        let mut railed_ferries: Vec<ObjectId> = Vec::new();
        let mut airborne_jobs: Vec<(ObjectId, Vec3)> = Vec::new();
        for &unit_id in units {
            let Some(obj) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            // C++ AIUpdateInterface::privateEvacuate — DISABLED_SUBDUED.
            if obj.is_subdued_disabled() {
                continue;
            }
            let is_container =
                obj.can_contain() || obj.is_kind_of(crate::game_logic::KindOf::Structure);
            if !is_container {
                continue;
            }
            if obj.is_railed_transport() {
                railed_ferries.push(unit_id);
                continue;
            }
            let airborne =
                obj.is_kind_of(crate::game_logic::KindOf::Aircraft) && obj.status.airborne_target;
            if airborne {
                let pos = obj.get_position();
                // C++ AIGroup::groupEvacuate (AIGroup.cpp:2418-2421): dest Z is
                // the terrain/layer height under the aircraft — never sea
                // level. `ground_height_from_terrain` marks a live terrain
                // sample; trust it over an empty-map probe that returns 0.
                let dest_y = if obj.ground_height_from_terrain && obj.ground_height > 0.0 {
                    obj.ground_height
                } else {
                    self.game_logic
                        .terrain_height_at(pos)
                        .filter(|h| *h > 0.0)
                        .unwrap_or(obj.ground_height)
                };
                airborne_jobs.push((unit_id, Vec3::new(pos.x, dest_y, pos.z)));
            } else {
                ground_containers.push(unit_id);
            }
        }

        let mut any = false;
        for ferry_id in railed_ferries {
            if self.game_logic.railed_transport_unload_all(ferry_id) {
                any = true;
            }
        }
        for (unit_id, dest) in airborne_jobs {
            // Path to ground, then unload. Do not execute_exit while airborne.
            if matches!(
                self.execute_move_to_and_evacuate(&[unit_id], dest, false),
                CommandResult::Success
            ) {
                any = true;
            }
        }
        if !ground_containers.is_empty() {
            if matches!(
                self.execute_exit(&ground_containers),
                CommandResult::Success
            ) {
                any = true;
            }
        }

        if any {
            self.game_logic.queue_picked_unit_voice(
                units,
                crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::Unload,
            );
            CommandResult::Success
        } else {
            // Fail-closed: no containers selected (unlike Exit which can free passengers).
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupMoveToAndEvacuate / AndExit residual.
    /// Path capable containers to `destination`, then unload on arrival.
    /// `and_exit` marks the transport for self-removal after unload (script exit residual).
    pub(crate) fn execute_move_to_and_evacuate(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
        and_exit: bool,
    ) -> CommandResult {
        if !destination.x.is_finite() || !destination.y.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        let mut any = false;
        for &unit_id in units {
            let can = match self.game_logic.host_object(unit_id) {
                Some(obj)
                    if obj.is_alive()
                        && !obj.is_subdued_disabled()
                        && obj.can_move()
                        && (obj.can_contain()
                            || obj.is_kind_of(crate::game_logic::KindOf::Aircraft)
                            || !obj.contained_units().is_empty()) =>
                {
                    true
                }
                _ => false,
            };
            if !can {
                continue;
            }
            let taking_off = if let Some(obj) = self.game_logic.host_object_mut(unit_id) {
                let name = obj.template_name.clone();
                if obj.chinook_ai.is_none()
                    && (crate::game_logic::host_combat_chinook::is_regular_chinook_template(&name)
                        || crate::game_logic::host_combat_chinook::is_combat_chinook_template(
                            &name,
                        ))
                {
                    if crate::game_logic::host_combat_chinook::is_combat_chinook_template(&name) {
                        obj.install_combat_chinook_transport();
                    } else {
                        obj.install_chinook_transport();
                    }
                }
                let p = obj.get_position();
                if let Some(ai) = obj.chinook_ai.as_mut() {
                    ai.pos = [p.x, p.z, p.y];
                    ai.command_evac([destination.x, destination.z, destination.y], and_exit);
                    ai.state
                        == crate::game_logic::host_combat_chinook::HostChinookAIState::TakingOff
                } else {
                    false
                }
            } else {
                false
            };
            if taking_off {
                let _ = self
                    .game_logic
                    .unit_command_set_pending_evacuate(unit_id, true, and_exit, true);
                any = true;
                continue;
            }

            // Wave 233: pending evacuate prep via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_set_pending_evacuate(unit_id, true, and_exit, true);
            if self.path_to_goal_with_state(unit_id, destination, AIState::Moving) {
                any = true;
            } else {
                // C++ AI_MOVE_AND_EVACUATE unloads only on arrival. A path
                // failure is not an arrival: keep the pending flag so
                // passengers stay contained instead of dumping mid-route.
                let has_passengers = self
                    .game_logic
                    .host_object(unit_id)
                    .is_some_and(|obj| !obj.contained_units().is_empty());
                if !has_passengers {
                    // Wave 233: clear pending evacuate via GameLogic authority API.
                    let _ = self
                        .game_logic
                        .unit_command_set_pending_evacuate(unit_id, false, false, false);
                }
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_dock(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let target_pos = match self.game_logic.host_object(target_id) {
            Some(target)
                if target.is_alive()
                    && !target.status.under_construction
                    && !target.status.sold =>
            {
                target.get_position()
            }
            _ => return CommandResult::InvalidTarget,
        };

        let mut issued = false;
        for &unit_id in units {
            let Some(dock_kind) = self.can_issue_dock(unit_id, target_id) else {
                continue;
            };

            let state = match dock_kind {
                crate::game_logic::DockKind::SupplyCenter => {
                    if !self
                        .game_logic
                        .unit_command_return_supplies(unit_id, target_id)
                    {
                        continue;
                    }
                    AIState::ReturningResources
                }
                crate::game_logic::DockKind::SupplyWarehouse => {
                    if !self
                        .game_logic
                        .unit_command_dock_at_supply_warehouse(unit_id, target_id)
                    {
                        continue;
                    }
                    AIState::Gathering
                }
                crate::game_logic::DockKind::RailedTransport => {
                    if !self
                        .game_logic
                        .unit_command_dock_at_railed_transport(unit_id, target_id)
                    {
                        continue;
                    }
                    AIState::Docking
                }
                crate::game_logic::DockKind::None => continue,
            };

            if self.path_to_goal_with_state(unit_id, target_pos, state)
                || self
                    .game_logic
                    .host_object(unit_id)
                    .is_some_and(|unit| unit.get_position().distance(target_pos) <= 0.1)
            {
                issued = true;
            }
        }
        if issued {
            self.game_logic.queue_picked_unit_voice(
                units,
                crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::Supply,
            );
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_combat_drop(
        &mut self,
        units: &[ObjectId],
        target: &DropTarget,
    ) -> CommandResult {
        // C++ ChinookAIUpdate::privateCombatDrop → MOVE_TO_COMBAT_DROP then
        // DO_COMBAT_DROP hover-rappel. Not MOVE_TO_AND_EVAC (land + dump).
        debug!("Executing combat drop at {:?}", target);
        let (dest, object_target) = match target {
            DropTarget::Location(pos) => (*pos, None),
            DropTarget::Object(target_id) => {
                let Some(target_obj) = self.game_logic.host_object(*target_id) else {
                    return CommandResult::InvalidTarget;
                };
                (target_obj.position, Some(*target_id))
            }
        };
        // C++ MoveToBldg: geom.getMaxHeightAbovePosition(), not object world Y.
        let bldg_h = object_target.and_then(|tid| {
            self.game_logic.host_object(tid).and_then(|o| {
                if o.is_alive() && o.is_kind_of(crate::game_logic::KindOf::Structure) {
                    Some(o.thing.template.geometry_info.max_height_above_position())
                } else {
                    None
                }
            })
        });
        let mut any = false;
        for &unit_id in units {
            let is_chinook = self.game_logic.host_object(unit_id).is_some_and(|o| {
                o.is_alive()
                    && (o.is_combat_chinook_style_container()
                        || o.chinook_ai.is_some()
                        || crate::game_logic::host_combat_chinook::is_regular_chinook_template(
                            &o.template_name,
                        ))
            });
            if !is_chinook {
                continue;
            }
            if crate::command_executor::leftover::host_rappeller_count(self.game_logic, unit_id)
                == 0
            {
                continue;
            }
            if let Some(tid) = object_target {
                let allowed = match (
                    self.game_logic.host_object(unit_id),
                    self.game_logic.host_object(tid),
                ) {
                    (Some(chinook), Some(tgt)) => {
                        crate::game_logic::host_combat_chinook::combat_drop_into_allowed(
                            tgt.is_alive(),
                            tgt.status.under_construction,
                            tgt.status.sold,
                            &tgt.template_name,
                            tgt.thing.template.contain_module.kind
                                != crate::game_logic::ContainModuleKind::None
                                || tgt.can_contain()
                                || tgt.is_garrison_contain(),
                            tgt.thing.template.contain_module.kind.is_heal_contain(),
                            chinook.health.current >= chinook.health.maximum,
                            tgt.is_faction_structure(),
                        )
                    }
                    _ => false,
                };
                if !allowed {
                    continue;
                }
                let _ = self
                    .game_logic
                    .unit_command_set_order_target(unit_id, Some(tid));
            }
            let mut hover = dest;
            if let Some(obj) = self.game_logic.host_object_mut(unit_id) {
                if obj.chinook_ai.is_none() {
                    if crate::game_logic::host_combat_chinook::is_regular_chinook_template(
                        &obj.template_name,
                    ) {
                        obj.install_chinook_transport();
                    } else {
                        obj.install_combat_chinook_transport();
                    }
                }
                let p = obj.get_position();
                if let Some(ai) = obj.chinook_ai.as_mut() {
                    ai.pos = [p.x, p.z, p.y];
                    ai.combat_drop_target = object_target.map(|id| id.0);
                    ai.command_combat_drop([dest.x, dest.z, dest.y], bldg_h);
                    hover.y = ai.combat_drop_dest_z;
                }
            }
            let _ = self
                .game_logic
                .unit_command_set_pending_evacuate(unit_id, true, false, true);
            if self.path_to_goal_with_state(unit_id, hover, AIState::Moving) {
                any = true;
            } else if let Some(obj) = self.game_logic.host_object_mut(unit_id) {
                if let Some(ai) = obj.chinook_ai.as_mut() {
                    ai.arrive_for_combat_drop();
                }
                any = true;
                let _ = self.game_logic.evacuate_container_now(unit_id, false);
            }
        }
        if any {
            // C++ CommandXlat.cpp:348-352 MSG_COMBATDROP_AT_* → PerUnitSound VoiceCombatDrop (skip=true).
            self.game_logic.queue_picked_unit_voice(
                units,
                crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::CombatDrop,
            );
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }
}
