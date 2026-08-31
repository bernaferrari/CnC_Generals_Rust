use super::super::super::*;
use super::firepoints::*;

impl GameLogic {
    /// Residual fire-from-garrison: enclosing occupants fire from a FIREPOINT
    /// bone (C++ `calcBestGarrisonPosition`). Non-enclosing Fire Base fires
    /// from the occupant's pre-assigned STATION bone, not the building center.
    pub(in crate::game_logic) fn try_garrison_residual_fire(&mut self, garrisoned_id: ObjectId) {
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
            .iter()
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
                .select_combat_weapon_slot(target_obj, current_time)
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
            if !Object::weapon_ready(weapon, current_time) {
                continue;
            }
            // C++ Weapon::isWithinAttackRange / getAttackRange applies
            // WEAPONBONUSCONDITION_GARRISONED RANGE 133% (not raw PrimaryAttackRange).
            let range = attacker.effective_weapon_range(weapon.range);
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
        let Some((target_id, _, slot, fire_pos, damage, point_index)) = best else {
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
                    occ.set_position(fire_pos);
                }
            }
        }
        let weapon_snap = self
            .objects
            .get(&garrisoned_id)
            .and_then(|a| a.weapon_slot(slot).cloned());
        let (destroyed, _) = self.residual_auto_fire_apply_damage(
            garrisoned_id,
            target_id,
            damage,
            fire_pos,
            weapon_snap.as_ref(),
            slot,
        );

        if let Some(attacker) = self.objects.get_mut(&garrisoned_id) {
            let _ = attacker.capture_pending_weapon_visual_dispatch(
                slot,
                self.frame,
                Some(target_id),
                None,
            );
            if let Some(w) = attacker.weapon_slot_mut(slot) {
                // Clip/ammo residual parity with fire_at path (not last_fire-only stamp).
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
            // AI attack authority: residual fire-intent for GameWorld last-writer.
            if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                let (dmg, rng) = attacker
                    .weapon_slot(slot)
                    .map(|w| (w.damage, w.range))
                    .unwrap_or((0.0, 0.0));
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let next_count = attacker.fire_intent_count.saturating_add(1);
                crate::game_logic::host_fire_intent_log::record(
                    attacker.id,
                    target_id.0,
                    slot,
                    dmg,
                    rng,
                    current_time,
                    frame,
                    next_count,
                );
                attacker.fire_intent_count = next_count;
            }
            // The occupant stays Garrisoned (C++ GarrisonContain never
            // rewrites occupant AI on the fire path;
            // GarrisonContain.cpp:1691-1700 isPassengerAllowedToFire, occupant
            // holds AI_BUSY per AIUpdate.cpp:4006 privateBusy). Stamp only the
            // order target: Object::set_target would force AIState::Attacking
            // (orders.rs:30-47).
            attacker.set_order_target(Some(target_id));
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(garrisoned_id, target_id);
                crate::game_logic::host_ai_decision_log::record_set_state(garrisoned_id, 2);
            }
            // Kill XP awarded after this borrow via award_experience.
        }
        // Occupant discharges their own current slot from the FIREPOINT offset.
        let _ = self.record_accepted_weapon_discharge(garrisoned_id, slot);

        if destroyed {
            self.award_score_the_kill_experience(garrisoned_id, target_id);
            self.mark_object_for_destruction(target_id, Some(team));
        }
        self.garrison_residual_fires = self.garrison_residual_fires.saturating_add(1);
        let flash_muzzle = self
            .objects
            .get(&garrisoned_id)
            .is_some_and(|a| !occupant_weapon_is_poison(a, slot));
        let aim_at = self.objects.get(&target_id).map(|t| t.get_position());
        self.ensure_garrison_gun_effect(container_id, point_index, fire_pos, flash_muzzle, aim_at);
    }

    /// C++ GarrisonContain::onContaining setTeam + academy + CAN_ATTACK + stations.
    /// C++ GarrisonContain::onObjectCreated InitialRoster spawn + addToContain.
    pub(in crate::game_logic) fn apply_garrison_initial_roster(
        &mut self,
        container_id: ObjectId,
        team: Team,
        position: glam::Vec3,
    ) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if container.thing.template.contain_module.kind != ContainModuleKind::Garrison {
            return;
        }
        let roster = gamelogic::object::contain::InitialRoster {
            template_name: container
                .thing
                .template
                .contain_module
                .initial_roster_template
                .clone(),
            count: container.thing.template.contain_module.initial_roster_count,
        };
        if !roster.is_populated() {
            return;
        }
        if !self.templates.contains_key(&roster.template_name) {
            return;
        }
        let payload_name = roster.template_name;
        for _ in 0..roster.count {
            let Some(occupant_id) = self.create_object(&payload_name, team, position) else {
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
                occupant.set_contained_by(Some(container_id));
                occupant.set_position(position);
                occupant.stop_moving();
                occupant.set_status_moving(false);
                occupant.set_ai_state(AIState::Garrisoned);
            }
            self.apply_garrison_contain_on_enter(container_id, occupant_id);
            self.stamp_player_who_entered(container_id, occupant_id);
            // C++ OpenContain::addToContain doLoadSound (once per frame).
            self.play_container_enter_sound(container_id);
        }
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
        for obj in self.objects.values_mut() {
            if !obj.player_who_entered.is_empty() {
                obj.player_who_entered.clear();
            }
        }
    }

    /// C++ OpenContain::update door countdown → DOOR_1_CLOSING.
    pub(in crate::game_logic) fn update_open_contain_exit_doors(&mut self) {
        let pulses =
            gamelogic::object::contain::open_contain::leftover_open_contain_update_exit_doors();
        for (id, pulse) in pulses {
            if let Some(obj) = self.objects.get_mut(&ObjectId(id)) {
                obj.door_close_countdown = pulse.countdown;
                apply_leftover_open_contain_door_pulse(obj, pulse);
            }
        }
    }

    pub(in crate::game_logic) fn apply_garrison_contain_on_enter(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if !container.is_garrison_contain() {
            return;
        }
        self.ensure_garrison_bones(container_id);
        if let Some(container) = self.objects.get_mut(&container_id) {
            container.set_garrison_can_attack(true);
        }
        self.place_occupant_at_garrison_station(container_id, occupant_id);
        self.stamp_player_who_entered(container_id, occupant_id);
        self.recalc_garrison_apparent_controller(container_id);
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
                    .thing
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
            if let Some(occ) = self.objects.get_mut(&occupant_id) {
                occ.set_position(pos);
            }
        }
    }

    /// C++ ScriptActions::doNamedSetGarrisonEvacDisposition.
    pub fn set_named_garrison_evac_disposition(
        &mut self,
        unit_name: &str,
        disposition: u32,
    ) -> bool {
        gamelogic::object::contain::record_named_evac_disposition(unit_name, disposition);
        let Some(id) = self.find_object_id_by_name(unit_name) else {
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
        let occupants = self
            .objects
            .get(&container_id)
            .map(|c| c.contained_units())
            .unwrap_or_default();
        if occupants.is_empty() {
            if let Some(container) = self.objects.get_mut(&container_id) {
                container.restore_garrison_original_team_if_empty();
            }
            return;
        }
        let first = occupants
            .first()
            .and_then(|id| self.objects.get(id))
            .map(|o| (o.team, o.owner_player_id, o.status.detected));
        let Some((first_team, first_owner, first_detected)) = first else {
            return;
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
                    bd.original_team = Some(container.team);
                }
                bd.hide_garrisoned_state = hide;
            }
            container.set_team_and_owner(first_team, first_owner);
        }
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
                let _ = c.remove_occupant(pid);
            }
            self.walk_unit_via_open_contain_exit(pid, container_id);
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
        let unit_pos = self.objects.get(&unit_id).map(|u| u.get_position());
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        let go_aggressive = container.transport_go_aggressive_on_exit();
        let airborne = container.is_above_terrain_for_exit();
        let keep_velocity = container.transport_keep_container_velocity_on_exit();
        let hull_vel = keep_velocity.then_some(container.movement.velocity);
        let yaw = container.get_orientation();
        let rally = container.building_data.as_ref().and_then(|b| b.rally_point);
        let is_garrison = container.is_garrison_contain();
        let container_layer = container.pathfind_layer;
        let door_open_time = container.thing.template.contain_module.door_open_time;
        let template_name = container.template_name.clone();
        let (start, end, next) = if is_garrison {
            let origin = container.get_position();
            let geom = container.thing.template.geometry_info;
            let major = if geom.authored {
                geom.major_radius.max(8.0)
            } else {
                20.0
            };
            let enclosing = container.is_enclosing_garrison_container();
            let (sin, cos) = yaw.sin_cos();
            let dest = glam::Vec3::new(origin.x + major * cos, origin.y, origin.z + major * sin);
            let start = if enclosing {
                origin
            } else {
                unit_pos.unwrap_or(origin)
            };
            (start, dest, 0u8)
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
            open_contain_exit_path(container, which, number_exits)
        };
        // C++ exitPath = [end, end, rally?]. Live dest is rally after the door.
        let dest = if is_garrison {
            end
        } else {
            rally.unwrap_or(end)
        };
        if next > 0 {
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.which_exit_path = next;
                if let Some(bd) = c.building_data.as_mut() {
                    bd.which_exit_path = next;
                }
            }
        }
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.set_contained_by(None);
            unit.target = None;
            unit.set_position(start);
            unit.set_orientation(yaw);
            if !is_garrison {
                // C++ exitObj->setLayer(me->getLayer()) so bridge/deck unload
                // does not pick a ground cell.
                unit.pathfind_layer = container_layer;
                // Amphibious transports unload ~3ft off the ground. Force
                // allowToFall off around aiFollowPath so riders pathfind
                // instead of stacking, then restore (onRemoving airborne
                // re-enables fall below).
                let previous_allow_to_fall = unit.allow_to_fall;
                unit.allow_to_fall = false;
                unit.set_destination(dest);
                unit.allow_to_fall = previous_allow_to_fall;
            } else {
                unit.set_destination(dest);
            }
            unit.set_ai_state(AIState::Moving);
            unit.status.moving = true;
            if is_garrison {
                unit.stamp_safe_occlusion_frame(self.frame);
            }
            // C++ OpenContain::exitObjectViaDoor: ignoreObstacle(NULL) +
            // setIgnoreCollisionTime(LOGICFRAMES_PER_SECOND).
            unit.ignore_collisions_with = None;
            unit.ignore_collisions_until_frame = self.frame.saturating_add(30);
            if go_aggressive {
                unit.set_ai_attitude(
                    crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive,
                );
            }
            if let Some(hull_vel) = hull_vel {
                // C++ onRemoving: KeepContainerVelocityOnExit copies parent
                // velocity×mass as motive force. Independent of airborne.
                let mass = unit.physics_get_mass();
                unit.apply_motive_force(hull_vel * mass);
            }
            if airborne {
                // C++ onRemoving: isAboveTerrain → setAllowToFall only.
                unit.allow_to_fall = true;
            }
        }
        // C++ TransportContain::onRemoving ResetMoodCheckTimeOnExit + OpenContain
        // template SoundExit / SoundFallingFromPlane.
        self.reset_rider_mood_check_on_exit(unit_id);
        self.play_container_removing_template_sounds(container_id, unit_id);
        // C++ OpenContain::exitObjectViaDoor door countdown + DOOR_1_OPENING.
        // GarrisonContain overrides and never diddles the door.
        if !is_garrison {
            let time = gamelogic::object::contain::open_contain::leftover_open_contain_resolved_door_open_time(
                &template_name,
                door_open_time,
            );
            let pulse =
                gamelogic::object::contain::open_contain::leftover_open_contain_arm_exit_door(
                    container_id.0,
                    time,
                );
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
            self.create_object("GarrisonGun", team, pos)
        });
        if let Some(gid) = gun_id {
            if let Some(gun) = self.objects.get_mut(&gid) {
                gun.set_position(pos);
                if let Some(target) = aim_at {
                    // C++ GarrisonContain::trackTargets Coord2D::toAngle();
                    // leftover dy.atan2(dx). Host Y-up ground plane is XZ.
                    let yaw = (target.z - pos.z).atan2(target.x - pos.x);
                    gun.set_orientation(yaw);
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
