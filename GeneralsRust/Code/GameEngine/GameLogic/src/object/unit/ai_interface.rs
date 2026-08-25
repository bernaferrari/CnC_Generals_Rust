//! AIUpdateInterface trait impl — delegates to inherent UnitAIUpdate methods.

#![allow(unused_imports)]

use super::ai_core::UnitAIUpdate;
use super::imports::*;

impl AIUpdateInterface for UnitAIUpdate {
    fn xfer_ai_update_state(&mut self, xfer: &mut dyn Xfer) -> Result<bool, String> {
        UnitAIUpdate::xfer_ai_update_state(self, xfer)
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::update(self)
    }

    fn apply_bump_speed_limit(&mut self, mut desired_speed: Real, mut blocked: bool) -> Real {
        UnitAIUpdate::apply_bump_speed_limit(self, desired_speed, blocked)
    }

    fn is_attacking(&self) -> bool {
        UnitAIUpdate::is_attacking(self)
    }

    fn get_enter_target(&self) -> Option<ObjectID> {
        UnitAIUpdate::get_enter_target(self)
    }

    fn set_demoralized(&mut self, duration_frames: UnsignedInt) {
        UnitAIUpdate::set_demoralized(self, duration_frames)
    }

    fn get_which_turret_for_cur_weapon(&self) -> TurretType {
        UnitAIUpdate::get_which_turret_for_cur_weapon(self)
    }

    fn get_which_turret_for_weapon_slot(&self, slot: WeaponSlotType) -> TurretType {
        UnitAIUpdate::get_which_turret_for_weapon_slot(self, slot)
    }

    fn set_turret_enabled(&mut self, turret: TurretType, enabled: bool) {
        UnitAIUpdate::set_turret_enabled(self, turret, enabled)
    }

    fn recenter_turret(&mut self, turret: TurretType) {
        UnitAIUpdate::recenter_turret(self, turret)
    }

    fn is_turret_in_natural_position(&self, turret: TurretType) -> bool {
        UnitAIUpdate::is_turret_in_natural_position(self, turret)
    }

    fn is_turret_enabled(&self, turret: TurretType) -> bool {
        UnitAIUpdate::is_turret_enabled(self, turret)
    }

    fn friend_get_turret_sync(&self) -> TurretType {
        UnitAIUpdate::friend_get_turret_sync(self)
    }

    fn friend_set_turret_sync(&mut self, turret: TurretType) {
        UnitAIUpdate::friend_set_turret_sync(self, turret)
    }

    fn clear_guard_target_type(&mut self) {
        UnitAIUpdate::clear_guard_target_type(self)
    }

    fn get_turret_rot_and_pitch(&self, turret: TurretType) -> Option<(Real, Real)> {
        UnitAIUpdate::get_turret_rot_and_pitch(self, turret)
    }

    fn get_turret_angle(&self, turret: TurretType) -> Real {
        UnitAIUpdate::get_turret_angle(self, turret)
    }

    fn get_turret_pitch(&self, turret: TurretType) -> Real {
        UnitAIUpdate::get_turret_pitch(self, turret)
    }

    fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        slot: WeaponSlotType,
        target: ObjectID,
    ) -> bool {
        UnitAIUpdate::is_weapon_slot_on_turret_and_aiming_at_target(self, slot, target)
    }

    fn is_moving(&self) -> bool {
        UnitAIUpdate::is_moving(self)
    }

    fn is_idle(&self) -> bool {
        UnitAIUpdate::is_idle(self)
    }

    fn is_busy(&self) -> bool {
        UnitAIUpdate::is_busy(self)
    }

    fn set_attitude(
        &mut self,
        attitude: AIAttitudeType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::set_attitude(self, attitude)
    }

    fn get_attitude(&self) -> AIAttitudeType {
        UnitAIUpdate::get_attitude(self)
    }

    fn is_idle_unrestricted(&self) -> bool {
        UnitAIUpdate::is_idle_unrestricted(self)
    }

    fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String> {
        UnitAIUpdate::set_movement_target(self, target)
    }

    fn set_current_goal_path_index(
        &mut self,
        index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::set_current_goal_path_index(self, index)
    }

    fn get_current_goal_path_index(&self) -> i32 {
        UnitAIUpdate::get_current_goal_path_index(self)
    }

    fn set_can_path_through_units(
        &mut self,
        value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::set_can_path_through_units(self, value)
    }

    fn get_can_path_through_units(&self) -> bool {
        UnitAIUpdate::get_can_path_through_units(self)
    }

    fn is_blocked_and_stuck(&self) -> bool {
        UnitAIUpdate::is_blocked_and_stuck(self)
    }

    fn set_is_blocked(&mut self, blocked: bool) {
        UnitAIUpdate::set_is_blocked(self, blocked)
    }

    fn set_blocked_and_stuck(&mut self, blocked: bool) {
        UnitAIUpdate::set_blocked_and_stuck(self, blocked)
    }

    fn get_num_frames_blocked(&self) -> u32 {
        UnitAIUpdate::get_num_frames_blocked(self)
    }

    fn destroy_path(&mut self) {
        UnitAIUpdate::destroy_path(self)
    }

    fn clear_move_out_of_way(&mut self) {
        UnitAIUpdate::clear_move_out_of_way(self)
    }

    fn execute_command(
        &mut self,
        command: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::execute_command(self, command)
    }

    fn get_preferred_height(&self) -> Option<Real> {
        UnitAIUpdate::get_preferred_height(self)
    }

    fn is_allowed_to_adjust_destination(&self) -> bool {
        UnitAIUpdate::is_allowed_to_adjust_destination(self)
    }

    fn get_ai_free_to_exit(&self, exiter: &Object) -> crate::object::production::AIFreeToExitType {
        UnitAIUpdate::get_ai_free_to_exit(self, exiter)
    }

    fn set_path_extra_distance(
        &mut self,
        distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::set_path_extra_distance(self, distance)
    }

    fn set_path_from_waypoint(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        group_offset: &Coord2D,
    ) -> Result<(), String> {
        UnitAIUpdate::set_path_from_waypoint(self, waypoint, group_offset)
    }

    fn is_waypoint_queue_empty(&self) -> bool {
        UnitAIUpdate::is_waypoint_queue_empty(self)
    }

    fn do_pathfind(&mut self) {
        UnitAIUpdate::do_pathfind(self)
    }

    fn is_waiting_for_path(&self) -> bool {
        UnitAIUpdate::is_waiting_for_path(self)
    }

    fn queue_waypoint(&mut self, pos: &Coord3D) {
        UnitAIUpdate::queue_waypoint(self, pos)
    }

    fn execute_waypoint_queue(&mut self) {
        UnitAIUpdate::execute_waypoint_queue(self)
    }

    fn clear_waypoint_queue(&mut self) {
        UnitAIUpdate::clear_waypoint_queue(self)
    }

    fn append_goal_position_to_path(&mut self, goal: &Coord3D) -> Result<(), String> {
        UnitAIUpdate::append_goal_position_to_path(self, goal)
    }

    fn set_path_from_coords(&mut self, path: &[Coord3D]) -> Result<(), String> {
        UnitAIUpdate::set_path_from_coords(self, path)
    }

    fn request_safe_path(&mut self, repulsor_id: ObjectID) -> Result<bool, String> {
        UnitAIUpdate::request_safe_path(self, repulsor_id)
    }

    fn is_doing_ground_movement(&self) -> bool {
        UnitAIUpdate::is_doing_ground_movement(self)
    }

    fn is_allowed_to_move_away_from_unit(&self) -> bool {
        UnitAIUpdate::is_allowed_to_move_away_from_unit(self)
    }

    fn get_sneaky_targeting_offset(&self, offset: &mut Coord3D) -> bool {
        UnitAIUpdate::get_sneaky_targeting_offset(self, offset)
    }

    fn is_temporarily_preventing_aim_success(&self) -> bool {
        UnitAIUpdate::is_temporarily_preventing_aim_success(self)
    }

    fn add_targeter(&mut self, id: ObjectID, add: bool) {
        UnitAIUpdate::add_targeter(self, id, add)
    }

    fn are_turrets_linked(&self) -> Bool {
        UnitAIUpdate::are_turrets_linked(self)
    }

    fn set_turret_target_object(
        &mut self,
        turret: TurretType,
        target_id: Option<ObjectID>,
        force_attacking: bool,
    ) {
        UnitAIUpdate::set_turret_target_object(self, turret, target_id, force_attacking)
    }

    fn set_turret_target_position(&mut self, turret: TurretType, pos: &Coord3D) {
        UnitAIUpdate::set_turret_target_position(self, turret, pos)
    }

    fn is_out_of_special_reload_ammo(&self) -> bool {
        UnitAIUpdate::is_out_of_special_reload_ammo(self)
    }

    fn get_treat_as_aircraft_for_loco_dist_to_goal(&self) -> bool {
        UnitAIUpdate::get_treat_as_aircraft_for_loco_dist_to_goal(self)
    }

    fn update_goal_position(
        &mut self,
        goal: &Coord3D,
        layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String> {
        UnitAIUpdate::update_goal_position(self, goal, layer)
    }

    fn adjust_destination(&mut self, goal: &mut Coord3D) -> bool {
        UnitAIUpdate::adjust_destination(self, goal)
    }

    fn set_adjusts_destination(&mut self, adjust: bool) {
        UnitAIUpdate::set_adjusts_destination(self, adjust)
    }

    fn set_allow_invalid_position(
        &mut self,
        allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::set_allow_invalid_position(self, allow)
    }

    fn set_allow_chase(&mut self, allowed: bool) {
        UnitAIUpdate::set_allow_chase(self, allowed)
    }

    fn set_locomotor_upgrade(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::set_locomotor_upgrade(self, enabled)
    }

    fn choose_locomotor_set(
        &mut self,
        set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::choose_locomotor_set(self, set)
    }

    fn set_ultra_accurate(
        &mut self,
        ultra: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::set_ultra_accurate(self, ultra)
    }

    fn set_precise_z_pos(
        &mut self,
        precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::set_precise_z_pos(self, precise)
    }

    fn get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
        UnitAIUpdate::get_cur_locomotor(self)
    }

    fn get_locomotor_set_clone(&self) -> Option<crate::locomotor::LocomotorSet> {
        UnitAIUpdate::get_locomotor_set_clone(self)
    }

    fn get_path_destination(&self) -> Option<Coord3D> {
        UnitAIUpdate::get_path_destination(self)
    }

    fn peek_cached_point_on_path(&self) -> Option<Coord3D> {
        UnitAIUpdate::peek_cached_point_on_path(self)
    }

    fn get_locomotor_distance_to_goal(&self) -> Real {
        UnitAIUpdate::get_locomotor_distance_to_goal(self)
    }

    fn get_speed(&self) -> f32 {
        UnitAIUpdate::get_speed(self)
    }

    fn get_last_command_source(&self) -> CommandSourceType {
        UnitAIUpdate::get_last_command_source(self)
    }

    fn set_last_command_source(&mut self, source: CommandSourceType) {
        UnitAIUpdate::set_last_command_source(self, source)
    }

    fn get_current_command(&self) -> Option<crate::ai::AiCommandType> {
        UnitAIUpdate::get_current_command(self)
    }

    fn get_pending_command_type(&self) -> Option<crate::ai::AiCommandType> {
        UnitAIUpdate::get_pending_command_type(self)
    }

    fn purge_pending_command(&mut self) {
        UnitAIUpdate::purge_pending_command(self)
    }

    fn is_taxiing_to_parking(&self) -> bool {
        UnitAIUpdate::is_taxiing_to_parking(self)
    }

    fn is_reloading(&self) -> bool {
        UnitAIUpdate::is_reloading(self)
    }

    fn is_clearing_mines(&self) -> bool {
        UnitAIUpdate::is_clearing_mines(self)
    }

    fn is_takeoff_or_landing_in_progress(&self) -> bool {
        UnitAIUpdate::is_takeoff_or_landing_in_progress(self)
    }

    fn get_current_state_id(&self) -> Option<u32> {
        UnitAIUpdate::get_current_state_id(self)
    }

    fn get_parking_offset(&self) -> Real {
        UnitAIUpdate::get_parking_offset(self)
    }

    fn keeps_parking_space_when_airborne(&self) -> bool {
        UnitAIUpdate::keeps_parking_space_when_airborne(self)
    }

    fn get_desired_speed(&self) -> Real {
        UnitAIUpdate::get_desired_speed(self)
    }

    fn set_desired_speed(&mut self, speed: Real) {
        UnitAIUpdate::set_desired_speed(self, speed)
    }

    fn is_in_rappel_state(&self) -> bool {
        UnitAIUpdate::is_in_rappel_state(self)
    }

    fn is_doing_combat_drop(&self) -> bool {
        UnitAIUpdate::is_doing_combat_drop(self)
    }

    fn is_aircraft_that_adjusts_destination(&self) -> bool {
        UnitAIUpdate::is_aircraft_that_adjusts_destination(self)
    }

    fn is_moving_away_from(&self, obj_id: ObjectID) -> bool {
        UnitAIUpdate::is_moving_away_from(self, obj_id)
    }

    fn set_ignore_collision_time(&mut self, duration_frames: UnsignedInt) {
        UnitAIUpdate::set_ignore_collision_time(self, duration_frames)
    }

    fn get_ignore_collisions_until(&self) -> UnsignedInt {
        UnitAIUpdate::get_ignore_collisions_until(self)
    }

    fn set_queue_for_path_time(&mut self, frames: UnsignedInt) {
        UnitAIUpdate::set_queue_for_path_time(self, frames)
    }

    fn ignore_obstacle(
        &mut self,
        obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::ignore_obstacle(self, obj_id)
    }

    fn ignore_obstacle_id(
        &mut self,
        id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::ignore_obstacle_id(self, id)
    }

    fn get_ignored_obstacle_id(&self) -> ObjectID {
        UnitAIUpdate::get_ignored_obstacle_id(self)
    }

    fn is_ai_in_dead_state(&self) -> bool {
        UnitAIUpdate::is_ai_in_dead_state(self)
    }

    fn mark_as_dead(&mut self) {
        UnitAIUpdate::mark_as_dead(self)
    }

    fn set_is_recruitable(&mut self, recruitable: Bool) {
        UnitAIUpdate::set_is_recruitable(self, recruitable)
    }

    fn is_recruitable(&self) -> bool {
        self.is_recruitable
    }

    fn get_goal_object_id(&self) -> ObjectID {
        UnitAIUpdate::get_goal_object_id(self)
    }

    fn set_goal_object(&mut self, obj_id: Option<ObjectID>) {
        UnitAIUpdate::set_goal_object(self, obj_id)
    }

    fn get_goal_position(&self) -> Option<Coord3D> {
        UnitAIUpdate::get_goal_position(self)
    }

    fn set_goal_position(&mut self, pos: Option<Coord3D>) {
        UnitAIUpdate::set_goal_position(self, pos)
    }

    /// C++ `AIUpdateInterface::joinTeam` (AIUpdate.cpp).
    ///
    /// After `clear()`, C++ `getCurrentStateID()` is `INVALID_STATE_ID` (NULL
    /// current state). `setState(INVALID)` then falls through to the default
    /// state. Port that literally — C++ does not read the teammate's state id.
    /// C++ `AIUpdateInterface::joinTeam` (AIUpdate.cpp).
    ///
    /// After `clear()`, C++ `getCurrentStateID()` is `INVALID_STATE_ID` (NULL
    /// current state). `setState(INVALID)` then falls through to the default
    /// state. Port that literally — C++ does not read the teammate's state id.
    fn join_team(&mut self) {
        UnitAIUpdate::join_team(self)
    }

    fn is_path_available(&self, destination: &Coord3D) -> bool {
        UnitAIUpdate::is_path_available(self, destination)
    }

    fn request_path(&mut self, destination: &Coord3D, _is_final_goal: bool) -> Result<(), String> {
        UnitAIUpdate::request_path(self, destination, _is_final_goal)
    }

    fn request_attack_path(
        &mut self,
        victim_id: ObjectID,
        victim_pos: &Coord3D,
    ) -> Result<(), String> {
        UnitAIUpdate::request_attack_path(self, victim_id, victim_pos)
    }

    fn request_approach_path(&mut self, destination: &Coord3D) -> Result<(), String> {
        UnitAIUpdate::request_approach_path(self, destination)
    }

    fn can_compute_quick_path(&self) -> bool {
        UnitAIUpdate::can_compute_quick_path(self)
    }

    fn compute_quick_path(&mut self, destination: &Coord3D) -> bool {
        UnitAIUpdate::compute_quick_path(self, destination)
    }

    fn is_quick_path_available(&self, destination: &Coord3D) -> bool {
        UnitAIUpdate::is_quick_path_available(self, destination)
    }

    fn is_valid_locomotor_position(&self, pos: &Coord3D) -> bool {
        UnitAIUpdate::is_valid_locomotor_position(self, pos)
    }

    fn need_to_rotate(&self) -> bool {
        UnitAIUpdate::need_to_rotate(self)
    }

    fn get_cur_locomotor_set_type(&self) -> LocomotorSetType {
        UnitAIUpdate::get_cur_locomotor_set_type(self)
    }

    fn has_locomotor_for_surface(&self, surface: crate::common::LocomotorSurfaceTypeMask) -> bool {
        UnitAIUpdate::has_locomotor_for_surface(self, surface)
    }

    fn get_cur_locomotor_speed(&self) -> Real {
        UnitAIUpdate::get_cur_locomotor_speed(self)
    }

    fn get_cur_max_blocked_speed(&self) -> Real {
        UnitAIUpdate::get_cur_max_blocked_speed(self)
    }

    fn set_cur_max_blocked_speed(&mut self, speed: Real) {
        UnitAIUpdate::set_cur_max_blocked_speed(self, speed)
    }

    fn set_locomotor_goal_none(&mut self) {
        UnitAIUpdate::set_locomotor_goal_none(self)
    }

    fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        UnitAIUpdate::set_locomotor_goal_orientation(self, angle)
    }

    fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        UnitAIUpdate::set_locomotor_goal_position_explicit(self, pos)
    }

    fn friend_ending_move(&mut self) {
        UnitAIUpdate::friend_ending_move(self)
    }

    fn friend_starting_move(&mut self) {
        UnitAIUpdate::friend_starting_move(self)
    }

    fn evaluate_morale_bonus(&mut self) {
        UnitAIUpdate::evaluate_morale_bonus(self)
    }

    fn set_surrendered(&mut self, to_object_id: Option<ObjectID>, surrendered: bool) {
        UnitAIUpdate::set_surrendered(self, to_object_id, surrendered)
    }

    fn transfer_attack(&mut self, from_id: ObjectID, to_id: ObjectID) {
        UnitAIUpdate::transfer_attack(self, from_id, to_id)
    }

    fn is_surrendered(&self) -> bool {
        UnitAIUpdate::is_surrendered(self)
    }

    fn get_surrendered_player_index(&self) -> Option<PlayerIndex> {
        UnitAIUpdate::get_surrendered_player_index(self)
    }

    fn ai_move_to_position(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::ai_move_to_position(self, pos)
    }

    fn ai_idle(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::ai_idle(self)
    }

    fn ai_busy(
        &mut self,
        cmd_source: crate::ai::CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::ai_busy(self, cmd_source)
    }

    fn ai_attack_object(
        &mut self,
        target_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::ai_attack_object(self, target_id)
    }

    fn ai_guard_position(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::ai_guard_position(self, pos)
    }

    fn get_crate_id(&self) -> ObjectID {
        UnitAIUpdate::get_crate_id(self)
    }

    fn get_current_victim(&self) -> Option<ObjectID> {
        UnitAIUpdate::get_current_victim(self)
    }

    fn set_current_victim(&mut self, victim: Option<ObjectID>) {
        UnitAIUpdate::set_current_victim(self, victim)
    }

    fn check_for_crate_to_pickup_id(&self) -> ObjectID {
        UnitAIUpdate::check_for_crate_to_pickup_id(self)
    }

    fn get_next_mood_target_id(
        &mut self,
        use_existing_target: bool,
        _ignore_attacked: bool,
    ) -> ObjectID {
        UnitAIUpdate::get_next_mood_target_id(self, use_existing_target, _ignore_attacked)
    }

    fn get_next_mood_check_time(&self) -> u32 {
        UnitAIUpdate::get_next_mood_check_time(self)
    }

    fn reset_next_mood_check_time(&mut self) {
        UnitAIUpdate::reset_next_mood_check_time(self)
    }

    fn set_next_mood_check_time(&mut self, frame: u32) {
        UnitAIUpdate::set_next_mood_check_time(self, frame)
    }

    fn get_mood_matrix_value(&self) -> u32 {
        UnitAIUpdate::get_mood_matrix_value(self)
    }

    fn get_mood_matrix_action_adjustment(&mut self, action: MoodMatrixAction) -> u32 {
        UnitAIUpdate::get_mood_matrix_action_adjustment(self, action)
    }

    fn notify_fired(&mut self) {
        UnitAIUpdate::notify_fired(self)
    }

    fn notify_new_victim_chosen(&mut self, victim: ObjectID) {
        UnitAIUpdate::notify_new_victim_chosen(self, victim)
    }

    fn is_weapon_slot_ok_to_fire(&self, _wslot: WeaponSlotType) -> Bool {
        UnitAIUpdate::is_weapon_slot_ok_to_fire(self, _wslot)
    }

    fn get_original_victim_pos(&self) -> Option<Coord3D> {
        UnitAIUpdate::get_original_victim_pos(self)
    }

    fn set_original_victim_pos(&mut self, pos: Option<Coord3D>) {
        UnitAIUpdate::set_original_victim_pos(self, pos)
    }

    fn is_in_attack_state(&self) -> bool {
        UnitAIUpdate::is_in_attack_state(self)
    }

    fn is_in_guard_idle_state(&self) -> bool {
        UnitAIUpdate::is_in_guard_idle_state(self)
    }

    fn set_temporary_state(&mut self, state: AIStateType, frame_limit: UnsignedInt) {
        UnitAIUpdate::set_temporary_state(self, state, frame_limit)
    }

    fn notify_crate(&mut self, crate_id: ObjectID) {
        UnitAIUpdate::notify_crate(self, crate_id)
    }

    fn notify_victim_is_dead(&mut self) {
        UnitAIUpdate::notify_victim_is_dead(self)
    }

    fn set_prior_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        UnitAIUpdate::set_prior_waypoint_id(self, waypoint_id)
    }

    fn set_current_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        UnitAIUpdate::set_current_waypoint_id(self, waypoint_id)
    }

    fn set_completed_waypoint_id(&mut self, waypoint_id: Option<crate::waypoint::WaypointId>) {
        UnitAIUpdate::set_completed_waypoint_id(self, waypoint_id)
    }

    fn get_completed_waypoint_id(&self) -> Option<crate::waypoint::WaypointId> {
        UnitAIUpdate::get_completed_waypoint_id(self)
    }

    fn get_supply_truck_ai_interface(&self) -> Option<&dyn crate::modules::SupplyTruckAIInterface> {
        UnitAIUpdate::get_supply_truck_ai_interface(self)
    }

    fn get_supply_truck_ai_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::SupplyTruckAIInterface> {
        UnitAIUpdate::get_supply_truck_ai_interface_mut(self)
    }

    fn get_pow_truck_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::POWTruckAIUpdateInterface> {
        UnitAIUpdate::get_pow_truck_ai_update_interface(self)
    }

    fn get_hack_internet_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::HackInternetAIUpdateInterface> {
        UnitAIUpdate::get_hack_internet_ai_update_interface(self)
    }

    fn get_assault_transport_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::AssaultTransportAIUpdateInterface> {
        UnitAIUpdate::get_assault_transport_ai_update_interface(self)
    }

    fn get_worker_ai_update_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::WorkerAIUpdateInterface> {
        UnitAIUpdate::get_worker_ai_update_interface_mut(self)
    }

    fn get_dozer_ai_update_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::DozerAIUpdateInterface> {
        UnitAIUpdate::get_dozer_ai_update_interface_mut(self)
    }

    fn get_deliver_payload_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::DeliverPayloadAIUpdateInterface> {
        UnitAIUpdate::get_deliver_payload_ai_update_interface(self)
    }

    fn ai_guard_object(
        &mut self,
        target_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        UnitAIUpdate::ai_guard_object(self, target_id)
    }

    fn ai_go_prone(&mut self, damage_info: &DamageInfo, _cmd_source: crate::ai::CommandSourceType) {
        UnitAIUpdate::ai_go_prone(self, damage_info, _cmd_source)
    }
}
