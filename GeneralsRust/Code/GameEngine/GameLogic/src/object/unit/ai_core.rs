//! UnitAIUpdate struct, construction, turrets, and rappel state.

#![allow(unused_imports)]

use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{dual_world_registry_unavailable, get_unit_arc};
use super::types::*;

/// Basic AI update interface that bridges AI commands to unit orders.
pub struct UnitAIUpdate {
    /// Owning unit id; resolve via UNIT_REGISTRY for the duration of an op.
    pub(super) unit_id: ObjectID,
    pub(super) crate_created: Mutex<ObjectID>,
    pub(super) supply_truck_ai: Option<SupplyTruckAIUpdate>,
    pub(super) chinook_ai: Option<ChinookAIUpdate>,
    pub(super) jet_ai: Option<JetAIUpdate>,
    pub(super) worker_ai: Option<WorkerAIUpdate>,
    pub(super) dozer_ai: Option<DozerAIUpdate>,
    #[cfg(feature = "allow_surrender")]
    pub(super) pow_truck_ai: Option<POWTruckAIUpdate>,
    pub(super) railed_transport_ai: Option<RailedTransportAIUpdate>,
    pub(super) hack_internet_ai: Option<HackInternetAIUpdate>,
    pub(super) assault_transport_ai: Option<AssaultTransportAIUpdate>,
    pub(super) deliver_payload_ai: Option<DeliverPayloadAIUpdate>,
    pub(super) transport_ai: Option<TransportAIUpdate>,
    pub(super) deploy_style_ai: Option<DeployStyleAIUpdate>,
    pub(super) wander_ai: Option<WanderAIUpdate>,
    pub(super) dock_machine: Option<AIDockMachine>,
    pub(super) ai_state_machine: Option<Arc<Mutex<AIStateMachine>>>,
    pub(super) can_path_through_units: bool,
    pub(super) allow_chase: bool,
    pub(super) attitude: AIAttitudeType,
    pub(super) last_command_source: CommandSourceType,
    pub(super) current_command: Option<crate::ai::AiCommandType>,
    pub(super) pending_command: Option<crate::ai::AiCommandType>,
    pub(super) surrendered_frames_left: UnsignedInt,
    pub(super) surrendered_player_index: Option<PlayerIndex>,
    pub(super) surrender_duration_frames: UnsignedInt,
    pub(super) demoralized_frames_left: UnsignedInt,
    pub(super) auto_acquire_enemies_when_idle: u32,
    pub(super) mood_attack_check_rate_frames: UnsignedInt,
    pub(super) forbid_player_commands: Bool,
    pub(super) turrets_linked: Bool,
    pub(super) turret_sync_flag: TurretType,
    pub(super) turret_primary_data: Option<TurretAIData>,
    pub(super) turret_secondary_data: Option<TurretAIData>,
    pub(super) locomotor_upgraded: Bool,
    pub(super) current_locomotor_set: LocomotorSetType,
    pub(super) locomotor_sets: HashMap<LocomotorSetType, Vec<AsciiString>>,
    pub(super) turret_primary_enabled: Bool,
    pub(super) turret_secondary_enabled: Bool,
    pub(super) turret_primary_natural: Bool,
    pub(super) turret_secondary_natural: Bool,
    pub(super) turret_primary_machine: Option<TurretStateMachine>,
    pub(super) turret_secondary_machine: Option<TurretStateMachine>,
    pub(super) enter_target: Option<ObjectID>,
    pub(super) desired_speed: Real,
    pub(super) prior_waypoint_id: Option<crate::waypoint::WaypointId>,
    pub(super) current_waypoint_id: Option<crate::waypoint::WaypointId>,
    pub(super) completed_waypoint_id: Option<crate::waypoint::WaypointId>,
    pub(super) current_goal_path_index: i32,
    pub(super) rappel_state: Option<RappelState>,
    pub(super) original_victim_pos: Option<Coord3D>,
    pub(super) pending_safe_path: Option<Vec<Coord3D>>,
    pub(super) guard_target_type: [GuardTargetType; 2],
    pub(super) location_to_guard: Coord3D,
    pub(super) object_to_guard: ObjectID,
    pub(super) planning_waypoint_queue: [Coord3D; AI_UPDATE_MAX_WAYPOINTS],
    pub(super) planning_waypoint_count: Int,
    pub(super) planning_waypoint_index: Int,
    pub(super) executing_waypoint_queue: Bool,
    pub(super) requested_victim_id: ObjectID,
    pub(super) requested_destination: Coord3D,
    pub(super) requested_destination2: Coord3D,
    pub(super) current_path_snapshot: Option<AiPath>,
    pub(super) pathfind_goal_cell: ICoord2D,
    pub(super) pathfind_cur_cell: ICoord2D,
    pub(super) pathfind_goal_layer: ClassicPathLayer,
    pub(super) move_out_of_way_1: ObjectID,
    pub(super) move_out_of_way_2: ObjectID,
    pub(super) repulsor1: ObjectID,
    pub(super) repulsor2: ObjectID,
    pub(super) ignore_obstacle_id: ObjectID,
    pub(super) ignore_collisions_until: UnsignedInt,
    pub(super) waiting_for_path: Bool,
    pub(super) queue_for_path_frame: UnsignedInt,
    pub(super) path_timestamp: UnsignedInt,
    pub(super) ai_dead: Bool,
    pub(super) is_recruitable: Bool,
    pub(super) next_enemy_scan_time: UnsignedInt,
    pub(super) final_position: Coord3D,
    pub(super) do_final_position: Bool,
    pub(super) is_attack_path: Bool,
    pub(super) is_final_goal: Bool,
    pub(super) is_approach_path: Bool,
    pub(super) is_safe_path: Bool,
    pub(super) movement_complete: Bool,
    pub(super) locomotor_goal_type: u32,
    pub(super) locomotor_goal_data: Coord3D,
    pub(super) is_blocked: Bool,
    pub(super) blocked_and_stuck: Bool,
    pub(super) retry_path: Bool,
    pub(super) blocked_frames: u32,
    pub(super) cur_max_blocked_speed: Real,
    pub(super) bump_speed_limit: Real,
}

