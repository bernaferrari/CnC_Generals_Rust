//! Order target, capture, supply, contain, weapon-set and upgrade-queue orders.

use super::*;

impl GameLogic {
    /// Wave 233: set order target (records host attack/order log residual).
    pub fn unit_command_set_order_target(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target_with_operation(
            target,
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        let can_construct = unit.can_construct();
        if can_construct {
            if let Some(sid) = target {
                if let Some(st) = self.objects.get_mut(&sid) {
                    if st.status.under_construction {
                        // C++ Object::setBuilder (DozerAIUpdate.cpp:1677 / 1986).
                        st.builder_id = Some(id);
                    }
                }
            }
        }
        true
    }

    /// Wave 233: stop moving then set order target (enter/gather/dock residual).
    pub fn unit_command_stop_moving_order_target(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> gamelogic::GameLogicResult<bool> {
        let should_enter_idle = {
            let Some(unit) = self.objects.get_mut(&id) else {
                return Ok(false);
            };
            unit.stop_moving_for_owned_idle_entry(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ))
        };
        if should_enter_idle && !self.enter_base_ai_idle_owned(id, &mut Default::default())? {
            return Ok(false);
        }
        let frame = self.frame;
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        unit.set_order_target_with_operation(
            target,
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        Ok(true)
    }

    /// Begin the C++ `SpecialAbilityUpdate` capture intent.  This records an
    /// order and starts approach only; `markSpecialPowerTriggered` belongs to
    /// the later preparation phase once the source reaches StartAbilityRange.
    pub fn unit_command_begin_capture(&mut self, id: ObjectId, target: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() || !unit.can_move() {
            return false;
        }
        unit.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.set_order_target_with_operation(
            Some(target),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        unit.capture_channel = None;
        unit.set_status_using_ability(false);
        unit.set_ai_state(
            AIState::Capturing,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// Begin a parsed Hacker Disable Building `SpecialAbilityUpdate` intent.
    ///
    /// Like capture, this records only the player-authorized target and
    /// resets an interrupted channel.  It deliberately does not spend the
    /// paired SpecialPower charge: C++ starts that reload in
    /// `startPreparation`, after the hacker reaches its authored range and
    /// finishes unpacking.
    pub fn unit_command_begin_hacker_disable_building(
        &mut self,
        id: ObjectId,
        target: ObjectId,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.set_order_target_with_operation(
            Some(target),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        unit.hacker_disable_channel = Some(crate::game_logic::HackerDisableChannelState::new(
            target,
            crate::game_logic::HackerDisableChannelPhase::Approaching,
            0,
        ));
        unit.set_status_using_ability(false);
        unit.set_ai_state(
            AIState::SpecialAbility,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
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
        unit.set_orientation_with_partition_owner(orientation, &mut self.partition_manager);
        true
    }

    /// Set the canonical building rally point at the receiving command phase.
    pub fn unit_command_set_rally_point(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        self.unit_command_set_rally_point_with_effects(id, location, &mut Default::default())
    }
    pub(crate) fn unit_command_set_rally_point_with_effects(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        // C++ ACTIONTYPE_SET_RALLY_POINT: KINDOF_AUTO_RALLYPOINT only.
        if !obj.is_kind_of(crate::game_logic::KindOf::AutoRallypoint) {
            return false;
        }
        // C++ GameLogicDispatch.cpp:156-161 format(GUI:RallyPointSet, displayName).
        let display_name = obj.get_display_name();
        let from = obj.get_position();
        if !self
            .pathfinding_system
            .grid
            .quick_path_exists(from, location)
        {
            if let Some(ui) = effects.ui.as_deref_mut() {
                ui.hud_message("GUI:RallyPointNoPath", self.get_frame());
            }
            self.play_ui_sound("UnableToSetRallyPoint");
            return false;
        }
        #[cfg(feature = "game_client")]
        {
            let formatted = format_rally_point_set_message(
                &game_client::game_text::GameText::fetch("GUI:RallyPointSet"),
                &display_name,
            );
            if let Some(ui) = effects.ui.as_deref_mut() {
                ui.hud_message(&formatted, self.get_frame());
            }
        }
        self.play_ui_sound("RallyPointSet");
        // C++ GameLogicDispatch.cpp:177-183 resolves and updates the winning
        // ExitInterface only after the feedback/UI/audio side effects above.
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.set_rally_point_for_exit(location);
        } else {
            return false;
        }
        true
    }

    /// Wave 233: return-supplies order target + ReturningResources state.
    pub fn unit_command_return_supplies(&mut self, id: ObjectId, supply_center: ObjectId) -> bool {
        if !self.objects.get(&supply_center).is_some_and(|dock| dock.is_alive()) {
            return false;
        }
        self.unit_command_ai_dock_supply_center(id, supply_center)
    }

    /// AIPlayer.cpp passes the resolved object to aiDock even if it is dying.
    pub(crate) fn unit_command_ai_dock_supply_center(
        &mut self,
        id: ObjectId,
        supply_center: ObjectId,
    ) -> bool {
        if self.objects.get(&supply_center).is_none() {
            return false;
        }
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.cancel_dock_reservation(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.preferred_dock_id = Some(supply_center);
            unit.supply_truck_state = crate::game_logic::SupplyTruckState::Wanting;
            unit.supply_truck_force_pending = true;
            unit.set_order_target_with_operation(
                Some(supply_center),
                &mut self.attack_events,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                self.frame,
                &mut crate::game_logic::ScriptPresentationEffects::default()
                    .reborrow_for_object(&mut self.partition_manager),
            );
            unit.set_ai_state(
                AIState::ReturningResources,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.ignored_obstacle_id = None;
        } else {
            return false;
        }
        // C++ aiDock: DockUpdate approach. Do not path at the center or ignore it.
        let _ = self.try_claim_dock(supply_center, id);
        true
    }

    /// C++ `SupplyTruckAIUpdate::privateDock` / `WorkerAIUpdate::privateDock`
    /// against `SupplyWarehouseDockUpdate`: remember the chosen warehouse and
    /// enter the collection loop.  This is intentionally not `Entering` — a
    /// warehouse grants supply boxes and never becomes the unit's container.
    pub fn unit_command_dock_at_supply_warehouse(
        &mut self,
        id: ObjectId,
        warehouse: ObjectId,
    ) -> bool {
        self.drop_jet_targeters_on_attack_exit(id);
        if !self
            .objects
            .get(&warehouse)
            .is_some_and(|dock| dock.is_alive())
        {
            return false;
        }
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.cancel_dock_reservation(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            unit.preferred_dock_id = Some(warehouse);
            unit.supply_truck_state = crate::game_logic::SupplyTruckState::Wanting;
            unit.supply_truck_force_pending = true;
            unit.set_order_target_with_operation(
                Some(warehouse),
                &mut self.attack_events,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                self.frame,
                &mut crate::game_logic::ScriptPresentationEffects::default()
                    .reborrow_for_object(&mut self.partition_manager),
            );
            unit.set_ai_state(
                AIState::Gathering,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.ignored_obstacle_id = None;
        } else {
            return false;
        }
        let _ = self.try_claim_dock(warehouse, id);
        true
    }

    /// C++ `RailedTransportDockUpdate` dispatches through `AI_DOCK`, not the
    /// generic Enter command.  The support-state loader may ultimately use
    /// the shared containment storage, but the command and legality stay
    /// distinct so normal transports do not accidentally qualify as docks.
    pub fn unit_command_dock_at_railed_transport(
        &mut self,
        id: ObjectId,
        transport: ObjectId,
    ) -> bool {
        self.drop_jet_targeters_on_attack_exit(id);
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.cancel_dock_reservation(id);
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.set_order_target_with_operation(
            Some(transport),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        unit.set_ai_state(
            AIState::Docking,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// C++ `RailedTransportAIUpdate::privateEvacuate` → dock `unloadAll`.
    /// Refuses in-transit / already loading-or-unloading. Does not walk ExitStart.
    pub fn railed_transport_unload_all(&mut self, ferry_id: ObjectId) -> bool {
        self.railed_transport_unload_all_with_effects(ferry_id, &mut Default::default())
    }

    pub(crate) fn railed_transport_unload_all_with_effects(
        &mut self,
        ferry_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(ferry) = self.objects.get(&ferry_id) else {
            return false;
        };
        if !ferry.is_railed_transport() {
            return false;
        }
        if ferry.railed_in_transit {
            return false;
        }
        if ferry.dock_active_docker.is_some() {
            return false;
        }
        let passengers = ferry.contained_units();
        if passengers.is_empty() {
            return false;
        }
        let hull = ferry.get_position();
        let yaw = ferry.get_orientation();
        let (sin, cos) = yaw.sin_cos();
        let dest = glam::Vec3::new(hull.x + 20.0 * cos, hull.y, hull.z + 20.0 * sin);
        let first = passengers[0];
        for pid in passengers {
            if let Some(c) = self.objects.get_mut(&ferry_id) {
                let _ = c.remove_occupant_with_partition_owner(
                    pid,
                    &mut self.ownership_events,
                    &mut self.partition_manager,
                );
            }
            if let Some(p) = self.objects.get_mut(&pid) {
                let containment_actor_id = p.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    &mut self.pathfinding_system,
                    containment_actor_id,
                    None,
                    true,
                );
                let p = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                p.target = None;
                p.set_position_with_effects_and_pose_owners(
                    hull,
                    self.frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                    &self.host_trigger_world,
                    &self.team_factory,
                );
                p.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
                p.set_destination_with_operation(
                    dest,
                    &mut self.movement_events.writer(self.frame),
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    self.frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                );
                p.set_status_disabled_held(true);
            }
            self.admit_host_radar_disabled_edges(pid);
        }
        if let Some(c) = self.objects.get_mut(&ferry_id) {
            c.dock_active_docker = Some(first);
        }
        true
    }

    /// Wave 233: waypoint-path prep — stop attack and clear guard anchors.
    pub fn unit_command_waypoint_path_prep(&mut self, id: ObjectId, as_team: bool) -> bool {
        self.drop_jet_targeters_on_attack_exit(id);
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.set_guard_position(
            None,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.set_guard_target(
            None,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.end_guard_retaliate(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
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
        self.unit_command_remove_occupant_with_effects(
            container_id,
            occupant_id,
            &mut Default::default(),
        )
    }

    pub(crate) fn unit_command_remove_occupant_with_effects(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        // C++ TransportContain onRemoving: bike secondary → Kell secondary.
        self.transfer_kell_snipe_reload_from_bike(container_id, occupant_id);
        let is_rider_change = self.objects.get(&container_id).is_some_and(|container| {
            container.thing().template.contain_module.kind
                == crate::game_logic::ContainModuleKind::RiderChange
        });
        if is_rider_change {
            // RiderChangeContain removal owns selection/veterancy/scuttle
            // ordering.  It must not be reduced to a bare Vec removal or the
            // next normal Enter would leave the old rider/container corrupted.
            return self.rider_change_remove_occupant_with_effects(
                container_id,
                occupant_id,
                effects,
            );
        }
        let is_garrison = self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_garrison_contain());
        let Some(container) = self.objects.get_mut(&container_id) else {
            return false;
        };
        let removed = container.remove_occupant_with_partition_owner(
            occupant_id,
            &mut self.ownership_events,
            &mut self.partition_manager,
        );
        if removed && is_garrison {
            self.recalc_garrison_apparent_controller(container_id);
        }
        removed
    }

    /// C++ TransportContain patch 1.01: CLIFF_JUMPER + HERO/SALVAGER shot-stat copy.
    pub(super) fn transfer_kell_snipe_reload_from_bike(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) {
        use crate::game_logic::host_combat_cycle::{
            is_kell_snipe_transfer_rider, transfer_next_shot_last_fire_time,
        };
        let Some(bike_fire) = self.objects.get(&container_id).and_then(|c| {
            if !c.is_combat_cycle_style_container() {
                return None;
            }
            c.secondary_weapon
                .as_ref()
                .or(c.weapon.as_ref())
                .map(|w| w.last_fire_time)
        }) else {
            return;
        };
        let Some(occupant) = self.objects.get_mut(&occupant_id) else {
            return;
        };
        if !is_kell_snipe_transfer_rider(
            occupant.is_kind_of(KindOf::Hero),
            occupant.is_kind_of(KindOf::Salvager),
            &occupant.template_name,
        ) {
            return;
        }
        if let Some(w) = occupant.secondary_weapon.as_mut() {
            transfer_next_shot_last_fire_time(bike_fire, w);
        } else if let Some(w) = occupant.weapon.as_mut() {
            transfer_next_shot_last_fire_time(bike_fire, w);
        }
    }

    /// Wave 233: exit-unit drop residual (position/contain/target/ai).
    /// C++ OpenContain::exitObjectViaDoor walks ExitStart/End; TransportContain
    /// onRemoving applies GoAggressiveOnExit. Garrison keeps drop_position.
    /// TunnelContain does not override exitObjectViaDoor — execute_exit must
    /// walk the *exit* entrance (not `contained_by` / entry) via
    /// `unit_command_exit_via_open_contain`.
    pub fn unit_command_exit_drop(
        &mut self,
        id: ObjectId,
        drop_position: glam::Vec3,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_exit_drop_with_effects(id, drop_position, &mut Default::default())
    }

    pub(crate) fn unit_command_exit_drop_with_effects(
        &mut self,
        id: ObjectId,
        drop_position: glam::Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let container_id = self.objects.get(&id).and_then(|u| u.contained_by);
        let walk = container_id.is_some_and(|cid| {
            self.objects
                .get(&cid)
                .is_some_and(|c| !c.is_garrison_contain() && !c.is_tunnel_network_style_container())
        });
        if let (true, Some(cid)) = (walk, container_id) {
            return Ok(self.unit_command_exit_via_open_contain_with_effects(id, cid, effects));
        }
        // TransportContain::onRemoving wakes only previously idle AI, before
        // exitObjectViaDoor replaces the AI state with an exit/follow order.
        if let Some(cid) = container_id {
            self.reset_rider_mood_check_on_exit(cid, id);
        }
        let go_aggressive = container_id
            .and_then(|cid| self.objects.get(&cid))
            .is_some_and(|c| c.transport_go_aggressive_on_exit());
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        unit.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        if !self.set_owned_object_position_with_effects(id, drop_position, self.frame, effects) {
            return Ok(false);
        }
        let containment_actor_id = id;
        Self::set_owned_containment_in_parts(
            &mut self.objects,
            &mut self.partition_manager,
            &mut self.ai_groups,
            &mut self.pathfinding_system,
            containment_actor_id,
            None,
            true,
        );
        let unit = self
            .objects
            .get_mut(&containment_actor_id)
            .expect("containment operation preserves this birth");
        unit.clear_target(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        let needs_idle_entry = unit.ai_state() != AIState::Idle;
        drop(unit);
        if needs_idle_entry && !self.enter_base_ai_idle_owned(id, effects)? {
            return Ok(false);
        }
        let unit = self
            .objects
            .get_mut(&id)
            .expect("owned Idle entry preserves this birth");
        if go_aggressive {
            unit.set_ai_attitude(
                crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive,
            );
        }
        unit.set_status_moving(false);
        unit.set_status_attacking(false);
        drop(unit);
        if let Some(cid) = container_id {
            self.play_container_removing_template_sounds(cid, id, effects);
        }
        Ok(true)
    }

    /// C++ OpenContain::exitObjectViaDoor — walk ExitStart/End even after
    /// `remove_occupant` cleared the container list.
    /// TunnelContain inherits this path (no override). Garrison stays burst/side.
    pub fn unit_command_exit_via_open_contain(
        &mut self,
        id: ObjectId,
        container_id: ObjectId,
    ) -> bool {
        self.with_own_services_mut(|this| {
            this.unit_command_exit_via_open_contain_with_effects(
                id,
                container_id,
                &mut Default::default(),
            )
        })
    }

    pub(crate) fn unit_command_exit_via_open_contain_with_effects(
        &mut self,
        id: ObjectId,
        container_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        if !self.objects.contains_key(&id) || !self.objects.contains_key(&container_id) {
            return false;
        }
        if self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_garrison_contain())
        {
            return false;
        }
        self.walk_unit_via_open_contain_exit_with_effects(id, container_id, effects);
        true
    }

    /// Wave 233: mine-clearing weapon-set detail residual.
    pub fn unit_command_set_mine_clearing_detail(&mut self, id: ObjectId, enabled: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_weapon_condition_at_frame(8, enabled, self.frame);
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
        if prep_move && obj.transport_return_state.is_some() {
            // AIUpdate.cpp3042–3048: clear/goal/state setters are locked,
            // but command bookkeeping still executes in its original order.
            obj.num_frames_blocked = 0;
            obj.is_blocked = false;
            obj.is_blocked_and_stuck = false;
            return true;
        }
        if prep_move && pending_evacuate_on_stop && obj.chinook_ai.is_none() {
            if !obj.is_mobile_for_ai_command() {
                return false;
            }
            obj.transport_return_state = if pending_exit_after_evacuate {
                Some(crate::game_logic::host_transport_return::TransportReturnState::MoveAndEvacuate { origin: obj.get_position() })
            } else {
                None
            };
        } else if !pending_evacuate_on_stop {
            obj.transport_return_state = None;
        }
        obj.pending_evacuate_on_stop = pending_evacuate_on_stop;
        obj.pending_exit_after_evacuate = pending_exit_after_evacuate;
        if !pending_evacuate_on_stop || prep_move {
            // Arrival-gated evacuate must not inherit a prior ExitDelay stream.
            obj.pending_stream_exit = false;
        }
        if prep_move {
            obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            obj.set_force_attack(false);
            obj.set_guard_position(
                None,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            obj.set_guard_target(
                None,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            obj.end_guard_retaliate(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
        }
        true
    }

    /// Wave 233: order target + Entering state (deploy-to-garrison residual).
    pub fn unit_command_order_enter(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target_with_operation(
            Some(target_id),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        unit.set_ai_state(
            AIState::Entering,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        // C++ AIEnterState::onEnter ignoreObstacle(goalObject).
        unit.ignored_obstacle_id = Some(target_id);
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
        let bit = match flag {
            0 => 3,
            1 => 8,
            2 => 7,
            3 => 6,
            _ => return false,
        };
        unit.set_weapon_condition_at_frame(bit, enabled, self.frame);
        true
    }

    /// Wave 233: surrender residual.
    pub fn unit_command_set_surrendered(&mut self, id: ObjectId, surrendered: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_surrendered(
            surrendered,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// Wave 233: set AI state if alive.
    pub fn unit_command_set_ai_state(
        &mut self,
        id: ObjectId,
        state: AIState,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_set_ai_state_with_effects(
            id,
            state,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn unit_command_set_ai_state_with_effects(
        &mut self,
        id: ObjectId,
        state: AIState,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.cancel_dock_reservation(id);
        if self.objects.get(&id).is_none() {
            return Ok(false);
        }
        self.set_ai_state_decision_aware_with_effects(id, state, effects)
    }

    /// Select a concrete weapon slot for a player-issued `FIRE_WEAPON` command.
    ///
    /// Every accepted ordinal must map to a real concrete WeaponSet slot.
    /// Unknown slots fail closed; in particular, TERTIARY never falls through
    /// to PRIMARY.
    pub fn unit_command_select_weapon_slot(&mut self, id: ObjectId, slot: u8) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        let has_requested_slot = matches!(slot, 0..=2) && unit.weapon_slot(slot).is_some();
        if !has_requested_slot {
            return false;
        }

        // C++ Object::doCommandButton sets a temporary weapon lock before it
        // starts the attack, so the command cannot be re-routed by auto-choose.
        unit.set_weapon_lock(slot, WeaponLockType::LockedTemporarily)
    }

    /// Wave 233: weapon fire at object/location residual.
    pub fn unit_command_fire_weapon(
        &mut self,
        id: ObjectId,
        target_object: Option<ObjectId>,
        target_location: Option<glam::Vec3>,
        max_shots_to_fire: i32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }

        // A new player order replaces a nested AI attack machine in retail.
        // Without clearing these ownership bits, update_combat correctly
        // defers to the nested machine, which can leave a manual TERTIARY
        // command behind an old auto-acquired target state.
        unit.set_status_aiming_weapon(false);
        unit.set_status_firing_weapon(false);
        unit.set_attack_substate(AttackSubState::AimAtTarget);

        if let Some(tid) = target_object {
            unit.set_target_with_operation(
                Some(tid),
                &mut self.attack_events,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                self.frame,
                &mut crate::game_logic::ScriptPresentationEffects::default()
                    .reborrow_for_object(&mut self.partition_manager),
            );
            unit.set_max_shots_to_fire(max_shots_to_fire);
            true
        } else if let Some(pos) = target_location {
            unit.target = None;

            unit.set_target_location(
                Some(pos),
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.set_force_attack(true);
            unit.set_ai_state(
                AIState::AttackingGround,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.set_max_shots_to_fire(max_shots_to_fire);
            drop(unit);
            self.set_turret_target_position(id, Some(pos));
            true
        } else {
            false
        }
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
        unit.go_prone(
            prone_secs,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
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
    /// DemoTrapUpdate: PRIMARY=detonate, SECONDARY=proximity, TERTIARY=manual.
    pub fn unit_command_set_weapon_lock(
        &mut self,
        id: ObjectId,
        slot: u8,
        lock_type: WeaponLockType,
    ) -> bool {
        self.unit_command_set_weapon_lock_with_effects(id, slot, lock_type, &mut Default::default())
    }

    pub(crate) fn unit_command_set_weapon_lock_with_effects(
        &mut self,
        id: ObjectId,
        slot: u8,
        lock_type: WeaponLockType,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let is_demo_trap = self
            .objects
            .get(&id)
            .and_then(|unit| unit.mine_data.as_ref())
            .is_some_and(|md| {
                matches!(
                    md.kind,
                    crate::game_logic::host_mines::HostMineKind::DemoTrap
                ) && !md.detonated
            });
        let locked = {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            if !unit.is_alive() {
                return false;
            }
            unit.set_weapon_lock(slot, lock_type)
        };
        if locked && is_demo_trap {
            use crate::game_logic::host_mines::DemoTrapMode;
            let mode = match slot {
                0 => DemoTrapMode::Detonate,
                1 => DemoTrapMode::Proximity,
                2 => DemoTrapMode::Manual,
                _ => return locked,
            };
            let _ = self.set_demo_trap_mode_with_effects(id, mode, effects);
        }
        locked
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
        let candidates = match unit.active_weapon_slot {
            0 => [1u8, 2u8, 0u8],
            1 => [2u8, 0u8, 1u8],
            2 => [0u8, 1u8, 2u8],
            _ => [0u8, 1u8, 2u8],
        };
        let next = candidates
            .into_iter()
            .find(|slot| unit.weapon_slot(*slot).is_some());
        let Some(next) = next else {
            return false;
        };
        let _ = unit.set_weapon_lock(next, WeaponLockType::LockedPermanently);
        unit.set_ai_state(
            AIState::SpecialAbility,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// C++ `setSpecialPowerOverridableDestination` on PUC / Spectre
    /// (SpectreGunshipUpdate.cpp:268-283).
    ///
    /// Gates: `!isDisabled()` (the whole click is dropped on a disabled ship)
    /// and gunship status `< DEPARTING`. Stores the dest on the live Object,
    /// then drives any matching `HostParticleBeamField` / the bound ship's
    /// `HostSpectreOrbitField` (and the gunship's `override_target`)
    /// immediately — not leftover-only. For the local controlling player the
    /// template's VoiceAttack plays on the ship (object id).
    pub fn unit_command_set_special_power_overridable_destination(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
    ) -> bool {
        let (producer, overridable) = {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            // C++ :271 — `!me->isDisabled()` gates the whole click.
            if !unit.is_alive() || unit.is_disabled() {
                return false;
            }
            unit.set_special_power_overridable_destination(location, None);
            let mut overridable = true;
            if let Some(flight) = unit.spectre_gunship_update.as_mut() {
                // C++ :271-281 — the click lands only while status < DEPARTING;
                // it is stored UNCLAMPED. The AttackAreaRadius -
                // TargetingReticleRadius constraint applies in the update tick
                // (constrain_override against the FIXED initial target).
                overridable = flight.status.overridable_destination_active();
                if overridable {
                    flight.override_target = location;
                }
            }
            (unit.producer_id, overridable)
        };
        if overridable {
            let frame = self.frame;
            self.special_power_strikes
                .apply_source_override_destination(id, location, frame);
            if let Some(producer) = producer {
                if producer != id {
                    self.special_power_strikes
                        .apply_source_override_destination(producer, location, frame);
                }
            }
            // C++ :275-280 — VoiceAttack for the local controlling player.
            let local_owner = self
                .objects
                .get(&id)
                .and_then(|o| self.player_owner_for_host_object(o))
                .map(|pid| self.is_local_player(pid))
                .unwrap_or(false);
            if local_owner {
                self.queue_resolved_per_unit_sound(id, "VoiceAttack", true, false, None, 140);
            }
        }
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
        let object_scoped = self.is_object_scoped_upgrade(upgrade_name);
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // C++ ProductionUpdate::queueUpgrade OBJECT hasUpgrade / !affectedByUpgrade.
        if object_scoped && obj.refuses_object_upgrade(upgrade_name) {
            return false;
        }
        // C++ ProductionUpdate::queueUpgrade — STOP cheaters:
        // Object::canProduceUpgrade CommandSet walk.
        if !crate::game_logic::host_upgrades::object_can_produce_upgrade_receiving(
            self.receiving_module_content
                .as_ref()
                .map(|content| content.action_definitions()),
            &self.control_bar_overrides,
            obj,
            upgrade_name,
        ) {
            return false;
        }

        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        building.add_upgrade_to_queue(upgrade_name.to_string(), research_secs, cost)
    }

    /// C++ ProductionUpdate::canQueueUpgrade — QUEUE_FULL when the producer
    /// production queue is at capacity (ControlBarCommandProcessing.cpp:505-509
    /// fires GUI:ProductionQueueFull on the click).
    pub fn producer_upgrade_queue_is_full(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .map(|obj| {
                obj.building_data.as_ref().is_some_and(|building| {
                    building.production_queue.len()
                        >= crate::game_logic::buildings::DEFAULT_PRODUCTION_QUEUE_LIMIT
                })
            })
            .unwrap_or(true)
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
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_path_with_state_ignoring(id, goal, state, None)
    }

    /// C++ `ignoreObstacle(goalObject)` then move (DozerAIUpdate.cpp:210-211).
    pub fn unit_command_path_with_state_ignoring(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        state: AIState,
        ignore_obstacle: Option<ObjectId>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_path_with_state_ignoring_with_effects(
            id,
            goal,
            state,
            ignore_obstacle,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn unit_command_path_with_state_ignoring_with_effects(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        state: AIState,
        ignore_obstacle: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if !self.assign_unit_path_ignoring(id, goal, &[], ignore_obstacle) {
            return Ok(false);
        }
        self.cancel_dock_reservation(id);
        if self.objects.get(&id).is_none() {
            return Ok(false);
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if ignore_obstacle.is_some() {
                unit.ignored_obstacle_id = ignore_obstacle;
            } else if !matches!(state, AIState::Entering) {
                unit.ignored_obstacle_id = None;
            }
        }
        self.set_ai_state_decision_aware_with_effects(id, state, effects)?;
        if let Some(unit) = self.objects.get_mut(&id) {
            if !unit.movement.path.is_empty() {
                unit.set_status_moving(true);
            }
        }
        Ok(true)
    }

    /// C++ AIGroup.cpp:2042-2061 groupIdle stealth mood delay for one unit:
    /// CAN_STEALTH + canAutoAcquire + not STEALTHED/DETECTED and
    /// !canAutoAcquireWhileStealthed delays the mood check by the
    /// StealthUpdate's StealthDelay plus `skew`.
    pub fn unit_command_apply_stealth_mood_delay(
        &mut self,
        id: ObjectId,
        now_frame: u32,
        skew: u32,
    ) -> bool {
        use gamelogic::object::update::ai_update_interface::AUTO_ACQUIRE_IDLE_STEALTHED;
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(stealth_data) = unit.stealth_update_data() else {
            return false;
        };
        let stealth_delay = stealth_data.stealth_delay_frames;
        let auto_acquire_while_stealthed = stealth_data.granted_by_special_power
            || unit.auto_acquire_idle_bits & AUTO_ACQUIRE_IDLE_STEALTHED != 0;
        if unit.innate_stealth
            && unit.auto_acquire_idle_bits != 0
            && !unit.status.stealthed
            && !unit.status.detected
            && !auto_acquire_while_stealthed
        {
            unit.next_mood_check_time = now_frame.wrapping_add(stealth_delay).wrapping_add(skew);
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
    /// C++ completion (DozerAIUpdate.cpp:536-561 / BuildAssistant.cpp:361-418)
    /// never writes max health; persist scaffold damage.
    pub fn force_complete_construction(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.construction_percent = 1.0;
        obj.status.under_construction = false;
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
            obj.ai_state(),
            AIState::GuardingArea | AIState::GuardingObject
        ) || obj.active_guard_position().is_some()
            || obj.active_guard_target().is_some();
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
