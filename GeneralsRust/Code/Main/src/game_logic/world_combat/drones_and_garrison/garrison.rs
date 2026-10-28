use super::super::super::*;
use super::firepoints::*;

impl GameLogic {
    /// Residual fire-from-garrison: enclosing occupants fire from a FIREPOINT
    /// bone (C++ `calcBestGarrisonPosition`). Non-enclosing Fire Base fires
    /// from the occupant's pre-assigned STATION bone, not the building center.
    pub(in crate::game_logic) fn try_garrison_residual_fire(&mut self, garrisoned_id: ObjectId) {
        self.try_garrison_residual_fire_with_effects(
            garrisoned_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn try_garrison_residual_fire_with_effects(
        &mut self,
        garrisoned_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

        let Some(attacker) = self.objects.get(&garrisoned_id) else {
            return;
        };
        if !attacker.is_alive() {
            return;
        }
        let container_id = attacker.container_id();
        if container_id
            .and_then(|cid| self.objects.get(&cid))
            .is_some_and(|container| {
                container.status.disabled_subdued
                    || container.is_tunnel_network_style_container()
                    || !container.is_garrison_contain()
            })
        {
            // C++ GarrisonContain::isPassengerAllowedToFire: DISABLED_SUBDUED
            // (flashbang / neutron) silences window fire.
            // C++ TunnelContain::isGarrisonable is FALSE — occupants never
            // shoot out of an entrance (only TunnelNetworkGun does).
            return;
        }
        let has_any_weapon = attacker.weapon_slot(0).is_some()
            || attacker.weapon_slot(1).is_some()
            || attacker.weapon_slot(2).is_some();
        if !has_any_weapon {
            return;
        }

        let team = attacker.team;
        if let Some(cid) = container_id {
            self.ensure_garrison_bones(cid);
        }
        let ordered_target =
            container_id.and_then(|cid| self.objects.get(&cid).and_then(|c| c.target));
        let occupants = container_id
            .and_then(|cid| self.objects.get(&cid).map(|c| c.contained_units()))
            .unwrap_or_default();
        let occupant_index = occupants
            .iter()
            .position(|&id| id == garrisoned_id)
            .unwrap_or(0);

        // Pure residual acquire query (fire decision choice phase).
        let candidates: Vec<_> = self
            .objects
            .registered()
            .filter(|(id, _)| **id != garrisoned_id && Some(**id) != container_id)
            .map(|(id, obj)| {
                let combat_kind = crate::game_logic::host_residual_acquire::residual_combat_kind(
                    obj.is_kind_of(KindOf::Attackable),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                );
                crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: *id,
                    team: obj.team,
                    position: obj.get_position(),
                    is_alive: obj.is_alive(),
                    is_neutral: obj.team == Team::Neutral,
                    under_construction: obj.status.under_construction,
                    combat_kind,
                    effectively_stealthed: obj.is_effectively_stealthed(),
                    is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                    eject_invulnerable: obj.is_eject_invulnerable(),
                }
            })
            .collect();

        // C++ GarrisonContain: occupant getCurrentWeapon + best FIREPOINT vs victim.
        let mut best: Option<(ObjectId, f32, u8, glam::Vec3, f32, usize)> = None;
        for cand in &candidates {
            if !(cand.is_alive && cand.team != team && !cand.is_neutral && cand.combat_kind) {
                continue;
            }
            // C++ PartitionFilter / AcquirePlayerTargets: undetected stealth
            // is not a legal auto-acquire victim (transport path already
            // skips via pick_nearest_residual_target).
            if cand.effectively_stealthed {
                continue;
            }
            if let Some(ordered) = ordered_target {
                if cand.id != ordered {
                    continue;
                }
            }
            let (point_index, fire_pos) = container_id
                .and_then(|cid| self.objects.get(&cid))
                .map(|container| {
                    garrison_occupant_fire_point(container, garrisoned_id, cand.position)
                })
                .unwrap_or((occupant_index, cand.position));
            let Some(target_obj) = self.objects.get(&cand.id) else {
                continue;
            };
            let Some(attacker) = self.objects.get(&garrisoned_id) else {
                return;
            };
            let slot = attacker
                .select_combat_weapon_slot_with_solo_set(
                    target_obj,
                    current_time,
                    &self.solo_weapon_bonus_set,
                )
                .or_else(|| {
                    let s = attacker.active_weapon_slot;
                    attacker
                        .weapon_slot(s)
                        .filter(|w| Object::weapon_ready(w, current_time))
                        .map(|_| s)
                });
            let Some(slot) = slot else {
                continue;
            };
            let Some(weapon) = attacker.weapon_slot(slot) else {
                continue;
            };
            // C++ Weapon::isWithinAttackRange / getAttackRange applies
            // WEAPONBONUSCONDITION_GARRISONED RANGE 133% (not raw PrimaryAttackRange).
            let under = crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
            let range = (attacker.effective_weapon_range_for_slot_with_solo_set(
                slot,
                weapon.range,
                &self.solo_weapon_bonus_set,
            ) - under)
                .max(0.0);
            let dist = fire_pos.distance(cand.position);
            if dist > range {
                continue;
            }
            if best
                .as_ref()
                .map(|(_, d, _, _, _, _)| dist < *d)
                .unwrap_or(true)
            {
                best = Some((cand.id, dist, slot, fire_pos, weapon.damage, point_index));
            }
        }

        let enclosing = container_id
            .and_then(|cid| self.objects.get(&cid))
            .is_some_and(|c| c.is_enclosing_garrison_container());
        let Some((target_id, _, slot, fire_pos, _damage, point_index)) = best else {
            // C++ removeInvalidObjectsFromGarrisonPoints: not attacking / out of
            // range frees the window so another occupant can take it.
            if enclosing {
                if let Some(cid) = container_id {
                    if let Some(container) = self.objects.get_mut(&cid) {
                        if let Some(bd) = container.building_data.as_mut() {
                            bd.free_garrison_point_for(garrisoned_id);
                        }
                    }
                }
            }
            return;
        };

        if let Some(cid) = container_id {
            if let Some(container) = self.objects.get_mut(&cid) {
                if let Some(bd) = container.building_data.as_mut() {
                    // C++ trackTargets / putObjectAtGarrisonPoint: release the old
                    // FIREPOINT before claiming the closer window.
                    if enclosing {
                        bd.free_garrison_point_for(garrisoned_id);
                    }
                    if bd.garrison_point_occupant.len() <= point_index {
                        bd.garrison_point_occupant.resize(point_index + 1, None);
                    }
                    bd.garrison_point_occupant[point_index] = Some(garrisoned_id);
                }
            }
            // C++ positionObjectsAtStationGarrisonPoints: stay on STATION.
            let pin_station = self
                .objects
                .get(&cid)
                .is_some_and(|c| !c.is_enclosing_garrison_container());
            if pin_station {
                if let Some(occ) = self.objects.get_mut(&garrisoned_id) {
                    occ.set_position_with_effects_and_pose_owners(
                        fire_pos,
                        self.frame,
                        &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                        &self.host_trigger_world,
                        &self.team_factory,
                    );
                }
            }
        }
        if !self.fire_owned_passive_weapon_with_effects(
            garrisoned_id,
            target_id,
            slot,
            fire_pos,
            presentation_effects,
        ) {
            return;
        }
        self.garrison_residual_fires = self.garrison_residual_fires.saturating_add(1);
        let flash_muzzle = self
            .objects
            .get(&garrisoned_id)
            .is_some_and(|a| !occupant_weapon_is_poison(a, slot));
        let aim_at = self.objects.get(&target_id).map(|t| t.get_position());
        self.ensure_garrison_gun_effect_with_effects(
            container_id,
            point_index,
            fire_pos,
            flash_muzzle,
            aim_at,
            presentation_effects,
        );
    }

    /// C++ GarrisonContain::onContaining setTeam + academy + CAN_ATTACK + stations.
    /// C++ GarrisonContain::onObjectCreated InitialRoster spawn + addToContain.
    pub(in crate::game_logic) fn apply_garrison_initial_roster(
        &mut self,
        container_id: ObjectId,
        team: Team,
        position: glam::Vec3,
    ) {
        self.apply_garrison_initial_roster_with_effects(
            container_id,
            team,
            position,
            None,
            &mut Default::default(),
        );
    }

    pub(in crate::game_logic) fn apply_garrison_initial_roster_with_effects(
        &mut self,
        container_id: ObjectId,
        team: Team,
        position: glam::Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) = self.apply_garrison_initial_roster_with_effects_checked(
            container_id,
            team,
            position,
            interpreter,
            ui,
        ) {
            log::error!("{error}");
        }
    }

