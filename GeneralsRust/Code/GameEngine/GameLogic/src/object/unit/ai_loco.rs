//! AIUpdateInterface locomotor, path request, mood, and command helpers.

#![allow(unused_imports)]

use super::ai_core::UnitAIUpdate;
use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{
    dual_world_registry_unavailable, get_unit_arc, with_unit_mut, with_unit_ref,
};
use super::types::*;

/// Bound services used only while one installed AI follows its owned AiPath.
pub(super) struct NativePathMovementContext {
    pathfinder: Arc<RwLock<crate::ai::Pathfinder>>,
    surfaces: crate::locomotor::LocomotorSurfaceTypeMask,
    ignore_obstacle_id: Option<ObjectID>,
    layer: crate::ai::pathfind::PathfindLayerEnum,
}

impl NativePathMovementContext {
    pub(super) fn new(
        pathfinder: Arc<RwLock<crate::ai::Pathfinder>>,
        surfaces: crate::locomotor::LocomotorSurfaceTypeMask,
        ignore_obstacle_id: Option<ObjectID>,
        layer: crate::ai::pathfind::PathfindLayerEnum,
    ) -> Self {
        Self {
            pathfinder,
            surfaces,
            ignore_obstacle_id,
            layer,
        }
    }
}

impl crate::ai::pathfind::PathMovementContext for NativePathMovementContext {
    fn object_layer(&mut self) -> crate::ai::pathfind::PathfindLayerEnum {
        self.layer
    }

    fn is_line_passable(
        &mut self,
        layer: crate::ai::pathfind::PathfindLayerEnum,
        from: &Coord3D,
        to: &Coord3D,
    ) -> bool {
        self.pathfinder.read().ok().is_some_and(|pathfinder| {
            pathfinder.is_line_passable_for_surfaces_on_layer(
                from,
                to,
                self.surfaces,
                crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32),
                self.ignore_obstacle_id,
            )
        })
    }

    fn set_debug_path_position(&mut self, position: &Coord3D) {
        if let Ok(mut pathfinder) = self.pathfinder.write() {
            pathfinder.set_debug_path_position(*position);
        }
    }
}

