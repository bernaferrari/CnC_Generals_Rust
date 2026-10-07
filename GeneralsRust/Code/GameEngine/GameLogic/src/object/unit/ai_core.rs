//! UnitAIUpdate struct, construction, turrets, and rappel state.

#![allow(unused_imports)]

use super::ai_data::UnitAiData;
use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{dual_world_registry_unavailable, get_unit_arc};
use super::types::*;
use crate::object::update::ai_update_interface::AIUpdateModuleData;

/// Specialized runtimes selected from one Object's authored modules.
/// They are prepared together before the runtime is published to callbacks.
pub(crate) struct UnitAiComponents {
    pub(crate) supply_truck_ai: Option<SupplyTruckAIUpdate>,
    pub(crate) chinook_ai: Option<ChinookAIUpdate>,
    pub(crate) jet_ai: Option<JetAIUpdate>,
    pub(crate) worker_ai: Option<WorkerAIUpdate>,
    pub(crate) dozer_ai: Option<DozerAIUpdate>,
    #[cfg(feature = "allow_surrender")]
    pub(crate) pow_truck_ai: Option<POWTruckAIUpdate>,
    pub(crate) railed_transport_ai: Option<RailedTransportAIUpdate>,
    pub(crate) hack_internet_ai: Option<HackInternetAIUpdate>,
    pub(crate) assault_transport_ai: Option<AssaultTransportAIUpdate>,
    pub(crate) deliver_payload_ai: Option<DeliverPayloadAIUpdate>,
    pub(crate) transport_ai: Option<TransportAIUpdate>,
    pub(crate) deploy_style_ai: Option<DeployStyleAIUpdate>,
    pub(crate) wander_ai: Option<WanderAIUpdate>,
}

/// Basic AI update interface that bridges AI commands to unit orders.
pub struct UnitAIUpdate {
    /// Owning Object ID. Legacy order/pose paths still use UNIT_REGISTRY;
    /// locomotors, current victim and mood timer belong to this runtime.
    pub(super) unit_id: ObjectID,
    /// Constructor-bound identity, never reselected by a global ID lookup.
    pub(super) owner: Option<Weak<RwLock<Object>>>,
    /// C++ AIUpdateInterface::m_currentVictimID; owned by this AI runtime.
    pub(super) current_victim_id: ObjectID,
    pub(super) crate_created: ObjectID,
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
    pub(super) data: UnitAiData,
}

