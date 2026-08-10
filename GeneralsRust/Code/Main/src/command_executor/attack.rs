//! Attack, guard, stop, attitude, and weapon-set/lock commands.
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
    /// C++ AIGroup::setWeaponSetFlag residual.
    pub(crate) fn execute_set_weapon_set_flag(
        &mut self,
        units: &[ObjectId],
        flag: u8,
        enabled: bool,
    ) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            // Wave 233: weapon-set flag via GameLogic authority API.
            if self
                .game_logic
                .unit_command_set_weapon_set_flag(unit_id, flag, enabled)
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

    /// C++ AIAttackFollowWaypointPathState residual —
    /// follow path while able to auto-engage (attack-move along waypoints).
    pub(crate) fn execute_attack_follow_waypoint_path(
        &mut self,
        units: &[ObjectId],
        waypoints: &[Vec3],
        exact: bool,
        as_team: bool,
    ) -> CommandResult {
        // Wave 232: promote attack-path via GameLogic unit_command_promote_attack_path.
        let path_res = self.execute_follow_waypoint_path(units, waypoints, exact, as_team);
        if !matches!(path_res, CommandResult::Success) {
            return path_res;
        }
        // Promote movers that can attack into AttackMoving + is_attack_path.
        for &unit_id in units {
            let _ = self.game_logic.unit_command_promote_attack_path(unit_id);
        }
        CommandResult::Success
    }

    /// C++ AIGroup::groupAttackTeam residual.
    pub(crate) fn execute_attack_team(
        &mut self,
        units: &[ObjectId],
        team_code: u8,
        _max_shots: i32,
    ) -> CommandResult {
        // Wave 232: attack-team last-writes via unit_command_attack_soft.
        use crate::game_logic::Team;
        let enemy_team = match team_code {
            0 => Team::GLA,
            1 => Team::USA,
            2 => Team::China,
            _ => return CommandResult::InvalidTarget,
        };
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            // Host residual: allow attack order even before weapon bind (can_attack may be false).
            if !unit.is_alive() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Structure) && !unit.can_attack() {
                continue;
            }
            let my_team = unit.team;
            if my_team == enemy_team {
                continue;
            }
            let origin = unit.get_position();
            // Nearest living enemy of that team.
            let mut best: Option<(ObjectId, f32)> = None;
            for (cid, cand) in self.game_logic.host_objects().iter() {
                if cand.team != enemy_team || !cand.is_alive() {
                    continue;
                }
                let d = origin.distance(cand.get_position());
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((*cid, d));
                }
            }
            if let Some((tid, _)) = best {
                if self.game_logic.unit_command_attack_soft(unit_id, tid) {
                    any = true;
                }
                let tpos = self.game_logic.host_object(tid).map(|o| o.get_position());
                if let Some(pos) = tpos {
                    let _ = self.path_to_goal_with_state(unit_id, pos, AIState::Attacking);
                }
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    // === Combat Commands ===

    pub(super) fn execute_attack(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        // Wave 232: attack last-writes via GameLogic unit_command_attack.
        let Some(target_team) = self
            .game_logic
            .host_object(target_id)
            .map(|target| target.team)
        else {
            return CommandResult::InvalidTarget;
        };

        if self
            .game_logic
            .host_object(target_id)
            .is_some_and(|target| !target.is_alive())
        {
            return CommandResult::TargetDestroyed;
        }

        // C++ groupAttackObjectPrivate: sort attackers near-to-far to victim first.
        let target_pos = self
            .game_logic
            .host_object(target_id)
            .map(|tg| tg.get_position())
            .unwrap_or(Vec3::ZERO);
        let mut ordered: Vec<(ObjectId, f32)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.can_attack() || unit.team == target_team {
                continue;
            }
            let p = unit.get_position();
            let d = (p.x - target_pos.x).hypot(p.z - target_pos.z);
            ordered.push((unit_id, d));
        }
        ordered.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut any_attacker = false;
        for (unit_id, _) in ordered {
            if self.game_logic.unit_command_attack(unit_id, target_id) {
                any_attacker = true;
            }
        }

        if any_attacker {
            CommandResult::Success
        } else {
            CommandResult::CannotAttackTarget
        }
    }

    pub(super) fn execute_attack_object(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        self.execute_attack(units, target_id)
    }

    pub(super) fn execute_force_attack(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        // Wave 232: force-attack last-writes via GameLogic unit_command_force_attack.
        if !self.validate_target_exists(target_id) {
            return CommandResult::InvalidTarget;
        }

        if self
            .game_logic
            .host_object(target_id)
            .is_some_and(|target| !target.is_alive())
        {
            return CommandResult::TargetDestroyed;
        }

        // C++ groupAttackObjectPrivate(forced=true): near-to-far order.
        let target_pos = self
            .game_logic
            .host_object(target_id)
            .map(|tg| tg.get_position())
            .unwrap_or(Vec3::ZERO);
        let mut ordered: Vec<(ObjectId, f32)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.can_attack() {
                continue;
            }
            let p = unit.get_position();
            let d = (p.x - target_pos.x).hypot(p.z - target_pos.z);
            ordered.push((unit_id, d));
        }
        ordered.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut any_attacker = false;
        for (unit_id, _) in ordered {
            if self
                .game_logic
                .unit_command_force_attack(unit_id, target_id)
            {
                any_attacker = true;
            }
        }

        if any_attacker {
            CommandResult::Success
        } else {
            CommandResult::CannotAttackTarget
        }
    }

    /// C++ AIGroup::groupGuardObject residual helper.
    pub(crate) fn execute_guard_object(
        &mut self,
        units: &[ObjectId],
        target: ObjectId,
        mode: crate::game_logic::GuardMode,
    ) -> CommandResult {
        self.execute_guard(
            units,
            &crate::command_system::GuardTarget::Object(target),
            mode,
        )
    }

    /// C++ AIGroup::groupGuardArea residual — area approximated as position + radius guard.
    pub(crate) fn execute_guard_area(
        &mut self,
        units: &[ObjectId],
        center: Vec3,
        radius: f32,
        mode: crate::game_logic::GuardMode,
    ) -> CommandResult {
        // Wave 232: guard area radius last-write via unit_command_set_guard_radius.
        let res = self.execute_guard(
            units,
            &crate::command_system::GuardTarget::Position(center),
            mode,
        );
        if matches!(res, CommandResult::Success) {
            let r = radius.max(80.0);
            for &id in units {
                let _ = self.game_logic.unit_command_set_guard_radius(id, r);
            }
        }
        res
    }

    /// C++ AIGroup::groupAttackPosition residual.
    /// `location` None → each unit attacks its own position.
    /// Orders fire-capable passengers when container allows passenger fire.
    pub(crate) fn execute_attack_ground(
        &mut self,
        units: &[ObjectId],
        location: Option<Vec3>,
        max_shots: i32,
    ) -> CommandResult {
        // Wave 232: attack-ground last-writes via unit_command_attack_ground_ex.
        let mut any = false;
        let mut extra_passengers: Vec<ObjectId> = Vec::new();

        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            // Collect fire-capable passengers (garrison residual).
            if unit.passengers_allowed_to_fire {
                for p in unit.contained_units() {
                    extra_passengers.push(p);
                }
            }
        }

        let mut all_units: Vec<ObjectId> = units.to_vec();
        for p in extra_passengers {
            if !all_units.contains(&p) {
                all_units.push(p);
            }
        }

        for &unit_id in &all_units {
            let attack_pos = match location {
                Some(loc) => {
                    if !loc.x.is_finite() || !loc.z.is_finite() {
                        continue;
                    }
                    loc
                }
                None => match self.game_logic.host_object(unit_id) {
                    Some(u) if u.is_alive() => u.get_position(),
                    _ => continue,
                },
            };

            if self
                .game_logic
                .unit_command_attack_ground_ex(unit_id, attack_pos, max_shots)
            {
                any = true;
            }
            // Face/path residual: movable units approach the ground point if far.
            let need_approach = self.game_logic.host_object(unit_id).and_then(|unit| {
                if !unit.can_move() {
                    return None;
                }
                let pos = unit.get_position();
                let dist = (pos.x - attack_pos.x).hypot(pos.z - attack_pos.z);
                let range = unit.weapon.as_ref().map(|w| w.range).unwrap_or(50.0);
                if dist > range.max(20.0) {
                    Some(attack_pos)
                } else {
                    None
                }
            });
            if let Some(dest) = need_approach {
                let _ = self.path_to_goal_with_state(unit_id, dest, AIState::AttackingGround);
            }
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(crate) fn execute_stop(&mut self, units: &[ObjectId]) -> CommandResult {
        // Wave 232: stop last-writes via GameLogic unit_command_stop.
        // C++ AIGroup::groupIdle (player stop):
        // aiIdle + stealth combat unit mood delay until stealthed again.
        for &unit_id in units {
            let _ = self.game_logic.unit_command_stop(unit_id);
        }
        self.apply_player_stealth_mood_delay(units);
        CommandResult::Success
    }

    pub(crate) fn execute_guard(
        &mut self,
        units: &[ObjectId],
        target: &GuardTarget,
        mode: crate::game_logic::GuardMode,
    ) -> CommandResult {
        // Wave 232: guard last-writes via GameLogic unit_command_guard_full.
        // C++ AIGroup::groupGuardPosition/Object — only units with AI/move;
        // guard radius residual ≈ adjusted vision (getStdGuardRange).
        const GUARD_MIN_RADIUS: f32 = 80.0;
        let mut any = false;
        for &unit_id in units {
            let (can, vision, weapon_r) = match self.game_logic.host_object(unit_id) {
                Some(unit)
                    if unit.is_alive()
                        && unit.can_move()
                        && !unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                        && !unit.is_kind_of(crate::game_logic::KindOf::Structure) =>
                {
                    let wr = unit
                        .weapon
                        .as_ref()
                        .map(|w| w.range)
                        .or_else(|| unit.secondary_weapon.as_ref().map(|w| w.range))
                        .unwrap_or(0.0);
                    (true, unit.vision_range, wr)
                }
                _ => (false, 0.0, 0.0),
            };
            if !can {
                continue;
            }

            let target_pos = match target {
                GuardTarget::Position(pos) => Some(*pos),
                GuardTarget::Object(target_id) => self
                    .game_logic
                    .host_object(*target_id)
                    .filter(|o| o.is_alive())
                    .map(|o| o.get_position()),
            };

            let guard_radius = vision.max(weapon_r).max(GUARD_MIN_RADIUS);
            let (position, obj_target) = match target {
                GuardTarget::Position(pos) => (Some(*pos), None),
                GuardTarget::Object(target_id) => (None, Some(*target_id)),
            };
            if !self.game_logic.unit_command_guard_full(
                unit_id,
                position,
                obj_target,
                guard_radius,
                mode,
            ) {
                continue;
            }

            match target {
                GuardTarget::Position(pos) => {
                    let _ = self.path_to_goal_with_state(unit_id, *pos, AIState::GuardingArea);
                }
                GuardTarget::Object(_) => {
                    if let Some(pos) = target_pos {
                        let _ = self.path_to_goal_with_state(unit_id, pos, AIState::GuardingObject);
                    }
                }
            }
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_set_attitude(
        &mut self,
        units: &[ObjectId],
        attitude: crate::game_logic::host_strategy_center::HostAiAttitude,
    ) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            // Wave 233: AI attitude via GameLogic authority API.
            if self
                .game_logic
                .unit_command_set_ai_attitude(unit_id, attitude)
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

    /// C++ AIGroup::setWeaponLockForGroup residual.
    pub(crate) fn execute_set_weapon_lock(
        &mut self,
        units: &[ObjectId],
        slot: u8,
        lock_type_code: u8,
    ) -> CommandResult {
        // Wave 233: weapon lock via GameLogic unit_command_set_weapon_lock.
        use crate::game_logic::WeaponLockType;
        let lock_type = match lock_type_code {
            1 => WeaponLockType::LockedTemporarily,
            2 => WeaponLockType::LockedPermanently,
            _ => WeaponLockType::NotLocked,
        };
        let mut any = false;
        for &unit_id in units {
            if self
                .game_logic
                .unit_command_set_weapon_lock(unit_id, slot, lock_type)
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

    /// C++ AIGroup::releaseWeaponLockForGroup residual.
    pub(crate) fn execute_release_weapon_lock(
        &mut self,
        units: &[ObjectId],
        lock_type_code: u8,
    ) -> CommandResult {
        // Wave 233: release weapon lock via GameLogic authority API.
        use crate::game_logic::WeaponLockType;
        let lock_type = match lock_type_code {
            1 => WeaponLockType::LockedTemporarily,
            _ => WeaponLockType::LockedPermanently,
        };
        let mut any = false;
        for &unit_id in units {
            if self
                .game_logic
                .unit_command_release_weapon_lock(unit_id, lock_type)
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

    /// C++ AIGroup::groupAttackArea residual — engage nearest enemy inside radius.
    pub(crate) fn execute_attack_area(
        &mut self,
        units: &[ObjectId],
        center: Vec3,
        radius: f32,
    ) -> CommandResult {
        let radius = radius.max(1.0);
        if !center.x.is_finite() || !center.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_attack() {
                continue;
            }
            let team = unit.team;
            // Find nearest enemy of this unit inside area.
            let mut best: Option<(ObjectId, f32)> = None;
            for (cid, cand) in self.game_logic.host_objects().iter() {
                if !cand.is_alive() || !cand.is_targetable_by_enemy_of(team) {
                    continue;
                }
                let d = center.distance(cand.get_position());
                if d > radius {
                    continue;
                }
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((*cid, d));
                }
            }
            if let Some((enemy_id, _)) = best {
                // Reuse attack path.
                if matches!(
                    self.execute_attack_object(&[unit_id], enemy_id),
                    CommandResult::Success
                ) {
                    any = true;
                }
            } else {
                // No target: move to area center (C++ attack-area still enters state).
                if unit.can_move()
                    && self.path_to_goal_with_state(unit_id, center, AIState::AttackMoving)
                {
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

}