    pub(in crate::game_logic) fn apply_garrison_initial_roster_with_effects_checked(
        &mut self,
        container_id: ObjectId,
        team: Team,
        position: glam::Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(container) = self.objects.get(&container_id) else {
            return Ok(());
        };
        if container.thing().template.contain_module.kind != ContainModuleKind::Garrison {
            return Ok(());
        }
        let roster = gamelogic::object::contain::InitialRoster {
            template_name: container
                .thing()
                .template
                .contain_module
                .initial_roster_template
                .clone(),
            count: container
                .thing()
                .template
                .contain_module
                .initial_roster_count,
        };
        if !roster.is_populated() {
            return Ok(());
        }
        // C++ findTemplate resolves the admitted content, not the host's lazy
        // template cache. Materialize a known receiving child before deciding
        // it is absent; never consult another world's compatibility catalog.
        if !self.templates.contains_key(&roster.template_name)
            && !(self.receiving_module_content.is_some()
                && self.ensure_host_spawn_template(&roster.template_name))
        {
            return Ok(());
        }
        let payload_name = roster.template_name;
        for _ in 0..roster.count {
            // GarrisonContain.cpp:1748 reads the container controller for
            // each child, before addToContain. Release that borrow for create.
            let owner = self
                .objects
                .get(&container_id)
                .and_then(|parent| parent.owner_player_id);
            let created = if let Some(owner) = owner {
                // CPP GarrisonContain1748 uses the controlling Player's default
                // Team even for a neutral map garrison. The player-command
                // factory intentionally refuses Neutral and is not this path.
                let player = self.players.get(&owner).ok_or_else(|| {
                    gamelogic::GameLogicError::Configuration(
                        "Garrison controlling Player disappeared".into(),
                    )
                })?;
                let name = self.default_host_team_instance_name(Some(owner), player.team);
                let identity = {
                    let factory = self
                        .team_factory
                        .lock()
                        .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
                    let default_team = factory
                        .find_team_instances(&name)
                        .into_iter()
                        .next()
                        .ok_or_else(|| {
                            gamelogic::GameLogicError::Configuration(
                                "Garrison controller has no receiving default Team".into(),
                            )
                        })?;
                    let team = default_team
                        .read()
                        .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
                    team.get_id()
                };
                // Release Team loans before the nested factory/callback chain.
                self.create_object_on_exact_script_team_with_effects_checked(
                    &payload_name,
                    owner,
                    &name,
                    identity,
                    position,
                    false,
                    interpreter,
                    ui,
                )?
            } else {
                // Unowned standalone/tool actors retain their explicit faction
                // adapter; an admitted controller never falls back into it.
                self.create_object_for_owner_or_team_with_effects_checked(
                    &payload_name,
                    team,
                    None,
                    position,
                    interpreter,
                    ui,
                )?
            };
            let Some(occupant_id) = created else {
                break;
            };
            let added = self
                .objects
                .get_mut(&container_id)
                .is_some_and(|container| container.add_occupant(occupant_id));
            if !added {
                continue;
            }
            self.tunnel_network
                .stamp_contained_by_frame(occupant_id, self.frame);
            if let Some(occupant) = self.objects.get_mut(&occupant_id) {
                let containment_actor_id = occupant.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    containment_actor_id,
                    Some(container_id),
                    true,
                );
                let occupant = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                occupant.set_position_with_effects_and_pose_owners(
                    position,
                    self.frame,
                    &mut ui.reborrow_for_object(&mut self.partition_manager),
                    &self.host_trigger_world,
                    &self.team_factory,
                );
                occupant.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                occupant.set_status_moving(false);
                occupant.set_ai_state(
                    AIState::Garrisoned,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            self.admit_object_drawable_deselection(occupant_id);
            self.apply_garrison_contain_on_enter_with_effects(container_id, occupant_id, ui)?;
            self.stamp_player_who_entered(container_id, occupant_id);
            // C++ OpenContain::addToContain doLoadSound (once per frame).
            self.play_container_enter_sound(container_id, ui);
        }

        Ok(())
    }

    /// C++ OpenContain::addToContain `m_playerEnteredMask = rider->getControllingPlayer()`.
    /// One-frame pulse: `OpenContain::update` zeros it next logic frame.
    pub(in crate::game_logic) fn stamp_player_who_entered(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) {
        let name = {
            let Some(occupant) = self.objects.get(&occupant_id) else {
                return;
            };
            if let Some(pid) = occupant.owner_player_id {
                self.player_name(pid)
            } else {
                let team = occupant.team;
                self.players
                    .values()
                    .find(|p| p.team == team)
                    .map(|p| p.name.clone())
            }
        };
        let Some(name) = name.filter(|n| !n.is_empty()) else {
            return;
        };
        if let Some(container) = self.objects.get_mut(&container_id) {
            container.player_who_entered = name;
        }
    }

    /// C++ OpenContain::update `m_playerEnteredMask = 0`.
    pub(in crate::game_logic) fn clear_open_contain_player_who_entered(&mut self) {
        for obj in self.objects.registered_values_mut() {
            if !obj.player_who_entered.is_empty() {
                obj.player_who_entered.clear();
            }
        }
    }

    /// C++ OpenContain::update door countdown → DOOR_1_CLOSING.
    pub(in crate::game_logic) fn update_open_contain_exit_doors(&mut self) {
        for obj in self.objects.registered_values_mut() {
            if obj.door_close_countdown == 0 {
                continue;
            }
            let pulse =
                gamelogic::object::contain::open_contain::leftover_open_contain_tick_exit_door(
                    obj.door_close_countdown,
                );
            obj.door_close_countdown = pulse.countdown;
            apply_leftover_open_contain_door_pulse(obj, pulse);
        }
    }

    pub(in crate::game_logic) fn apply_garrison_contain_on_enter(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) {
        self.apply_garrison_contain_on_enter_with_effects(
            container_id,
            occupant_id,
            &mut Default::default(),
        );
    }

    pub(in crate::game_logic) fn apply_garrison_contain_on_enter_with_effects(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(container) = self.objects.get(&container_id) else {
            return Ok(());
        };
        if !container.is_garrison_contain() {
            return Ok(());
        }
        self.ensure_garrison_bones(container_id);
        if let Some(container) = self.objects.get_mut(&container_id) {
            container.set_garrison_can_attack(true);
        }
        self.place_occupant_at_garrison_station(container_id, occupant_id, presentation_effects);
        self.stamp_player_who_entered(container_id, occupant_id);
        let occupant_owner = self
            .objects
            .get(&occupant_id)
            .and_then(|o| o.owner_player_id);
        let occupant_team = self.objects.get(&occupant_id).map(|o| o.team);
        if let Some(pid) = occupant_owner {
            if let Some(player) = self.players.get_mut(&pid) {
                player.record_building_garrisoned();
            }
        } else if let Some(team) = occupant_team {
            if let Some(player) = self.players.values_mut().find(|p| p.team == team) {
                player.record_building_garrisoned();
            }
        }
        // C++ GarrisonContain::onContaining records the stat before it
        // recalculates the building's apparent controller.
        self.recalc_garrison_apparent_controller_with_effects(container_id, presentation_effects)?;
        Ok(())
    }

    /// C++ loadGarrisonPoints / loadStationGarrisonPoints.
    fn ensure_garrison_bones(&mut self, container_id: ObjectId) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if !container.is_garrison_contain() {
            return;
        }
        let enclosing = container.is_enclosing_garrison_container();
        let already = container
            .building_data
            .as_ref()
            .is_some_and(|b| b.garrison_points_initialized);
        if !already {
            let (pristine, damaged, really) = if enclosing {
                load_garrison_condition_bone_sets(container)
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };
            let stations = if enclosing {
                Vec::new()
            } else {
                let max = container
                    .thing()
                    .template
                    .contain_module
                    .slots
                    .unwrap_or(MAX_GARRISON_FIRE_POINTS)
                    .min(MAX_GARRISON_FIRE_POINTS);
                load_prefix_bones_world(container, "STATION", max)
            };
            if let Some(container) = self.objects.get_mut(&container_id) {
                if let Some(bd) = container.building_data.as_mut() {
                    if enclosing {
                        bd.garrison_fire_points = pristine;
                        bd.garrison_fire_points_damaged = damaged;
                        bd.garrison_fire_points_really_damaged = really;
                        bd.garrison_point_occupant
                            .resize(bd.garrison_fire_points.len(), None);
                    } else {
                        bd.garrison_station_points = stations;
                        bd.garrison_point_occupant
                            .resize(bd.garrison_station_points.len(), None);
                    }
                    bd.garrison_points_initialized = true;
                }
            }
        }
        // C++ findConditionIndex + redeployOccupants is enclosing-only.
        // Non-enclosing Fire Base keeps pre-assigned STATION occupants.
        if enclosing {
            let idx = self
                .objects
                .get(&container_id)
                .map(|c| garrison_condition_index(c.body_damage_state))
                .unwrap_or(0);
            if let Some(container) = self.objects.get_mut(&container_id) {
                if let Some(bd) = container.building_data.as_mut() {
                    if bd.garrison_points_condition != idx {
                        bd.garrison_points_condition = idx;
                        for slot in &mut bd.garrison_point_occupant {
                            *slot = None;
                        }
                        let n = garrison_points_for_condition(bd, idx).len();
                        if n > 0 {
                            bd.garrison_point_occupant.resize(n, None);
                        }
                    }
                }
            }
        }
    }

