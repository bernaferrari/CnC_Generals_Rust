//! C++ SupplyCenter/RepairDock/DockUpdate support behavior.
use super::super::super::*;

impl GameLogic {
    pub(super) fn expire_temporary_stealth_grant(&mut self, object_id: ObjectId) {
        let Some(object) = self.objects.get(&object_id) else {
            return;
        };
        let expire = object.temporary_stealth_expires_frame;
        // Host residual for C++ getLastCommandSource() == CMD_FROM_PLAYER:
        // a move/attack order after stash grant is the player-visible exploit path.
        let last_command_from_player = matches!(
            object.ai_state,
            AIState::Moving | AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
        );
        if !Object::temporary_stealth_grant_should_expire(
            expire,
            self.frame,
            last_command_from_player,
        ) {
            return;
        }
        if let Some(object) = self.objects.get_mut(&object_id) {
            // Leftover receive_grant(false): strip CAN_STEALTH even when the
            // grant latched innate_stealth. Skipping that latch left stash
            // workers permanently cloaked.
            object.revoke_grant_stealth();
        }
    }

    pub(super) fn try_claim_dock(&mut self, dock_id: ObjectId, docker_id: ObjectId) -> bool {
        let (
            current,
            template_name,
            is_repair,
            dock_kind,
            delete_when_empty,
            dock_pos,
            dock_major,
            ignore_bones,
            dock_crippled,
        ) = match self.objects.get(&dock_id) {
            Some(dock) => {
                let warehouse = dock.thing.template.dock_kind
                    == crate::game_logic::DockKind::SupplyWarehouse
                    || dock.thing.template.dock_delete_when_empty
                    || dock
                        .template_name
                        .to_ascii_lowercase()
                        .contains("supplypile");
                let body = crate::game_logic::host_enum_table_residual::host_calc_body_damage_state(
                    dock.health.current,
                    dock.health.maximum,
                );
                let crippled = warehouse
                    && matches!(
                        body,
                        crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged
                            | crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                    );
                (
                    dock.dock_active_docker,
                    dock.template_name.clone(),
                    dock.is_kind_of(crate::game_logic::KindOf::RepairPad),
                    dock.thing.template.dock_kind,
                    dock.thing.template.dock_delete_when_empty,
                    dock.get_position(),
                    if dock.thing.template.geometry_info.authored {
                        dock.thing.template.geometry_info.bounding_circle_radius()
                    } else {
                        dock.selection_radius
                    },
                    false,
                    crippled,
                )
            }
            None => return false,
        };
        let current_alive = current
            .and_then(|id| self.objects.get(&id))
            .is_some_and(|object| {
                object.is_alive()
                    && crate::game_logic::host_supply_gather::is_live_dock_ai_state(
                        &object.ai_state,
                    )
            });
        let docker_alive = self
            .objects
            .get(&docker_id)
            .is_some_and(|object| object.is_alive());
        let docker_pos = self
            .objects
            .get(&docker_id)
            .map(|o| o.get_position())
            .unwrap_or(dock_pos);
        let n = crate::game_logic::host_supply_gather::number_approach_positions_for_dock(
            &template_name,
            dock_kind,
            is_repair,
            delete_when_empty,
        );
        let waiting_bones = if ignore_bones || n <= 0 {
            Vec::new()
        } else {
            self.load_dock_waiting_bones_world(dock_id, n as usize)
        };
        let tick = crate::game_logic::host_supply_gather::tick_live_dock_approach_ex(
            dock_id,
            docker_id,
            n,
            docker_alive,
            current,
            current_alive,
            docker_pos,
            dock_pos,
            dock_major,
            &waiting_bones,
            self.frame,
            dock_crippled,
            |id| {
                self.objects.get(&id).is_some_and(|object| {
                    object.is_alive()
                        && (id == docker_id
                            || crate::game_logic::host_supply_gather::is_live_dock_ai_state(
                                &object.ai_state,
                            ))
                })
            },
        );
        match tick {
            crate::game_logic::host_supply_gather::DockApproachTick::ClearToAct => {
                if let Some(dock) = self.objects.get_mut(&dock_id) {
                    dock.dock_active_docker = Some(docker_id);
                }
                if current != Some(docker_id) {
                    self.apply_docking_model_conditions(dock_id, docker_id, true);
                }
                self.clear_worker_moving_while_docking(dock_id, docker_id);
                true
            }
            crate::game_logic::host_supply_gather::DockApproachTick::PathTo(goal) => {
                let state = self
                    .objects
                    .get(&docker_id)
                    .map(|o| o.ai_state.clone())
                    .unwrap_or(AIState::Idle);
                self.path_approach_with_state(docker_id, goal, state);
                false
            }
            crate::game_logic::host_supply_gather::DockApproachTick::TimedOut => {
                self.release_dock_if_holder(dock_id, docker_id);
                self.on_dock_wait_timeout(dock_id, docker_id);
                false
            }
            crate::game_logic::host_supply_gather::DockApproachTick::Blocked => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn try_claim_dock_for_test(
        &mut self,
        dock_id: ObjectId,
        docker_id: ObjectId,
    ) -> bool {
        self.try_claim_dock(dock_id, docker_id)
    }

    #[cfg(test)]
    pub(crate) fn supply_center_accepts_deposit_for_test(
        &self,
        center_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
    ) -> bool {
        self.supply_center_accepts_deposit(center_id, team, owner_player_id)
    }

    #[cfg(test)]
    pub(crate) fn update_support_states_for_test(&mut self, ids: &[ObjectId], dt: f32) {
        self.update_support_states(ids, dt);
    }

    #[cfg(test)]
    pub(crate) fn scan_guard_inner_target_for_test(
        &self,
        object_id: ObjectId,
        team: Team,
        scan_anchor: glam::Vec3,
        acquire_radius: f32,
        flying_only: bool,
        enter_guard: bool,
        hijack_guard: bool,
        polygon: Option<&gamelogic::polygon_trigger::PolygonTrigger>,
    ) -> Option<ObjectId> {
        self.scan_guard_inner_target(
            object_id,
            team,
            scan_anchor,
            acquire_radius,
            flying_only,
            enter_guard,
            hijack_guard,
            polygon,
        )
    }

    pub(super) fn release_dock_if_holder(&mut self, dock_id: ObjectId, docker_id: ObjectId) {
        let was_holder = self
            .objects
            .get(&dock_id)
            .is_some_and(|dock| dock.dock_active_docker == Some(docker_id));
        if let Some(dock) = self.objects.get_mut(&dock_id) {
            if dock.dock_active_docker == Some(docker_id) {
                dock.dock_active_docker = None;
            }
            if dock.repair_dock_last_id == Some(docker_id) {
                dock.repair_dock_last_id = None;
                dock.repair_dock_health_per_sec = 0.0;
            }
        }
        crate::game_logic::host_supply_gather::cancel_live_dock_approach(dock_id, docker_id);
        if was_holder {
            self.apply_docking_model_conditions(dock_id, docker_id, false);
        }
    }

    /// C++ `AIDockState::onExit` → `AIDockMachine::halt` → `DockUpdate::cancelDock`.
    pub(crate) fn cancel_dock_reservation(&mut self, docker_id: ObjectId) {
        let mut docks = Vec::new();
        if let Some(obj) = self.objects.get(&docker_id) {
            if let Some(id) = obj.preferred_dock_id {
                docks.push(id);
            }
            if let Some(id) = obj.target {
                if !docks.contains(&id) {
                    docks.push(id);
                }
            }
        }
        for (id, obj) in &self.objects {
            if (obj.dock_active_docker == Some(docker_id)
                || obj.repair_dock_last_id == Some(docker_id))
                && !docks.contains(id)
            {
                docks.push(*id);
            }
        }
        for dock_id in docks {
            self.release_dock_if_holder(dock_id, docker_id);
        }
        crate::game_logic::host_supply_gather::cancel_live_dock_for_docker(docker_id);
    }

    /// C++ `RepairDockUpdate::isRallyPointAfterDockType` + `AIDockMoveToRallyState`.
    pub(super) fn send_to_rally_after_repair_dock(
        &mut self,
        docker_id: ObjectId,
        dock_id: ObjectId,
    ) {
        let Some(dock) = self.objects.get(&dock_id) else {
            return;
        };
        if !dock.is_kind_of(KindOf::RepairPad) {
            return;
        }
        let Some(rally) = dock.building_data.as_ref().and_then(|b| b.rally_point) else {
            return;
        };
        self.path_approach_with_state(docker_id, rally, AIState::Moving);
    }

    /// C++ `onEnterReached` / `onDockReached` / `onExitReached` MODELCONDITION_DOCKING*.
    fn apply_docking_model_conditions(
        &mut self,
        dock_id: ObjectId,
        docker_id: ObjectId,
        entering: bool,
    ) {
        use crate::game_logic::host_enum_table_residual::{
            docking_active_model_bit, docking_beginning_model_bit, docking_ending_model_bit,
            model_condition_bit_name_index,
        };
        let beginning = docking_beginning_model_bit();
        let active = docking_active_model_bit();
        let ending = docking_ending_model_bit();
        let docking = model_condition_bit_name_index("DOCKING").unwrap_or(0) as u32;
        for id in [dock_id, docker_id] {
            if let Some(obj) = self.objects.get_mut(&id) {
                if entering {
                    obj.model_condition_bits &= !(1u128 << ending);
                    obj.model_condition_bits |= 1u128 << beginning;
                    obj.model_condition_bits |= 1u128 << docking;
                    obj.model_condition_bits |= 1u128 << active;
                } else {
                    obj.model_condition_bits &= !(1u128 << beginning);
                    obj.model_condition_bits &= !(1u128 << docking);
                    obj.model_condition_bits &= !(1u128 << active);
                    obj.model_condition_bits |= 1u128 << ending;
                }
                obj.record_host_model_condition();
            }
        }
        if entering {
            self.clear_worker_moving_while_docking(dock_id, docker_id);
        }
    }

    /// C++ `DockUpdate::update` Worker MOVING clear at a supply-source dock.
    fn clear_worker_moving_while_docking(&mut self, dock_id: ObjectId, docker_id: ObjectId) {
        use crate::game_logic::host_enum_table_residual::{
            docking_beginning_model_bit, moving_model_bit,
        };
        let dock_is_supply = self
            .objects
            .get(&dock_id)
            .is_some_and(|dock| dock.is_kind_of(KindOf::SupplySource));
        if !dock_is_supply {
            return;
        }
        let beginning = docking_beginning_model_bit();
        let moving = moving_model_bit();
        let Some(obj) = self.objects.get_mut(&docker_id) else {
            return;
        };
        if obj.is_kind_of(KindOf::Dozer) && obj.is_kind_of(KindOf::Harvester) {
            if (obj.model_condition_bits & (1u128 << beginning)) != 0 {
                obj.model_condition_bits &= !(1u128 << moving);
                obj.record_host_model_condition();
            }
        }
    }

    /// Pristine `DockWaiting01..NN` in host world space.
    fn load_dock_waiting_bones_world(&self, dock_id: ObjectId, max: usize) -> Vec<glam::Vec3> {
        let Some(dock) = self.objects.get(&dock_id) else {
            return Vec::new();
        };
        let model = dock.thing.template.get_model_name();
        let scale = dock.thing.template.asset_scale;
        let pos = dock.get_position();
        let yaw = dock.get_orientation();
        let (sin, cos) = yaw.sin_cos();
        let mut out = Vec::new();
        for i in 1..=max.max(1) {
            let name = format!("DockWaiting{i:02}");
            let Some(local) =
                gamelogic::object::draw::lookup_pristine_bone_translation(model, scale, &name)
            else {
                break;
            };
            let host_local = glam::Vec3::new(local.x, local.z, local.y);
            out.push(glam::Vec3::new(
                pos.x + host_local.x * cos - host_local.z * sin,
                pos.y + host_local.y,
                pos.z + host_local.x * sin + host_local.z * cos,
            ));
        }
        out
    }

    fn on_dock_wait_timeout(&mut self, dock_id: ObjectId, docker_id: ObjectId) {
        let Some((state, team, owner, pos)) = self.objects.get(&docker_id).map(|o| {
            (
                o.ai_state.clone(),
                o.team,
                o.owner_player_id,
                o.get_position(),
            )
        }) else {
            return;
        };
        match state {
            AIState::Gathering => {
                let scan = self.collector_warehouse_scan(docker_id, owner);
                if let Some(next) =
                    self.find_nearest_harvestable_supply_within(team, pos, scan, docker_id)
                {
                    if next != dock_id {
                        if let Some(dest) = self.objects.get(&next).map(|s| s.get_position()) {
                            if let Some(obj) = self.objects.get_mut(&docker_id) {
                                obj.set_target(Some(next));
                            }
                            self.path_approach_with_state(docker_id, dest, AIState::Gathering);
                            return;
                        }
                    }
                }
                self.begin_supply_regroup(docker_id, team, owner, pos);
            }
            AIState::ReturningResources => {
                if let Some(obj) = self.objects.get_mut(&docker_id) {
                    obj.supply_truck_state = SupplyTruckState::Wanting;
                }
                self.begin_supply_regroup(docker_id, team, owner, pos);
            }
            _ => {}
        }
    }

    pub(crate) fn try_aircraft_land_for_repair(
        &mut self,
        unit_id: ObjectId,
        airfield_id: ObjectId,
    ) {
        if self.try_jet_enter_or_repair_airfield(unit_id, airfield_id) {
            return;
        }
        let Some(pos) = self.objects.get(&airfield_id).map(|a| a.get_position()) else {
            return;
        };
        let helipad = self
            .objects
            .get(&unit_id)
            .is_some_and(Self::object_is_produced_at_helipad);
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            if let Some(ai) = unit.chinook_ai.as_mut() {
                ai.command_repair([pos.x, pos.z, pos.y], airfield_id.0);
                return;
            }
            if helipad {
                unit.return_to_base_requested = true;
                if unit.producer_id.is_none() {
                    unit.producer_id = Some(airfield_id);
                }
            }
        }
        if helipad {
            let _ = self.try_return_to_base_rearm(unit_id);
        }
    }
    /// C++ `RepairDockUpdate::action` drone snap-to-max via `findMyDrone`.
    pub(super) fn heal_slave_drone_with_repair_dock(&mut self, master_id: ObjectId) {
        let drone_id = self.objects.iter().find_map(|(id, obj)| {
            (obj.is_alive() && obj.is_kind_of(KindOf::Drone) && obj.producer_id == Some(master_id))
                .then_some(*id)
        });
        let Some(drone_id) = drone_id else {
            return;
        };
        if let Some(drone) = self.objects.get_mut(&drone_id) {
            let max = drone.health.maximum;
            drone.heal(max);
        }
    }

    pub(super) fn repair_dock_rate_for_docker(
        &mut self,
        pad_id: ObjectId,
        docker_id: ObjectId,
        max_hp: f32,
        current_hp: f32,
    ) -> f32 {
        let need_recompute = self.objects.get(&pad_id).is_some_and(|pad| {
            pad.repair_dock_last_id != Some(docker_id) || pad.repair_dock_health_per_sec <= 0.0
        });
        if !need_recompute {
            return self
                .objects
                .get(&pad_id)
                .map(|pad| pad.repair_dock_health_per_sec)
                .unwrap_or(0.0);
        }
        let rate =
            crate::game_logic::host_repair::repair_dock_hp_per_sec_from_missing(max_hp, current_hp);
        if let Some(pad) = self.objects.get_mut(&pad_id) {
            pad.repair_dock_last_id = Some(docker_id);
            pad.repair_dock_health_per_sec = rate;
        }
        rate
    }

    pub(super) fn grant_center_temporary_stealth(
        &mut self,
        center_id: ObjectId,
        docker_id: ObjectId,
    ) {
        let Some(center) = self.objects.get(&center_id) else {
            return;
        };
        let grant_frames =
            crate::game_logic::host_supply_gather::grant_temporary_stealth_frames_for_center(
                &center.template_name,
            );
        self.apply_temporary_stealth_grant(center_id, docker_id, grant_frames);
    }

    /// C++ `SupplyCenterProductionExitUpdate::exitObjectViaDoor` after Wanting:
    /// grant when the producer is STEALTHED and (`isTemporaryGrant` or the
    /// new unit lacks `CAN_STEALTH`) so GPS / innate stealth wins.
    pub(crate) fn grant_supply_center_exit_temporary_stealth(
        &mut self,
        producer_id: ObjectId,
        new_id: ObjectId,
    ) {
        let Some(producer) = self.objects.get(&producer_id) else {
            return;
        };
        let Some(exit) = producer.thing.template.production_exit_metadata else {
            return;
        };
        if !exit.is_supply_center() {
            return;
        }
        self.apply_temporary_stealth_grant(
            producer_id,
            new_id,
            exit.grant_temporary_stealth_frames,
        );
    }

    fn apply_temporary_stealth_grant(
        &mut self,
        center_id: ObjectId,
        unit_id: ObjectId,
        grant_frames: u32,
    ) {
        let Some(center) = self.objects.get(&center_id) else {
            return;
        };
        let center_stealthed = center.status.stealthed;
        let Some(unit) = self.objects.get(&unit_id) else {
            return;
        };
        let unit_is_temp = unit.temporary_stealth_expires_frame > self.frame;
        let unit_can_stealth = unit.innate_stealth || unit.status.stealthed;
        if !crate::game_logic::host_supply_gather::should_grant_temporary_stealth(
            center_stealthed,
            grant_frames,
            unit_is_temp,
            unit_can_stealth,
        ) {
            return;
        }
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.apply_grant_stealth();
            unit.temporary_stealth_expires_frame = self.frame.saturating_add(grant_frames);
        }
    }

