//! Host scripts `impl GameLogic` — `rebuild_dozer`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! capture / sell / dozer / rebuild holes / sole benefactor
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// C++ `DozerAIUpdate::findGoodBuildOrRepairPosition` (cpp:1855-1894).
    /// Half-radius seed, then `findPositionAround` (0–100, maxZDelta 10,
    /// `sourceToPathToDest`, ignore target when airborne). Failure keeps seed.
    pub fn find_good_build_or_repair_position(
        &self,
        source: Vec3,
        target: Vec3,
        target_selection_radius: f32,
        airborne: bool,
        ignore_object: Option<ObjectId>,
        source_id: Option<ObjectId>,
    ) -> Vec3 {
        use crate::game_logic::host_repair::{
            DOZER_FIND_POSITION_OVERLAP_SPHERE, DozerFindPositionQuery,
            find_good_build_or_repair_position as snap,
        };
        use gamelogic::ai::pathfind_astar::PathfindCellType;

        let grid_live =
            self.pathfinding_system.grid.width() > 0 && self.pathfinding_system.grid.height() > 0;
        let height_at = |p: Vec3| self.terrain_height_at(p);
        let is_cliff = |p: Vec3| {
            if let Some(t) = self.terrain.as_ref() {
                if t.is_cliff_at_world(p) {
                    return true;
                }
            }
            if let Ok(tl) = gamelogic::terrain::get_terrain_logic().read() {
                return tl.is_cliff_cell(p.x, p.z);
            }
            false
        };
        let is_underwater = |p: Vec3| {
            if let Some(t) = self.terrain.as_ref() {
                if t.is_underwater_at_world(p) {
                    return true;
                }
            }
            if let Ok(tl) = gamelogic::terrain::get_terrain_logic().read() {
                return tl.is_underwater(p.x, p.z, None, None);
            }
            false
        };
        let is_impassable = |p: Vec3| {
            if !grid_live {
                return false;
            }
            let cell = self.pathfinding_system.grid.world_to_grid(p);
            matches!(
                self.pathfinding_system.grid.cell_type(cell),
                PathfindCellType::Impassable
            )
        };
        let overlaps_object = |p: Vec3| {
            self.objects.values().any(|o| {
                if !o.is_alive() {
                    return false;
                }
                if Some(o.id) == ignore_object || Some(o.id) == source_id {
                    return false;
                }
                let r = o.selection_radius.max(0.0) + DOZER_FIND_POSITION_OVERLAP_SPHERE;
                let d = o.get_position();
                let dx = d.x - p.x;
                let dz = d.z - p.z;
                dx * dx + dz * dz <= r * r
            })
        };
        let path_exists = |from: Vec3, to: Vec3| {
            if !grid_live {
                return true;
            }
            self.pathfinding_system
                .client_safe_quick_does_path_exist(from, to)
        };
        snap(
            source,
            target,
            target_selection_radius,
            DozerFindPositionQuery {
                airborne,
                source,
                height_at: Some(&height_at),
                is_cliff: Some(&is_cliff),
                is_impassable: Some(&is_impassable),
                is_underwater: Some(&is_underwater),
                overlaps_object: Some(&overlaps_object),
                path_exists: Some(&path_exists),
            },
        )
    }

    /// C++ PhysicsUpdate collision handoff into `VeterancyCrateCollide`
    /// `IsPilot` re-crew.
    ///
    /// Returns true when the pair was handled (vehicle recrewed, infantry destroyed).
    pub fn try_infantry_unmanned_reclaim(
        &mut self,
        infantry_id: ObjectId,
        vehicle_id: ObjectId,
    ) -> bool {
        let (inf_team, inf_owner_player_id, inf_level, is_pilot) =
            match self.objects.get(&infantry_id) {
                Some(inf) => (
                    inf.team,
                    inf.owner_player_id,
                    inf.experience.level,
                    inf.is_alive()
                        && inf
                            .thing
                            .template
                            .veterancy_crate_collide
                            .as_ref()
                            .is_some_and(|metadata| metadata.supports_pilot_recrew()),
                ),
                None => return false,
            };
        if is_pilot && self.can_execute_pilot_recrew(infantry_id, vehicle_id) {
            if let Some(veh) = self.objects.get_mut(&vehicle_id) {
                let _ = veh.apply_pilot_recrew(inf_team, inf_owner_player_id, inf_level);
            }
            let _ = self.transfer_script_object_name(infantry_id, vehicle_id);
            self.destroy_object(infantry_id);
            self.process_destroy_list();
            self.unmanned_reclaims = self.unmanned_reclaims.saturating_add(1);
            return true;
        }
        if !self.can_execute_infantry_unmanned_recrew(infantry_id, vehicle_id) {
            return false;
        }
        if let Some(veh) = self.objects.get_mut(&vehicle_id) {
            if veh.status.disabled_unmanned {
                veh.set_status_disabled_unmanned(false);
                veh.status.unmanned_owner_team = None;
                veh.status.unmanned_owner_player_id = None;
                veh.set_status_disabled_hacked(false);
                veh.status.disabled_hacked_until_frame = 0;
                veh.stop_moving();
                veh.target = None;
                veh.set_ai_state(crate::game_logic::AIState::Idle);
                veh.set_team_and_owner(inf_team, inf_owner_player_id);
                veh.set_private_captured(true);
            }
        }
        let _ = self.transfer_script_object_name(infantry_id, vehicle_id);
        self.destroy_object(infantry_id);
        // C++ destroyObject is immediate for collision reclaim residual path.
        self.process_destroy_list();
        self.unmanned_reclaims = self.unmanned_reclaims.saturating_add(1);
        true
    }

    /// C++ TunnelContain::onCapture residual.
    ///
    /// Re-home entrance: `onTunnelDestroyed(old)` then `onTunnelCreated(new)`.
    /// Passengers are NOT kicked (`isKickOutOnCapture=false`). Last old-owner
    /// entrance cave-in destroys the shared pool (`TunnelTracker::destroyObject`).
    pub fn on_capture_tunnel_network_residual(
        &mut self,
        tunnel_id: ObjectId,
        old_team: Team,
        new_team: Team,
    ) {
        let is_tunnel = self
            .objects
            .get(&tunnel_id)
            .map(|o| {
                o.is_tunnel_network_style_container()
                    || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                        &o.template_name,
                    )
            })
            .unwrap_or(false);
        if !is_tunnel {
            return;
        }

        let registered = self.tunnel_network.player_holding_tunnel(tunnel_id);
        let mut new_key = self
            .objects
            .get(&tunnel_id)
            .map(|o| o.tunnel_system_key())
            .unwrap_or_else(|| {
                crate::game_logic::host_tunnel_network::tunnel_system_key(None, new_team)
            });
        let old_key = registered
            .or_else(|| self.unique_player_id_for_team(old_team))
            .unwrap_or_else(|| {
                crate::game_logic::host_tunnel_network::tunnel_system_key(None, old_team)
            });
        if old_key == new_key {
            if old_team == new_team {
                return;
            }
            // `set_team` without owner flip (tests / stale writeback): still
            // rematch onto the new faction's controlling player.
            new_key = self.unique_player_id_for_team(new_team).unwrap_or_else(|| {
                crate::game_logic::host_tunnel_network::tunnel_system_key(None, new_team)
            });
            if old_key == new_key {
                return;
            }
        }

        let remaining_old: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(id, o)| {
                **id != tunnel_id
                    && o.tunnel_system_key() == old_key
                    && o.is_alive()
                    && !o.status.sold
                    && (o.is_tunnel_network_style_container()
                        || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                            &o.template_name,
                        ))
            })
            .map(|(id, _)| *id)
            .collect();

        let outcome = self
            .tunnel_network
            .on_tunnel_destroyed(old_key, tunnel_id, &remaining_old);
        if outcome.cave_in {
            for uid in outcome.cave_in_units {
                if let Some(unit) = self.objects.get_mut(&uid) {
                    unit.set_contained_by(None);
                    unit.stop_moving();
                    unit.set_status_moving(false);
                    unit.set_status_attacking(false);
                    unit.status.destroyed = true;
                    unit.health.current = 0.0;
                }
                // C++ dying-object combat engagement clear goes through the
                // AI decision channel (Object::onDie → aiIdle family); the
                // cave-in eject must log StopAttack under decision authority.
                self.clear_target_decision_aware(uid);
                self.mark_object_for_destruction(uid, Some(new_team));
                self.capture_tunnel_last_ejects = self.capture_tunnel_last_ejects.saturating_add(1);
            }
        } else if let Some(valid) = outcome.remapped_to {
            let pool = self.tunnel_network.contained_for_player(old_key);
            let remapped: Vec<ObjectId> = pool
                .into_iter()
                .filter(|&uid| {
                    self.objects
                        .get(&uid)
                        .is_some_and(|u| u.contained_by == Some(tunnel_id))
                })
                .collect();
            for uid in remapped {
                if let Some(unit) = self.objects.get_mut(&uid) {
                    unit.set_contained_by(Some(valid));
                }
                // C++ Object::onContainedBy restamps m_containedByFrame.
                self.tunnel_network
                    .stamp_contained_by_frame(uid, self.frame);
            }
        }

        self.tunnel_network.on_tunnel_created(new_key, tunnel_id);
        self.capture_tunnel_transfers = self.capture_tunnel_transfers.saturating_add(1);
        // C++ Object::onCapture → getAIUpdateInterface()->aiIdle(CMD_FROM_AI)
        // (Object.cpp:4512-4514): the captured entrance's AI decision flips
        // through the decision channel; GameWorld stays last-writer.
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            let ordinal =
                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&AIState::Idle);
            crate::game_logic::host_ai_decision_log::record_set_state(tunnel_id, ordinal);
        }
    }

    /// C++ Object::onCapture residual (after ownership flip).
    ///
    /// - OpenContain/TransportContain: kick passengers (tunnels/caves skip)
    /// - AIUpdateInterface::aiIdle when owner changes
    /// - Skirmish AI sells captured faction structures
    /// - Deselect from former owners' selection lists
    pub fn on_capture_object_residual(
        &mut self,
        object_id: ObjectId,
        old_team: Team,
        new_team: Team,
    ) {
        // TunnelContain::onCapture remeshes Player::m_tunnelSystem even when
        // two same-faction players trade an entrance.
        self.on_capture_tunnel_network_residual(object_id, old_team, new_team);
        // C++ Object::onCapture handlePartitionCellMaintenance (Object.cpp:6165)
        // after the ownership flip: unstamp old player mask, restamp new.
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.handle_partition_cell_maintenance();
        }
        if old_team == new_team {
            return;
        }
        // Capture has already changed this object's owner by the time the
        // C++-style post-capture hooks run.  Credit that concrete owner rather
        // than whichever same-faction player happens to be first in the map.
        let captured_owner_player_id = self
            .objects
            .get(&object_id)
            .and_then(|object| self.player_owner_for_host_object(object));
        // Contain kick residual.
        self.on_capture_kick_passengers(object_id, old_team, new_team);
        // C++ Object::defect → ParkingPlaceBehavior::defectAllParkedUnits
        // (SpecialAbilityUpdate.cpp:1442). Only on capture, not a per-tick walk.
        self.defect_all_parked_units(object_id);

        // Deselect from all players (C++ TheGameLogic->deselectObject residual).
        for player in self.players.values_mut() {
            let before = player.selected_objects.len();
            player.selected_objects.retain(|&id| id != object_id);
            if player.selected_objects.len() != before {
                self.capture_deselections = self.capture_deselections.saturating_add(1);
            }
        }

        // aiIdle residual — stop orders on the captured object.
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.stop_moving();
            obj.set_status_moving(false);
            obj.set_status_attacking(false);
            // C++ Object::setCaptured(true) residual (sticky private status).
            obj.set_private_captured(true);
            // C++ clearScriptStatus(OBJECT_STATUS_SCRIPT_UNSELLABLE) residual.
            obj.set_script_unsellable(false);
        }
        self.clear_target_decision_aware(object_id);
        // Capture must clear host AI/orders immediately (observable residual).
        // Under AI_DECISION_AUTHORITY also log for GameWorld last-write channel.
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.set_ai_state(AIState::Idle);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            let ordinal =
                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&AIState::Idle);
            crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
        }
        // C++ ScoreKeeper::addObjectCaptured residual for new owner.
        if let Some(p) =
            captured_owner_player_id.and_then(|player_id| self.get_player_mut(player_id))
        {
            p.record_object_captured();
        }

        // C++ Player::becomingTeamMember → awardInitialCaptureBonus always
        // resets depositOnFrame. Recapture must re-arm the 12s clock even
        // when InitialCaptureBonus was already paid.
        if crate::game_logic::host_oil_derrick::is_oil_derrick_template(
            self.objects
                .get(&object_id)
                .map(|o| o.template_name.as_str())
                .unwrap_or(""),
        ) {
            if new_team == Team::Neutral {
                self.oil_derricks.mark_neutral_owner(object_id);
            } else {
                let owner_key = captured_owner_player_id.unwrap_or(u32::MAX);
                if self
                    .oil_derricks
                    .note_non_neutral_gain(object_id, owner_key)
                {
                    self.oil_derricks
                        .reschedule_after_capture(object_id, self.frame);
                }
            }
        }

        // C++ TechBuildingBehavior MODELCONDITION_CAPTURED residual:
        // playable side owner → set CAPTURED; neutral → clear.
        let is_tech = self
            .objects
            .get(&object_id)
            .map(|o| {
                let n = o.template_name.to_ascii_lowercase();
                n.starts_with("tech")
                    || crate::game_logic::host_oil_derrick::is_oil_derrick_template(
                        &o.template_name,
                    )
                    || n.contains("oilrefinery")
                    || n.contains("hospital")
                    || n.contains("artilleryplatform")
                    || n.contains("reinforcementpad")
                    || n.contains("repairbay") && n.contains("tech")
            })
            .unwrap_or(false);
        if is_tech {
            let playable = new_team != Team::Neutral;
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.set_captured_model_condition(playable);
            }
            self.capture_tech_model_updates = self.capture_tech_model_updates.saturating_add(1);
        }

        // Skirmish AI sells captured faction structures (C++ isSkirmishAIPlayer residual).
        let new_owner_is_ai = self
            .players
            .values()
            .filter(|p| p.team == new_team && p.is_alive)
            .any(|p| {
                // C++ Player::isSkirmishAIPlayer residual: registered AI difficulty.
                self.ai_manager.ai_difficulty(p.id).is_some()
            });
        let is_faction = self
            .objects
            .get(&object_id)
            .map(|o| o.is_faction_structure())
            .unwrap_or(false);
        if new_owner_is_ai && is_faction {
            // C++ TheBuildAssistant->sellObject(this)
            if self.start_sell_object(object_id) {
                self.capture_ai_auto_sells = self.capture_ai_auto_sells.saturating_add(1);
            }
        }
    }

    /// C++ TransportContain/OpenContain::onCapture residual.
    ///
    /// Default containers kick passengers on capture (tunnels/caves do not).
    /// Unmanned vehicles eject instantly (residual: same eject path).
    /// Overlord/Helix portable addon is `setTeam`'d and stays attached
    /// (`OverlordContain.cpp:227-235` / `HelixContain.cpp:217-222`). Infantry
    /// inside the addon still eject (`Object.cpp:6187-6191` removeAllContained).
    pub fn on_capture_kick_passengers(
        &mut self,
        container_id: ObjectId,
        old_team: Team,
        new_team: Team,
    ) {
        if old_team == new_team {
            return;
        }
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        // C++ TunnelContain/CaveContain isKickOutOnCapture = false.
        if container.is_tunnel_network_style_container()
            || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                &container.template_name,
            )
            || container.is_cave_style_container()
        {
            return;
        }

        // C++ OverlordContain/HelixContain::onCapture: setTeam portable rider.
        let portable_id = self.on_capture_overlord_helix_portable_addon(container_id, new_team);

        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        // C++ OverlordContain::isKickOutOnCapture: FALSE unless redirected bunker.
        let overlord_no_redirect = container.is_overlord_style_container()
            && !container.is_helix_transport
            && container.overlord_bunker_slot_capacity() == 0;
        if overlord_no_redirect {
            return;
        }

        let pos = container.get_position();
        let unmanned = container.status.disabled_unmanned;
        let is_garrison = container.is_garrison_contain();
        let uses_exit_busy = container.uses_transport_contain_exit_busy();
        let can_contain = container.can_contain();
        let template_lc = container.template_name.to_ascii_lowercase();
        let is_overlord_helix = container.is_overlord_style_container()
            || container.is_helix_transport
            || portable_id.is_some();
        let walk_exit = !is_overlord_helix
            && !unmanned
            && !is_garrison
            && (uses_exit_busy || can_contain || {
                template_lc.contains("humvee")
                    || template_lc.contains("chinook")
                    || template_lc.contains("crawler")
                    || template_lc.contains("battlebus")
                    || template_lc.contains("technical")
                    || template_lc.contains("outpost")
                    || template_lc.contains("helix")
            });
        let portable_id = portable_id.or(container.overlord_portable_occupant);
        let mut occupants: Vec<ObjectId> = container
            .contained_units()
            .into_iter()
            .filter(|&id| Some(id) != portable_id)
            .collect();
        occupants.retain(|&id| {
            !self.objects.get(&id).is_some_and(|occ| {
                crate::game_logic::host_battlemaster::is_portable_structure_template(
                    &occ.template_name,
                )
            })
        });
        if let Some(pid) = portable_id {
            if let Some(addon) = self.objects.get(&pid) {
                occupants.extend(addon.contained_units());
            }
        }
        if occupants.is_empty() {
            return;
        }
        // C++ TransportContain::onCapture: unmanned → removeAllContained;
        // otherwise orderAllPassengersToExit (walk + ExitDelay).
        // Overlord/Helix infantry use Object.cpp:6187-6191 removeAllContained.
        if walk_exit {
            let n = occupants.len() as u32;
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.pending_evacuate_on_stop = true;
            }
            self.capture_kick_outs = self.capture_kick_outs.saturating_add(n);
            return;
        }
        for (i, uid) in occupants.into_iter().enumerate() {
            if let Some(unit) = self.objects.get_mut(&uid) {
                let angle = (uid.0 as f32 + i as f32 * 1.11) * 0.7;
                let offset = glam::Vec3::new(angle.cos(), 0.0, angle.sin()) * 10.0;
                unit.stop_moving();
                unit.set_position(pos + offset);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    let p = pos + offset;
                    crate::game_logic::host_move_log::record(unit.id, Some([p.x, p.y, p.z]));
                    unit.record_host_movement();
                }
                unit.set_target(None);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_stop_attack(uid);
                }
                unit.set_contained_by(None);
                unit.set_ai_state(AIState::Idle);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(uid, 0);
                }
                unit.set_status_moving(false);
                unit.set_status_attacking(false);
            }
            if let Some(c) = self.objects.get_mut(&container_id) {
                let _ = c.remove_occupant(uid);
            }
            if let Some(pid) = portable_id {
                if let Some(addon) = self.objects.get_mut(&pid) {
                    let _ = addon.remove_occupant(uid);
                }
            }
            self.capture_kick_outs = self.capture_kick_outs.saturating_add(1);
        }
    }

    /// C++ OpenContain::onSelling + ParkingPlaceBehavior::killAllParkedUnits residual.
    ///
    /// - Eject garrison/transport occupants (orderAllPassengersToExit residual)
    /// - Kill parked aircraft at airfield hangar (grounded only)
    pub fn on_selling_container_residual(&mut self, structure_id: ObjectId) {
        let Some(pos) = self.objects.get(&structure_id).map(|o| o.get_position()) else {
            return;
        };
        let is_airfield = self
            .objects
            .get(&structure_id)
            .map(|o| {
                o.is_kind_of(KindOf::FSAirfield)
                    || o.template_name.to_ascii_lowercase().contains("airfield")
            })
            .unwrap_or(false);

        // Snapshot occupants.
        let occupants: Vec<ObjectId> = self
            .objects
            .get(&structure_id)
            .map(|o| o.contained_units())
            .unwrap_or_default();

        // Eject passengers around structure (OpenContain::onSelling).
        for (i, uid) in occupants.iter().copied().enumerate() {
            if is_airfield {
                // Parked jets: kill if not airborne takeoff residual.
                let kill = self
                    .objects
                    .get(&uid)
                    .map(|u| {
                        let aircraft =
                            u.is_kind_of(KindOf::Aircraft) || u.object_type == ObjectType::Aircraft;
                        let airborne = u.status.airborne_target || u.get_position().y > 5.0;
                        aircraft && !airborne
                    })
                    .unwrap_or(false);
                if kill {
                    self.destroy_object_for_sell_residual(uid);
                    self.sell_parked_units_killed = self.sell_parked_units_killed.saturating_add(1);
                    continue;
                }
            }
            // Eject residual.
            if let Some(unit) = self.objects.get_mut(&uid) {
                let angle = (uid.0 as f32 + i as f32 * 1.11) * 0.7;
                let offset = glam::Vec3::new(angle.cos(), 0.0, angle.sin()) * 10.0;
                unit.stop_moving();
                unit.set_position(pos + offset);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    let p = pos + offset;
                    crate::game_logic::host_move_log::record(unit.id, Some([p.x, p.y, p.z]));
                    unit.record_host_movement();
                }
                unit.set_target(None);
                unit.set_contained_by(None);
                unit.set_ai_state(AIState::Idle);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(uid, 0);
                }
                unit.set_status_moving(false);
                unit.set_status_attacking(false);
            }
            if let Some(st) = self.objects.get_mut(&structure_id) {
                let _ = st.remove_occupant(uid);
            }
            self.sell_passengers_ejected = self.sell_passengers_ejected.saturating_add(1);
        }

        // C++ TunnelContain::onSelling: last tunnel kicks shared pool passengers.
        let is_tunnel = self
            .objects
            .get(&structure_id)
            .map(|o| {
                o.is_tunnel_network_style_container()
                    || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                        &o.template_name,
                    )
            })
            .unwrap_or(false);
        if is_tunnel {
            let player_id = self
                .objects
                .get(&structure_id)
                .map(|o| o.tunnel_system_key());
            if let Some(player_id) = player_id {
                let tunnel_count = self
                    .objects
                    .values()
                    .filter(|o| {
                        o.is_alive()
                            && o.tunnel_system_key() == player_id
                            && o.id != structure_id
                            && !o.status.sold
                            && (o.is_tunnel_network_style_container()
                                || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                                    &o.template_name,
                                ))
                    })
                    .count();
                // friend_getTunnelCount()==1 means this is the last (others already gone).
                // Count other live tunnels; if 0, we are last.
                if tunnel_count == 0 {
                    let units = self.tunnel_network.contained_for_player(player_id);
                    for (i, uid) in units.into_iter().enumerate() {
                        let _ = self
                            .tunnel_network
                            .record_exit(player_id, uid, structure_id);

                        if let Some(unit) = self.objects.get_mut(&uid) {
                            let angle = (uid.0 as f32 + i as f32 * 1.11) * 0.7;
                            let offset = glam::Vec3::new(angle.cos(), 0.0, angle.sin()) * 10.0;
                            unit.stop_moving();
                            unit.set_position(pos + offset);
                            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                                let p = pos + offset;
                                crate::game_logic::host_move_log::record(
                                    unit.id,
                                    Some([p.x, p.y, p.z]),
                                );
                                unit.record_host_movement();
                            }
                            unit.set_target(None);
                            unit.set_contained_by(None);
                            unit.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_set_state(uid, 0);
                            }
                            unit.set_status_moving(false);
                            unit.set_status_attacking(false);
                        }
                        self.sell_passengers_ejected =
                            self.sell_passengers_ejected.saturating_add(1);
                        self.sell_tunnel_last_ejects =
                            self.sell_tunnel_last_ejects.saturating_add(1);
                    }
                }
            }
        }

        // Also kill any jet with contained_by = structure (hangar roster residual).
        if is_airfield {
            let parked: Vec<ObjectId> = self
                .objects
                .iter()
                .filter_map(|(id, o)| {
                    if o.contained_by != Some(structure_id) {
                        return None;
                    }
                    let aircraft =
                        o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft;
                    let airborne = o.status.airborne_target || o.get_position().y > 5.0;
                    if aircraft && !airborne {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            for pid in parked {
                self.destroy_object_for_sell_residual(pid);
                self.sell_parked_units_killed = self.sell_parked_units_killed.saturating_add(1);
            }
        }
        // Wave 482: flush sell residual kills (parked aircraft) same frame.
        if self.sell_parked_units_killed > 0 {
            self.process_destroy_list();
        }
    }

    pub fn start_sell_object(&mut self, object_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&object_id) else {
            return false;
        };
        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
            return false;
        }
        // C++ BuildAssistant::sellObject has no UNDER_CONSTRUCTION / RECONSTRUCTING
        // gate (BuildAssistant.cpp:1478-1500). Already-sold matches the sell-list
        // duplicate check.
        if obj.status.sold {
            return false;
        }
        if obj.is_script_unsellable() {
            return false;
        }
        if self.sell_list.iter().any(|s| s.id == object_id) {
            return false;
        }
        let team = obj.team;
        let frame = self.frame;
        // Cancel production + refund queue first (C++ ProductionUpdate cancelAndRefundAllProduction).
        self.cancel_all_production(object_id);
        // C++ contain->onSelling() + ParkingPlace killAllParkedUnits residual.
        self.on_selling_container_residual(object_id);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            // C++ setConstructionPercent(99.9f) on 0..100 scale → host 0.999
            obj.construction_percent = 0.999;
            crate::game_logic::host_construction_progress_log::record(object_id, 0.999, false, 0.0);
            obj.set_status_sold(true);
            obj.set_status_unselectable(true);
            obj.set_status_under_construction(false);
            // Wave 212: deselect logs host_status selected last-writer.
            obj.deselect();
            obj.set_ai_state(AIState::Idle);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(object_id, 0);
            }
            obj.apply_sell_scaffold_model_conditions();
        }
        // Deselect from all players.
        for p in self.players.values_mut() {
            p.selected_objects.retain(|&id| id != object_id);
        }
        self.sell_list.insert(
            0,
            ObjectSellInfo {
                id: object_id,
                sell_frame: frame,
            },
        );
        self.sell_process_starts = self.sell_process_starts.saturating_add(1);
        // C++ sellObject: destroy mines owned by this structure (producerID).
        let mine_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if !o.is_alive() {
                    return None;
                }
                let is_mine = o.mine_data.is_some()
                    || crate::game_logic::host_mines::infer_mine_kind(&o.template_name).is_some();
                if !is_mine {
                    return None;
                }
                let producer = o
                    .producer_id
                    .or_else(|| o.mine_data.as_ref().and_then(|m| m.producer_id));
                if producer == Some(object_id) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for mid in mine_ids {
            self.destroy_object(mid);
            self.sell_owned_mines_destroyed = self.sell_owned_mines_destroyed.saturating_add(1);
        }
        let _ = team;
        true
    }

    /// C++ BuildAssistant::update sell list residual.
    pub(crate) fn tick_host_systems_residuals_sole(&mut self) {
        // Wave 827: post-writeback sole-tick for remaining host system residuals.
        self.update_main_crate_vision();
        self.update_sell_list();
        self.update_special_power_strikes();
        self.update_player_upgrades();
    }

    pub(crate) fn update_sell_list(&mut self) {
        if self.sell_list.is_empty() {
            return;
        }
        let frame = self.frame;
        let mut finished: Vec<ObjectId> = Vec::new();
        let mut still: Vec<ObjectSellInfo> = Vec::with_capacity(self.sell_list.len());
        // Wave 619: under construction sole-tick, GameWorld writeback records sell-ready
        // structures (pct <= -0.5); host finishes after writeback same frame (Wave 716).
        let construction_sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
        // Empty mid-update ready set under sole: finish only via post-writeback helper.
        // Non-sole finishes via projected percent (may_finish without ready membership).
        let ready_sells: std::collections::HashSet<ObjectId> = std::collections::HashSet::new();
        for entry in std::mem::take(&mut self.sell_list) {
            let Some(obj) = self.objects.get_mut(&entry.id) else {
                // Object gone by other means.
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            let elapsed = frame.saturating_sub(entry.sell_frame);
            if elapsed >= FRAMES_TO_ALLOW_SCAFFOLD_RESIDUAL {
                let previous = obj.construction_percent;
                let sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
                // Allow percent to fall through SELL_FINISH (-0.5); do not floor at -0.01
                // (that made finish unreachable and stalled multi-frame sell forever).
                let projected = if sole {
                    // Wave 481: GW sole-ticks sell percent via negative rate; host uses writeback.
                    previous
                } else {
                    previous - SELL_CONSTRUCTION_DECREMENT_RESIDUAL
                };
                if !sole {
                    obj.construction_percent = projected;
                    crate::game_logic::host_construction_progress_log::record(
                        entry.id, projected, false, 0.0,
                    );
                } else {
                    // Negative fraction/sec so tick_construction_progress advances sell.
                    let rate =
                        -SELL_CONSTRUCTION_DECREMENT_RESIDUAL / LOGIC_FRAME_TIMESTEP.max(1e-6);
                    crate::game_logic::host_construction_progress_log::record_rate_only(
                        entry.id, false, rate,
                    );
                }
                // Cross from positive to <= 0 → MODELCONDITION_SOLD.
                // Under sole-tick, projected == writeback previous; fire when writeback already ≤ 0.
                if (previous > 0.0 && projected <= 0.0) || (sole && previous <= 0.0) {
                    obj.apply_sold_model_condition();
                }
                if projected <= SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL {
                    // Wave 619: under sole-tick, only finish IDs GW recorded ready.
                    if !construction_sole || ready_sells.contains(&entry.id) {
                        finished.push(entry.id);
                    } else {
                        still.push(entry);
                    }
                } else {
                    still.push(entry);
                }
            } else if obj.construction_percent <= SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL {
                if !construction_sole || ready_sells.contains(&entry.id) {
                    finished.push(entry.id);
                } else {
                    still.push(entry);
                }
            } else {
                still.push(entry);
            }
        }
        self.sell_list = still;
        for id in finished {
            // Refund structure sell value then destroy.
            let (team, owner_player_id, refund) = if let Some(obj) = self.objects.get(&id) {
                let owner_player_id = self.player_owner_for_host_object(obj);
                (
                    obj.team,
                    owner_player_id,
                    self.sell_refund_for_object(obj, owner_player_id),
                )
            } else {
                continue;
            };
            if refund > 0 {
                if let Some(player_id) = owner_player_id {
                    if let Some(player) = self.get_player_mut(player_id) {
                        player.apply_supply_gain(refund);
                    }
                }
            }
            // Cancel any leftover production (C++ cancel again at finish).
            self.cancel_all_production(id);
            self.destroy_object(id);
            self.sell_process_finishes = self.sell_process_finishes.saturating_add(1);
            let msg = crate::localization::localize("hud.sell.complete", "Structure sold");
            self.queue_radar_message_for_team(team, msg);
        }
    }

    /// Wave 716: after GW construction writeback records sell-ready structures,
    /// host applies refund/destroy in the same coupled tick (not next frame).
    pub(crate) fn host_apply_sell_completions_after_ready_writeback(&mut self) {
        if !crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
            return;
        }
        let ready: std::collections::HashSet<ObjectId> =
            crate::game_logic::host_sell_ready_log::drain()
                .into_iter()
                .map(|ev| ev.structure)
                .collect();
        if ready.is_empty() {
            return;
        }
        // Drop finished entries from sell_list residual.
        self.sell_list.retain(|entry| !ready.contains(&entry.id));
        for id in ready {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            // Writeback already pushed percent <= SELL_FINISH while sold.
            if !obj.status.sold
                && obj.construction_percent > SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL + 1e-6
            {
                continue;
            }
            let team = obj.team;
            let owner_player_id = self.player_owner_for_host_object(obj);
            let refund = self.sell_refund_for_object(obj, owner_player_id);
            if refund > 0 {
                if let Some(player_id) = owner_player_id {
                    if let Some(player) = self.get_player_mut(player_id) {
                        player.apply_supply_gain(refund);
                    }
                }
            }
            self.cancel_all_production(id);
            self.destroy_object(id);
            self.sell_process_finishes = self.sell_process_finishes.saturating_add(1);
            let msg = crate::localization::localize("hud.sell.complete", "Structure sold");
            self.queue_radar_message_for_team(team, msg);
        }
    }

    /// C++ `BuildAssistant::update`: a non-zero Object INI `RefundValue`
    /// overrides the ordinary `calcCostToBuild(player) * SellPercentage`
    /// calculation.  The completion paths (normal host tick and coupled
    /// GameWorld writeback) deliberately share this one calculation.
    fn sell_refund_for_object(&self, object: &Object, owner_player_id: Option<u32>) -> u32 {
        let authored_refund = object.thing.template.refund_value;
        if authored_refund != 0 {
            return authored_refund as u32;
        }

        let build_cost = owner_player_id
            .map(|player_id| {
                self.modified_build_cost_supplies(
                    player_id,
                    &object.template_name,
                    object.thing.template.build_cost.supplies,
                )
            })
            .unwrap_or(object.thing.template.build_cost.supplies);
        let sell_percentage = game_engine::common::global_data::read().sell_percentage;
        ((build_cost as f32) * sell_percentage).max(0.0) as u32
    }

    pub fn honesty_sell_process_ok(&self) -> bool {
        self.sell_process_starts > 0 && self.sell_process_finishes > 0
    }

    /// C++ DozerAIUpdate::cancelTask residual when construction is cancelled/killed.
    ///
    /// Dozers targeting `structure_id` (or actively Constructing nearby same team)
    /// go Idle and clear ACTIVELY_CONSTRUCTING model residual.
    pub fn cancel_dozers_building(&mut self, structure_id: ObjectId) {
        let (build_pos, structure_owner) = match self.objects.get(&structure_id) {
            Some(structure) => (
                Some(structure.get_position()),
                self.player_owner_for_host_object(structure),
            ),
            None => (None, None),
        };
        // An active builder belongs to the same controller, not simply the
        // same faction.  Build this immutable set before mutably clearing
        // dozer orders below.
        let nearby_owner_builders: std::collections::HashSet<ObjectId> = structure_owner
            .map(|owner_player_id| {
                self.objects
                    .iter()
                    .filter(|(_, object)| {
                        object.is_alive()
                            && object.can_construct()
                            && self.player_owner_for_host_object(object) == Some(owner_player_id)
                    })
                    .map(|(id, _)| *id)
                    .collect()
            })
            .unwrap_or_default();
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        let mut cancelled = 0u32;
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            if !obj.is_alive() || !obj.can_construct() {
                continue;
            }
            let targeting = obj.target == Some(structure_id);
            let constructing = matches!(obj.ai_state, AIState::Constructing);
            let nearby = match build_pos {
                Some(bp) => {
                    nearby_owner_builders.contains(&id) && obj.get_position().distance(bp) <= 40.0
                }
                _ => false,
            };
            if targeting || (constructing && nearby) {
                obj.target = None;
                obj.set_ai_state(AIState::Idle);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(id, 0);
                }
                obj.set_actively_constructing(false);
                cancelled = cancelled.saturating_add(1);
            }
        }
        if cancelled > 0 {
            self.dozer_cancel_task_events = self.dozer_cancel_task_events.saturating_add(cancelled);
            // Refresh ACTIVELY_CONSTRUCTING residual globally after cancel.
            // Wave 828: under coupled shadow, ACTIVELY_CONSTRUCTING bit owned by GW expire.
            if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                && crate::gameworld_shadow::shadow_coupled_tick_active())
            {
                self.update_actively_constructing_model_conditions();
            }
        }
    }

    /// C++ ActionManager.cpp:463-485 — builder still owns the BUILD task.
    fn builder_is_actively_building(&self, builder_id: ObjectId, structure_id: ObjectId) -> bool {
        if self.worker_is_acting_as_supply_truck(builder_id) {
            return false;
        }
        self.objects.get(&builder_id).is_some_and(|builder| {
            builder.is_alive()
                && (builder.dozer_task_build_target == Some(structure_id)
                    || (matches!(builder.ai_state, AIState::Constructing)
                        && builder.target == Some(structure_id)))
        })
    }

    /// Drop a recorded builder that is dead or no longer constructing this scaffold.
    fn clear_stale_structure_builder(&mut self, structure_id: ObjectId) {
        let Some(bid) = self.objects.get(&structure_id).and_then(|s| s.builder_id) else {
            return;
        };
        if self.builder_is_actively_building(bid, structure_id) {
            return;
        }
        if let Some(st) = self.objects.get_mut(&structure_id) {
            st.builder_id = None;
        }
    }

    /// C++ WorkerAIUpdate::newTask — leave supply-truck mode and drop dock claim.
    ///
    /// `m_preferredDock = INVALID_ID` plus `AS_DOZER` so other workers are not
    /// queued behind a dock this unit will never finish.
    pub fn worker_exit_supply_for_dozer_task(&mut self, worker_id: ObjectId) {
        self.cancel_dock_reservation(worker_id);
        if let Some(w) = self.objects.get_mut(&worker_id) {
            w.preferred_dock_id = None;
            w.supply_truck_state = crate::game_logic::SupplyTruckState::Idle;
            w.supply_truck_force_pending = false;
            w.supply_truck_next_dock_action_frame = 0;
        }
    }

    /// C++ WorkerStateMachine::supplyTruckSubMachineReadyToLeave → AS_DOZER.
    ///
    /// Player move/attack is not idle and not AI_DOCK, so the supply brain
    /// goes ST_BUSY (`ownerNotDockingOrIdle`) and the master machine leaves
    /// AS_SUPPLY_TRUCK. `resetSupplyTruckBrain` returns the sub-brain to its
    /// default. `m_preferredDock` stays — only `newTask` clears it.
    /// Live Idle must not re-enter Wanting after this park.
    pub fn worker_exit_supply_for_player_command(&mut self, worker_id: ObjectId) {
        let Some(obj) = self.objects.get(&worker_id) else {
            return;
        };
        let eligible = obj.is_kind_of(KindOf::Worker)
            || obj.is_resource_collector()
            || obj.thing.template.supply_truck_metadata.is_some()
            || obj.template_name.to_ascii_lowercase().contains("worker");
        if !eligible {
            return;
        }
        let ferrying = matches!(
            obj.ai_state,
            AIState::Gathering | AIState::ReturningResources
        ) || matches!(
            obj.supply_truck_state,
            crate::game_logic::SupplyTruckState::Wanting
                | crate::game_logic::SupplyTruckState::DockingWarehouse
                | crate::game_logic::SupplyTruckState::DockingCenter
                | crate::game_logic::SupplyTruckState::Regrouping
        ) || obj.supply_truck_force_pending;
        if !ferrying {
            return;
        }
        self.cancel_dock_reservation(worker_id);
        if let Some(w) = self.objects.get_mut(&worker_id) {
            w.supply_truck_state = crate::game_logic::SupplyTruckState::Idle;
            w.supply_truck_force_pending = false;
            w.supply_truck_next_dock_action_frame = 0;
        }
    }

    /// C++ ActionManager::canResumeConstructionOf residual.
    pub fn can_resume_construction_of(&self, dozer_id: ObjectId, structure_id: ObjectId) -> bool {
        let Some(dozer) = self.objects.get(&dozer_id) else {
            return false;
        };
        let Some(structure) = self.objects.get(&structure_id) else {
            return false;
        };
        if !dozer.is_alive() || !dozer.can_construct() {
            return false;
        }
        if !structure.is_alive() || !structure.is_kind_of(KindOf::Structure) {
            return false;
        }
        let Some(dozer_owner_player_id) = self.player_owner_for_host_object(dozer) else {
            return false;
        };
        let Some(structure_owner_player_id) = self.player_owner_for_host_object(structure) else {
            return false;
        };
        if self.player_relationship(dozer_owner_player_id, structure_owner_player_id)
            != gamelogic::common::Relationship::Allies
        {
            return false;
        }
        if !structure.status.under_construction || structure.status.sold {
            return false;
        }
        // C++ ActionManager.cpp:458-485 — reject only while the recorded
        // builder is alive and currently DOZER_TASK_BUILD on this structure.
        // A dead or re-tasked exclusive builder must not freeze the scaffold.
        if let Some(bid) = structure.builder_id {
            if bid != dozer_id && self.builder_is_actively_building(bid, structure_id) {
                return false;
            }
        }
        // Another dozer already actively building this structure.
        for (id, obj) in &self.objects {
            if *id == dozer_id || !obj.is_alive() || !obj.can_construct() {
                continue;
            }
            if self.player_owner_for_host_object(obj) != Some(dozer_owner_player_id) {
                continue;
            }
            if matches!(obj.ai_state, AIState::Constructing) && obj.target == Some(structure_id) {
                return false;
            }
        }
        true
    }

    /// C++ DozerAIUpdate::privateResumeConstruction residual.
    /// Returns true if a dozer was assigned.
    pub fn resume_construction(&mut self, dozer_ids: &[ObjectId], structure_id: ObjectId) -> bool {
        self.clear_stale_structure_builder(structure_id);
        // Only one dozer resumes (C++ groupResumeConstruction — first that accepts).
        for &dozer_id in dozer_ids {
            if !self.can_resume_construction_of(dozer_id, structure_id) {
                continue;
            }
            self.worker_exit_supply_for_dozer_task(dozer_id);
            // C++ privateResumeConstruction → newTask BUILD (stores ACTION dock).
            self.dozer_new_task_build(dozer_id, structure_id);

            if let Some(dozer) = self.objects.get_mut(&dozer_id) {
                dozer.target = Some(structure_id); // non-combat build association stays host
                dozer.set_ai_state(AIState::Constructing);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(dozer_id, 7);
                }
                // C++ sets MODELCONDITION_ACTIVELY_CONSTRUCTING only at the dock
                // (DozerAIUpdate.cpp:511). Driving there stays un-animated.
            }
            let approach = self
                .objects
                .get(&dozer_id)
                .and_then(|d| d.dozer_dock_action)
                .unwrap_or_else(|| {
                    let dozer_pos = self
                        .objects
                        .get(&dozer_id)
                        .map(|d| d.get_position())
                        .unwrap_or(glam::Vec3::ZERO);
                    let airborne = self.objects.get(&dozer_id).is_some_and(|d| {
                        d.is_kind_of(KindOf::Aircraft) || d.status.airborne_target
                    });
                    let (st_pos, st_radius) = self
                        .objects
                        .get(&structure_id)
                        .map(|s| (s.get_position(), s.selection_radius))
                        .unwrap_or((glam::Vec3::ZERO, 0.0));
                    self.find_good_build_or_repair_position(
                        dozer_pos,
                        st_pos,
                        st_radius,
                        airborne,
                        airborne.then_some(structure_id),
                        Some(dozer_id),
                    )
                });
            // C++ DozerActionPickActionPosState::update aiMoveToPosition (DozerAIUpdate.cpp:211).
            self.path_approach_with_state(dozer_id, approach, AIState::Constructing);
            // Structure awaiting → actively being constructed residual when dozer assigned.
            if let Some(st) = self.objects.get_mut(&structure_id) {
                st.set_under_construction_model_conditions(true);
                // C++ DozerAIUpdate::newTask setBuilder (DozerAIUpdate.cpp:1986).
                st.builder_id = Some(dozer_id);
            }

            self.resume_construction_events = self.resume_construction_events.saturating_add(1);
            // Wave 828: under coupled shadow, ACTIVELY_CONSTRUCTING bit owned by GW expire.
            if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                && crate::gameworld_shadow::shadow_coupled_tick_active())
            {
                self.update_actively_constructing_model_conditions();
            }
            return true;
        }
        false
    }

    pub fn honesty_resume_construction_ok(&self) -> bool {
        self.resume_construction_events > 0
    }

    pub fn honesty_repair_complete_ok(&self) -> bool {
        self.repair_complete_events > 0
    }

    /// C++ `DozerAIUpdate::getBoredRange` (DozerAIUpdate.cpp:2305-2312).
    /// Computer *dozers* scan 2×; `WorkerAIUpdate::getBoredRange` is unmodified.
    fn dozer_bored_scan_range(&self, dozer_id: ObjectId) -> f32 {
        let Some(dozer) = self.objects.get(&dozer_id) else {
            return crate::game_logic::host_repair::DOZER_BORED_RANGE;
        };
        if dozer.is_resource_collector()
            || dozer.template_name.to_ascii_lowercase().contains("worker")
        {
            return crate::game_logic::host_repair::DOZER_BORED_RANGE;
        }
        let is_computer = self
            .player_owner_for_host_object(dozer)
            .is_some_and(|pid| self.ai_manager.ai_players.contains_key(&pid));
        crate::game_logic::host_repair::dozer_bored_range(is_computer)
    }

    /// C++ `WorkerAIUpdate::aiDoCommand` drop-boxes-on-clear-mines tail.
    /// `isClearingMines` requires OBJECT_STATUS_IS_ATTACKING + ANTI_MINE, so
    /// the initial mine-clear order keeps carried boxes.
    pub(crate) fn drop_worker_supply_boxes_for_mine_clear(&mut self, id: ObjectId) {
        if !self.worker_is_clearing_mines(id) {
            return;
        }
        let Some(obj) = self.objects.get_mut(&id) else {
            return;
        };
        if obj.stored_resources.supplies == 0 {
            return;
        }
        if !(obj.is_resource_collector()
            || obj.template_name.to_ascii_lowercase().contains("worker"))
        {
            return;
        }
        obj.set_stored_supplies(0);
    }

    /// C++ `AIUpdateInterface::isClearingMines` (AIUpdate.cpp:3120-3135).
    pub(crate) fn worker_is_clearing_mines(&self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        let attacking = obj.status.attacking || matches!(obj.ai_state, AIState::Attacking);
        if !attacking {
            return false;
        }
        obj.weapon_set_mine_clearing_detail
    }

    /// C++ WorkerStateMachine AS_SUPPLY_TRUCK — dozer brain is not updated.
    pub(crate) fn worker_is_acting_as_supply_truck(&self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        if !obj.is_resource_collector()
            && !obj.template_name.to_ascii_lowercase().contains("worker")
        {
            return false;
        }
        matches!(
            obj.ai_state,
            AIState::Gathering | AIState::ReturningResources
        ) || matches!(
            obj.supply_truck_state,
            crate::game_logic::SupplyTruckState::Wanting
                | crate::game_logic::SupplyTruckState::DockingWarehouse
                | crate::game_logic::SupplyTruckState::DockingCenter
                | crate::game_logic::SupplyTruckState::Regrouping
        ) || obj.supply_truck_force_pending
    }

    /// C++ DozerAIUpdate findObjectToRepair residual (same player, structure, damaged).
    pub fn find_dozer_bored_repair_target(&self, dozer_id: ObjectId) -> Option<ObjectId> {
        let dozer = self.objects.get(&dozer_id)?;
        if !dozer.is_alive() || !dozer.can_repair() {
            return None;
        }
        let pos = dozer.get_position();
        let team = dozer.team;
        let dozer_owner_player_id = self.player_owner_for_host_object(dozer)?;
        let range = self.dozer_bored_scan_range(dozer_id);
        // Pure residual service acquire (2D/XZ bored range).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive()
                    || self.player_owner_for_host_object(obj) != Some(dozer_owner_player_id)
                    || !obj.is_kind_of(KindOf::Structure)
                    || obj.status.under_construction
                    || obj.status.sold
                    || obj.health.current + 0.01 >= obj.health.maximum
                {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: false,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
            Some(dozer_id),
            (pos.x, pos.z),
            candidates,
            range,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    /// C++ DozerAIUpdate findMine residual (enemy/neutral mines in BoredRange).
    pub fn find_dozer_bored_mine_target(&self, dozer_id: ObjectId) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{can_clear_mine_kind, is_mine_clearer};
        let dozer = self.objects.get(&dozer_id)?;
        if !dozer.is_alive() {
            return None;
        }
        if !is_mine_clearer(dozer.is_worker(), &dozer.template_name) {
            return None;
        }
        let pos = dozer.get_position();
        let team = dozer.team;
        let dozer_owner_player_id = self.player_owner_for_host_object(dozer);
        let range = self.dozer_bored_scan_range(dozer_id);
        // Pure residual acquire (enemy/neutral mines in BoredRange, XZ).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let allied_or_same_owner = match (
                    dozer_owner_player_id,
                    self.player_owner_for_host_object(obj),
                ) {
                    (Some(dozer_owner), Some(mine_owner)) => {
                        self.player_relationship(dozer_owner, mine_owner)
                            == gamelogic::common::Relationship::Allies
                    }
                    // Preserve team behavior for genuinely unowned legacy
                    // mines, which do not carry player relationship data.
                    _ => obj.team == team,
                };
                if allied_or_same_owner {
                    return None;
                }
                // C++ ALLOW_ENEMIES | ALLOW_NEUTRAL only (not allies / own mines).
                let inferred = crate::game_logic::host_mines::infer_mine_kind(&obj.template_name);
                let is_mine = obj.mine_data.is_some() || inferred.is_some();
                if !is_mine {
                    return None;
                }
                if let Some(md) = obj.mine_data.as_ref() {
                    if !can_clear_mine_kind(md.kind) {
                        return None;
                    }
                } else if inferred.is_some_and(|kind| !can_clear_mine_kind(kind)) {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: obj.team == Team::Neutral,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
            Some(dozer_id),
            (pos.x, pos.z),
            candidates,
            range,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    /// C++ DozerPrimaryIdleState bored residual: repair, else mine-clear.
    pub(crate) fn process_dozer_bored_event(&mut self, id: ObjectId) {
        if self.worker_is_acting_as_supply_truck(id) {
            return;
        }
        let Some((alive, can_repair)) = self
            .objects
            .get(&id)
            .map(|obj| (obj.is_alive(), obj.can_repair()))
        else {
            return;
        };
        if !alive || !can_repair {
            return;
        }
        // C++ idleConditions: isBuildMostImportant before bored scan.
        if self.dozer_idle_resume_pending_build(id) {
            return;
        }
        // Idle stamp already advanced on GW; attempt service residual once.
        if let Some(target_id) = self.find_dozer_bored_repair_target(id) {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.target = Some(target_id);
                obj.idle_since_frame = 0;
            }
            self.set_ai_state_decision_aware(id, AIState::Repairing);
            self.dozer_bored_repair_events = self.dozer_bored_repair_events.saturating_add(1);
            return;
        }
        if let Some(mine_id) = self.find_dozer_bored_mine_target(id) {
            let mine_pos = self
                .objects
                .get(&mine_id)
                .map(|m| m.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.set_weapon_set_mine_clearing_detail(true);
                obj.idle_since_frame = 0;
            }

            if self.apply_engagement_decision_aware(id, mine_id) {
                self.path_approach_with_state(id, mine_pos, AIState::Attacking);
                self.dozer_bored_mine_clear_events =
                    self.dozer_bored_mine_clear_events.saturating_add(1);
            }
        }
    }

    pub(in super::super) fn update_dozer_bored_repair(&mut self) {
        let now = self.frame;
        let bored = crate::game_logic::host_repair::DOZER_BORED_TIME_FRAMES;
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            if self.worker_is_acting_as_supply_truck(id) {
                continue;
            }
            {
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                if !obj.is_alive() || !obj.can_repair() {
                    continue;
                }
                // Track idle timestamp residual.
                if matches!(obj.ai_state, AIState::Idle) {
                    if obj.idle_since_frame == 0 {
                        obj.idle_since_frame = now.max(1);
                    }
                } else {
                    obj.idle_since_frame = 0;
                    continue;
                }
            }
            // C++ DozerPrimaryStateMachine idleConditions before bored (DozerAIUpdate.cpp:1260).
            if self.dozer_idle_resume_pending_build(id) {
                continue;
            }
            let idle_since = self
                .objects
                .get(&id)
                .map(|obj| obj.idle_since_frame)
                .unwrap_or(0);
            if now.saturating_sub(idle_since) < bored {
                continue;
            }
            // Reset stamp so we don't scan every frame (C++ resets idle timestamp).
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.idle_since_frame = now.max(1);
            }

            if let Some(target_id) = self.find_dozer_bored_repair_target(id) {
                if let Some(obj) = self.objects.get_mut(&id) {
                    // Repair target is non-combat host association (not AttackTarget).
                    obj.target = Some(target_id);
                    obj.idle_since_frame = 0;
                }
                self.set_ai_state_decision_aware(id, AIState::Repairing);
                self.dozer_bored_repair_events = self.dozer_bored_repair_events.saturating_add(1);
                continue;
            }

            // C++ else branch: WEAPONSET_MINE_CLEARING_DETAIL + findMine + aiAttackObject.
            if let Some(mine_id) = self.find_dozer_bored_mine_target(id) {
                let mine_pos = self
                    .objects
                    .get(&mine_id)
                    .map(|m| m.get_position())
                    .unwrap_or(glam::Vec3::ZERO);
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_weapon_set_mine_clearing_detail(true);
                    obj.idle_since_frame = 0;
                }

                // Combat engagement via decision authority; path_approach logs Attacking too.
                if self.apply_engagement_decision_aware(id, mine_id) {
                    // Approach residual — attack resolution happens in combat update.
                    self.path_approach_with_state(id, mine_pos, AIState::Attacking);
                    self.dozer_bored_mine_clear_events =
                        self.dozer_bored_mine_clear_events.saturating_add(1);
                }
            }
        }
    }

    pub fn honesty_dozer_bored_repair_ok(&self) -> bool {
        self.dozer_bored_repair_events > 0
    }

    /// C++ RebuildHoleBehavior::onDie / newWorkerRespawnProcess / dtor:
    /// always `TheGameLogic->destroyObject(worker)` so the UNSELECTABLE
    /// generated worker cannot leak when the hole or scaffold dies.
    fn destroy_rebuild_hole_worker(&mut self, hole_id: ObjectId) {
        let Some(wid) = self.objects.get(&hole_id).and_then(|h| h.rebuild_worker_id) else {
            return;
        };
        if self.objects.contains_key(&wid) {
            self.destroy_object(wid);
        }
        if let Some(h) = self.objects.get_mut(&hole_id) {
            h.rebuild_worker_id = None;
        }
    }

    /// C++ Object::onDie RECONSTRUCTING residual — transfer attackers back to hole
    /// and restart RebuildHole worker spawn process.
    pub fn handle_reconstructing_death(&mut self, destroyed_id: ObjectId) -> bool {
        let (is_recon, producer, template_name) = {
            let Some(o) = self.objects.get(&destroyed_id) else {
                return false;
            };
            (
                o.status.reconstructing,
                o.producer_id,
                o.template_name.clone(),
            )
        };
        if !is_recon {
            return false;
        }
        let Some(hole_id) = producer else {
            return false;
        };
        let Some(hole) = self.objects.get(&hole_id) else {
            return false;
        };
        if !hole.is_rebuild_hole {
            return false;
        }
        // C++ startRebuildProcess → newWorkerRespawnProcess(existingWorker).
        self.destroy_rebuild_hole_worker(hole_id);
        let Some(hole) = self.objects.get_mut(&hole_id) else {
            return false;
        };
        // Restart rebuild process residual.
        hole.rebuild_template_name = Some(template_name);
        hole.rebuild_reconstructing_id = None;
        hole.set_status_masked(false);
        hole.set_status_unselectable(false);
        hole.rebuild_ready_frame = self
            .frame
            .max(1)
            .saturating_add(REBUILD_HOLE_WORKER_RESPAWN_FRAMES);
        // Transfer attackers from lost reconstruction to hole.
        let n = self.transfer_attack(destroyed_id, hole_id);
        if n > 0 {
            self.rebuild_hole_attack_transfers =
                self.rebuild_hole_attack_transfers.saturating_add(n as u32);
        }
        self.rebuild_hole_recon_deaths = self.rebuild_hole_recon_deaths.saturating_add(1);
        true
    }

    /// C++ RebuildHoleBehavior::transferBombs residual.
    ///
    /// Sticky bombs / mines attached to `from_id` retarget to `to_id`.
    pub fn transfer_bombs(&mut self, from_id: ObjectId, to_id: ObjectId) -> usize {
        if from_id == to_id {
            return 0;
        }
        if !self.objects.contains_key(&to_id) {
            return 0;
        }
        let mut n = 0usize;
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            if id == from_id || id == to_id {
                continue;
            }
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            // StickyBombUpdate target residual stored as mine_data.attached_to.
            if let Some(md) = obj.mine_data.as_mut() {
                if md.attached_to == Some(from_id) {
                    md.attached_to = Some(to_id);
                    n = n.saturating_add(1);
                }
            }
            // Also retarget if attacking the old host as sticky residual.
            // Fail-closed: only mine-kind objects (mine_data present).
        }
        if n > 0 {
            self.rebuild_hole_bomb_transfers =
                self.rebuild_hole_bomb_transfers.saturating_add(n as u32);
        }
        n
    }

    /// C++ RebuildHoleExposeDie HoleName — authored module only.
    /// A template-name substring never fabricates USA/China/fake/Scud holes.
    pub(in super::super) fn rebuild_hole_name_for_template(
        &self,
        template_name: &str,
    ) -> Option<String> {
        self.templates
            .get(template_name)
            .and_then(|template| template.rebuild_hole_expose.as_ref())
            .map(|expose| expose.hole_name.clone())
            .filter(|name| !name.is_empty())
    }

    /// C++ RebuildHoleExposeDie::onDie residual — spawn hole for authored
    /// RebuildHoleExposeDie templates. Gate is Neutral / isPlayerActive /
    /// UNDER_CONSTRUCTION, not faction (captured GLA still exposes a hole).
    pub fn maybe_spawn_rebuild_hole(&mut self, destroyed_id: ObjectId) -> Option<ObjectId> {
        let (
            team,
            owner_player_id,
            pos,
            orient,
            template_name,
            under_construction,
            is_structure,
            is_hole,
            script_name,
            dying_geometry,
            dying_selection_radius,
            expose,
        ) = {
            let o = self.objects.get(&destroyed_id)?;
            (
                o.team,
                self.player_owner_for_host_object(o),
                o.get_position(),
                o.get_orientation(),
                o.template_name.clone(),
                o.status.under_construction,
                o.is_kind_of(KindOf::Structure),
                o.is_rebuild_hole,
                o.name.clone(),
                o.thing.geometry.clone(),
                o.selection_radius,
                o.thing.template.rebuild_hole_expose.clone(),
            )
        };
        // C++ RebuildHoleBehavior::onDie: hole death destroys the generated worker.
        if is_hole {
            self.destroy_rebuild_hole_worker(destroyed_id);
            return None;
        }
        // Death-start peel + destroy-list fallback: do not spawn a second crater.
        if self
            .objects
            .values()
            .any(|o| o.is_rebuild_hole && o.rebuild_spawner_id == Some(destroyed_id))
        {
            return None;
        }
        if !is_structure || under_construction {
            return None;
        }
        // C++ RebuildHoleExposeDie.cpp:108-110: controlling player != Neutral
        // && isPlayerActive() && !UNDER_CONSTRUCTION. No KINDOF_GLA / team==GLA test.
        if matches!(team, Team::Neutral) || owner_player_id.is_none() {
            return None;
        }
        let expose = expose.filter(|m| !m.hole_name.is_empty())?;
        let hole_name = expose.hole_name.clone();
        if !self.templates.contains_key(&hole_name) {
            let mut ht = ThingTemplate::new(&hole_name);
            ht.add_kind_of(KindOf::Structure)
                .set_health(REBUILD_HOLE_MAX_HEALTH_RESIDUAL);
            self.templates.insert(hole_name.clone(), ht);
        }
        // C++ setMaxHealth(modData->m_holeMaxHealth) when authored; else hole
        // template ActiveBody / residual 500.
        let hole_max_health = if expose.hole_max_health > 0.0 {
            expose.hole_max_health
        } else {
            self.templates
                .get(&hole_name)
                .map(|t| t.max_health)
                .filter(|&h| h > 0.0)
                .unwrap_or(REBUILD_HOLE_MAX_HEALTH_RESIDUAL)
        };
        // Wave 742: under construction sole-tick, pre-spawn hole entity on coupled
        // shadow and bind host ObjectId (entity-first). Non-sole / no-shadow falls
        // back to host create_object. Missing bind under sole is fail-closed via
        // host_spawn_rebuild_bound_object (Wave 741) unless opt-in.
        let gw_raw = if crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
            crate::gameworld_shadow::spawn_rebuild_hole_entity_if_coupled(
                &hole_name,
                [pos.x, pos.y, pos.z],
                orient,
                hole_max_health,
            )
        } else {
            None
        };
        let hole_id =
            self.host_spawn_rebuild_bound_object(&hole_name, team, owner_player_id, pos, gw_raw)?;
        if let Some(h) = self.objects.get_mut(&hole_id) {
            // A rebuild hole is a continuation of the destroyed building, not
            // a new team-level object.  Preserve exact ownership for the
            // later worker and reconstruction spawn chain.
            h.set_team_and_owner(team, owner_player_id);
            h.set_orientation(orient);
            h.set_status_under_construction(false);
            // Defer percent only when shadow sole-ticks construction; host-only
            // must set percent immediately or rebuild holes stay incomplete forever.
            if crate::gameworld_shadow::gameworld_construction_authority_live() {
                crate::game_logic::host_construction_progress_log::record(hole_id, 1.0, false, 0.0);
            } else {
                h.construction_percent = 1.0;
                crate::game_logic::host_construction_progress_log::record(hole_id, 1.0, false, 0.0);
            }
            Self::write_object_health_authority_aware(h, hole_max_health);
            h.health.maximum = hole_max_health;
            h.is_rebuild_hole = true;
            h.rebuild_template_name = Some(template_name);
            h.rebuild_spawner_id = Some(destroyed_id);
            // C++ RebuildHoleExposeDie.cpp:126 hole->setGeometryInfo(us->getGeometryInfo()).
            // Preserve the dying pad's collision / selection / pathfind footprint.
            let hole_pos = h.thing.geometry.position;
            h.thing.geometry = dying_geometry;
            h.thing.geometry.position = hole_pos;
            h.selection_radius = dying_selection_radius;
            h.rebuild_ready_frame = self
                .frame
                .max(1)
                .saturating_add(REBUILD_HOLE_WORKER_RESPAWN_FRAMES);
        }
        self.rebuild_hole_spawns = self.rebuild_hole_spawns.saturating_add(1);
        // C++ RebuildHoleExposeDie.cpp:131 TheScriptEngine->transferObjectName.
        if !script_name.is_empty() {
            let _ = self.transfer_script_object_name(destroyed_id, hole_id);
        }

        // C++ RebuildHoleExposeDie TransferAttackers residual (default true).
        if expose.transfer_attackers {
            let n = self.transfer_attack(destroyed_id, hole_id);
            if n > 0 {
                self.rebuild_hole_attack_transfers =
                    self.rebuild_hole_attack_transfers.saturating_add(n as u32);
            }
        }
        Some(hole_id)
    }

    /// C++ RebuildHoleBehavior::update residual:
    /// - hole health regen while waiting
    /// - spawn unselectable worker after WorkerRespawnDelay
    /// - worker starts reconstruction; hole masked while reconstructing
    /// - when reconstruction completes, hole is destroyed
    pub fn update_rebuild_holes(&mut self) {
        let now = self.frame;
        let dt = 1.0 / 30.0;
        let heal_frac = REBUILD_HOLE_HEALTH_REGEN_PERCENT_PER_SEC * dt;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.is_rebuild_hole)
            .map(|(id, _)| *id)
            .collect();
        let mut holes_to_remove: Vec<ObjectId> = Vec::new();
        // Wave 620: under construction sole-tick, GameWorld writeback records
        // rebuild-ready holes; host only starts worker/building for those IDs.
        let construction_sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
        // Wave 740: keep full ready events (GW entity-first worker/rebuild raws).
        let ready_by_hole: std::collections::HashMap<
            ObjectId,
            crate::game_logic::host_rebuild_ready_log::HostRebuildReadyEvent,
        > = if construction_sole {
            crate::game_logic::host_rebuild_ready_log::drain()
                .into_iter()
                .map(|ev| (ev.hole, ev))
                .collect()
        } else {
            std::collections::HashMap::new()
        };
        let ready_holes: std::collections::HashSet<ObjectId> =
            ready_by_hole.keys().copied().collect();
        for hole_id in ids {
            // Hole health regen residual (always while hole alive).
            if let Some(h) = self.objects.get_mut(&hole_id) {
                if h.health.current + 1e-3 < h.health.maximum {
                    let add = h.health.maximum * heal_frac;
                    h.heal(add);
                    self.rebuild_hole_heals = self.rebuild_hole_heals.saturating_add(1);
                }
            }

            // C++ newWorkerRespawnProcess: worker gone → restart delay residual.
            {
                let (worker_id, recon_id) = {
                    let Some(h) = self.objects.get(&hole_id) else {
                        continue;
                    };
                    (h.rebuild_worker_id, h.rebuild_reconstructing_id)
                };
                let worker_gone = match worker_id {
                    Some(wid) => self
                        .objects
                        .get(&wid)
                        .map(|w| !w.is_alive())
                        .unwrap_or(true),
                    None => false,
                };
                if worker_gone {
                    let recon_alive = match recon_id {
                        Some(rid) => self
                            .objects
                            .get(&rid)
                            .map(|b| b.is_alive())
                            .unwrap_or(false),
                        None => false,
                    };
                    if let Some(h) = self.objects.get_mut(&hole_id) {
                        h.rebuild_worker_id = None;
                        if !recon_alive {
                            h.rebuild_reconstructing_id = None;
                        }
                        // C++ newWorkerRespawnProcess always maskObject(FALSE).
                        h.set_status_masked(false);
                        h.rebuild_ready_frame = now
                            .max(1)
                            .saturating_add(REBUILD_HOLE_WORKER_RESPAWN_FRAMES);
                        self.rebuild_hole_worker_restarts =
                            self.rebuild_hole_worker_restarts.saturating_add(1);
                    }
                }
            }

            // If reconstructing building finished, destroy hole.// If reconstructing building finished, destroy hole.
            let recon_done = {
                let Some(h) = self.objects.get(&hole_id) else {
                    continue;
                };
                if let Some(rid) = h.rebuild_reconstructing_id {
                    match self.objects.get(&rid) {
                        None => {
                            // Building gone — clear and respawn worker residual next cycle.
                            true // treat as need reset
                        }
                        Some(b) if b.is_alive() && !b.status.under_construction => true,
                        _ => false,
                    }
                } else {
                    false
                }
            };
            if recon_done {
                let finished = self
                    .objects
                    .get(&hole_id)
                    .and_then(|h| h.rebuild_reconstructing_id)
                    .and_then(|rid| self.objects.get(&rid))
                    .map(|b| b.is_alive() && !b.status.under_construction)
                    .unwrap_or(false);
                if finished {
                    // C++ RebuildHoleBehavior.cpp:302 transferObjectName(hole, reconstructing).
                    if let Some(rid) = self
                        .objects
                        .get(&hole_id)
                        .and_then(|h| h.rebuild_reconstructing_id)
                    {
                        let _ = self.transfer_script_object_name(hole_id, rid);
                        if let Some(b) = self.objects.get_mut(&rid) {
                            b.set_status_reconstructing(false);
                            b.set_status_masked(false);
                        }
                    }
                    // Destroy residual unselectable worker if still around.
                    if let Some(wid) = self.objects.get(&hole_id).and_then(|h| h.rebuild_worker_id)
                    {
                        self.destroy_object(wid);
                    }
                    holes_to_remove.push(hole_id);
                    self.rebuild_hole_completes = self.rebuild_hole_completes.saturating_add(1);
                    continue;
                } else {
                    // Reconstructing object died — C++ newWorkerRespawnProcess(worker).
                    self.destroy_rebuild_hole_worker(hole_id);
                    if let Some(h) = self.objects.get_mut(&hole_id) {
                        h.rebuild_reconstructing_id = None;
                        h.set_status_masked(false);
                        h.rebuild_ready_frame = now
                            .max(1)
                            .saturating_add(REBUILD_HOLE_WORKER_RESPAWN_FRAMES);
                    }
                    continue;
                }
            }

            let Some(h) = self.objects.get(&hole_id) else {
                continue;
            };
            // Already reconstructing with a live worker — keep masked.
            // Worker-gone keeps reconstructing_id so a later spawn can
            // aiResumeConstruction instead of creating a second scaffold.
            if h.rebuild_reconstructing_id.is_some() && h.rebuild_worker_id.is_some() {
                continue;
            }
            if h.rebuild_ready_frame == 0 || now < h.rebuild_ready_frame {
                continue;
            }
            // Wave 620: under sole-tick, only start rebuild for IDs GW recorded ready.
            if construction_sole && !ready_holes.contains(&hole_id) {
                continue;
            }
            let team = h.team;
            let owner_player_id = self.player_owner_for_host_object(h);
            let pos = h.get_position();
            let orient = h.get_orientation();
            let rebuild_name = match h.rebuild_template_name.clone() {
                Some(n) => n,
                None => continue,
            };

            // Ensure worker template residual. Prefer existing INI GLAInfantryWorker.
            if !self.templates.contains_key(REBUILD_HOLE_WORKER_TEMPLATE) {
                let mut wt = ThingTemplate::new(REBUILD_HOLE_WORKER_TEMPLATE);
                wt.add_kind_of(KindOf::Infantry)
                    .add_kind_of(KindOf::Worker)
                    .set_health(100.0);
                self.templates
                    .insert(REBUILD_HOLE_WORKER_TEMPLATE.to_string(), wt);
            }
            // Wave 740: under construction sole-tick, bind host ObjectIds to
            // GameWorld pre-spawned worker + reconstruct entities when present.
            let ready_ev = ready_by_hole.get(&hole_id);
            // Wave 740: prefer GW spawn pose residual when writeback provided one.
            let pos = ready_ev
                .and_then(|e| e.spawn_pos)
                .map(|p| Vec3::new(p[0], p[1], p[2]))
                .unwrap_or(pos);
            let worker_id = self.host_spawn_rebuild_bound_object(
                REBUILD_HOLE_WORKER_TEMPLATE,
                team,
                owner_player_id,
                pos,
                ready_ev.and_then(|e| e.worker_entity_raw),
            );
            let Some(worker_id) = worker_id else {
                continue;
            };
            if let Some(w) = self.objects.get_mut(&worker_id) {
                w.set_team_and_owner(team, owner_player_id);
                w.set_status_unselectable(true);
                w.set_status_masked(false);
                if let Some(ev) = ready_ev {
                    w.set_orientation(ev.orientation);
                }
            }
            self.set_ai_state_decision_aware(worker_id, AIState::Constructing);
            self.rebuild_hole_workers = self.rebuild_hole_workers.saturating_add(1);

            // C++ ai->aiResumeConstruction when the scaffold is still standing.
            let existing_recon = self
                .objects
                .get(&hole_id)
                .and_then(|h| h.rebuild_reconstructing_id)
                .filter(|&rid| {
                    self.objects
                        .get(&rid)
                        .map(|b| b.is_alive() && b.status.under_construction)
                        .unwrap_or(false)
                });
            if let Some(rid) = existing_recon {
                if let Some(o) = self.objects.get_mut(&rid) {
                    o.producer_id = Some(hole_id);
                    o.builder_id = Some(worker_id);
                    o.set_under_construction_model_conditions(true);
                }
                if let Some(w) = self.objects.get_mut(&worker_id) {
                    w.target = Some(rid);
                    w.set_actively_constructing(true);
                }
                self.set_ai_state_decision_aware(worker_id, AIState::Constructing);
                if let Some(h) = self.objects.get_mut(&hole_id) {
                    h.rebuild_worker_id = Some(worker_id);
                    h.set_status_masked(true);
                    h.set_status_unselectable(true);
                }
                let n = self.transfer_attack(hole_id, rid);
                if n > 0 {
                    self.rebuild_hole_attack_transfers =
                        self.rebuild_hole_attack_transfers.saturating_add(n as u32);
                }
                let _ = self.transfer_bombs(hole_id, rid);
                continue;
            }

            // Spawn reconstructing building (C++ ai->construct residual).
            // Wave 740: entity-first bind when GW pre-spawned the structure.
            let Some(new_id) = self.host_spawn_rebuild_bound_object(
                &rebuild_name,
                team,
                owner_player_id,
                pos,
                ready_ev.and_then(|e| e.rebuild_entity_raw),
            ) else {
                self.destroy_object(worker_id);
                continue;
            };
            if let Some(o) = self.objects.get_mut(&new_id) {
                o.set_team_and_owner(team, owner_player_id);
                o.set_orientation(orient);
                o.set_status_under_construction(true);
                o.set_status_reconstructing(true);
                if crate::gameworld_shadow::gameworld_construction_authority_live() {
                    crate::game_logic::host_construction_progress_log::record(
                        new_id, 0.0, true, 0.0,
                    );
                } else {
                    o.construction_percent = 0.0;
                    crate::game_logic::host_construction_progress_log::record(
                        new_id, 0.0, true, 0.0,
                    );
                }
                o.set_under_construction_model_conditions(true);
                // C++ DozerAIUpdate::construct starts at 1 HP (DozerAIUpdate.cpp:1708).
                Self::write_object_health_authority_aware(o, 1.0);
                // C++ setProducer(hole) residual.
                o.producer_id = Some(hole_id);
                o.builder_id = Some(worker_id);
            }
            if let Some(w) = self.objects.get_mut(&worker_id) {
                // Construction target association stays host (not combat AttackTarget).
                w.target = Some(new_id);
                w.set_actively_constructing(true);
            }
            self.set_ai_state_decision_aware(worker_id, AIState::Constructing);
            if let Some(h) = self.objects.get_mut(&hole_id) {
                h.rebuild_worker_id = Some(worker_id);
                h.rebuild_reconstructing_id = Some(new_id);
                // C++ maskObject(TRUE) while reconstructing.
                h.set_status_masked(true);
                h.set_status_unselectable(true);
            }
            // C++ transferAttack(hole, reconstructing) residual.
            let n = self.transfer_attack(hole_id, new_id);
            if n > 0 {
                self.rebuild_hole_attack_transfers =
                    self.rebuild_hole_attack_transfers.saturating_add(n as u32);
            }
            // C++ transferBombs(reconstructing) residual.
            let _ = self.transfer_bombs(hole_id, new_id);
            self.rebuild_hole_reconstructs = self.rebuild_hole_reconstructs.saturating_add(1);
        }
        for hid in holes_to_remove {
            // C++ RebuildHoleBehavior.cpp:309 TheGameLogic->destroyObject(hole).
            // Hashmap-remove left radar / pathfind / GameWorld crater ghosts.
            self.destroy_object(hid);
        }
    }

    pub fn honesty_rebuild_hole_ok(&self) -> bool {
        self.rebuild_hole_spawns > 0
            && self.rebuild_hole_reconstructs > 0
            && self.rebuild_hole_workers > 0
    }

    pub fn honesty_rebuild_hole_heal_ok(&self) -> bool {
        self.rebuild_hole_heals > 0
    }

    pub fn honesty_dozer_bored_mine_clear_ok(&self) -> bool {
        self.dozer_bored_mine_clear_events > 0
    }

    pub fn honesty_sole_benefactor_repair_ok(&self) -> bool {
        self.sole_benefactor_repair_rejects > 0
    }

    pub fn honesty_dozer_cancel_task_ok(&self) -> bool {
        self.dozer_cancel_task_events > 0
    }

    pub fn honesty_construction_complete_clear_ok(&self) -> bool {
        self.construction_complete_clears > 0
    }

    pub fn is_object_being_sold(&self, id: ObjectId) -> bool {
        self.sell_list.iter().any(|s| s.id == id)
            || self
                .objects
                .get(&id)
                .map(|o| o.status.sold)
                .unwrap_or(false)
    }

    /// C++ `BuildAssistant::xferTheSellList` capture (id + sell frame).
    pub fn sell_list_for_snapshot(&self) -> Vec<(ObjectId, u32)> {
        self.sell_list
            .iter()
            .map(|entry| (entry.id, entry.sell_frame))
            .collect()
    }

    /// Restore BuildAssistant sell list so mid-sell buildings keep tearing down.
    pub fn restore_sell_list_from_snapshot(&mut self, entries: &[(ObjectId, u32)]) {
        self.sell_list = entries
            .iter()
            .copied()
            .map(|(id, sell_frame)| ObjectSellInfo { id, sell_frame })
            .collect();
    }

    pub fn honesty_actively_constructing_ok(&self) -> bool {
        self.actively_constructing_updates > 0
    }

    pub fn honesty_construction_model_condition_ok(&self) -> bool {
        self.construction_model_condition_updates > 0
    }

    pub fn honesty_unit_ready_ok(&self) -> bool {
        self.unit_ready_events > 0
    }
}
