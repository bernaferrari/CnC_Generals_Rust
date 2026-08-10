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
    /// C++ AIGroup::groupExecuteRailedTransport residual.
    pub(crate) fn execute_railed_transport(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let is_railish = match self.game_logic.host_object(unit_id) {
                Some(o) if o.is_alive() => {
                    let n = o.template_name.to_ascii_lowercase();
                    o.can_contain()
                        || n.contains("train")
                        || n.contains("rail")
                        || n.contains("locomotive")
                }
                _ => false,
            };
            if !is_railish {
                continue;
            }
            if matches!(self.execute_evacuate(&[unit_id]), CommandResult::Success) {
                any = true;
            }
            let dest = self.game_logic.host_object(unit_id).and_then(|o| {
                o.movement
                    .path
                    .last()
                    .copied()
                    .or(o.movement.target_position)
            });
            if let Some(dest) = dest {
                if self.path_to_goal_with_state(unit_id, dest, AIState::Moving) {
                    any = true;
                }
            }
        }
        if any {
            CommandResult::Success
        } else {
            self.execute_evacuate(units)
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
                if !obj.has_capacity_for(1) {
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

    pub(super) fn execute_enter(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        // USA Pilot residual: Enter unmanned vehicle for recrew (not transport contain).
        let pilot_recrew_target = self.game_logic.host_object(target_id).map(|t| {
            crate::game_logic::host_usa_pilot::is_recrewable_unmanned_vehicle(
                t.is_alive(),
                t.is_kind_of(crate::game_logic::KindOf::Vehicle),
                t.is_kind_of(crate::game_logic::KindOf::Aircraft) || t.status.airborne_target,
                t.is_unmanned(),
                t.status.under_construction,
                t.is_worker() || t.template_name.to_ascii_lowercase().contains("dozer"),
            )
        });
        let target_pos = match self.game_logic.host_object(target_id) {
            Some(transport)
                if transport.is_alive()
                    && !transport.status.under_construction
                    && (transport.can_contain() || pilot_recrew_target == Some(true)) =>
            {
                transport.get_position()
            }
            _ => return CommandResult::InvalidTarget,
        };

        let mut issued = false;
        for &unit_id in units {
            let pilot_recrew = self.game_logic.host_object(unit_id).map(|u| {
                crate::game_logic::host_usa_pilot::should_recrew_on_enter(
                    crate::game_logic::host_usa_pilot::is_pilot_template(&u.template_name),
                    pilot_recrew_target.unwrap_or(false),
                ) && u.is_alive()
                    && u.can_move()
            });
            if pilot_recrew != Some(true) && !self.can_issue_enter_or_dock(unit_id, target_id) {
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

    pub(super) fn execute_exit(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut to_unload: Vec<(ObjectId, Option<ObjectId>, Vec3)> = Vec::new();
        let mut seen_units: HashSet<ObjectId> = HashSet::new();
        // Tunnel network residual: exit tunnel id for shared-pool bookkeeping.
        let mut tunnel_exit_for: HashMap<ObjectId, ObjectId> = HashMap::new();

        for &selected_id in units {
            let Some(selected_obj) = self.game_logic.host_object(selected_id) else {
                continue;
            };

            if selected_obj.can_contain() {
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
        // C++ AIGroup::groupEvacuate:
        //  - airborne aircraft containers: move to ground then evacuate
        //  - structures without AI: order passengers out
        //  - other AI containers: aiEvacuate(false) → unload residual
        // Host residual: unload selected containers via execute_exit; airborne
        // aircraft path to ground (Y=0) first so chinook-style drop has a dest.
        let mut ground_containers: Vec<ObjectId> = Vec::new();
        let mut airborne_containers: Vec<ObjectId> = Vec::new();
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
                airborne_containers.push(unit_id);
            } else {
                ground_containers.push(unit_id);
            }
        }

        let mut any = false;
        for unit_id in airborne_containers {
            let Some(pos) = self
                .game_logic
                .host_object(unit_id)
                .map(|o| o.get_position())
            else {
                continue;
            };
            // C++: highest ground layer at dest — host residual uses Y=0 ground plane.
            let dest = Vec3::new(pos.x, 0.0, pos.z);
            if self.path_to_goal_with_state(unit_id, dest, AIState::Moving) {
                any = true;
            }
            // Also attempt unload residual if already near ground / has passengers.
            if matches!(self.execute_exit(&[unit_id]), CommandResult::Success) {
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

    pub(super) fn execute_dock(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        let target_pos = if let Some(target) = self.game_logic.host_object(target_id) {
            if target.is_alive() && !target.status.under_construction && target.can_contain() {
                target.get_position()
            } else {
                return CommandResult::InvalidTarget;
            }
        } else {
            return CommandResult::InvalidTarget;
        };

        let mut issued = false;
        for &unit_id in units {
            if !self.can_issue_enter_or_dock(unit_id, target_id) {
                continue;
            }

            // Wave 233: order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_set_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::Docking) {
                issued = true;
            }
        }
        if issued {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_combat_drop(&mut self, units: &[ObjectId], target: &DropTarget) -> CommandResult {
        debug!("Executing combat drop at {:?}", target);
        match target {
            DropTarget::Location(pos) => {
                for &unit_id in units {
                    let _ = self.path_to_goal_with_state(unit_id, *pos, AIState::Entering);
                }
            }
            DropTarget::Object(target_id) => {
                if let Some(target_obj) = self.game_logic.host_object(*target_id) {
                    let target_pos = target_obj.position;
                    for &unit_id in units {
                        // Wave 233: combat-drop order-target via GameLogic authority API.
                        let _ = self
                            .game_logic
                            .unit_command_set_order_target(unit_id, Some(*target_id));
                        let _ =
                            self.path_to_goal_with_state(unit_id, target_pos, AIState::Entering);
                    }
                } else {
                    return CommandResult::InvalidTarget;
                }
            }
        }
        CommandResult::Success
    }

}
