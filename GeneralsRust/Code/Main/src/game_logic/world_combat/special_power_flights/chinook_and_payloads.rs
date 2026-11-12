//! Chinook, listening outpost and troop crawler flows.

use super::*;

impl GameLogic {
    /// Residual Combat Chinook honesty: successful load count.
    pub fn combat_chinook_residual_loads(&self) -> u32 {
        self.combat_chinook.loads
    }

    /// Residual Combat Chinook honesty: successful unload/evacuate count.
    pub fn combat_chinook_residual_unloads(&self) -> u32 {
        self.combat_chinook.unloads
    }

    /// Residual Combat Chinook honesty: passenger fire-from-chinook shots.
    pub fn combat_chinook_residual_passenger_fires(&self) -> u32 {
        self.combat_chinook.passenger_fires
    }

    /// Residual Combat Chinook honesty: armed-riders weapon-set upgrades.
    pub fn combat_chinook_residual_weapon_set_upgrades(&self) -> u32 {
        self.combat_chinook.weapon_set_upgrades
    }

    /// Record a residual Combat Chinook load (tests / host path).
    pub fn record_combat_chinook_residual_load(&mut self) {
        self.combat_chinook.record_load();
    }

    /// Record a residual Combat Chinook unload/evacuate (tests / host path).
    pub fn record_combat_chinook_residual_unload(&mut self) {
        self.combat_chinook.record_unload();
    }

    /// C++ `ChinookAIUpdate::update` residual: auto-land / evac / HeadOffMap / combat-drop height.
    pub fn tick_chinook_ai(&mut self, _dt: f32) -> gamelogic::GameLogicResult<()> {
        self.tick_chinook_ai_with_effects(
            _dt,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn tick_chinook_ai_with_effects(
        &mut self,
        _dt: f32,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // Flight states observe the owned locomotor's pose. Render/update dt
        // must not grant this callback a second movement integration.
        let ids: Vec<_> = self
            .objects
            .registered()
            .filter(|(_, o)| o.chinook_ai.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let wanting = self.objects.registered_values().any(|o| {
                o.target == Some(id)
                    && matches!(
                        o.ai_state(),
                        AIState::Entering | AIState::Docking | AIState::Exiting
                    )
            }) || self
                .objects
                .get(&id)
                .is_some_and(|c| c.pending_evacuate_on_stop);
            // ChinookAIUpdate.cpp: stored inherited aiIdle runs before the
            // parent update's auto-land and state-machine advance.
            let replay_idle = self.objects.get(&id).is_some_and(|obj| {
                obj.thing().template.contain_module.kind
                    != crate::game_logic::ContainModuleKind::None
                    && !obj.status.moving
                    && obj.movement.path.is_empty()
                    && obj.chinook_ai.as_ref().is_some_and(|ai| {
                        ai.state == crate::game_logic::host_combat_chinook::HostChinookAIState::Idle
                            && ai.pending_ai_idle
                    })
            });
            if replay_idle {
                if let Some(ai) = self
                    .objects
                    .get_mut(&id)
                    .and_then(|obj| obj.chinook_ai.as_mut())
                {
                    ai.pending_ai_idle = false;
                }
                self.unit_command_ai_idle_owned_with_effects(id, presentation_effects)?;
            }
            let (was_landing, was_taking_off, previous_state, effect) = {
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                let p = obj.get_position();
                let contained = obj.contained_units().len() as u32;
                let idle = !obj.status.moving && obj.movement.path.is_empty();
                let Some(ai) = obj.chinook_ai.as_mut() else {
                    continue;
                };
                ai.pos = [p.x, p.z, p.y];
                ai.parent_idle = idle;
                ai.wanting_enter_or_exit = wanting;
                ai.contained_count = contained;
                let was_landing = matches!(
                    ai.flight_status,
                    crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
                );
                let was_taking_off = matches!(
                    ai.flight_status,
                    crate::game_logic::host_combat_chinook::HostChinookFlightStatus::TakingOff
                );
                let previous_state = ai.state;
                let effect = ai.update_from_observed_pose_after_idle_replay(replay_idle);
                (was_landing, was_taking_off, previous_state, effect)
            };
            // OnRemoving may access the owner and passengers. Never hold an
            // Object borrow while invoking the immediate contain-domain action.
            if let Some(effect) = effect {
                if self.evacuate_chinook_flight_with_effects(id, presentation_effects) {
                    if let Some(obj) = self.objects.get_mut(&id) {
                        let p = obj.get_position();
                        let remaining = obj.contained_units().len() as u32;
                        if let Some(ai) = obj.chinook_ai.as_mut() {
                            ai.pos = [p.x, p.z, p.y];
                            let _ = ai.complete_evacuation(effect, remaining);
                        }
                    }
                }
            }
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let p = obj.get_position();
            let Some(ai) = obj.chinook_ai.as_mut() else {
                continue;
            };
            let landing_now = matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
            );
            let taking_off_now = matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::TakingOff
            );
            let precise_now = landing_now || taking_off_now;
            let landing_from = glam::Vec3::new(ai.pos[0], ai.pos[2], ai.pos[1]);
            let landing_dest = glam::Vec3::new(ai.dest[0], ai.dest[2], ai.dest[1]);
            let apply_landing_dest = landing_now && !was_landing;
            let path_fly = matches!(
                ai.state,
                crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToAndLand
                    | crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToAndEvac
                    | crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToAndEvacAndExit
                    | crate::game_logic::host_combat_chinook::HostChinookAIState::HeadOffMap
            )
            .then_some(glam::Vec3::new(ai.dest[0], ai.dest[2], ai.dest[1]));
            let landed = matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
            );
            let head_off_map = matches!(
                ai.state,
                crate::game_logic::host_combat_chinook::HostChinookAIState::HeadOffMap
            );
            let entered_head_off_map = head_off_map && ai.state != previous_state;
            let evacuating = matches!(
                ai.state,
                crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToAndEvac
                    | crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToAndEvacAndExit
            );
            let preferred = if matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
                    | crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
            ) {
                0.0
            } else {
                ai.preferred_height
            };
            let destroyed = ai.destroyed;
            let dump_crates =
                crate::game_logic::host_combat_chinook::chinook_flight_dumps_carried_boxes(
                    ai.flight_status,
                );
            drop(ai);

