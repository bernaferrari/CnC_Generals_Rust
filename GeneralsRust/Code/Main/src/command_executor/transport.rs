//! Enter, exit, evacuate, dock, combat drop, railed transport.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    radar_notifications::RadarKind, AIState, GameLogic, KindOf, ObjectId, ObjectType,
    PendingSpecialAbility, Resources, Team,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::common::AsciiString;
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};

impl<'a> CommandExecutor<'a> {
    /// C++ `AIGroup::groupExecuteRailedTransport` delegates only to
    /// `RailedTransportAIUpdate::privateExecuteRailedTransport`.
    ///
    /// The active Main host world retains the separate railed dock/contain
    /// metadata, but not the AI module's authored `PathPrefixName`, paired
    /// terrain waypoints, current-path/transit state, or loading interface.
    /// Treating a generic contain object (or a name containing "rail") as a
    /// railed transport used to evacuate passengers and replay an unrelated
    /// movement target.  That is a different command.  Reject until that
    /// complete source-backed runtime exists rather than claiming success or
    /// mutating a player-visible transport.
    pub(crate) fn execute_railed_transport(&mut self, _units: &[ObjectId]) -> CommandResult {
        CommandResult::InvalidCommand
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
        // USA Pilot residual: Enter unmanned vehicle for recrew (not transport contain).
        // A non-container target is legal only when at least one selected
        // source passes the parsed `VeterancyCrateCollide IsPilot` authority
        // predicate.  Do not make a generic unmanned VEHICLE look like a
        // transport while the command is queued.
        let has_pilot_recrew_source = units
            .iter()
            .copied()
            .any(|unit_id| self.game_logic.can_execute_pilot_recrew(unit_id, target_id));
        let target_pos = match self.game_logic.host_object(target_id) {
            Some(transport)
                if transport.is_alive()
                    && !transport.status.under_construction
                    && (transport.can_contain() || has_pilot_recrew_source) =>
            {
                transport.get_position()
            }
            _ => return CommandResult::InvalidTarget,
        };

        let mut issued = false;
        for &unit_id in units {
            let pilot_recrew = self
                .game_logic
                .host_object(unit_id)
                .is_some_and(|unit| {
                    unit.is_alive()
                        && unit.can_move()
                        && self.game_logic.can_execute_pilot_recrew(unit_id, target_id)
                });
            if !pilot_recrew && !self.can_issue_enter(unit_id, target_id) {
                continue;
            }

            let unit_in_tunnel = self
                .game_logic
                .tunnel_network_residual()
                .team_holding_unit(unit_id)
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
                    // Wave 233: remove prior occupant via GameLogic authority API.
                    let _ = self
                        .game_logic
                        .unit_command_remove_occupant(previous_container, unit_id);
                }
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::Entering) {
                issued = true;
            }
        }

        if issued {
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
            .team_holding_unit(id)
            .is_some();
        is_contained || in_tunnel || obj.container_id().is_some() || obj.contained_by.is_some()
    }

    pub(super) fn execute_exit(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut to_unload: Vec<(ObjectId, Option<ObjectId>, Vec3)> = Vec::new();
        let mut seen_units: HashSet<ObjectId> = HashSet::new();
        // Tunnel network residual: exit tunnel id for shared-pool bookkeeping.
        let mut tunnel_exit_for: HashMap<ObjectId, ObjectId> = HashMap::new();

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
                // Prefer get_position() (authoritative Thing pos). The pub `position`
                // field is often left at default ZERO after create_object set_position.
                let origin = selected_obj
                    .building_data
                    .as_ref()
                    .and_then(|b| b.rally_point)
                    .unwrap_or_else(|| selected_obj.get_position());

                // Tunnel Network residual: Evacuate/Exit on ANY team tunnel dumps the
                // shared MaxTunnelCapacity pool at THIS tunnel (cross-tunnel path).
                if selected_obj.is_tunnel_network_style_container() {
                    let team = selected_obj.team;
                    let shared = self.game_logic.tunnel_network_contained_for_team(team);
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
                .team_holding_unit(selected_id)
                .is_some();
            if !is_contained && !in_tunnel {
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
            } else {
                (selected_obj.get_position(), None)
            };

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
                    }
                }
            }
        }

        if to_unload.is_empty() {
            return CommandResult::InvalidCommand;
        }

        for (i, (unit_id, container_id, origin)) in to_unload.into_iter().enumerate() {
            // Stagger exits deterministically to avoid clumping on the same point.
            let angle = (unit_id.0 as f32 + i as f32 * 1.37).sin().atan2(1.0) + i as f32 * 0.7;
            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 6.0;
            let drop_position = origin + offset;

            let tunnel_exit = tunnel_exit_for.get(&unit_id).copied();
            let was_tunnel = if let Some(exit_tid) = tunnel_exit {
                self.game_logic.exit_tunnel_network_unit(unit_id, exit_tid)
            } else if let Some(cid) = container_id {
                // Fallback: unit in shared pool exiting via entry tunnel.
                if self
                    .game_logic
                    .tunnel_network_residual()
                    .team_holding_unit(unit_id)
                    .is_some()
                {
                    self.game_logic.exit_tunnel_network_unit(unit_id, cid)
                } else {
                    false
                }
            } else {
                false
            };

            if !was_tunnel {
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
            ) = if was_tunnel {
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
                let is_structure = container
                    .map(|c| c.is_kind_of(KindOf::Structure))
                    .unwrap_or(false);
                if garrisoned {
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
                        (false, false, false, false, false, false, false, false)
                    }
                } else if unit.contained_by.is_some() || container_id.is_some() {
                    if is_structure {
                        (true, false, false, false, false, false, false, false)
                    } else if is_overlord {
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
                        (false, false, false, false, false, false, false, false)
                    }
                } else {
                    (false, false, false, false, false, false, false, false)
                }
            } else {
                (false, false, false, false, false, false, false, false)
            };

            // Wave 233: exit drop via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_exit_drop(unit_id, drop_position);
            if was_tunnel {
                // Counters already recorded in exit_tunnel_network_unit.
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
        let mut airborne_jobs: Vec<(ObjectId, Vec3)> = Vec::new();
        for &unit_id in units {
            let Some(obj) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            let is_container =
                obj.can_contain() || obj.is_kind_of(crate::game_logic::KindOf::Structure);
            if !is_container {
                continue;
            }
            let airborne =
                obj.is_kind_of(crate::game_logic::KindOf::Aircraft) && obj.status.airborne_target;
            if airborne {
                let pos = obj.get_position();
                let dest_y = self
                    .game_logic
                    .terrain_height_at(pos)
                    .unwrap_or(obj.ground_height);
                airborne_jobs.push((unit_id, Vec3::new(pos.x, dest_y, pos.z)));
            } else {
                ground_containers.push(unit_id);
            }
        }

        let mut any = false;
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
            // Wave 233: pending evacuate prep via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_set_pending_evacuate(unit_id, true, and_exit, true);
            if self.path_to_goal_with_state(unit_id, destination, AIState::Moving) {
                any = true;
            } else {
                // Already at dest or path fail — evacuate immediately.
                let exit = and_exit;
                // Wave 233: clear pending evacuate via GameLogic authority API.
                let _ = self
                    .game_logic
                    .unit_command_set_pending_evacuate(unit_id, false, false, false);
                if self.game_logic.evacuate_container_now(unit_id, exit) {
                    any = true;
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
        let bldg_h = object_target.and_then(|tid| {
            self.game_logic
                .host_object(tid)
                .map(|o| o.get_position().y.max(0.0))
        });
        let mut any = false;
        for &unit_id in units {
            let is_chinook = self.game_logic.host_object(unit_id).is_some_and(|o| {
                o.is_alive()
                    && (o.is_combat_chinook_style_container() || o.chinook_ai.is_some())
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
                let _ = self
                    .game_logic
                    .unit_command_set_order_target(unit_id, Some(tid));
            }
            let mut hover = dest;
            if let Some(obj) = self.game_logic.host_object_mut(unit_id) {
                if obj.chinook_ai.is_none() {
                    obj.install_combat_chinook_transport();
                }
                let p = obj.get_position();
                if let Some(ai) = obj.chinook_ai.as_mut() {
                    ai.pos = [p.x, p.z, p.y];
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
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_executor::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{
        ContainModuleKind, ContainModuleMetadata, GameLogic, KindOf, Team, ThingTemplate,
    };

    fn contain_pair(logic: &mut GameLogic, transport: ObjectId, pax: ObjectId) {
        {
            let t = logic.host_object_mut(transport).unwrap();
            assert!(t.add_occupant(pax));
        }
        {
            let p = logic.host_object_mut(pax).unwrap();
            p.set_contained_by(Some(transport));
            p.set_ai_state(AIState::Docked);
        }
    }

    #[test]
    fn execute_exit_unloads_only_the_selected_occupant() {
        // C++ GameLogicDispatch.cpp:978-1004 MSG_EXIT exits argument 0 occupant,
        // not every rider on the selected container (that is MSG_EVACUATE).
        let mut logic = GameLogic::new();
        for (name, kind, transport) in [
            ("EXIT_T", KindOf::Vehicle, true),
            ("EXIT_A", KindOf::Infantry, false),
            ("EXIT_B", KindOf::Infantry, false),
        ] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(kind);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(200.0);
            if transport {
                tpl.contain_module = ContainModuleMetadata {
                    kind: ContainModuleKind::Transport,
                    slots: Some(5),
                    ..Default::default()
                };
            }
            logic.templates.insert(name.to_string(), tpl);
        }
        let transport = logic
            .create_object("EXIT_T", Team::USA, Vec3::ZERO)
            .unwrap();
        let a = logic
            .create_object("EXIT_A", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("EXIT_B", Team::USA, Vec3::new(2.0, 0.0, 0.0))
            .unwrap();
        contain_pair(&mut logic, transport, a);
        contain_pair(&mut logic, transport, b);

        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(exec.execute_exit(&[a]), CommandResult::Success);
        }
        let remaining = logic
            .host_object(transport)
            .unwrap()
            .contained_units();
        assert_eq!(remaining, vec![b], "EXIT must leave the unclicked occupant");
        assert!(logic.host_object(a).unwrap().contained_by.is_none());
        assert_eq!(
            logic.host_object(b).unwrap().contained_by,
            Some(transport)
        );
    }

    #[test]
    fn execute_combat_drop_uses_hover_not_land_evac() {
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("CD_T");
        t.add_kind_of(KindOf::Aircraft);
        t.add_kind_of(KindOf::Selectable);
        t.set_health(200.0);
        logic.templates.insert("CD_T".to_string(), t);
        let mut p = ThingTemplate::new("CD_P");
        p.add_kind_of(KindOf::Infantry);
        p.add_kind_of(KindOf::Selectable);
        p.set_health(100.0);
        logic.templates.insert("CD_P".to_string(), p);
        let transport = logic
            .create_object("CD_T", Team::USA, Vec3::new(0.0, 100.0, 0.0))
            .unwrap();
        let pax = logic
            .create_object("CD_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        {
            let t = logic.host_object_mut(transport).unwrap();
            t.install_combat_chinook_transport();
            let _ = t.add_occupant(pax);
        }
        {
            let p = logic.host_object_mut(pax).unwrap();
            p.set_contained_by(Some(transport));
            p.set_ai_state(AIState::Docked);
        }
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_combat_drop(
                    &[transport],
                    &DropTarget::Location(Vec3::new(80.0, 0.0, 80.0))
                ),
                CommandResult::Success
            );
        }
        let t = logic.host_object(transport).unwrap();
        assert!(t.pending_evacuate_on_stop);
        let ai = t.chinook_ai.as_ref().expect("chinook_ai");
        assert_eq!(
            ai.state,
            crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToCombatDrop
        );
        assert_ne!(
            ai.flight_status,
            crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
        );
        assert!(
            logic.host_object(pax).unwrap().contained_by.is_some(),
            "passengers stay aboard until hover rappel"
        );
    }

    #[test]
    fn combat_drop_arrival_rappels_not_teleport() {
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("CD_T2");
        t.add_kind_of(KindOf::Aircraft);
        t.set_health(200.0);
        logic.templates.insert("CD_T2".to_string(), t);
        let mut p = ThingTemplate::new("CD_P2");
        p.add_kind_of(KindOf::Infantry);
        p.set_health(100.0);
        logic.templates.insert("CD_P2".to_string(), p);
        let transport = logic
            .create_object("CD_T2", Team::USA, Vec3::new(10.0, 100.0, 10.0))
            .unwrap();
        let pax = logic
            .create_object("CD_P2", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        {
            let t = logic.host_object_mut(transport).unwrap();
            t.install_combat_chinook_transport();
            let _ = t.add_occupant(pax);
            if let Some(ai) = t.chinook_ai.as_mut() {
                ai.command_combat_drop([10.0, 10.0, 0.0], None);
                ai.arrive_for_combat_drop();
            }
        }
        {
            let p = logic.host_object_mut(pax).unwrap();
            p.set_contained_by(Some(transport));
        }
        assert!(logic.evacuate_container_now(transport, false));
        let pax_obj = logic.host_object(pax).unwrap();
        assert!(pax_obj.contained_by.is_none());
        assert!(
            pax_obj.get_position().y > 50.0,
            "rappeller starts at hover, not teleported to ground"
        );
        assert!((pax_obj.movement.max_speed - 30.0).abs() < 0.01);
        assert_eq!(
            logic
                .host_object(transport)
                .unwrap()
                .chinook_ai
                .as_ref()
                .unwrap()
                .ai_free_to_exit(true),
            crate::game_logic::host_combat_chinook::HostChinookFreeToExit::FreeToExit
        );
    }
}

