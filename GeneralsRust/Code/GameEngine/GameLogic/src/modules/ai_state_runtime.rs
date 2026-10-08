//! Runtime operations available while the parent machine is borrowed.
//! This capability owns no parent FSM or ambient active-world selection.
use super::*;

/// C++ `Pathfinder::snapClosestGoalPosition`, using the exact supplied owner
/// and the installed interface's own locomotor-set snapshot. Missing inputs
/// fail without changing the requested goal.
fn snap_closest_goal_position_with_interface(
    ai: &dyn AIUpdateInterface,
    owner: &crate::object::Object,
    goal: &mut Coord3D,
) -> bool {
    let Some(locomotor_set) = AIUpdateInterface::get_locomotor_set_clone(ai) else {
        return false;
    };
    let Some(pathfinder) = crate::ai::the_ai()
        .read()
        .ok()
        .and_then(|ai| ai.pathfinder())
    else {
        return false;
    };
    let Ok(pathfinder) = pathfinder.read() else {
        return false;
    };
    pathfinder.snap_closest_goal_position(owner, &locomotor_set, goal);
    true
}

pub(crate) trait AiStateRuntime {
    fn dispatch_command_with_driver(
        &mut self,
        params: &crate::ai::AiCommandParams,
        driver: &mut crate::ai::states::AIStateMachineDriver<'_>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn set_final_position(&mut self, _position: &Coord3D);
    fn is_idle_with_parent_state(&self, parent_is_idle: bool) -> bool;
    fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String>;
    fn get_preferred_height(&self) -> Option<Real>;
    fn with_cur_locomotor(&self, _f: &mut dyn FnMut(&crate::locomotor::Locomotor));
    fn with_cur_locomotor_mut(&mut self, _f: &mut dyn FnMut(&mut crate::locomotor::Locomotor));
    fn get_path(&self) -> Option<()>;
    fn get_path_destination(&self) -> Option<Coord3D>;
    fn get_path_last_node(&self) -> Option<Coord3D>;
    fn installed_path_last_layer(&self) -> Option<u8>;
    fn get_retry_path(&self) -> bool;
    fn get_locomotor_distance_to_goal(&mut self, fallback_goal: Option<Coord3D>) -> Real;
    fn get_last_command_source(&self) -> crate::ai::CommandSourceType;
    fn set_last_command_source(&mut self, _source: crate::ai::CommandSourceType);
    fn set_locomotor_goal_none(&mut self);
    fn remove_pathfinder_goal(&mut self);
    fn set_locomotor_goal_orientation(&mut self, angle: Real);
    fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D);
    fn set_locomotor_goal_position_on_path(&mut self);
    fn friend_ending_move(&mut self);
    fn friend_starting_move(&mut self);
    fn is_allowed_to_adjust_destination(&self) -> bool;
    fn get_desired_speed(&self) -> Real;
    fn set_desired_speed(&mut self, speed: Real);
    fn is_in_rappel_state(&self) -> bool;
    fn is_doing_combat_drop(&self) -> bool;
    fn set_queue_for_path_time(&mut self, _frames: UnsignedInt);
    fn is_temporarily_preventing_aim_success(&self) -> bool;
    fn add_targeter(&mut self, _id: ObjectID, _add: bool);
    fn clear_guard_target_type(&mut self);
    fn set_turret_target_object(
        &mut self,
        _turret: TurretType,
        _target_id: Option<ObjectID>,
        _force_attacking: bool,
    );
    fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        _slot: crate::weapon::WeaponSlotType,
        _target: crate::common::ObjectID,
    ) -> bool;
    fn get_supply_truck_ai_interface(&self) -> Option<&dyn SupplyTruckAIInterface>;
    fn get_supply_truck_ai_interface_mut(&mut self) -> Option<&mut dyn SupplyTruckAIInterface>;
    fn ignore_obstacle(
        &mut self,
        _obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn ignore_obstacle_id(
        &mut self,
        id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn set_current_goal_path_index(
        &mut self,
        _index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn get_current_goal_path_index(&self) -> i32;
    fn set_can_path_through_units(
        &mut self,
        _value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn get_can_path_through_units(&self) -> bool;
    fn is_blocked_and_stuck(&self) -> bool;
    fn get_num_frames_blocked(&self) -> u32;
    fn destroy_path(&mut self);
    fn clear_move_out_of_way(&mut self);
    fn request_path(&mut self, _destination: &Coord3D, _is_final_goal: bool) -> Result<(), String>;
    fn can_compute_quick_path(&self) -> bool;
    fn get_crate_id(&self) -> ObjectID;
    fn get_current_victim(&self) -> Option<ObjectID>;
    fn set_current_victim(&mut self, _victim: Option<ObjectID>);
    fn check_for_crate_to_pickup_id(&mut self) -> ObjectID;
    fn get_next_mood_target_with_attack_state(
        &mut self,
        called_by_ai: bool,
        called_during_idle: bool,
        _is_attacking: bool,
    ) -> Option<Arc<RwLock<Object>>>;
    fn get_next_mood_check_time(&self) -> u32;
    fn reset_next_mood_check_time(&mut self);
    fn get_mood_matrix_value(&self) -> u32;
    fn get_mood_matrix_action_adjustment(&mut self, _action: crate::ai::MoodMatrixAction) -> u32;
    fn set_original_victim_pos(&mut self, _pos: Option<Coord3D>);
    fn set_prior_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId);
    fn set_current_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId);
    fn set_completed_waypoint_id(&mut self, _waypoint_id: Option<crate::waypoint::WaypointId>);
    fn choose_locomotor_set(
        &mut self,
        _set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn set_allow_invalid_position(
        &mut self,
        _allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn set_precise_z_pos(
        &mut self,
        _precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn get_speed(&self) -> f32;
    fn set_path_extra_distance(
        &mut self,
        _distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn set_path_from_waypoint(
        &mut self,
        _waypoint: &crate::waypoint::Waypoint,
        _group_offset: &Coord2D,
    ) -> Result<(), String>;
    fn is_waiting_for_path(&self) -> bool;
    fn append_goal_position_to_path(&mut self, _goal: &Coord3D) -> Result<(), String>;
    fn request_safe_path(&mut self, _repulsor_id: ObjectID) -> Result<bool, String>;
    fn is_doing_ground_movement(&self) -> bool;
    fn update_goal_position(
        &mut self,
        _goal: &Coord3D,
        _layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String>;
    fn adjust_destination(&mut self, _goal: &mut Coord3D) -> bool;
    fn snap_closest_goal_position(
        &mut self,
        owner: &crate::object::Object,
        goal: &mut Coord3D,
    ) -> bool;
    fn set_adjusts_destination(&mut self, adjust: bool);
    fn should_adjust_destination(&self, state_adjusts: bool) -> bool;
}

/// Borrow the existing installed-interface contract at a standalone boundary.
pub(crate) struct AiUpdateRuntimeAdapter<'a>(pub(crate) &'a mut dyn AIUpdateInterface);

impl AiStateRuntime for AiUpdateRuntimeAdapter<'_> {
    fn dispatch_command_with_driver(
        &mut self,
        params: &crate::ai::AiCommandParams,
        _driver: &mut crate::ai::states::AIStateMachineDriver<'_>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::execute_command(self.0, params)
    }

    fn set_final_position(&mut self, _position: &Coord3D) {
        AIUpdateInterface::set_final_position(self.0, _position)
    }
    fn is_idle_with_parent_state(&self, parent_is_idle: bool) -> bool {
        AIUpdateInterface::is_idle_with_parent_state(self.0, parent_is_idle)
    }
    fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String> {
        AIUpdateInterface::set_movement_target(self.0, target)
    }
    fn get_preferred_height(&self) -> Option<Real> {
        AIUpdateInterface::get_preferred_height(self.0)
    }
    fn with_cur_locomotor(&self, _f: &mut dyn FnMut(&crate::locomotor::Locomotor)) {
        AIUpdateInterface::with_cur_locomotor(self.0, _f)
    }
    fn with_cur_locomotor_mut(&mut self, _f: &mut dyn FnMut(&mut crate::locomotor::Locomotor)) {
        AIUpdateInterface::with_cur_locomotor_mut(self.0, _f)
    }
    fn get_path(&self) -> Option<()> {
        AIUpdateInterface::get_path(self.0)
    }
    fn get_path_destination(&self) -> Option<Coord3D> {
        AIUpdateInterface::get_path_destination(self.0)
    }
    fn get_path_last_node(&self) -> Option<Coord3D> {
        AIUpdateInterface::get_path_last_node(self.0)
    }
    fn installed_path_last_layer(&self) -> Option<u8> {
        AIUpdateInterface::installed_path_last_layer(self.0)
    }
    fn get_retry_path(&self) -> bool {
        AIUpdateInterface::get_retry_path(self.0)
    }
    fn get_locomotor_distance_to_goal(&mut self, _fallback_goal: Option<Coord3D>) -> Real {
        AIUpdateInterface::get_locomotor_distance_to_goal(self.0)
    }
    fn get_last_command_source(&self) -> crate::ai::CommandSourceType {
        AIUpdateInterface::get_last_command_source(self.0)
    }
    fn set_last_command_source(&mut self, _source: crate::ai::CommandSourceType) {
        AIUpdateInterface::set_last_command_source(self.0, _source)
    }
    fn set_locomotor_goal_none(&mut self) {
        AIUpdateInterface::set_locomotor_goal_none(self.0)
    }
    fn remove_pathfinder_goal(&mut self) {
        AIUpdateInterface::remove_pathfinder_goal(self.0)
    }
    fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        AIUpdateInterface::set_locomotor_goal_orientation(self.0, angle)
    }
    fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        AIUpdateInterface::set_locomotor_goal_position_explicit(self.0, pos)
    }
    fn set_locomotor_goal_position_on_path(&mut self) {
        AIUpdateInterface::set_locomotor_goal_position_on_path(self.0)
    }
    fn friend_ending_move(&mut self) {
        AIUpdateInterface::friend_ending_move(self.0)
    }
    fn friend_starting_move(&mut self) {
        AIUpdateInterface::friend_starting_move(self.0)
    }
    fn is_allowed_to_adjust_destination(&self) -> bool {
        AIUpdateInterface::is_allowed_to_adjust_destination(self.0)
    }
    fn get_desired_speed(&self) -> Real {
        AIUpdateInterface::get_desired_speed(self.0)
    }
    fn set_desired_speed(&mut self, speed: Real) {
        AIUpdateInterface::set_desired_speed(self.0, speed)
    }
    fn is_in_rappel_state(&self) -> bool {
        AIUpdateInterface::is_in_rappel_state(self.0)
    }
    fn is_doing_combat_drop(&self) -> bool {
        AIUpdateInterface::is_doing_combat_drop(self.0)
    }
    fn set_queue_for_path_time(&mut self, _frames: UnsignedInt) {
        AIUpdateInterface::set_queue_for_path_time(self.0, _frames)
    }
    fn is_temporarily_preventing_aim_success(&self) -> bool {
        AIUpdateInterface::is_temporarily_preventing_aim_success(self.0)
    }
    fn add_targeter(&mut self, _id: ObjectID, _add: bool) {
        AIUpdateInterface::add_targeter(self.0, _id, _add)
    }
    fn clear_guard_target_type(&mut self) {
        AIUpdateInterface::clear_guard_target_type(self.0)
    }
    fn set_turret_target_object(
        &mut self,
        _turret: TurretType,
        _target_id: Option<ObjectID>,
        _force_attacking: bool,
    ) {
        AIUpdateInterface::set_turret_target_object(self.0, _turret, _target_id, _force_attacking)
    }
    fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        _slot: crate::weapon::WeaponSlotType,
        _target: crate::common::ObjectID,
    ) -> bool {
        AIUpdateInterface::is_weapon_slot_on_turret_and_aiming_at_target(self.0, _slot, _target)
    }
    fn get_supply_truck_ai_interface(&self) -> Option<&dyn SupplyTruckAIInterface> {
        AIUpdateInterface::get_supply_truck_ai_interface(self.0)
    }
    fn get_supply_truck_ai_interface_mut(&mut self) -> Option<&mut dyn SupplyTruckAIInterface> {
        AIUpdateInterface::get_supply_truck_ai_interface_mut(self.0)
    }
    fn ignore_obstacle(
        &mut self,
        _obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::ignore_obstacle(self.0, _obj_id)
    }
    fn ignore_obstacle_id(
        &mut self,
        id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::ignore_obstacle_id(self.0, id)
    }
    fn set_current_goal_path_index(
        &mut self,
        _index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::set_current_goal_path_index(self.0, _index)
    }
    fn get_current_goal_path_index(&self) -> i32 {
        AIUpdateInterface::get_current_goal_path_index(self.0)
    }
    fn set_can_path_through_units(
        &mut self,
        _value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::set_can_path_through_units(self.0, _value)
    }
    fn get_can_path_through_units(&self) -> bool {
        AIUpdateInterface::get_can_path_through_units(self.0)
    }
    fn is_blocked_and_stuck(&self) -> bool {
        AIUpdateInterface::is_blocked_and_stuck(self.0)
    }
    fn get_num_frames_blocked(&self) -> u32 {
        AIUpdateInterface::get_num_frames_blocked(self.0)
    }
    fn destroy_path(&mut self) {
        AIUpdateInterface::destroy_path(self.0)
    }
    fn clear_move_out_of_way(&mut self) {
        AIUpdateInterface::clear_move_out_of_way(self.0)
    }
    fn request_path(&mut self, _destination: &Coord3D, _is_final_goal: bool) -> Result<(), String> {
        AIUpdateInterface::request_path(self.0, _destination, _is_final_goal)
    }
    fn can_compute_quick_path(&self) -> bool {
        AIUpdateInterface::can_compute_quick_path(self.0)
    }
    fn get_crate_id(&self) -> ObjectID {
        AIUpdateInterface::get_crate_id(self.0)
    }
    fn get_current_victim(&self) -> Option<ObjectID> {
        AIUpdateInterface::get_current_victim(self.0)
    }
    fn set_current_victim(&mut self, _victim: Option<ObjectID>) {
        AIUpdateInterface::set_current_victim(self.0, _victim)
    }
    fn check_for_crate_to_pickup_id(&mut self) -> ObjectID {
        AIUpdateInterface::check_for_crate_to_pickup_id(self.0)
    }
    fn get_next_mood_target_with_attack_state(
        &mut self,
        called_by_ai: bool,
        called_during_idle: bool,
        _is_attacking: bool,
    ) -> Option<Arc<RwLock<Object>>> {
        AIUpdateInterface::get_next_mood_target_with_attack_state(
            self.0,
            called_by_ai,
            called_during_idle,
            _is_attacking,
        )
    }
    fn get_next_mood_check_time(&self) -> u32 {
        AIUpdateInterface::get_next_mood_check_time(self.0)
    }
    fn reset_next_mood_check_time(&mut self) {
        AIUpdateInterface::reset_next_mood_check_time(self.0)
    }
    fn get_mood_matrix_value(&self) -> u32 {
        AIUpdateInterface::get_mood_matrix_value(self.0)
    }
    fn get_mood_matrix_action_adjustment(&mut self, _action: crate::ai::MoodMatrixAction) -> u32 {
        AIUpdateInterface::get_mood_matrix_action_adjustment(self.0, _action)
    }
    fn set_original_victim_pos(&mut self, _pos: Option<Coord3D>) {
        AIUpdateInterface::set_original_victim_pos(self.0, _pos)
    }
    fn set_prior_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId) {
        AIUpdateInterface::set_prior_waypoint_id(self.0, _waypoint_id)
    }
    fn set_current_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId) {
        AIUpdateInterface::set_current_waypoint_id(self.0, _waypoint_id)
    }
    fn set_completed_waypoint_id(&mut self, _waypoint_id: Option<crate::waypoint::WaypointId>) {
        AIUpdateInterface::set_completed_waypoint_id(self.0, _waypoint_id)
    }
    fn choose_locomotor_set(
        &mut self,
        _set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::choose_locomotor_set(self.0, _set)
    }
    fn set_allow_invalid_position(
        &mut self,
        _allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::set_allow_invalid_position(self.0, _allow)
    }
    fn set_precise_z_pos(
        &mut self,
        _precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::set_precise_z_pos(self.0, _precise)
    }
    fn get_speed(&self) -> f32 {
        AIUpdateInterface::get_speed(self.0)
    }
    fn set_path_extra_distance(
        &mut self,
        _distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::set_path_extra_distance(self.0, _distance)
    }
    fn set_path_from_waypoint(
        &mut self,
        _waypoint: &crate::waypoint::Waypoint,
        _group_offset: &Coord2D,
    ) -> Result<(), String> {
        AIUpdateInterface::set_path_from_waypoint(self.0, _waypoint, _group_offset)
    }
    fn is_waiting_for_path(&self) -> bool {
        AIUpdateInterface::is_waiting_for_path(self.0)
    }
    fn append_goal_position_to_path(&mut self, _goal: &Coord3D) -> Result<(), String> {
        AIUpdateInterface::append_goal_position_to_path(self.0, _goal)
    }
    fn request_safe_path(&mut self, _repulsor_id: ObjectID) -> Result<bool, String> {
        AIUpdateInterface::request_safe_path(self.0, _repulsor_id)
    }
    fn is_doing_ground_movement(&self) -> bool {
        AIUpdateInterface::is_doing_ground_movement(self.0)
    }
    fn update_goal_position(
        &mut self,
        _goal: &Coord3D,
        _layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String> {
        AIUpdateInterface::update_goal_position(self.0, _goal, _layer)
    }
    fn adjust_destination(&mut self, _goal: &mut Coord3D) -> bool {
        AIUpdateInterface::adjust_destination(self.0, _goal)
    }
    fn snap_closest_goal_position(
        &mut self,
        owner: &crate::object::Object,
        goal: &mut Coord3D,
    ) -> bool {
        snap_closest_goal_position_with_interface(self.0, owner, goal)
    }
    fn set_adjusts_destination(&mut self, adjust: bool) {
        AIUpdateInterface::set_adjusts_destination(self.0, adjust)
    }
    fn should_adjust_destination(&self, state_adjusts: bool) -> bool {
        state_adjusts && AIUpdateInterface::get_adjusts_destination(self.0)
    }
}

