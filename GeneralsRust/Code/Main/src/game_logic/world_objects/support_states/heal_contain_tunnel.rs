//! C++ HealContain and TunnelContain update behavior.
use super::super::super::*;

impl GameLogic {
    pub(super) fn tick_heal_contain_and_tunnel(&mut self) {
        use crate::game_logic::host_tunnel_network::{
            TUNNEL_FULL_HEAL_FRAMES, heal_contain_done, tunnel_tracker_heal_amount,
        };

        // C++ GarrisonContain::update: drop effectively-dead occupants so
        // FIREPOINT/STATION slots free before survivors pick a window.
        let dirty_garrisons =
            crate::game_logic::buildings::BuildingBehavior::sweep_dead_garrison_occupants(
                &mut self.objects,
                self.frame,
            );
        for cid in dirty_garrisons {
            self.recalc_garrison_apparent_controller(cid);
        }

        let mut heal_jobs: Vec<(ObjectId, u32, Vec<ObjectId>)> = Vec::new();
        for (&id, obj) in &self.objects {
            if !obj.thing().template.contain_module.kind.is_heal_contain() {
                continue;
            }
            if !obj.is_alive() || obj.status.under_construction {
                continue;
            }
            let frames = obj
                .thing()
                .template
                .contain_module
                .frames_for_full_heal
                .unwrap_or(0);
            let occupants = obj.contained_units();
            if occupants.is_empty() {
                continue;
            }
            heal_jobs.push((id, frames, occupants));
        }
        for (container_id, frames, occupants) in heal_jobs {
            for unit_id in occupants {
                let enter_frame = self
                    .tunnel_network
                    .contained_by_frame(unit_id)
                    .unwrap_or(self.frame);
                let contained_frames = self.frame.saturating_sub(enter_frame);
                let done = heal_contain_done(contained_frames, frames);
                let mut healed = false;
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    let amount =
                        tunnel_tracker_heal_amount(unit.health.maximum, contained_frames, frames);
                    if amount > 0.0 {
                        unit.heal(amount, &mut self.health_events);
                        healed = true;
                    }
                }
                if healed {
                    self.tunnel_network.record_heal_tick();
                }
                if !done {
                    continue;
                }
                if let Some(container) = self.objects.get_mut(&container_id) {
                    let _ = container.remove_occupant(unit_id);
                }
                // C++ HealContain::update → exitObjectViaDoor (ExitStart/End + path).
                self.walk_unit_via_open_contain_exit(unit_id, container_id);
                self.tunnel_network.clear_contained_by_frame(unit_id);
                self.tunnel_network.record_heal_auto_exit();
            }
        }