    /// C++ pickAStationForMe + positionObjectsAtStationGarrisonPoints.
    fn place_occupant_at_garrison_station(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let enclosing = self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_enclosing_garrison_container());
        if enclosing {
            return;
        }
        let station = {
            let Some(container) = self.objects.get_mut(&container_id) else {
                return;
            };
            let Some(bd) = container.building_data.as_mut() else {
                return;
            };
            let mut chosen = None;
            for (i, slot) in bd.garrison_point_occupant.iter().enumerate() {
                if *slot == Some(occupant_id) {
                    chosen = bd.garrison_station_points.get(i).copied();
                    break;
                }
            }
            if chosen.is_none() {
                for (i, slot) in bd.garrison_point_occupant.iter_mut().enumerate() {
                    if slot.is_none() {
                        *slot = Some(occupant_id);
                        chosen = bd.garrison_station_points.get(i).copied();
                        break;
                    }
                }
            }
            chosen
        };
        if let Some(pos) = station {
            self.set_registered_object_position_with_effects(
                occupant_id,
                pos,
                self.frame,
                presentation_effects,
            );
        }
    }

    /// C++ ScriptActions::doNamedSetGarrisonEvacDisposition.
    pub fn set_named_garrison_evac_disposition(
        &mut self,
        unit_name: &str,
        disposition: u32,
    ) -> bool {
        let Some(id) = self.host_object_id_by_script_name(unit_name) else {
            return false;
        };
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.set_garrison_evac_disposition(disposition as u8);
            return true;
        }
        false
    }

    /// C++ GarrisonContain::recalcApparentControllingPlayer.
    pub(in crate::game_logic) fn recalc_garrison_apparent_controller(
        &mut self,
        container_id: ObjectId,
    ) {
        if let Err(error) = self.recalc_garrison_apparent_controller_with_effects(
            container_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        ) {
            log::error!("Garrison owner-team recalculation failed: {error}");
        }
    }

    pub(in crate::game_logic) fn recalc_garrison_apparent_controller_with_effects(
        &mut self,
        container_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let occupants = self
            .objects
            .get(&container_id)
            .map(|c| c.contained_units())
            .unwrap_or_default();
        if occupants.is_empty() {
            let original = self.objects.get(&container_id).and_then(|container| {
                let building = container.building_data.as_ref()?;
                Some((
                    building.original_team_instance_id,
                    building.original_team_instance_name.clone(),
                    building.original_team,
                    building.original_garrison_player_id,
                ))
            });
            if let Some(container) = self.objects.get_mut(&container_id) {
                // C++ GarrisonContain::onRemoving clears these on the final
                // rider before recalcApparentControllingPlayer runs.
                container.set_garrison_can_attack(false);
                if let Some(building) = container.building_data.as_mut() {
                    building.garrison_guns.clear();
                    building.garrison_point_occupant.clear();
                }
            }
            if let Some((Some(original_id), _, _, _)) = original {
                if !self.set_host_object_team_with_effects(container_id, original_id, effects) {
                    return Err(gamelogic::GameLogicError::Configuration(
                        "Garrison original Team is unavailable in the receiving factory".into(),
                    ));
                }
            } else if let Some((_, _, Some(original_team), original_owner)) = original {
                // Legacy snapshots only retained faction/player identity. New
                // entries always carry the exact Team instance above.
                if let Some(container) = self.objects.get_mut(&container_id) {
                    container.set_team_and_owner_with_partition_owner(
                        original_team,
                        original_owner,
                        &mut self.ownership_events,
                        &mut self.partition_manager,
                    );
                }
            }
            if let Some(container) = self.objects.get_mut(&container_id) {
                // C++ onRemoving clears m_originalTeam before calling
                // recalcApparentControllingPlayer; recalc immediately
                // records the now-restored current Team again. Keep that
                // exact identity as the empty-container postcondition too.
                if let Some(building) = container.building_data.as_mut() {
                    let has_team = container.team_instance_id.is_some();
                    building.original_team = has_team.then_some(container.team);
                    building.original_garrison_player_id =
                        has_team.then_some(container.owner_player_id).flatten();
                    building.original_team_instance_id = container.team_instance_id;
                    building.original_team_instance_name = container
                        .team_instance_id
                        .map(|_| container.team_instance_name.clone());
                    building.hide_garrisoned_state = false;
                }
            }
            return Ok(());
        }
        let first = occupants
            .first()
            .and_then(|id| self.objects.get(id))
            .map(|o| (o.team, o.owner_player_id, o.status.detected));
        let Some((first_team, first_owner, first_detected)) = first else {
            return Ok(());
        };
        let stealth_kind_count = occupants
            .iter()
            .filter(|id| {
                self.objects
                    .get(id)
                    .is_some_and(|o| o.is_kind_of(KindOf::StealthGarrison))
            })
            .count();
        let hide = !first_detected && stealth_kind_count == occupants.len();
        if let Some(container) = self.objects.get_mut(&container_id) {
            if let Some(bd) = container.building_data.as_mut() {
                if bd.original_team.is_none() {
                    let has_team = container.team_instance_id.is_some();
                    bd.original_team = has_team.then_some(container.team);
                    bd.original_garrison_player_id =
                        has_team.then_some(container.owner_player_id).flatten();
                    bd.original_team_instance_id = container.team_instance_id;
                    bd.original_team_instance_name = container
                        .team_instance_id
                        .map(|_| container.team_instance_name.clone());
                }
                bd.hide_garrisoned_state = hide;
            }
        }
        let Some(owner) = first_owner else {
            // C++ only switches when the rider has a controlling Player and
            // that Player exposes a default Team.
            return Ok(());
        };
        let player = self.players.get(&owner).ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "Garrison rider controller is missing from the receiving world".into(),
            )
        })?;
        if player.default_team_id.is_none() {
            // C++ retains the rider and leaves the current Garrison Team in
            // place when its controlling Player has no default Team.
            return Ok(());
        }
        let default_team = self.host_default_team_id(owner).ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "Garrison rider controller has no receiving default Team".into(),
            )
        })?;
        if !self.set_host_object_team_with_effects(container_id, default_team, effects) {
            return Err(gamelogic::GameLogicError::Configuration(
                "Garrison owner failed receiving default-Team transfer".into(),
            ));
        }
        let _ = first_team; // Owner faction is resolved from the exact default Team.
        Ok(())
    }

    /// C++ `StealthUpdate.cpp:786-801` — DETECTED flip on a contained rider
    /// calls `GarrisonContain::recalcApparentControllingPlayer`.
    pub(in crate::game_logic) fn recalc_garrisons_after_occupant_detect_change(
        &mut self,
        container_ids: &[ObjectId],
    ) {
        let mut seen: Vec<ObjectId> = Vec::new();
        for &cid in container_ids {
            if seen.contains(&cid) {
                continue;
            }
            seen.push(cid);
            if self
                .objects
                .get(&cid)
                .is_some_and(|c| c.is_garrison_contain())
            {
                self.recalc_garrison_apparent_controller(cid);
            }
        }
    }

    /// C++ OpenContain::onCollide: eject other-player riders (STEALTH_GARRISON
    /// markAsDetected + aiExit) before the arriver boards.
    pub(in crate::game_logic) fn kick_other_controller_occupants_for_enter(
        &mut self,
        container_id: ObjectId,
        arriver_id: ObjectId,
    ) {
        self.kick_other_controller_occupants_for_enter_with_effects(
            container_id,
            arriver_id,
            &mut Default::default(),
        );
    }

    pub(in crate::game_logic) fn kick_other_controller_occupants_for_enter_with_effects(
        &mut self,
        container_id: ObjectId,
        arriver_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let arriver_owner = self
            .objects
            .get(&arriver_id)
            .and_then(|o| o.owner_player_id);
        let arriver_team = self.objects.get(&arriver_id).map(|o| o.team);
        let occupants = self
            .objects
            .get(&container_id)
            .map(|c| c.contained_units())
            .unwrap_or_default();
        let mut kick: Vec<ObjectId> = Vec::new();
        for pid in occupants {
            if pid == arriver_id {
                continue;
            }
            let Some(occ) = self.objects.get(&pid) else {
                continue;
            };
            let same = match (arriver_owner, occ.owner_player_id) {
                (Some(a), Some(b)) => a == b,
                _ => arriver_team == Some(occ.team),
            };
            if !same {
                kick.push(pid);
            }
        }
        if kick.is_empty() {
            return;
        }
        let now = self.frame;
        for pid in kick {
            let stealth_garrison = self
                .objects
                .get(&pid)
                .is_some_and(|o| o.is_kind_of(KindOf::StealthGarrison));
            let delay = self
                .objects
                .get(&pid)
                .map(|o| o.stealth_delay_frames)
                .unwrap_or(0)
                .max(60);
            if stealth_garrison {
                if let Some(occ) = self.objects.get_mut(&pid) {
                    occ.mark_detected(now.saturating_add(delay));
                }
            }
            if let Some(c) = self.objects.get_mut(&container_id) {
                let _ = c.remove_occupant_with_partition_owner(
                    pid,
                    &mut self.ownership_events,
                    &mut self.partition_manager,
                );
            }
            self.walk_unit_via_open_contain_exit_with_effects(
                pid,
                container_id,
                presentation_effects,
            );
        }
        if self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_garrison_contain())
        {
            self.recalc_garrison_apparent_controller(container_id);
        }
    }

    /// C++ OpenContain::exitObjectViaDoor — ExitStart/End + follow-path.
    /// TransportContain::onRemoving then matches hull orientation, GoAggressiveOnExit,
    /// airborne setAllowToFall. KeepContainerVelocityOnExit hull motive is
    /// independent and only runs when that INI flag is authored (default false).
    pub(in crate::game_logic) fn walk_unit_via_open_contain_exit(
        &mut self,
        unit_id: ObjectId,
        container_id: ObjectId,
    ) {
        self.walk_unit_via_open_contain_exit_with_effects(
            unit_id,
            container_id,
            &mut Default::default(),
        );
    }

    pub(in crate::game_logic) fn walk_unit_via_open_contain_exit_with_effects(
        &mut self,
        unit_id: ObjectId,
        container_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let container_id = self.redirected_containment_owner(container_id);
        let Some(container_id_birth) = self
            .objects
            .get(&container_id)
            .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token))
        else {
            return;
        };

        let unit_pos = self.objects.get(&unit_id).map(|u| u.get_position());
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        let go_aggressive = container.transport_go_aggressive_on_exit();
        let airborne = container.is_above_terrain_for_exit();
        let keep_velocity = container.transport_keep_container_velocity_on_exit();
        let hull_vel = keep_velocity.then_some(container.movement.velocity);
        let yaw = container.get_orientation();
        // OpenContain::exitObjectViaDoor reads its own rally state, even
        // when another behavior wins Object::getObjectExitInterface.
        let rally = container.open_contain_rally_point;
        let is_garrison = container.is_garrison_contain();
        let container_layer = container.pathfind_layer;
        let door_open_time = container.thing().template.contain_module.door_open_time;
        let template_name = container.template_name.clone();
        let mut exit_count = 1i32;
        let snap_garrison_to_ground = is_garrison && container.is_enclosing_garrison_container();
        let (mut start, end, next) = if is_garrison {
            let origin = container.get_position();
            let enclosing = container.is_enclosing_garrison_container();
            let disposition = container.garrison_evac_disposition();
            let (sin, cos) = yaw.sin_cos();
            if disposition == 1 || disposition == 2 {
                let geom = (*container.geometry_info());
                let half_length = if geom.authored {
                    geom.major_radius
                } else {
                    20.0
                };
                let half_width = if geom.authored {
                    geom.minor_radius
                } else {
                    10.0
                };
                let scalar = if disposition == 1 { 1.0 } else { -1.0 };
                let rnd = gamelogic::helpers::get_game_logic_random_value_real;
                let door_x = rnd(-half_length / 4.0, half_length / 4.0);
                let door_y = rnd(half_width / 2.0, half_width * 2.0) * scalar;
                let walk_x = rnd(-half_length, half_length);
                let walk_y = half_width * 10.0 * scalar;
                let to_world = |local_x: f32, local_y: f32| {
                    glam::Vec3::new(
                        origin.x + local_x * cos - local_y * sin,
                        origin.y,
                        origin.z + local_x * sin + local_y * cos,
                    )
                };
                (to_world(door_x, door_y), to_world(walk_x, walk_y), 0u8)
            } else {
                let start = if enclosing {
                    origin
                } else {
                    unit_pos.unwrap_or(origin)
                };
                (start, origin, 0u8)
            }
        } else {
            let which = if container.which_exit_path > 0 {
                container.which_exit_path
            } else {
                container
                    .building_data
                    .as_ref()
                    .map(|b| b.which_exit_path)
                    .unwrap_or(0)
            };
            let number_exits = container.transport_number_of_exit_paths();
            exit_count = number_exits;
            open_contain_exit_path(container, which, number_exits)
        };
        if snap_garrison_to_ground {
            let side_dump = self.objects.get(&container_id).is_some_and(|c| {
                let disposition = c.garrison_evac_disposition();
                disposition == 1 || disposition == 2
            });
            if !side_dump {
                if let Some(height) = self.terrain_height_at(start) {
                    start.y = height;
                }
            }
        }
        if let Some(c) = self.objects.get_mut(&container_id) {
            let _ = c.remove_occupant_with_partition_owner(
                unit_id,
                &mut self.ownership_events,
                &mut self.partition_manager,
            );
        }
        // TransportContain::onRemoving wakes only an idle rider, before
        // OpenContain::exitObjectViaDoor issues its follow-path command.
        self.reset_rider_mood_check_on_exit(container_id, unit_id);
        // C++ exitPath = [adjusted end, adjusted end, rally?]. Rally is not snapped.
        let mut door_end = end;
        if exit_count > 1 {
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.which_exit_path = next;
                if let Some(bd) = c.building_data.as_mut() {
                    bd.which_exit_path = next;
                }
            }
        }
        if is_garrison || exit_count > 0 {
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                let containment_actor_id = unit.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    containment_actor_id,
                    None,
                    true,
                );
                let mut unit = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                unit.target = None;
                let _ = unit;
                if !self.set_owned_object_position_with_effects(
                    containment_actor_id,
                    start,
                    self.frame,
                    presentation_effects,
                ) {
                    return;
                }
                if !self.objects.get(&container_id).is_some_and(|object| {
                    container_id_birth.ptr_eq(&std::sync::Arc::downgrade(
                        &object.visual_draw_identity_token,
                    ))
                }) {
                    return;
                }
                unit = match self.objects.get_mut(&containment_actor_id) {
                    Some(object) => object,
                    None => return,
                };
                unit.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
                if !is_garrison {
                    unit.pathfind_layer = container_layer;
                }
            }
        }
        let skip_follow = !is_garrison && exit_count <= 0;
        if is_garrison || !skip_follow {
            let _ = self.adjust_to_possible_destination(unit_id, &mut door_end);
        }
        let dest = door_end;
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            let containment_actor_id = unit.id;
            Self::set_owned_containment_in_parts(
                &mut self.objects,
                &mut self.partition_manager,
                containment_actor_id,
                None,
                true,
            );
            let mut unit = self
                .objects
                .get_mut(&containment_actor_id)
                .expect("containment operation preserves this birth");
            unit.target = None;
            if !skip_follow {
                let _ = unit;
                if !self.set_owned_object_position_with_effects(
                    containment_actor_id,
                    start,
                    self.frame,
                    presentation_effects,
                ) {
                    return;
                }
                if !self.objects.get(&container_id).is_some_and(|object| {
                    container_id_birth.ptr_eq(&std::sync::Arc::downgrade(
                        &object.visual_draw_identity_token,
                    ))
                }) {
                    return;
                }
                unit = match self.objects.get_mut(&containment_actor_id) {
                    Some(object) => object,
                    None => return,
                };
                unit.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
                if !is_garrison {
                    unit.pathfind_layer = container_layer;
                }
            }
            if !skip_follow {
                if !is_garrison {
                    let previous_allow_to_fall = unit.allow_to_fall;
                    unit.allow_to_fall = false;
                    unit.set_destination_with_operation(
                        dest,
                        &mut self.movement_events.writer(self.frame),
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                        self.frame,
                        &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                    );
                    unit.allow_to_fall = previous_allow_to_fall;
                    let mut path = vec![start, door_end, door_end];
                    if let Some(rally_point) = rally {
                        path.push(rally_point);
                    }
                    unit.movement.path = path;
                    unit.movement.current_path_index = 1;
                    unit.movement.target_position = Some(door_end);
                } else {
                    unit.set_destination_with_operation(
                        dest,
                        &mut self.movement_events.writer(self.frame),
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                        self.frame,
                        &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                    );
                    unit.movement.path = vec![start, door_end];
                    unit.movement.current_path_index = 1;
                    unit.movement.target_position = Some(door_end);
                }
                unit.set_ai_state(
                    AIState::Moving,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                unit.last_command_source =
                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
                unit.status.moving = true;
            }
            if is_garrison {
                unit.stamp_safe_occlusion_frame(self.frame);
            }
            // OpenContain only. GarrisonContain::exitObjectViaDoor does not
            // call ignoreObstacle or setIgnoreCollisionTime.
            if !is_garrison {
                unit.ignored_obstacle_id = None;
                unit.ignore_collisions_with = None;
                unit.ignore_collisions_until_frame = self.frame.saturating_add(30);
            }
            if go_aggressive && !is_garrison {
                unit.set_ai_attitude(
                    crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive,
                );
            }
            if !is_garrison {
                if let Some(hull_vel) = hull_vel {
                    // TransportContain::onRemoving KeepContainerVelocityOnExit.
                    let mass = unit.physics_get_mass();
                    unit.apply_motive_force(hull_vel * mass);
                }
                if airborne {
                    // TransportContain::onRemoving isAboveTerrain → setAllowToFall.
                    unit.allow_to_fall = true;
                }
            }
        }
        // C++ updateGoal is inside the numberExits > 0 path (OpenContain.cpp:1016).
        if !skip_follow {
            self.register_ground_path_goal(unit_id, door_end);
        }
        // C++ TransportContain::onRemoving ResetMoodCheckTimeOnExit + OpenContain
        // template SoundExit / SoundFallingFromPlane.
        let transport_mood = self.objects.get(&container_id).is_some_and(|c| {
            matches!(
                c.thing().template.contain_module.kind,
                crate::game_logic::ContainModuleKind::Transport
                    | crate::game_logic::ContainModuleKind::RiderChange
                    | crate::game_logic::ContainModuleKind::RailedTransport
                    | crate::game_logic::ContainModuleKind::InternetHack
            )
        });
        if transport_mood {
            if let Some(container) = self.objects.get_mut(&container_id) {
                if container.uses_transport_contain_exit_busy() {
                    let delay = container.transport_exit_delay_frames();
                    container.frame_exit_not_busy = self.frame.saturating_add(delay);
                }
                if container.contained_units().is_empty() {
                    let bit = crate::game_logic::host_enum_table_residual::loaded_model_bit();
                    container.model_condition_bits &= !(1u128 << bit);
                }
            }
        }
        if transport_mood {
            self.refresh_battle_bus_armed_riders_weapon_set(container_id);
        }
        self.play_container_removing_template_sounds(container_id, unit_id, presentation_effects);
        // C++ OpenContain::exitObjectViaDoor door countdown + DOOR_1_OPENING.
        // GarrisonContain overrides and never diddles the door.
        if !is_garrison {
            let time = gamelogic::object::contain::open_contain::leftover_open_contain_resolved_door_open_time(
                &template_name,
                door_open_time,
            );
            let pulse =
                gamelogic::object::contain::open_contain::leftover_open_contain_arm_exit_door(time);
            if let Some(container) = self.objects.get_mut(&container_id) {
                container.door_close_countdown = pulse.countdown;
                apply_leftover_open_contain_door_pulse(container, pulse);
            }
        }
    }

    /// C++ putObjectAtGarrisonPoint + updateEffects GarrisonGun / FIRING_A
    /// + trackTargets `v.toAngle()` barrel aim.
    fn ensure_garrison_gun_effect(
        &mut self,
        container_id: Option<ObjectId>,
        point_index: usize,
        pos: glam::Vec3,
        flash_muzzle: bool,
        aim_at: Option<glam::Vec3>,
    ) {
        self.ensure_garrison_gun_effect_with_effects(
            container_id,
            point_index,
            pos,
            flash_muzzle,
            aim_at,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn ensure_garrison_gun_effect_with_effects(
        &mut self,
        container_id: Option<ObjectId>,
        point_index: usize,
        pos: glam::Vec3,
        flash_muzzle: bool,
        aim_at: Option<glam::Vec3>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        const MUZZLE_FLASH_LIFETIME: u32 = 30 / 7;
        let Some(cid) = container_id else {
            return;
        };
        // C++ putObjectAtGarrisonPoint: if occupants are shown (Fire Base
        // IsEnclosingContainer=No), do not spawn a GarrisonGun drawable.
        if !self
            .objects
            .get(&cid)
            .is_some_and(|c| c.is_enclosing_garrison_container())
        {
            return;
        }
        self.expire_garrison_gun_muzzle_flashes(cid, MUZZLE_FLASH_LIFETIME);
        let existing = self
            .objects
            .get(&cid)
            .and_then(|c| c.building_data.as_ref())
            .and_then(|b| b.garrison_guns.get(point_index))
            .and_then(|g| g.drawable_id);
        let gun_id = existing.or_else(|| {
            if !self.templates.contains_key("GarrisonGun") {
                return None;
            }
            let team = self
                .objects
                .get(&cid)
                .map(|c| c.team)
                .unwrap_or(Team::Neutral);
            self.create_object_with_script_effects(
                "GarrisonGun",
                team,
                pos,
                None,
                presentation_effects,
            )
        });
        if let Some(gid) = gun_id {
            if let Some(gun) = self.objects.get_mut(&gid) {
                gun.set_position_with_effects_and_pose_owners(
                    pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                    &self.host_trigger_world,
                    &self.team_factory,
                );
                if let Some(target) = aim_at {
                    // C++ GarrisonContain::trackTargets Coord2D::toAngle();
                    // leftover dy.atan2(dx). Host Y-up ground plane is XZ.
                    let yaw = (target.z - pos.z).atan2(target.x - pos.x);
                    gun.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
                }
                // C++ updateEffects: no MODELCONDITION_FIRING_A for DAMAGE_POISON.
                if flash_muzzle {
                    gun.model_condition_bits |=
                        1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FIRING_A;
                }
            }
        }
        if let Some(container) = self.objects.get_mut(&cid) {
            if let Some(bd) = container.building_data.as_mut() {
                if bd.garrison_guns.len() <= point_index {
                    bd.garrison_guns.resize(
                        point_index + 1,
                        crate::game_logic::GarrisonGunEffect::default(),
                    );
                }
                let gun = &mut bd.garrison_guns[point_index];
                gun.drawable_id = gun_id;
                gun.last_effect_frame = self.frame;
                gun.firing = flash_muzzle;
            }
        }
    }

    fn expire_garrison_gun_muzzle_flashes(&mut self, container_id: ObjectId, lifetime: u32) {
        let frame = self.frame;
        let mut expire_ids = Vec::new();
        if let Some(container) = self.objects.get_mut(&container_id) {
            if let Some(bd) = container.building_data.as_mut() {
                for gun in &mut bd.garrison_guns {
                    if gun.firing && frame.saturating_sub(gun.last_effect_frame) > lifetime {
                        gun.firing = false;
                        if let Some(id) = gun.drawable_id {
                            expire_ids.push(id);
                        }
                    }
                }
            }
        }
        for id in expire_ids {
            if let Some(gun) = self.objects.get_mut(&id) {
                gun.model_condition_bits &=
                    !(1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FIRING_A);
            }
        }
    }

    /// Residual honesty: enter → garrisoned → exit path was exercised.
    pub fn honesty_garrison_enter_exit_ok(&self) -> bool {
        self.garrison_residual_enters > 0 && self.garrison_residual_exits > 0
    }

    /// Residual honesty: at least one fire-from-garrison residual shot.
    pub fn honesty_garrison_fire_ok(&self) -> bool {
        self.garrison_residual_fires > 0
    }

    /// Residual honesty: load → docked → unload path was exercised.
    pub fn honesty_transport_load_unload_ok(&self) -> bool {
        self.transport_residual_loads > 0 && self.transport_residual_unloads > 0
    }

    /// Residual honesty: Overlord BattleBunker enter → docked → exit path.
    /// Fail-closed: not full OverlordContain redirect / portable-structure spawn.
    pub fn honesty_overlord_bunker_enter_exit_ok(&self) -> bool {
        self.overlord_bunker_residual_enters > 0 && self.overlord_bunker_residual_exits > 0
    }
}