            if dump_crates {
                crate::game_logic::host_combat_chinook::lose_all_chinook_object_boxes(obj);
            }

            obj.loco_preferred_height = preferred;
            if head_off_map {
                // C++ ChinookHeadOffMapState::onEnter: original-position
                // move order permits leaving the map through the locomotor.
                obj.set_allow_invalid_position(true);
            }
            if let Some(dest) = path_fly {
                if entered_head_off_map || (obj.movement.path.is_empty() && !obj.status.moving) {
                    obj.movement.path = vec![p, dest];
                    obj.movement.current_path_index = 1;
                    obj.movement.target_position = Some(dest);
                    obj.set_locomotor_goal_position_on_path();
                    obj.is_attack_path = false;
                    obj.is_exact_path = false;
                    obj.refresh_follow_path_extra_distance();
                    obj.set_ai_state(
                        AIState::Moving,
                        self.frame,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                    obj.status.moving = true;
                    obj.pending_evacuate_on_stop = evacuating;
                }
            }
            if landed {
                // C++ ChinookAIUpdate::chooseLocomotorSet — CHINOOK_LANDED → TAXIING.
                obj.apply_taxiing_locomotor_set_for_surfaces_in(
                    0,
                    self.locomotor_definitions.receiving(),
                );
            } else if obj
                .jet_ai
                .cur_locomotor_set
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("SET_TAXIING"))
            {
                obj.apply_airborne_locomotor_set_in(self.locomotor_definitions.receiving());
            }
            // C++ ChinookTakeoffOrLandingState::onEnter/onExit (ChinookAIUpdate.cpp:223-225, :294-295).
            if precise_now {
                obj.set_precise_z_and_ultra_accurate(true);
            } else if was_landing || was_taking_off {
                obj.set_precise_z_and_ultra_accurate(false);
            }
            if destroyed {
                obj.status.destroyed = true;
            }
            drop(obj);
            if apply_landing_dest {
                let adj = ({
                    let ground_owner = self.world_services.terrain();
                    let ground_loan = ground_owner
                        .read()
                        .unwrap_or_else(|error| error.into_inner());
                    let ground_source =
                        crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                            &ground_loan,
                            self.map_loaded,
                            self.terrain.as_ref(),
                            self.pathfinding_height_samples.as_ref(),
                            &self.pathfinding_system.grid,
                        );
                    self.pathfinding_system.adjust_landing_destination_for(
                        id.0,
                        self.objects.read(),
                        landing_from,
                        landing_dest,
                        &ground_source,
                    )
                });
                if let Some(obj) = self.objects.get_mut(&id) {
                    if let Some(ai) = obj.chinook_ai.as_mut() {
                        ai.dest = [adj.x, adj.z, adj.y];
                    }
                    obj.set_locomotor_goal_position_explicit(adj);
                    obj.set_precise_z_and_ultra_accurate(true);
                }
            } else if precise_now {
                // C++ setLocomotorGoalPositionExplicit(m_destLoc) each update.
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_locomotor_goal_position_explicit(landing_dest);
                }
            }
        }

        Ok(())
    }

    /// Residual honesty: Combat Chinook load → docked → unload path.
    pub fn honesty_combat_chinook_load_unload_ok(&self) -> bool {
        self.combat_chinook.honesty_load_unload_ok()
    }

    /// Residual honesty: Combat Chinook passenger residual fire.
    pub fn honesty_combat_chinook_passenger_fire_ok(&self) -> bool {
        self.combat_chinook.honesty_passenger_fire_ok()
    }

    /// Residual honesty: Combat Chinook armed-riders weapon-set upgrade.
    pub fn honesty_combat_chinook_weapon_set_upgrade_ok(&self) -> bool {
        self.combat_chinook.honesty_weapon_set_upgrade_ok()
    }

    /// C++ TransportContain armed-riders weapon-set residual.
    /// When `armed_riders_upgrade_weapon_set` and any infantry rider has a viable
    /// ranged damage weapon, set WEAPONSET_PLAYER_UPGRADE and bind the passenger
    /// dummy weapon. Clears the flag when no armed riders remain.
    /// Battle Bus binds BattleBusPassengerDummyWeapon; Combat Chinook and
    /// Listening Outpost bind ListeningOutpostUpgradedDummyWeapon.
    pub fn refresh_battle_bus_armed_riders_weapon_set(&mut self, container_id: ObjectId) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if !container.armed_riders_upgrade_weapon_set {
            return;
        }
        let is_combat_chinook = container.is_combat_chinook_style_container();
        let is_listening_outpost = container.is_listening_outpost_style_container();
        let is_battle_bus = container.is_battle_bus_style_container();
        let occupant_ids = container.contained_units();
        let was_upgraded = container.test_weapon_condition(3);

        let mut any_armed = false;
        for oid in &occupant_ids {
            if let Some(rider) = self.objects.get(oid) {
                let infantry =
                    gamelogic::object::contain::transport_contain_passenger_kind_allowed_to_fire(
                        rider.is_kind_of(KindOf::Infantry),
                    );
                let armed = if is_combat_chinook {
                    crate::game_logic::host_combat_chinook::combat_chinook_rider_has_viable_weapon(
                        rider.weapon.as_ref(),
                        infantry,
                        rider.is_kind_of(KindOf::Vehicle),
                    )
                } else {
                    crate::game_logic::host_battle_bus::rider_has_viable_weapon(
                        rider.weapon.as_ref(),
                        infantry,
                    )
                };
                if armed {
                    any_armed = true;
                    break;
                }
            }
        }

        let mut newly_upgraded = false;
        if let Some(container) = self.objects.get_mut(&container_id) {
            let receiving = container.thing().template.has_receiving_weapon_sets();
            container.set_weapon_condition_at_frame(3, any_armed, self.frame);
            if any_armed && !receiving {
                // Bind residual dummy weapon when primary is empty or still a
                // passenger dummy (PLAYER_UPGRADE weapon set residual).
                let need_dummy = match container.weapon.as_ref() {
                    None => true,
                    Some(w) => {
                        crate::game_logic::host_combat_chinook::is_passenger_dummy_weapon(w)
                            || w.damage < 0.01
                    }
                };
                if need_dummy {
                    // Combat Chinook + Listening Outpost share ListeningOutpost dummy.
                    let _ = container.replace_weapon_set_slot(
                        0,
                        Some(if is_combat_chinook || is_listening_outpost {
                            crate::game_logic::host_combat_chinook::listening_outpost_upgraded_dummy_weapon(
                            )
                        } else {
                            crate::game_logic::host_battle_bus::battle_bus_passenger_dummy_weapon()
                        }),
                    );
                }
            }
            if any_armed {
                newly_upgraded = !was_upgraded;
            } else if was_upgraded && !receiving {
                // Clear dummy primary when no armed riders remain.
                if container
                    .weapon
                    .as_ref()
                    .map(|w| {
                        crate::game_logic::host_combat_chinook::is_passenger_dummy_weapon(w)
                            || w.damage < 0.01
                    })
                    .unwrap_or(false)
                {
                    let _ = container.replace_weapon_set_slot(0, None);
                }
            }
        }
        if newly_upgraded {
            if is_listening_outpost {
                self.listening_outpost.record_weapon_set_upgrade();
            } else if is_combat_chinook {
                self.combat_chinook.record_weapon_set_upgrade();
            } else if is_battle_bus {
                self.battle_bus.record_weapon_set_upgrade();
            }
        }
    }

    /// Residual Listening Outpost honesty: successful infantry load count.
    pub fn listening_outpost_residual_loads(&self) -> u32 {
        self.listening_outpost.loads
    }

    /// Residual Listening Outpost honesty: successful unload/evacuate count.
    pub fn listening_outpost_residual_unloads(&self) -> u32 {
        self.listening_outpost.unloads
    }

    /// Residual Listening Outpost honesty: passenger fire-from-outpost shots.
    pub fn listening_outpost_residual_passenger_fires(&self) -> u32 {
        self.listening_outpost.passenger_fires
    }

    /// Residual Listening Outpost honesty: armed-riders weapon-set upgrades.
    pub fn listening_outpost_residual_weapon_set_upgrades(&self) -> u32 {
        self.listening_outpost.weapon_set_upgrades
    }

    /// Residual Listening Outpost honesty: InitialPayload TankHunter docks.
    pub fn listening_outpost_residual_initial_payload_docks(&self) -> u32 {
        self.listening_outpost.initial_payload_docks
    }

    /// Record a residual Listening Outpost load (tests / host path).
    pub fn record_listening_outpost_residual_load(&mut self) {
        self.listening_outpost.record_load();
    }

    /// Record a residual Listening Outpost unload/evacuate (tests / host path).
    pub fn record_listening_outpost_residual_unload(&mut self) {
        self.listening_outpost.record_unload();
    }

    /// Residual honesty: Listening Outpost load → docked → unload path.
    pub fn honesty_listening_outpost_load_unload_ok(&self) -> bool {
        self.listening_outpost.honesty_load_unload_ok()
    }

    /// Residual honesty: Listening Outpost passenger residual fire.
    pub fn honesty_listening_outpost_passenger_fire_ok(&self) -> bool {
        self.listening_outpost.honesty_passenger_fire_ok()
    }

    /// Residual honesty: Listening Outpost armed-riders weapon-set upgrade.
    pub fn honesty_listening_outpost_weapon_set_upgrade_ok(&self) -> bool {
        self.listening_outpost.honesty_weapon_set_upgrade_ok()
    }

    /// Residual honesty: Listening Outpost InitialPayload TankHunter residual.
    pub fn honesty_listening_outpost_initial_payload_ok(&self) -> bool {
        self.listening_outpost.honesty_initial_payload_ok()
    }

    /// Residual honesty: any Listening Outpost residual path.
    pub fn honesty_listening_outpost_ok(&self) -> bool {
        self.listening_outpost.honesty_any_ok()
    }

    /// Residual Troop Crawler honesty accessors.
    pub fn troop_crawler_residual_loads(&self) -> u32 {
        self.troop_crawler.loads
    }
    pub fn troop_crawler_residual_unloads(&self) -> u32 {
        self.troop_crawler.unloads
    }
    pub fn troop_crawler_residual_assault_deploys(&self) -> u32 {
        self.troop_crawler.assault_deploys
    }
    pub fn troop_crawler_residual_initial_payloads(&self) -> u32 {
        self.troop_crawler.initial_payloads
    }
    pub fn record_troop_crawler_residual_load(&mut self) {
        self.troop_crawler.record_load();
    }
    pub fn record_troop_crawler_residual_unload(&mut self) {
        self.troop_crawler.record_unload();
    }
    pub fn honesty_troop_crawler_load_unload_ok(&self) -> bool {
        self.troop_crawler.honesty_load_unload_ok()
    }
    pub fn honesty_troop_crawler_wounded_retrieve_ok(&self) -> bool {
        self.troop_crawler.honesty_wounded_retrieve_ok()
    }

    pub fn honesty_troop_crawler_assault_deploy_ok(&self) -> bool {
        self.troop_crawler.honesty_assault_deploy_ok()
    }
    pub fn honesty_troop_crawler_initial_payload_ok(&self) -> bool {
        self.troop_crawler.honesty_initial_payload_ok()
    }
    pub fn honesty_troop_crawler_ok(&self) -> bool {
        self.troop_crawler.honesty_any_ok()
    }

    /// Dock InitialPayload TankHunter × 2 into a Listening Outpost residual.
    ///
    /// Fail-closed: no dock when TankHunter template is missing or capacity full.
    pub(in super::super::super) fn apply_listening_outpost_initial_payload(
        &mut self,
        outpost_id: ObjectId,
        team: Team,
        position: Vec3,
    ) {
        self.apply_listening_outpost_initial_payload_with_effects(
            outpost_id,
            team,
            position,
            None,
            &mut Default::default(),
        );
    }

    pub(in super::super::super) fn apply_listening_outpost_initial_payload_with_effects(
        &mut self,
        outpost_id: ObjectId,
        team: Team,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) = self.apply_listening_outpost_initial_payload_with_effects_checked(
            outpost_id,
            team,
            position,
            interpreter,
            ui,
        ) {
            log::error!("{error}");
        }
    }

    pub(in super::super::super) fn apply_listening_outpost_initial_payload_with_effects_checked(
        &mut self,
        outpost_id: ObjectId,
        team: Team,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::host_listening_outpost::{
            LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT, LISTENING_OUTPOST_PAYLOAD_TEMPLATE,
            LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT, preferred_payload_template,
            tank_hunter_missile_weapon,
        };

        // Ensure a payload template is available (retail or host seed).
        if !self
            .templates
            .contains_key(LISTENING_OUTPOST_PAYLOAD_TEMPLATE)
            && !self
                .templates
                .contains_key(LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT)
        {
            // Inject residual TankHunter template for host playability residual.
            let mut th = ThingTemplate::new(LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT);
            th.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(70.0)
                .set_cost(110, 0);
            self.templates
                .insert(LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT.to_string(), th);
        }

        let payload_name = preferred_payload_template(
            self.templates
                .contains_key(LISTENING_OUTPOST_PAYLOAD_TEMPLATE),
            self.templates
                .contains_key(LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT),
        );
        let Some(payload_name) = payload_name else {
            return Ok(());
        };

        for _ in 0..LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT {
            // Capacity check before spawn (avoid orphan infantry on full residual).
            let has_space = self
                .objects
                .get(&outpost_id)
                .map(|o| o.has_capacity_for(1))
                .unwrap_or(false);
            if !has_space {
                break;
            }

            let owner = self
                .objects
                .get(&outpost_id)
                .and_then(|parent| parent.owner_player_id);
            let Some(hunter_id) = self.create_object_for_owner_or_team_with_effects_checked(
                payload_name,
                team,
                owner,
                position,
                interpreter,
                ui,
            )?
            else {
                break;
            };

            // Ensure TankHunter residual missile weapon for armed-riders residual.
            if let Some(hunter) = self.objects.get_mut(&hunter_id) {
                if hunter.weapon.is_none()
                    || hunter
                        .weapon
                        .as_ref()
                        .map(|w| w.damage < 1.0)
                        .unwrap_or(true)
                {
                    hunter.weapon = Some(tank_hunter_missile_weapon());
                }
                let containment_actor_id = hunter.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    &mut self.pathfinding_system,
                    containment_actor_id,
                    Some(outpost_id),
                    true,
                );
                let hunter = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                hunter.set_ai_state(
                    AIState::Docked,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                {}
                hunter.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                hunter.set_status_moving(false);
                hunter.set_status_attacking(false);
                hunter.target = None;
                hunter.set_position_with_effects_and_pose_owners(
                    position,
                    self.frame,
                    &mut ui.reborrow_for_object(&mut self.partition_manager),
                    &self.host_trigger_world,
                    &self.team_factory,
                );
                {}
            }

            if let Some(outpost) = self.objects.get_mut(&outpost_id) {
                outpost.add_occupant(hunter_id);
            }
            self.listening_outpost.record_initial_payload_dock();
        }

        self.refresh_battle_bus_armed_riders_weapon_set(outpost_id);

        Ok(())
    }

    /// Dock InitialPayload Redguard × 8 into a Troop Crawler residual.
    ///
    /// Fail-closed: injects host seed RedGuard template when retail name missing.
    pub(in super::super::super) fn apply_troop_crawler_initial_payload(
        &mut self,
        crawler_id: ObjectId,
        team: Team,
        position: Vec3,
    ) {
        self.apply_troop_crawler_initial_payload_with_effects(
            crawler_id,
            team,
            position,
            None,
            &mut Default::default(),
        );
    }

    pub(in super::super::super) fn apply_troop_crawler_initial_payload_with_effects(
        &mut self,
        crawler_id: ObjectId,
        team: Team,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) = self.apply_troop_crawler_initial_payload_with_effects_checked(
            crawler_id,
            team,
            position,
            interpreter,
            ui,
        ) {
            log::error!("{error}");
        }
    }

    pub(in super::super::super) fn apply_troop_crawler_initial_payload_with_effects_checked(
        &mut self,
        crawler_id: ObjectId,
        team: Team,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::host_troop_crawler::{
            TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT, TROOP_CRAWLER_PAYLOAD_TEMPLATE,
            TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS, resolve_payload_template_name,
        };
        use crate::game_logic::weapon_bootstrap::REDGUARD_PRIMARY_WEAPON;

        // Ensure a payload template is available (retail or host seed).
        if !self.templates.contains_key(TROOP_CRAWLER_PAYLOAD_TEMPLATE)
            && !self
                .templates
                .contains_key(TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS)
            && !self.templates.contains_key("TestInfantry")
        {
            let mut rg = ThingTemplate::new(TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS);
            rg.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(55.0)
                .set_cost(70, 0)
                .set_primary_weapon_name(REDGUARD_PRIMARY_WEAPON);
            self.templates
                .insert(TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS.to_string(), rg);
        }

        let payload_name = resolve_payload_template_name(|n| self.templates.contains_key(n));
        let Some(payload_name) = payload_name else {
            return Ok(());
        };

        for i in 0..TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT {
            let has_space = self
                .objects
                .get(&crawler_id)
                .map(|o| o.has_capacity_for(1))
                .unwrap_or(false);
            if !has_space {
                break;
            }

            let owner = self
                .objects
                .get(&crawler_id)
                .and_then(|parent| parent.owner_player_id);
            let Some(guard_id) = self.create_object_for_owner_or_team_with_effects_checked(
                payload_name,
                team,
                owner,
                position,
                interpreter,
                ui,
            )?
            else {
                break;
            };

            if let Some(guard) = self.objects.get_mut(&guard_id) {
                // Ensure residual combat weapon so assault-deploy attack residual works.
                if guard.weapon.is_none() {
                    guard.weapon = Some(Weapon {
                        damage: 15.0,
                        range: 100.0,
                        reload_time: 1.0,
                        last_fire_time: 0.0,
                        ..Weapon::default()
                    });
                }
                let containment_actor_id = guard.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    &mut self.pathfinding_system,
                    containment_actor_id,
                    Some(crawler_id),
                    true,
                );
                let guard = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                guard.set_ai_state(
                    AIState::Docked,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                {}
                guard.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                guard.set_status_moving(false);
                guard.set_status_attacking(false);
                guard.target = None;
                guard.set_position_with_effects_and_pose_owners(
                    position,
                    self.frame,
                    &mut ui.reborrow_for_object(&mut self.partition_manager),
                    &self.host_trigger_world,
                    &self.team_factory,
                );
                {}
            }

            if let Some(crawler) = self.objects.get_mut(&crawler_id) {
                crawler.add_occupant(guard_id);
            }
            self.troop_crawler.record_initial_payload();
            // Initial payload docks count as residual loads for honesty bookkeeping.
            let _ = i;
        }

        Ok(())
    }

    /// Residual Troop Crawler assault deploy: unload docked infantry and order attack.
    ///
    /// C++ AssaultTransportAIUpdate::beginAssault + update exit/attack residual.
    /// Fail-closed: not wounded-retrieve / multi-exit stagger / heal matrix.
    pub(in super::super::super) fn apply_troop_crawler_assault_deploy(
        &mut self,
        crawler_id: ObjectId,
        target_id: ObjectId,
    ) -> u32 {
        self.apply_troop_crawler_assault_deploy_with_effects(
            crawler_id,
            target_id,
            &mut Default::default(),
        )
    }

    pub(in super::super::super) fn apply_troop_crawler_assault_deploy_with_effects(
        &mut self,
        crawler_id: ObjectId,
        target_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        use crate::game_logic::host_troop_crawler::{
            HostAssaultTransportState, TROOP_CRAWLER_DEPLOY_AUDIO, is_assault_member_wounded,
        };

        let Some(crawler) = self.objects.get(&crawler_id) else {
            return 0;
        };
        if !crawler.is_troop_crawler_style_container() {
            return 0;
        }
        let crawler_pos = crawler.get_position();
        let occupants = crawler.contained_units();
        if occupants.is_empty() {
            // Still record deploy attempt residual (weapon fired DEPLOY pulse).
            self.troop_crawler.record_assault_deploy();
            return 0;
        }

        // C++ update ejects healthy contained members; wounded stay aboard to heal.
        let mut eject_ids: Vec<ObjectId> = Vec::new();
        for occ_id in occupants.iter().copied() {
            let wounded = self
                .objects
                .get(&occ_id)
                .map(|u| is_assault_member_wounded(u.health.current, u.health.maximum))
                .unwrap_or(false);
            if !wounded {
                eject_ids.push(occ_id);
            }
        }

        let mut ordered = 0u32;
        let mut outside_members: Vec<u32> = Vec::new();
        for (i, occ_id) in eject_ids.into_iter().enumerate() {
            // Remove from container.
            if let Some(crawler) = self.objects.get_mut(&crawler_id) {
                crawler.remove_occupant_with_partition_owner(
                    occ_id,
                    &mut self.ownership_events,
                    &mut self.partition_manager,
                );
            }
            // Drop near crawler and order attack.
            let angle = (occ_id.0 as f32 + i as f32 * 1.37).sin().atan2(1.0) + i as f32 * 0.7;
            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 8.0;
            if let Some(unit) = self.objects.get_mut(&occ_id) {
                unit.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                unit.set_position_with_effects_and_pose_owners(
                    crawler_pos + offset,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                    &self.host_trigger_world,
                    &self.team_factory,
                );
                {}
                let containment_actor_id = unit.id;
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
                unit.set_status_moving(false);
            }
            // GoAggressiveOnExit residual: attack designated target.
            if self.apply_engagement_decision_aware(occ_id, target_id) {
                // The helper has already stored the legal target and attack
                // state.  Do not re-stamp it here: that used to bypass the
                // C++ WeaponSet gate for an incompatible assault passenger.
                if let Some(unit) = self.objects.get_mut(&occ_id) {
                    unit.set_status_attacking(true);
                }
            }
            outside_members.push(occ_id.0);
            ordered = ordered.saturating_add(1);
            self.troop_crawler.record_deploy_attack_order();
            self.troop_crawler.record_unload();
        }

        // Track assault members (outside + still-contained wounded) for retrieve residual.
        // Preserve Attack / AttackMove flags from the player's prior order.
        if let Some(crawler) = self.objects.get_mut(&crawler_id) {
            for occ in crawler.occupants.iter() {
                if !outside_members.contains(&occ.0) {
                    outside_members.push(occ.0);
                }
            }
            let prev = crawler.assault_transport.clone();
            crawler.assault_transport = Some(HostAssaultTransportState::begin_from(
                target_id.0,
                outside_members,
                prev.as_ref(),
            ));
        }

        self.troop_crawler.record_assault_deploy();
        self.queue_audio_event(
            AudioEventRequest::new(TROOP_CRAWLER_DEPLOY_AUDIO)
                .with_object(crawler_id)
                .with_position(crawler_pos)
                .with_priority(140),
        );
        ordered
    }

    #[cfg(test)]
    pub fn apply_troop_crawler_assault_deploy_for_test(
        &mut self,
        crawler_id: ObjectId,
        target_id: ObjectId,
    ) -> u32 {
        self.apply_troop_crawler_assault_deploy(crawler_id, target_id)
    }
}
