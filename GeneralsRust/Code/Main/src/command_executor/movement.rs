//! Movement, waypoints, formation travel, patrol, and scatter.
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
    // === Movement Commands ===

    pub(crate) fn execute_move(&mut self, units: &[ObjectId], destination: Vec3) -> CommandResult {
        // Wave 232: move last-writes via GameLogic unit_command_move_free.
        // C++ groupMoveToPosition: click inside group bounds → tighten (all to point).
        if self.should_tighten_group_move(units, destination) {
            return self.execute_tighten_to_position(units, destination);
        }
        // C++ friend_computeGroundPath + friend_moveFormationToPos residual.
        if units.len() > 1 && self.compute_ground_path_should_group(units, destination) {
            let fid0 = units
                .first()
                .and_then(|id| self.game_logic.host_object(*id))
                .map(|o| o.formation_id)
                .unwrap_or(0);
            let is_formation = fid0 != 0
                && units.iter().all(|&id| {
                    self.game_logic
                        .host_object(id)
                        .map(|o| o.formation_id == fid0)
                        .unwrap_or(false)
                });
            if is_formation {
                return self.execute_move_formation_to_position(units, destination);
            }
        }
        let goals = self.group_move_destinations(units, destination);
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            if !self
                .game_logic
                .unit_command_move_free(unit_id, goal, destination)
            {
                // Distinguish missing unit vs path failure like prior residual.
                if self.game_logic.host_object(unit_id).is_none() {
                    return CommandResult::InvalidTarget;
                }
                return CommandResult::InvalidCommand;
            }
            moved.push(unit_id);
            debug!("Unit {} moving to {:?}", unit_id.0, goal);
        }
        self.apply_player_stealth_mood_delay(&moved);
        CommandResult::Success
    }

    pub(super) fn execute_move_to(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> CommandResult {
        // Wave 232: move last-writes via GameLogic unit_command_move_to_waypoints.
        if waypoints.is_empty() && self.should_tighten_group_move(units, destination) {
            return self.execute_tighten_to_position(units, destination);
        }
        let goals = self.group_move_destinations(units, destination);
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            if !self
                .game_logic
                .unit_command_move_to_waypoints(unit_id, goal, waypoints)
            {
                return CommandResult::InvalidCommand;
            }
            moved.push(unit_id);
            debug!("Unit {} moving via waypoints to {:?}", unit_id.0, goal);
        }
        self.apply_player_stealth_mood_delay(&moved);
        CommandResult::Success
    }

    /// C++ AIGroup::groupMoveToPosition / computeIndividualDestination residual.
    ///
    /// Sort movers near→far to the click, take the nearest unit as the free-move
    /// "center", then offset each unit's goal by its (clamped) vector from that
    /// center — preserves relative formation instead of inventing a ring.

    /// C++ AIGroup player move/stop stealth residual: delay mood auto-acquire until
    /// unstealthed combat stealth units can cloak again.
    pub(super) fn apply_player_stealth_mood_delay(&mut self, unit_ids: &[ObjectId]) {
        // Wave 233: stealth mood delay via GameLogic authority API.
        let now = self.game_logic.get_frame();
        for (i, &unit_id) in unit_ids.iter().enumerate() {
            let skew = (i as u32) % 30;
            let _ = self
                .game_logic
                .unit_command_apply_stealth_mood_delay(unit_id, now, skew);
        }
    }

    pub(crate) fn group_move_destinations(
        &self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> Vec<(ObjectId, Vec3)> {
        if units.is_empty() {
            return Vec::new();
        }
        if units.len() == 1 {
            return vec![(units[0], destination)];
        }

        // Gather movable members with positions (skip dead / immobile).
        let mut movers: Vec<(ObjectId, Vec3, f32, u32, glam::Vec2, bool, bool)> =
            Vec::with_capacity(units.len());
        for &unit_id in units {
            let Some(obj) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Immobile)
                || obj.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            let radius = obj.selection_radius.max(5.0);
            movers.push((
                unit_id,
                obj.get_position(),
                radius,
                obj.formation_id,
                obj.formation_offset,
                obj.is_kind_of(crate::game_logic::KindOf::Infantry),
                obj.is_kind_of(crate::game_logic::KindOf::Vehicle),
            ));
        }
        if movers.is_empty() {
            return units.iter().map(|&id| (id, destination)).collect();
        }
        if movers.len() == 1 {
            return vec![(movers[0].0, destination)];
        }

        // Shared non-zero formation id → C++ formation move offsets.
        let fid0 = movers[0].3;
        let is_formation = fid0 != 0 && movers.iter().all(|m| m.3 == fid0);
        if is_formation {
            return movers
                .into_iter()
                .map(|(id, _pos, _r, _fid, off, _inf, _veh)| {
                    (
                        id,
                        Vec3::new(destination.x + off.x, destination.y, destination.z + off.y),
                    )
                })
                .collect();
        }

        // C++ friend_moveInfantryToPos / friend_moveVehicleToPos residual:
        // when enough pure infantry or vehicles move far enough, pack into columns
        // along the move direction instead of free-move center offsets.
        if let Some(column) = self.group_column_destinations(&movers, destination) {
            return column;
        }

        // Near-to-far vs goal (C++ SimpleObjectIterator ITER_SORTED_NEAR_TO_FAR).
        movers.sort_by(|a, b| {
            let da = (a.1.x - destination.x).hypot(a.1.z - destination.z);
            let db = (b.1.x - destination.x).hypot(b.1.z - destination.z);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Free-move center is the nearest unit's current position (C++ firstUnit branch).
        let center = movers[0].1;
        let mut out = Vec::with_capacity(movers.len());
        for (i, (unit_id, pos, radius, _fid, _off, _inf, _veh)) in movers.into_iter().enumerate() {
            let goal = if i == 0 {
                destination
            } else {
                let mut dx = pos.x - center.x;
                let mut dz = pos.z - center.z;
                let mut length = (dx * dx + dz * dz).sqrt();
                let max_length = 6.0 * radius;
                if length > max_length {
                    length = max_length;
                }
                if length > 0.001 {
                    let nlen = (dx * dx + dz * dz).sqrt().max(0.001);
                    dx = (dx / nlen) * length;
                    dz = (dz / nlen) * length;
                } else {
                    let angle = (i as f32) * 1.7;
                    dx = angle.cos() * radius * 0.5;
                    dz = angle.sin() * radius * 0.5;
                }
                Vec3::new(destination.x + dx, destination.y, destination.z + dz)
            };
            out.push((unit_id, goal));
        }
        out
    }

    /// C++ GlobalData::m_groupMoveClickToGatherFactor residual (1.0 = full bbox).
    const GROUP_MOVE_CLICK_TO_GATHER_FACTOR: f32 = 1.0;

    /// True when destination lies inside the selected group's XZ bounding rect
    /// scaled by gather factor — C++ groupMoveToPosition tighten path.
    pub(crate) fn should_tighten_group_move(&self, units: &[ObjectId], destination: Vec3) -> bool {
        if Self::GROUP_MOVE_CLICK_TO_GATHER_FACTOR <= 0.0 || units.len() < 2 {
            return false;
        }
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        let mut count = 0u32;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !o.is_alive() || !o.can_move() {
                continue;
            }
            if o.is_kind_of(crate::game_logic::KindOf::Immobile)
                || o.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            // Airborne fixed-wing: C++ disables tighten.
            if o.is_kind_of(crate::game_logic::KindOf::Aircraft)
                && o.status.airborne_target
                && !o.template_name.to_ascii_lowercase().contains("heli")
                && !o.template_name.to_ascii_lowercase().contains("chinook")
                && !o.template_name.to_ascii_lowercase().contains("comanche")
            {
                return false;
            }
            let p = o.get_position();
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
            count += 1;
        }
        if count < 2 {
            return false;
        }
        // Scale rect about center by gather factor.
        let cx = 0.5 * (min_x + max_x);
        let cz = 0.5 * (min_z + max_z);
        let hx = 0.5 * (max_x - min_x) * Self::GROUP_MOVE_CLICK_TO_GATHER_FACTOR;
        let hz = 0.5 * (max_z - min_z) * Self::GROUP_MOVE_CLICK_TO_GATHER_FACTOR;
        // Pad tiny groups so a click near the cluster still gathers.
        let hx = hx.max(20.0);
        let hz = hz.max(20.0);
        destination.x >= cx - hx
            && destination.x <= cx + hx
            && destination.z >= cz - hz
            && destination.z <= cz + hz
    }

    /// C++ AIGroup::groupTightenToPosition — near-to-far, all path to same pos.
    pub(crate) fn execute_tighten_to_position(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        // Wave 232: tighten last-writes via GameLogic unit_command_tighten_to.
        if !destination.x.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        // Sort near-to-far (C++ SimpleObjectIterator ITER_SORTED_NEAR_TO_FAR).
        let mut movers: Vec<(ObjectId, f32)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            let p = unit.get_position();
            let dx = p.x - destination.x;
            let dz = p.z - destination.z;
            movers.push((unit_id, dx * dx + dz * dz));
        }
        movers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut any = false;
        for (unit_id, _) in movers {
            if self
                .game_logic
                .unit_command_tighten_to(unit_id, destination)
            {
                any = true;
            }
        }
        if any {
            self.apply_player_stealth_mood_delay(units);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupFollowWaypointPath / Exact / AsTeam residual.
    pub(crate) fn execute_follow_waypoint_path(
        &mut self,
        units: &[ObjectId],
        waypoints: &[Vec3],
        exact: bool,
        as_team: bool,
    ) -> CommandResult {
        // `exact` → assign_unit_path_exact (C++ AIFollowWaypointPathExactState).
        if waypoints.is_empty() {
            return CommandResult::InvalidLocation;
        }
        for wp in waypoints {
            if !wp.x.is_finite() || !wp.z.is_finite() {
                return CommandResult::InvalidLocation;
            }
        }

        // Collect movers + optional formation/group offsets (AsTeam residual).
        let mut movers: Vec<(ObjectId, Vec3, glam::Vec2)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            movers.push((unit_id, unit.get_position(), unit.formation_offset));
        }
        if movers.is_empty() {
            return CommandResult::InvalidCommand;
        }

        // Group center from current positions.
        let (mut cx, mut cz) = (0.0f32, 0.0f32);
        for (_, pos, _) in &movers {
            cx += pos.x;
            cz += pos.z;
        }
        let n = movers.len() as f32;
        cx /= n;
        cz /= n;

        // Prefer stamped formation offsets when shared; else relative-to-center.
        let fid0 = self
            .game_logic
            .host_object(movers[0].0)
            .map(|o| o.formation_id)
            .unwrap_or(0);
        let use_formation = as_team
            && fid0 != 0
            && movers.iter().all(|(id, _, _)| {
                self.game_logic
                    .host_object(*id)
                    .map(|o| o.formation_id == fid0)
                    .unwrap_or(false)
            });

        // Near-to-far vs first waypoint.
        let first = waypoints[0];
        movers.sort_by(|a, b| {
            let da = (a.1.x - first.x).hypot(a.1.z - first.z);
            let db = (b.1.x - first.x).hypot(b.1.z - first.z);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut any = false;
        for (unit_id, pos, form_off) in movers {
            let offset = if as_team {
                if use_formation {
                    form_off
                } else {
                    glam::Vec2::new(pos.x - cx, pos.z - cz)
                }
            } else {
                glam::Vec2::ZERO
            };

            let unit_wps: Vec<Vec3> = waypoints
                .iter()
                .map(|wp| Vec3::new(wp.x + offset.x, wp.y, wp.z + offset.y))
                .collect();
            let goal = *unit_wps.last().unwrap();
            let via = &unit_wps[..unit_wps.len().saturating_sub(1)];

            // Wave 233: waypoint-path prep via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_waypoint_path_prep(unit_id, as_team);
            // C++ AIFollowWaypointPathExact vs smoothed follow residual.
            let ok = if exact {
                self.game_logic.assign_unit_path_exact(unit_id, goal, via)
            } else {
                self.game_logic.assign_unit_path(unit_id, goal, via)
            };
            if ok {
                any = true;
            } else if self.path_to_goal_with_state(unit_id, goal, AIState::Moving) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup column path residual (infantry 3-col / vehicle group).
    /// Fail-closed: not full ground-path node following; destination-side pack only.
    pub(super) fn group_column_destinations(
        &self,
        movers: &[(ObjectId, Vec3, f32, u32, glam::Vec2, bool, bool)],
        destination: Vec3,
    ) -> Option<Vec<(ObjectId, Vec3)>> {
        use crate::game_logic::host_ai_path_combat_residual_wave105::{
            MIN_DISTANCE_FOR_GROUP_RESIDUAL, MIN_INFANTRY_FOR_GROUP_RESIDUAL,
            MIN_VEHICLES_FOR_GROUP_RESIDUAL,
        };

        let n = movers.len() as i32;
        let all_infantry = movers.iter().all(|m| m.5);
        let all_vehicles = movers.iter().all(|m| m.6);
        if !all_infantry && !all_vehicles {
            return None;
        }
        let min_count = if all_infantry {
            MIN_INFANTRY_FOR_GROUP_RESIDUAL
        } else {
            MIN_VEHICLES_FOR_GROUP_RESIDUAL
        };
        if n < min_count {
            return None;
        }

        let mut center = Vec3::ZERO;
        for m in movers {
            center += m.1;
        }
        center /= movers.len() as f32;

        let mut dir_x = destination.x - center.x;
        let mut dir_z = destination.z - center.z;
        let dist = (dir_x * dir_x + dir_z * dir_z).sqrt();
        if dist < MIN_DISTANCE_FOR_GROUP_RESIDUAL {
            return None;
        }
        dir_x /= dist;
        dir_z /= dist;
        // Perpendicular (C++ startVectorNormal: (-y, x) on XY → (-z, x) on XZ).
        let nx = -dir_z;
        let nz = dir_x;

        // Sort by projection on normal (C++ FAR_TO_NEAR on normal dot).
        let mut ordered: Vec<(ObjectId, Vec3, f32, f32)> = movers
            .iter()
            .map(|m| {
                let dx = m.1.x - center.x;
                let dz = m.1.z - center.z;
                let proj = dx * nx + dz * nz;
                (m.0, m.1, m.2, proj)
            })
            .collect();
        ordered.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));

        let num_columns = 3i32;
        let half = num_columns / 2;
        let units_to_path = ordered.len() as i32;
        // C++: spacing uses path cell size; host residual ≈ average radius.
        let avg_r: f32 = ordered.iter().map(|o| o.2).sum::<f32>() / (ordered.len() as f32).max(1.0);
        let col_spacing = avg_r.max(8.0) * 1.25;
        let rank_spacing = avg_r.max(8.0) * 1.5;

        let mut out = Vec::with_capacity(ordered.len());
        for (cur_index, (id, _pos, _r, _proj)) in ordered.into_iter().enumerate() {
            let cur_index = cur_index as i32;
            // C++: divisor = (unitsToPath+1)/numColumns; columnDelta = 1 - curIndex/divisor
            let mut divisor = (units_to_path + 1) / num_columns;
            if divisor < 1 {
                divisor = 1;
            }
            let mut column_delta = 1 - (cur_index / divisor);
            if column_delta < -half {
                column_delta = -half;
            }
            if column_delta > half {
                column_delta = half;
            }
            // Rank depth along move direction (rows).
            let rank = cur_index / num_columns;
            let goal = Vec3::new(
                destination.x + nx * (column_delta as f32) * col_spacing
                    - dir_x * (rank as f32) * rank_spacing,
                destination.y,
                destination.z + nz * (column_delta as f32) * col_spacing
                    - dir_z * (rank as f32) * rank_spacing,
            );
            out.push((id, goal));
        }
        Some(out)
    }

    /// C++ AIGroup::groupAttackMoveToPosition residual.
    /// ableToAttack → attack-move path + maxShots; else plain move.
    pub(crate) fn execute_attack_move(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
        max_shots: i32,
    ) -> CommandResult {
        // Wave 232: attack-move last-writes via GameLogic unit_command_attack_move_to_ex.
        if !destination.x.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        let goals = self.group_move_destinations(units, destination);
        let mut any = false;
        for (unit_id, goal) in goals {
            if self
                .game_logic
                .unit_command_attack_move_to_ex(unit_id, goal, max_shots)
            {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_force_move(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        // Wave 232: force-move via GameLogic unit_command_force_move_to.
        let goals = self.group_move_destinations(units, destination);
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            if !self.game_logic.unit_command_force_move_to(unit_id, goal) {
                return CommandResult::InvalidCommand;
            }
            moved.push(unit_id);
        }
        self.apply_player_stealth_mood_delay(&moved);
        CommandResult::Success
    }

    pub(crate) fn execute_add_waypoint(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        // C++ groupMoveToPosition(addWaypoint): individual dests + path append.
        let goals = self.group_move_destinations(units, destination);
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            if self.game_logic.host_object(unit_id).is_none() {
                return CommandResult::InvalidTarget;
            }
            if !self.game_logic.append_unit_waypoint(unit_id, goal) {
                return CommandResult::InvalidCommand;
            }
            moved.push(unit_id);
            debug!("Added waypoint for unit {} at {:?}", unit_id.0, goal);
        }
        self.apply_player_stealth_mood_delay(&moved);
        CommandResult::Success
    }

    /// C++ AIGroup::friend_moveFormationToPos residual.
    /// Paths each formation member to dest + stamped offset.
    pub(crate) fn execute_move_formation_to_position(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        // Wave 232: formation move via unit_command_force_move_to.
        if !destination.x.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        // Ensure formation stamps exist.
        let need_stamp = {
            let fid0 = units
                .first()
                .and_then(|id| self.game_logic.host_object(*id))
                .map(|o| o.formation_id)
                .unwrap_or(0);
            fid0 == 0
                || !units.iter().all(|&id| {
                    self.game_logic
                        .host_object(id)
                        .map(|o| o.formation_id == fid0 && fid0 != 0)
                        .unwrap_or(false)
                })
        };
        if need_stamp {
            let _ = self.execute_create_formation(units);
        }
        let goals = self.group_move_destinations(units, destination);
        let mut any = false;
        for (unit_id, goal) in goals {
            if self.game_logic.unit_command_force_move_to(unit_id, goal)
                || self.path_to_goal_with_state(unit_id, goal, AIState::Moving)
            {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupFollowPath residual (empty body in retail) —
    /// host uses non-exact waypoint follow.
    pub(crate) fn execute_follow_path(
        &mut self,
        units: &[ObjectId],
        path: &[Vec3],
    ) -> CommandResult {
        self.execute_follow_waypoint_path(units, path, false, false)
    }

    pub(crate) fn execute_patrol(&mut self, units: &[ObjectId]) -> CommandResult {
        // Wave 232: patrol last-writes via GameLogic unit_command_patrol.
        let mut any = false;
        for &unit_id in units {
            if self.game_logic.unit_command_patrol(unit_id) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(crate) fn execute_scatter(&mut self, units: &[ObjectId]) -> CommandResult {
        // Wave 232: scatter last-writes via GameLogic unit_command_move_to_moving.
        // C++ AIGroup::groupScatter — far-to-near from group center, push out by
        // 4 * bounding radius along the unit→center vector (host XZ plane).
        let mut movers: Vec<(ObjectId, Vec3, f32)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            let pos = unit.get_position();
            let radius = unit.selection_radius.max(5.0);
            movers.push((unit_id, pos, radius));
        }
        if movers.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut center = Vec3::ZERO;
        for (_, pos, _) in &movers {
            center += *pos;
        }
        center /= movers.len() as f32;

        movers.sort_by(|a, b| {
            let da = (a.1.x - center.x).hypot(a.1.z - center.z);
            let db = (b.1.x - center.x).hypot(b.1.z - center.z);
            db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut any = false;
        let mut center_nudge = center;
        for (unit_id, pos, radius) in movers {
            center_nudge.x -= 0.01;
            let mut dx = pos.x - center_nudge.x;
            let mut dz = pos.z - center_nudge.z;
            let len = (dx * dx + dz * dz).sqrt();
            if len > 0.001 {
                dx /= len;
                dz /= len;
            } else {
                dx = 1.0;
                dz = 0.0;
            }
            let push = 4.0 * radius;
            let dest = Vec3::new(pos.x + dx * push, pos.y, pos.z + dz * push);
            if self.game_logic.unit_command_move_to_moving(unit_id, dest) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }
}
