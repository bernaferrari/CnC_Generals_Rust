//! Machine-independent native UnitAI operations and the short-lived callback view.
use super::UnitAiStateRuntime;
use super::ai_core::{UnitAiRuntime, play_combat_drop_kill_fx};
use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{dual_world_registry_unavailable, get_unit_arc};
use super::types::*;
use crate::ai::vision_factors;
use crate::modules::ai_state_runtime::AiStateRuntime;
use crate::object::update::ai_update_interface::{
    AUTO_ACQUIRE_IDLE, AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS, AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING,
    AUTO_ACQUIRE_IDLE_STEALTHED,
};

impl UnitAiRuntime {
    /// Upgrade this runtime's construction-bound owner. A stale native owner
    /// never selects another instance by numeric ObjectID.
    pub(super) fn native_owner(&self) -> Option<Arc<RwLock<Object>>> {
        self.owner.as_ref().and_then(Weak::upgrade)
    }

    pub(super) fn get_supply_truck_ai_interface(
        &self,
    ) -> Option<&dyn crate::modules::SupplyTruckAIInterface> {
        if let Some(ai) = self.components.chinook_ai.as_ref() {
            Some(ai as &dyn crate::modules::SupplyTruckAIInterface)
        } else if let Some(ai) = self.components.worker_ai.as_ref() {
            Some(ai as &dyn crate::modules::SupplyTruckAIInterface)
        } else {
            self.components
                .supply_truck_ai
                .as_ref()
                .map(|ai| ai as &dyn crate::modules::SupplyTruckAIInterface)
        }
    }
    pub(super) fn get_supply_truck_ai_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::SupplyTruckAIInterface> {
        if let Some(ai) = self.components.chinook_ai.as_mut() {
            Some(ai as &mut dyn crate::modules::SupplyTruckAIInterface)
        } else if let Some(ai) = self.components.worker_ai.as_mut() {
            Some(ai as &mut dyn crate::modules::SupplyTruckAIInterface)
        } else {
            self.components
                .supply_truck_ai
                .as_mut()
                .map(|ai| ai as &mut dyn crate::modules::SupplyTruckAIInterface)
        }
    }

    pub(super) fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String> {
        if let Some(path) = self.data.pending_safe_path.take() {
            return self.set_path_from_coords(&path);
        }
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard
            .give_move_order(*target, Vec::new(), false, false)
            .map_err(|err| err.to_string())
    }
    pub(super) fn get_preferred_height(&self) -> Option<Real> {
        self.data.get_preferred_height()
    }
    pub(super) fn with_cur_locomotor(&self, f: &mut dyn FnMut(&crate::locomotor::Locomotor)) {
        self.data.with_cur_locomotor(f)
    }
    pub(super) fn with_cur_locomotor_mut(
        &mut self,
        f: &mut dyn FnMut(&mut crate::locomotor::Locomotor),
    ) {
        self.data.with_cur_locomotor_mut(f)
    }
    pub(super) fn get_path_destination(&self) -> Option<Coord3D> {
        self.data
            .current_path_snapshot
            .as_ref()?
            .get_last_node_position()
            .copied()
    }
    pub(super) fn get_path_last_node(&self) -> Option<Coord3D> {
        self.data
            .current_path_snapshot
            .as_ref()?
            .get_last_node_position()
            .copied()
    }

    /// C++ AIUpdateInterface::getLocomotorDistanceToGoal over this runtime's
    /// construction-bound Object and serialized AiPath. Mutable path access is
    /// required because computePointOnPath updates its path-local CPOP cache.
    pub(super) fn get_locomotor_distance_to_goal(
        &mut self,
        fallback_goal: Option<Coord3D>,
    ) -> Real {
        // C++ POSITION_EXPLICIT is unimplemented and NONE/ANGLE are already there.
        if self.data.locomotor_goal_type != 1 {
            return 0.0;
        }
        let Some(owner_arc) = self.native_owner() else {
            return Real::INFINITY;
        };
        let Some(locomotor) = self.data.locomotor_set.get_active() else {
            return 0.0;
        };
        let legal_surfaces = locomotor.get_legal_surfaces();
        let close_enough_3d = locomotor.is_close_enough_dist_3d();
        let appearance = locomotor.get_appearance();
        let path_extra_distance = self.data.path_extra_distance;
        let ignore_obstacle_id =
            (self.data.ignore_obstacle_id != INVALID_ID).then_some(self.data.ignore_obstacle_id);
        let (position, layer, is_projectile, is_ground) = {
            let Ok(owner) = owner_arc.read() else {
                return Real::INFINITY;
            };
            (
                *owner.get_position(),
                crate::ai::pathfind::PathfindLayerEnum::from_u32(owner.get_layer() as u32),
                owner.is_kind_of(KindOf::Projectile),
                self.is_doing_ground_movement_for_owner(&owner),
            )
        };
        let treat_as_aircraft = self.components.jet_ai.as_ref().map_or_else(
            || {
                !is_ground
                    || path_extra_distance > PATHFIND_CLOSE_ENOUGH
                    || appearance == LocomotorAppearance::Hover
            },
            |jet| jet.get_treat_as_aircraft_for_loco_dist_to_goal(),
        );

        let Some(path) = self.data.current_path_snapshot.as_mut() else {
            // C++ returns zero (after its debug assertion) when POSITION_ON_PATH
            // has no Path. The native view does not consult a Unit projection.
            return 0.0;
        };
        let last_node = path.get_last_node_position().copied();
        if close_enough_3d || is_projectile {
            let Some(goal_position) = last_node.or(fallback_goal) else {
                // C++ reads the parent FSM goal on this branch. If neither that
                // goal nor a path tail exists, infinity avoids false arrival.
                return Real::INFINITY;
            };
            return (goal_position - position).length();
        }

        let (distance, mut goal_position) = if treat_as_aircraft {
            path.compute_flight_dist_to_goal(&position)
        } else {
            let pathfinder = crate::ai::the_ai()
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder());
            let Some(pathfinder) = pathfinder else {
                return Real::INFINITY;
            };
            let mut context = super::ai_loco::NativePathMovementContext::new(
                pathfinder,
                legal_surfaces,
                ignore_obstacle_id,
                layer,
            );
            let info = path.compute_point_on_path(&position, &mut context);
            (info.dist_along_path, info.pos_on_path)
        };
        if let Some(last_node) = last_node {
            goal_position = last_node;
        }