impl std::fmt::Debug for UnitAIUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnitAIUpdate")
            .field("can_path_through_units", &self.data.can_path_through_units)
            .field("allow_chase", &self.data.allow_chase)
            .field("last_command_source", &self.data.last_command_source)
            .field("current_command", &self.data.current_command)
            .field("pending_command", &self.data.pending_command)
            .field("ai_dead", &self.data.ai_dead)
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
        let owner = get_unit_arc(unit_id).and_then(|unit| {
            unit.read()
                .ok()
                .map(|unit| Arc::downgrade(&unit.base_arc()))
        });
        Self::from_components(
            unit_id,
            owner,
            UnitAiComponents {
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
            },
        )
    }

    /// The factory already has the exact Object. No legacy registry selects it.
    pub(crate) fn new_for_object(
        unit_id: ObjectID,
        owner: &Arc<RwLock<Object>>,
        components: UnitAiComponents,
    ) -> Self {
        Self::from_components(unit_id, Some(Arc::downgrade(owner)), components)
    }

    fn from_components(
        unit_id: ObjectID,
        owner: Option<Weak<RwLock<Object>>>,
        components: UnitAiComponents,
    ) -> Self {
        let ai_state_machine = owner.as_ref().map(|owner| {
            Arc::new(Mutex::new(AIStateMachine::new(
                owner.clone(),
                "AIStateMachine",
            )))
        });

        Self {
            unit_id,
            owner,
            current_victim_id: INVALID_ID,
            crate_created: crate::common::INVALID_ID,
            supply_truck_ai: components.supply_truck_ai,
            chinook_ai: components.chinook_ai,
            jet_ai: components.jet_ai,
            worker_ai: components.worker_ai,
            dozer_ai: components.dozer_ai,
            #[cfg(feature = "allow_surrender")]
            pow_truck_ai: components.pow_truck_ai,
            railed_transport_ai: components.railed_transport_ai,
            hack_internet_ai: components.hack_internet_ai,
            assault_transport_ai: components.assault_transport_ai,
            deliver_payload_ai: components.deliver_payload_ai,
            transport_ai: components.transport_ai,
            deploy_style_ai: components.deploy_style_ai,
            wander_ai: components.wander_ai,
            dock_machine: None,
            ai_state_machine,
            data: UnitAiData {
                can_path_through_units: false,
                randomly_offset_mood_check: false,
                next_mood_check_time: 0,
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
                current_locomotor_set: LocomotorSetType::Invalid,
                locomotor_set: LocomotorSet::new(),
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
                installed_path_layers: Vec::new(),
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
                cpp_is_moving: false,
                locomotor_goal_type: 0,
                locomotor_goal_data: Coord3D::ZERO,
                is_blocked: false,
                blocked_and_stuck: false,
                retry_path: false,
                blocked_frames: 0,
                cur_max_blocked_speed: FAST_AS_POSSIBLE,
                bump_speed_limit: FAST_AS_POSSIBLE,
            },
        }
    }
    /// Enter `state_id`. The machine stays on `self` so `on_enter` can see it.
    /// Do not `take` it and do not swap in an empty shell: `on_enter` writes
    /// through the `Arc` other owners hold, and a shell would discard those writes.
    pub(super) fn enter_ai_state(&mut self, state_id: u32) {
        let Some(state_machine) = self.ai_state_machine.clone() else {
            return;
        };
        let mut guard = state_machine.lock().unwrap_or_else(|err| err.into_inner());
        let _ = guard.base.set_state_entering(state_id, Some(self));
    }
    pub(super) fn push_guard_target_type(&mut self, target_type: GuardTargetType) {
        if self.data.guard_target_type[1] == GuardTargetType::None_ {
            self.data.guard_target_type[1] = target_type;
        } else {
            self.data.guard_target_type[0] = target_type;
        }
    }
    pub(super) fn clear_guard_target_type(&mut self) {
        self.data.guard_target_type[1] = self.data.guard_target_type[0];
        self.data.guard_target_type[0] = GuardTargetType::None_;
    }
    pub(super) fn friend_get_turret_sync(&self) -> TurretType {
        self.data.turret_sync_flag
    }
    pub(super) fn friend_set_turret_sync(&mut self, turret: TurretType) {
        self.data.turret_sync_flag = turret;
    }
    pub(super) fn owner_object_id(&self) -> Option<ObjectID> {
        if self.unit_id != INVALID_ID {
            Some(self.unit_id)
        } else {
            None
        }
    }
    pub(super) fn wake_up_now(&self) {
        let Some(owner_id) = self.owner_object_id() else {
            return;
        };
        let now = TheGameLogic::get_frame();
        if let Some(object) = crate::object::registry::OBJECT_REGISTRY.get_object(owner_id) {
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
    pub fn apply_ai_update_module_data(
        &mut self,
        data: &crate::object::update::AIUpdateModuleData,
    ) {
        self.apply_module_data(data, true);
    }

    pub(crate) fn apply_factory_ai_update_module_data(&mut self, data: &AIUpdateModuleData) {
        self.apply_module_data(data, false);
    }

    fn apply_module_data(&mut self, data: &AIUpdateModuleData, legacy_unit: bool) {
        self.data.surrender_duration_frames = data.surrender_duration_frames();
        self.data.auto_acquire_enemies_when_idle = data.auto_acquire_enemies_when_idle();
        self.data.mood_attack_check_rate_frames = data.mood_attack_check_rate();
        self.data.forbid_player_commands = data.forbid_player_commands();
        self.data.turrets_linked = data.turrets_linked();
        self.data.turret_primary_data = data.turret_primary().cloned();
        self.data.turret_secondary_data = data.turret_secondary().cloned();
        self.data.locomotor_sets = data.locomotor_sets().clone();

        if legacy_unit {
            if let Some(unit) = get_unit_arc(self.unit_id) {
                if let Ok(mut guard) = unit.write() {
                    let allow = (self.data.auto_acquire_enemies_when_idle
                        & crate::object::update::AUTO_ACQUIRE_IDLE)
                        != 0;
                    let deny = (self.data.auto_acquire_enemies_when_idle
                        & crate::object::update::AUTO_ACQUIRE_IDLE_NO)
                        != 0;
                    guard.auto_acquire_enemies = allow && !deny;
                    guard.auto_acquire_while_stealthed = (self.data.auto_acquire_enemies_when_idle
                        & crate::object::update::AUTO_ACQUIRE_IDLE_STEALTHED)
                        != 0;
                    guard.auto_acquire_not_while_attacking =
                        (self.data.auto_acquire_enemies_when_idle
                            & crate::object::update::AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING)
                            != 0;
                    guard.auto_acquire_attack_buildings =
                        (self.data.auto_acquire_enemies_when_idle
                            & crate::object::update::AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS)
                            != 0;
                    guard.mood_attack_check_rate_frames = data.mood_attack_check_rate();
                }
            }
        }
        if let Some(mut jet_ai) = self.jet_ai.take() {
            jet_ai.on_object_created(self);
            self.jet_ai = Some(jet_ai);
        }

        if self.data.turret_primary_data.is_some() {
            let _ = self.ensure_turret_machine(TurretType::Primary);
        }
        if self.data.turret_secondary_data.is_some() {
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
        let unit = get_unit_arc(self.unit_id)?;
        let owner_id = unit
            .read()
            .ok()
            .and_then(|guard| guard.base_arc().read().ok().map(|obj| obj.get_id()))
            .unwrap_or(crate::common::INVALID_ID);
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
        let data = match turret {
            TurretType::Primary => self.data.turret_primary_data.as_ref(),
            TurretType::Secondary => self.data.turret_secondary_data.as_ref(),
            TurretType::Invalid => None,
        };

        if let Some(data) = data {
            data.apply_to(&mut turret_ai);
            if data.turret_weapon_slots == 0 {
                error!("TurretAIData missing ControlledWeaponSlots; applying slot fallback.");
                turret_ai.set_turret_weapon_slots_mask(mask);
            }
        } else {
            turret_ai.set_turret_weapon_slots_mask(mask);
        }
        Some(TurretStateMachine::new(turret_ai))
    }
    pub(super) fn xfer_turret_ai(
        machine: &mut TurretStateMachine,
        xfer: &mut dyn Xfer,
    ) -> Result<(), String> {
        machine.turret_mut().xfer(xfer)
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
    pub(super) fn update_rappel_state(&mut self) {
        let Some(mut state) = self.data.rappel_state.take() else {
            return;
        };
        let Some(owner) = self.rappel_owner() else {
            self.finish_rappel_state();
            return;
        };
        let facts = owner.read().ok().map(|obj| {
            (
                obj.is_effectively_dead(),
                *obj.get_position(),
                obj.get_layer(),
                obj.get_physics(),
            )
        });
        let Some((false, pos, layer, physics)) = facts else {
            self.finish_rappel_state();
            return;
        };
        let Some(terrain) = TheTerrainLogic::get() else {
            self.finish_rappel_state();
            return;
        };

        // C++ AIStates.cpp:527-541: validate the target, then scrub velocities.
        // Neither target lookup nor physics callback retains an owner guard.
        if state.target_is_bldg {
            let gone = state
                .target_id
                .and_then(|id| {
                    OBJECT_REGISTRY.with_object(id, |target| target.is_effectively_dead())
                })
                .unwrap_or(true);
            if gone {
                state.target_is_bldg = false;
                state.dest_z = terrain.get_ground_height(pos.x, pos.y, None);
            }
        }
        if let Some(physics) = physics {
            physics.scrub_velocity_2d(0.0);
            physics.scrub_velocity_z(state.rappel_rate);
        }
        if !state.target_is_bldg {
            state.dest_z = terrain.get_layer_height(pos.x, pos.y, layer);
        }
        if pos.z > state.dest_z {
            self.data.rappel_state = Some(state);
            return;
        }

        let mut landing = pos;
        landing.z = state.dest_z;
        if let Ok(mut obj) = owner.write() {
            if let Err(err) = obj.set_position(&landing) {
                log::debug!(
                    "Unit::update_rappel_state landing failed for {}: {}",
                    self.unit_id,
                    err
                );
            }
        }
        if state.target_is_bldg {
            if let Some(target_id) = state.target_id {
                // C++ AIStates.cpp:562-584: score/kill callbacks can read and
                // mutate the rappeller; no caller-held owner guard spans them.
                let max_to_kill = 2;
                let killed = kill_enemies_in_container(self.unit_id, target_id, max_to_kill);
                if killed > 0 {
                    let name = owner
                        .read()
                        .ok()
                        .map(|obj| obj.get_template_name().to_string());
                    if let Some(name) = name {
                        play_combat_drop_kill_fx(&name, target_id);
                    }
                }
                if killed == max_to_kill {
                    if let Ok(mut obj) = owner.write() {
                        obj.kill(None, None);
                    }
                } else {
                    let target = OBJECT_REGISTRY.with_object(target_id, |target| {
                        (
                            target.get_contain(),
                            target.get_orientation(),
                            target.get_geometry_info().get_bounding_circle_radius(),
                            *target.get_position(),
                        )
                    });
                    if let Some((contain, exit_angle, target_radius, target_pos)) = target {
                        let valid = contain.as_ref().is_some_and(|contain| {
                            owner
                                .read()
                                .ok()
                                .is_some_and(|obj| contain.is_valid_container_for(&obj, true))
                        });
                        if valid {
                            // Installed Open/Transport/Garrison addToContain
                            // delegates to this same ID operation. End the
                            // validation borrow before immediate enter effects.
                            if let Some(contain) = contain {
                                if let Ok(mut contain) = contain.lock() {
                                    if let Err(err) = contain.contain_object(self.unit_id) {
                                        log::debug!(
                                            "Rappel containment failed for {}: {}",
                                            self.unit_id,
                                            err
                                        );
                                    }
                                }
                            }
                        } else {
                            // C++ also scatters when the building has no contain.
                            let radius = owner
                                .read()
                                .ok()
                                .map(|obj| obj.get_geometry_info().get_bounding_circle_radius())
                                .unwrap_or(0.0);
                            let offset = radius.min(target_radius);
                            let angle = get_game_logic_random_value_real(PI, 2.0 * PI);
                            let mut start = target_pos;
                            start.x += offset * angle.cos();
                            start.y += offset * angle.sin();
                            start.z = terrain.get_ground_height(start.x, start.y, None);
                            if let Ok(mut obj) = owner.write() {
                                if let Err(err) = obj.set_position(&start) {
                                    log::debug!(
                                        "Rappel scatter position failed for {}: {}",
                                        self.unit_id,
                                        err
                                    );
                                }
                                if let Err(err) = obj.set_orientation(exit_angle) {
                                    log::debug!(
                                        "Rappel scatter orientation failed for {}: {}",
                                        self.unit_id,
                                        err
                                    );
                                }
                            }
                            let mut options = FindPositionOptions::default();
                            options.start_angle = Some(1.5 * PI);
                            options.max_radius = 200.0;
                            let mut end = Coord3D::ZERO;
                            let found = ThePartitionManager::get()
                                .map(|partition| {
                                    partition.find_position_around_with_options(
                                        &start, &options, &mut end,
                                    )
                                })
                                .unwrap_or(false);
                            if found {
                                // Invoke this executing AI directly; cloning its
                                // cached mutex would reenter/skip this command.
                                let mut command = crate::ai::AiCommandParams::new(
                                    crate::ai::AiCommandType::FollowPath,
                                    CommandSourceType::FromAi,
                                );
                                command.coords.push(end);
                                command.obj = state.target_id;
                                if let Err(err) = self.execute_command(&command) {
                                    log::warn!(
                                        "Rappel exit path requires its driving Unit for {}: {}",
                                        self.unit_id,
                                        err
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        self.finish_rappel_state();
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