        let mut garrison_jobs: Vec<(u32, Vec<ObjectId>)> = Vec::new();
        for obj in self.objects.values() {
            if obj.thing().template.contain_module.kind
                != crate::game_logic::thing::ContainModuleKind::Garrison
            {
                continue;
            }
            // C++ GarrisonContain::update heals only when HealObjects=Yes
            // (`healObjects`). Retail bunkers author both HealObjects and
            // TimeForFullHeal — skip this unguarded sliver so they do not
            // double-heal with the HealObjects pass below.
            if obj.thing().template.contain_module.heal_objects {
                continue;
            }
            if !obj.is_alive() || obj.status.under_construction {
                continue;
            }
            let Some(frames) = obj.thing().template.contain_module.frames_for_full_heal else {
                continue;
            };
            let occupants = obj.contained_units();
            if occupants.is_empty() {
                continue;
            }
            garrison_jobs.push((frames, occupants));
        }
        for (frames, occupants) in garrison_jobs {
            for unit_id in occupants {
                let enter_frame = self
                    .tunnel_network
                    .contained_by_frame(unit_id)
                    .unwrap_or(self.frame);
                let contained_frames = self.frame.saturating_sub(enter_frame);
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    let amount =
                        tunnel_tracker_heal_amount(unit.health.maximum, contained_frames, frames);
                    if amount > 0.0 {
                        unit.heal(amount, &mut self.health_events);
                    }
                }
            }
        }

        // Each living TunnelContain::update heals the shared tracker (C++ per-entrance).
        let mut tunnel_ticks: Vec<(u32, u32)> = Vec::new();
        for obj in self.objects.values() {
            if !obj.is_alive() || obj.status.under_construction {
                continue;
            }
            let is_tunnel = obj.is_tunnel_network_style_container()
                || obj.thing().template.contain_module.kind.is_tunnel_contain();
            if !is_tunnel {
                continue;
            }
            let frames = obj
                .thing()
                .template
                .contain_module
                .frames_for_full_heal
                .unwrap_or(TUNNEL_FULL_HEAL_FRAMES);
            tunnel_ticks.push((obj.tunnel_system_key(), frames));
        }
        for (player_id, frames) in tunnel_ticks {
            let passengers = self.tunnel_network.contained_for_player(player_id);
            for unit_id in passengers {
                let enter_frame = self
                    .tunnel_network
                    .contained_by_frame(unit_id)
                    .unwrap_or(self.frame);
                let contained_frames = self.frame.saturating_sub(enter_frame);
                let mut healed = false;
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    let amount =
                        tunnel_tracker_heal_amount(unit.health.maximum, contained_frames, frames);
                    if amount > 0.0 {
                        unit.heal(amount, &mut self.health_events);
                        healed = true;
                    }
                }
                if healed {
                    self.tunnel_network.record_heal_tick();
                }
            }
        }

        // C++ GarrisonContain::update → healObjects when HealObjects=Yes.
        let mut garrison_jobs: Vec<(u32, Vec<ObjectId>)> = Vec::new();
        for obj in self.objects.values() {
            if obj.thing().template.contain_module.kind
                != crate::game_logic::ContainModuleKind::Garrison
            {
                continue;
            }
            if !obj.thing().template.contain_module.heal_objects {
                continue;
            }
            if !obj.is_alive() || obj.status.under_construction {
                continue;
            }
            let occupants = obj.contained_units();
            if occupants.is_empty() {
                continue;
            }
            let frames = obj
                .thing()
                .template
                .contain_module
                .frames_for_full_heal
                .unwrap_or(1);
            garrison_jobs.push((frames, occupants));
        }
        for (frames, occupants) in garrison_jobs {
            for unit_id in occupants {
                let enter_frame = self
                    .tunnel_network
                    .contained_by_frame(unit_id)
                    .unwrap_or(self.frame);
                let contained_frames = self.frame.saturating_sub(enter_frame);
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    let amount = gamelogic::object::contain::garrison_heal_single_amount(
                        unit.health.maximum,
                        contained_frames,
                        frames as f32,
                    );
                    if amount > 0.0 {
                        unit.heal(amount, &mut self.health_events);
                    }
                }
            }
        }
    }

    /// C++ `TunnelTracker::getCurNemesis` object-validity half.
    pub(super) fn resolved_tunnel_nemesis(&mut self, player_id: u32) -> Option<ObjectId> {
        let Some(id) = self
            .tunnel_network
            .get_cur_nemesis_id(player_id, self.frame)
        else {
            return None;
        };
        let Some(obj) = self.objects.get(&id) else {
            self.tunnel_network.clear_nemesis(player_id);
            return None;
        };
        if !obj.is_alive() || obj.status.effectively_dead || obj.is_effectively_stealthed() {
            self.tunnel_network.clear_nemesis(player_id);
            return None;
        }
        Some(id)
    }

    /// C++ TunnelContain::update nemesis write + AITNGuard sally from the pool.
    pub(super) fn tick_tunnel_network_nemesis(&mut self) {
        use crate::game_logic::KindOf;
        use gamelogic::common::Relationship;

        // C++ AITNGuardIdleState::lookForInnerTarget (goal victim first) then
        // TunnelContain::update last-damage write. Scan-rate is AIData
        // m_guardEnemyScanRate (host_guard_enemy_scan_rate), not a hard 30.
        let scan_rate = self.host_guard_enemy_scan_rate();
        let mut writes: Vec<(u32, ObjectId, bool)> = Vec::new();
        for obj in self.objects.values() {
            if !obj.is_alive() || obj.status.under_construction {
                continue;
            }
            let is_tunnel = obj.is_tunnel_network_style_container()
                || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                    &obj.template_name,
                );
            if !is_tunnel {
                continue;
            }
            let player_id = obj.tunnel_system_key();
            if let Some(vid) = obj.target {
                writes.push((player_id, vid, false));
            }
            let Some(src) = obj.last_damage_source else {
                continue;
            };
            let Some(ts) = obj.last_damage_timestamp else {
                continue;
            };
            // C++ `if (info && info->m_noEffect) continue;` — status / subdual
            // / kill-pilot / kill-garrisoned do not rally the pool.
            if obj
                .last_damage_info_type
                .is_some_and(|t| !t.is_health_damaging())
            {
                continue;
            }
            if ts.saturating_add(scan_rate) <= self.frame {
                continue;
            }
            writes.push((player_id, src, true));
        }
        for (player_id, src, from_damage) in writes {
            let Some((v, s, inf, air, att_team, att_alive, att_owner, tunnel_rel_enemies)) = self
                .objects
                .get(&src)
                .map(|attacker| {
                    (
                        attacker.is_kind_of(KindOf::Vehicle),
                        attacker.is_kind_of(KindOf::Structure),
                        attacker.is_kind_of(KindOf::Infantry),
                        attacker.is_kind_of(KindOf::Aircraft),
                        attacker.team,
                        attacker.is_alive(),
                        attacker.owner_player_id,
                    )
                })
                .and_then(|(v, s, inf, air, att_team, att_alive, att_owner)| {
                    if !att_alive {
                        return None;
                    }
                    let tunnel = self.objects.values().find(|o| {
                        o.tunnel_system_key() == player_id
                            && o.is_alive()
                            && (o.is_tunnel_network_style_container()
                                || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                                    &o.template_name,
                                ))
                    })?;
                    let rel = match (tunnel.owner_player_id, att_owner) {
                        (Some(a), Some(b)) => self.player_relationship(a, b),
                        _ => Relationship::Neutral,
                    };
                    let enemies = rel == Relationship::Enemies
                        || (rel == Relationship::Neutral
                            && tunnel.team != att_team
                            && tunnel.team != Team::Neutral
                            && att_team != Team::Neutral);
                    Some((v, s, inf, air, att_team, att_alive, att_owner, enemies))
                })
            else {
                continue;
            };
            let _ = (att_team, att_alive, att_owner);
            if !tunnel_rel_enemies {
                continue;
            }
            // C++ lookForInnerTarget: getAbleToAttackSpecificObject(
            // ATTACK_TUNNEL_NETWORK_GUARD) before setNemesisID + team target.
            // Goal-object victims skip this gate (C++). Empty pool still writes
            // (TunnelContain::update).
            let able_guard = if from_damage {
                self.first_tn_guard_able_to_attack(player_id, src)
            } else {
                None
            };
            if from_damage && self.tn_guard_pool_has_occupants(player_id) && able_guard.is_none() {
                continue;
            }
            self.tunnel_network
                .update_nemesis(player_id, src, v, s, inf, air, self.frame);
            if let Some(gid) = able_guard {
                self.set_host_team_common_target(gid, Some(src));
            }
        }

        // C++ AITNGuardIdleState::update (AITNGuard.cpp:682-707):
        // lookForInnerTarget → findBestTunnel(nemesis) + isExitBusy +
        // exitObjectInAHurry; else if not contained → Return / enter nearest.
        let players: Vec<u32> = self.tunnel_network.occupant_player_ids();
        for player_id in players {
            let Some(nemesis) = self.resolved_tunnel_nemesis(player_id) else {
                continue;
            };
            let Some(nemesis_pos) = self.objects.get(&nemesis).map(|o| o.get_position()) else {
                continue;
            };
            let passengers = self.tunnel_network.contained_for_player(player_id);
            let mut sally: Vec<(ObjectId, ObjectId)> = Vec::new();
            for uid in passengers {
                let Some(unit) = self.objects.get(&uid) else {
                    continue;
                };
                if !unit.is_alive() || unit.target == Some(nemesis) {
                    continue;
                }
                if !self.is_tunnel_network_guard_unit(uid) {
                    continue;
                }
                // C++ findBestTunnel(ownerPlayer, nemesis->getPosition()).
                let Some(exit_tunnel) = self.find_best_tunnel(player_id, nemesis_pos) else {
                    continue;
                };
                // C++ goalExitInterface->isExitBusy() → STATE_SLEEP(0).
                if self.tunnel_exit_is_busy(exit_tunnel) {
                    continue;
                }
                sally.push((uid, exit_tunnel));
            }
            for (uid, exit_tunnel) in sally {
                let _ = self.exit_tunnel_network_unit(uid, exit_tunnel);
                self.tunnel_network.mark_sally(uid);
                let (start, mut door_end, layer, yaw, next_exit, exit_count) = self
                    .objects
                    .get(&exit_tunnel)
                    .map(|container| {
                        let which = if container.which_exit_path > 0 {
                            container.which_exit_path
                        } else {
                            1
                        };
                        let n = container.transport_number_of_exit_paths();
                        let (start, end, next) =
                            super::super::super::world_combat::open_contain_exit_path(
                                container, which, n,
                            );
                        (
                            start,
                            end,
                            container.pathfind_layer,
                            container.get_orientation(),
                            next,
                            n,
                        )
                    })
                    .unwrap_or((nemesis_pos, nemesis_pos, 1, 0.0, 1, 1));
                // OpenContain.cpp only advances m_whichExitPath when numberExits > 1.
                if exit_count > 1 {
                    if let Some(container) = self.objects.get_mut(&exit_tunnel) {
                        container.which_exit_path = next_exit;
                    }
                }
                if let Some(container) = self.objects.get_mut(&exit_tunnel) {
                    let time = gamelogic::object::contain::open_contain::leftover_open_contain_resolved_door_open_time(
                        &container.template_name,
                        container.thing().template.contain_module.door_open_time,
                    );
                    if time > 0 {
                        let pulse = gamelogic::object::contain::open_contain::leftover_open_contain_arm_exit_door(
                            time,
                        );
                        container.door_close_countdown = pulse.countdown;
                        super::super::super::world_combat::apply_leftover_open_contain_door_pulse(
                            container, pulse,
                        );
                    }
                }
                if let Some(unit) = self.objects.get_mut(&uid) {
                    unit.set_contained_by(None);
                    unit.set_position(start);
                    unit.set_orientation(yaw);
                    // OpenContain.cpp:1086 setLayer before adjustToPossibleDestination.
                    unit.pathfind_layer = layer;
                    unit.ignored_obstacle_id = None;
                }
                let _ = self.adjust_to_possible_destination(uid, &mut door_end);
                if let Some(unit) = self.objects.get_mut(&uid) {
                    // doQuickExit: path [end, end], guard machine stays locked.
                    unit.movement.path.clear();
                    unit.movement.path.push(door_end);
                    unit.movement.path.push(door_end);
                    unit.movement.current_path_index = 0;
                    unit.movement.target_position = Some(door_end);
                    // AI_FOLLOW_EXITPRODUCTION_PATH (AIStates.cpp:3250-3273).
                    unit.can_path_through_units = true;
                    unit.is_attack_path = false;
                    unit.adjust_destinations = false;
                    unit.refresh_follow_path_extra_distance();
                    unit.ignore_collisions_with = None;
                    unit.ignore_collisions_until_frame = self.frame.saturating_add(30);
                    unit.ignored_obstacle_id = None;
                    // Install the temporary overlay before changing its host
                    // locomotion state so the nested Guard remains suspended.
                    unit.unit_ai_runtime
                        .set_quick_exit_deadline(Some(self.frame.wrapping_add(300)));
                    unit.set_ai_state(AIState::Moving);
                    unit.set_status_moving(true);
                    if crate::gameworld_shadow::gameworld_movement_authority_live() {
                        crate::game_logic::host_move_log::record(
                            unit.id,
                            Some([door_end.x, door_end.y, door_end.z]),
                        );
                        unit.record_host_movement();
                    }
                }
                self.register_ground_path_goal(uid, door_end);
            }
        }

        // C++ AITNGuardIdleState no-target → AITNGuardReturnState
        // findBestTunnel(owner pos) + AIEnterState.
        let returning: Vec<ObjectId> = self.tunnel_network.sally_unit_ids();
        for uid in returning {
            let Some((player_id, entering, pos)) = self.objects.get(&uid).and_then(|unit| {
                if !unit.is_alive() || unit.status.effectively_dead {
                    return None;
                }
                if unit.is_contained() {
                    return None;
                }
                Some((
                    unit.tunnel_system_key(),
                    matches!(unit.ai_state, AIState::Entering),
                    unit.get_position(),
                ))
            }) else {
                self.tunnel_network.clear_sally(uid);
                continue;
            };
            // AIStateMachine::updateStateMachine suspends the base Guard
            // while its temporary exit state continues, even if the tunnel
            // forgets its nemesis. Do not replace that route with Enter.
            if self
                .objects
                .get(&uid)
                .is_some_and(Object::has_guard_quick_exit_overlay)
            {
                continue;
            }
            if self.resolved_tunnel_nemesis(player_id).is_some() {
                continue;
            }
            if entering {
                continue;
            }
            // C++ Return::update does not consult the unit's current attack
            // target — only team victim / tracker nemesis. After chase the
            // machine always returns to findBestTunnel(owner pos).
            let Some(best) = self.find_best_tunnel(player_id, pos) else {
                continue;
            };
            if let Some(o) = self.objects.get_mut(&uid) {
                o.target = Some(best);
                o.set_order_target(Some(best));
                o.set_ai_state(AIState::Entering);
                o.ignored_obstacle_id = Some(best);
            }
            if let Some(tpos) = self.objects.get(&best).map(|t| t.get_position()) {
                self.path_approach_with_state_ignoring(uid, tpos, AIState::Entering, Some(best));
            }
        }
    }

    /// C++ `findBestTunnel` (AITNGuard.cpp:84-105).
    fn find_best_tunnel(&self, player_id: u32, pos: glam::Vec3) -> Option<ObjectId> {
        let registered = self.tunnel_network.tunnel_ids_for(player_id);
        let mut candidates: Vec<(ObjectId, f32, f32)> = Vec::new();
        if !registered.is_empty() {
            for &tid in registered {
                if let Some(t) = self.objects.get(&tid) {
                    if t.is_alive() && !t.status.sold && self.is_living_tunnel_network(t) {
                        let p = t.get_position();
                        candidates.push((tid, p.x, p.z));
                    }
                }
            }
        } else {
            for (id, o) in &self.objects {
                if o.tunnel_system_key() == player_id
                    && o.is_alive()
                    && !o.status.sold
                    && self.is_living_tunnel_network(o)
                {
                    let p = o.get_position();
                    candidates.push((*id, p.x, p.z));
                }
            }
        }
        crate::game_logic::host_tunnel_network::find_best_tunnel_xz(candidates, pos.x, pos.z)
    }

    /// C++ `OpenContain::isExitBusy` is FALSE for TunnelContain.
    fn tunnel_exit_is_busy(&self, tunnel_id: ObjectId) -> bool {
        let Some(t) = self.objects.get(&tunnel_id) else {
            return true;
        };
        if !t.is_alive() || t.status.sold {
            return true;
        }
        if t.uses_transport_contain_exit_busy() {
            return t.is_transport_exit_busy(self.frame);
        }
        false
    }

    fn is_living_tunnel_network(&self, obj: &crate::game_logic::Object) -> bool {
        obj.is_tunnel_network_style_container()
            || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                &obj.template_name,
            )
    }

    fn is_tunnel_network_guard_unit(&self, uid: ObjectId) -> bool {
        let Some(unit) = self.objects.get(&uid) else {
            return false;
        };
        let guard_is_tunnel = unit.guard_target.is_some_and(|gid| {
            self.objects
                .get(&gid)
                .is_some_and(|g| self.is_living_tunnel_network(g))
        });
        let is_defender =
            crate::game_logic::host_rpg_trooper::is_rpg_trooper_template(&unit.template_name);
        guard_is_tunnel || is_defender
    }

    fn tn_guard_pool_has_occupants(&self, player_id: u32) -> bool {
        self.tunnel_network
            .contained_for_player(player_id)
            .into_iter()
            .any(|uid| self.is_tunnel_network_guard_unit(uid))
    }

    /// C++ AITNGuardIdleState::lookForInnerTarget able-to-attack gate
    /// (`ATTACK_TUNNEL_NETWORK_GUARD`) over the shared pool.
    fn first_tn_guard_able_to_attack(
        &self,
        player_id: u32,
        victim_id: ObjectId,
    ) -> Option<ObjectId> {
        for uid in self.tunnel_network.contained_for_player(player_id) {
            if !self.is_tunnel_network_guard_unit(uid) {
                continue;
            }
            match self.get_able_to_attack_specific_object(
                uid,
                victim_id,
                AbleToAttackType::TunnelNetworkGuard,
                false,
            ) {
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                    return Some(uid);
                }
                _ => {}
            }
        }
        None
    }

    /// C++ `ChinookAIUpdate::isAvailableForSupplying` (ChinookAIUpdate.cpp:982-991).
    pub(super) fn collector_available_for_supplying(&self, object_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&object_id) else {
            return false;
        };
        let is_chinook =
            crate::game_logic::host_supply_gather::is_chinook_supply_collector(&obj.template_name)
                || obj.chinook_ai.is_some()
                || obj.is_combat_chinook_style_container();
        let wanting = obj
            .chinook_ai
            .as_ref()
            .is_some_and(|ai| ai.wanting_enter_or_exit);
        crate::game_logic::host_supply_gather::chinook_available_for_supplying(
            is_chinook,
            obj.contained_units().len(),
            wanting,
            obj.is_overlord_style_container(),
        )
    }

    /// C++ `WorkerAIUpdate::update` harvest branch (WorkerAIUpdate.cpp:283-287).
    pub(super) fn arm_worker_harvest_mine_clearing(&mut self, object_id: ObjectId) {
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !crate::game_logic::host_gla_worker::is_gla_worker_template(&obj.template_name) {
            return;
        }
        obj.set_weapon_set_mine_clearing_detail(true);
    }

    /// C++ / leftover WANTING update: cargo → center; empty → warehouse;
    /// neither → regroup. Never harvests while carrying.
    pub(super) fn route_supply_wanting(
        &mut self,
        object_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        can_move: bool,
    ) {
        use crate::game_logic::host_supply_gather::{WantingDockTarget, wanting_dock_target};
        const SUPPLY_BOX_VALUE: u32 =
            crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX as u32;
        let cash = self
            .objects
            .get(&object_id)
            .map(|o| o.stored_resources.supplies)
            .unwrap_or(0);
        let number_boxes = (cash / SUPPLY_BOX_VALUE.max(1)) as i32;
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.supply_truck_state = SupplyTruckState::Wanting;
        }
        match wanting_dock_target(number_boxes) {
            WantingDockTarget::Center => {
                let has_center = self
                    .preferred_or_allied_supply_center(object_id, team, owner_player_id, position)
                    .is_some();
                if has_center && can_move {
                    self.begin_return_to_supply_center(object_id, team, owner_player_id, position);
                } else if has_center {
                    self.set_ai_state_decision_aware(object_id, AIState::ReturningResources);
                } else {
                    self.begin_supply_regroup(object_id, team, owner_player_id, position);
                }
            }
            WantingDockTarget::Warehouse => {
                let scan = self.collector_warehouse_scan(object_id, owner_player_id);
                if let Some(next) =
                    self.find_nearest_harvestable_supply_within(team, position, scan, object_id)
                {
                    let warehouse = self.objects.get(&next).is_some_and(|s| {
                        s.thing().template.dock_kind == crate::game_logic::DockKind::SupplyWarehouse
                            || s.thing().template.dock_delete_when_empty
                            || s.template_name.to_ascii_lowercase().contains("supplypile")
                    });
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_order_target(Some(next));
                        obj.supply_truck_state = SupplyTruckState::Wanting;
                    }
                    self.set_ai_state_decision_aware(object_id, AIState::Gathering);
                    if warehouse {
                        let _ = self.try_claim_dock(next, object_id);
                    } else if let Some(dest) = self.objects.get(&next).map(|s| s.get_position()) {
                        self.path_approach_with_state(object_id, dest, AIState::Gathering);
                    }
                    return;
                }
                self.begin_supply_regroup(object_id, team, owner_player_id, position);
            }
        }
    }

    /// C++ Idle `isForcedIntoWantingState` → Wanting, and Regrouping success → Wanting.
    pub(super) fn tick_supply_force_wanting(
        &mut self,
        object_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        can_move: bool,
    ) {
        let Some(obj) = self.objects.get(&object_id) else {
            return;
        };
        if obj.thing().template.supply_truck_metadata.is_none() {
            return;
        }
        let force = obj.supply_truck_force_pending;
        let state = obj.supply_truck_state;
        if !force && state != SupplyTruckState::Regrouping && state != SupplyTruckState::Wanting {
            return;
        }
        let still_moving =
            obj.status.moving || obj.movement.current_path_index < obj.movement.path.len();
        // C++ RegroupingState::update succeeds only once AI is idle.
        if still_moving {
            return;
        }
        if !self.collector_available_for_supplying(object_id) {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.supply_truck_force_pending = false;
            }
            return;
        }
        // C++ Wanting onEnter: setForceWantingState(false) — one try.
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.supply_truck_force_pending = false;
            obj.supply_truck_state = SupplyTruckState::Wanting;
        }
        self.arm_worker_harvest_mine_clearing(object_id);
        self.route_supply_wanting(object_id, team, owner_player_id, position, can_move);
    }

    pub(super) fn collector_warehouse_scan(
        &self,
        object_id: ObjectId,
        owner_player_id: Option<u32>,
    ) -> Option<f32> {
        let authored = self
            .objects
            .get(&object_id)
            .and_then(|object| object.thing().template.supply_truck_metadata)
            .map(|metadata| metadata.warehouse_scan_distance)?;
        let is_computer =
            owner_player_id.is_some_and(|pid| self.ai_manager.ai_players.contains_key(&pid));
        Some(crate::game_logic::host_supply_gather::warehouse_scan_distance(authored, is_computer))
    }

    pub(super) fn begin_supply_regroup(
        &mut self,
        object_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        from: Vec3,
    ) {
        use crate::game_logic::host_repair::{DozerFindPositionQuery, find_position_around_dozer};
        use crate::game_logic::host_supply_gather::REGROUP_SUCCESS_DISTANCE_SQUARED;
        use gamelogic::ai::pathfind_astar::PathfindCellType;
        // C++ RegroupingState::onEnter clears the ignore before the search.
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.ignored_obstacle_id = None;
        }
        let Some((dest_pos, radius)) = self.find_supply_regroup_target(team, owner_player_id, from)
        else {
            self.stop_attack_decision_aware(object_id);
            self.set_ai_state_decision_aware(object_id, AIState::Idle);
            return;
        };
        let truck_r = self
            .objects
            .get(&object_id)
            .map(|obj| {
                if obj.thing().template.geometry_info.authored {
                    obj.thing().template.geometry_info.bounding_circle_radius()
                } else {
                    obj.selection_radius
                }
            })
            .unwrap_or(0.0);
        let dx = dest_pos.x - from.x;
        let dz = dest_pos.z - from.z;
        let center_dist = (dx * dx + dz * dz).sqrt();
        // C++ FROM_BOUNDINGSPHERE_2D against 15. Overlapping spheres stay put.
        let gap = (center_dist - radius - truck_r).max(0.0);
        let goal = if gap * gap > REGROUP_SUCCESS_DISTANCE_SQUARED {
            use crate::game_logic::host_repair::DOZER_FIND_POSITION_OVERLAP_SPHERE;
            let grid_live = self.pathfinding_system.grid.width() > 0
                && self.pathfinding_system.grid.height() > 0;
            let is_cliff = |p: Vec3| {
                if let Some(t) = self.terrain.as_ref() {
                    if t.is_cliff_at_world(p) {
                        return true;
                    }
                }
                if let Ok(tl) = self.world_services.terrain().read() {
                    return tl.is_cliff_cell(p.x, p.z);
                }
                false
            };
            let is_underwater = |p: Vec3| self.is_underwater_at(p);
            let is_impassable = |p: Vec3| {
                if !grid_live {
                    return false;
                }
                let cell = self.pathfinding_system.grid.world_to_grid(p);
                self.pathfinding_system.grid.cell_type(cell) == PathfindCellType::Impassable
            };
            let overlaps_object = |p: Vec3| {
                self.objects.values().any(|o| {
                    if !o.is_alive() {
                        return false;
                    }
                    let r = o.selection_radius.max(0.0) + DOZER_FIND_POSITION_OVERLAP_SPHERE;
                    let d = o.get_position();
                    let dx = d.x - p.x;
                    let dz = d.z - p.z;
                    dx * dx + dz * dz <= r * r
                })
            };
            let query = DozerFindPositionQuery {
                source: from,
                is_cliff: Some(&is_cliff),
                is_impassable: Some(&is_impassable),
                is_underwater: Some(&is_underwater),
                overlaps_object: Some(&overlaps_object),
                ..DozerFindPositionQuery::default()
            };
            find_position_around_dozer(dest_pos, &query)
        } else {
            None
        };
        if let Some(approach) = goal {
            self.path_approach_with_state(object_id, approach, AIState::Moving);
        }
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.supply_truck_state = SupplyTruckState::Regrouping;
            object.supply_truck_force_pending = true;
            object.supply_truck_next_dock_action_frame = 0;
        }
    }

    fn find_supply_regroup_target(
        &self,
        team: Team,
        owner_player_id: Option<u32>,
        from: Vec3,
    ) -> Option<(Vec3, f32)> {
        let mut best_cash: Option<(f32, Vec3, f32)> = None;
        let mut best_cc: Option<(f32, Vec3, f32)> = None;
        let mut best_struct: Option<(f32, Vec3, f32)> = None;
        for obj in self.objects.values() {
            if !obj.is_alive() || obj.status.destroyed || obj.team != team {
                continue;
            }
            if owner_player_id.is_some()
                && self.player_owner_for_host_object(obj) != owner_player_id
            {
                continue;
            }
            if !obj.is_constructed() {
                continue;
            }
            let pos = obj.get_position();
            let radius = if obj.thing().template.geometry_info.authored {
                obj.thing().template.geometry_info.bounding_circle_radius()
            } else {
                obj.selection_radius
            };
            let dx = pos.x - from.x;
            let dz = pos.z - from.z;
            let dist2 = dx * dx + dz * dz;
            let is_cash = obj.is_kind_of(KindOf::SupplyCenter)
                || obj.is_kind_of(KindOf::FSSupplyCenter)
                || obj.thing().template.dock_kind == crate::game_logic::DockKind::SupplyCenter;
            let is_cc = obj.is_kind_of(KindOf::CommandCenter);
            let is_struct = obj.is_kind_of(KindOf::Structure);
            if is_cash && best_cash.is_none_or(|(d, _, _)| dist2 < d) {
                best_cash = Some((dist2, pos, radius));
            }
            if is_cc && best_cc.is_none_or(|(d, _, _)| dist2 < d) {
                best_cc = Some((dist2, pos, radius));
            }
            if is_struct && best_struct.is_none_or(|(d, _, _)| dist2 < d) {
                best_struct = Some((dist2, pos, radius));
            }
        }
        best_cash
            .or(best_cc)
            .or(best_struct)
            .map(|(_, pos, radius)| (pos, radius))
    }
}