impl UnitAIUpdate {
    pub(super) fn get_preferred_height(&self) -> Option<Real> {
        self.runtime.get_preferred_height()
    }
    pub(super) fn is_allowed_to_adjust_destination(&self) -> bool {
        self.runtime.is_allowed_to_adjust_destination()
    }
    pub(super) fn get_ai_free_to_exit(
        &self,
        exiter: &Object,
    ) -> crate::object::production::AIFreeToExitType {
        if let Some(chinook_ai) = self.runtime.components.chinook_ai.as_ref() {
            chinook_ai.get_ai_free_to_exit(exiter)
        } else {
            crate::object::production::AIFreeToExitType::FreeToExit
        }
    }
    pub(super) fn set_path_extra_distance(
        &mut self,
        distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.set_path_extra_distance(distance)
    }
    pub(super) fn set_path_from_waypoint(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        group_offset: &Coord2D,
    ) -> Result<(), String> {
        self.runtime.set_path_from_waypoint(waypoint, group_offset)
    }
    pub(super) fn is_waypoint_queue_empty(&self) -> bool {
        if self.runtime.owner.is_some() {
            // C++ AIUpdate owns m_waypointQueue and m_waypointCount. A native
            // tick must not consult another Unit's queue with the same ID.
            return self.runtime.data.planning_waypoint_count == 0;
        }
        if let Some(unit) = get_unit_arc(self.runtime.unit_id) {
            if let Ok(guard) = unit.read() {
                return guard.waypoint_queue.is_empty();
            }
        }
        true
    }
    pub(super) fn do_pathfind(&mut self) {
        // C++ AIUpdateInterface::doPathfind — process queued path request.
        if let Err(e) = self.do_queued_pathfind_now() {
            log::trace!("UnitAIUpdate::do_pathfind: {e}");
        }
    }
    pub(super) fn do_pathfind_with_pathfinder(
        &mut self,
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
    ) {
        if let Err(e) = self.do_queued_pathfind_with_pathfinder(pathfinder) {
            log::trace!("UnitAIUpdate::do_pathfind_with_pathfinder: {e}");
        }
    }
    pub(super) fn is_waiting_for_path(&self) -> bool {
        self.runtime.is_waiting_for_path()
    }
    pub(super) fn queue_waypoint(&mut self, pos: &Coord3D) {
        if (self.runtime.data.planning_waypoint_count as usize) < AI_UPDATE_MAX_WAYPOINTS {
            self.runtime.data.planning_waypoint_queue
                [self.runtime.data.planning_waypoint_count as usize] = *pos;
            self.runtime.data.planning_waypoint_count += 1;
            if let Some(unit) = get_unit_arc(self.runtime.unit_id) {
                if let Ok(mut guard) = unit.write() {
                    guard
                        .waypoint_queue
                        .push(Waypoint::new(0, *pos, String::new()));
                }
            }
        }
    }
    pub(super) fn clear_waypoint_queue(&mut self) {
        self.runtime.data.planning_waypoint_count = 0;
        self.runtime.data.planning_waypoint_index = 0;
        self.runtime.data.executing_waypoint_queue = false;
        if let Some(unit) = get_unit_arc(self.runtime.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.waypoint_queue.clear();
            }
        }
    }
    pub(super) fn execute_waypoint_queue(&mut self) {
        if self.runtime.data.planning_waypoint_count > 0 {
            self.runtime.data.planning_waypoint_index = 0;
            self.runtime.data.executing_waypoint_queue = true;
        }
        let first_pos = {
            let unit = match get_unit_arc(self.runtime.unit_id) {
                Some(u) => u,
                None => return,
            };
            let mut guard = match unit.write() {
                Ok(g) => g,
                Err(_) => return,
            };
            if guard.waypoint_queue.is_empty() {
                return;
            }
            let first = guard.waypoint_queue.remove(0);
            first.position
        };
        if let Err(e) = self.ai_move_to_position(&first_pos) {
            log::warn!("execute_waypoint_queue failed: {}", e);
        }
    }
    pub(super) fn append_goal_position_to_path(&mut self, goal: &Coord3D) -> Result<(), String> {
        self.runtime.append_goal_position_to_path(goal)
    }
    pub(super) fn set_path_from_coords(&mut self, path: &[Coord3D]) -> Result<(), String> {
        self.runtime.set_path_from_coords(path)
    }
    pub(super) fn request_safe_path(&mut self, repulsor_id: ObjectID) -> Result<bool, String> {
        self.runtime.request_safe_path(repulsor_id)
    }
    pub(super) fn is_doing_ground_movement(&self) -> bool {
        self.runtime.is_doing_ground_movement()
    }
    pub(super) fn is_allowed_to_move_away_from_unit(&self) -> bool {
        self.runtime
            .components
            .jet_ai
            .as_ref()
            .map(|jet| jet.is_allowed_to_move_away_from_unit())
            .unwrap_or(true)
    }
    pub(super) fn get_sneaky_targeting_offset(&self, offset: &mut Coord3D) -> bool {
        self.runtime
            .components
            .jet_ai
            .as_ref()
            .map(|jet| jet.get_sneaky_targeting_offset(offset))
            .unwrap_or(false)
    }
    pub(super) fn is_temporarily_preventing_aim_success(&self) -> bool {
        self.runtime.is_temporarily_preventing_aim_success()
    }
    pub(super) fn add_targeter(&mut self, id: ObjectID, add: bool) {
        self.runtime.add_targeter(id, add)
    }
    pub(super) fn are_turrets_linked(&self) -> Bool {
        self.runtime.data.are_turrets_linked()
    }
    pub(super) fn set_turret_target_object(
        &mut self,
        turret: TurretType,
        target_id: Option<ObjectID>,
        force_attacking: bool,
    ) {
        self.runtime
            .set_turret_target_object(turret, target_id, force_attacking)
    }
    pub(super) fn set_turret_target_position(&mut self, turret: TurretType, pos: &Coord3D) {
        if let Some(machine) = self.ensure_turret_machine(turret) {
            machine.turret_mut().set_target_position(Some(*pos));
        }
    }
    pub(super) fn is_out_of_special_reload_ammo(&self) -> bool {
        self.runtime
            .components
            .jet_ai
            .as_ref()
            .map(|jet| jet.is_out_of_special_reload_ammo())
            .unwrap_or(false)
    }
    pub(super) fn get_treat_as_aircraft_for_loco_dist_to_goal(&self) -> bool {
        if let Some(jet_ai) = self.runtime.components.jet_ai.as_ref() {
            return jet_ai.get_treat_as_aircraft_for_loco_dist_to_goal();
        }
        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return true;
        };
        let Ok(guard) = unit.read() else {
            return true;
        };

        let mut treat_as_aircraft = !self.is_doing_ground_movement();
        if guard.path_extra_distance > PATHFIND_CLOSE_ENOUGH {
            treat_as_aircraft = true;
        }
        if let Some(locomotor) = self.runtime.data.locomotor_set.get_active() {
            if locomotor.get_appearance() == LocomotorAppearance::Hover {
                treat_as_aircraft = true;
            }
        }
        treat_as_aircraft
    }
    pub(super) fn update_goal_position(
        &mut self,
        goal: &Coord3D,
        layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String> {
        self.runtime.update_goal_position(goal, layer)
    }
    pub(super) fn adjust_destination(&mut self, goal: &mut Coord3D) -> bool {
        self.runtime.adjust_destination(goal)
    }
    pub(super) fn set_adjusts_destination(&mut self, adjust: bool) {
        self.runtime.set_adjusts_destination(adjust)
    }
    pub(super) fn set_allow_invalid_position(
        &mut self,
        allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.set_allow_invalid_position(allow)
    }
    pub(super) fn set_allow_chase(&mut self, allowed: bool) {
        self.runtime.data.set_allow_chase(allowed)
    }
    pub(super) fn set_locomotor_upgrade(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.data.locomotor_upgraded = enabled;
        if matches!(
            self.runtime.data.current_locomotor_set,
            LocomotorSetType::Normal | LocomotorSetType::NormalUpgraded
        ) {
            let _ = self.choose_locomotor_set(LocomotorSetType::Normal);
        }
        Ok(())
    }
    pub(super) fn choose_locomotor_set(
        &mut self,
        set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.choose_locomotor_set(set)
    }
    pub(super) fn set_ultra_accurate(
        &mut self,
        ultra: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.data.set_ultra_accurate(ultra)
    }
    pub(super) fn set_precise_z_pos(
        &mut self,
        precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.set_precise_z_pos(precise)
    }
    pub(super) fn with_cur_locomotor(&self, f: &mut dyn FnMut(&crate::locomotor::Locomotor)) {
        self.runtime.with_cur_locomotor(f)
    }
    pub(super) fn with_cur_locomotor_mut(
        &mut self,
        f: &mut dyn FnMut(&mut crate::locomotor::Locomotor),
    ) {
        self.runtime.with_cur_locomotor_mut(f)
    }
    pub(super) fn get_locomotor_set_clone(&self) -> Option<crate::locomotor::LocomotorSet> {
        self.runtime.data.get_locomotor_set_clone()
    }
    pub(super) fn get_path_destination(&self) -> Option<Coord3D> {
        self.runtime.get_path_destination()
    }
    pub(super) fn get_path_last_node(&self) -> Option<Coord3D> {
        self.runtime.get_path_last_node()
    }
    pub(super) fn has_nonempty_path(&self) -> bool {
        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        guard
            .current_path
            .as_ref()
            .is_some_and(|path| !path.is_empty())
    }
    pub(super) fn peek_cached_point_on_path(&self) -> Option<Coord3D> {
        let unit = get_unit_arc(self.runtime.unit_id)?;
        let guard = unit.read().ok()?;
        let pos = guard.get_position();
        if let Some(path) = guard.current_path.as_ref() {
            if !path.is_empty() {
                let waypoints: Vec<Coord3D> =
                    path.iter().map(|p| Coord3D::new(p.x, p.y, pos.z)).collect();
                return Some(
                    crate::ai::pathfind_complete::peek_point_on_path_from_waypoints(
                        &pos, &waypoints,
                    ),
                );
            }
        }
        self.get_path_destination()
    }

    pub(super) fn get_locomotor_distance_to_goal(&mut self) -> Real {
        if self.runtime.owner.is_some() {
            let fallback_goal = self
                .ai_state_machine
                .as_ref()
                .and_then(|machine| machine.get_goal_position());
            self.runtime.get_locomotor_distance_to_goal(fallback_goal)
        } else {
            self.runtime.get_legacy_locomotor_distance_to_goal()
        }
    }
    pub(super) fn get_speed(&self) -> f32 {
        self.runtime.get_speed()
    }
    pub(super) fn get_last_command_source(&self) -> CommandSourceType {
        self.runtime.get_last_command_source()
    }
    pub(super) fn set_last_command_source(&mut self, source: CommandSourceType) {
        self.runtime.set_last_command_source(source)
    }
    pub(super) fn get_current_command(&self) -> Option<crate::ai::AiCommandType> {
        self.runtime.data.get_current_command()
    }
    pub(super) fn get_pending_command_type(&self) -> Option<crate::ai::AiCommandType> {
        if let Some(jet_ai) = self.runtime.components.jet_ai.as_ref() {
            if let Some(cmd) = jet_ai.pending_command_type() {
                return Some(cmd);
            }
        }
        self.runtime.data.pending_command
    }
    pub(super) fn purge_pending_command(&mut self) {
        if let Some(jet_ai) = self.runtime.components.jet_ai.as_mut() {
            jet_ai.set_has_pending_command(false);
        }
        self.runtime.data.pending_command = None;
    }
    pub(super) fn is_taxiing_to_parking(&self) -> bool {
        self.runtime
            .components
            .jet_ai
            .as_ref()
            .map(|jet| jet.is_taxiing_to_parking())
            .unwrap_or(false)
    }
    pub(super) fn is_reloading(&self) -> bool {
        self.runtime
            .components
            .jet_ai
            .as_ref()
            .map(|jet| jet.is_reloading())
            .unwrap_or(false)
    }
    pub(super) fn is_clearing_mines(&self) -> bool {
        if let Some(owner) = self.runtime.owner.as_ref() {
            // C++ getObject() identifies this AI's owner, even beside another
            // world's same-ID Unit. An expired native owner never falls back.
            let Some(obj) = owner.upgrade() else {
                return false;
            };
            let Ok(obj_guard) = obj.read() else {
                return false;
            };
            return Self::object_is_clearing_mines(&obj_guard);
        }
        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let obj = guard.base_arc();
        let Ok(obj_guard) = obj.read() else {
            return false;
        };
        Self::object_is_clearing_mines(&obj_guard)
    }

    fn object_is_clearing_mines(obj: &Object) -> bool {
        if !obj.test_status(ObjectStatusTypes::OBJECT_STATUS_IS_ATTACKING) {
            return false;
        }
        let Some((weapon, _slot)) = obj.get_current_weapon() else {
            return false;
        };
        (weapon.get_anti_mask() & WeaponAntiMask::MINE) != 0
    }
    pub(super) fn is_takeoff_or_landing_in_progress(&self) -> bool {
        self.runtime
            .components
            .jet_ai
            .as_ref()
            .map(|jet| jet.is_takeoff_or_landing_in_progress())
            .unwrap_or(false)
    }
    pub(super) fn get_current_state_id(&self) -> Option<u32> {
        self.ai_state_machine
            .as_ref()
            .and_then(AIStateMachine::get_current_state_id)
    }
    pub(super) fn get_parking_offset(&self) -> Real {
        self.runtime
            .components
            .jet_ai
            .as_ref()
            .map(|jet| jet.parking_offset())
            .unwrap_or(0.0)
    }
    pub(super) fn keeps_parking_space_when_airborne(&self) -> bool {
        self.runtime
            .components
            .jet_ai
            .as_ref()
            .map(|jet| jet.keeps_parking_space_when_airborne())
            .unwrap_or(true)
    }
    pub(super) fn get_desired_speed(&self) -> Real {
        self.runtime.get_desired_speed()
    }
    pub(super) fn set_desired_speed(&mut self, speed: Real) {
        self.runtime.set_desired_speed(speed)
    }
    pub(super) fn is_in_rappel_state(&self) -> bool {
        self.runtime.is_in_rappel_state()
    }
    pub(super) fn is_doing_combat_drop(&self) -> bool {
        self.runtime.is_doing_combat_drop()
    }
    pub(super) fn is_aircraft_that_adjusts_destination(&self) -> bool {
        self.runtime.data.is_aircraft_that_adjusts_destination()
    }
    pub(super) fn is_moving_away_from(&self, obj_id: ObjectID) -> bool {
        self.ai_state_machine.as_ref().is_some_and(|machine| {
            machine.get_temporary_state() == Some(AIStateType::MoveOutOfTheWay as u32)
        }) && (self.runtime.data.move_out_of_way_1 == obj_id
            || self.runtime.data.move_out_of_way_2 == obj_id)
    }
    pub(super) fn set_ignore_collision_time(&mut self, duration_frames: UnsignedInt) {
        self.runtime.set_ignore_collision_time(duration_frames)
    }
    pub(super) fn get_ignore_collisions_until(&self) -> UnsignedInt {
        self.runtime.data.get_ignore_collisions_until()
    }
    pub(super) fn set_queue_for_path_time(&mut self, frames: UnsignedInt) {
        self.runtime.set_queue_for_path_time(frames)
    }
    pub(super) fn ignore_obstacle(
        &mut self,
        obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.ignore_obstacle(obj_id)
    }
    pub(super) fn ignore_obstacle_id(
        &mut self,
        id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.ignore_obstacle_id(id)
    }
    pub(super) fn get_ignored_obstacle_id(&self) -> ObjectID {
        self.runtime.data.get_ignored_obstacle_id()
    }
    pub(super) fn is_ai_in_dead_state(&self) -> bool {
        self.runtime.data.is_ai_in_dead_state()
    }
    pub(super) fn mark_as_dead(&mut self) {
        self.runtime.data.ai_dead = true;
        if let Some(unit) = get_unit_arc(self.runtime.unit_id) {
            if let Ok(unit_guard) = unit.read() {
                if let Ok(mut object_guard) = unit_guard.base_arc().write() {
                    object_guard.set_effectively_dead(true);
                }
            }
        }
        self.wake_up_now();
    }
    pub(super) fn set_is_recruitable(&mut self, recruitable: Bool) {
        self.runtime.data.set_is_recruitable(recruitable)
    }
    pub(super) fn get_goal_object_id(&self) -> ObjectID {
        self.ai_state_machine
            .as_ref()
            .map(AIStateMachine::get_goal_object_id)
            .unwrap_or(INVALID_ID)
    }
    pub(super) fn set_goal_object(&mut self, obj_id: Option<ObjectID>) {
        let Some(machine) = self.ai_state_machine.as_mut() else {
            return;
        };
        let was_locked = machine.is_locked();
        machine.unlock();
        machine.set_goal_object(obj_id.unwrap_or(INVALID_ID));
        if was_locked {
            machine.lock();
        }
    }
    pub(super) fn get_goal_position(&self) -> Option<Coord3D> {
        self.ai_state_machine
            .as_ref()
            .and_then(AIStateMachine::get_goal_position)
    }
    pub(super) fn get_current_victim_pos(&self) -> Option<Coord3D> {
        if self
            .get_current_victim()
            .is_some_and(|id| id != crate::common::INVALID_ID)
        {
            return None;
        }
        let unit = get_unit_arc(self.runtime.unit_id)?;
        let unit_guard = unit.read().ok()?;
        let attacking = unit_guard
            .base_arc()
            .read()
            .ok()
            .is_some_and(|obj| obj.test_status(crate::common::ObjectStatusTypes::IsAttacking));
        if !attacking {
            return None;
        }
        self.get_goal_position()
    }

    pub(super) fn set_goal_position(&mut self, pos: Option<Coord3D>) {
        if let (Some(pos), Some(machine)) = (pos, self.ai_state_machine.as_mut()) {
            machine.set_goal_position(pos);
        }
    }
    /// C++ `AIUpdateInterface::joinTeam` (AIUpdate.cpp).
    ///
    /// After `clear()`, C++ `getCurrentStateID()` is `INVALID_STATE_ID` (NULL
    /// current state). `setState(INVALID)` then falls through to the default
    /// state. Port that literally — C++ does not read the teammate's state id.
    pub(super) fn join_team(&mut self) {
        // Wave 258: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            panic!("dual-world registry unavailable in test helper");
        }

        if self.is_ai_in_dead_state() {
            return;
        }
        let Some(unit_arc) = get_unit_arc(self.runtime.unit_id) else {
            return;
        };
        let (mobile, self_id, team) = {
            let Ok(g) = unit_arc.read() else {
                return;
            };
            let self_id = g.get_id();
            let base = g.base_arc();
            let Ok(obj) = base.read() else {
                return;
            };
            (obj.is_mobile(), self_id, obj.get_team())
        };
        if !mobile {
            return;
        }

        let _ = self.choose_locomotor_set(LocomotorSetType::Normal);

        // getStateMachine()->clear(); setGoalWaypoint(NULL);
        if let Some(machine) = self.ai_state_machine.as_mut() {
            let mut runtime = super::UnitAiStateRuntime::new(&mut self.runtime, true);
            machine.driver().clear_with_ai(&mut runtime);
            machine.set_goal_waypoint(None);
        }

        let mut other_pos = None;
        let mut other_idle = false;
        let mut other_goal_id = INVALID_ID;
        let mut other_goal_pos: Option<Coord3D> = None;
        let mut found_other = false;

        if let Some(team_arc) = team {
            let members = team_arc
                .read()
                .ok()
                .map(|tg| tg.get_members().to_vec())
                .unwrap_or_default();
            for mid in members {
                if mid == self_id {
                    continue;
                }
                let Some((pos, ai)) = crate::object::registry::OBJECT_REGISTRY
                    .with_object(mid, |og| {
                        let Some(oai) = og.get_ai_update_interface() else {
                            return None;
                        };
                        if og.is_disabled_by_type(crate::common::types::DisabledType::Held) {
                            return None;
                        }
                        Some((*og.get_position(), oai))
                    })
                    .flatten()
                else {
                    continue;
                };
                other_pos = Some(pos);
                if let Ok(aig) = ai.try_lock() {
                    other_idle = aig.is_idle();
                    other_goal_id = aig.get_goal_object_id();
                    other_goal_pos = aig.get_goal_position();
                }
                found_other = true;
                break;
            }
        }

        if !found_other {
            return;
        }
        let Some(pos) = other_pos else {
            return;
        };

        if other_idle {
            self.runtime.data.last_command_source = CommandSourceType::FromAi;
            let _ = self.ai_move_to_position(&pos);
            return;
        }

        if let Some(machine) = self.ai_state_machine.as_mut() {
            if other_goal_id != INVALID_ID {
                machine.set_goal_object(other_goal_id);
            } else if let Some(gp) = other_goal_pos {
                machine.set_goal_position(gp);
            }
        }

        // C++ after clear: getCurrentStateID() == INVALID_STATE_ID → default state.
        self.runtime.data.last_command_source = CommandSourceType::FromAi;
        if let Some(machine) = self.ai_state_machine.as_mut() {
            let mut runtime = super::UnitAiStateRuntime::new(&mut self.runtime, true);
            let _ = machine
                .driver()
                .set_state_with_ai(crate::state_machine::INVALID_STATE_ID, &mut runtime);
        }
    }
    pub(super) fn is_path_available(&self, destination: &Coord3D) -> bool {
        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let ai_store = the_ai();
        let Some(ai) = ai_store.read().ok() else {
            return false;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return false;
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return false;
        };
        let pos = guard.get_position();
        let ignore = if self.runtime.data.ignore_obstacle_id == INVALID_ID {
            None
        } else {
            Some(self.runtime.data.ignore_obstacle_id)
        };
        pf_guard.client_safe_quick_does_path_exist_with_ignore(
            &self.runtime.data.locomotor_set,
            &pos,
            destination,
            ignore,
        )
    }
    pub(super) fn request_path(
        &mut self,
        destination: &Coord3D,
        _is_final_goal: bool,
    ) -> Result<(), String> {
        self.runtime.request_path(destination, _is_final_goal)
    }
    pub(super) fn request_attack_path(
        &mut self,
        victim_id: ObjectID,
        victim_pos: &Coord3D,
    ) -> Result<(), String> {
        // Wave 258: empty dual-world → Ok(()).

        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !self.has_valid_locomotor_surfaces() {
            return Err("Attempting to path immobile unit".to_string());
        }
        self.runtime.data.requested_destination = *victim_pos;
        self.runtime.data.requested_victim_id = victim_id;
        self.runtime.data.is_attack_path = true;
        self.runtime.data.is_approach_path = false;
        self.runtime.data.is_safe_path = false;
        self.runtime.data.waiting_for_path = true;
        let victim = get_legacy_object(victim_id);
        let _ = self.set_goal_object(
            victim
                .as_ref()
                .and_then(|a| a.read().ok().map(|g| g.get_id())),
        );
        let _ = self.ignore_obstacle(
            victim
                .as_ref()
                .and_then(|a| a.read().ok().map(|g| g.get_id())),
        );
        let now = TheGameLogic::get_frame();
        if self.runtime.data.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            self.set_locomotor_goal_none();
            return Ok(());
        }
        self.set_queue_for_path_time(0);
        let _ = self.queue_path_request_now(*victim_pos);
        self.runtime.data.path_timestamp = now;
        Ok(())
    }
    pub(super) fn request_approach_path(&mut self, destination: &Coord3D) -> Result<(), String> {
        if !self.has_valid_locomotor_surfaces() {
            return Err("Attempting to path immobile unit".to_string());
        }
        self.runtime.data.requested_destination = *destination;
        self.runtime.data.is_final_goal = true;
        self.runtime.data.is_attack_path = false;
        self.runtime.data.requested_victim_id = INVALID_ID;
        self.runtime.data.is_approach_path = true;
        self.runtime.data.is_safe_path = false;
        self.runtime.data.waiting_for_path = true;
        let now = TheGameLogic::get_frame();
        if self.runtime.data.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            return Ok(());
        }
        self.set_queue_for_path_time(0);
        let _ = self.queue_path_request_now(*destination);
        self.runtime.data.path_timestamp = now;
        Ok(())
    }
    pub(super) fn can_compute_quick_path(&self) -> bool {
        self.runtime.can_compute_quick_path()
    }
    pub(super) fn compute_quick_path(&mut self, destination: &Coord3D) -> bool {
        self.runtime.compute_quick_path(destination)
    }
    pub(super) fn is_quick_path_available(&self, destination: &Coord3D) -> bool {
        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let ai_store = the_ai();
        let Some(ai) = ai_store.read().ok() else {
            return false;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return false;
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return false;
        };
        let pos = guard.get_position();
        pf_guard.client_safe_quick_does_path_exist_for_ui(
            &self.runtime.data.locomotor_set,
            &pos,
            destination,
        )
    }
    pub(super) fn is_valid_locomotor_position(&self, pos: &Coord3D) -> bool {
        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let ai_store = the_ai();
        let Some(ai) = ai_store.read().ok() else {
            return false;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return false;
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return false;
        };
        pf_guard.valid_movement_position(
            &self.runtime.data.locomotor_set,
            guard.get_crusher_level() > 0,
            pos,
            if self.runtime.data.ignore_obstacle_id == INVALID_ID {
                None
            } else {
                Some(self.runtime.data.ignore_obstacle_id)
            },
        )
    }
    pub(super) fn need_to_rotate(&self) -> bool {
        if self.is_waiting_for_path() {
            return true;
        }
        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let Some(loc_guard) = self.runtime.data.locomotor_set.get_active() else {
            return false;
        };
        if loc_guard.template.wander_width_factor > 0.0 {
            return false;
        }
        let Some(active_path) = loc_guard.active_path.as_ref() else {
            return false;
        };
        let Some(target) = active_path.current_target() else {
            return false;
        };
        let pos = guard.get_position();
        let mut path_point = target;
        if active_path.current_waypoint + 1 < active_path.waypoints.len() {
            let start = active_path.waypoints[active_path.current_waypoint];
            let end = active_path.waypoints[active_path.current_waypoint + 1];
            let seg = Coord3D::new(end.x - start.x, end.y - start.y, 0.0);
            let seg_len_sqr = seg.x * seg.x + seg.y * seg.y;
            if seg_len_sqr > f32::EPSILON {
                let to_pos = Coord3D::new(pos.x - start.x, pos.y - start.y, 0.0);
                let mut t = (to_pos.x * seg.x + to_pos.y * seg.y) / seg_len_sqr;
                if t < 0.0 {
                    t = 0.0;
                } else if t > 1.0 {
                    t = 1.0;
                }
                path_point = Coord3D::new(start.x + seg.x * t, start.y + seg.y * t, pos.z);
            }
        }
        let delta = path_point - pos;
        if delta.length_squared() < f32::EPSILON {
            return false;
        }
        let desired_angle = delta.y.atan2(delta.x);
        let current_angle = guard.get_orientation();
        let mut delta_angle = desired_angle - current_angle;
        while delta_angle > std::f32::consts::PI {
            delta_angle -= std::f32::consts::PI * 2.0;
        }
        while delta_angle < -std::f32::consts::PI {
            delta_angle += std::f32::consts::PI * 2.0;
        }
        delta_angle.abs() > (std::f32::consts::PI / 30.0)
    }
    pub(super) fn get_cur_locomotor_set_type(&self) -> LocomotorSetType {
        self.runtime.data.get_cur_locomotor_set_type()
    }
    pub(super) fn has_locomotor_for_surface(
        &self,
        surface: crate::common::LocomotorSurfaceTypeMask,
    ) -> bool {
        let Some(entries) = self
            .runtime
            .data
            .locomotor_sets
            .get(&self.runtime.data.current_locomotor_set)
        else {
            return false;
        };
        for name in entries {
            if let Some(template) = crate::locomotor::LOCOMOTOR_STORE.get_template(name.as_str()) {
                if (template.surfaces & surface) != 0 {
                    return true;
                }
            }
        }
        false
    }
    pub(super) fn get_cur_locomotor_speed(&self) -> Real {
        let Some(owner) = self.runtime.native_owner() else {
            return 0.0;
        };
        let Some(loc_guard) = self.runtime.data.locomotor_set.get_active() else {
            return 0.0;
        };
        let body_state = owner
            .read()
            .ok()
            .and_then(|obj| obj.get_body_module())
            .and_then(|body| {
                body.lock()
                    .ok()
                    .map(|b| to_locomotor_body_damage_type(b.get_damage_state()))
            })
            .unwrap_or(BodyDamageType::Pristine);
        loc_guard.get_max_speed_for_condition(body_state)
    }
    pub(super) fn get_cur_max_blocked_speed(&self) -> Real {
        self.runtime.data.get_cur_max_blocked_speed()
    }
    pub(super) fn set_cur_max_blocked_speed(&mut self, speed: Real) {
        self.runtime.data.set_cur_max_blocked_speed(speed)
    }
    pub(super) fn set_locomotor_goal_none(&mut self) {
        self.runtime.set_locomotor_goal_none()
    }
    pub(super) fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        self.runtime.set_locomotor_goal_orientation(angle)
    }
    pub(super) fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        self.runtime.set_locomotor_goal_position_explicit(pos)
    }
    /// Native C++ `AIUpdateInterface::doLocomotor` POSITION_ON_PATH consumer
    /// (AIUpdate.cpp:2107-2187). It follows only the construction-bound owner;
    /// the standalone Unit projection is not consulted for native path state.
    fn apply_native_position_on_path(&mut self) {
        let Some(owner) = self.runtime.owner.as_ref().and_then(Weak::upgrade) else {
            return;
        };
        let Some(active) = self.runtime.data.locomotor_set.get_active() else {
            return;
        };
        let surfaces = active.get_legal_surfaces();
        let valid_surfaces = self.runtime.data.locomotor_set.get_valid_surfaces();
        let ignore_obstacle_id = (self.runtime.data.ignore_obstacle_id != INVALID_ID)
            .then_some(self.runtime.data.ignore_obstacle_id);
        let airborne_height = active.template.airborne_targeting_height as Real;
        let is_dead = self.runtime.data.is_ai_in_dead_state();

        // C++ applies setPhysicsOptions before checking the dead-state gate or
        // querying the path. Release the Object before Pathfinder callbacks.
        let (current, angle, body, physics, layer, is_ground_movement) = {
            let Ok(mut object) = owner.write() else {
                return;
            };
            let Some(loco) = self.runtime.data.locomotor_set.get_active_mut() else {
                return;
            };
            let body = object
                .get_body_module()
                .and_then(|body| {
                    body.lock()
                        .ok()
                        .map(|body| to_locomotor_body_damage_type(body.get_damage_state()))
                })
                .unwrap_or(crate::locomotor::BodyDamageType::Pristine);
            let physics = object.get_physics();
            if let Some(physics) = physics.as_ref() {
                if let Ok(mut physics) = physics.access() {
                    loco.apply_physics_options(&mut *physics);
                }
            }
            if is_dead && !loco.template.locomotor_works_when_dead {
                return;
            }
            let ground = if let Some(jet) = self.runtime.components.jet_ai.as_ref() {
                jet.is_doing_ground_movement()
            } else if object.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned)
                && object.is_kind_of(crate::common::KindOf::ProducedAtHelipad)
            {
                true
            } else if valid_surfaces == crate::ai::pathfind_complete::SURFACE_AIR
                || (loco.get_legal_surfaces() & crate::ai::pathfind_complete::SURFACE_AIR) != 0
                || object.is_disabled_by_type(crate::common::DisabledType::Held)
            {
                false
            } else if object.is_above_terrain()
                && physics
                    .as_ref()
                    .and_then(|physics| physics.access().ok())
                    .is_some_and(|physics| physics.get_allow_to_fall())
            {
                false
            } else {
                true
            };
            (
                *object.get_position(),
                object.get_orientation(),
                body,
                physics,
                crate::ai::pathfind::PathfindLayerEnum::from_u32(object.get_layer() as u32),
                ground,
            )
        };

        // Read the selected world service once, after C++'s physics/dead-state
        // gates. Keep this same Arc for path checks and final layer adjustment.
        let pathfinder = if is_ground_movement {
            let pathfinder = the_ai().read().ok().and_then(|ai| ai.pathfinder());
            let Some(pathfinder) = pathfinder else {
                return;
            };
            Some(pathfinder)
        } else {
            None
        };
        let (goal, path_distance, path_layer) = {
            let Some(path) = self.runtime.data.current_path_snapshot.as_mut() else {
                if !self.runtime.data.waiting_for_path {
                    log::error!(
                        "native POSITION_ON_PATH has no path and is not waiting for a result"
                    );
                }
                return;
            };
            if is_ground_movement {
                let mut context = NativePathMovementContext {
                    pathfinder: pathfinder
                        .as_ref()
                        .expect("ground movement has pathfinder")
                        .clone(),
                    surfaces,
                    ignore_obstacle_id,
                    layer,
                };
                let info = path.compute_point_on_path(&current, &mut context);
                (info.pos_on_path, info.dist_along_path, Some(info.layer))
            } else {
                let (distance, goal) = path.compute_flight_dist_to_goal(&current);
                (goal, distance, None)
            }
        };

        if let Some(path_layer) = path_layer {
            let Some(pathfinder) = pathfinder.as_ref() else {
                return;
            };
            let bridge_layer = crate::common::PathfindLayerEnum::from_u32(path_layer as u32);
            let pathfinder_layer =
                crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(path_layer as u32);
            let interacts = owner
                .read()
                .ok()
                .and_then(|object| {
                    (bridge_layer != crate::common::PathfindLayerEnum::Ground).then(|| {
                        TheTerrainLogic::get()
                            .map(|terrain| {
                                terrain.object_interacts_with_bridge_layer(
                                    &object,
                                    bridge_layer,
                                    true,
                                )
                            })
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            let Ok(pathfinder) = pathfinder.read() else {
                return;
            };
            let adjusted_layer = pathfinder.update_layer_for_object(pathfinder_layer, interacts);
            if let Ok(mut object) = owner.write() {
                object.set_layer(crate::common::PathfindLayerEnum::from_u32(
                    adjusted_layer as u32,
                ));
            }
        }

        let Ok(mut object) = owner.write() else {
            return;
        };
        let Some(max_speed) = self
            .runtime
            .data
            .locomotor_set
            .get_active()
            .map(|loco| loco.get_max_speed_for_condition(body))
        else {
            return;
        };
        let mut speed = self.runtime.data.desired_speed;
        if speed == crate::modules::FAST_AS_POSSIBLE || speed > max_speed {
            speed = max_speed;
        }
        let blocked = self.runtime.data.blocked_frames > 0;
        let (speed, blocked) = self
            .runtime
            .data
            .apply_bump_speed_limit_with_blocked(speed, blocked);
        let Some(loco) = self.runtime.data.locomotor_set.get_active_mut() else {
            return;
        };
        let forward_speed = physics
            .as_ref()
            .and_then(|physics| {
                physics
                    .access()
                    .ok()
                    .map(|physics| physics.get_forward_speed_2d_with_object(&object))
            })
            .unwrap_or(0.0);
        let mut physics_write = physics.as_ref().and_then(|physics| physics.access().ok());
        if let Some(physics) = physics_write.as_deref_mut() {
            loco.apply_physics_options(physics);
        }
        // C++ locomotor speed, acceleration and turn rate are per logic frame.
        let delta = 1.0;
        let (new_pos, new_angle, _new_speed) = loco.loco_update_move_towards_position(
            current,
            angle,
            forward_speed,
            goal,
            path_distance + self.runtime.data.path_extra_distance,
            speed,
            body,
            delta,
            blocked,
            physics_write.as_deref_mut(),
            Some(&mut *object),
        );
        object.set_orientation(new_angle);
        if let Some(physics) = physics_write.as_deref_mut() {
            let mut yaw_delta = new_angle - angle;
            let two_pi = std::f32::consts::PI * 2.0;
            while yaw_delta > std::f32::consts::PI {
                yaw_delta -= two_pi;
            }
            while yaw_delta < -std::f32::consts::PI {
                yaw_delta += two_pi;
            }
            physics.set_turning(if yaw_delta > 0.0 {
                1
            } else if yaw_delta < 0.0 {
                -1
            } else {
                0
            });
        }
        let airborne = object.get_height_above_terrain() > airborne_height;
        object.set_status(
            crate::common::ObjectStatusMaskType::from_status(
                crate::common::ObjectStatusTypes::AirborneTarget,
            ),
            airborne,
        );
        self.runtime.data.do_final_position = false;
        let _ = new_pos;
    }

    pub(super) fn apply_stored_locomotor_goal(&mut self) {
        if self.runtime.data.movement_complete {
            return;
        }
        if self.runtime.data.locomotor_goal_type == 1 {
            self.apply_native_position_on_path();
            return;
        }
        if self.runtime.data.locomotor_goal_type != 2 && self.runtime.data.locomotor_goal_type != 3
        {
            return;
        }
        let goal_type = self.runtime.data.locomotor_goal_type;
        let goal = self.runtime.data.locomotor_goal_data;
        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return;
        };
        let Ok(mut guard) = unit.write() else {
            return;
        };
        let base = guard.base_arc();
        let (current, angle, body, forward_speed, physics) = {
            let Ok(object) = base.read() else {
                return;
            };
            let body = object
                .get_body_module()
                .and_then(|body| {
                    body.lock()
                        .ok()
                        .map(|b| to_locomotor_body_damage_type(b.get_damage_state()))
                })
                .unwrap_or(crate::locomotor::BodyDamageType::Pristine);
            let physics = object.get_physics();
            let forward_speed = physics
                .as_ref()
                .and_then(|physics| physics.access().ok().map(|g| g.get_forward_speed_2d()))
                .unwrap_or(0.0);
            (
                *object.get_position(),
                object.get_orientation(),
                body,
                forward_speed,
                physics,
            )
        };
        let object_arc = guard.base_arc().clone();
        let max_speed = self
            .runtime
            .data
            .locomotor_set
            .get_active()
            .map(|loco| loco.get_max_speed_for_condition(body))
            .unwrap_or(0.0);
        let mut speed = self.runtime.data.desired_speed;
        if speed == crate::modules::FAST_AS_POSSIBLE || speed > max_speed {
            speed = max_speed;
        }
        speed = self.apply_bump_speed_limit(speed, self.runtime.data.blocked_frames > 0);
        let Some(loco) = self.runtime.data.locomotor_set.get_active_mut() else {
            return;
        };
        if let Some(physics) = physics.as_ref() {
            if let Ok(mut physics) = physics.access() {
                loco.apply_physics_options(&mut *physics);
            }
        }
        let delta = 1.0 / crate::common::LOGICFRAMES_PER_SECOND as Real;

        let airborne_height = loco.template.airborne_targeting_height;
        let mut object_write = object_arc.write().ok();
        let mut physics_write = physics.as_ref().and_then(|physics| physics.access().ok());
        let (new_pos, new_angle, _new_speed) = if goal_type == 2 {
            loco.loco_update_move_towards_position(
                current,
                angle,
                forward_speed,
                goal,
                0.0,
                speed,
                body,
                delta,
                self.runtime.data.blocked_frames > 0,
                physics_write.as_deref_mut(),
                object_write.as_deref_mut(),
            )
        } else {
            loco.loco_update_move_towards_angle(current, angle, goal.x, forward_speed, body, delta)
        };
        drop(object_write);
        drop(physics_write);
        drop(guard);
        if let Some(unit) = get_unit_arc(self.runtime.unit_id) {
            if let Ok(guard) = unit.read() {
                if let Ok(mut object) = guard.base_arc().write() {
                    if goal_type != 2 {
                        let _ = object.set_position(&new_pos);
                    }
                    let _ = object.set_orientation(new_angle);
                    if let Some(physics) = object.get_physics() {
                        if let Ok(mut physics) = physics.access() {
                            if goal_type != 2 {
                                let velocity = (new_pos - current) / delta;
                                physics.set_velocity(&velocity);
                            }
                            let mut yaw_delta = new_angle - angle;
                            let two_pi = std::f32::consts::PI * 2.0;
                            while yaw_delta > std::f32::consts::PI {
                                yaw_delta -= two_pi;
                            }
                            while yaw_delta < -std::f32::consts::PI {
                                yaw_delta += two_pi;
                            }
                            physics.set_yaw_rate(yaw_delta / delta);
                            physics.set_turning(if yaw_delta > 0.0 {
                                1
                            } else if yaw_delta < 0.0 {
                                -1
                            } else {
                                0
                            });
                        }
                    }
                    let airborne = object.get_height_above_terrain() > airborne_height as Real;
                    object.set_status(
                        crate::common::ObjectStatusMaskType::from_status(
                            crate::common::ObjectStatusTypes::AirborneTarget,
                        ),
                        airborne,
                    );
                }
            }
        }
    }
    pub(super) fn friend_ending_move(&mut self) {
        self.runtime.friend_ending_move()
    }
    pub(super) fn friend_starting_move(&mut self) {
        self.runtime.friend_starting_move()
    }
    pub(super) fn evaluate_morale_bonus(&mut self) {
        let Some(unit_arc) = get_unit_arc(self.runtime.unit_id) else {
            return;
        };
        let base_object = match unit_arc.read() {
            Ok(guard) => guard.base_arc(),
            Err(_) => return,
        };
        let Ok(mut obj_guard) = base_object.write() else {
            return;
        };

        let mut nationalism = false;
        let mut fanaticism = false;
        obj_guard.with_controlling_player(|player_guard| {
            if let Ok(center) = get_upgrade_center().read() {
                if let Some(upgrade) = center.find_upgrade("Upgrade_Nationalism") {
                    if player_guard.has_upgrade_complete(&upgrade) {
                        nationalism = true;
                    }
                }
                if let Some(upgrade) = center.find_upgrade("Upgrade_Fanaticism") {
                    if player_guard.has_upgrade_complete(&upgrade) {
                        fanaticism = true;
                    }
                }
            }
        });

        let mut horde = false;
        let mut allow_nationalism = true;
        obj_guard.with_horde_update_interface(|hui| {
            if hui.is_in_horde() {
                horde = true;
                if !hui.is_allowed_nationalism() {
                    allow_nationalism = false;
                }
            }
        });

        if !allow_nationalism {
            nationalism = false;
            fanaticism = false;
        }

        let demoralized = self.runtime.data.demoralized_frames_left > 0;

        if !demoralized {
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Demoralized);
        }

        if horde {
            obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Horde);
        } else {
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Horde);
        }

        if nationalism {
            obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Nationalism);
            if fanaticism {
                obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);
            } else {
                obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);
            }
        } else {
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Nationalism);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);
        }

        if demoralized {
            obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Demoralized);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Horde);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Nationalism);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);

            if !obj_guard.is_kind_of(KindOf::PortableStructure) {
                if let Some(drawable) = obj_guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        draw_guard.set_terrain_decal(TerrainDecalType::Demoralized);
                    }
                }
            }
        }
    }
    pub(super) fn set_surrendered(&mut self, to_object_id: Option<ObjectID>, surrendered: bool) {
        // Wave 258: empty dual-world → no factory object walks.

        if dual_world_registry_unavailable() {
            panic!("dual-world registry unavailable in test helper");
        }

        if surrendered {
            self.runtime.data.surrendered_frames_left = self.runtime.data.surrender_duration_frames;
            self.runtime.data.surrendered_player_index = to_object_id.and_then(|id| {
                let obj = crate::object::registry::OBJECT_REGISTRY.get_object(id)?;
                let guard = obj.read().ok()?;
                guard
                    .get_controlling_player_id()
                    .map(|idx| idx as PlayerIndex)
            });
        } else {
            self.runtime.data.surrendered_frames_left = 0;
            self.runtime.data.surrendered_player_index = None;
        }
    }
    pub(super) fn transfer_attack(&mut self, from_id: ObjectID, to_id: ObjectID) {
        use crate::helpers::TheGameLogic;

        let new_target = TheGameLogic::find_object_by_id(to_id);

        if self.runtime.current_victim_id == from_id {
            self.runtime.current_victim_id = to_id;
        }

        let goal_id = self.get_goal_object_id();
        if goal_id != INVALID_ID && goal_id == from_id {
            self.set_goal_object(
                new_target
                    .as_ref()
                    .and_then(|a| a.read().ok().map(|g| g.get_id())),
            );
        }

        for turret in [TurretType::Primary, TurretType::Secondary] {
            let needs_transfer = match turret {
                TurretType::Primary => self
                    .runtime
                    .data
                    .turret_primary_machine
                    .as_mut()
                    .map(|machine| {
                        // transferAttack passes FALSE, so a dead goal is kept and still compared.
                        let (kind, id, _) = machine.turret_mut().friend_get_turret_target(false);
                        kind == crate::ai::turret::TurretTargetKind::Object && id == Some(from_id)
                    })
                    .unwrap_or(false),
                TurretType::Secondary => self
                    .runtime
                    .data
                    .turret_secondary_machine
                    .as_mut()
                    .map(|machine| {
                        // transferAttack passes FALSE, so a dead goal is kept and still compared.
                        let (kind, id, _) = machine.turret_mut().friend_get_turret_target(false);
                        kind == crate::ai::turret::TurretTargetKind::Object && id == Some(from_id)
                    })
                    .unwrap_or(false),
                _ => continue,
            };
            if needs_transfer {
                self.set_turret_target_object(
                    turret,
                    new_target
                        .as_ref()
                        .and_then(|a| a.read().ok().map(|g| g.get_id())),
                    true,
                );
            }
        }
    }
    pub(super) fn is_surrendered(&self) -> bool {
        self.runtime.data.is_surrendered()
    }
    pub(super) fn get_surrendered_player_index(&self) -> Option<PlayerIndex> {
        self.runtime.data.get_surrendered_player_index()
    }
    pub(super) fn ai_move_to_position(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit = get_unit_arc(self.runtime.unit_id)
            .ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.give_move_order(*pos, Vec::new(), false, false)?;
        Ok(())
    }
    pub(super) fn ai_idle(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(machine) = self.ai_state_machine.as_mut() {
            let mut runtime = super::UnitAiStateRuntime::new(&mut self.runtime, true);
            let mut driver = machine.driver();
            driver.clear_with_ai(&mut runtime);
            let params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::Idle,
                CommandSourceType::FromAi,
            );
            let _ = driver.ai_do_command_with_ai(&params, &mut runtime);
            return Ok(());
        }
        if let Some(unit) = get_unit_arc(self.runtime.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.stop_movement();
            }
        }
        Ok(())
    }
    pub(super) fn ai_busy(
        &mut self,
        cmd_source: crate::ai::CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let params = crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Busy, cmd_source);
        self.execute_command(&params)
    }
    pub(super) fn ai_attack_object(
        &mut self,
        target_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit = get_unit_arc(self.runtime.unit_id)
            .ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.give_attack_order(target_id, true, false)?;
        Ok(())
    }
    pub(super) fn ai_guard_position(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit = get_unit_arc(self.runtime.unit_id)
            .ok_or_else(|| "unit no longer available".to_string())?;
        self.push_guard_target_type(GuardTargetType::Location);
        self.runtime.data.location_to_guard = *pos;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.current_order = Some(UnitOrder::Guard {
            position: *pos,
            area_radius: guard.engagement_range,
        });
        guard.order_queue.clear();
        Ok(())
    }
    pub(super) fn get_crate_id(&self) -> ObjectID {
        self.runtime.get_crate_id()
    }
    pub(super) fn get_current_victim(&self) -> Option<ObjectID> {
        self.runtime.get_current_victim()
    }
    pub(super) fn set_current_victim(&mut self, victim: Option<ObjectID>) {
        self.runtime.set_current_victim(victim)
    }
    pub(super) fn check_for_crate_to_pickup_id(&mut self) -> ObjectID {
        self.runtime.check_for_crate_to_pickup_id()
    }
    pub(super) fn get_next_mood_check_time(&self) -> u32 {
        self.runtime.get_next_mood_check_time()
    }
    pub(super) fn reset_next_mood_check_time(&mut self) {
        self.runtime.reset_next_mood_check_time()
    }
    pub(super) fn set_next_mood_check_time(&mut self, frame: u32) {
        self.runtime.data.set_next_mood_check_time(frame)
    }
    pub(super) fn get_mood_matrix_value(&self) -> u32 {
        self.runtime
            .get_mood_matrix_value(self.ai_state_machine.is_some())
    }
    pub(super) fn get_mood_matrix_action_adjustment(&mut self, action: MoodMatrixAction) -> u32 {
        self.runtime
            .get_mood_matrix_action_adjustment(self.ai_state_machine.is_some(), action)
    }
    pub(super) fn notify_fired(&mut self) {}
    pub(super) fn notify_new_victim_chosen(&mut self, victim: ObjectID) {
        if let Some(machine) = self.ai_state_machine.as_mut() {
            machine.set_goal_object(victim);
        }
    }
    pub(super) fn is_weapon_slot_ok_to_fire(&self, _wslot: WeaponSlotType) -> Bool {
        self.runtime.data.is_weapon_slot_ok_to_fire(_wslot)
    }
    pub(super) fn get_original_victim_pos(&self) -> Option<Coord3D> {
        self.runtime.data.get_original_victim_pos()
    }
    pub(super) fn set_original_victim_pos(&mut self, pos: Option<Coord3D>) {
        self.runtime.set_original_victim_pos(pos)
    }
    pub(super) fn is_in_attack_state(&self) -> bool {
        self.ai_state_machine
            .as_ref()
            .is_some_and(AIStateMachine::is_in_attack_state)
    }
    pub(super) fn is_in_guard_idle_state(&self) -> bool {
        self.ai_state_machine
            .as_ref()
            .is_some_and(AIStateMachine::is_in_guard_idle_state)
    }
    pub(super) fn set_temporary_state(&mut self, state: AIStateType, frame_limit: UnsignedInt) {
        if let Some(machine) = self.ai_state_machine.as_mut() {
            let mut runtime = super::UnitAiStateRuntime::new(&mut self.runtime, true);
            let _ =
                machine
                    .driver()
                    .enter_temporary_with_ai(state as u32, frame_limit, &mut runtime);
        }
    }
    pub(super) fn do_quick_exit(&mut self, path: &[Coord3D]) {
        let Some(machine) = self.ai_state_machine.as_mut() else {
            return;
        };
        let locked = machine.is_locked();
        machine.unlock();
        machine.set_goal_path(path);
        machine.install_follow_exit_path(path);
        let mut runtime = super::UnitAiStateRuntime::new(&mut self.runtime, true);
        let _ = machine.driver().enter_temporary_with_ai(
            AIStateType::FollowExitProductionPath as u32,
            10 * crate::common::LOGICFRAMES_PER_SECOND as UnsignedInt,
            &mut runtime,
        );
        if locked {
            machine.lock();
        }
    }
    pub(super) fn notify_crate(&mut self, crate_id: ObjectID) {
        self.runtime.crate_created = crate_id;
    }
    pub(super) fn notify_victim_is_dead(&mut self) {
        if let Some(jet_ai) = self.runtime.components.jet_ai.as_mut() {
            jet_ai.notify_victim_is_dead();
        }
    }
    pub(super) fn set_prior_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        self.runtime.set_prior_waypoint_id(waypoint_id)
    }
    pub(super) fn set_current_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        self.runtime.set_current_waypoint_id(waypoint_id)
    }
    pub(super) fn set_completed_waypoint_id(
        &mut self,
        waypoint_id: Option<crate::waypoint::WaypointId>,
    ) {
        self.runtime.set_completed_waypoint_id(waypoint_id)
    }
    pub(super) fn get_completed_waypoint_id(&self) -> Option<crate::waypoint::WaypointId> {
        self.runtime.data.get_completed_waypoint_id()
    }
}
