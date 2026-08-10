//! Host scripts `impl GameLogic` — `unit_commands`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! pick / unit commands
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Wave 246: world-position object pick without exposing object dual-walk to callers.
    ///
    /// Priority bands mirror command_integration residual acquire:
    /// - with selection: enemy attackable, then friendly selectable, then other
    /// - without selection: own selectable only
    pub fn pick_object_id_at_world(
        &self,
        origin: glam::Vec3,
        player_team: Option<Team>,
        has_selected_units: bool,
        base_selection_radius: f32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_residual_acquire::{
            pick_best_priority_residual_target, PriorityAcquireCandidate,
        };

        let cands: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                let distance = (pos - origin).length();
                let radius = base_selection_radius.max(obj.selection_radius);
                if distance > radius {
                    return None;
                }
                let priority = if has_selected_units {
                    match player_team {
                        Some(team) if obj.team != team && obj.is_attackable() => Some(0),
                        Some(team) if obj.team == team && obj.is_selectable() => Some(1),
                        _ if obj.is_attackable() => Some(2),
                        _ if obj.is_selectable() => Some(3),
                        _ => None,
                    }
                } else {
                    match player_team {
                        Some(team) if obj.team == team && obj.is_selectable() => Some(0),
                        Some(_) => None,
                        None if obj.is_selectable() => Some(0),
                        None => None,
                    }
                };
                Some(PriorityAcquireCandidate {
                    id,
                    position: pos,
                    is_alive: true,
                    priority,
                })
            })
            .collect();

        pick_best_priority_residual_target(
            ObjectId(0),
            origin,
            (origin.x, origin.z),
            f32::MAX,
            cands,
        )
        .map(|(id, _, _)| id)
    }

    #[inline]
    pub fn unit_is_dead_or_missing(&self, id: ObjectId) -> bool {
        match self.objects.get(&id) {
            Some(o) => !o.is_alive(),
            None => true,
        }
    }

    /// Prepare move: stop attack then assign path (fallback set_destination).
    /// Wave 230/232: stop attack residual then path or set destination + Moving.
    pub fn unit_command_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        if !self.unit_can_move(id) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        let ok = if self.assign_unit_path(id, destination, &[]) {
            true
        } else if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_destination(destination);
            true
        } else {
            false
        };
        if ok {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_ai_state(AIState::Moving);
            }
        }
        ok
    }

    /// Wave 232: path with waypoints after stop_attack (executor move_to residual).
    pub fn unit_command_move_to_waypoints(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        waypoints: &[glam::Vec3],
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        self.assign_unit_path(id, destination, waypoints)
    }

    /// Wave 232: force-move — stop attack, path, force Moving state.
    pub fn unit_command_force_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        if !self.assign_unit_path(id, destination, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ai_state(AIState::Moving);
        }
        true
    }

    /// Wave 230/232: attack target (records host attack log).
    pub fn unit_command_attack(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.can_attack() {
            return false;
        }
        unit.set_force_attack(false);
        unit.set_target(Some(target_id));
        crate::game_logic::host_attack_log::record(id, Some(target_id));
        unit.set_ai_state(AIState::Attacking);
        true
    }

    /// Wave 230/232: force-attack target (records host attack log).
    pub fn unit_command_force_attack(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.can_attack() {
            return false;
        }
        unit.set_target(Some(target_id));
        crate::game_logic::host_attack_log::record(id, Some(target_id));
        unit.set_force_attack(true);
        unit.set_ai_state(AIState::Attacking);
        true
    }

    /// Wave 230/232: full player stop (idle + clear guard/target/force + logs).
    pub fn unit_command_stop(&mut self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop();
        unit.set_target(None);
        unit.set_force_attack(false);
        unit.set_guard_position(None);
        unit.set_guard_target(None);
        crate::game_logic::host_attack_log::record(id, None);
        crate::game_logic::host_guard_log::record(id, None, 0, 0.0);
        unit.end_guard_retaliate();
        unit.set_ai_state(AIState::Idle);
        true
    }

    pub fn unit_command_guard_position(&mut self, id: ObjectId, pos: glam::Vec3) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_guard_position(Some(pos));
        unit.set_ai_state(AIState::GuardingArea);
        true
    }

    pub fn unit_command_guard_object(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_guard_target(Some(target_id));
        unit.set_ai_state(AIState::GuardingObject);
        true
    }

    /// Wave 231/232: attack-move via path + AttackMoving state.
    pub fn unit_command_attack_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        self.unit_command_attack_move_to_ex(id, destination, -1)
    }

    /// Wave 232: attack-move with max-shots + attack-path flags (executor residual).
    pub fn unit_command_attack_move_to_ex(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        max_shots: i32,
    ) -> bool {
        let (can_move, can_attack) = match self.objects.get(&id) {
            Some(unit) => (
                unit.is_alive() && unit.can_move(),
                unit.can_attack() || unit.weapon.is_some(),
            ),
            None => return false,
        };
        if !can_move {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
            unit.set_force_attack(false);
            unit.set_max_shots_to_fire(max_shots);
        }
        let path_ok = self.assign_unit_path(id, destination, &[]);
        if !path_ok {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_destination(destination);
            } else {
                return false;
            }
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if can_attack {
                unit.is_attack_path = true;
                unit.auto_acquire_when_idle = true;
                unit.set_ai_state(AIState::AttackMoving);
            } else {
                unit.is_attack_path = false;
                unit.set_ai_state(AIState::Moving);
            }
            return true;
        }
        false
    }

    /// Wave 232: promote unit onto attack-move path after waypoint follow.
    pub fn unit_command_promote_attack_path(&mut self, id: ObjectId) -> bool {
        let can_attack = self
            .objects
            .get(&id)
            .map(|u| u.is_alive() && (u.can_attack() || u.weapon.is_some()))
            .unwrap_or(false);
        if !can_attack {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if let Some(slot) = unit.find_waypoint_following_capable_weapon_slot() {
                unit.set_active_weapon_slot(slot);
            }
            unit.is_attack_path = true;
            unit.set_ai_state(AIState::AttackMoving);
            return true;
        }
        false
    }

    /// Wave 231: force-attack ground location.
    pub fn unit_command_attack_ground(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.can_attack() {
            return false;
        }
        unit.set_target_location(Some(location));
        unit.set_ai_state(AIState::AttackingGround);
        true
    }

    /// Wave 231: move helper that always leaves unit in Moving state (scatter/formation).
    pub fn unit_command_move_to_moving(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        if !self.unit_can_move(id) {
            return false;
        }
        let path_ok = self.assign_unit_path(id, destination, &[]);
        if let Some(unit) = self.objects.get_mut(&id) {
            if !path_ok {
                unit.set_destination(destination);
            }
            unit.set_ai_state(AIState::Moving);
            return true;
        }
        false
    }

    /// Wave 232: dozer construct — path/destination + Constructing AI state.
    pub fn unit_command_begin_construct(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if !self.assign_unit_path(id, location, &[]) {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_destination(location);
            } else {
                return false;
            }
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ai_state(AIState::Constructing);
            return true;
        }
        false
    }

    /// Wave 231: additive selection mark on a selectable friendly unit.
    pub fn unit_select_if_team(&mut self, id: ObjectId, player_team: Team) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.team == player_team && obj.is_selectable() {
            obj.select();
            obj.flash_as_selected();
            true
        } else {
            false
        }
    }

    /// Wave 232: path after stop_attack; optionally clear formation id (free move).
    pub fn unit_command_move_clear_formation(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        clear_formation: bool,
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        if !self.assign_unit_path(id, destination, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if clear_formation && unit.formation_id != 0 {
                unit.set_formation(0, glam::Vec2::ZERO);
            }
            unit.set_ai_state(AIState::Moving);
        }
        true
    }

    /// Wave 232: tighten-group prep — stop attack, clear formation/guard, then Moving path.
    pub fn unit_command_tighten_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && u.can_move()
                    && !u.is_kind_of(KindOf::Immobile)
                    && !u.is_kind_of(KindOf::Structure)
            })
            .unwrap_or(false);
        if !can {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
            unit.set_formation(0, glam::Vec2::ZERO);
            unit.set_guard_position(None);
            unit.set_guard_target(None);
            unit.end_guard_retaliate();
        }
        let path_ok = self.assign_unit_path(id, destination, &[]);
        if let Some(unit) = self.objects.get_mut(&id) {
            if !path_ok {
                unit.set_destination(destination);
            }
            unit.set_ai_state(AIState::Moving);
            return true;
        }
        false
    }

    /// Wave 232: stamp formation id + offset (create/dissolve).
    pub fn unit_command_set_formation(
        &mut self,
        id: ObjectId,
        formation_id: u32,
        offset: glam::Vec2,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_formation(formation_id, offset);
        true
    }

    /// Wave 232: full guard order (position or object) with radius/mode.
    /// Returns whether unit accepted the order; caller may still path.
    pub fn unit_command_guard_full(
        &mut self,
        id: ObjectId,
        position: Option<glam::Vec3>,
        target: Option<ObjectId>,
        guard_radius: f32,
        mode: GuardMode,
    ) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && u.can_move()
                    && !u.is_kind_of(KindOf::Immobile)
                    && !u.is_kind_of(KindOf::Structure)
            })
            .unwrap_or(false);
        if !can {
            return false;
        }
        // For object guard, require living target position when provided.
        let (gpos, gtarget, ai_state) = if let Some(tid) = target {
            let tpos = self
                .objects
                .get(&tid)
                .filter(|o| o.is_alive())
                .map(|o| o.get_position());
            if tpos.is_none() {
                return false;
            }
            (
                tpos.map(|p| [p.x, p.y, p.z]),
                tid.0,
                AIState::GuardingObject,
            )
        } else if let Some(pos) = position {
            (Some([pos.x, pos.y, pos.z]), 0u32, AIState::GuardingArea)
        } else {
            return false;
        };
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.guard_radius = guard_radius;
            unit.set_guard_mode(mode);
            unit.set_target(None);
            unit.set_force_attack(false);
            unit.end_guard_retaliate();
            if let Some(tid) = target {
                unit.guard_position = None;
                unit.set_guard_target(Some(tid));
            } else if let Some(pos) = position {
                unit.set_guard_target(None);
                unit.set_guard_position(Some(pos));
            }
            unit.set_ai_state(ai_state);
            crate::game_logic::host_guard_log::record(id, gpos, gtarget, guard_radius);
            crate::game_logic::host_attack_log::record(id, None);
            return true;
        }
        false
    }

    /// Wave 232: set guard radius only (guard area residual).
    pub fn unit_command_set_guard_radius(&mut self, id: ObjectId, radius: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.guard_radius = radius;
        true
    }

    /// Wave 232: attack-ground with max shots + force flag.
    pub fn unit_command_attack_ground_ex(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
        max_shots: i32,
    ) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && (u.can_attack() || u.weapon.is_some() || u.is_kind_of(KindOf::Structure))
            })
            .unwrap_or(false);
        if !can {
            // Still allow soft structure residual if alive.
            let alive = self.objects.get(&id).map(|u| u.is_alive()).unwrap_or(false);
            if !alive {
                return false;
            }
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_target(None);
        unit.set_force_attack(true);
        unit.set_max_shots_to_fire(max_shots);
        unit.set_target_location(Some(location));
        unit.set_ai_state(AIState::AttackingGround);
        true
    }

    /// Wave 232: hunt/patrol residual.
    pub fn unit_command_patrol(&mut self, id: ObjectId) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && u.can_move()
                    && !u.is_kind_of(KindOf::Immobile)
                    && !u.is_kind_of(KindOf::Structure)
            })
            .unwrap_or(false);
        if !can {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_target(None);
            unit.set_force_attack(false);
            unit.set_guard_position(None);
            unit.set_guard_target(None);
            unit.end_guard_retaliate();
            crate::game_logic::host_guard_log::record(id, None, 0, 0.0);
            crate::game_logic::host_attack_log::record(id, None);
            unit.auto_acquire_when_idle = true;
            unit.set_ai_state(AIState::Patrolling);
            unit.set_status_moving(false);
            return true;
        }
        false
    }

    /// Wave 232: cheer model-condition residual.
    pub fn unit_command_cheer(
        &mut self,
        id: ObjectId,
        cheer_secs: f32,
        cheer_bit: Option<usize>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.begin_cheer(cheer_secs, cheer_bit);
        true
    }

    /// Wave 232: toggle deployed status for deploy-style units.
    pub fn unit_command_set_deployed(&mut self, id: ObjectId, deployed: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_deployed(deployed);
        if deployed {
            unit.set_ai_state(AIState::Idle);
        }
        true
    }

    /// Wave 232: attack nearest of team without force flag (attack-team residual).
    pub fn unit_command_attack_soft(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_target(Some(target_id));
        unit.set_force_attack(false);
        unit.set_ai_state(AIState::Attacking);
        true
    }

    /// Wave 232: free group move — stop attack, path, clear formation if goal not offset.
    pub fn unit_command_move_free(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        click_destination: glam::Vec3,
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        if !self.assign_unit_path(id, goal, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            // C++ setFormationID(NO_FORMATION) on free individual move when goal is not
            // the stamped formation offset destination.
            if unit.formation_id != 0 {
                let off = unit.formation_offset;
                let expected = glam::Vec3::new(
                    click_destination.x + off.x,
                    click_destination.y,
                    click_destination.z + off.y,
                );
                if (goal - expected).length() > 0.5 {
                    unit.set_formation(0, glam::Vec2::ZERO);
                }
            }
            unit.set_ai_state(AIState::Moving);
        }
        true
    }

    /// Wave 233: set order target (records host attack/order log residual).
    pub fn unit_command_set_order_target(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target(target);
        true
    }

    /// Wave 233: stop moving then set order target (enter/gather/dock residual).
    pub fn unit_command_stop_moving_order_target(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_moving();
        unit.set_order_target(target);
        true
    }

    /// Wave 233: set AI attitude residual.
    pub fn unit_command_set_ai_attitude(
        &mut self,
        id: ObjectId,
        attitude: crate::game_logic::host_strategy_center::HostAiAttitude,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_ai_attitude(attitude);
        true
    }

    /// Wave 233: building orientation stamp after under-construction create.
    pub fn unit_command_set_orientation(&mut self, id: ObjectId, orientation: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_orientation(orientation);
        true
    }

    /// Wave 233: set building rally point + host_rally_log.
    pub fn unit_command_set_rally_point(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        building.rally_point = Some(location);
        crate::game_logic::host_rally_log::record(id, Some([location.x, location.y, location.z]));
        true
    }

    /// Wave 233: return-supplies order target + ReturningResources state.
    pub fn unit_command_return_supplies(&mut self, id: ObjectId, supply_center: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target(Some(supply_center));
        unit.set_ai_state(AIState::ReturningResources);
        true
    }

    /// Wave 233: waypoint-path prep — stop attack and clear guard anchors.
    pub fn unit_command_waypoint_path_prep(&mut self, id: ObjectId, as_team: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_attack();
        unit.set_guard_position(None);
        unit.set_guard_target(None);
        unit.end_guard_retaliate();
        // AsTeam keeps formation identity; free follow clears it.
        if !as_team {
            unit.set_formation(0, glam::Vec2::ZERO);
        }
        true
    }

    /// Wave 233: remove occupant from container (enter/exit residual).
    pub fn unit_command_remove_occupant(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) -> bool {
        let Some(container) = self.objects.get_mut(&container_id) else {
            return false;
        };
        container.remove_occupant(occupant_id);
        true
    }

    /// Wave 233: exit-unit drop residual (position/contain/target/ai).
    pub fn unit_command_exit_drop(&mut self, id: ObjectId, drop_position: glam::Vec3) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_moving();
        unit.set_position(drop_position);
        unit.set_contained_by(None);
        unit.set_target(None);
        unit.set_ai_state(AIState::Idle);
        unit.set_status_moving(false);
        unit.set_status_attacking(false);
        true
    }

    /// Wave 233: mine-clearing weapon-set detail residual.
    pub fn unit_command_set_mine_clearing_detail(&mut self, id: ObjectId, enabled: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_weapon_set_mine_clearing_detail(enabled);
        true
    }

    /// Wave 233: evacuate-on-stop pending flags residual.
    pub fn unit_command_set_pending_evacuate(
        &mut self,
        id: ObjectId,
        pending_evacuate_on_stop: bool,
        pending_exit_after_evacuate: bool,
        prep_move: bool,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.pending_evacuate_on_stop = pending_evacuate_on_stop;
        obj.pending_exit_after_evacuate = pending_exit_after_evacuate;
        if prep_move {
            obj.set_target(None);
            obj.set_force_attack(false);
            obj.set_guard_position(None);
            obj.set_guard_target(None);
            obj.end_guard_retaliate();
        }
        true
    }

    /// Wave 233: order target + Entering state (deploy-to-garrison residual).
    pub fn unit_command_order_enter(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target(Some(target_id));
        unit.set_ai_state(AIState::Entering);
        true
    }

    /// Wave 233: set weapon-set flag residual.
    pub fn unit_command_set_weapon_set_flag(
        &mut self,
        id: ObjectId,
        flag: u8,
        enabled: bool,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_weapon_set_flag(flag, enabled)
    }

    /// Wave 233: surrender residual.
    pub fn unit_command_set_surrendered(&mut self, id: ObjectId, surrendered: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_surrendered(surrendered);
        true
    }

    /// Wave 233: set AI state if alive.
    pub fn unit_command_set_ai_state(&mut self, id: ObjectId, state: AIState) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_ai_state(state);
        true
    }

    /// Wave 233: weapon fire at object/location residual.
    pub fn unit_command_fire_weapon(
        &mut self,
        id: ObjectId,
        target_object: Option<ObjectId>,
        target_location: Option<glam::Vec3>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if let Some(tid) = target_object {
            unit.set_target(Some(tid));
            unit.set_ai_state(AIState::Attacking);
        } else if let Some(pos) = target_location {
            unit.target_location = Some(pos);
            unit.set_ai_state(AIState::AttackingGround);
        } else {
            return false;
        }
        true
    }

    /// Wave 233: infantry go-prone residual.
    pub fn unit_command_go_prone(&mut self, id: ObjectId, prone_secs: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        if unit.is_kind_of(KindOf::Structure) || unit.is_kind_of(KindOf::Immobile) {
            return false;
        }
        let is_infantry =
            unit.is_kind_of(KindOf::Infantry) || unit.object_type == ObjectType::Infantry;
        if !is_infantry {
            return false;
        }
        unit.go_prone(prone_secs);
        true
    }

    /// Wave 233: emoticon residual.
    pub fn unit_command_set_emoticon(
        &mut self,
        id: ObjectId,
        name: &str,
        duration_frames: i32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_emoticon(name, duration_frames);
        true
    }

    /// Wave 233: weapon lock residual.
    pub fn unit_command_set_weapon_lock(
        &mut self,
        id: ObjectId,
        slot: u8,
        lock_type: WeaponLockType,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_weapon_lock(slot, lock_type)
    }

    /// Wave 233: release weapon lock residual.
    pub fn unit_command_release_weapon_lock(
        &mut self,
        id: ObjectId,
        lock_type: WeaponLockType,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.release_weapon_lock(lock_type);
        true
    }

    /// Wave 233: switch weapons residual.
    pub fn unit_command_switch_weapons(&mut self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        let next = unit.active_weapon_slot ^ 1;
        if unit.weapon_slot(next).is_some() {
            let _ = unit.set_weapon_lock(next, WeaponLockType::LockedPermanently);
        } else {
            unit.set_active_weapon_slot(next);
        }
        unit.set_ai_state(AIState::SpecialAbility);
        true
    }

    /// Wave 233: special-power overridable destination residual.
    pub fn unit_command_set_special_power_overridable_destination(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_special_power_overridable_destination(location, None);
        true
    }

    /// Wave 233: queue upgrade on producer building residual.
    pub fn unit_command_building_add_upgrade_to_queue(
        &mut self,
        id: ObjectId,
        upgrade_name: &str,
        research_secs: f32,
        cost: Resources,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        building.add_upgrade_to_queue(upgrade_name.to_string(), research_secs, cost)
    }

    /// Wave 233: remove upgrade entry from producer production queue.
    pub fn unit_command_building_remove_upgrade_from_queue(
        &mut self,
        id: ObjectId,
        upgrade_name: &str,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        let before = building.production_queue.len();
        building.production_queue.retain(|item| {
            !(item.is_upgrade() && item.template_name.eq_ignore_ascii_case(upgrade_name))
        });
        building.production_queue.len() < before
    }

    /// Wave 233: path then set AI state (executor path_to_goal_with_state residual).
    pub fn unit_command_path_with_state(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        state: AIState,
    ) -> bool {
        if !self.assign_unit_path(id, goal, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ai_state(state);
            return true;
        }
        false
    }

    /// Wave 233: C++ groupIdle stealth mood delay residual for one unit.
    pub fn unit_command_apply_stealth_mood_delay(
        &mut self,
        id: ObjectId,
        now_frame: u32,
        skew: u32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        let can_stealth = unit.innate_stealth || unit.stealth_delay_frames > 0;
        if can_stealth
            && unit.auto_acquire_when_idle
            && unit.can_attack()
            && !unit.status.stealthed
            && !unit.status.detected
        {
            let delay = unit.stealth_delay_frames.max(1);
            unit.next_mood_check_time = now_frame.saturating_add(delay).saturating_add(skew);
            return true;
        }
        false
    }

    #[inline]
    pub fn unit_position_if_movable(&self, id: ObjectId) -> Option<glam::Vec3> {
        self.objects
            .get(&id)
            .filter(|o| o.can_move())
            .map(|o| o.get_position())
    }

    /// Wave 227: world position probe without exposing `&Object` to engine dual-read paths.
    #[inline]
    pub fn object_position(&self, id: ObjectId) -> Option<glam::Vec3> {
        self.objects.get(&id).map(|o| o.get_position())
    }

    /// Wave 224: host residual — force-complete an under-construction structure
    /// (train/construct producer path). Authority mutation owned by GameLogic.
    pub fn force_complete_construction(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.construction_percent = 1.0;
        obj.status.under_construction = false;
        obj.health.current = obj.health.maximum;
        true
    }

    /// Wave 224: host residual — ensure barracks `building_data` for force-picked
    /// producers so production queue identity is honest without engine dual-scan.
    pub fn ensure_barracks_building_data(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let need_bd = obj.building_data.is_none()
            || obj
                .building_data
                .as_ref()
                .map(|b| !matches!(b.building_type, BuildingType::Barracks))
                .unwrap_or(true);
        let name_ok = obj.template_name.to_ascii_lowercase().contains("barracks")
            || obj.is_kind_of(KindOf::FSBarracks);
        // Wave 834: also accept force-complete residual producers that already
        // look like infantry factories (Barracks building_type stamp only).
        if need_bd && name_ok {
            // Mirror engine residual: stamp Barracks building_data when missing/mismatched.
            obj.building_data = Some(BuildingData::new(BuildingType::Barracks));
            return true;
        }
        false
    }

    /// Wave 834: force-stamp Barracks building_data for auto_target train residual
    /// when the producer is already known (host spawn / force-complete path).
    pub fn force_ensure_barracks_building_data(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.building_data = Some(BuildingData::new(BuildingType::Barracks));
        obj.status.under_construction = false;
        obj.construction_percent = 1.0;
        true
    }

    /// Wave 225: clear movement path / target on a unit (host residual).
    pub fn clear_unit_movement_path(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.movement.path.is_empty() && obj.movement.target_position.is_none() {
            return false;
        }
        obj.movement.path.clear();
        obj.movement.current_path_index = 0;
        obj.movement.target_position = None;
        obj.status.moving = false;
        true
    }

    /// Wave 225: adjust guard radius residual on a unit. Returns new radius when applied.
    pub fn adjust_unit_guard_radius(&mut self, id: ObjectId, delta: f32) -> Option<f32> {
        let obj = self.objects.get_mut(&id)?;
        let guarding = matches!(
            obj.ai_state,
            AIState::GuardingArea | AIState::GuardingObject
        ) || obj.guard_position.is_some()
            || obj.guard_target.is_some();
        if !guarding && obj.guard_radius <= 0.0 {
            let base = obj.selection_radius.max(20.0) * 2.0;
            obj.guard_radius = (base + delta).clamp(30.0, 400.0);
        } else {
            let cur = if obj.guard_radius > 1.0 {
                obj.guard_radius
            } else {
                obj.selection_radius.max(20.0) * 2.0
            };
            obj.guard_radius = (cur + delta).clamp(30.0, 400.0);
        }
        Some(obj.guard_radius)
    }
}