// Concrete existing interface implementations retain their exact operations.
// Native UnitAiRuntime implements the capability directly after its field split.
impl<T: AIUpdateInterface> AiStateRuntime for T {
    fn dispatch_command_with_driver(
        &mut self,
        params: &crate::ai::AiCommandParams,
        _driver: &mut crate::ai::states::AIStateMachineDriver<'_>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::execute_command(self, params)
    }

    fn set_final_position(&mut self, _position: &Coord3D) {
        AIUpdateInterface::set_final_position(self, _position)
    }
    fn is_idle_with_parent_state(&self, parent_is_idle: bool) -> bool {
        AIUpdateInterface::is_idle_with_parent_state(self, parent_is_idle)
    }
    fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String> {
        AIUpdateInterface::set_movement_target(self, target)
    }
    fn get_preferred_height(&self) -> Option<Real> {
        AIUpdateInterface::get_preferred_height(self)
    }
    fn with_cur_locomotor(&self, _f: &mut dyn FnMut(&crate::locomotor::Locomotor)) {
        AIUpdateInterface::with_cur_locomotor(self, _f)
    }
    fn with_cur_locomotor_mut(&mut self, _f: &mut dyn FnMut(&mut crate::locomotor::Locomotor)) {
        AIUpdateInterface::with_cur_locomotor_mut(self, _f)
    }
    fn get_path(&self) -> Option<()> {
        AIUpdateInterface::get_path(self)
    }
    fn get_path_destination(&self) -> Option<Coord3D> {
        AIUpdateInterface::get_path_destination(self)
    }
    fn get_path_last_node(&self) -> Option<Coord3D> {
        AIUpdateInterface::get_path_last_node(self)
    }
    fn installed_path_last_layer(&self) -> Option<u8> {
        AIUpdateInterface::installed_path_last_layer(self)
    }
    fn get_retry_path(&self) -> bool {
        AIUpdateInterface::get_retry_path(self)
    }
    fn get_locomotor_distance_to_goal(&mut self, _fallback_goal: Option<Coord3D>) -> Real {
        AIUpdateInterface::get_locomotor_distance_to_goal(self)
    }
    fn get_last_command_source(&self) -> crate::ai::CommandSourceType {
        AIUpdateInterface::get_last_command_source(self)
    }
    fn set_last_command_source(&mut self, _source: crate::ai::CommandSourceType) {
        AIUpdateInterface::set_last_command_source(self, _source)
    }
    fn set_locomotor_goal_none(&mut self) {
        AIUpdateInterface::set_locomotor_goal_none(self)
    }
    fn remove_pathfinder_goal(&mut self) {
        AIUpdateInterface::remove_pathfinder_goal(self)
    }
    fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        AIUpdateInterface::set_locomotor_goal_orientation(self, angle)
    }
    fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        AIUpdateInterface::set_locomotor_goal_position_explicit(self, pos)
    }
    fn set_locomotor_goal_position_on_path(&mut self) {
        AIUpdateInterface::set_locomotor_goal_position_on_path(self)
    }
    fn friend_ending_move(&mut self) {
        AIUpdateInterface::friend_ending_move(self)
    }
    fn friend_starting_move(&mut self) {
        AIUpdateInterface::friend_starting_move(self)
    }
    fn is_allowed_to_adjust_destination(&self) -> bool {
        AIUpdateInterface::is_allowed_to_adjust_destination(self)
    }
    fn get_desired_speed(&self) -> Real {
        AIUpdateInterface::get_desired_speed(self)
    }
    fn set_desired_speed(&mut self, speed: Real) {
        AIUpdateInterface::set_desired_speed(self, speed)
    }
    fn is_in_rappel_state(&self) -> bool {
        AIUpdateInterface::is_in_rappel_state(self)
    }
    fn is_doing_combat_drop(&self) -> bool {
        AIUpdateInterface::is_doing_combat_drop(self)
    }
    fn set_queue_for_path_time(&mut self, _frames: UnsignedInt) {
        AIUpdateInterface::set_queue_for_path_time(self, _frames)
    }
    fn is_temporarily_preventing_aim_success(&self) -> bool {
        AIUpdateInterface::is_temporarily_preventing_aim_success(self)
    }
    fn add_targeter(&mut self, _id: ObjectID, _add: bool) {
        AIUpdateInterface::add_targeter(self, _id, _add)
    }
    fn clear_guard_target_type(&mut self) {
        AIUpdateInterface::clear_guard_target_type(self)
    }
    fn set_turret_target_object(
        &mut self,
        _turret: TurretType,
        _target_id: Option<ObjectID>,
        _force_attacking: bool,
    ) {
        AIUpdateInterface::set_turret_target_object(self, _turret, _target_id, _force_attacking)
    }
    fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        _slot: crate::weapon::WeaponSlotType,
        _target: crate::common::ObjectID,
    ) -> bool {
        AIUpdateInterface::is_weapon_slot_on_turret_and_aiming_at_target(self, _slot, _target)
    }
    fn get_supply_truck_ai_interface(&self) -> Option<&dyn SupplyTruckAIInterface> {
        AIUpdateInterface::get_supply_truck_ai_interface(self)
    }
    fn get_supply_truck_ai_interface_mut(&mut self) -> Option<&mut dyn SupplyTruckAIInterface> {
        AIUpdateInterface::get_supply_truck_ai_interface_mut(self)
    }
    fn ignore_obstacle(
        &mut self,
        _obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::ignore_obstacle(self, _obj_id)
    }
    fn ignore_obstacle_id(
        &mut self,
        id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::ignore_obstacle_id(self, id)
    }
    fn set_current_goal_path_index(
        &mut self,
        _index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::set_current_goal_path_index(self, _index)
    }
    fn get_current_goal_path_index(&self) -> i32 {
        AIUpdateInterface::get_current_goal_path_index(self)
    }
    fn set_can_path_through_units(
        &mut self,
        _value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::set_can_path_through_units(self, _value)
    }
    fn get_can_path_through_units(&self) -> bool {
        AIUpdateInterface::get_can_path_through_units(self)
    }
    fn is_blocked_and_stuck(&self) -> bool {
        AIUpdateInterface::is_blocked_and_stuck(self)
    }
    fn get_num_frames_blocked(&self) -> u32 {
        AIUpdateInterface::get_num_frames_blocked(self)
    }
    fn destroy_path(&mut self) {
        AIUpdateInterface::destroy_path(self)
    }
    fn clear_move_out_of_way(&mut self) {
        AIUpdateInterface::clear_move_out_of_way(self)
    }
    fn request_path(&mut self, _destination: &Coord3D, _is_final_goal: bool) -> Result<(), String> {
        AIUpdateInterface::request_path(self, _destination, _is_final_goal)
    }
    fn can_compute_quick_path(&self) -> bool {
        AIUpdateInterface::can_compute_quick_path(self)
    }
    fn get_crate_id(&self) -> ObjectID {
        AIUpdateInterface::get_crate_id(self)
    }
    fn get_current_victim(&self) -> Option<ObjectID> {
        AIUpdateInterface::get_current_victim(self)
    }
    fn set_current_victim(&mut self, _victim: Option<ObjectID>) {
        AIUpdateInterface::set_current_victim(self, _victim)
    }
    fn check_for_crate_to_pickup_id(&mut self) -> ObjectID {
        AIUpdateInterface::check_for_crate_to_pickup_id(self)
    }
    fn get_next_mood_target_with_attack_state(
        &mut self,
        called_by_ai: bool,
        called_during_idle: bool,
        _is_attacking: bool,
    ) -> Option<Arc<RwLock<Object>>> {
        AIUpdateInterface::get_next_mood_target_with_attack_state(
            self,
            called_by_ai,
            called_during_idle,
            _is_attacking,
        )
    }
    fn get_next_mood_check_time(&self) -> u32 {
        AIUpdateInterface::get_next_mood_check_time(self)
    }
    fn reset_next_mood_check_time(&mut self) {
        AIUpdateInterface::reset_next_mood_check_time(self)
    }
    fn get_mood_matrix_value(&self) -> u32 {
        AIUpdateInterface::get_mood_matrix_value(self)
    }
    fn get_mood_matrix_action_adjustment(&mut self, _action: crate::ai::MoodMatrixAction) -> u32 {
        AIUpdateInterface::get_mood_matrix_action_adjustment(self, _action)
    }
    fn set_original_victim_pos(&mut self, _pos: Option<Coord3D>) {
        AIUpdateInterface::set_original_victim_pos(self, _pos)
    }
    fn set_prior_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId) {
        AIUpdateInterface::set_prior_waypoint_id(self, _waypoint_id)
    }
    fn set_current_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId) {
        AIUpdateInterface::set_current_waypoint_id(self, _waypoint_id)
    }
    fn set_completed_waypoint_id(&mut self, _waypoint_id: Option<crate::waypoint::WaypointId>) {
        AIUpdateInterface::set_completed_waypoint_id(self, _waypoint_id)
    }
    fn choose_locomotor_set(
        &mut self,
        _set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::choose_locomotor_set(self, _set)
    }
    fn set_allow_invalid_position(
        &mut self,
        _allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::set_allow_invalid_position(self, _allow)
    }
    fn set_precise_z_pos(
        &mut self,
        _precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::set_precise_z_pos(self, _precise)
    }
    fn get_speed(&self) -> f32 {
        AIUpdateInterface::get_speed(self)
    }
    fn set_path_extra_distance(
        &mut self,
        _distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        AIUpdateInterface::set_path_extra_distance(self, _distance)
    }
    fn set_path_from_waypoint(
        &mut self,
        _waypoint: &crate::waypoint::Waypoint,
        _group_offset: &Coord2D,
    ) -> Result<(), String> {
        AIUpdateInterface::set_path_from_waypoint(self, _waypoint, _group_offset)
    }
    fn is_waiting_for_path(&self) -> bool {
        AIUpdateInterface::is_waiting_for_path(self)
    }
    fn append_goal_position_to_path(&mut self, _goal: &Coord3D) -> Result<(), String> {
        AIUpdateInterface::append_goal_position_to_path(self, _goal)
    }
    fn request_safe_path(&mut self, _repulsor_id: ObjectID) -> Result<bool, String> {
        AIUpdateInterface::request_safe_path(self, _repulsor_id)
    }
    fn is_doing_ground_movement(&self) -> bool {
        AIUpdateInterface::is_doing_ground_movement(self)
    }
    fn update_goal_position(
        &mut self,
        _goal: &Coord3D,
        _layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String> {
        AIUpdateInterface::update_goal_position(self, _goal, _layer)
    }
    fn adjust_destination(&mut self, _goal: &mut Coord3D) -> bool {
        AIUpdateInterface::adjust_destination(self, _goal)
    }
    fn snap_closest_goal_position(
        &mut self,
        owner: &crate::object::Object,
        goal: &mut Coord3D,
    ) -> bool {
        snap_closest_goal_position_with_interface(self, owner, goal)
    }
    fn set_adjusts_destination(&mut self, adjust: bool) {
        AIUpdateInterface::set_adjusts_destination(self, adjust)
    }
    fn should_adjust_destination(&self, state_adjusts: bool) -> bool {
        state_adjusts && AIUpdateInterface::get_adjusts_destination(self)
    }
}