    /// C++ ResourceManager + SupplyCenterDock: same controlling player only.
    pub(super) fn preferred_or_allied_supply_center(
        &self,
        collector_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        from_position: glam::Vec3,
    ) -> Option<ObjectId> {
        let preferred = self
            .objects
            .get(&collector_id)
            .and_then(|collector| collector.preferred_dock_id);
        if let Some(center_id) = preferred {
            if self.supply_center_accepts_deposit(center_id, team, owner_player_id) {
                return Some(center_id);
            }
        }
        let mut best: Option<(f32, ObjectId)> = None;
        for (&id, obj) in &self.objects {
            if !self.supply_center_accepts_deposit(id, team, owner_player_id) {
                continue;
            }
            let d = from_position.distance(obj.get_position());
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, id));
            }
        }
        best.map(|(_, id)| id)
    }

    fn supply_center_accepts_deposit(
        &self,
        center_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
    ) -> bool {
        let Some(center) = self.objects.get(&center_id) else {
            return false;
        };
        if !center.is_alive() || !center.is_constructed() {
            return false;
        }
        let is_center = center.thing.template.dock_kind
            == crate::game_logic::DockKind::SupplyCenter
            || center.is_kind_of(KindOf::SupplyCenter)
            || center.thing.template.has_supply_center_create;
        if !is_center {
            return false;
        }
        let center_owner = self.player_owner_for_host_object(center);
        if center_owner.is_some() && center_owner == owner_player_id {
            return true;
        }
        let _ = team;
        false
    }
}