        let dx = goal_position.x - position.x;
        let dy = goal_position.y - position.y;
        let horizontal_distance_squared = dx * dx + dy * dy;
        if treat_as_aircraft {
            if distance * distance > horizontal_distance_squared {
                horizontal_distance_squared.sqrt()
            } else {
                distance
            }
        } else if distance < PATHFIND_CELL_SIZE_F
            || distance * distance < horizontal_distance_squared
        {
            horizontal_distance_squared.sqrt()
        } else {
            distance
        }
    }

    pub(super) fn get_legacy_locomotor_distance_to_goal(&self) -> Real {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return 0.0;
        };
        let Ok(guard) = unit.read() else {
            return 0.0;
        };
        let Some(loc_guard) = self.data.locomotor_set.get_active() else {
            return 0.0;
        };

        let obj_pos = guard.get_position();
        let is_projectile = guard
            .base_arc()
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::Projectile))
            .unwrap_or(false);
        let mut treat_as_aircraft = guard.path_extra_distance > PATHFIND_CLOSE_ENOUGH
            || loc_guard.get_appearance() == LocomotorAppearance::Hover;
        if let Some(jet_ai) = self.components.jet_ai.as_ref() {
            treat_as_aircraft = jet_ai.get_treat_as_aircraft_for_loco_dist_to_goal();
        }

        if let Some(active_path) = loc_guard.active_path.as_ref() {
            let last_waypoint = active_path.waypoints.last().copied();
            let goal_pos = last_waypoint
                .or(guard.target_position)
                .or_else(|| {
                    guard
                        .path_following_state
                        .as_ref()
                        .map(|state| state.goal_position)
                })
                .unwrap_or(obj_pos);

            if loc_guard.is_close_enough_dist_3d() || is_projectile {
                return (goal_pos - obj_pos).length();
            }

            if treat_as_aircraft {
                let delta = goal_pos - obj_pos;
                let dist = delta.length();
                let dist_sqr = delta.x * delta.x + delta.y * delta.y;
                if dist * dist > dist_sqr {
                    return dist_sqr.sqrt();
                }
                return dist;
            }

            let dist_remaining = active_path.distance_remaining().max(0.0);
            let dist = if let Some(current_target) = active_path.current_target() {
                let delta = current_target - obj_pos;
                (delta.x * delta.x + delta.y * delta.y).sqrt() + dist_remaining
            } else {
                dist_remaining
            };

            let dx = goal_pos.x - obj_pos.x;
            let dy = goal_pos.y - obj_pos.y;
            let dist_sqr = dx * dx + dy * dy;
            if dist < PATHFIND_CELL_SIZE_F || dist * dist < dist_sqr {
                return dist_sqr.sqrt();
            }
            return dist;
        }

        if let Some(state) = guard.path_following_state.as_ref() {
            let delta = state.goal_position - obj_pos;
            return (delta.x * delta.x + delta.y * delta.y).sqrt();
        }

        0.0
    }
    pub(super) fn get_last_command_source(&self) -> CommandSourceType {
        self.data.get_last_command_source()
    }
    pub(super) fn set_last_command_source(&mut self, source: CommandSourceType) {
        self.data.set_last_command_source(source)
    }
    pub(super) fn set_locomotor_goal_none(&mut self) {
        let jet_keeps_air_goal = self.components.jet_ai.as_ref().is_some_and(|jet_ai| {
            jet_ai.is_takeoff_or_landing_in_progress()
                && jet_ai.allow_air_loco()
                && !jet_ai.allow_circling()
        });
        if jet_keeps_air_goal {
            if let Some(owner) = self.native_owner() {
                if let Ok(guard) = owner.read() {
                    let (dir_x, dir_y) = guard.get_unit_direction_vector_2d();
                    let mut desired = *guard.get_position();
                    desired.x += dir_x * 1000.0;
                    desired.y += dir_y * 1000.0;
                    drop(guard);
                    self.set_locomotor_goal_position_explicit(desired);
                    return;
                }
            }
        }
        self.data.locomotor_goal_type = 0;
    }
    pub(super) fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        self.data.set_locomotor_goal_orientation(angle)
    }
    pub(super) fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        self.data.set_locomotor_goal_position_explicit(pos)
    }
    pub(super) fn friend_ending_move(&mut self) {
        self.data.friend_ending_move()
    }
    pub(super) fn friend_starting_move(&mut self) {
        self.data.friend_starting_move()
    }
    pub(super) fn is_allowed_to_adjust_destination(&self) -> bool {
        if let Some(chinook) = self.components.chinook_ai.as_ref() {
            if self
                .data
                .locomotor_set
                .get_active()
                .is_some_and(|loco| loco.is_allowing_invalid_positions())
            {
                return false;
            }
            chinook.is_allowed_to_adjust_destination()
        } else {
            true
        }
    }
    pub(super) fn get_desired_speed(&self) -> Real {
        self.data.get_desired_speed()
    }
    pub(super) fn set_desired_speed(&mut self, speed: Real) {
        self.data.set_desired_speed(speed)
    }
    pub(super) fn is_in_rappel_state(&self) -> bool {
        self.data.is_in_rappel_state()
    }
    pub(super) fn is_doing_combat_drop(&self) -> bool {
        self.components
            .chinook_ai
            .as_ref()
            .map(|ai| ai.is_doing_combat_drop())
            .unwrap_or(false)
    }
    pub(super) fn set_queue_for_path_time(&mut self, frames: UnsignedInt) {
        self.data.queue_for_path_frame = if frames == 0 {
            0
        } else {
            TheGameLogic::get_frame().saturating_add(frames)
        };
    }
    pub(super) fn is_temporarily_preventing_aim_success(&self) -> bool {
        self.components
            .jet_ai
            .as_ref()
            .map(|jet| jet.is_temporarily_preventing_aim_success())
            .unwrap_or(false)
    }
    pub(super) fn add_targeter(&mut self, id: ObjectID, add: bool) {
        if let Some(jet_ai) = self.components.jet_ai.as_mut() {
            jet_ai.add_targeter(id, add);
        }
    }
    pub(super) fn clear_guard_target_type(&mut self) {
        self.data.clear_guard_target_type()
    }
    pub(super) fn set_turret_target_object(
        &mut self,
        turret: TurretType,
        target_id: Option<ObjectID>,
        force_attacking: bool,
    ) {
        if let Some(machine) = self.ensure_turret_machine(turret) {
            machine
                .turret_mut()
                .set_current_target_with_force(target_id, force_attacking);
        }
    }
    pub(super) fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        slot: WeaponSlotType,
        target: ObjectID,
    ) -> bool {
        self.data
            .is_weapon_slot_on_turret_and_aiming_at_target(slot, target)
    }
    pub(super) fn ignore_obstacle(
        &mut self,
        obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.data.ignore_obstacle(obj_id)
    }
    pub(super) fn ignore_obstacle_id(
        &mut self,
        id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.data.ignore_obstacle_id(id)
    }
    pub(super) fn set_current_goal_path_index(
        &mut self,
        index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.data.set_current_goal_path_index(index)
    }
    pub(super) fn get_current_goal_path_index(&self) -> i32 {
        self.data.get_current_goal_path_index()
    }
    pub(super) fn set_can_path_through_units(
        &mut self,
        value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.data.set_can_path_through_units(value)
    }
    pub(super) fn get_can_path_through_units(&self) -> bool {
        self.data.get_can_path_through_units()
    }
    pub(super) fn is_blocked_and_stuck(&self) -> bool {
        const BLOCKED_RECOMPUTE_THRESHOLD: u32 = 60;
        if self.owner.is_some() {
            return self.data.blocked_and_stuck;
        }
        if self.data.blocked_and_stuck {
            return true;
        }
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        guard.path_following_state.as_ref().map_or(false, |state| {
            state.frames_blocked > BLOCKED_RECOMPUTE_THRESHOLD
        })
    }
    pub(super) fn get_num_frames_blocked(&self) -> u32 {
        let mut frames = self.data.blocked_frames;
        if self.owner.is_some() {
            return frames;
        }
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return frames;
        };
        let Ok(guard) = unit.read() else {
            return frames;
        };
        if let Some(state) = guard.path_following_state.as_ref() {
            frames = frames.max(state.frames_blocked);
        }
        frames
    }
    pub(super) fn destroy_path(&mut self) {
        self.data.current_path_snapshot = None;
        self.data.installed_path_layers.clear();
        self.data.waiting_for_path = false;
        self.data.is_attack_path = false;
        self.set_locomotor_goal_none();
    }
    pub(super) fn clear_move_out_of_way(&mut self) {
        self.data.clear_move_out_of_way()
    }
    pub(super) fn request_path(
        &mut self,
        destination: &Coord3D,
        _is_final_goal: bool,
    ) -> Result<(), String> {
        self.data.requested_destination = *destination;
        self.data.is_final_goal = _is_final_goal;
        self.data.is_attack_path = false;
        self.data.requested_victim_id = INVALID_ID;
        self.data.is_approach_path = false;
        self.data.is_safe_path = false;
        if !self.has_valid_locomotor_surfaces() {
            return Err("Attempting to path immobile unit".to_string());
        }
        if self.can_compute_quick_path() {
            self.compute_quick_path(destination);
            return Ok(());
        }
        self.data.waiting_for_path = true;
        let now = TheGameLogic::get_frame();
        if self.data.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);
            if self.data.blocked_and_stuck {
                self.set_ignore_collision_time(LOGICFRAMES_PER_SECOND * 2);
                self.data.blocked_frames = 0;
                self.data.is_blocked = false;
                self.data.blocked_and_stuck = false;
            }
            return Ok(());
        }
        self.set_queue_for_path_time(0);
        let _ = self.queue_path_request_now(*destination);
        self.data.path_timestamp = now;
        Ok(())
    }
    pub(super) fn can_compute_quick_path(&self) -> bool {
        let Some(owner) = self.native_owner() else {
            return false;
        };
        let Ok(owner) = owner.read() else {
            return false;
        };
        let Some(surfaces) = self
            .data
            .locomotor_set
            .get_default_locomotor()
            .map(|loco| loco.get_legal_surfaces())
        else {
            return false;
        };
        if (surfaces & SURFACE_AIR) == 0 {
            return false;
        }
        !self.is_doing_ground_movement_for_owner(&owner)
    }
    pub(super) fn get_crate_id(&self) -> ObjectID {
        self.crate_created
    }
    pub(super) fn get_current_victim(&self) -> Option<ObjectID> {
        (self.current_victim_id != INVALID_ID).then_some(self.current_victim_id)
    }
    pub(super) fn set_current_victim(&mut self, victim: Option<ObjectID>) {
        // AIUpdate.cpp:4173–4190 notifies the old target synchronously before
        // clearing the victim. A self-target uses this exact runtime instead
        // of trying to lock its cached interface while we are executing it.
        if victim.is_none() && self.current_victim_id != INVALID_ID {
            let old_id = self.current_victim_id;
            if old_id == self.unit_id {
                self.add_targeter(self.unit_id, false);
            } else if let Some(ai) = crate::object::registry::OBJECT_REGISTRY
                .with_object(old_id, |old_guard| old_guard.get_ai_update_interface())
                .flatten()
            {
                if let Ok(mut ai_guard) = ai.lock() {
                    ai_guard.add_targeter(self.unit_id, false);
                }
            }
        }

        // A non-null assignment deliberately does not add a targeter yet.
        self.current_victim_id = victim.unwrap_or(INVALID_ID);
    }
    pub(super) fn check_for_crate_to_pickup_id(&mut self) -> ObjectID {
        if self.crate_created == crate::common::INVALID_ID {
            return INVALID_ID;
        }
        // C++ clears m_crateCreated before the lookup, so the processed marker
        // does not yield a crate object from this path.
        self.crate_created = crate::common::INVALID_ID;
        INVALID_ID
    }
    pub(super) fn get_next_mood_check_time(&self) -> u32 {
        self.data.get_next_mood_check_time()
    }
    pub(super) fn reset_mood_check_at(&mut self, frame: u32, force_idle_frames: u32) {
        // AIUpdate.cpp:4443-4448: these are AI-owned fields. The driving
        // operation supplies its clock/rules; no Unit identity lookup belongs here.
        self.data.next_mood_check_time = frame.wrapping_add(force_idle_frames);
        self.data.randomly_offset_mood_check = true;
    }
    pub(super) fn reset_next_mood_check_time(&mut self) {
        // Owner-free callers are a distinct adapter. Native ticks reset through
        // reset_mood_check_at with their driving operation's inputs.
        if self.owner.is_some() {
            return;
        }
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return;
        };
        // This legacy adapter still needs a registered Unit. Do not discover
        // an ambient frame for ordinary factory runtimes: their driving
        // frame/rules execution boundary is tracked separately in hq-6tx39.
        let now = TheGameLogic::get_frame();
        let ai = the_ai();
        let Ok(ai_rules) = ai.read() else {
            return;
        };
        let force_idle_frames = ai_rules.get_ai_data().force_idle_frames_count;
        drop(ai_rules);
        let Ok(mut guard) = unit.write() else {
            return;
        };
        // Preserve the independent legacy scan kernel until its ownership
        // migration. This is a last-scan stamp, not the AI's next-mood timer.
        guard.last_target_scan_frame = now;
        self.reset_mood_check_at(now, force_idle_frames);
    }
    pub(super) fn get_mood_matrix_value(&self, parent_exists: bool) -> u32 {
        if !parent_exists {
            return 0;
        }

        let Some(owner_arc) = self.owner.as_ref().and_then(Weak::upgrade) else {
            return 0;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return 0;
        };
        let Some(controller_is_human) = owner_guard.with_controlling_player(|player_guard| {
            player_guard.get_player_type() == crate::player::PlayerType::Human
        }) else {
            return 0;
        };

        let mut value = 0u32;
        if controller_is_human {
            value |= mood_matrix_parameters::CONTROLLER_PLAYER;
        } else {
            value |= mood_matrix_parameters::CONTROLLER_AI;
            value |= match self.data.attitude {
                AIAttitudeType::Passive => mood_matrix_parameters::MOOD_PASSIVE,
                AIAttitudeType::Defensive => mood_matrix_parameters::MOOD_ALERT,
                AIAttitudeType::Aggressive => mood_matrix_parameters::MOOD_AGGRESSIVE,
                AIAttitudeType::Sleep => mood_matrix_parameters::MOOD_SLEEP,
                AIAttitudeType::Normal => mood_matrix_parameters::MOOD_NORMAL,
            };
        }

        let is_air = self
            .data
            .locomotor_set
            .get_active()
            .is_some_and(|loco| (loco.get_legal_surfaces() & SURFACE_AIR) != 0);
        if is_air {
            value |= mood_matrix_parameters::UNITTYPE_AIR;
        } else if self.data.turret_primary_machine.is_some() {
            value |= mood_matrix_parameters::UNITTYPE_TURRETED;
        } else {
            value |= mood_matrix_parameters::UNITTYPE_NON_TURRETED;
        }

        value
    }
    pub(super) fn get_mood_matrix_action_adjustment(
        &mut self,
        parent_exists: bool,
        action: MoodMatrixAction,
    ) -> u32 {
        let Some(owner_arc) = self.owner.as_ref().and_then(Weak::upgrade) else {
            return mood_matrix_adjustment::ACTION_OK;
        };
        // C++ mob-member special case. Release this read before calculating
        // controller/mood through the same bound Object.
        let is_mob_member = owner_arc.read().ok().is_some_and(|owner| {
            owner.is_kind_of(KindOf::Infantry) && owner.is_kind_of(KindOf::IgnoredInGui)
        });
        if is_mob_member {
            return mood_matrix_adjustment::ACTION_OK;
        }

        let mood_matrix = self.get_mood_matrix_value(parent_exists);
        if (mood_matrix & mood_matrix_parameters::CONTROLLER_PLAYER) != 0 {
            return mood_matrix_adjustment::ACTION_OK;
        }

        match action {
            MoodMatrixAction::Idle => match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                mood_matrix_parameters::MOOD_SLEEP => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                }
                mood_matrix_parameters::MOOD_PASSIVE => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_WAIT_FOR_ATTACK
                }
                mood_matrix_parameters::MOOD_ALERT => {
                    mood_matrix_adjustment::ACTION_OK | mood_matrix_adjustment::AFFECT_RANGE_ALERT
                }
                mood_matrix_parameters::MOOD_AGGRESSIVE => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_AGGRESSIVE
                }
                _ => mood_matrix_adjustment::ACTION_OK,
            },
            MoodMatrixAction::Move => match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                mood_matrix_parameters::MOOD_SLEEP => {
                    mood_matrix_adjustment::ACTION_TO_IDLE
                        | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                }
                mood_matrix_parameters::MOOD_PASSIVE => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_WAIT_FOR_ATTACK
                }
                mood_matrix_parameters::MOOD_ALERT => {
                    mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE
                        | mood_matrix_adjustment::AFFECT_RANGE_ALERT
                }
                mood_matrix_parameters::MOOD_AGGRESSIVE => {
                    mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE
                        | mood_matrix_adjustment::AFFECT_RANGE_AGGRESSIVE
                }
                _ => mood_matrix_adjustment::ACTION_OK,
            },
            MoodMatrixAction::Attack => match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                mood_matrix_parameters::MOOD_SLEEP => {
                    mood_matrix_adjustment::ACTION_TO_IDLE
                        | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                }
                _ => mood_matrix_adjustment::ACTION_OK,
            },
            MoodMatrixAction::AttackMove => {
                match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                    mood_matrix_parameters::MOOD_SLEEP => {
                        mood_matrix_adjustment::ACTION_TO_IDLE
                            | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                    }
                    mood_matrix_parameters::MOOD_ALERT => {
                        mood_matrix_adjustment::ACTION_OK
                            | mood_matrix_adjustment::AFFECT_RANGE_ALERT
                    }
                    mood_matrix_parameters::MOOD_AGGRESSIVE => {
                        mood_matrix_adjustment::ACTION_OK
                            | mood_matrix_adjustment::AFFECT_RANGE_AGGRESSIVE
                    }
                    _ => mood_matrix_adjustment::ACTION_OK,
                }
            }
        }
    }
    pub(super) fn set_original_victim_pos(&mut self, pos: Option<Coord3D>) {
        self.data.set_original_victim_pos(pos)
    }
    pub(super) fn set_prior_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        self.data.set_prior_waypoint_id(waypoint_id)
    }
    pub(super) fn set_current_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        self.data.set_current_waypoint_id(waypoint_id)
    }
    pub(super) fn set_completed_waypoint_id(
        &mut self,
        waypoint_id: Option<crate::waypoint::WaypointId>,
    ) {
        self.data.set_completed_waypoint_id(waypoint_id)
    }
    pub(super) fn choose_locomotor_set(
        &mut self,
        set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut target_set = set;
        if self
            .components
            .chinook_ai
            .as_ref()
            .is_some_and(|chinook| chinook.is_landed())
        {
            target_set = LocomotorSetType::Taxiing;
        }
        if target_set == LocomotorSetType::Normal && self.data.locomotor_upgraded {
            target_set = LocomotorSetType::NormalUpgraded;
        }
        if let Some(jet_ai) = self.components.jet_ai.as_ref() {
            if let Some(desired) = jet_ai.desired_locomotor_set() {
                target_set = desired;
            }
        }

        if target_set == self.data.current_locomotor_set {
            return Ok(());
        }

        let Some(locomotors) = self.data.locomotor_sets.get(&target_set) else {
            return Ok(());
        };

        self.data.current_locomotor_set = target_set;

        let mut new_set = LocomotorSet::new();
        for locomotor_name in locomotors {
            if let Some(template) =
                crate::locomotor::LOCOMOTOR_STORE.get_template(locomotor_name.as_str())
            {
                new_set.add_locomotor(
                    locomotor_name.as_str().to_string(),
                    Locomotor::new(template),
                );
            } else {
                log::warn!("Locomotor template '{}' not found", locomotor_name.as_str());
            }
        }

        let prev_name = self
            .data
            .locomotor_set
            .active_name()
            .map(|name| name.to_string());
        self.data.locomotor_set = new_set;
        let new_name = self
            .data
            .locomotor_set
            .active_name()
            .map(|name| name.to_string());
        if prev_name != new_name {
            if let Some(loco) = self.data.locomotor_set.get_active_mut() {
                loco.set_precise_z_pos(false);
                loco.set_no_slow_down(false);
                loco.set_ultra_accurate(false);
            }
        }

        Ok(())
    }
    pub(super) fn set_allow_invalid_position(
        &mut self,
        allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.data.set_allow_invalid_position(allow)
    }
    pub(super) fn set_precise_z_pos(
        &mut self,
        precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.data.set_precise_z_pos(precise)
    }
    pub(super) fn get_speed(&self) -> f32 {
        if let Some(owner) = self.owner.as_ref() {
            let Some(owner) = owner.upgrade() else {
                return 0.0;
            };
            let Ok(owner) = owner.read() else {
                return 0.0;
            };
            let Some(locomotor) = self.data.locomotor_set.get_active() else {
                return 0.0;
            };
            let body_state = owner
                .get_body_module()
                .and_then(|body| {
                    body.lock()
                        .ok()
                        .map(|body| to_locomotor_body_damage_type(body.get_damage_state()))
                })
                .unwrap_or(BodyDamageType::Pristine);
            // C++ AIGroup::recompute asks AIUpdateInterface::getCurLocomotorSpeed.
            return locomotor.get_max_speed_for_condition(body_state);
        }

        // Preserve the standalone Unit adapter when no native owner was captured.
        get_unit_arc(self.unit_id)
            .and_then(|unit| unit.read().ok().map(|guard| guard.current_speed))
            .unwrap_or(0.0)
    }
    pub(super) fn set_path_extra_distance(
        &mut self,
        distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.data.path_extra_distance = distance;
        Ok(())
    }
    pub(super) fn set_path_from_waypoint(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        group_offset: &Coord2D,
    ) -> Result<(), String> {
        let owner = self
            .native_owner()
            .ok_or_else(|| "unit owner no longer available".to_string())?;
        let start_pos = *owner
            .read()
            .map_err(|_| "unit owner lock poisoned".to_string())?
            .get_position();

        let terrain_owner_handle = crate::terrain::get_terrain_logic();

        let terrain = terrain_owner_handle
            .read()
            .map_err(|_| "terrain lock poisoned".to_string())?;

        self.destroy_path();

        // Build a chain following link 0 to match the classic path order.
        let mut visited = std::collections::HashSet::new();
        let mut waypoints = Vec::new();
        let mut path_coords = vec![start_pos];
        let mut current = waypoint.clone();
        for _ in 0..=WAYPOINT_PATH_LIMIT {
            if !visited.insert(current.id) {
                break;
            }
            let next_id = current.get_link(0);
            let mut adjusted = current.clone();
            adjusted.position.x += group_offset.x;
            adjusted.position.y += group_offset.y;
            adjusted.position.z =
                terrain.get_ground_height(adjusted.position.x, adjusted.position.y, None);
            if next_id.is_none() {
                adjusted.position = the_ai()
                    .read()
                    .ok()
                    .and_then(|ai| ai.pathfinder())
                    .and_then(|pathfinder| {
                        pathfinder
                            .read()
                            .ok()
                            .map(|pf| pf.snap_position(&adjusted.position))
                    })
                    .unwrap_or(adjusted.position);
            }
            path_coords.push(adjusted.position);
            waypoints.push(adjusted);

            let Some(next_id) = next_id else {
                break;
            };
            let Some(next) = terrain.get_waypoint_by_id(next_id) else {
                break;
            };
            current = crate::waypoint::Waypoint::from_terrain(next);
        }

        if waypoints.is_empty() {
            return Ok(());
        }

        let last = waypoints
            .last()
            .map(|waypoint| waypoint.position)
            .expect("waypoints is not empty");
        self.data.requested_destination = last;
        self.data.planning_waypoint_count = 0;
        self.data.planning_waypoint_index = 0;
        self.data.executing_waypoint_queue = false;
        self.data.blocked_frames = 0;
        self.data.blocked_and_stuck = false;
        self.data.waiting_for_path = false;
        self.data.queue_for_path_frame = 0;
        self.data.path_timestamp = TheGameLogic::get_frame();
        self.set_current_path_snapshot_from_coords(&path_coords);

        Ok(())
    }
    pub(super) fn is_waiting_for_path(&self) -> bool {
        self.data.is_waiting_for_path()
    }
    pub(super) fn append_goal_position_to_path(&mut self, goal: &Coord3D) -> Result<(), String> {
        if self.data.current_path_snapshot.is_some() {
            self.append_current_path_snapshot_goal(goal);
        }
        Ok(())
    }
    pub(super) fn request_safe_path(&mut self, repulsor_id: ObjectID) -> Result<bool, String> {
        self.data.is_final_goal = false;
        self.data.is_attack_path = false;
        self.data.requested_victim_id = INVALID_ID;
        self.data.is_approach_path = false;
        self.data.is_safe_path = true;
        self.data.waiting_for_path = true;
        if repulsor_id != self.data.repulsor1 {
            self.data.repulsor2 = self.data.repulsor1;
        }
        self.data.repulsor1 = repulsor_id;
        let now = TheGameLogic::get_frame();
        if self.data.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            return Ok(false);
        }
        self.set_queue_for_path_time(0);
        self.data.path_timestamp = now;
        Ok(true)
    }
    pub(super) fn is_doing_ground_movement(&self) -> bool {
        let Some(owner) = self.native_owner() else {
            return false;
        };
        let Ok(owner) = owner.read() else {
            return false;
        };
        self.is_doing_ground_movement_for_owner(&owner)
    }
    fn is_doing_ground_movement_for_owner(&self, object: &Object) -> bool {
        if self
            .components
            .jet_ai
            .as_ref()
            .is_some_and(|jet| jet.is_doing_ground_movement())
        {
            return true;
        }
        if object.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned)
            && object.is_kind_of(crate::common::KindOf::ProducedAtHelipad)
        {
            return true;
        }
        if self.data.locomotor_set.get_valid_surfaces() == crate::ai::pathfind_complete::SURFACE_AIR
        {
            return false;
        }
        let Some(locomotor) = self.data.locomotor_set.get_active() else {
            return false;
        };
        if locomotor.get_legal_surfaces() & crate::ai::pathfind_complete::SURFACE_AIR != 0 {
            return false;
        }
        if object.is_disabled_by_type(crate::common::DisabledType::Held) {
            return false;
        }
        if object.is_above_terrain()
            && object.get_physics().is_some_and(|physics| {
                physics
                    .access()
                    .ok()
                    .is_some_and(|physics| physics.get_allow_to_fall())
            })
        {
            return false;
        }
        true
    }

    pub(super) fn update_goal_position(
        &mut self,
        goal: &Coord3D,
        layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String> {
        self.update_goal_position_impl(goal, layer, None)
    }

    pub(super) fn update_goal_position_with_pathfinder(
        &mut self,
        goal: &Coord3D,
        layer: crate::common::PathfindLayerEnum,
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<(), String> {
        self.update_goal_position_impl(goal, layer, Some(pathfinder))
    }

    fn update_goal_position_impl(
        &mut self,
        goal: &Coord3D,
        layer: crate::common::PathfindLayerEnum,
        mut driving_pathfinder: Option<&mut crate::ai::pathfind_complete::PathfindingSystem>,
    ) -> Result<(), String> {
        let is_ground_movement = self.is_doing_ground_movement();
        let owner_arc = self
            .native_owner()
            .ok_or_else(|| "unit owner no longer available".to_string())?;
        let mut owner = owner_arc
            .write()
            .map_err(|_| "unit owner lock poisoned".to_string())?;
        let owner_id = owner.get_id();
        let mut adjusted = *goal;
        let terrain_layer = match layer {
            crate::common::PathfindLayerEnum::Invalid => crate::path::PathfindLayerEnum::Invalid,
            crate::common::PathfindLayerEnum::Ground => crate::path::PathfindLayerEnum::Ground,
            crate::common::PathfindLayerEnum::Wall => crate::path::PathfindLayerEnum::Wall,
            crate::common::PathfindLayerEnum::Tunnel
            | crate::common::PathfindLayerEnum::Water
            | crate::common::PathfindLayerEnum::Air
            | crate::common::PathfindLayerEnum::Last => crate::path::PathfindLayerEnum::Ground,
            other => crate::path::PathfindLayerEnum::from_u32(other as u32),
        };
        let mut interacts_with_bridge_end = false;
        if let Ok(terrain) = crate::terrain::get_terrain_logic().read() {
            if layer == crate::common::PathfindLayerEnum::Wall {
                adjusted.z = crate::ai::the_ai()
                    .read()
                    .ok()
                    .map(|ai| ai.get_ai_data().wall_height)
                    .unwrap_or(adjusted.z);
            } else {
                adjusted.z =
                    terrain.get_layer_height(adjusted.x, adjusted.y, terrain_layer, None, true);
            }
            if layer != crate::common::PathfindLayerEnum::Ground {
                interacts_with_bridge_end =
                    terrain.object_interacts_with_bridge_layer(&owner, terrain_layer, true);
            }
            owner.set_destination_layer(layer);
        }
        if let Some(path) = self.data.current_path_snapshot.as_mut() {
            path.update_last_node(&adjusted);
        }
        if let Some(loco) = self.data.locomotor_set.get_active_mut() {
            if let Some(active_path) = loco.active_path.as_mut() {
                active_path.set_last_waypoint(adjusted);
            }
        }
        let is_immobile = owner.is_kind_of(KindOf::Immobile);
        let is_unmanned_heli = owner.is_kind_of(KindOf::ProducedAtHelipad)
            && owner.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned);
        let radius = owner.get_geometry_info().get_bounding_circle_radius();
        let mut diameter = 2.0 * radius;
        if diameter > PATHFIND_CELL_SIZE_F && diameter < 2.0 * PATHFIND_CELL_SIZE_F {
            diameter = 2.0 * PATHFIND_CELL_SIZE_F;
        }
        let mut radius_cells = (diameter / PATHFIND_CELL_SIZE_F + 0.3).floor() as i32;
        let mut center_in_cell = false;
        if radius_cells == 0 {
            radius_cells = 1;
        }
        if (radius_cells & 1) != 0 {
            center_in_cell = true;
        }
        radius_cells /= 2;
        if radius_cells > 2 {
            radius_cells = 2;
            center_in_cell = true;
        }
        drop(owner);
        if is_immobile {
            return Ok(());
        }
        let path_layer = ClassicPathLayer::from_u32(layer as u32);
        let new_cell = Self::compute_goal_cell(&adjusted, center_in_cell);
        if let Some(pf) = driving_pathfinder.as_deref_mut() {
            if !is_ground_movement && !is_unmanned_heli {
                self.update_aircraft_goal_cells_with_system(
                    pf,
                    owner_id,
                    new_cell,
                    radius_cells,
                    center_in_cell,
                );
            } else {
                self.update_ground_goal_cells_with_system(
                    pf,
                    owner_id,
                    new_cell,
                    path_layer,
                    radius_cells,
                    center_in_cell,
                    interacts_with_bridge_end,
                );
            }
        } else if let Ok(ai) = the_ai().read() {
            if let Some(pathfinder) = ai.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    if !is_ground_movement && !is_unmanned_heli {
                        self.update_aircraft_goal_cells(
                            &mut pf,
                            owner_id,
                            new_cell,
                            radius_cells,
                            center_in_cell,
                        );
                    } else {
                        self.update_ground_goal_cells(
                            &mut pf,
                            owner_id,
                            new_cell,
                            path_layer,
                            radius_cells,
                            center_in_cell,
                            interacts_with_bridge_end,
                        );
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn snap_closest_goal_position(
        &mut self,
        owner: &Object,
        goal: &mut Coord3D,
    ) -> bool {
        let Some(pathfinder) = the_ai().read().ok().and_then(|ai| ai.pathfinder()) else {
            return false;
        };
        let Some(pathfinder) = pathfinder.read().ok() else {
            return false;
        };
        pathfinder.snap_closest_goal_position(owner, &self.data.locomotor_set, goal);
        true
    }
    pub(super) fn adjust_destination(&mut self, goal: &mut Coord3D) -> bool {
        let Some(owner) = self.native_owner() else {
            return false;
        };
        let Ok(owner) = owner.read() else {
            return false;
        };
        if self.data.locomotor_set.get_active().is_none() {
            return false;
        }
        let ai = the_ai();
        let Some(pathfinder) = ai.read().ok().and_then(|ai| ai.pathfinder()) else {
            return false;
        };
        let Some(pathfinder) = pathfinder.read().ok() else {
            return false;
        };
        let mut candidate = *goal;
        if !pathfinder.adjust_destination(&owner, &self.data.locomotor_set, &mut candidate) {
            return false;
        }
        *goal = candidate;
        true
    }
    pub(super) fn set_adjusts_destination(&mut self, adjust: bool) {
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.path_adjusts_destination = adjust;
                if let Some(state) = guard.path_following_state.as_mut() {
                    state.adjusts_destination = adjust;
                }
            }
        }
    }
    pub(super) fn owner_object_id(&self) -> Option<ObjectID> {
        if self.unit_id != INVALID_ID {
            Some(self.unit_id)
        } else {
            None
        }
    }
    pub(super) fn wake_up_now(&self) {
        let now = TheGameLogic::get_frame();
        let object = match self.owner.as_ref() {
            Some(owner) => owner.upgrade(),
            None => crate::object::registry::OBJECT_REGISTRY.get_object(self.unit_id),
        };
        if let Some(object) = object {
            if let Ok(guard) = object.read() {
                guard.reschedule_ai_update(now.saturating_add(1));
            }
        }
    }
    pub(super) fn xfer_locomotor_set_state(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ AIUpdate.cpp:5130-5145 clears only the receiving set before load.
        if xfer.is_loading() {
            self.data.locomotor_set.clear();
        }
        let mut current_name = self.data.locomotor_set.active_name().map(str::to_owned);
        self.data
            .locomotor_set
            .xfer_self_and_cur_loco_ptr(xfer, &mut current_name)?;
        let mut current_set = self.data.current_locomotor_set as i32;
        xfer.xfer_int(&mut current_set).map_err(|e| e.to_string())?;
        if xfer.is_loading() {
            self.data.current_locomotor_set = locomotor_set_type_from_i32(current_set)?;
        }
        Ok(())
    }
    pub(super) fn friend_get_turret_sync(&self) -> TurretType {
        self.data.friend_get_turret_sync()
    }
    pub(super) fn friend_set_turret_sync(&mut self, turret: TurretType) {
        self.data.friend_set_turret_sync(turret)
    }
    pub(super) fn ensure_turret_machine(
        &mut self,
        turret: TurretType,
    ) -> Option<&mut TurretStateMachine> {
        match turret {
            TurretType::Primary => {
                if self.data.turret_primary_machine.is_none() {
                    self.data.turret_primary_machine =
                        self.build_turret_machine(TurretType::Primary);
                }
                self.data.turret_primary_machine.as_mut()
            }
            TurretType::Secondary => {
                if self.data.turret_secondary_machine.is_none() {
                    self.data.turret_secondary_machine =
                        self.build_turret_machine(TurretType::Secondary);
                }
                self.data.turret_secondary_machine.as_mut()
            }
            TurretType::Invalid => None,
        }
    }
    /// C++ `UnitAI::UnitAI` turret build (AIUpdate.cpp): create the `TurretAI`,
    /// apply `TurretAIData`, then construct `TurretStateMachine`, which defines
    /// the states and enters IDLE (TurretAI.cpp:248-298). The turret bundle is
    /// owned outright — no shared handle.
    pub(super) fn build_turret_machine(&self, turret: TurretType) -> Option<TurretStateMachine> {
        // C++ AIUpdate only constructs configured turrets. Clearing a target
        // on an absent turret is a no-op, not a new Idle state (and RNG draw).
        let data = match turret {
            TurretType::Primary => self.data.turret_primary_data.as_ref()?,
            TurretType::Secondary => self.data.turret_secondary_data.as_ref()?,
            TurretType::Invalid => return None,
        };
        let owner_id = match self.owner.as_ref() {
            Some(owner) => owner.upgrade()?.read().ok()?.get_id(),
            None => {
                let unit = get_unit_arc(self.unit_id)?;
                unit.read().ok()?.base_arc().read().ok()?.get_id()
            }
        };
        let mut turret_ai = TurretAI::new(owner_id);
        let slot = match turret {
            TurretType::Primary => WeaponSlotType::Primary,
            TurretType::Secondary => WeaponSlotType::Secondary,
            TurretType::Invalid => WeaponSlotType::Primary,
        };
        turret_ai.set_weapon_slot(slot);
        let mask = match slot {
            WeaponSlotType::Primary => 1u32 << 0,
            WeaponSlotType::Secondary => 1u32 << 1,
            WeaponSlotType::Tertiary => 1u32 << 2,
        };
        data.apply_to(&mut turret_ai);
        if data.turret_weapon_slots == 0 {
            error!("TurretAIData missing ControlledWeaponSlots; applying slot fallback.");
            turret_ai.set_turret_weapon_slots_mask(mask);
        }
        Some(TurretStateMachine::new(turret_ai))
    }
    /// Resolve the existing admitted owner; legacy Unit test fixtures retain
    /// their actual base Object, rather than constructing a second owner.
    pub(super) fn rappel_owner(&self) -> Option<Arc<RwLock<Object>>> {
        if let Some(owner) = self.owner.as_ref() {
            // Expired native identity cannot select another world's same ID.
            return owner.upgrade();
        }
        OBJECT_REGISTRY.get_object(self.unit_id).or_else(|| {
            get_unit_arc(self.unit_id).and_then(|unit| unit.read().ok().map(|unit| unit.base_arc()))
        })
    }

    pub(super) fn start_rappel_state(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        target_id: Option<ObjectID>,
    ) -> Result<(), String> {
        // C++ AIStates.cpp:481-514 — release the exact owner before callbacks
        // that can synchronously inspect or reschedule that Object.
        let physics = {
            let mut obj = owner.write().map_err(|_| "base object lock poisoned")?;
            if !obj.is_kind_of(KindOf::CanRappel) {
                return Err("unit cannot rappel".to_string());
            }
            obj.set_model_condition_state(ModelConditionFlags::RAPPELLING);
            obj.get_physics()
        };
        if let Some(physics) = physics {
            physics.reset_dynamic_physics();
        }

        // No owner guard spans the target query, including target == owner.
        let target = target_id.and_then(|id| {
            OBJECT_REGISTRY
                .with_object(id, |obj| {
                    (!obj.is_effectively_dead() && obj.is_kind_of(KindOf::Structure))
                        .then(|| (id, obj.get_geometry_info().get_max_height_above_position()))
                })
                .flatten()
        });
        let terrain = TheTerrainLogic::get().ok_or("terrain logic unavailable")?;
        let mut obj = owner.write().map_err(|_| "base object lock poisoned")?;
        let pos = *obj.get_position();
        let layer = terrain.get_highest_layer_for_destination(&pos);
        let mut dest_z = terrain.get_layer_height(pos.x, pos.y, layer);
        if let Some((_, height)) = target {
            dest_z += height;
        } else {
            obj.set_layer(layer);
            obj.set_destination_layer(layer);
        }
        let max_rappel_rate = GRAVITY.abs() * (LOGICFRAMES_PER_SECOND as Real) * 2.5;
        self.data.rappel_state = Some(RappelState {
            rappel_rate: -self.data.desired_speed.min(max_rappel_rate),
            dest_z,
            target_is_bldg: target.is_some(),
            target_id: target.map(|(id, _)| id),
        });
        Ok(())
    }

    pub(super) fn finish_rappel_state(&mut self) {
        if let Some(owner) = self.rappel_owner() {
            if let Ok(mut obj) = owner.write() {
                obj.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
            }
        }
        self.data.desired_speed = FAST_AS_POSSIBLE;
        self.data.rappel_state = None;
        if self.data.current_command == Some(crate::ai::AiCommandType::RappelInto) {
            self.data.current_command = None;
        }
    }

    pub(super) fn idle_blocked_by_specialized_ai(&self) -> bool {
        self.components
            .jet_ai
            .as_ref()
            .is_some_and(|ai| ai.should_block_idle(self.data.pending_command))
            || self
                .components
                .hack_internet_ai
                .as_ref()
                .is_some_and(|ai| ai.has_pending_command())
    }
    pub(super) fn remove_stored_pathfinder_goal(&mut self) {
        let (owner_id, radius, center_in_cell) = match self.owner.as_ref() {
            Some(owner) => {
                let Some(owner) = owner.upgrade() else {
                    return;
                };
                let Ok(owner) = owner.read() else {
                    return;
                };
                let radius = owner.get_geometry_info().get_bounding_circle_radius();
                let (radius, center_in_cell) =
                    Self::compute_pathfind_radius_and_center_from_radius(radius);
                (owner.get_id(), radius, center_in_cell)
            }
            None => {
                let Some(unit) = get_unit_arc(self.unit_id) else {
                    return;
                };
                let Ok(guard) = unit.read() else {
                    return;
                };
                let Some(base) = guard.get_base_object() else {
                    return;
                };
                let owner_id = base
                    .read()
                    .ok()
                    .map(|obj| obj.get_id())
                    .unwrap_or(INVALID_ID);
                let (radius, center_in_cell) = Self::compute_pathfind_radius_and_center(&guard);
                (owner_id, radius, center_in_cell)
            }
        };
        let ai_store = the_ai();
        let Ok(ai_lock) = ai_store.read() else {
            return;
        };
        let Some(pathfinder) = ai_lock.pathfinder() else {
            return;
        };
        let Ok(mut pf_guard) = pathfinder.write() else {
            return;
        };
        self.remove_goal_cells(&mut pf_guard, owner_id, radius, center_in_cell);
    }
    /// An in-state caller supplies the current State's real virtual classifier.
    /// External callers obtain it from the machine. No classification is cached.
    pub(super) fn get_next_mood_target_for_state(
        &mut self,
        called_by_ai: bool,
        called_during_idle: bool,
        is_attacking: Option<bool>,
        parent_is_attacking: &mut dyn FnMut() -> bool,
    ) -> ObjectID {
        if self.owner.is_none() {
            return self.get_legacy_mood_target(called_by_ai, called_during_idle);
        }
        let Some(owner) = self.owner.as_ref().and_then(Weak::upgrade) else {
            return INVALID_ID;
        };
        let Ok(source) = owner.read() else {
            return INVALID_ID;
        };
        if source.is_effectively_dead() || source.test_status(ObjectStatusTypes::IsUsingAbility) {
            return INVALID_ID;
        }
        let mask = self.data.auto_acquire_enemies_when_idle;
        if called_during_idle && mask & AUTO_ACQUIRE_IDLE == 0 {
            return INVALID_ID;
        }
        if mask & AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING != 0
            && is_attacking.unwrap_or_else(parent_is_attacking)
        {
            return INVALID_ID;
        }
        if called_during_idle && source.test_status(ObjectStatusTypes::Stealthed) {
            if !self.can_auto_acquire_while_stealthed_for_source(&source) {
                let passenger_may_fire = source
                    .get_contained_by()
                    .and_then(|id| {
                        crate::object::registry::OBJECT_REGISTRY.with_object(id, |container| {
                            container.get_contain().is_some_and(|contain| {
                                contain.lock().ok().is_some_and(|contain| {
                                    contain.is_passenger_allowed_to_fire(None)
                                })
                            })
                        })
                    })
                    .unwrap_or(false);
                if !passenger_may_fire {
                    return INVALID_ID;
                }
            }
        }
        let now = TheGameLogic::get_frame();
        if called_by_ai {
            let common_target = source
                .get_team()
                .and_then(|team| {
                    let team = team.read().ok()?;
                    team.attack_common_target()
                        .then(|| team.get_team_target_object())
                })
                .unwrap_or(INVALID_ID);
            if common_target != INVALID_ID && common_target != source.get_id() {
                let can_attack = crate::object::registry::OBJECT_REGISTRY
                    .with_object(common_target, |target| {
                        matches!(
                            source.get_able_to_attack_specific_object_for_objects(
                                crate::attack::AbleToAttackType::NewTarget,
                                target,
                                CommandSourceType::FromAi,
                            ),
                            crate::attack::CanAttackResult::Possible
                                | crate::attack::CanAttackResult::PossibleAfterMoving
                        )
                    })
                    .unwrap_or(false);
                if can_attack
                    && matches!(
                        self.data.attitude,
                        AIAttitudeType::Normal
                            | AIAttitudeType::Defensive
                            | AIAttitudeType::Aggressive
                    )
                {
                    return common_target;
                }
            }
            if now < self.data.next_mood_check_time {
                return INVALID_ID;
            }
            let rate = self.data.mood_attack_check_rate_frames as i32;
            self.data.next_mood_check_time = now.wrapping_add(rate as u32);
            if self.data.randomly_offset_mood_check {
                let half_rate = rate >> 1;
                let offset = game_engine::common::random_value::get_game_logic_random_value(
                    -half_rate, half_rate,
                );
                self.data.next_mood_check_time =
                    self.data.next_mood_check_time.wrapping_add(offset as u32);
                self.data.randomly_offset_mood_check = false;
            }
        }
        let ai_store = the_ai();
        let Ok(ai) = ai_store.read() else {
            return INVALID_ID;
        };
        let mut range = ai.get_adjusted_vision_range_for_source(
            &source,
            vision_factors::OWNER_TYPE | vision_factors::MOOD,
            Some(self.data.attitude),
        );
        if range <= 0.0 {
            return INVALID_ID;
        }
        if let Some(container_id) = source.get_contained_by() {
            if let Some(radius) = crate::object::registry::OBJECT_REGISTRY
                .with_object(container_id, |container| {
                    container.get_geometry_info().get_bounding_circle_radius()
                })
            {
                range += radius;
            }
        }
        let controller_is_human = source.with_controlling_player(|player| {
            player.get_player_type() == crate::player::PlayerType::Human
        });
        let human = controller_is_human == Some(true);
        if controller_is_human == Some(false) && self.data.attitude == AIAttitudeType::Passive {
            if source.get_body_module().is_none() {
                return INVALID_ID;
            }
            let Some(damage) = source.get_last_damage_info() else {
                return INVALID_ID;
            };
            if damage.input.damage_type != crate::damage::DamageType::Healing {
                return crate::object::registry::OBJECT_REGISTRY
                    .get_object(damage.input.source_id)
                    .map(|_| damage.input.source_id)
                    .unwrap_or(INVALID_ID);
            }
        }
        let rules = ai.get_ai_data();
        let mut qualifiers = search_qualifiers::CAN_ATTACK;
        if rules.attack_uses_line_of_sight && source.is_kind_of(KindOf::AttackNeedsLineOfSight) {
            qualifiers |= search_qualifiers::CAN_SEE;
        }
        if rules.attack_ignore_insignificant_buildings {
            qualifiers |= search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS;
        }
        if mask & AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS != 0 {
            qualifiers |= search_qualifiers::ATTACK_BUILDINGS;
        }
        if called_by_ai && human {
            qualifiers |= search_qualifiers::WITHIN_ATTACK_RANGE | search_qualifiers::UNFOGGED;
        }
        let priorities = ai.attack_priority_info_for_source(&source);
        ai.find_closest_enemy_for_source(&source, range, qualifiers, priorities.as_ref(), None)
            .ok()
            .flatten()
            .unwrap_or(INVALID_ID)
    }
    pub(super) fn get_legacy_mood_target(
        &mut self,
        use_existing_target: bool,
        _ignore_attacked: bool,
    ) -> ObjectID {
        // Wave 258: empty dual-world → invalid id.

        if dual_world_registry_unavailable() {
            return INVALID_ID;
        }

        let Some(unit) = get_unit_arc(self.unit_id) else {
            return INVALID_ID;
        };
        let Ok(guard) = unit.read() else {
            return INVALID_ID;
        };
        if !guard.can_auto_acquire_now() {
            return INVALID_ID;
        }

        let max_range = guard.engagement_range;
        if use_existing_target {
            if let Some(existing_id) = self.get_current_victim() {
                if let Some(existing_arc) =
                    crate::object::registry::OBJECT_REGISTRY.get_object(existing_id)
                {
                    if let Ok(existing_guard) = existing_arc.read() {
                        let relationship = guard
                            .base_arc()
                            .read()
                            .ok()
                            .map(|base| base.relationship_to(&existing_guard))
                            .unwrap_or(Relationship::Neutral);
                        if relationship == Relationship::Enemies {
                            let target_pos = *existing_guard.get_position();
                            let self_pos = guard.get_position();
                            let dx = target_pos.x - self_pos.x;
                            let dy = target_pos.y - self_pos.y;
                            let dist = (dx * dx + dy * dy).sqrt();
                            if dist <= max_range && guard.can_detect_target(&existing_guard, dist) {
                                return existing_id;
                            }
                        }
                    }
                }
            }
        }

        let ai_store = the_ai();
        let Ok(ai) = ai_store.read() else {
            return INVALID_ID;
        };
        let ai_data = ai.get_ai_data();

        let mut qualifiers = search_qualifiers::CAN_ATTACK;
        if ai_data.attack_uses_line_of_sight {
            qualifiers |= search_qualifiers::CAN_SEE;
        }
        if ai_data.attack_ignore_insignificant_buildings {
            qualifiers |= search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS;
        }
        if guard.auto_acquire_attack_buildings {
            qualifiers |= search_qualifiers::ATTACK_BUILDINGS;
        }

        ai.find_closest_enemy(guard.get_id(), max_range, qualifiers, None, None)
            .ok()
            .flatten()
            .unwrap_or(INVALID_ID)
    }

    pub(super) fn can_auto_acquire_while_stealthed_for_source(
        &self,
        source: &crate::object::Object,
    ) -> bool {
        // AIUpdate.cpp:4459 checks the real stealth module before the mask.
        source.get_stealth().is_some_and(|handle| {
            handle
                .lock()
                .ok()
                .is_some_and(|stealth| stealth.is_granted_by_special_power())
        }) || self.data.auto_acquire_enemies_when_idle & AUTO_ACQUIRE_IDLE_STEALTHED != 0
    }

    pub(super) fn set_final_position(&mut self, position: &Coord3D) {
        self.data.final_position = *position;
        self.data.do_final_position = false;
    }
    pub(super) fn installed_path_last_layer(&self) -> Option<u8> {
        self.data.installed_path_last_layer()
    }
    pub(super) fn get_retry_path(&self) -> bool {
        self.data.get_retry_path()
    }
    pub(super) fn remove_pathfinder_goal(&mut self) {
        self.remove_stored_pathfinder_goal();
    }
    pub(super) fn set_locomotor_goal_position_on_path(&mut self) {
        self.data.set_locomotor_goal_position_on_path();
    }
    pub(super) fn get_path(&self) -> Option<()> {
        self.get_path_destination().map(|_| ())
    }
    pub(super) fn is_idle_with_parent_state(&self, parent_is_idle: bool) -> bool {
        !self.idle_blocked_by_specialized_ai() && parent_is_idle
    }
    pub(super) fn get_next_mood_target_with_attack_state(
        &mut self,
        called_by_ai: bool,
        called_during_idle: bool,
        is_attacking: bool,
    ) -> Option<Arc<RwLock<Object>>> {
        let mut no_parent_query = || false;
        let id = self.get_next_mood_target_for_state(
            called_by_ai,
            called_during_idle,
            Some(is_attacking),
            &mut no_parent_query,
        );
        if id == INVALID_ID {
            None
        } else {
            TheGameLogic::find_object_by_id(id)
        }
    }
}

pub(super) fn is_attacking_for_runtime(
    machine: Option<&AIStateMachine>,
    owner_exists: bool,
    unit_id: ObjectID,
) -> bool {
    if let Some(machine) = machine {
        let attacking = machine.is_in_attack_state();
        if owner_exists || attacking {
            return attacking;
        }
    }
    if owner_exists {
        return false;
    }
    let Some(unit) = get_unit_arc(unit_id) else {
        return false;
    };
    let Ok(guard) = unit.read() else {
        return false;
    };
    guard
        .base_arc()
        .read()
        .ok()
        .map(|obj| obj.test_status(ObjectStatusTypes::OBJECT_STATUS_IS_ATTACKING))
        .unwrap_or(false)
        || matches!(
            guard.current_order,
            Some(UnitOrder::Attack { .. }) | Some(UnitOrder::AttackMove { .. })
        )
        || guard.movement_state == MovementState::Attacking
}

impl<'a> UnitAiStateRuntime<'a> {
    pub(crate) fn owner(&self) -> Option<Arc<RwLock<Object>>> {
        self.runtime.owner.as_ref().and_then(Weak::upgrade)
    }
    pub(crate) fn new(runtime: &'a mut UnitAiRuntime, parent_exists: bool) -> Self {
        Self {
            runtime,
            parent_exists,
        }
    }
}
impl AiStateRuntime for UnitAiStateRuntime<'_> {
    fn dispatch_command_with_driver(
        &mut self,
        params: &crate::ai::AiCommandParams,
        driver: &mut crate::ai::states::AIStateMachineDriver<'_>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.execute_command_native(params, driver)
    }

    fn get_supply_truck_ai_interface(&self) -> Option<&dyn crate::modules::SupplyTruckAIInterface> {
        self.runtime.get_supply_truck_ai_interface()
    }
    fn get_supply_truck_ai_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::SupplyTruckAIInterface> {
        self.runtime.get_supply_truck_ai_interface_mut()
    }
    fn set_final_position(&mut self, _position: &Coord3D) {
        self.runtime.set_final_position(_position)
    }
    fn is_idle_with_parent_state(&self, parent_is_idle: bool) -> bool {
        self.runtime.is_idle_with_parent_state(parent_is_idle)
    }
    fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String> {
        self.runtime.set_movement_target(target)
    }
    fn get_preferred_height(&self) -> Option<Real> {
        self.runtime.get_preferred_height()
    }
    fn with_cur_locomotor(&self, _f: &mut dyn FnMut(&crate::locomotor::Locomotor)) {
        self.runtime.with_cur_locomotor(_f)
    }
    fn with_cur_locomotor_mut(&mut self, _f: &mut dyn FnMut(&mut crate::locomotor::Locomotor)) {
        self.runtime.with_cur_locomotor_mut(_f)
    }
    fn get_path(&self) -> Option<()> {
        self.runtime.get_path()
    }
    fn get_path_destination(&self) -> Option<Coord3D> {
        self.runtime.get_path_destination()
    }
    fn get_path_last_node(&self) -> Option<Coord3D> {
        self.runtime.get_path_last_node()
    }
    fn installed_path_last_layer(&self) -> Option<u8> {
        self.runtime.installed_path_last_layer()
    }
    fn get_retry_path(&self) -> bool {
        self.runtime.get_retry_path()
    }
    fn get_locomotor_distance_to_goal(&mut self, fallback_goal: Option<Coord3D>) -> Real {
        self.runtime.get_locomotor_distance_to_goal(fallback_goal)
    }
    fn get_last_command_source(&self) -> crate::ai::CommandSourceType {
        self.runtime.get_last_command_source()
    }
    fn set_last_command_source(&mut self, _source: crate::ai::CommandSourceType) {
        self.runtime.set_last_command_source(_source)
    }
    fn set_locomotor_goal_none(&mut self) {
        self.runtime.set_locomotor_goal_none()
    }
    fn remove_pathfinder_goal(&mut self) {
        self.runtime.remove_pathfinder_goal()
    }
    fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        self.runtime.set_locomotor_goal_orientation(angle)
    }
    fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        self.runtime.set_locomotor_goal_position_explicit(pos)
    }
    fn set_locomotor_goal_position_on_path(&mut self) {
        self.runtime.set_locomotor_goal_position_on_path()
    }
    fn friend_ending_move(&mut self) {
        self.runtime.friend_ending_move()
    }
    fn friend_starting_move(&mut self) {
        self.runtime.friend_starting_move()
    }
    fn is_allowed_to_adjust_destination(&self) -> bool {
        self.runtime.is_allowed_to_adjust_destination()
    }
    fn get_desired_speed(&self) -> Real {
        self.runtime.get_desired_speed()
    }
    fn set_desired_speed(&mut self, speed: Real) {
        self.runtime.set_desired_speed(speed)
    }
    fn is_in_rappel_state(&self) -> bool {
        self.runtime.is_in_rappel_state()
    }
    fn is_doing_combat_drop(&self) -> bool {
        self.runtime.is_doing_combat_drop()
    }
    fn set_queue_for_path_time(&mut self, _frames: UnsignedInt) {
        self.runtime.set_queue_for_path_time(_frames)
    }
    fn is_temporarily_preventing_aim_success(&self) -> bool {
        self.runtime.is_temporarily_preventing_aim_success()
    }
    fn add_targeter(&mut self, _id: ObjectID, _add: bool) {
        self.runtime.add_targeter(_id, _add)
    }
    fn clear_guard_target_type(&mut self) {
        self.runtime.clear_guard_target_type()
    }
    fn set_turret_target_object(
        &mut self,
        _turret: TurretType,
        _target_id: Option<ObjectID>,
        _force_attacking: bool,
    ) {
        self.runtime
            .set_turret_target_object(_turret, _target_id, _force_attacking)
    }
    fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        _slot: crate::weapon::WeaponSlotType,
        _target: crate::common::ObjectID,
    ) -> bool {
        self.runtime
            .is_weapon_slot_on_turret_and_aiming_at_target(_slot, _target)
    }
    fn ignore_obstacle(
        &mut self,
        _obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.ignore_obstacle(_obj_id)
    }
    fn ignore_obstacle_id(
        &mut self,
        id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.ignore_obstacle_id(id)
    }
    fn set_current_goal_path_index(
        &mut self,
        _index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.set_current_goal_path_index(_index)
    }
    fn get_current_goal_path_index(&self) -> i32 {
        self.runtime.get_current_goal_path_index()
    }
    fn set_can_path_through_units(
        &mut self,
        _value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.set_can_path_through_units(_value)
    }
    fn get_can_path_through_units(&self) -> bool {
        self.runtime.get_can_path_through_units()
    }
    fn is_blocked_and_stuck(&self) -> bool {
        self.runtime.is_blocked_and_stuck()
    }
    fn get_num_frames_blocked(&self) -> u32 {
        self.runtime.get_num_frames_blocked()
    }
    fn destroy_path(&mut self) {
        self.runtime.destroy_path()
    }
    fn clear_move_out_of_way(&mut self) {
        self.runtime.clear_move_out_of_way()
    }
    fn request_path(&mut self, _destination: &Coord3D, _is_final_goal: bool) -> Result<(), String> {
        self.runtime.request_path(_destination, _is_final_goal)
    }
    fn can_compute_quick_path(&self) -> bool {
        self.runtime.can_compute_quick_path()
    }
    fn get_crate_id(&self) -> ObjectID {
        self.runtime.get_crate_id()
    }
    fn get_current_victim(&self) -> Option<ObjectID> {
        self.runtime.get_current_victim()
    }
    fn set_current_victim(&mut self, _victim: Option<ObjectID>) {
        self.runtime.set_current_victim(_victim)
    }
    fn check_for_crate_to_pickup_id(&mut self) -> ObjectID {
        self.runtime.check_for_crate_to_pickup_id()
    }
    fn get_next_mood_target_with_attack_state(
        &mut self,
        called_by_ai: bool,
        called_during_idle: bool,
        _is_attacking: bool,
    ) -> Option<Arc<RwLock<Object>>> {
        self.runtime.get_next_mood_target_with_attack_state(
            called_by_ai,
            called_during_idle,
            _is_attacking,
        )
    }
    fn get_next_mood_check_time(&self) -> u32 {
        self.runtime.get_next_mood_check_time()
    }
    fn reset_next_mood_check_time(&mut self) {
        self.runtime.reset_next_mood_check_time()
    }
    fn get_mood_matrix_value(&self) -> u32 {
        self.runtime.get_mood_matrix_value(self.parent_exists)
    }
    fn get_mood_matrix_action_adjustment(&mut self, _action: crate::ai::MoodMatrixAction) -> u32 {
        self.runtime
            .get_mood_matrix_action_adjustment(self.parent_exists, _action)
    }
    fn set_original_victim_pos(&mut self, _pos: Option<Coord3D>) {
        self.runtime.set_original_victim_pos(_pos)
    }
    fn set_prior_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId) {
        self.runtime.set_prior_waypoint_id(_waypoint_id)
    }
    fn set_current_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId) {
        self.runtime.set_current_waypoint_id(_waypoint_id)
    }
    fn set_completed_waypoint_id(&mut self, _waypoint_id: Option<crate::waypoint::WaypointId>) {
        self.runtime.set_completed_waypoint_id(_waypoint_id)
    }
    fn choose_locomotor_set(
        &mut self,
        _set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.choose_locomotor_set(_set)
    }
    fn set_allow_invalid_position(
        &mut self,
        _allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.set_allow_invalid_position(_allow)
    }
    fn set_precise_z_pos(
        &mut self,
        _precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.set_precise_z_pos(_precise)
    }
    fn get_speed(&self) -> f32 {
        self.runtime.get_speed()
    }
    fn set_path_extra_distance(
        &mut self,
        _distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.runtime.set_path_extra_distance(_distance)
    }
    fn set_path_from_waypoint(
        &mut self,
        _waypoint: &crate::waypoint::Waypoint,
        _group_offset: &Coord2D,
    ) -> Result<(), String> {
        self.runtime
            .set_path_from_waypoint(_waypoint, _group_offset)
    }
    fn is_waiting_for_path(&self) -> bool {
        self.runtime.is_waiting_for_path()
    }
    fn append_goal_position_to_path(&mut self, _goal: &Coord3D) -> Result<(), String> {
        self.runtime.append_goal_position_to_path(_goal)
    }
    fn request_safe_path(&mut self, _repulsor_id: ObjectID) -> Result<bool, String> {
        self.runtime.request_safe_path(_repulsor_id)
    }
    fn is_doing_ground_movement(&self) -> bool {
        self.runtime.is_doing_ground_movement()
    }
    fn update_goal_position(
        &mut self,
        _goal: &Coord3D,
        _layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String> {
        self.runtime.update_goal_position(_goal, _layer)
    }
    fn adjust_destination(&mut self, _goal: &mut Coord3D) -> bool {
        self.runtime.adjust_destination(_goal)
    }
    fn snap_closest_goal_position(
        &mut self,
        owner: &crate::object::Object,
        goal: &mut Coord3D,
    ) -> bool {
        self.runtime.snap_closest_goal_position(owner, goal)
    }
    fn set_adjusts_destination(&mut self, adjust: bool) {
        self.runtime.set_adjusts_destination(adjust)
    }
    fn should_adjust_destination(&self, state_adjusts: bool) -> bool {
        if !state_adjusts {
            return false;
        }
        let Some(owner) = self.runtime.native_owner() else {
            return false;
        };
        let Ok(owner) = owner.read() else {
            return false;
        };
        !owner.test_status(ObjectStatusTypes::Parachuting)
            && self.runtime.is_allowed_to_adjust_destination()
    }
}

