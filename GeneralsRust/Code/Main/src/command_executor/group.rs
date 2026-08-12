//! AIGroup query/filter/formation helpers.
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
    /// C++ AIData::m_distanceRequiresGroup residual (force group moving when far).
    const DISTANCE_REQUIRES_GROUP: f32 = 200.0;

    /// C++ AIData::m_minDistanceForGroup residual.
    const MIN_DISTANCE_FOR_GROUP: f32 = 40.0;

    /// C++ AIGroup::getCommandButtonSourceObject residual —
    /// first living member that can act on `command` capability.
    pub(crate) fn command_button_source_object(
        &self,
        units: &[ObjectId],
        command: &crate::command_system::CommandType,
    ) -> Option<ObjectId> {
        use crate::command_system::CommandType;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            let ok = match command {
                CommandType::Attack { .. }
                | CommandType::AttackObject { .. }
                | CommandType::ForceAttackObject { .. }
                | CommandType::AttackPosition { .. }
                | CommandType::ForceAttackGround { .. }
                | CommandType::AttackMoveTo { .. }
                | CommandType::AttackFollowWaypointPath { .. } => {
                    // Prefer a member that actually carries a weapon module.
                    o.weapon.is_some()
                }
                CommandType::Move { .. }
                | CommandType::MoveTo { .. }
                | CommandType::ForceMoveTo { .. }
                | CommandType::FollowWaypointPath { .. }
                | CommandType::Scatter { .. }
                | CommandType::Guard { .. } => o.can_move(),
                CommandType::Stop => true,
                CommandType::Evacuate | CommandType::MoveToAndEvacuate { .. } => {
                    o.can_contain() || !o.contained_units().is_empty()
                }
                CommandType::DoSpecialPower { power_type, .. } => {
                    self.game_logic.is_special_power_ready_for(id, power_type)
                        || o.special_power_cooldowns.contains_key(power_type)
                }
                CommandType::Sell { .. } | CommandType::ToggleOvercharge => {
                    o.is_kind_of(crate::game_logic::KindOf::Structure)
                }
                CommandType::HackInternet => {
                    o.can_move() || o.template_name.to_ascii_lowercase().contains("hacker")
                }
                CommandType::GetRepaired { .. } | CommandType::GetHealed { .. } => o.can_move(),
                CommandType::CreateFormation => o.can_move(),
                _ => {
                    // Fall open: any living selectable member.
                    o.is_kind_of(crate::game_logic::KindOf::Selectable) || o.can_move()
                }
            };
            if ok {
                return Some(id);
            }
        }
        None
    }

    /// C++ AIGroup::getAllIDs residual — living members in selection order.
    pub(crate) fn group_all_ids(&self, units: &[ObjectId]) -> Vec<ObjectId> {
        let mut out = Vec::with_capacity(units.len());
        for &id in units {
            if self
                .game_logic
                .host_object(id)
                .map(|o| o.is_alive())
                .unwrap_or(false)
            {
                out.push(id);
            }
        }
        out
    }

    /// C++ AIGroup::getAttitude residual — retail always returns AI_PASSIVE.
    pub(crate) fn group_attitude(
        &self,
        _units: &[ObjectId],
    ) -> crate::game_logic::host_strategy_center::HostAiAttitude {
        crate::game_logic::host_strategy_center::HostAiAttitude::Passive
    }

    /// C++ AIGroup::getCount residual.
    pub(crate) fn group_count(&self, units: &[ObjectId]) -> usize {
        units
            .iter()
            .filter(|&&id| {
                self.game_logic
                    .host_object(id)
                    .map(|o| o.is_alive())
                    .unwrap_or(false)
            })
            .count()
    }

    /// C++ AIGroup::getSpeed / recompute residual —
    /// slowest non-held, non-immobile locomotor among members whose body
    /// damage state is BETTER than MovementPenaltyDamageState (REALLYDAMAGED).
    /// Heavily damaged units do not drag the whole group down.
    pub(crate) fn group_speed(&self, units: &[ObjectId]) -> f32 {
        use crate::game_logic::host_ai_path_combat_residual_wave105::{
            calc_damage_state_residual, is_body_condition_better, BODY_REALLYDAMAGED,
        };
        let mut best = f32::INFINITY;
        let mut saw = false;
        for &id in units {
            // Authoritative HP first (GameWorld when coupled) so we do not
            // hold a host_object borrow across a second game_logic call.
            let auth_hp = self.game_logic.host_authoritative_health(id);
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
            if o.contained_by.is_some() {
                continue; // HELD residual — skip riders
            }
            let max_h = o.health.maximum.max(1.0);
            // Fail-closed when no authoritative HP (no HashMap mid-frame truth).
            let Some(cur_h) = auth_hp else {
                continue;
            };
            let dmg = calc_damage_state_residual(cur_h, max_h);
            // C++: only if IS_CONDITION_BETTER(damageState, movementPenaltyDamageState)
            if !is_body_condition_better(dmg, BODY_REALLYDAMAGED) {
                continue;
            }
            let spd = o.effective_max_speed().max(0.0);
            if spd > 0.0 && spd < best {
                best = spd;
                saw = true;
            }
        }
        if saw {
            best
        } else {
            0.0
        }
    }

    /// C++ AIGroup::recompute leadership residual —
    /// closest non-immobile, non-held member to group center.
    pub(crate) fn group_leader_id(&self, units: &[ObjectId]) -> Option<ObjectId> {
        let (_, _, center) = self.group_min_max_and_center(units)?;
        let mut best_id = None;
        let mut best_d2 = f32::INFINITY;
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
            if o.contained_by.is_some() {
                continue;
            }
            let p = o.get_position();
            let d2 = (p.x - center.x).powi(2) + (p.z - center.z).powi(2);
            if d2 < best_d2 {
                best_d2 = d2;
                best_id = Some(id);
            }
        }
        best_id
    }

    /// C++ AIGroup::getMinMaxAndCenter residual (XZ plane; skip held).
    /// Returns (min_xz, max_xz, center) or None if empty.
    pub(crate) fn group_min_max_and_center(
        &self,
        units: &[ObjectId],
    ) -> Option<(glam::Vec2, glam::Vec2, Vec3)> {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        let mut cz = 0.0f32;
        let mut count = 0u32;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !o.is_alive() || o.contained_by.is_some() {
                continue;
            }
            let p = o.get_position();
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
            cx += p.x;
            cy += p.y;
            cz += p.z;
            count += 1;
        }
        if count == 0 {
            return None;
        }
        let n = count as f32;
        Some((
            glam::Vec2::new(min_x, min_z),
            glam::Vec2::new(max_x, max_z),
            Vec3::new(cx / n, cy / n, cz / n),
        ))
    }

    /// C++ AIGroup::friend_computeGroundPath residual (simplified).
    /// True when the group should path as a formation/group toward `dest`.
    pub(crate) fn compute_ground_path_should_group(&self, units: &[ObjectId], dest: Vec3) -> bool {
        let Some((min, max, center)) = self.group_min_max_and_center(units) else {
            return false;
        };
        let mut num_infantry = 0u32;
        let mut num_vehicles = 0u32;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !o.is_alive() || o.contained_by.is_some() {
                continue;
            }
            if o.is_kind_of(crate::game_logic::KindOf::Infantry)
                || o.object_type == crate::game_logic::ObjectType::Infantry
            {
                num_infantry += 1;
            } else if o.is_kind_of(crate::game_logic::KindOf::Vehicle)
                && !o.is_kind_of(crate::game_logic::KindOf::Aircraft)
            {
                num_vehicles += 1;
            }
        }
        if num_infantry + num_vehicles == 0 {
            return false;
        }

        // Closest unit → dest distance.
        let mut closest_sqr = f32::INFINITY;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            let p = o.get_position();
            let d2 = (p.x - dest.x).powi(2) + (p.z - dest.z).powi(2);
            closest_sqr = closest_sqr.min(d2);
        }
        let bbox_dx = max.x - min.x;
        let bbox_dz = max.y - min.y;
        let mut span_sqr = bbox_dx * bbox_dx + bbox_dz * bbox_dz;
        let req = Self::DISTANCE_REQUIRES_GROUP;
        let min_d = Self::MIN_DISTANCE_FOR_GROUP;
        if span_sqr > req * req {
            // Use group span as the distance metric (C++).
            closest_sqr = span_sqr;
        }
        if closest_sqr < min_d * min_d {
            return false;
        }
        let mut close_enough = closest_sqr > req * req;
        if num_infantry > 6 {
            close_enough = true;
        }
        if num_vehicles > 4 {
            close_enough = true;
        }
        // Formation already stamped → always group-path.
        let fid0 = units
            .first()
            .and_then(|id| self.game_logic.host_object(*id))
            .map(|o| o.formation_id)
            .unwrap_or(0);
        if fid0 != 0
            && units.iter().all(|&id| {
                self.game_logic
                    .host_object(id)
                    .map(|o| o.formation_id == fid0)
                    .unwrap_or(false)
            })
        {
            close_enough = true;
        }
        let _ = center;
        close_enough
    }

    /// C++ AIGroup::isMember residual.
    pub(crate) fn is_member(&self, units: &[ObjectId], obj: ObjectId) -> bool {
        units.iter().any(|&id| id == obj)
    }

    /// C++ AIGroup::getCenter residual (skip held/immobile without move).
    pub(crate) fn group_center(&self, units: &[ObjectId]) -> Option<Vec3> {
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        let mut cz = 0.0f32;
        let mut count = 0u32;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            // C++ skips DISABLED_HELD riders.
            if o.contained_by.is_some() {
                continue;
            }
            if o.is_kind_of(crate::game_logic::KindOf::Immobile)
                && !o.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                // Still count structures with AI-like commands; skip pure immobile props.
            }
            let p = o.get_position();
            cx += p.x;
            cy += p.y;
            cz += p.z;
            count += 1;
        }
        if count == 0 {
            // Fallback: any alive member.
            for &id in units {
                if let Some(o) = self.game_logic.host_object(id) {
                    if o.is_alive() {
                        return Some(o.get_position());
                    }
                }
            }
            return None;
        }
        let n = count as f32;
        Some(Vec3::new(cx / n, cy / n, cz / n))
    }

    /// C++ AIGroup::containsAnyObjectsNotOwnedByPlayer residual.
    pub(crate) fn contains_any_objects_not_owned_by_player(
        &self,
        units: &[ObjectId],
        player_id: u32,
    ) -> bool {
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if o.owner_player_id != Some(player_id) {
                return true;
            }
        }
        false
    }

    /// C++ AIGroup::removeAnyObjectsNotOwnedByPlayer residual.
    /// Returns (kept_units, group_now_empty).
    pub(crate) fn remove_any_objects_not_owned_by_player(
        &self,
        units: &[ObjectId],
        player_id: u32,
    ) -> (Vec<ObjectId>, bool) {
        let kept: Vec<ObjectId> = units
            .iter()
            .copied()
            .filter(|&id| {
                self.game_logic
                    .host_object(id)
                    .map(|o| o.owner_player_id == Some(player_id))
                    .unwrap_or(false)
            })
            .collect();
        let empty = kept.is_empty();
        (kept, empty)
    }

    /// C++ AIGroup::isIdle residual — every member idle or effectively dead.
    pub(crate) fn group_is_idle(&self, units: &[ObjectId]) -> bool {
        let mut saw = false;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            saw = true;
            if o.is_alive() && !matches!(o.ai_state, AIState::Idle) {
                return false;
            }
        }
        saw
    }

    /// C++ AIGroup::isBusy residual — every living member is non-idle/busy.
    pub(crate) fn group_is_busy(&self, units: &[ObjectId]) -> bool {
        let mut saw = false;
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            saw = true;
            // Host residual: busy = not idle (C++ AIUpdateInterface::isBusy is narrower).
            if matches!(o.ai_state, AIState::Idle) {
                return false;
            }
        }
        saw
    }

    /// C++ AIGroup::isGroupAiDead residual — every member effectively dead.
    pub(crate) fn group_is_ai_dead(&self, units: &[ObjectId]) -> bool {
        if units.is_empty() {
            return true;
        }
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if o.is_alive() {
                return false;
            }
        }
        true
    }

    // === Formation Commands ===

    pub(crate) fn execute_create_formation(&mut self, units: &[ObjectId]) -> CommandResult {
        // Wave 232: formation stamp via GameLogic unit_command_set_formation.
        // C++ AIGroup::groupCreateFormation — stamp formation id + offset from
        // centroid. Does NOT path units or enter GuardingArea.
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut members: Vec<(ObjectId, Vec3, u32)> = Vec::new();
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
            members.push((unit_id, unit.get_position(), unit.formation_id));
        }
        if members.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut center = Vec3::ZERO;
        for (_, pos, _) in &members {
            center += *pos;
        }
        center /= members.len() as f32;

        // C++: if already a formation (shared id, or single unit with id), clear.
        let mut is_formation = false;
        if members.len() == 1 && members[0].2 != 0 {
            is_formation = true;
        } else if members.len() >= 2 {
            let first_id = members[0].2;
            if first_id != 0 && members.iter().all(|m| m.2 == first_id) {
                is_formation = true;
            }
        }

        let new_id = if is_formation {
            0 // NO_FORMATION_ID — dissolve
        } else {
            self.game_logic.alloc_formation_id()
        };

        for (unit_id, pos, _) in members {
            let _ = self.game_logic.unit_command_set_formation(
                unit_id,
                new_id,
                glam::Vec2::new(pos.x - center.x, pos.z - center.z),
            );
        }

        CommandResult::Success
    }
}