impl std::fmt::Debug for UnitAIUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnitAIUpdate")
            .field("can_path_through_units", &self.can_path_through_units)
            .field("allow_chase", &self.allow_chase)
            .field("last_command_source", &self.last_command_source)
            .field("current_command", &self.current_command)
            .field("pending_command", &self.pending_command)
            .field("ai_dead", &self.ai_dead)
            .finish()
    }
}

impl UnitAIUpdate {
    pub fn new(
        unit_id: ObjectID,
        supply_truck_ai: Option<SupplyTruckAIUpdate>,
        chinook_ai: Option<ChinookAIUpdate>,
        jet_ai: Option<JetAIUpdate>,
        worker_ai: Option<WorkerAIUpdate>,
        dozer_ai: Option<DozerAIUpdate>,
        #[cfg(feature = "allow_surrender")] pow_truck_ai: Option<POWTruckAIUpdate>,
        railed_transport_ai: Option<RailedTransportAIUpdate>,
        hack_internet_ai: Option<HackInternetAIUpdate>,
        assault_transport_ai: Option<AssaultTransportAIUpdate>,
        deliver_payload_ai: Option<DeliverPayloadAIUpdate>,
        transport_ai: Option<TransportAIUpdate>,
        deploy_style_ai: Option<DeployStyleAIUpdate>,
        wander_ai: Option<WanderAIUpdate>,
    ) -> Self {
        let ai_state_machine = get_unit_arc(unit_id).and_then(|unit_arc| {
            let owner = unit_arc
                .read()
                .ok()
                .map(|guard| Arc::downgrade(&guard.base_arc()))?;
            Some(Arc::new(Mutex::new(AIStateMachine::new(
                owner,
                "AIStateMachine",
            ))))
        });

        Self {
            unit_id,
            crate_created: Mutex::new(crate::common::INVALID_ID),
            supply_truck_ai,
            chinook_ai,
            jet_ai,
            worker_ai,
            dozer_ai,
            #[cfg(feature = "allow_surrender")]
            pow_truck_ai,
            railed_transport_ai,
            hack_internet_ai,
            assault_transport_ai,
            deliver_payload_ai,
            transport_ai,
            deploy_style_ai,
            wander_ai,
            dock_machine: None,
            ai_state_machine,
            can_path_through_units: false,
            allow_chase: false,
            attitude: AIAttitudeType::Normal,
            last_command_source: CommandSourceType::FromAi,
            current_command: None,
            pending_command: None,
            surrendered_frames_left: 0,
            surrendered_player_index: None,
            surrender_duration_frames: LOGICFRAMES_PER_SECOND * 120,
            demoralized_frames_left: 0,
            auto_acquire_enemies_when_idle: 0,
            mood_attack_check_rate_frames: LOGICFRAMES_PER_SECOND * 2,
            forbid_player_commands: false,
            turrets_linked: false,
            turret_sync_flag: TurretType::Invalid,
            turret_primary_data: None,
            turret_secondary_data: None,
            locomotor_upgraded: false,
            current_locomotor_set: LocomotorSetType::Normal,
            locomotor_sets: HashMap::new(),
            turret_primary_enabled: true,
            turret_secondary_enabled: true,
            turret_primary_natural: true,
            turret_secondary_natural: true,
            turret_primary_machine: None,
            turret_secondary_machine: None,
            enter_target: None,
            desired_speed: FAST_AS_POSSIBLE,
            prior_waypoint_id: None,
            current_waypoint_id: None,
            completed_waypoint_id: None,
            current_goal_path_index: -1,
            rappel_state: None,
            original_victim_pos: None,
            pending_safe_path: None,
            guard_target_type: [GuardTargetType::None_; 2],
            location_to_guard: Coord3D::ZERO,
            object_to_guard: INVALID_ID,
            planning_waypoint_queue: [Coord3D::ZERO; AI_UPDATE_MAX_WAYPOINTS],
            planning_waypoint_count: 0,
            planning_waypoint_index: 0,
            executing_waypoint_queue: false,
            requested_victim_id: INVALID_ID,
            requested_destination: Coord3D::ZERO,
            requested_destination2: Coord3D::ZERO,
            current_path_snapshot: None,
            pathfind_goal_cell: ICoord2D::new(-1, -1),
            pathfind_cur_cell: ICoord2D::new(-1, -1),
            pathfind_goal_layer: ClassicPathLayer::Invalid,
            move_out_of_way_1: INVALID_ID,
            move_out_of_way_2: INVALID_ID,
            repulsor1: INVALID_ID,
            repulsor2: INVALID_ID,
            ignore_obstacle_id: INVALID_ID,
            ignore_collisions_until: 0,
            waiting_for_path: false,
            queue_for_path_frame: 0,
            path_timestamp: 0,
            ai_dead: false,
            is_recruitable: true,
            next_enemy_scan_time: 0,
            final_position: Coord3D::ZERO,
            do_final_position: false,
            is_attack_path: false,
            is_final_goal: false,
            is_approach_path: false,
            is_safe_path: false,
            movement_complete: false,
            locomotor_goal_type: 0,
            locomotor_goal_data: Coord3D::ZERO,
            is_blocked: false,
            blocked_and_stuck: false,
            retry_path: false,
            blocked_frames: 0,
            cur_max_blocked_speed: FAST_AS_POSSIBLE,
            bump_speed_limit: FAST_AS_POSSIBLE,
        }
    }
    pub(super) fn push_guard_target_type(&mut self, target_type: GuardTargetType) {
        if self.guard_target_type[1] == GuardTargetType::None_ {
            self.guard_target_type[1] = target_type;
        } else {
            self.guard_target_type[0] = target_type;
        }
    }
    pub(super) fn clear_guard_target_type(&mut self) {
        self.guard_target_type[1] = self.guard_target_type[0];
        self.guard_target_type[0] = GuardTargetType::None_;
    }
    pub(super) fn friend_get_turret_sync(&self) -> TurretType {
        self.turret_sync_flag
    }
    pub(super) fn friend_set_turret_sync(&mut self, turret: TurretType) {
        self.turret_sync_flag = turret;
    }
    pub(super) fn owner_object_id(&self) -> Option<ObjectID> {
        if self.unit_id != INVALID_ID {
            Some(self.unit_id)
        } else {
            None
        }
    }
    pub(super) fn wake_up_now(&self) {
        if let Some(owner_id) = self.owner_object_id() {
            TheGameLogic::set_wake_frame(owner_id, UPDATE_SLEEP_NONE);
        }
    }
    pub(super) fn xfer_locomotor_set_state(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Some(unit) = get_unit_arc(self.unit_id) {
            let mut guard = unit
                .write()
                .map_err(|_| "unit lock poisoned during locomotor xfer".to_string())?;
            let guard = &mut *guard;
            guard
                .locomotor_set
                .xfer_self_and_cur_loco_ptr(xfer, &mut guard.current_locomotor)?;
        } else {
            let mut empty_set = LocomotorSet::new();
            let mut current_locomotor = None;
            empty_set.xfer_self_and_cur_loco_ptr(xfer, &mut current_locomotor)?;
        }

        let mut current_locomotor_set = self.current_locomotor_set as i32;
        xfer.xfer_int(&mut current_locomotor_set)
            .map_err(|e| e.to_string())?;
        if xfer.is_loading() {
            self.current_locomotor_set = locomotor_set_type_from_i32(current_locomotor_set)?;
        }
        Ok(())
    }
    pub fn apply_ai_update_module_data(
        &mut self,
        data: &crate::object::update::AIUpdateModuleData,
    ) {
        self.surrender_duration_frames = data.surrender_duration_frames();
        self.auto_acquire_enemies_when_idle = data.auto_acquire_enemies_when_idle();
        self.mood_attack_check_rate_frames = data.mood_attack_check_rate();
        self.forbid_player_commands = data.forbid_player_commands();
        self.turrets_linked = data.turrets_linked();
        self.turret_primary_data = data.turret_primary().cloned();
        self.turret_secondary_data = data.turret_secondary().cloned();
        self.locomotor_sets = data.locomotor_sets().clone();

        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                let allow = (self.auto_acquire_enemies_when_idle
                    & crate::object::update::AUTO_ACQUIRE_IDLE)
                    != 0;
                let deny = (self.auto_acquire_enemies_when_idle
                    & crate::object::update::AUTO_ACQUIRE_IDLE_NO)
                    != 0;
                guard.auto_acquire_enemies = allow && !deny;
                guard.auto_acquire_while_stealthed = (self.auto_acquire_enemies_when_idle
                    & crate::object::update::AUTO_ACQUIRE_IDLE_STEALTHED)
                    != 0;
                guard.auto_acquire_not_while_attacking = (self.auto_acquire_enemies_when_idle
                    & crate::object::update::AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING)
                    != 0;
                guard.auto_acquire_attack_buildings = (self.auto_acquire_enemies_when_idle
                    & crate::object::update::AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS)
                    != 0;
                guard.mood_attack_check_rate_frames = data.mood_attack_check_rate();
            }
        }

        if let Some(mut jet_ai) = self.jet_ai.take() {
            jet_ai.on_object_created(self);
            self.jet_ai = Some(jet_ai);
        }

        if self.turret_primary_data.is_some() {
            let _ = self.ensure_turret_machine(TurretType::Primary);
        }
        if self.turret_secondary_data.is_some() {
            let _ = self.ensure_turret_machine(TurretType::Secondary);
        }

        let _ = self.choose_locomotor_set(LocomotorSetType::Normal);
    }
    pub(super) fn ensure_turret_machine(
        &mut self,
        turret: TurretType,
    ) -> Option<&mut TurretStateMachine> {
        match turret {
            TurretType::Primary => {
                if self.turret_primary_machine.is_none() {
                    self.turret_primary_machine = self.build_turret_machine(TurretType::Primary);
                }
                self.turret_primary_machine.as_mut()
            }
            TurretType::Secondary => {
                if self.turret_secondary_machine.is_none() {
                    self.turret_secondary_machine =
                        self.build_turret_machine(TurretType::Secondary);
                }
                self.turret_secondary_machine.as_mut()
            }
            TurretType::Invalid => None,
        }
    }
    pub(super) fn build_turret_machine(&self, turret: TurretType) -> Option<TurretStateMachine> {
        let unit = get_unit_arc(self.unit_id)?;
        let base_object = unit.read().ok().map(|guard| guard.base_arc())?;
        let owner = Arc::downgrade(&base_object);
        let turret_ai = Arc::new(Mutex::new(TurretAI::new(Arc::downgrade(&base_object))));
        if let Ok(mut guard) = turret_ai.lock() {
            let slot = match turret {
                TurretType::Primary => WeaponSlotType::Primary,
                TurretType::Secondary => WeaponSlotType::Secondary,
                TurretType::Invalid => WeaponSlotType::Primary,
            };
            guard.set_weapon_slot(slot);
            let mask = match slot {
                WeaponSlotType::Primary => 1u32 << 0,
                WeaponSlotType::Secondary => 1u32 << 1,
                WeaponSlotType::Tertiary => 1u32 << 2,
            };
            let data = match turret {
                TurretType::Primary => self.turret_primary_data.as_ref(),
                TurretType::Secondary => self.turret_secondary_data.as_ref(),
                TurretType::Invalid => None,
            };

            if let Some(data) = data {
                data.apply_to(&mut guard);
                if data.turret_weapon_slots == 0 {
                    error!("TurretAIData missing ControlledWeaponSlots; applying slot fallback.");
                    guard.set_turret_weapon_slots_mask(mask);
                }
            } else {
                guard.set_turret_weapon_slots_mask(mask);
            }
        }
        Some(TurretStateMachine::new(Some(turret_ai), owner, "TurretAI"))
    }
    pub(super) fn xfer_turret_ai(
        machine: &TurretStateMachine,
        xfer: &mut dyn Xfer,
    ) -> Result<(), String> {
        if let Some(turret_ai) = machine.get_turret_ai() {
            let mut guard = turret_ai
                .lock()
                .map_err(|_| "TurretAI lock poisoned during AIUpdate xfer".to_string())?;
            guard.xfer(xfer)?;
        }
        Ok(())
    }
    pub(super) fn start_rappel_state(&mut self, target_id: Option<ObjectID>) -> Result<(), String> {
        // Wave 258: empty dual-world → Ok(()).

        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let unit = get_unit_arc(self.unit_id).ok_or("unit no longer available")?;
        let base_object = unit.read().map_err(|_| "unit lock poisoned")?.base_arc();

        let mut obj_guard = base_object
            .write()
            .map_err(|_| "base object lock poisoned")?;

        if !obj_guard.is_kind_of(KindOf::CanRappel) {
            return Err("unit cannot rappel".to_string());
        }

        obj_guard.set_model_condition_state(ModelConditionFlags::RAPPELLING);

        if let Some(physics) = obj_guard.get_physics() {
            physics.reset_dynamic_physics();
        }

        let mut target_is_bldg = false;
        let mut target_valid = None;
        if let Some(target_id) = target_id {
            let is_bldg = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_id, |target_guard| {
                    !target_guard.is_effectively_dead()
                        && target_guard.is_kind_of(KindOf::Structure)
                })
                .unwrap_or(false);
            if is_bldg {
                target_is_bldg = true;
                target_valid = Some(target_id);
            }
        }

        let Some(terrain) = TheTerrainLogic::get() else {
            return Err("terrain logic unavailable".to_string());
        };

        let pos = *obj_guard.get_position();
        let layer = terrain.get_highest_layer_for_destination(&pos);
        let mut dest_z = terrain.get_layer_height(pos.x, pos.y, layer);

        if target_is_bldg {
            if let Some(target_id) = target_valid {
                if let Some(extra_z) = crate::object::registry::OBJECT_REGISTRY.with_object(
                    target_id,
                    |target_guard| {
                        target_guard
                            .get_geometry_info()
                            .get_max_height_above_position()
                    },
                ) {
                    dest_z += extra_z;
                }
            }
        } else {
            obj_guard.set_layer(layer);
            obj_guard.set_destination_layer(layer);
        }

        let max_rappel_rate = GRAVITY.abs() * (LOGICFRAMES_PER_SECOND as Real) * 2.5;
        let rappel_rate = -self.desired_speed.min(max_rappel_rate);

        self.rappel_state = Some(RappelState {
            rappel_rate,
            dest_z,
            target_is_bldg,
            target_id: target_valid,
        });

        Ok(())
    }
    pub(super) fn finish_rappel_state(&mut self) {
        let unit = get_unit_arc(self.unit_id);
        if let Some(unit) = unit {
            let base = unit.read().ok().map(|guard| guard.base_arc());
            if let Some(base) = base {
                if let Ok(mut obj_guard) = base.write() {
                    obj_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                }
            }
        }
        self.desired_speed = FAST_AS_POSSIBLE;
        self.rappel_state = None;
        if self.current_command == Some(crate::ai::AiCommandType::RappelInto) {
            self.current_command = None;
        }
    }
    pub(super) fn update_rappel_state(&mut self) {
        // Wave 258: empty dual-world → no factory object walks.

        if dual_world_registry_unavailable() {
            panic!("dual-world registry unavailable in test helper");
        }

        let Some(mut current_state) = self.rappel_state.take() else {
            return;
        };

        let Some(unit) = get_unit_arc(self.unit_id) else {
            self.finish_rappel_state();
            return;
        };

        let base_object = {
            let unit_guard = unit.read().ok();
            unit_guard.map(|guard| guard.base_arc())
        };

        let Some(base_object) = base_object else {
            self.finish_rappel_state();
            return;
        };

        let mut obj_guard = match base_object.write() {
            Ok(guard) => guard,
            Err(_) => {
                self.finish_rappel_state();
                return;
            }
        };

        if obj_guard.is_effectively_dead() {
            drop(obj_guard);
            self.finish_rappel_state();
            return;
        }

        let Some(terrain) = TheTerrainLogic::get() else {
            drop(obj_guard);
            self.finish_rappel_state();
            return;
        };

        if current_state.target_is_bldg {
            let target_gone = current_state
                .target_id
                .map(|id| {
                    crate::object::registry::OBJECT_REGISTRY
                        .with_object(id, |target_guard| {
                            target_guard.is_effectively_dead()
                                || !target_guard.is_kind_of(KindOf::Structure)
                        })
                        .unwrap_or(true)
                })
                .unwrap_or(true);
            if target_gone {
                current_state.target_is_bldg = false;
                let pos = obj_guard.get_position();
                current_state.dest_z = terrain.get_ground_height(pos.x, pos.y, None);
            }
        }

        if let Some(physics) = obj_guard.get_physics() {
            physics.scrub_velocity_2d(0.0);
            physics.scrub_velocity_z(current_state.rappel_rate);
        }

        if !current_state.target_is_bldg {
            let pos = obj_guard.get_position();
            current_state.dest_z = terrain.get_layer_height(pos.x, pos.y, obj_guard.get_layer());
        }

        let pos = *obj_guard.get_position();
        if pos.z <= current_state.dest_z {
            let mut landing = pos;
            landing.z = current_state.dest_z;
            if let Err(err) = obj_guard.set_position(&landing) {
                log::debug!(
                    "Unit::update_rappel_state failed to set landing position for {}: {}",
                    obj_guard.get_id(),
                    err
                );
            }

            if current_state.target_is_bldg {
                let target_id = current_state.target_id;
                if let Some(target_id) = target_id {
                    let max_to_kill = 2;
                    let num_killed =
                        kill_enemies_in_container(obj_guard.get_id(), target_id, max_to_kill);
                    if num_killed > 0 {
                        play_combat_drop_kill_fx(obj_guard.get_template_name(), target_id);
                    }

                    if num_killed == max_to_kill {
                        obj_guard.kill(None, None);
                    } else {
                        let extracted = crate::object::registry::OBJECT_REGISTRY.with_object(
                            target_id,
                            |target_guard| {
                                (
                                    target_guard.get_contain(),
                                    target_guard.get_orientation(),
                                    target_guard
                                        .get_geometry_info()
                                        .get_bounding_circle_radius(),
                                    *target_guard.get_position(),
                                )
                            },
                        );
                        if let Some((contain, exit_angle, target_radius, target_pos)) = extracted {
                            if let Some(contain) = contain {
                                if contain.is_valid_container_for(&obj_guard, true) {
                                    contain.add_to_contain(&obj_guard);
                                } else {
                                    let offset = obj_guard
                                        .get_geometry_info()
                                        .get_bounding_circle_radius()
                                        .min(target_radius);
                                    let angle = get_game_logic_random_value_real(PI, 2.0 * PI);
                                    let mut start_position = target_pos;
                                    start_position.x += offset * angle.cos();
                                    start_position.y += offset * angle.sin();
                                    start_position.z = terrain.get_ground_height(
                                        start_position.x,
                                        start_position.y,
                                        None,
                                    );

                                    if let Err(err) = obj_guard.set_position(&start_position) {
                                        log::debug!(
                                            "Unit::update_rappel_state failed to set start position for {}: {}",
                                            obj_guard.get_id(),
                                            err
                                        );
                                    }
                                    if let Err(err) = obj_guard.set_orientation(exit_angle) {
                                        log::debug!(
                                            "Unit::update_rappel_state failed to set exit orientation for {}: {}",
                                            obj_guard.get_id(),
                                            err
                                        );
                                    }

                                    let mut options = FindPositionOptions::default();
                                    options.start_angle = Some(1.5 * PI);
                                    options.max_radius = 200.0;
                                    let mut end_position = Coord3D::new(0.0, 0.0, 0.0);
                                    let found_position = ThePartitionManager::get()
                                        .map(|partition| {
                                            partition.find_position_around_with_options(
                                                &start_position,
                                                &options,
                                                &mut end_position,
                                            )
                                        })
                                        .unwrap_or(false);

                                    if found_position {
                                        let mut used_ai_path = false;
                                        if let Ok(unit_guard) = unit.read() {
                                            if let Some(ai) = unit_guard.get_ai_update_interface() {
                                                ai.ai_follow_path(
                                                    &[end_position],
                                                    current_state.target_id,
                                                    CommandSourceType::FromAi,
                                                );
                                                used_ai_path = true;
                                            }
                                        }
                                        if !used_ai_path {
                                            if let Ok(mut unit_guard) = unit.write() {
                                                if let Err(err) = unit_guard.give_move_order(
                                                    end_position,
                                                    Vec::new(),
                                                    false,
                                                    false,
                                                ) {
                                                    log::debug!(
                                                        "Unit::update_rappel_state give_move_order failed: {}",
                                                        err
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            drop(obj_guard);
            self.finish_rappel_state();
            return;
        }

        self.rappel_state = Some(current_state);
    }
}

/// C++ `obj->getTemplate()->getPerUnitFX("CombatDropKillFX")` then `FXList::doFXObj(fx, bldg, NULL)`.
fn play_combat_drop_kill_fx(template_name: &str, building_id: ObjectID) {
    let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() else {
        return;
    };
    let Some(factory) = guard.as_ref() else {
        return;
    };
    let Some(tmpl) = factory.find_template(template_name, false) else {
        return;
    };
    let key = "CombatDropKillFX".to_string();
    let Some(fx) = tmpl.get_per_unit_fx(&key) else {
        return;
    };
    if let Some(store_fx) = TheFXListStore::lookup_fx_list(fx.name.as_str()) {
        if let Err(err) = store_fx.do_fx_obj_ids(building_id, None, None) {
            log::debug!(
                "Unit::update_rappel_state CombatDropKillFX failed for target {}: {}",
                building_id,
                err
            );
        }
    } else {
        fx.do_fx_obj(Some(building_id), None);
    }
}