impl UnitAiRuntime {
    pub(super) fn append_current_path_snapshot_goal(&mut self, goal: &Coord3D) {
        match self.data.current_path_snapshot.as_mut() {
            Some(path) => {
                path.append_node(goal, AiPathLayer::Ground);
                if !self.data.installed_path_layers.is_empty() {
                    self.data.installed_path_layers.push(1);
                }
            }
            None => self.set_current_path_snapshot_from_coords(&[*goal]),
        }
    }
    pub(super) fn compute_quick_path(&mut self, destination: &Coord3D) -> bool {
        if !self.can_compute_quick_path() {
            return false;
        }
        if let Some(path) = self.data.current_path_snapshot.as_ref() {
            if let Some(last) = path.get_last_node_position() {
                let dx = destination.x - last.x;
                let dy = destination.y - last.y;
                let dz = destination.z - self.data.requested_destination.z;
                if dx * dx + dy * dy + dz * dz < 0.25 {
                    return true;
                }
            }
        }
        self.install_direct_path_from_current_position(destination)
    }
    pub(super) fn has_valid_locomotor_surfaces(&self) -> bool {
        self.data.has_valid_locomotor_surfaces()
    }
    pub(super) fn queue_path_request_now(&self, destination: Coord3D) -> Result<(), String> {
        let request = self.build_classic_path_request(destination, false)?;

        let ai_store = the_ai();
        if let Some(ai) = ai_store.read().ok() {
            if let Some(pathfinder) = ai.pathfinder() {
                pathfinder
                    .write()
                    .map_err(|_| "pathfinder lock poisoned".to_string())?
                    .queue_for_path_request(request)
                    .map_err(|err| err.to_string())?;
            }
        }

        Ok(())
    }
    pub(super) fn remove_goal_cells_with_system(
        &mut self,
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
        unit_id: ObjectID,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.data
            .remove_goal_cells_with_system(pathfinder, unit_id, radius, center_in_cell)
    }
    pub(super) fn remove_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.data
            .remove_goal_cells(pathfinder, unit_id, radius, center_in_cell)
    }
    pub(super) fn set_current_path_snapshot_from_coords(&mut self, path: &[Coord3D]) {
        self.data.set_current_path_snapshot_from_coords(path)
    }
    pub(super) fn set_ignore_collision_time(&mut self, duration_frames: UnsignedInt) {
        self.data.ignore_collisions_until =
            TheGameLogic::get_frame().saturating_add(duration_frames);
    }
    pub(super) fn set_path_from_coords(&mut self, path: &[Coord3D]) -> Result<(), String> {
        let installed_path = self.path_with_cpp_final_node(path)?;
        let last = *installed_path
            .last()
            .ok_or_else(|| "set_path_from_coords missing path points".to_string())?;
        self.data.blocked_frames = 0;
        self.data.blocked_and_stuck = false;
        self.data.queue_for_path_frame = 0;
        self.data.path_timestamp = TheGameLogic::get_frame();
        self.data.movement_complete = false;
        self.data.locomotor_goal_type = 1;
        self.data.locomotor_goal_data = Coord3D::ZERO;
        if let Some(loco) = self.data.locomotor_set.get_active_mut() {
            loco.clear_path();
        }
        self.set_current_path_snapshot_from_coords(&installed_path);
        if self.data.is_final_goal && self.is_doing_ground_movement() {
            let layer = TheTerrainLogic::get()
                .map(|terrain| terrain.get_layer_for_destination(&last))
                .unwrap_or(crate::common::PathfindLayerEnum::Ground);
            self.update_goal_position(&last, layer)?;
        }
        Ok(())
    }
    pub(super) fn set_path_from_coords_with_pathfinder(
        &mut self,
        path: &[Coord3D],
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<(), String> {
        let installed_path = self.path_with_cpp_final_node(path)?;
        let last = *installed_path
            .last()
            .ok_or_else(|| "set_path_from_coords missing path points".to_string())?;
        self.data.blocked_frames = 0;
        self.data.blocked_and_stuck = false;
        self.data.queue_for_path_frame = 0;
        self.data.path_timestamp = TheGameLogic::get_frame();
        self.data.movement_complete = false;
        self.data.locomotor_goal_type = 1;
        self.data.locomotor_goal_data = Coord3D::ZERO;
        if let Some(loco) = self.data.locomotor_set.get_active_mut() {
            loco.clear_path();
        }
        self.set_current_path_snapshot_from_coords(&installed_path);
        if self.data.is_final_goal && self.is_doing_ground_movement() {
            let layer = TheTerrainLogic::get()
                .map(|terrain| terrain.get_layer_for_destination(&last))
                .unwrap_or(crate::common::PathfindLayerEnum::Ground);
            self.update_goal_position_with_pathfinder(&last, layer, pathfinder)?;
        }
        Ok(())
    }
    pub(super) fn update_aircraft_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        new_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.remove_goal_cells(pathfinder, unit_id, radius, center_in_cell);

        if !self.data.is_aircraft_that_adjusts_destination() {
            return;
        }

        self.data.pathfind_goal_cell = new_cell;
        self.data.pathfind_goal_layer = ClassicPathLayer::Ground;

        pathfinder.set_aircraft_goal_cells(unit_id, new_cell, radius, center_in_cell);
    }
    pub(super) fn update_aircraft_goal_cells_with_system(
        &mut self,
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
        unit_id: ObjectID,
        new_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.data
            .remove_goal_cells_with_system(pathfinder, unit_id, radius, center_in_cell);
        if !self.data.is_aircraft_that_adjusts_destination() {
            return;
        }
        self.data.pathfind_goal_cell = new_cell;
        self.data.pathfind_goal_layer = ClassicPathLayer::Ground;
        pathfinder.set_aircraft_goal_cells(unit_id, new_cell, radius, center_in_cell);
    }
    pub(super) fn update_ground_goal_cells_with_system(
        &mut self,
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
        unit_id: ObjectID,
        new_cell: ICoord2D,
        layer: ClassicPathLayer,
        radius: i32,
        center_in_cell: bool,
        interacts_with_bridge_end: bool,
    ) {
        let layer_changed = self.data.pathfind_goal_layer != layer;
        if !layer_changed
            && self.data.pathfind_goal_cell.x == new_cell.x
            && self.data.pathfind_goal_cell.y == new_cell.y
        {
            return;
        }
        self.data
            .remove_goal_cells_with_system(pathfinder, unit_id, radius, center_in_cell);
        self.data.pathfind_goal_cell = new_cell;
        self.data.pathfind_goal_layer = layer;
        let mut do_ground = layer == ClassicPathLayer::Ground;
        let do_layer = layer != ClassicPathLayer::Ground;
        if do_layer && interacts_with_bridge_end {
            do_ground = true;
        }
        pathfinder.set_goal_cells(
            unit_id,
            new_cell,
            radius,
            center_in_cell,
            layer,
            do_ground,
            do_layer,
        );
    }
    pub(super) fn update_ground_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        new_cell: ICoord2D,
        layer: ClassicPathLayer,
        radius: i32,
        center_in_cell: bool,
        interacts_with_bridge_end: bool,
    ) {
        let layer_changed = self.data.pathfind_goal_layer != layer;
        if !layer_changed
            && self.data.pathfind_goal_cell.x == new_cell.x
            && self.data.pathfind_goal_cell.y == new_cell.y
        {
            return;
        }

        self.remove_goal_cells(pathfinder, unit_id, radius, center_in_cell);

        self.data.pathfind_goal_cell = new_cell;
        self.data.pathfind_goal_layer = layer;

        let mut do_ground = layer == ClassicPathLayer::Ground;
        let do_layer = layer != ClassicPathLayer::Ground;
        if do_layer && interacts_with_bridge_end {
            do_ground = true;
        }

        pathfinder.set_goal_cells(
            unit_id,
            new_cell,
            radius,
            center_in_cell,
            layer,
            do_ground,
            do_layer,
        );
    }
    pub(super) fn install_direct_path_from_current_position(
        &mut self,
        destination: &Coord3D,
    ) -> bool {
        let Some(owner) = self.native_owner() else {
            return false;
        };
        let Ok(owner) = owner.read() else {
            return false;
        };
        let mut start = *owner.get_position();
        start.z = destination.z;
        drop(owner);
        self.data.blocked_frames = 0;
        self.data.blocked_and_stuck = false;
        self.data.waiting_for_path = false;
        self.data.queue_for_path_frame = 0;
        self.data.path_timestamp = TheGameLogic::get_frame();
        self.data.movement_complete = false;
        self.data.requested_destination = *destination;
        self.data.locomotor_goal_type = 1;
        self.data.locomotor_goal_data = Coord3D::ZERO;
        self.set_current_path_snapshot_from_coords(&[start, *destination]);
        true
    }
    pub(super) fn build_classic_path_request(
        &self,
        destination: Coord3D,
        allow_partial: bool,
    ) -> Result<crate::ai::pathfind_complete::PathRequest, String> {
        let owner = self
            .native_owner()
            .ok_or_else(|| "unit owner no longer available".to_string())?;
        let object = owner
            .read()
            .map_err(|_| "unit base object lock poisoned".to_string())?;
        let surfaces = self
            .data
            .locomotor_set
            .get_active()
            .map(|loco| loco.get_legal_surfaces())
            .unwrap_or(crate::locomotor::SURFACE_GROUND);
        Ok(crate::ai::pathfind_complete::PathRequest {
            object_id: object.get_id(),
            from: *object.get_position(),
            to: destination,
            surfaces,
            is_crusher: object.get_crusher_level() > 0,
            unit_radius: object.get_geometry_info().get_major_radius(),
            allow_partial,
            move_allies: self.data.can_path_through_units,
            ignore_obstacle_id: (self.data.ignore_obstacle_id != INVALID_ID)
                .then_some(self.data.ignore_obstacle_id),
            is_human: false,
        })
    }
    pub(super) fn path_with_cpp_final_node(
        &self,
        path: &[Coord3D],
    ) -> Result<Vec<Coord3D>, String> {
        if path.is_empty() {
            return Err("set_path_from_coords missing path points".to_string());
        }

        let mut installed_path = path.to_vec();
        if self.data.current_locomotor_is_ultra_accurate() {
            if let Some(last) = installed_path.last_mut() {
                *last = self.data.requested_destination;
            }
        }
        Ok(installed_path)
    }
    pub(super) fn compute_pathfind_radius_and_center(unit: &Unit) -> (i32, bool) {
        let radius = unit
            .base_arc()
            .read()
            .ok()
            .map(|obj| obj.get_geometry_info().get_bounding_circle_radius())
            .unwrap_or(PATHFIND_CELL_SIZE_F * 0.5);
        Self::compute_pathfind_radius_and_center_from_radius(radius)
    }
    fn compute_pathfind_radius_and_center_from_radius(radius: Real) -> (i32, bool) {
        let mut diameter = 2.0 * radius;
        if diameter > PATHFIND_CELL_SIZE_F && diameter < 2.0 * PATHFIND_CELL_SIZE_F {
            diameter = 2.0 * PATHFIND_CELL_SIZE_F;
        }

        let mut radius = (diameter / PATHFIND_CELL_SIZE_F + 0.3).floor() as i32;
        let mut center_in_cell = false;

        if radius == 0 {
            radius = 1;
        }
        if (radius & 1) != 0 {
            center_in_cell = true;
        }
        radius /= 2;
        if radius > 2 {
            radius = 2;
            center_in_cell = true;
        }

        (radius, center_in_cell)
    }
    pub(super) fn compute_goal_cell(pos: &Coord3D, center_in_cell: bool) -> ICoord2D {
        if center_in_cell {
            ICoord2D::new(
                (pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
            )
        } else {
            ICoord2D::new(
                (0.5 + pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (0.5 + pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
            )
        }
    }
}
